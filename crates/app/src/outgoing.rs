//! Background outgoing editing. UI mutations revoke the actual TX authority first.
use crate::{
    incoming::Row,
    scheduler::{self, Drift, Handle, Missed, Options},
    transmission::{
        self, Binding, Expected, Failure, Format, Outcome, SourceRef, Structured, TemplateSpec,
        TxWorker,
    },
};
use exmon_capture::{
    adapter::{Availability, Cancellation, Error as BackendError},
    transmit::{Completion, Identity, Info, Kind, Packet, Port, Progress},
};
use exmon_domain::{
    config::{Framing, ProjectConfig, Protocol, Representation},
    fields::{Literal, Number},
    observation::Metadata,
};
use exmon_protocol::{
    checksum::Registry,
    codec::{Codec, Edit},
    outbound,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, SyncSender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
const LIMIT: usize = 1_048_576;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Busy,
    Invalid,
    Unknown,
    Capacity,
    Unavailable,
    Exhausted,
    Dirty,
    NotReady,
    Codec,
    Validation,
    Transport(Failure),
}
impl From<Failure> for Error {
    fn from(e: Failure) -> Self {
        Self::Transport(e)
    }
}
pub struct Create {
    pub source_quality: Option<exmon_protocol::classification::Quality>,
    pub source_revision: Option<u64>,
    pub name: String,
    pub body: Option<Vec<u8>>,
    pub protocol: Option<Arc<Protocol>>,
    pub header: Vec<u8>,
    pub format: Format,
    pub source: Option<SourceRef>,
}
impl Create {
    pub fn raw() -> Self {
        Self {
            source_quality: None,
            source_revision: None,
            name: "Manual message".into(),
            body: None,
            protocol: None,
            header: vec![],
            format: Format::Bytes(Kind::Serial),
            source: None,
        }
    }
}
#[derive(Clone)]
pub struct Field {
    pub name: String,
    pub raw: String,
    pub engineering: String,
    pub display: String,
    pub editable: bool,
    pub invalid: bool,
}
#[derive(Clone)]
pub struct Template {
    pub source_quality: Option<exmon_protocol::classification::Quality>,
    pub source_revision: Option<u64>,
    pub id: u64,
    pub revision: u64,
    pub name: Arc<str>,
    pub bytes: Arc<[u8]>,
    pub hex: Arc<str>,
    pub header_hex: Arc<str>,
    pub protocol: Option<Arc<Protocol>>,
    pub fields: Arc<[Field]>,
    pub source: Option<SourceRef>,
    pub valid: bool,
    pub dirty: bool,
    pub error: Option<Error>,
    pub tx_count: u64,
    pub last: Option<Outcome>,
    pub last_requested: Option<usize>,
}
#[derive(Clone, Default)]
pub struct Snapshot {
    pub templates: Vec<Arc<Template>>,
    pub error: Option<Error>,
    pub failed: bool,
    pub preparing: bool,
    pub connected: bool,
    pub binding: Option<Binding>,
    pub history_incomplete: bool,
    pub tx: Option<transmission::Snapshot>,
}
pub enum Operation {
    Hex(String),
    Header(String),
    Field {
        index: usize,
        text: String,
        engineering: bool,
        label: bool,
    },
    Repair,
    Validate,
}
#[derive(Clone)]
pub struct ScheduleInput {
    pub interval: String,
    pub count: String,
    pub lateness: String,
    pub drift: Drift,
    pub missed: Missed,
}
impl Default for ScheduleInput {
    fn default() -> Self {
        Self {
            interval: "1000".into(),
            count: String::new(),
            lateness: String::new(),
            drift: Drift::FixedRate,
            missed: Missed::Skip,
        }
    }
}
impl ScheduleInput {
    fn options(&self) -> Result<Options, Error> {
        fn integer(s: &str) -> Result<u64, Error> {
            if s.is_empty() || s.len() > 20 || !s.bytes().all(|b| b.is_ascii_digit()) {
                return Err(Error::Invalid);
            }
            s.parse().map_err(|_| Error::Invalid)
        }
        let o = Options {
            interval_ms: u32::try_from(integer(&self.interval)?).map_err(|_| Error::Invalid)?,
            count: if self.count.is_empty() {
                None
            } else {
                Some(integer(&self.count)?)
            },
            max_lateness_ms: if self.lateness.is_empty() {
                None
            } else {
                Some(u32::try_from(integer(&self.lateness)?).map_err(|_| Error::Invalid)?)
            },
            drift: self.drift,
            missed: self.missed,
        };
        o.validate().map_err(|_| Error::Invalid)?;
        Ok(o)
    }
}
#[derive(Default)]
struct Control {
    schedule_versions: BTreeMap<u64, u64>,
    versions: BTreeMap<u64, u64>,
    epoch: u64,
    next: u64,
    binding: Option<Binding>,
    bind_serial: u64,
    connected: bool,
    allow_publish: bool,
    preparing: bool,
    dirty: BTreeSet<u64>,
    schedule_epoch: u64,
    handles: BTreeMap<u64, Handle>,
}
impl Control {
    fn advance(&mut self) -> Result<u64, Error> {
        self.epoch = self.epoch.checked_add(1).ok_or(Error::Exhausted)?;
        Ok(self.epoch)
    }
    fn stop_all(&mut self) {
        for h in self.handles.values() {
            h.stop();
        }
    }
}
struct Shared {
    control: Mutex<Control>,
    state: Mutex<Snapshot>,
    quit: AtomicBool,
}
enum Command {
    Create {
        epoch: u64,
        id: u64,
        value: Create,
    },
    Edit {
        epoch: u64,
        id: u64,
        op: Operation,
    },
    Schedule {
        epoch: u64,
        schedule_epoch: u64,
        id: u64,
        input: ScheduleInput,
    },
}
#[derive(Clone, Default)]
pub struct MockStatus {
    pub calls: u64,
    pub bytes: u64,
    pub last: Arc<[u8]>,
}
pub struct Editor {
    tx: Arc<TxWorker>,
    shared: Arc<Shared>,
    commands: SyncSender<Command>,
    thread: Option<JoinHandle<()>>,
    mock: Arc<Mutex<MockStatus>>,
}
impl Editor {
    pub fn spawn() -> Result<Self, Error> {
        let tx = Arc::new(TxWorker::spawn()?);
        let shared = Arc::new(Shared {
            control: Mutex::new(Control::default()),
            state: Mutex::new(Snapshot::default()),
            quit: AtomicBool::new(false),
        });
        let (commands, rx) = mpsc::sync_channel(8);
        let state = shared.clone();
        let source = tx.clone();
        let handle = thread::Builder::new()
            .name("exmon-outgoing".into())
            .spawn(move || run(state, source, rx))
            .map_err(|_| Error::Unavailable)?;
        Ok(Self {
            tx,
            shared,
            commands,
            thread: Some(handle),
            mock: Arc::new(Mutex::new(MockStatus::default())),
        })
    }
    fn enqueue(&self, c: Command) -> Result<(), Error> {
        self.commands.try_send(c).map_err(|e| match e {
            mpsc::TrySendError::Full(_) => Error::Busy,
            mpsc::TrySendError::Disconnected(_) => Error::Unavailable,
        })?;
        if let Some(t) = &self.thread {
            t.thread().unpark();
        }
        Ok(())
    }
    fn mutation(&self, c: &mut Control) -> Result<u64, Error> {
        if let Some(s) = self.tx.snapshot()
            && s.ticket
                .as_ref()
                .is_some_and(|t| Some(t.binding()) == c.binding)
        {
            c.connected = true;
        }
        c.stop_all();
        c.allow_publish = false;
        c.preparing = true;
        self.tx.invalidate()?;
        let epoch = match c.advance() {
            Ok(epoch) => epoch,
            Err(error) => {
                c.connected = false;
                c.binding = None;
                c.preparing = false;
                self.tx.close();
                return Err(error);
            }
        };
        Ok(epoch)
    }
    pub fn invalidate(&self, id: u64) -> Result<(), Error> {
        let mut c = self.shared.control.lock().map_err(|_| Error::Unavailable)?;
        if !self
            .shared
            .state
            .lock()
            .map_err(|_| Error::Unavailable)?
            .templates
            .iter()
            .any(|t| t.id == id)
        {
            return Err(Error::Unknown);
        }
        self.mutation(&mut c)?;
        let epoch = c.epoch;
        c.versions.insert(id, epoch);
        c.dirty.insert(id);
        Ok(())
    }
    pub fn create(&self, value: Create) -> Result<u64, Error> {
        if !exmon_domain::classification::valid_name(&value.name)
            || value.body.as_ref().is_some_and(|b| b.len() > 65536)
            || value.header.len() > 65536
        {
            return Err(Error::Invalid);
        }
        let mut c = self.shared.control.lock().map_err(|_| Error::Unavailable)?;
        let epoch = self.mutation(&mut c)?;
        c.next = c.next.checked_add(1).ok_or(Error::Exhausted)?;
        let id = c.next;
        c.versions.insert(id, epoch);
        if let Err(error) = self.enqueue(Command::Create { epoch, id, value }) {
            c.versions.remove(&id);
            return Err(error);
        }
        Ok(id)
    }
    pub fn clone_received(&self, row: &Arc<Row>) -> Result<u64, Error> {
        let bytes = row.normalized.as_ref().ok_or(Error::Invalid)?;
        if bytes.len() > 65536 || row.name.len() > 128 {
            return Err(Error::Invalid);
        }
        self.create(Create {
            source_quality: row.quality,
            source_revision: Some(row.revision),
            name: row.name.clone(),
            body: Some(bytes.to_vec()),
            protocol: row.protocol.clone(),
            header: vec![],
            format: Format::Bytes(Kind::Serial),
            source: Some(SourceRef {
                session: row.key.generation,
                interface: row.interface,
                epoch: row.epoch,
                observation: row.sequence,
            }),
        })
    }
    pub fn edit(&self, id: u64, op: Operation) -> Result<(), Error> {
        let text_len = match &op {
            Operation::Hex(t) | Operation::Header(t) => t.len(),
            Operation::Field { text, .. } => text.len(),
            _ => 0,
        };
        let mut c = self.shared.control.lock().map_err(|_| Error::Unavailable)?;
        if !self
            .shared
            .state
            .lock()
            .map_err(|_| Error::Unavailable)?
            .templates
            .iter()
            .any(|t| t.id == id)
        {
            return Err(Error::Unknown);
        }
        let epoch = self.mutation(&mut c)?;
        c.versions.insert(id, epoch);
        c.dirty.insert(id);
        if text_len > 3 * 65536 {
            return Err(Error::Capacity);
        }
        self.enqueue(Command::Edit { epoch, id, op })
    }
    pub fn bind_port(
        &self,
        key: Binding,
        expected: Expected,
        permission: bool,
        factory: impl FnOnce() -> Result<Box<dyn Port>, BackendError> + Send + 'static,
    ) -> Result<(), Error> {
        let mut c = self.shared.control.lock().map_err(|_| Error::Unavailable)?;
        self.mutation(&mut c)?;
        c.binding = Some(key);
        c.connected = false;
        c.preparing = true;
        c.allow_publish = true;
        c.bind_serial = c.bind_serial.checked_add(1).ok_or(Error::Exhausted)?;
        self.tx.bind(key, expected, permission, factory)?;
        Ok(())
    }
    pub fn bind_mock(&self, interface: u64, permission: bool) -> Result<(), Error> {
        if !(1..=2).contains(&interface) {
            self.disconnect();
            return Err(Error::Invalid);
        }
        let next_epoch = self
            .shared
            .control
            .lock()
            .map_err(|_| Error::Unavailable)?
            .bind_serial
            .checked_add(1);
        let Some(epoch) = next_epoch else {
            self.disconnect();
            return Err(Error::Exhausted);
        };
        let key = Binding {
            session: 1,
            interface,
            epoch,
            configuration: 1,
            protocol: 1,
        };
        let audit = self.mock.clone();
        self.bind_port(
            key,
            Expected::Exact(Identity::Serial {
                device: interface,
                revision: epoch,
            }),
            permission,
            move || Ok(Box::new(Mock { key, audit })),
        )
    }
    pub fn disconnect(&self) {
        self.tx.close();
        if let Ok(mut c) = self.shared.control.lock() {
            let _ = c.advance();
            c.stop_all();
            c.binding = None;
            c.connected = false;
            c.allow_publish = false;
            c.preparing = false;
        }
    }
    fn ready(&self, c: &Control, id: u64) -> Result<(Arc<Template>, transmission::Ticket), Error> {
        if c.preparing || !c.connected {
            return Err(Error::NotReady);
        }
        if c.dirty.contains(&id) {
            return Err(Error::Dirty);
        }
        let state = self.shared.state.lock().map_err(|_| Error::Unavailable)?;
        if state.failed {
            return Err(Error::Unavailable);
        }
        let view = state
            .templates
            .iter()
            .find(|t| t.id == id)
            .cloned()
            .ok_or(Error::Unknown)?;
        let source = state.tx.clone();
        drop(state);
        if !view.valid {
            return Err(Error::Validation);
        }
        let s = source.ok_or(Error::NotReady)?;
        let t = s
            .ticket
            .filter(|t| Some(t.binding()) == c.binding)
            .ok_or(Error::NotReady)?;
        if !s.templates.contains(&(id, view.revision)) || !self.tx.ticket_current(&t) {
            return Err(Error::NotReady);
        }
        Ok((view, t))
    }
    pub fn can_send(&self, id: u64) -> bool {
        self.shared
            .control
            .try_lock()
            .is_ok_and(|c| self.ready(&c, id).is_ok())
    }
    pub fn send_once(&self, id: u64) -> Result<u64, Error> {
        let c = self.shared.control.lock().map_err(|_| Error::Unavailable)?;
        let (v, t) = self.ready(&c, id)?;
        Ok(self.tx.send_once(&t, id, v.revision)?)
    }
    pub fn start_schedule(&self, id: u64, input: ScheduleInput) -> Result<(), Error> {
        if input.interval.len() > 20 || input.count.len() > 20 || input.lateness.len() > 20 {
            self.stop_schedule(id);
            return Err(Error::Invalid);
        }
        let mut c = self.shared.control.lock().map_err(|_| Error::Unavailable)?;
        if let Some(h) = c.handles.remove(&id) {
            h.stop();
        }
        c.schedule_epoch = match c.schedule_epoch.checked_add(1) {
            Some(next) => next,
            None => {
                c.stop_all();
                c.connected = false;
                c.allow_publish = false;
                self.tx.close();
                return Err(Error::Exhausted);
            }
        };
        self.ready(&c, id)?;
        let serial = c.schedule_epoch;
        c.schedule_versions.insert(id, serial);
        self.enqueue(Command::Schedule {
            epoch: c.epoch,
            schedule_epoch: c.schedule_epoch,
            id,
            input,
        })
    }
    pub fn stop_schedule(&self, id: u64) {
        if let Ok(mut c) = self.shared.control.lock() {
            if let Some(next) = c.schedule_epoch.checked_add(1) {
                c.schedule_epoch = next;
                if c.versions.contains_key(&id) {
                    c.schedule_versions.insert(id, next);
                }
            } else {
                c.stop_all();
                c.connected = false;
                self.tx.close();
            }
            if let Some(h) = c.handles.remove(&id) {
                h.stop();
            }
        }
    }
    pub fn snapshot(&self) -> Option<Snapshot> {
        let c = self.shared.control.try_lock().ok()?;
        let mut s = self.shared.state.try_lock().ok()?.clone();
        s.binding = c.binding;
        s.connected = c.connected;
        s.preparing = c.preparing;
        s.tx = self.tx.snapshot().or(s.tx);
        s.templates = s
            .templates
            .iter()
            .map(|t| {
                let mut v = (**t).clone();
                v.dirty = c.dirty.contains(&v.id);
                v.valid &= !v.dirty;
                Arc::new(v)
            })
            .collect();
        Some(s)
    }
    pub fn mock_status(&self) -> Option<MockStatus> {
        self.mock.try_lock().ok().map(|s| s.clone())
    }
}
impl Drop for Editor {
    fn drop(&mut self) {
        self.disconnect();
        self.shared.quit.store(true, Ordering::Release);
        if let Some(t) = self.thread.take() {
            t.thread().unpark();
            let _ = t.join();
        }
    }
}
struct Mock {
    key: Binding,
    audit: Arc<Mutex<MockStatus>>,
}
impl Port for Mock {
    fn info(&self) -> Result<Info, BackendError> {
        Ok(Info {
            identity: Identity::Serial {
                device: self.key.interface,
                revision: self.key.epoch,
            },
            writable: Availability::Available,
            max_bytes: 65536,
        })
    }
    fn metadata(&self, _: &Packet<'_>) -> Result<Metadata, BackendError> {
        Ok(Metadata::Serial {
            configuration_revision: self.key.epoch,
        })
    }
    fn submit(&mut self, p: &Packet<'_>, c: &Cancellation) -> Result<Progress, BackendError> {
        if c.is_cancelled() {
            return Err(BackendError::Cancelled);
        }
        let mut s = self.audit.lock().map_err(|_| BackendError::Backend)?;
        s.calls = s.calls.saturating_add(1);
        s.bytes = s.bytes.saturating_add(p.bytes().len() as u64);
        s.last = Arc::from(p.bytes());
        Ok(Progress::accepted(p.bytes().len()))
    }
    fn completion(&mut self) -> Result<Progress, BackendError> {
        Err(BackendError::InvalidState)
    }
    fn close(&mut self) -> Result<(), BackendError> {
        Ok(())
    }
}
#[derive(Clone)]
struct Entry {
    view: Arc<Template>,
    body: Vec<u8>,
    header: Vec<u8>,
    protocol: Option<Arc<Protocol>>,
    codec: Option<Arc<Codec>>,
    format: Format,
    cost: usize,
    needs_register: bool,
}
impl Entry {
    fn create(id: u64, value: Create) -> Result<Self, Error> {
        if value
            .protocol
            .as_ref()
            .is_some_and(|p| p.fields.len() > 256 || p.checksums.len() > 32)
            || value
                .source
                .is_some_and(|s| s.session == 0 || s.interface == 0 || s.epoch == 0)
        {
            return Err(Error::Invalid);
        }
        let body = if let Some(b) = value.body {
            b
        } else if let Some(p) = &value.protocol {
            let length = match p.framing {
                Framing::Fixed { length } => length as usize,
                _ => p
                    .fields
                    .iter()
                    .map(|f| f.offset.saturating_add(f.length) as usize)
                    .max()
                    .unwrap_or(0),
            };
            if length > 65536 {
                return Err(Error::Capacity);
            }
            vec![0; length]
        } else {
            vec![165, 0, 0]
        };
        let mut e = Self {
            view: Arc::new(Template {
                source_quality: value.source_quality,
                source_revision: value.source_revision,
                id,
                revision: 1,
                name: Arc::from(value.name),
                bytes: Arc::from([]),
                hex: Arc::from(""),
                header_hex: Arc::from(""),
                protocol: value.protocol.clone(),
                fields: Arc::from([]),
                source: value.source,
                valid: false,
                dirty: false,
                error: None,
                tx_count: 0,
                last: None,
                last_requested: None,
            }),
            body,
            header: value.header,
            protocol: value.protocol,
            codec: None,
            format: value.format,
            cost: 0,
            needs_register: true,
        };
        transmission::charge(&e.spec(1))?;
        if let Some(p) = &e.protocol {
            exmon_storage::config::save_project(&ProjectConfig {
                protocols: vec![(**p).clone()],
                ..Default::default()
            })
            .map_err(|_| Error::Validation)?;
        }
        e.codec = e
            .protocol
            .as_ref()
            .map(|p| Codec::new(p, &Registry::default()).map(Arc::new))
            .transpose()
            .map_err(|_| Error::Validation)?;
        e.refresh()?;
        Ok(e)
    }
    fn spec(&self, revision: u64) -> TemplateSpec {
        TemplateSpec {
            id: self.view.id,
            revision: self.view.revision,
            protocol_revision: revision,
            format: self.format,
            body: self.body.clone(),
            structured: self.protocol.as_ref().map(|p| Structured {
                protocol: p.clone(),
                registry: Registry::default(),
                header: self.header.clone(),
            }),
            source: self.view.source,
        }
    }
    fn refresh(&mut self) -> Result<(), Error> {
        let mut v = (*self.view).clone();
        v.bytes = Arc::from(self.body.as_slice());
        v.hex = Arc::from(hex(&self.body));
        v.header_hex = Arc::from(hex(&self.header));
        v.error = None;
        v.valid = true;
        let mut fields = vec![];
        if let Some(codec) = &self.codec {
            let draft = codec.decode(&self.body).map_err(|_| Error::Codec)?;
            let report = draft.inspect();
            v.valid = report.is_valid();
            let p = self.protocol.as_ref().ok_or(Error::Invalid)?;
            for f in report.fields {
                let d = p.fields.get(f.index).ok_or(Error::Invalid)?;
                let raw = literal_text(f.raw.as_ref());
                let engineering = match f.number {
                    Ok(Number::Integer(n)) => n.to_string(),
                    Ok(Number::Real(n)) => n.to_string(),
                    Err(_) => "Unavailable".into(),
                };
                fields.push(Field {
                    name: d.id.clone(),
                    raw,
                    engineering: engineering.clone(),
                    display: format!(
                        "{engineering} {}{}",
                        d.unit,
                        if f.violations.is_empty() {
                            String::new()
                        } else {
                            format!(" [{:?}]", f.violations[0])
                        }
                    ),
                    editable: d.representation != Representation::Derived,
                    invalid: !f.violations.is_empty(),
                });
            }
            if outbound::encode(&p.framing, p.escape, &self.header, &self.body).is_err() {
                v.valid = false;
            }
        }
        if let Format::Can { id_word, dlc } = self.format
            && exmon_capture::can::Frame::new(id_word, dlc, &self.body).is_err()
        {
            v.valid = false;
        }
        if !v.valid {
            v.error = Some(Error::Validation);
        }
        v.fields = Arc::from(fields);
        v.dirty = false;
        self.cost = transmission::charge(&self.spec(1))?
            + v.hex.len()
            + v.header_hex.len()
            + v.fields
                .iter()
                .map(|f| f.name.len() + f.raw.len() + f.engineering.len() + f.display.len() + 128)
                .sum::<usize>();
        self.view = Arc::new(v);
        Ok(())
    }
    fn edit(&mut self, op: Operation) -> Result<(), Error> {
        match op {
            Operation::Hex(text) => {
                let mut bytes = parse_hex(&text)?;
                if let Some(codec) = &self.codec {
                    codec.decode(&bytes).map_err(|_| Error::Codec)?;
                    let protocol = self.protocol.as_ref().ok_or(Error::Invalid)?;
                    // Whole-message HEX may resize unprotected tails. Existing checksum
                    // outputs remain owned by the codec, including across a resize.
                    for checksum in &protocol.checksums {
                        let start = checksum.offset as usize;
                        let end = start
                            .checked_add(checksum.length as usize)
                            .ok_or(Error::Codec)?;
                        if bytes.len() != self.body.len()
                            && (end > bytes.len() || end > self.body.len())
                        {
                            return Err(Error::Codec);
                        }
                        let retained = end.min(bytes.len()).min(self.body.len());
                        if start < retained && bytes[start..retained] != self.body[start..retained]
                        {
                            return Err(Error::Codec);
                        }
                    }
                    let mut draft = codec.decode(&self.body).map_err(|_| Error::Codec)?;
                    let mut runs = vec![];
                    let mut i = 0;
                    let common = bytes.len().min(self.body.len());
                    while i < common {
                        if bytes[i] == self.body[i] {
                            i += 1;
                            continue;
                        }
                        let start = i;
                        while i < common && bytes[i] != self.body[i] {
                            i += 1;
                        }
                        if runs.len() >= 256 {
                            return Err(Error::Capacity);
                        }
                        runs.push((start as u32, hex(&bytes[start..i])));
                    }
                    let edits: Vec<_> = runs
                        .iter()
                        .map(|(offset, text)| Edit::Hex {
                            offset: *offset,
                            text,
                        })
                        .collect();
                    draft.edit(&edits).map_err(|_| Error::Codec)?;
                    let updated = draft.encode();
                    for checksum in &protocol.checksums {
                        let start = checksum.offset as usize;
                        let end = start + checksum.length as usize;
                        if end <= bytes.len() && end <= updated.len() {
                            bytes[start..end].copy_from_slice(&updated[start..end]);
                        }
                    }
                    self.body = bytes;
                } else {
                    self.body = bytes;
                }
            }
            Operation::Header(text) => {
                if self.protocol.is_none() {
                    return Err(Error::Invalid);
                }
                self.header = parse_hex(&text)?;
            }
            Operation::Field {
                index,
                text,
                engineering,
                label,
            } => {
                let p = self.protocol.as_ref().ok_or(Error::Invalid)?;
                let def = p.fields.get(index).ok_or(Error::Invalid)?;
                let codec = self.codec.as_ref().ok_or(Error::Invalid)?;
                let mut draft = codec.decode(&self.body).map_err(|_| Error::Codec)?;
                if label {
                    draft.edit(&[Edit::Label {
                        index,
                        label: &text,
                    }])
                } else if engineering {
                    let number = if let Ok(n) = text.parse::<i128>() {
                        Number::Integer(n)
                    } else {
                        Number::real(text.parse::<f64>().map_err(|_| Error::Invalid)?)
                            .map_err(|_| Error::Invalid)?
                    };
                    draft.edit(&[Edit::Number {
                        index,
                        value: number,
                        tolerance: 1e-9,
                    }])
                } else {
                    let literal = match def.representation {
                        Representation::Unsigned => Literal::Unsigned {
                            value: text.parse().map_err(|_| Error::Invalid)?,
                        },
                        Representation::Signed => Literal::Signed {
                            value: text.parse().map_err(|_| Error::Invalid)?,
                        },
                        Representation::Float if def.length == 4 => Literal::Float32 {
                            bits: text.parse::<f32>().map_err(|_| Error::Invalid)?.to_bits(),
                        },
                        Representation::Float => Literal::Float64 {
                            bits: text.parse::<f64>().map_err(|_| Error::Invalid)?.to_bits(),
                        },
                        Representation::Bytes => Literal::Bytes {
                            value: parse_hex(&text)?,
                        },
                        Representation::Ascii | Representation::Utf8 => {
                            Literal::Text { value: text }
                        }
                        Representation::Derived => return Err(Error::Invalid),
                    };
                    draft.edit(&[Edit::Field {
                        index,
                        value: &literal,
                    }])
                }
                .map_err(|_| Error::Codec)?;
                self.body = draft.encode();
            }
            Operation::Repair => {
                let codec = self.codec.as_ref().ok_or(Error::Invalid)?;
                let mut draft = codec.decode(&self.body).map_err(|_| Error::Codec)?;
                draft.repair_checksums().map_err(|_| Error::Codec)?;
                self.body = draft.encode();
            }
            Operation::Validate => {}
        }
        let mut v = (*self.view).clone();
        v.revision = v.revision.checked_add(1).ok_or(Error::Exhausted)?;
        self.view = Arc::new(v);
        self.needs_register = true;
        self.refresh()
    }
}
fn literal_text(value: Option<&Literal>) -> String {
    match value {
        Some(Literal::Unsigned { value }) => value.to_string(),
        Some(Literal::Signed { value }) => value.to_string(),
        Some(Literal::Float32 { bits }) => f32::from_bits(*bits).to_string(),
        Some(Literal::Float64 { bits }) => f64::from_bits(*bits).to_string(),
        Some(Literal::Bytes { value }) => hex(value),
        Some(Literal::Text { value }) => value.clone(),
        None => "Unavailable".into(),
    }
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X} ")).collect()
}
fn parse_hex(text: &str) -> Result<Vec<u8>, Error> {
    if text.len() > 3 * 65536 {
        return Err(Error::Capacity);
    }
    let mut out = vec![];
    let mut high = None;
    for b in text.bytes() {
        if b.is_ascii_whitespace() {
            continue;
        }
        let n = match b {
            b'0'..=b'9' => b - b'0',
            b'a'..=b'f' => b - b'a' + 10,
            b'A'..=b'F' => b - b'A' + 10,
            _ => return Err(Error::Invalid),
        };
        if let Some(h) = high.take() {
            if out.len() == 65536 {
                return Err(Error::Capacity);
            }
            out.push(h * 16 + n);
        } else {
            high = Some(n)
        }
    }
    if high.is_some() {
        return Err(Error::Invalid);
    }
    Ok(out)
}
struct Exit {
    shared: Arc<Shared>,
    tx: Arc<TxWorker>,
}
impl Drop for Exit {
    fn drop(&mut self) {
        self.tx.close();
        if thread::panicking()
            && let Ok(mut s) = self.shared.state.lock()
        {
            s.failed = true;
            s.error = Some(Error::Unavailable);
        }
    }
}
fn run(shared: Arc<Shared>, tx: Arc<TxWorker>, rx: mpsc::Receiver<Command>) {
    let _exit = Exit {
        shared: shared.clone(),
        tx: tx.clone(),
    };
    let mut entries: BTreeMap<u64, Entry> = BTreeMap::new();
    let mut seen_bind = 0;
    let mut publishing: Option<(u64, u64, u64, Instant)> = None;
    let mut after = 0;
    while !shared.quit.load(Ordering::Acquire) {
        if shared.control.is_poisoned() {
            if let Ok(mut s) = shared.state.lock() {
                s.failed = true;
                s.error = Some(Error::Unavailable);
            }
            break;
        }
        let snapshot = tx
            .snapshot()
            .or_else(|| shared.state.lock().ok().and_then(|s| s.tx.clone()));
        if let Ok(mut c) = shared.control.lock() {
            if let Some(s) = &snapshot {
                if s.ticket
                    .as_ref()
                    .is_some_and(|t| Some(t.binding()) == c.binding)
                {
                    c.connected = true;
                }
                if s.identity.is_none() {
                    c.connected = false;
                }
            }
            if seen_bind != c.bind_serial {
                seen_bind = c.bind_serial;
                for e in entries.values_mut() {
                    let mut v = (*e.view).clone();
                    if let Some(next) = v.revision.checked_add(1) {
                        v.revision = next;
                        e.view = Arc::new(v);
                        e.needs_register = true;
                    } else {
                        c.allow_publish = false;
                    }
                }
                publishing = None;
            }
            if publishing
                .as_ref()
                .is_some_and(|(_, _, epoch, _)| *epoch != c.epoch)
            {
                publishing = None;
            }
        }
        if let Some(s) = &snapshot {
            for e in entries.values_mut() {
                if s.templates.contains(&(e.view.id, e.view.revision)) {
                    e.needs_register = false;
                }
            }
        }
        for event in tx.history(after, 4) {
            if event.sequence > after.saturating_add(1)
                && let Ok(mut s) = shared.state.lock()
            {
                s.history_incomplete = true;
            }
            after = event.sequence;
            if event.outcome != Outcome::Pending
                && let Some(e) = entries.get_mut(&event.template)
            {
                let mut v = (*e.view).clone();
                if matches!(event.outcome,Outcome::Complete{accepted:Some(n),status:Completion::Accepted} if Some(n)==event.requested)
                {
                    v.tx_count = v.tx_count.saturating_add(1);
                }
                v.last = Some(event.outcome);
                v.last_requested = event.requested;
                e.view = Arc::new(v);
            }
        }
        if let Ok(command) = rx.try_recv() {
            let (epoch, id) = match &command {
                Command::Create { epoch, id, .. }
                | Command::Edit { epoch, id, .. }
                | Command::Schedule { epoch, id, .. } => (*epoch, *id),
            };
            let current = shared.control.lock().is_ok_and(|c| match &command {
                Command::Schedule { .. } => c.epoch == epoch,
                _ => c.versions.get(&id) == Some(&epoch),
            });
            if current {
                match command {
                    Command::Create { value, .. } => {
                        let result = Entry::create(id, value).and_then(|e| {
                            if entries.len() >= 64
                                || entries.values().map(|e| e.cost).sum::<usize>() + e.cost > LIMIT
                            {
                                Err(Error::Capacity)
                            } else {
                                Ok(e)
                            }
                        });
                        if let Ok(mut c) = shared.control.lock()
                            && c.versions.get(&id) == Some(&epoch)
                        {
                            match result {
                                Ok(e) => {
                                    entries.insert(id, e);
                                    if c.epoch == epoch {
                                        c.allow_publish = true;
                                    }
                                    if let Ok(mut s) = shared.state.lock() {
                                        s.error = None;
                                    }
                                }
                                Err(error) => {
                                    c.versions.remove(&id);
                                    c.schedule_versions.remove(&id);
                                    c.preparing = false;
                                    if let Ok(mut s) = shared.state.lock() {
                                        s.error = Some(error);
                                    }
                                }
                            }
                        }
                    }
                    Command::Edit { op, .. } => {
                        let result =
                            entries
                                .get(&id)
                                .cloned()
                                .ok_or(Error::Unknown)
                                .and_then(|mut e| {
                                    e.edit(op)?;
                                    if entries
                                        .iter()
                                        .filter(|(other, _)| **other != id)
                                        .map(|(_, e)| e.cost)
                                        .sum::<usize>()
                                        + e.cost
                                        > LIMIT
                                    {
                                        return Err(Error::Capacity);
                                    }
                                    Ok(e)
                                });
                        if let Ok(mut c) = shared.control.lock()
                            && c.versions.get(&id) == Some(&epoch)
                        {
                            match result {
                                Ok(e) => {
                                    entries.insert(id, e);
                                    c.dirty.remove(&id);
                                    if c.epoch == epoch {
                                        c.allow_publish = true;
                                    }
                                    if let Ok(mut s) = shared.state.lock() {
                                        s.error = None;
                                    }
                                }
                                Err(error) => {
                                    c.preparing = false;
                                    if let Some(e) = entries.get_mut(&id) {
                                        let mut v = (*e.view).clone();
                                        v.valid = false;
                                        v.error = Some(error);
                                        e.view = Arc::new(v);
                                    }
                                    if let Ok(mut s) = shared.state.lock() {
                                        s.error = Some(error);
                                    }
                                }
                            }
                        }
                    }
                    Command::Schedule {
                        schedule_epoch,
                        input,
                        ..
                    } => {
                        let options = input.options();
                        if let Ok(mut c) = shared.control.lock()
                            && c.epoch == epoch
                            && c.schedule_versions.get(&id) == Some(&schedule_epoch)
                        {
                            let result = options.and_then(|options| {
                                let e = entries.get(&id).ok_or(Error::Unknown)?;
                                if !e.view.valid || c.dirty.contains(&id) || c.preparing {
                                    return Err(Error::NotReady);
                                }
                                let ticket = snapshot
                                    .clone()
                                    .and_then(|s| s.ticket)
                                    .filter(|t| Some(t.binding()) == c.binding)
                                    .ok_or(Error::NotReady)?;
                                Ok(tx.enable_schedule(
                                    &ticket,
                                    scheduler::Spec {
                                        id,
                                        revision: schedule_epoch,
                                        template: id,
                                        template_revision: e.view.revision,
                                        options,
                                    },
                                )?)
                            });
                            match result {
                                Ok(handle) => {
                                    c.handles.insert(id, handle);
                                }
                                Err(error) => {
                                    if let Ok(mut s) = shared.state.lock() {
                                        s.error = Some(error);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        if let Some((id, revision, _, start)) = publishing {
            if snapshot
                .as_ref()
                .is_some_and(|s| s.templates.contains(&(id, revision)))
            {
                if let Some(e) = entries.get_mut(&id) {
                    e.needs_register = false;
                }
                publishing = None;
            } else if start.elapsed() > Duration::from_secs(1) {
                if let Some(e) = entries.get_mut(&id) {
                    let mut v = (*e.view).clone();
                    v.valid = false;
                    v.error = Some(Error::Unavailable);
                    e.view = Arc::new(v);
                }
                publishing = None;
            }
        }
        let candidate = if publishing.is_none() {
            shared.control.lock().ok().and_then(|c| {
                if c.allow_publish && c.connected {
                    entries
                        .values()
                        .find(|e| e.needs_register && e.view.valid && !c.dirty.contains(&e.view.id))
                        .map(|e| {
                            (
                                e.view.id,
                                e.view.revision,
                                c.epoch,
                                c.binding.map(|b| b.protocol).unwrap_or(0),
                                e.spec(c.binding.map(|b| b.protocol).unwrap_or(0)),
                            )
                        })
                } else {
                    None
                }
            })
        } else {
            None
        };
        if let Some((id, revision, epoch, _, spec)) = candidate
            && let Ok(mut c) = shared.control.lock()
            && c.epoch == epoch
            && c.allow_publish
            && c.connected
        {
            match tx.register(spec) {
                Ok(()) => publishing = Some((id, revision, epoch, Instant::now())),
                Err(error) => {
                    c.preparing = false;
                    c.allow_publish = false;
                    if let Some(e) = entries.get_mut(&id) {
                        let mut v = (*e.view).clone();
                        v.valid = false;
                        v.error = Some(error.into());
                        e.view = Arc::new(v);
                    }
                }
            }
        }
        if let Ok(mut c) = shared.control.lock() {
            if c.allow_publish
                && publishing.is_none()
                && !entries
                    .values()
                    .any(|e| e.needs_register && e.view.valid && !c.dirty.contains(&e.view.id))
            {
                c.preparing = false;
            }
            if let Ok(mut s) = shared.state.lock() {
                s.templates = entries.values().map(|e| e.view.clone()).collect();
                s.tx = snapshot.clone();
                if let Some(t) = &snapshot {
                    s.history_incomplete |= t.history_lost > 0 || t.history_evicted > 0;
                }
            }
        }
        thread::park_timeout(Duration::from_millis(1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn wait(mut f: impl FnMut() -> bool) {
        let end = Instant::now() + Duration::from_secs(3);
        while !f() {
            assert!(Instant::now() < end);
            thread::sleep(Duration::from_millis(1));
        }
    }
    fn ready_editor() -> (Editor, u64) {
        let e = Editor::spawn().unwrap();
        let id = e.create(Create::raw()).unwrap();
        wait(|| {
            e.snapshot()
                .is_some_and(|s| s.templates.iter().any(|t| t.id == id))
        });
        e.bind_mock(1, true).unwrap();
        wait(|| e.can_send(id));
        (e, id)
    }
    #[test]
    fn underlying_revocation_defeats_cached_ready_state_while_publication_is_blocked() {
        let (e, id) = ready_editor();
        let control = e.shared.control.lock().unwrap();
        assert!(
            e.shared
                .state
                .lock()
                .unwrap()
                .tx
                .as_ref()
                .unwrap()
                .ticket
                .is_some()
        );
        e.tx.close();
        wait(|| e.tx.snapshot().is_some_and(|s| s.ticket.is_none()));
        // The editor worker cannot refresh its cache while control is held.
        assert!(
            e.shared
                .state
                .lock()
                .unwrap()
                .tx
                .as_ref()
                .unwrap()
                .ticket
                .is_some()
        );
        assert!(matches!(e.ready(&control, id), Err(Error::NotReady)));
        assert_eq!(e.mock_status().unwrap().calls, 0);
    }
    #[test]
    fn exhausted_editor_authority_closes_existing_source_without_wrapping() {
        let (e, id) = ready_editor();
        e.shared.control.lock().unwrap().epoch = u64::MAX;
        assert_eq!(e.invalidate(id), Err(Error::Exhausted));
        assert!(!e.can_send(id));
        assert_eq!(e.shared.control.lock().unwrap().epoch, u64::MAX);
        assert_eq!(e.mock_status().unwrap().calls, 0);
    }
    #[test]
    fn stop_fences_valid_queued_enables_even_when_editor_command_queue_is_full() {
        let (e, id) = ready_editor();
        e.start_schedule(id, ScheduleInput::default()).unwrap();
        wait(|| {
            e.shared
                .control
                .try_lock()
                .is_ok_and(|c| c.handles.get(&id).is_some_and(|h| h.is_enabled()))
        });
        let c = e.shared.control.lock().unwrap();
        for _ in 0..8 {
            assert!(
                e.commands
                    .try_send(Command::Schedule {
                        epoch: c.epoch,
                        schedule_epoch: c.schedule_epoch,
                        id,
                        input: ScheduleInput::default()
                    })
                    .is_ok()
            );
        }
        assert!(matches!(
            e.commands.try_send(Command::Schedule {
                epoch: c.epoch,
                schedule_epoch: c.schedule_epoch,
                id,
                input: ScheduleInput::default()
            }),
            Err(mpsc::TrySendError::Full(_))
        ));
        drop(c);
        let start = Instant::now();
        e.stop_schedule(id);
        assert!(start.elapsed() < Duration::from_millis(100));
        thread::sleep(Duration::from_millis(1100));
        assert_eq!(e.mock_status().unwrap().calls, 0);
    }
    #[test]
    fn dropping_editor_joins_workers_and_leaves_no_orphan_payload_stream() {
        let (e, id) = ready_editor();
        e.start_schedule(
            id,
            ScheduleInput {
                interval: "10".into(),
                ..Default::default()
            },
        )
        .unwrap();
        wait(|| e.mock_status().is_some_and(|s| s.calls > 0));
        let audit = e.mock.clone();
        drop(e);
        let calls = audit.lock().unwrap().calls;
        thread::sleep(Duration::from_millis(30));
        assert_eq!(audit.lock().unwrap().calls, calls);
    }
}
