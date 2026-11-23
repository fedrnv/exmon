//! Explicit one-shot worker. Preparation, source factories and submissions never run on UI.
use crate::scheduler::{self, Clock, Engine, Grant, Handle, Reason, Spec, State, Status, Timing};
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
pub(crate) fn charge(spec: &TemplateSpec) -> Result<usize, Failure> {
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
    pub scheduled: Option<Timing>,
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
    pub schedules: Vec<Status>,
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
    schedule_clock: Arc<dyn Clock>,
    grants: Mutex<BTreeMap<u64, Arc<Grant>>>,
    activation: AtomicU64,
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
        self.stop_schedules(Reason::Stale);
        self.authority.fetch_or(REVOKED, Ordering::AcqRel);
        if let Ok(c) = self.cancel.lock() {
            c.cancel();
        }
    }
    fn stop_schedules(&self, why: Reason) {
        if let Ok(grants) = self.grants.lock() {
            for grant in grants.values() {
                grant.stop(why);
            }
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
    Schedule {
        ticket: Ticket,
        spec: Spec,
        grant: Arc<Grant>,
    },
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
        due: Option<Due>,
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
        Self::spawn_with_clock(Arc::new(scheduler::MonotonicClock::default()))
    }
    /// Inject only a trusted, bounded, nonblocking monotonic source.
    pub fn spawn_with_clock(clock: Arc<dyn Clock>) -> Result<Self, Failure> {
        let shared = Arc::new(Shared {
            schedule_clock: clock,
            grants: Mutex::new(BTreeMap::new()),
            activation: AtomicU64::new(0),
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
    /// Revoke prepared/queued work immediately. An idle selected port is retained;
    /// a fresh explicit validation/registration is required to obtain another ticket.
    pub fn invalidate(&self) -> Result<(), Failure> {
        self.shared.revoke();
        let _guard = self
            .shared
            .control
            .lock()
            .map_err(|_| Failure::Disconnected)?;
        self.shared.change().map(|_| ())
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
                due: None,
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
    /// Explicit activation for this worker, session, binding and template revision.
    /// Dropping the final returned handle revokes the activation.
    pub fn enable_schedule(&self, ticket: &Ticket, spec: Spec) -> Result<Handle, Failure> {
        let _guard = self
            .shared
            .control
            .lock()
            .map_err(|_| Failure::Disconnected)?;
        if !Arc::ptr_eq(&ticket.owner, &self.shared.owner)
            || self.shared.authority.load(Ordering::Acquire) & !SUBMITTED != ticket.generation
        {
            return Err(Failure::Stale);
        }
        let mut grants = self
            .shared
            .grants
            .lock()
            .map_err(|_| Failure::Disconnected)?;
        if let Some(old) = grants.get(&spec.id) {
            old.stop(Reason::Replaced);
        }
        spec.validate().map_err(|_| Failure::Validation)?;
        grants.retain(|id, grant| *id == spec.id || grant.stopped().is_none());
        if !grants.contains_key(&spec.id) && grants.len() >= 64 {
            return Err(Failure::Capacity);
        }
        let activation =
            self.shared
                .activation
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |v| v.checked_add(1));
        let activation = match activation {
            Ok(value) => value + 1,
            Err(_) => {
                drop(grants);
                self.shared.stop_schedules(Reason::Exhausted);
                self.shared.revoke();
                self.shared
                    .close_generation
                    .store(ticket.generation, Ordering::Release);
                return Err(Failure::Exhausted);
            }
        };
        let grant = Grant::new(activation);
        let handle = Handle::new(grant.clone());
        grants.insert(spec.id, grant.clone());
        drop(grants);
        if let Err(error) = self.enqueue(Command::Schedule {
            ticket: ticket.clone(),
            spec,
            grant: grant.clone(),
        }) {
            grant.stop(Reason::Invalid);
            return Err(error);
        }
        Ok(handle)
    }
    pub fn stop_schedules(&self) {
        self.shared.stop_schedules(Reason::User);
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
        self.shared.stop_schedules(Reason::Shutdown);
        self.shared.revoke();
        self.shared.quit.store(true, Ordering::Release);
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }
}
fn rejected(id: u64, binding: Binding, template: u64, revision: u64, failure: Failure) -> Attempt {
    Attempt {
        scheduled: None,
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
struct Due {
    id: u64,
    activation: u64,
    grant: Arc<Grant>,
}
struct Scheduled {
    ticket: Ticket,
    spec: Spec,
    grant: Arc<Grant>,
    engine: Engine,
}
#[derive(Default)]
struct Schedules {
    items: BTreeMap<u64, Scheduled>,
    cursor: u64,
}
impl Schedules {
    fn sync(&mut self, shared: &Shared) {
        if self.items.is_empty() {
            return;
        }
        let now = shared.schedule_clock.now_ns();
        for item in self.items.values_mut() {
            item.engine.tick(now);
            if let Some(why) = item.grant.stopped() {
                item.engine.stop(why);
            }
            if let State::Stopped(why) = item.engine.state() {
                item.grant.stop(why);
            }
        }
        shared.update(|s| {
            s.schedules = self
                .items
                .values()
                .map(|i| Status {
                    id: i.spec.id,
                    revision: i.spec.revision,
                    activation: i.grant.activation,
                    state: i.engine.state(),
                    next_ns: i.engine.next_ns(),
                    stats: i.engine.stats().clone(),
                })
                .collect()
        });
    }
    fn add(&mut self, shared: &Shared, ticket: Ticket, spec: Spec, grant: Arc<Grant>, valid: bool) {
        let compatible = self
            .items
            .get(&spec.id)
            .is_none_or(|old| spec.revision > old.spec.revision || spec == old.spec);
        self.items
            .retain(|id, item| *id == spec.id || item.grant.stopped().is_none());
        if !valid || !compatible || (!self.items.contains_key(&spec.id) && self.items.len() >= 64) {
            grant.stop(Reason::Invalid);
            return;
        }
        let Ok(mut engine) = Engine::new(spec.options) else {
            grant.stop(Reason::Invalid);
            return;
        };
        match shared.schedule_clock.now_ns() {
            Ok(now) => engine.arm(now),
            Err(_) => engine.stop(Reason::Clock),
        }
        if let State::Stopped(why) = engine.state() {
            grant.stop(why);
        }
        grant.activate();
        self.items.insert(
            spec.id,
            Scheduled {
                ticket,
                spec,
                grant,
                engine,
            },
        );
    }
    fn due(&mut self, shared: &Shared) -> Option<Command> {
        let now = shared.schedule_clock.now_ns().ok()?;
        // Cyclic selection bounds starvation without storing missed-period work.
        let ids: Vec<_> = self
            .items
            .range((
                std::ops::Bound::Excluded(self.cursor),
                std::ops::Bound::Unbounded,
            ))
            .chain(self.items.range(..=self.cursor))
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            let item = self.items.get_mut(&id)?;
            if item.grant.stopped().is_some() {
                continue;
            }
            if item.engine.offer(now).is_some() {
                self.cursor = id;
                let Ok(attempt) =
                    shared
                        .attempt
                        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |v| v.checked_add(1))
                else {
                    item.engine.stop(Reason::Exhausted);
                    item.grant.stop(Reason::Exhausted);
                    shared.stop_schedules(Reason::Exhausted);
                    shared.revoke();
                    return None;
                };
                return Some(Command::Send {
                    ticket: item.ticket.clone(),
                    template: item.spec.template,
                    revision: item.spec.template_revision,
                    id: attempt + 1,
                    due: Some(Due {
                        id,
                        activation: item.grant.activation,
                        grant: item.grant.clone(),
                    }),
                });
            }
        }
        None
    }
    fn item(&mut self, due: &Due) -> Option<&mut Scheduled> {
        self.items
            .get_mut(&due.id)
            .filter(|i| i.grant.activation == due.activation)
    }
    fn timing(&mut self, due: &Due) -> Option<Timing> {
        let item = self.item(due)?;
        let stats = item.engine.stats();
        Some(Timing {
            id: due.id,
            activation: due.activation,
            deadline_ns: stats.deadline_ns?,
            decision_ns: stats.decision_ns?,
            submitted_ns: None,
            completed_ns: None,
        })
    }
    fn reject(&mut self, shared: &Shared, due: &Due, error: Failure) {
        if let Some(item) = self.item(due) {
            if error == Failure::Backend(BackendError::Busy) {
                match shared.schedule_clock.now_ns() {
                    Ok(now) => item.engine.skip(now),
                    Err(_) => item.engine.stop(Reason::Clock),
                }
            } else {
                item.engine.stop(Reason::Invalid);
            }
            if let State::Stopped(why) = item.engine.state() {
                item.grant.stop(why);
            }
        }
    }
    fn commit(&mut self, shared: &Shared, due: &Due) -> Option<Timing> {
        let item = self.item(due)?;
        if !due.grant.acquire() {
            item.engine
                .stop(due.grant.stopped().unwrap_or(Reason::Stale));
            return None;
        }
        let now = shared.schedule_clock.now_ns();
        let committed = match now {
            Ok(now) => item.engine.commit(now),
            Err(_) => {
                item.engine.stop(Reason::Clock);
                false
            }
        };
        if !committed {
            due.grant.release();
            if let State::Stopped(why) = item.engine.state() {
                due.grant.stop(why);
            }
            return None;
        }
        let stats = item.engine.stats();
        Some(Timing {
            id: due.id,
            activation: due.activation,
            deadline_ns: stats.deadline_ns?,
            decision_ns: stats.decision_ns?,
            submitted_ns: stats.submitted_ns,
            completed_ns: None,
        })
    }
    fn finish(&mut self, shared: &Shared, due: &Due, attempt: &mut Attempt) {
        if let Some(item) = self.item(due) {
            if let Some(why) = due.grant.stopped() {
                item.engine.stop(why);
            }
            let why = match attempt.outcome {
                Outcome::Complete {
                    accepted: Some(n),
                    status: Completion::Accepted,
                } if Some(n) == attempt.requested => None,
                Outcome::Complete {
                    status: Completion::Accepted,
                    ..
                } => Some(Reason::Partial),
                Outcome::Failed {
                    error: BackendError::Busy,
                    ..
                } if item.spec.options.missed == scheduler::Missed::Skip => None,
                Outcome::Failed {
                    error: BackendError::Busy,
                    ..
                } => Some(Reason::Busy),
                _ => Some(Reason::Backend),
            };
            // A finite-count completion must not conceal a partial/error outcome.
            if let Some(why) = why {
                item.engine.stop(why);
            }
            match shared.schedule_clock.now_ns() {
                Ok(now) => {
                    item.engine.finish(now);
                    if let Some(t) = &mut attempt.scheduled {
                        t.completed_ns = t
                            .submitted_ns
                            .filter(|submitted| now >= *submitted)
                            .map(|_| now);
                    }
                }
                Err(_) => item.engine.stop(Reason::Clock),
            }
            due.grant.release();
            if let Some(why) = due.grant.stopped() {
                item.engine.stop(why);
            }
            if let State::Stopped(why) = item.engine.state() {
                due.grant.stop(why);
            }
        }
    }
}
struct Pending {
    due: Option<Due>,
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
        self.0.stop_schedules(Reason::Shutdown);
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
    schedules: &mut Schedules,
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
    if let Some(due) = &pending.due {
        schedules.finish(shared, due, &mut pending.attempt);
    }
    shared.record(pending.attempt);
    shared.release(pending.generation);
    shared.update(|s| s.pending = false);
    successful
}
fn run(shared: Arc<Shared>, rx: mpsc::Receiver<Command>) {
    let mut schedules = Schedules::default();
    let mut prefer_schedule = false;
    let mut active: Option<Active> = None;
    let _exit = Exit(shared.clone());
    let mut pending: Option<Pending> = None;
    let mut templates: BTreeMap<u64, Arc<Template>> = BTreeMap::new();
    let clock = shared.clock;
    let mut last_write: Option<(Instant, u64)> = None;
    while !shared.quit.load(Ordering::Acquire) {
        schedules.sync(&shared);
        if schedules.items.values().any(|i| i.grant.enabled())
            && active.as_ref().is_some_and(|a| {
                !a.port
                    .info()
                    .is_ok_and(|info| info == a.info && info.writable == Availability::Available)
            })
        {
            shared.revoke();
        }
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
                    &mut schedules,
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
            shared.revoke();
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
                let busy_live = matches!(result, Err(BackendError::Busy))
                    && active
                        .as_ref()
                        .is_some_and(|a| a.port.info().is_ok_and(|info| info == a.info));
                let failed = !terminal(&shared, &mut schedules, p, result, clock) && !busy_live;
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
        let command = if pending.is_some() {
            None
        } else if prefer_schedule {
            schedules.due(&shared).or_else(|| rx.try_recv().ok())
        } else {
            rx.try_recv().ok().or_else(|| schedules.due(&shared))
        };
        if let Some(command) = command {
            prefer_schedule = !prefer_schedule;
            match command {
                Command::Schedule {
                    ticket,
                    spec,
                    grant,
                } => {
                    let valid = shared.current(ticket.generation)
                        && shared.authority.load(Ordering::Acquire) & REVOKED == 0
                        && active.as_ref().is_some_and(|a| a.key == ticket.binding)
                        && templates
                            .get(&spec.template)
                            .is_some_and(|t| t.spec.revision == spec.template_revision);
                    schedules.add(&shared, ticket, spec, grant, valid);
                }
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
                    due,
                    ticket,
                    template,
                    revision,
                    id,
                } => {
                    shared.inflight.store(id, Ordering::Release);
                    let mut record =
                        rejected(id, ticket.binding, template, revision, Failure::Stale);
                    if let Some(due) = &due {
                        record.scheduled = schedules.timing(due);
                    }
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
                        if due.is_some() && !a.port.info().is_ok_and(|latest| latest == info) {
                            return Err(Failure::Binding);
                        }
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
                            if let Some(due) = &due {
                                schedules.reject(&shared, due, e);
                            }
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
                                if let Some(due) = &due {
                                    schedules.reject(&shared, due, Failure::Stale);
                                }
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
                            let cancel = if let Some(due) = &due {
                                let Some(timing) = schedules.commit(&shared, due) else {
                                    shared.release(ticket.generation);
                                    shared.record(record);
                                    continue;
                                };
                                record.scheduled = Some(timing);
                                due.grant.cancel.clone()
                            } else {
                                cancel
                            };
                            let p = Pending {
                                due,
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
                                let failed = !terminal(&shared, &mut schedules, p, result, clock)
                                    && !busy_live;
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
            &mut schedules,
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
    #[test]
    fn schedule_activation_exhaustion_revokes_all_without_reusing_identity() {
        let w = TxWorker::spawn().unwrap();
        w.shared.authority.store(4, Ordering::Release);
        let ticket = Ticket {
            owner: w.shared.owner.clone(),
            generation: 4,
            binding: key(),
        };
        let grant = Grant::new(1);
        grant.activate();
        w.shared.grants.lock().unwrap().insert(1, grant.clone());
        w.shared.activation.store(u64::MAX, Ordering::Release);
        let spec = Spec {
            id: 2,
            revision: 1,
            template: 1,
            template_revision: 1,
            options: scheduler::Options {
                interval_ms: 1,
                count: None,
                drift: scheduler::Drift::FixedRate,
                missed: scheduler::Missed::Skip,
                max_lateness_ms: None,
            },
        };
        assert_eq!(
            w.enable_schedule(&ticket, spec).unwrap_err(),
            Failure::Exhausted
        );
        assert_eq!(grant.stopped(), Some(Reason::Exhausted));
        assert_eq!(w.shared.activation.load(Ordering::Acquire), u64::MAX);
        assert_ne!(w.shared.authority.load(Ordering::Acquire) & REVOKED, 0);
    }
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
