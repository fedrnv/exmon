//! Explicit one-shot worker. Preparation, source factories and submissions never run on UI.
use exmon_capture::{
    adapter::{Availability, Cancellation, Error as BackendError},
    can::Frame,
    transmit::{Completion, Identity, Info, Kind, Packet, Port, Progress},
};
use exmon_domain::{
    Direction,
    config::{Framing, Protocol},
    observation::{Metadata, Observation},
};
use exmon_protocol::{checksum::Registry, codec::Codec, outbound};
use std::{
    collections::{BTreeMap, VecDeque},
    fmt,
    net::SocketAddr,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, SyncSender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
const REVOKED: u64 = 1;
const SUBMITTED: u64 = 2;
const MAX_BYTES: usize = 1_048_576;

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Binding {
    pub session: u64,
    pub interface: u64,
    pub epoch: u64,
    pub configuration: u64,
    pub protocol: u64,
}
impl fmt::Debug for Binding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("TxBinding([REDACTED])")
    }
}
impl Binding {
    fn valid(self) -> bool {
        [
            self.session,
            self.interface,
            self.epoch,
            self.configuration,
            self.protocol,
        ]
        .into_iter()
        .all(|v| v != 0)
    }
}
#[derive(Clone)]
pub struct Ticket {
    owner: Arc<()>,
    generation: u64,
    binding: Binding,
}
impl Ticket {
    pub fn binding(&self) -> Binding {
        self.binding
    }
}
impl fmt::Debug for Ticket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("TxTicket([REDACTED])")
    }
}
pub enum Expected {
    Exact(Identity),
    Tcp { connection: u64, remote: SocketAddr },
    Udp { remote: SocketAddr },
}
impl Expected {
    fn matches(&self, id: &Identity) -> bool {
        match (self, id) {
            (Self::Exact(a), b) => a == b,
            (
                Self::Tcp {
                    connection: a,
                    remote: r,
                },
                Identity::Tcp {
                    connection: b,
                    remote: s,
                    ..
                },
            ) => a == b && r == s,
            (Self::Udp { remote: r }, Identity::Udp { remote: s, .. }) => r == s,
            _ => false,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Failure {
    Queue,
    Stale,
    Permission,
    Disconnected,
    Binding,
    Template,
    Validation,
    Framing,
    Payload,
    Capacity,
    Exhausted,
    Backend(BackendError),
}
impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "transmission: {self:?}")
    }
}
impl std::error::Error for Failure {}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Bytes(Kind),
    HidUnnumbered,
    Can { id_word: u32, dlc: u8 },
}
impl fmt::Debug for Format {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TxFormat({:?})", self.kind())
    }
}
impl Format {
    fn kind(self) -> Kind {
        match self {
            Self::Bytes(k) => k,
            Self::HidUnnumbered => Kind::Hid,
            Self::Can { .. } => Kind::Can,
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct SourceRef {
    pub session: u64,
    pub interface: u64,
    pub epoch: u64,
    pub observation: u64,
}
impl fmt::Debug for SourceRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("TxSourceRef([REDACTED])")
    }
}
pub struct Structured {
    pub protocol: Arc<Protocol>,
    pub registry: Registry,
    pub header: Vec<u8>,
}
pub struct TemplateSpec {
    pub id: u64,
    pub revision: u64,
    pub protocol_revision: u64,
    pub format: Format,
    pub body: Vec<u8>,
    pub structured: Option<Structured>,
    pub source: Option<SourceRef>,
}
/// Structural count/byte admission only, no decoding or extension calls. Charge
/// conservatively includes compiled index/node overhead and escaping copies.
fn charge(spec: &TemplateSpec) -> Result<usize, Failure> {
    use exmon_domain::fields::{Literal, Operation};
    fn literal(v: &Literal) -> usize {
        match v {
            Literal::Bytes { value } => value.len(),
            Literal::Text { value } => value.len(),
            _ => 16,
        }
    }
    if spec.body.len() > 65536 {
        return Err(Failure::Payload);
    }
    let mut cost = 8192usize.saturating_add(spec.body.len().saturating_mul(4));
    if let Some(s) = &spec.structured {
        let p = &s.protocol;
        if s.header.len() > 65536
            || p.id.len() > 128
            || p.name.len() > 128
            || p.fields.len() > 256
            || p.checksums.len() > 32
            || p.classification.prefix.len() > 65536
            || p.classification.mask.len() > 65536
        {
            return Err(Failure::Capacity);
        }
        p.framing.limit(p.escape).map_err(|_| Failure::Framing)?;
        cost = cost.saturating_add(s.header.len().saturating_mul(4));
        cost = cost.saturating_add(
            (p.classification.prefix.len() + p.classification.mask.len()).saturating_mul(4),
        );
        for f in &p.fields {
            if f.enumeration.len() > 256 || f.derived.as_ref().is_some_and(|ops| ops.len() > 64) {
                return Err(Failure::Capacity);
            }
            let mut bytes = std::mem::size_of_val(f)
                .saturating_add(f.id.len())
                .saturating_add(f.unit.len());
            for v in [
                f.constant.as_ref(),
                f.constraints.min.as_ref(),
                f.constraints.max.as_ref(),
            ]
            .into_iter()
            .flatten()
            {
                bytes = bytes.saturating_add(literal(v));
            }
            for e in &f.enumeration {
                bytes = bytes
                    .saturating_add(std::mem::size_of_val(e))
                    .saturating_add(e.name.len())
                    .saturating_add(literal(&e.value));
            }
            for op in f.derived.iter().flatten() {
                bytes = bytes
                    .saturating_add(std::mem::size_of_val(op))
                    .saturating_add(match op {
                        Operation::Field { id } => id.len(),
                        Operation::Literal { value } => literal(value),
                        _ => 0,
                    });
            }
            cost = cost.saturating_add(bytes.saturating_mul(4));
            if cost > MAX_BYTES {
                return Err(Failure::Capacity);
            }
        }
        for c in &p.checksums {
            if c.ranges.len() > 32 {
                return Err(Failure::Capacity);
            }
            cost = cost.saturating_add(
                (std::mem::size_of_val(c) + c.id.len() + c.algorithm.len() + c.ranges.len() * 8)
                    .saturating_mul(4),
            );
        }
    }
    if cost > MAX_BYTES {
        Err(Failure::Capacity)
    } else {
        Ok(cost)
    }
}
impl TemplateSpec {
    /// Clone normalized immutable message data, never a send action or inferred target.
    pub fn incoming(
        id: u64,
        revision: u64,
        protocol_revision: u64,
        format: Format,
        body: &[u8],
        structured: Structured,
        source: SourceRef,
    ) -> Result<Self, Failure> {
        if body.len() > 65536 {
            return Err(Failure::Payload);
        }
        Ok(Self {
            id,
            revision,
            protocol_revision,
            format,
            body: body.to_vec(),
            structured: Some(structured),
            source: Some(source),
        })
    }
}
#[derive(Debug, Clone, Default)]
pub struct Validation {
    pub fields: Vec<(usize, exmon_protocol::fields::Issue)>,
    pub checksums: Vec<(usize, exmon_protocol::checksum::Issue)>,
}
struct Template {
    spec: TemplateSpec,
    wire: Vec<u8>,
    frame: Option<Frame>,
    codec: Option<Codec>,
    gap_ns: u64,
    charge: usize,
}
impl Template {
    fn new(mut spec: TemplateSpec) -> Result<Self, (Failure, Validation)> {
        let fail = |e| (e, Validation::default());
        let charge = charge(&spec).map_err(fail)?;
        if spec.format == Format::Bytes(Kind::Can) {
            return Err(fail(Failure::Payload));
        }
        if spec.id == 0
            || spec.revision == 0
            || spec.protocol_revision == 0
            || spec.body.len() > 65536
        {
            return Err(fail(Failure::Template));
        }
        if spec
            .source
            .is_some_and(|s| s.session == 0 || s.interface == 0 || s.epoch == 0)
        {
            return Err(fail(Failure::Template));
        }
        let codec = spec
            .structured
            .as_ref()
            .map(|s| Codec::new(&s.protocol, &s.registry))
            .transpose()
            .map_err(|_| fail(Failure::Validation))?;
        Self::validate_body(codec.as_ref(), &spec.body)?;
        let mut wire = if let Some(s) = &spec.structured {
            outbound::encode(
                &s.protocol.framing,
                s.protocol.escape,
                &s.header,
                &spec.body,
            )
            .map_err(|_| fail(Failure::Framing))?
        } else {
            spec.body.clone()
        };
        if spec.format == Format::HidUnnumbered {
            if wire.len() >= 65536 {
                return Err(fail(Failure::Payload));
            }
            wire.insert(0, 0);
        }
        let frame = if let Format::Can { id_word, dlc } = spec.format {
            Some(Frame::new(id_word, dlc, &wire).map_err(|_| fail(Failure::Payload))?)
        } else {
            None
        };
        let gap_ns = spec
            .structured
            .as_ref()
            .and_then(|s| {
                if let Framing::IdleGap { gap_ns, .. } = s.protocol.framing {
                    Some(gap_ns)
                } else {
                    None
                }
            })
            .unwrap_or(0);
        // Retain only bounded compiled definitions; unrelated classification,
        // names and external protocol Arc allocations cannot pin template memory.
        spec.structured = None;
        let template = Self {
            spec,
            wire,
            frame,
            codec,
            gap_ns,
            charge,
        };
        template.validate()?;
        Ok(template)
    }
    fn validate(&self) -> Result<(), (Failure, Validation)> {
        Self::validate_body(self.codec.as_ref(), &self.spec.body)
    }
    fn validate_body(codec: Option<&Codec>, body: &[u8]) -> Result<(), (Failure, Validation)> {
        if let Some(codec) = codec {
            let draft = codec
                .decode(body)
                .map_err(|_| (Failure::Validation, Validation::default()))?;
            if let Err(invalid) = draft.prepare() {
                let fields = invalid
                    .report
                    .fields
                    .into_iter()
                    .filter_map(|f| f.violations.first().map(|issue| (f.index, *issue)))
                    .collect();
                let checksums = invalid
                    .report
                    .checksums
                    .into_iter()
                    .filter_map(|s| s.result.err().map(|issue| (s.index, issue)))
                    .collect();
                return Err((Failure::Validation, Validation { fields, checksums }));
            }
        }
        Ok(())
    }
    fn packet(&self) -> Packet<'_> {
        if let Some(frame) = &self.frame {
            Packet::Can(frame)
        } else {
            Packet::Bytes(&self.wire)
        }
    }
    fn cost(&self) -> usize {
        self.charge
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Rejected(Failure),
    Pending,
    Complete {
        accepted: Option<usize>,
        status: Completion,
    },
    Failed {
        error: BackendError,
        accepted: Option<usize>,
    },
}
#[derive(Clone, Debug)]
pub struct Attempt {
    pub timestamp_ns: Option<u64>,
    pub sequence: u64,
    pub id: u64,
    pub binding: Binding,
    pub template: u64,
    pub revision: u64,
    pub source: Option<SourceRef>,
    pub requested: Option<usize>,
    pub outcome: Outcome,
    pub observation: Option<Observation>,
    pub validation: Validation,
}
impl Attempt {
    fn cost(&self) -> usize {
        self.observation.as_ref().map_or(0, |o| o.bytes().len())
            + self.validation.fields.len() * 16
            + self.validation.checksums.len() * 16
            + 256
    }
}
#[derive(Clone, Default, Debug)]
pub struct Snapshot {
    pub ticket: Option<Ticket>,
    pub identity: Option<Identity>,
    pub templates: Vec<(u64, u64)>,
    pub last_error: Option<Failure>,
    pub validation: Validation,
    pub pending: bool,
    pub history_evicted: u64,
    pub history_lost: u64,
    pub queue_rejected: u64,
}
struct History {
    items: VecDeque<Attempt>,
    bytes: usize,
    next: u64,
    evicted: u64,
    lost: u64,
}
struct Shared {
    inflight: AtomicU64,
    owner: Arc<()>,
    clock: Instant,
    authority: AtomicU64,
    next: AtomicU64,
    attempt: AtomicU64,
    close_generation: AtomicU64,
    quit: AtomicBool,
    control: Mutex<()>,
    cancel: Mutex<Cancellation>,
    snapshot: Mutex<Snapshot>,
    history: Mutex<History>,
    queue_rejected: AtomicU64,
}
impl Shared {
    fn update(&self, f: impl FnOnce(&mut Snapshot)) {
        if let Ok(mut s) = self.snapshot.lock() {
            f(&mut s)
        }
    }
    fn revoke(&self) {
        self.authority.fetch_or(REVOKED, Ordering::AcqRel);
        if let Ok(c) = self.cancel.lock() {
            c.cancel();
        }
    }
    fn change(&self) -> Result<u64, Failure> {
        self.revoke();
        let n = self
            .next
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |v| {
                v.checked_add(1).filter(|n| *n <= u64::MAX >> 2)
            })
            .map_err(|_| {
                self.close_generation.store(
                    self.authority.load(Ordering::Acquire) & !3,
                    Ordering::Release,
                );
                Failure::Exhausted
            })?
            + 1;
        let generation = n << 2;
        self.authority
            .store(generation | REVOKED, Ordering::Release);
        Ok(generation)
    }
    fn current(&self, generation: u64) -> bool {
        self.authority.load(Ordering::Acquire) & !3 == generation
    }
    fn release(&self, generation: u64) {
        let mut value = self.authority.load(Ordering::Acquire);
        while value & !3 == generation {
            match self.authority.compare_exchange_weak(
                value,
                value & !SUBMITTED,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => break,
                Err(next) => value = next,
            }
        }
    }
    fn record(&self, mut a: Attempt) {
        if a.outcome != Outcome::Pending {
            let _ = self
                .inflight
                .compare_exchange(a.id, 0, Ordering::AcqRel, Ordering::Acquire);
        }
        a.timestamp_ns = u64::try_from(self.clock.elapsed().as_nanos()).ok();
        if let Ok(mut h) = self.history.lock() {
            let Some(next) = h.next.checked_add(1) else {
                h.lost = h.lost.saturating_add(1);
                self.revoke();
                self.close_generation.store(
                    self.authority.load(Ordering::Acquire) & !3,
                    Ordering::Release,
                );
                self.update(|s| s.last_error = Some(Failure::Exhausted));
                return;
            };
            h.next = next;
            a.sequence = next;
            let cost = a.cost();
            while h.items.len() >= 128 || h.bytes + cost > MAX_BYTES {
                if let Some(old) = h.items.pop_front() {
                    h.bytes -= old.cost();
                    h.evicted = h.evicted.saturating_add(1)
                } else {
                    break;
                }
            }
            if cost > MAX_BYTES {
                h.lost = h.lost.saturating_add(1)
            } else {
                h.bytes += cost;
                h.items.push_back(a)
            }
        }
    }
    fn fresh_cancel(&self) -> Option<Cancellation> {
        let c = Cancellation::default();
        *self.cancel.lock().ok()? = c.clone();
        Some(c)
    }
    fn activate(&self, generation: u64) -> bool {
        let Ok(_guard) = self.control.lock() else {
            return false;
        };
        if self.authority.load(Ordering::Acquire) != generation | REVOKED {
            return false;
        }
        if self.fresh_cancel().is_none() {
            return false;
        }
        self.authority
            .compare_exchange(
                generation | REVOKED,
                generation,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
    }
}
type Factory = Box<dyn FnOnce() -> Result<Box<dyn Port>, BackendError> + Send>;
enum Command {
    Bind {
        generation: u64,
        key: Binding,
        expected: Expected,
        factory: Factory,
    },
    Template {
        generation: u64,
        spec: TemplateSpec,
    },
    Send {
        ticket: Ticket,
        template: u64,
        revision: u64,
        id: u64,
    },
}
pub struct TxWorker {
    shared: Arc<Shared>,
    commands: SyncSender<Command>,
    thread: Option<JoinHandle<()>>,
}
impl TxWorker {
    pub fn spawn() -> Result<Self, Failure> {
        let shared = Arc::new(Shared {
            inflight: AtomicU64::new(0),
            owner: Arc::new(()),
            clock: Instant::now(),
            authority: AtomicU64::new(REVOKED),
            next: AtomicU64::new(0),
            attempt: AtomicU64::new(0),
            close_generation: AtomicU64::new(0),
            quit: AtomicBool::new(false),
            control: Mutex::new(()),
            cancel: Mutex::new(Cancellation::default()),
            snapshot: Mutex::new(Snapshot::default()),
            history: Mutex::new(History {
                items: VecDeque::new(),
                bytes: 0,
                next: 0,
                evicted: 0,
                lost: 0,
            }),
            queue_rejected: AtomicU64::new(0),
        });
        let (tx, rx) = mpsc::sync_channel(4);
        let worker = shared.clone();
        let handle = thread::Builder::new()
            .name("exmon-tx".into())
            .spawn(move || run(worker, rx))
            .map_err(|_| Failure::Backend(BackendError::Backend))?;
        Ok(Self {
            shared,
            commands: tx,
            thread: Some(handle),
        })
    }
    pub fn bind(
        &self,
        key: Binding,
        expected: Expected,
        permission: bool,
        factory: impl FnOnce() -> Result<Box<dyn Port>, BackendError> + Send + 'static,
    ) -> Result<(), Failure> {
        let _guard = self
            .shared
            .control
            .lock()
            .map_err(|_| Failure::Disconnected)?;
        let generation = self.shared.change()?;
        if !key.valid() || !permission {
            self.shared
                .close_generation
                .store(generation, Ordering::Release);
            return Err(if permission {
                Failure::Binding
            } else {
                Failure::Permission
            });
        }
        self.enqueue(Command::Bind {
            generation,
            key,
            expected,
            factory: Box::new(factory),
        })
        .inspect_err(|_| {
            self.shared
                .close_generation
                .store(generation, Ordering::Release)
        })
    }
    pub fn register(&self, spec: TemplateSpec) -> Result<(), Failure> {
        let _guard = self
            .shared
            .control
            .lock()
            .map_err(|_| Failure::Disconnected)?;
        let current = self.shared.authority.load(Ordering::Acquire) & !3;
        if current == self.shared.close_generation.load(Ordering::Acquire)
            || !self
                .shared
                .snapshot
                .lock()
                .is_ok_and(|s| s.identity.is_some())
        {
            return Err(Failure::Disconnected);
        }
        let generation = self.shared.change()?;
        charge(&spec)?;
        self.enqueue(Command::Template { generation, spec })
    }
    fn enqueue(&self, command: Command) -> Result<(), Failure> {
        self.commands
            .try_send(command)
            .map_err(|error| match error {
                mpsc::TrySendError::Full(_) => {
                    let _ = self.shared.queue_rejected.fetch_update(
                        Ordering::Relaxed,
                        Ordering::Relaxed,
                        |v| Some(v.saturating_add(1)),
                    );
                    Failure::Queue
                }
                mpsc::TrySendError::Disconnected(_) => Failure::Disconnected,
            })
    }
    pub fn send_once(&self, ticket: &Ticket, template: u64, revision: u64) -> Result<u64, Failure> {
        let id = self
            .shared
            .attempt
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |v| v.checked_add(1))
            .map_err(|_| {
                self.shared.revoke();
                self.shared.close_generation.store(
                    self.shared.authority.load(Ordering::Acquire) & !3,
                    Ordering::Release,
                );
                Failure::Exhausted
            })?
            + 1;
        let failure = if !Arc::ptr_eq(&ticket.owner, &self.shared.owner)
            || self.shared.authority.load(Ordering::Acquire) & !SUBMITTED != ticket.generation
        {
            Some(Failure::Stale)
        } else {
            self.enqueue(Command::Send {
                ticket: ticket.clone(),
                template,
                revision,
                id,
            })
            .err()
        };
        if let Some(failure) = failure {
            self.shared
                .record(rejected(id, ticket.binding, template, revision, failure));
            return Err(failure);
        }
        Ok(id)
    }
    /// Revoke first, independently of a full command queue; never waits on source I/O.
    pub fn close(&self) {
        self.shared.revoke();
        if let Ok(_guard) = self.shared.control.lock() {
            if let Ok(generation) = self.shared.change() {
                self.shared
                    .close_generation
                    .store(generation, Ordering::Release)
            }
        } else {
            self.shared.close_generation.store(
                self.shared.authority.load(Ordering::Acquire) & !3,
                Ordering::Release,
            );
        }
    }
    pub fn snapshot(&self) -> Option<Snapshot> {
        self.shared.snapshot.try_lock().ok().and_then(|s| {
            let mut s = s.clone();
            s.queue_rejected = self.shared.queue_rejected.load(Ordering::Relaxed);
            let h = self.shared.history.try_lock().ok()?;
            s.history_evicted = h.evicted;
            s.history_lost = h.lost;
            if s.ticket.as_ref().is_some_and(|t| {
                !self.shared.current(t.generation)
                    || self.shared.authority.load(Ordering::Acquire) & REVOKED != 0
            }) {
                s.ticket = None;
            }
            Some(s)
        })
    }
    pub fn history(&self, after: u64, maximum: usize) -> Vec<Attempt> {
        self.shared
            .history
            .try_lock()
            .map(|h| {
                h.items
                    .iter()
                    .filter(|a| a.sequence > after)
                    .take(maximum.min(4))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }
}
impl Drop for TxWorker {
    fn drop(&mut self) {
        self.shared.revoke();
        self.shared.quit.store(true, Ordering::Release);
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }
}
fn rejected(id: u64, binding: Binding, template: u64, revision: u64, failure: Failure) -> Attempt {
    Attempt {
        timestamp_ns: None,
        sequence: 0,
        id,
        binding,
        template,
        revision,
        source: None,
        requested: None,
        outcome: Outcome::Rejected(failure),
        observation: None,
        validation: Validation::default(),
    }
}
struct Pending {
    attempt: Attempt,
    template: Arc<Template>,
    metadata: Metadata,
    generation: u64,
    cancel: Cancellation,
    started: Instant,
}
struct Active {
    port: Box<dyn Port>,
    info: Info,
    key: Binding,
}
impl Drop for Active {
    fn drop(&mut self) {
        let _ = self.port.close();
    }
}
struct Exit(Arc<Shared>);
impl Drop for Exit {
    fn drop(&mut self) {
        self.0.revoke();
        self.0.quit.store(true, Ordering::Release);
        if thread::panicking()
            && self.0.inflight.load(Ordering::Acquire) != 0
            && let Ok(mut h) = self.0.history.lock()
        {
            h.lost = h.lost.saturating_add(1)
        }
        self.0.update(|s| {
            s.ticket = None;
            s.identity = None;
            s.pending = false;
            if thread::panicking() {
                s.last_error = Some(Failure::Disconnected)
            }
        });
    }
}
fn terminal(
    shared: &Shared,
    mut pending: Pending,
    result: Result<Progress, BackendError>,
    clock: Instant,
) -> bool {
    let mut successful = false;
    match result {
        Ok(Progress::Complete { accepted, status })
            if accepted.is_none_or(|n| n <= pending.template.wire.len())
                && (status != Completion::Accepted || accepted.is_some()) =>
        {
            pending.attempt.outcome = Outcome::Complete { accepted, status };
            successful = status == Completion::Accepted;
            if let Some(n) = accepted {
                if let Metadata::Usb { completed, .. } = &mut pending.metadata {
                    *completed = status == Completion::Accepted;
                }
                pending.attempt.observation = u64::try_from(clock.elapsed().as_nanos())
                    .ok()
                    .and_then(|time| {
                        Observation::new(
                            time,
                            Direction::Tx,
                            pending.metadata,
                            None,
                            &pending.template.wire[..n],
                        )
                        .ok()
                    });
                if pending.attempt.observation.is_none() {
                    successful = false;
                    pending.attempt.outcome = Outcome::Failed {
                        error: BackendError::Backend,
                        accepted: Some(n),
                    };
                }
            }
        }
        Err(error) => {
            pending.attempt.outcome = Outcome::Failed {
                error,
                accepted: None,
            }
        }
        _ => {
            pending.attempt.outcome = Outcome::Failed {
                error: BackendError::Backend,
                accepted: None,
            }
        }
    }
    shared.record(pending.attempt);
    shared.release(pending.generation);
    shared.update(|s| s.pending = false);
    successful
}
fn run(shared: Arc<Shared>, rx: mpsc::Receiver<Command>) {
    let mut active: Option<Active> = None;
    let _exit = Exit(shared.clone());
    let mut pending: Option<Pending> = None;
    let mut templates: BTreeMap<u64, Arc<Template>> = BTreeMap::new();
    let clock = shared.clock;
    let mut last_write: Option<(Instant, u64)> = None;
    while !shared.quit.load(Ordering::Acquire) {
        let authority = shared.authority.load(Ordering::Acquire);
        let closing = authority & !3 == shared.close_generation.load(Ordering::Acquire)
            && authority & REVOKED != 0;
        if pending.as_ref().is_some_and(|p| {
            closing
                || !shared.current(p.generation)
                || authority & REVOKED != 0
                || p.cancel.is_cancelled()
                || p.started.elapsed() >= Duration::from_secs(1)
        }) {
            let mut completed = None;
            if let Some(mut a) = active.take() {
                let _ = a.port.close();
                completed = a
                    .port
                    .completion()
                    .ok()
                    .filter(|p| matches!(p, Progress::Complete { .. }));
            }
            if let Some(p) = pending.take() {
                let status = if p.started.elapsed() >= Duration::from_secs(1) {
                    Completion::Timeout
                } else {
                    Completion::Cancelled
                };
                terminal(
                    &shared,
                    p,
                    Ok(completed.unwrap_or(Progress::Complete {
                        accepted: None,
                        status,
                    })),
                    clock,
                );
            }
            shared.update(|s| {
                s.ticket = None;
                s.identity = None;
            });
        }
        if closing {
            if let Some(mut a) = active.take() {
                let _ = a.port.close();
            }
            shared.update(|s| {
                s.ticket = None;
                s.identity = None;
            });
        }
        if let Some(p) = pending.take() {
            let result = active
                .as_mut()
                .ok_or(BackendError::Disconnected)
                .and_then(|a| a.port.completion());
            if matches!(result, Ok(Progress::Pending)) {
                pending = Some(p)
            } else {
                let failed = !terminal(&shared, p, result, clock);
                if failed {
                    shared.revoke();
                    if let Some(mut a) = active.take() {
                        let _ = a.port.close();
                    }
                    shared.update(|s| {
                        s.ticket = None;
                        s.identity = None;
                    });
                }
            }
        }
        if pending.is_none()
            && let Ok(command) = rx.try_recv()
        {
            match command {
                Command::Bind {
                    generation,
                    key,
                    expected,
                    factory,
                } => {
                    if shared
                        .authority
                        .compare_exchange(
                            generation | REVOKED,
                            generation | REVOKED | SUBMITTED,
                            Ordering::AcqRel,
                            Ordering::Acquire,
                        )
                        .is_err()
                    {
                        continue;
                    }
                    if let Some(mut a) = active.take() {
                        let _ = a.port.close();
                    }
                    shared.update(|s| {
                        s.ticket = None;
                        s.identity = None;
                        s.last_error = None;
                        s.validation = Validation::default();
                    });
                    let result = factory().and_then(|mut port| match port.info() {
                        Ok(info) if expected.matches(&info.identity) => {
                            Ok(Active { port, info, key })
                        }
                        Ok(_) => {
                            let _ = port.close();
                            Err(BackendError::InvalidConfig)
                        }
                        Err(e) => {
                            let _ = port.close();
                            Err(e)
                        }
                    });
                    shared.release(generation);
                    match result {
                        Ok(a) if shared.activate(generation) => {
                            shared.update(|s| {
                                s.ticket = Some(Ticket {
                                    owner: shared.owner.clone(),
                                    generation,
                                    binding: key,
                                });
                                s.identity = Some(a.info.identity.clone());
                            });
                            active = Some(a);
                            last_write = None;
                        }
                        Ok(a) => {
                            let mut a = a;
                            let _ = a.port.close();
                        }
                        Err(e) => shared.update(|s| s.last_error = Some(Failure::Backend(e))),
                    }
                }
                Command::Template { generation, spec } => {
                    if !shared.current(generation) {
                        continue;
                    }
                    let id = spec.id;
                    let revision = spec.revision;
                    let result = Template::new(spec).and_then(|t| {
                        if templates
                            .get(&id)
                            .is_some_and(|old| old.spec.revision >= revision)
                        {
                            return Err((Failure::Stale, Validation::default()));
                        }
                        let used: usize = templates
                            .values()
                            .filter(|old| old.spec.id != id)
                            .map(|t| t.cost())
                            .sum();
                        if (!templates.contains_key(&id) && templates.len() >= 64)
                            || used + t.cost() > MAX_BYTES
                        {
                            return Err((Failure::Capacity, Validation::default()));
                        }
                        Ok(t)
                    });
                    match result {
                        Ok(t) => {
                            templates.insert(id, Arc::new(t));
                            shared.update(|s| {
                                s.templates = templates
                                    .values()
                                    .map(|t| (t.spec.id, t.spec.revision))
                                    .collect()
                            });
                            if let Some(a) = &active
                                && a.port.info().is_ok_and(|info| info == a.info)
                                && shared.activate(generation)
                            {
                                shared.update(|s| {
                                    s.ticket = Some(Ticket {
                                        owner: shared.owner.clone(),
                                        generation,
                                        binding: a.key,
                                    });
                                    s.last_error = None;
                                    s.validation = Validation::default();
                                    s.templates = templates
                                        .values()
                                        .map(|t| (t.spec.id, t.spec.revision))
                                        .collect();
                                });
                            } else if shared.current(generation) {
                                shared.update(|s| s.last_error = Some(Failure::Disconnected));
                            }
                        }
                        Err((e, v)) => shared.update(|s| {
                            s.last_error = Some(e);
                            s.validation = v;
                        }),
                    }
                }
                Command::Send {
                    ticket,
                    template,
                    revision,
                    id,
                } => {
                    shared.inflight.store(id, Ordering::Release);
                    let mut record =
                        rejected(id, ticket.binding, template, revision, Failure::Stale);
                    let validation = (|| {
                        if !shared.current(ticket.generation)
                            || shared.authority.load(Ordering::Acquire) & REVOKED != 0
                        {
                            return Err(Failure::Stale);
                        }
                        let a = active.as_ref().ok_or(Failure::Disconnected)?;
                        if a.key != ticket.binding {
                            return Err(Failure::Binding);
                        }
                        let t = templates
                            .get(&template)
                            .filter(|t| t.spec.revision == revision)
                            .ok_or(Failure::Template)?
                            .clone();
                        record.source = t.spec.source;
                        record.requested = Some(t.wire.len());
                        if t.spec.protocol_revision != a.key.protocol
                            || t.spec.format.kind() != a.info.identity.kind()
                        {
                            return Err(Failure::Binding);
                        }
                        let info = a.port.info().map_err(Failure::Backend)?;
                        if info.identity != a.info.identity || info.max_bytes != a.info.max_bytes {
                            return Err(Failure::Binding);
                        }
                        match info.writable {
                            Availability::Available => {}
                            Availability::Unavailable(e) => return Err(Failure::Backend(e)),
                            Availability::Unsupported => {
                                return Err(Failure::Backend(BackendError::Unsupported));
                            }
                        }
                        if t.wire.len() > info.max_bytes {
                            return Err(Failure::Payload);
                        }
                        if let Identity::Hid { numbered, .. } = info.identity
                            && t.spec.format == Format::HidUnnumbered
                            && numbered
                        {
                            return Err(Failure::Payload);
                        }
                        if last_write.is_some_and(|(last, gap)| {
                            last.elapsed().as_nanos() < u128::from(gap.max(t.gap_ns))
                        }) {
                            return Err(Failure::Backend(BackendError::Busy));
                        }
                        if let Err((e, v)) = t.validate() {
                            record.validation = v;
                            return Err(e);
                        }
                        let metadata = a.port.metadata(&t.packet()).map_err(Failure::Backend)?;
                        Observation::new(
                            0,
                            Direction::Tx,
                            metadata.clone(),
                            None,
                            t.packet().bytes(),
                        )
                        .map_err(|_| Failure::Payload)?;
                        Ok((t, metadata))
                    })();
                    match validation {
                        Err(e) => {
                            record.outcome = Outcome::Rejected(e);
                            shared.record(record);
                            if matches!(
                                e,
                                Failure::Binding
                                    | Failure::Backend(
                                        BackendError::Disconnected
                                            | BackendError::PermissionDenied
                                            | BackendError::Backend
                                    )
                            ) {
                                shared.revoke();
                                if let Some(mut a) = active.take() {
                                    let _ = a.port.close();
                                }
                                shared.update(|s| {
                                    s.ticket = None;
                                    s.identity = None;
                                });
                            }
                        }
                        Ok((t, metadata)) => {
                            record.source = t.spec.source;
                            record.requested = Some(t.wire.len());
                            if shared
                                .authority
                                .compare_exchange(
                                    ticket.generation,
                                    ticket.generation | SUBMITTED,
                                    Ordering::AcqRel,
                                    Ordering::Acquire,
                                )
                                .is_err()
                            {
                                shared.record(record);
                                continue;
                            }
                            let cancel = shared.cancel.lock().ok().map(|c| c.clone());
                            let Some(cancel) = cancel else {
                                shared.revoke();
                                shared.release(ticket.generation);
                                shared.record(record);
                                continue;
                            };
                            let p = Pending {
                                attempt: record,
                                template: t.clone(),
                                metadata,
                                generation: ticket.generation,
                                cancel: cancel.clone(),
                                started: Instant::now(),
                            };
                            let result = active.as_mut().unwrap().port.submit(&t.packet(), &cancel);
                            last_write = Some((Instant::now(), t.gap_ns));
                            if matches!(result, Ok(Progress::Pending)) {
                                let mut event = p.attempt.clone();
                                event.outcome = Outcome::Pending;
                                shared.record(event);
                                shared.update(|s| s.pending = true);
                                pending = Some(p)
                            } else {
                                // A nonblocking socket may be temporarily full.
                                // Preserve only an unchanged live port; retry still
                                // requires another explicit Send action.
                                let busy_live = matches!(result, Err(BackendError::Busy))
                                    && active.as_ref().is_some_and(|a| {
                                        a.port.info().is_ok_and(|info| info == a.info)
                                    });
                                let failed = !terminal(&shared, p, result, clock) && !busy_live;
                                if failed {
                                    shared.revoke();
                                    if let Some(mut a) = active.take() {
                                        let _ = a.port.close();
                                    }
                                    shared.update(|s| {
                                        s.ticket = None;
                                        s.identity = None;
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
        thread::sleep(Duration::from_millis(1));
    }
    shared.revoke();
    let mut completed = None;
    if let Some(mut a) = active {
        let _ = a.port.close();
        completed = a
            .port
            .completion()
            .ok()
            .filter(|p| matches!(p, Progress::Complete { .. }));
    }
    if let Some(p) = pending {
        terminal(
            &shared,
            p,
            Ok(completed.unwrap_or(Progress::Complete {
                accepted: None,
                status: Completion::Cancelled,
            })),
            clock,
        );
    }
    shared.update(|s| {
        s.ticket = None;
        s.identity = None;
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    fn key() -> Binding {
        Binding {
            session: 1,
            interface: 1,
            epoch: 1,
            configuration: 1,
            protocol: 1,
        }
    }
    #[test]
    fn authority_attempt_and_history_counters_exhaust_without_wrapping_or_leaving_send_authority() {
        let w = TxWorker::spawn().unwrap();
        w.shared.next.store(u64::MAX >> 2, Ordering::Release);
        assert_eq!(
            w.bind(
                key(),
                Expected::Exact(Identity::Serial {
                    device: 1,
                    revision: 1
                }),
                true,
                || panic!("exhausted factory must never run")
            ),
            Err(Failure::Exhausted)
        );
        assert_ne!(w.shared.authority.load(Ordering::Acquire) & REVOKED, 0);
        w.shared.attempt.store(u64::MAX, Ordering::Release);
        let t = Ticket {
            owner: w.shared.owner.clone(),
            generation: 0,
            binding: key(),
        };
        assert_eq!(w.send_once(&t, 1, 1), Err(Failure::Exhausted));
        w.shared.history.lock().unwrap().next = u64::MAX;
        w.shared.record(rejected(1, key(), 1, 1, Failure::Stale));
        let h = w.shared.history.lock().unwrap();
        assert_eq!(h.next, u64::MAX);
        assert_eq!(h.lost, 1);
        assert!(h.items.is_empty());
    }
    #[test]
    fn drop_collects_known_completion_after_close_instead_of_inventing_unknown_cancellation() {
        struct FinishOnClose(bool);
        impl Port for FinishOnClose {
            fn info(&self) -> Result<Info, BackendError> {
                Ok(Info {
                    identity: Identity::Serial {
                        device: 1,
                        revision: 1,
                    },
                    writable: Availability::Available,
                    max_bytes: 65536,
                })
            }
            fn metadata(&self, _: &Packet<'_>) -> Result<Metadata, BackendError> {
                Ok(Metadata::Serial {
                    configuration_revision: 1,
                })
            }
            fn submit(
                &mut self,
                _: &Packet<'_>,
                _: &Cancellation,
            ) -> Result<Progress, BackendError> {
                Ok(Progress::Pending)
            }
            fn completion(&mut self) -> Result<Progress, BackendError> {
                Ok(if self.0 {
                    Progress::accepted(3)
                } else {
                    Progress::Pending
                })
            }
            fn close(&mut self) -> Result<(), BackendError> {
                self.0 = true;
                Ok(())
            }
        }
        fn wait(mut f: impl FnMut() -> bool) {
            let deadline = Instant::now() + Duration::from_secs(3);
            while !f() {
                assert!(Instant::now() < deadline);
                thread::sleep(Duration::from_millis(1));
            }
        }
        let w = TxWorker::spawn().unwrap();
        w.bind(
            key(),
            Expected::Exact(Identity::Serial {
                device: 1,
                revision: 1,
            }),
            true,
            || Ok(Box::new(FinishOnClose(false))),
        )
        .unwrap();
        wait(|| w.snapshot().is_some_and(|s| s.ticket.is_some()));
        w.register(TemplateSpec {
            id: 1,
            revision: 1,
            protocol_revision: 1,
            format: Format::Bytes(Kind::Serial),
            body: vec![1, 2, 3],
            structured: None,
            source: None,
        })
        .unwrap();
        wait(|| {
            w.snapshot()
                .is_some_and(|s| s.ticket.is_some() && s.templates.len() == 1)
        });
        let t = w.snapshot().unwrap().ticket.unwrap();
        let id = w.send_once(&t, 1, 1).unwrap();
        wait(|| w.snapshot().is_some_and(|s| s.pending));
        let shared = w.shared.clone();
        drop(w);
        let history = shared.history.lock().unwrap();
        assert!(history.items.iter().any(|a| a.id == id
            && a.outcome
                == Outcome::Complete {
                    accepted: Some(3),
                    status: Completion::Accepted
                }));
    }
}
