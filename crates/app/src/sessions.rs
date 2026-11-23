//! Nonblocking session producers and transactional recording/recovery controller.
use exmon_domain::{config::ProjectConfig, observation::Observation};
pub use exmon_storage::sessions::{
    Entry, Event, Limits, Raw, Recovery, Sample, Source, Span, Summary,
};
/// The dedicated HID tool uses a disjoint session-local interface namespace.
pub const HID_INTERFACE_BASE: u64 = 1 << 63;
use exmon_storage::sessions::{Error, MAX_RECORD, Writer};
use std::{
    path::Path,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        mpsc::{self, SyncSender},
    },
    thread,
    time::Duration,
};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum State {
    #[default]
    Idle,
    Recording,
    Stopped,
    Recovered,
    Failed,
}
#[derive(Clone, Default)]
pub struct Snapshot {
    pub revision: u64,
    pub recovery: u64,
    pub state: State,
    pub summary: Option<Summary>,
    pub dropped: u64,
    pub last_loss: Option<Source>,
    pub notice: Option<&'static str>,
    pub entries: Vec<Entry>,
    pub more: bool,
    pub after: u64,
}
#[derive(Default)]
struct Shared {
    active: AtomicU64,
    desired: AtomicU64,
    next: AtomicU64,
    dropped: AtomicU64,
    last_loss: Mutex<Option<Source>>,
    gate: Mutex<()>,
    quit: AtomicBool,
    busy: AtomicBool,
    producers: AtomicUsize,
    wake: OnceLock<thread::Thread>,
    snapshot: Mutex<Arc<Snapshot>>,
}
enum Input {
    Raw(Vec<(Source, Observation)>),
    Rows(u64, Vec<Arc<crate::incoming::Row>>),
    Encoded(Vec<u8>),
    Note(Option<Source>, String),
    Charts(Vec<crate::charts::Spec>),
}
impl Input {
    fn scope(&self) -> Option<Source> {
        match self {
            Self::Raw(rows) => rows.last().map(|r| r.0),
            Self::Rows(generation, rows) => rows.last().map(|r| Source {
                generation: *generation,
                interface: r.interface,
                epoch: r.epoch,
                sequence: r.sequence,
            }),
            Self::Note(source, _) => *source,
            _ => None,
        }
    }
}
struct Batch {
    token: u64,
    input: Input,
    count: u64,
}
enum Command {
    Note {
        token: u64,
        source: Option<Source>,
        text: String,
    },
    Start {
        token: u64,
        path: String,
        limits: Limits,
        config: Arc<ProjectConfig>,
        charts: Vec<crate::charts::Spec>,
    },
    Recover(String),
    Page(u64),
}
#[derive(Clone)]
pub struct Client {
    shared: Arc<Shared>,
    data: SyncSender<Batch>,
}
struct Producer<'a>(&'a AtomicUsize);
impl<'a> Producer<'a> {
    fn enter(counter: &'a AtomicUsize) -> Self {
        counter.fetch_add(1, Ordering::SeqCst);
        Self(counter)
    }
}
impl Drop for Producer<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}
fn increment(value: &AtomicU64, n: u64) {
    let _ = value.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| {
        Some(v.saturating_add(n))
    });
}
fn loss(shared: &Shared, count: u64, scope: Option<Source>) {
    increment(&shared.dropped, count);
    if let Some(source) = scope
        && let Ok(mut loss) = shared.last_loss.try_lock()
    {
        *loss = Some(source);
    }
}
fn wake(shared: &Shared) {
    if let Some(thread) = shared.wake.get() {
        thread.unpark();
    }
}
impl Client {
    pub fn observation(&self, source: Source, observation: &Observation) -> bool {
        let token = self.token();
        if token == 0 {
            return false;
        }
        self.admit(token, Input::Raw(vec![(source, observation.clone())]), 1)
    }
    fn reject(&self, token: u64, count: u64, source: Option<Source>) {
        let _producer = Producer::enter(&self.shared.producers);
        if token != 0 && self.token() == token {
            loss(&self.shared, count, source);
        }
    }
    pub fn token(&self) -> u64 {
        self.shared.active.load(Ordering::SeqCst)
    }
    fn admit(&self, expected: u64, input: Input, count: u64) -> bool {
        if count == 0 {
            return true;
        }
        let _producer = Producer::enter(&self.shared.producers);
        let scope = input.scope();
        let Ok(_gate) = self.shared.gate.try_lock() else {
            if self.token() == expected && expected != 0 {
                loss(&self.shared, count, scope);
            }
            return false;
        };
        let token = self.token();
        if token == 0 || token != expected {
            return false;
        }
        if self
            .data
            .try_send(Batch {
                token,
                input,
                count,
            })
            .is_err()
        {
            loss(&self.shared, count, scope);
            false
        } else {
            wake(&self.shared);
            true
        }
    }
    pub fn raw(&self, generation: u64, records: &[crate::shell::Captured]) -> bool {
        let token = self.token();
        if token == 0 {
            return false;
        }
        if records.len() > 96
            || records
                .iter()
                .map(|r| r.observation.bytes().len() + 256)
                .sum::<usize>()
                > MAX_RECORD
        {
            self.reject(
                token,
                records.len() as u64,
                records.last().map(|r| Source {
                    generation,
                    interface: r.interface_id,
                    epoch: r.epoch,
                    sequence: r.sequence,
                }),
            );
            return false;
        }
        let rows = records
            .iter()
            .map(|r| {
                (
                    Source {
                        generation,
                        interface: r.interface_id,
                        epoch: r.epoch,
                        sequence: r.sequence,
                    },
                    r.observation.clone(),
                )
            })
            .collect();
        self.admit(token, Input::Raw(rows), records.len() as u64)
    }
    pub fn rows(&self, generation: u64, rows: &[Arc<crate::incoming::Row>]) -> bool {
        let token = self.token();
        if token == 0 {
            return false;
        }
        if rows.len() > 96 || rows.iter().map(|r| r.cost()).sum::<usize>() > MAX_RECORD {
            self.reject(
                token,
                rows.len() as u64,
                rows.last().map(|r| Source {
                    generation,
                    interface: r.interface,
                    epoch: r.epoch,
                    sequence: r.sequence,
                }),
            );
            return false;
        }
        let rows: Vec<_> = rows.iter().filter(|r| !r.raw()).cloned().collect();
        let count = rows.len() as u64;
        self.admit(token, Input::Rows(generation, rows), count)
    }
    /// Called on the incoming worker, never on the capture or GUI thread.
    pub fn definitions(&self, generation: u64, revision: u64, config: &ProjectConfig) -> bool {
        let token = self.token();
        if token == 0 {
            return false;
        }
        match (Event::Definitions {
            generation,
            revision,
            config: config.clone(),
        })
        .encode()
        {
            Ok(bytes) => self.admit(token, Input::Encoded(bytes), 1),
            Err(_) => {
                self.reject(token, 1, None);
                false
            }
        }
    }
    pub fn note(&self, source: Option<Source>, text: &str) -> Result<(), &'static str> {
        if text.is_empty() || text.len() > 4096 {
            return Err("Annotation must contain 1..4096 bytes");
        }
        // Bounded text is encoded by the recorder; admission never touches a file.
        if self.admit(self.token(), Input::Note(source, text.into()), 1) {
            Ok(())
        } else {
            Err("Recording is stopped or its queue is full")
        }
    }
}
fn prepare(input: Input) -> Result<Vec<Event>, Error> {
    Ok(match input {
        Input::Raw(rows) => rows
            .into_iter()
            .map(|(source, o)| Event::Raw(Raw::new(source, &o)))
            .collect(),
        Input::Encoded(bytes) => vec![Event::decode(&bytes)?],
        Input::Note(source, text) => vec![Event::Annotation { source, text }],
        Input::Charts(specs) => vec![Event::Charts {
            definitions: serde_json::to_value(specs).map_err(|_| Error::Invalid)?,
        }],
        Input::Rows(generation, rows) => rows
            .into_iter()
            .map(|row| Event::Message {
                timestamp_ns: row.observation.timestamp_ns(),
                source_timestamp_ns: row.observation.source_timestamp_ns(),
                source: Source {
                    generation,
                    interface: row.interface,
                    epoch: row.epoch,
                    sequence: row.sequence,
                },
                revision: row.revision,
                protocol: row
                    .protocol
                    .as_ref()
                    .map_or(String::new(), |p| p.id.clone()),
                name: row.name.clone(),
                group: row.group.clone().unwrap_or_default(),
                bytes: row.bytes().to_vec(),
                samples: row
                    .fields
                    .iter()
                    .map(|f| Sample {
                        field: f.name.clone(),
                        unit: row
                            .protocol
                            .as_ref()
                            .and_then(|p| p.fields.iter().find(|d| d.id == f.name))
                            .map_or(String::new(), |d| d.unit.clone()),
                        value: f.numeric.filter(|v| v.is_finite()),
                    })
                    .collect(),
                spans: row
                    .sources
                    .iter()
                    .map(|s| Span {
                        observation: s.observation,
                        offsets: s.offsets.clone(),
                        stream: s.stream.clone(),
                    })
                    .collect(),
            })
            .collect(),
    })
}
pub struct Controller {
    client: Client,
    commands: SyncSender<Command>,
}
impl Controller {
    pub fn new() -> Result<Self, &'static str> {
        let shared = Arc::new(Shared::default());
        let (data, rx) = mpsc::sync_channel::<Batch>(8);
        let (commands, cmd) = mpsc::sync_channel::<Command>(8);
        let state = shared.clone();
        thread::Builder::new()
            .name("session-store".into())
            .spawn(move || run(state, rx, cmd))
            .map_err(|_| "Session worker unavailable")?;
        Ok(Self {
            client: Client { shared, data },
            commands,
        })
    }
    pub fn client(&self) -> Client {
        self.client.clone()
    }
    pub fn busy(&self) -> bool {
        self.client.shared.busy.load(Ordering::Acquire)
    }
    pub fn requested(&self) -> bool {
        self.client.shared.desired.load(Ordering::Acquire) != 0
    }
    pub fn snapshot(&self) -> Option<Arc<Snapshot>> {
        let mut s = self.client.shared.snapshot.try_lock().ok()?.clone();
        let losses = self.client.shared.dropped.load(Ordering::Relaxed);
        if s.dropped != losses {
            Arc::make_mut(&mut s).dropped = losses;
        }
        if losses > 0
            && let Ok(scope) = self.client.shared.last_loss.try_lock()
        {
            Arc::make_mut(&mut s).last_loss = *scope;
        }
        Some(s)
    }
    pub fn start(
        &self,
        path: &str,
        limits: Limits,
        config: Arc<ProjectConfig>,
        charts: Vec<crate::charts::Spec>,
    ) -> Result<(), &'static str> {
        if path.is_empty()
            || path.len() > 4096
            || limits.validate().is_err()
            || charts.len() > 4
            || charts.iter().any(|s| !s.valid())
        {
            return Err("Invalid recording path or limits");
        }
        let _gate = self
            .client
            .shared
            .gate
            .lock()
            .map_err(|_| "Recorder unavailable")?;
        let shared = &self.client.shared;
        if shared.busy.load(Ordering::Acquire)
            || shared.desired.load(Ordering::Acquire) != 0
            || shared.active.load(Ordering::SeqCst) != 0
        {
            return Err("Recording already requested");
        }
        let token = shared
            .next
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |v| v.checked_add(1))
            .map_err(|_| "Recording identity exhausted")?
            + 1;
        shared.busy.store(true, Ordering::Release);
        shared.dropped.store(0, Ordering::Relaxed);
        if let Ok(mut scope) = shared.last_loss.lock() {
            *scope = None;
        }
        shared.desired.store(token, Ordering::Release);
        if self
            .commands
            .try_send(Command::Start {
                token,
                path: path.into(),
                limits,
                config,
                charts,
            })
            .is_err()
        {
            shared.desired.store(0, Ordering::Release);
            shared.busy.store(false, Ordering::Release);
            return Err("Session command queue full");
        }
        wake(shared);
        Ok(())
    }
    pub fn stop(&self) {
        if let Ok(_gate) = self.client.shared.gate.lock() {
            self.client.shared.active.store(0, Ordering::SeqCst);
            self.client.shared.desired.store(0, Ordering::Release);
        }
        wake(&self.client.shared);
    }
    pub fn note(&self, source: Option<Source>, text: &str) -> Result<(), &'static str> {
        if text.is_empty() || text.len() > 4096 {
            return Err("Annotation must contain 1..4096 bytes");
        }
        let _gate = self
            .client
            .shared
            .gate
            .lock()
            .map_err(|_| "Recorder unavailable")?;
        let token = self.client.token();
        if token == 0 {
            return Err("Recording is stopped");
        }
        self.commands
            .try_send(Command::Note {
                token,
                source,
                text: text.into(),
            })
            .inspect(|_| wake(&self.client.shared))
            .map_err(|_| "Session command queue full")
    }
    pub fn recover(&self, path: &str) -> Result<(), &'static str> {
        let _gate = self
            .client
            .shared
            .gate
            .lock()
            .map_err(|_| "Recorder unavailable")?;
        if self.busy() {
            return Err("Stop recording and wait for finalization before recovery");
        }
        if path.is_empty() || path.len() > 4096 {
            return Err("Invalid recovery path");
        }
        self.commands
            .try_send(Command::Recover(path.into()))
            .inspect(|_| wake(&self.client.shared))
            .map_err(|_| "Session command queue full")
    }
    pub fn page(&self, after: u64) -> Result<(), &'static str> {
        let _gate = self
            .client
            .shared
            .gate
            .lock()
            .map_err(|_| "Recorder unavailable")?;
        if self.busy() {
            return Err("Stop recording before browsing recovery");
        }
        self.commands
            .try_send(Command::Page(after))
            .inspect(|_| wake(&self.client.shared))
            .map_err(|_| "Session command queue full")
    }
    pub fn charts(&self, specs: &[crate::charts::Spec]) -> bool {
        if self.client.token() == 0 {
            return false;
        }
        if specs.len() > 4 || specs.iter().any(|s| !s.valid()) {
            return false;
        }
        // The existing chart controller owns validated small specifications. No sample processing here.
        self.client
            .admit(self.client.token(), Input::Charts(specs.to_vec()), 1)
    }
}
impl Drop for Controller {
    fn drop(&mut self) {
        self.stop();
        self.client.shared.quit.store(true, Ordering::Release);
        wake(&self.client.shared);
    }
}
fn error(e: Error) -> &'static str {
    match e {
        Error::Exists => "Recording destination exists; nothing overwritten",
        Error::Limit => "Recording budget or file limit reached; recording stopped",
        Error::Busy => "Session file is busy; committed data retained",
        Error::Version => "Unsupported session version",
        Error::Path => "Invalid local session path",
        _ => "Session I/O or validation failed; no automatic repair",
    }
}

fn run(state: Arc<Shared>, rx: mpsc::Receiver<Batch>, cmd: mpsc::Receiver<Command>) {
    let _ = state.wake.set(thread::current());
    let mut writer: Option<(u64, Writer)> = None;
    let mut recovered: Option<Recovery> = None;
    let mut view = Snapshot::default();
    let mut terminal = None;
    let publish = |view: &mut Snapshot| {
        view.revision = view.revision.saturating_add(1);
        view.dropped = state.dropped.load(Ordering::Relaxed);
        view.last_loss = if view.dropped > 0 {
            state.last_loss.try_lock().ok().and_then(|s| *s)
        } else {
            None
        };
        if let Ok(mut s) = state.snapshot.lock() {
            *s = Arc::new(view.clone());
        }
    };
    while !state.quit.load(Ordering::Acquire) {
        if writer
            .as_ref()
            .is_some_and(|(token, _)| state.desired.load(Ordering::Acquire) != *token)
            && state.producers.load(Ordering::SeqCst) == 0
            && let Some((token, mut store)) = writer.take()
        {
            let mut failure = terminal.take();
            for batch in rx.try_iter().take(8) {
                let scope = batch.input.scope();
                if batch.token == token
                    && let Err(e) = prepare(batch.input).and_then(|events| store.append(&events))
                {
                    loss(&state, batch.count, scope);
                    failure = Some(e);
                }
            }
            for command in cmd.try_iter().take(8) {
                if let Command::Note {
                    token: note_token,
                    source,
                    text,
                } = command
                    && note_token == token
                    && let Err(e) = store.append(&[Event::Annotation { source, text }])
                {
                    loss(&state, 1, source);
                    failure = Some(e);
                }
            }
            let dropped = state.dropped.load(Ordering::Relaxed);
            let scope = state.last_loss.lock().ok().and_then(|s| *s);
            let result = store.finish_with_loss(dropped, if dropped > 0 { scope } else { None });
            view.summary = Some(store.summary());
            view.state = if failure.is_some() || result.is_err() {
                State::Failed
            } else {
                State::Stopped
            };
            view.notice = Some(if let Some(e) = failure {
                error(e)
            } else if view.state == State::Failed {
                "Recording stopped with a storage failure; recording incomplete"
            } else if dropped > 0 {
                "Recording stopped; input loss makes it incomplete"
            } else {
                "Recording stopped; committed data finalized"
            });
            publish(&mut view);
            state.busy.store(false, Ordering::Release);
        }
        if let Ok(command) = cmd.try_recv() {
            match command {
                Command::Note {
                    token,
                    source,
                    text,
                } => {
                    if let Some((active, store)) = &mut writer
                        && *active == token
                    {
                        match store.append(&[Event::Annotation { source, text }]) {
                            Ok(()) => view.summary = Some(store.summary()),
                            Err(e) => {
                                loss(&state, 1, source);
                                terminal = Some(e);
                                state.active.store(0, Ordering::SeqCst);
                                state.desired.store(0, Ordering::Release);
                                view.notice = Some(error(e));
                            }
                        }
                    }
                }

                Command::Start {
                    token,
                    path,
                    limits,
                    config,
                    charts,
                } => {
                    if writer.is_some() {
                        continue;
                    }
                    if state.desired.load(Ordering::Acquire) != token {
                        view.state = State::Stopped;
                        view.notice = Some("Recording start cancelled; no input admitted");
                        publish(&mut view);
                        state.busy.store(false, Ordering::Release);
                        continue;
                    }
                    recovered = None;
                    view.entries.clear();
                    view.more = false;
                    view.summary = None;
                    state.dropped.store(0, Ordering::Relaxed);
                    if let Ok(mut source) = state.last_loss.lock() {
                        *source = None;
                    }
                    let result = Writer::create(Path::new(&path), limits).and_then(|mut store| {
                        let definitions = Event::Definitions {
                            generation: 0,
                            revision: 0,
                            config: (*config).clone(),
                        };
                        let charts = Event::Charts {
                            definitions: serde_json::to_value(charts)
                                .map_err(|_| Error::Invalid)?,
                        };
                        store.append(&[definitions, charts])?;
                        Ok(store)
                    });
                    match result {
                        Ok(store) => {
                            view.state = State::Recording;
                            view.summary = Some(store.summary());
                            view.notice = Some("Recording raw observations and prepared samples");
                            writer = Some((token, store));
                            let _gate = state.gate.lock();
                            if state.desired.load(Ordering::Acquire) == token {
                                state.active.store(token, Ordering::SeqCst);
                            }
                        }
                        Err(e) => {
                            state.desired.store(0, Ordering::Release);
                            state.busy.store(false, Ordering::Release);
                            view.state = State::Failed;
                            view.notice = Some(error(e));
                        }
                    }
                }
                Command::Recover(path) => {
                    if writer.is_none() {
                        match Recovery::open(Path::new(&path)).and_then(|r| {
                            let page = r.page(0)?;
                            let context = view.recovery.checked_add(1).ok_or(Error::Limit)?;
                            Ok((r, page, context))
                        }) {
                            Ok((r, entries, context)) => {
                                view.recovery = context;
                                state.dropped.store(r.summary.dropped, Ordering::Relaxed);
                                if let Ok(mut source) = state.last_loss.lock() {
                                    *source = r.summary.last_loss;
                                }
                                view.more =
                                    entries.last().is_some_and(|e| e.id < r.summary.records);
                                view.after = 0;
                                view.entries = entries;
                                view.summary = Some(r.summary.clone());
                                view.state = State::Recovered;
                                view.notice = Some(if !r.summary.closed {
                                    "Interrupted session: committed data recovered"
                                } else if r.summary.dropped > 0 {
                                    "Recovered session contains recording loss"
                                } else {
                                    "Clean session recovered; no capture or TX action requested"
                                });
                                recovered = Some(r);
                            }
                            Err(e) => view.notice = Some(error(e)),
                        }
                    }
                }
                Command::Page(after) => {
                    if let Some(r) = &recovered {
                        match r.page(after) {
                            Ok(entries) => {
                                view.more =
                                    entries.last().is_some_and(|e| e.id < r.summary.records);
                                view.after = after;
                                view.entries = entries;
                            }
                            Err(e) => view.notice = Some(error(e)),
                        }
                    }
                }
            }
            publish(&mut view);
        }
        if let Ok(batch) = rx.try_recv() {
            let scope = batch.input.scope();
            if let Some((token, store)) = &mut writer
                && batch.token == *token
            {
                match prepare(batch.input).and_then(|events| store.append(&events)) {
                    Ok(()) => view.summary = Some(store.summary()),
                    Err(e) => {
                        loss(&state, batch.count, scope);
                        state.active.store(0, Ordering::SeqCst);
                        state.desired.store(0, Ordering::Release);
                        terminal = Some(e);
                        view.state = State::Failed;
                        view.notice = Some(error(e));
                    }
                }
                publish(&mut view);
            }
        }
        thread::park_timeout(Duration::from_millis(if writer.is_some() {
            2
        } else {
            100
        }));
    }
    // No implicit clean completion: process shutdown may not drain all admitted input.
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;
    #[test]
    fn cancelled_queued_start_is_acknowledged_without_creating_file() {
        let path = std::env::temp_dir().join(format!(
            "exmon031-cancel-{}-{}.sqlite",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let shared = Arc::new(Shared::default());
        shared.busy.store(true, Ordering::Release);
        let (_data, rx) = mpsc::sync_channel(8);
        let (commands, cmd) = mpsc::sync_channel(8);
        commands
            .send(Command::Start {
                token: 1,
                path: path.to_str().unwrap().into(),
                limits: Limits::default(),
                config: Arc::new(ProjectConfig::default()),
                charts: vec![],
            })
            .unwrap();
        let state = shared.clone();
        let thread = thread::spawn(move || run(state, rx, cmd));
        let end = Instant::now() + Duration::from_secs(3);
        loop {
            if shared
                .snapshot
                .try_lock()
                .ok()
                .is_some_and(|s| s.state == State::Stopped)
            {
                break;
            }
            assert!(Instant::now() < end);
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(!path.exists());
        assert_eq!(shared.active.load(Ordering::SeqCst), 0);
        shared.quit.store(true, Ordering::Release);
        wake(&shared);
        thread.join().unwrap();
    }
    #[test]
    fn stop_finalization_fences_inflight_loss_accounting_without_waiting_on_gui() {
        let path = std::env::temp_dir().join(format!(
            "exmon031-loss-fence-{}-{}.sqlite",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let controller = Controller::new().unwrap();
        controller
            .start(
                path.to_str().unwrap(),
                Limits::default(),
                Arc::new(ProjectConfig::default()),
                vec![],
            )
            .unwrap();
        let end = Instant::now() + Duration::from_secs(4);
        while controller.client.token() == 0 {
            assert!(Instant::now() < end);
            thread::sleep(Duration::from_millis(2));
        }
        let producer = Producer::enter(&controller.client.shared.producers);
        let start = Instant::now();
        controller.stop();
        assert!(start.elapsed() < Duration::from_millis(100));
        assert!(controller.busy());
        let source = Source {
            generation: 1,
            interface: 2,
            epoch: 3,
            sequence: 4,
        };
        loss(&controller.client.shared, 5, Some(source));
        drop(producer);
        let end = Instant::now() + Duration::from_secs(4);
        while controller.busy() {
            assert!(Instant::now() < end);
            thread::sleep(Duration::from_millis(2));
        }
        let r = Recovery::open(&path).unwrap();
        assert_eq!(r.summary.dropped, 5);
        assert_eq!(r.summary.last_loss, Some(source));
        assert!(r.summary.closed);
        drop(r);
        drop(controller);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn full_data_queue_stop_and_stale_token_are_independent_of_store_progress() {
        let path = std::env::temp_dir().join(format!(
            "exmon031-queue-{}-{}.sqlite",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let controller = Controller::new().unwrap();
        controller
            .start(
                path.to_str().unwrap(),
                Limits::default(),
                Arc::new(ProjectConfig::default()),
                vec![],
            )
            .unwrap();
        let end = Instant::now() + Duration::from_secs(4);
        while controller.client.token() == 0 {
            assert!(Instant::now() < end);
            thread::sleep(Duration::from_millis(2));
        }
        let token = controller.client.token();
        let guard = controller.client.shared.snapshot.lock().unwrap();
        let start = Instant::now();
        let mut rejected = 0;
        for n in 1..=64 {
            let record = crate::shell::Captured {
                interface_id: 1,
                epoch: 2,
                sequence: n,
                observation: Observation::new(
                    0,
                    exmon_domain::Direction::Rx,
                    exmon_domain::observation::Metadata::Serial {
                        configuration_revision: 1,
                    },
                    None,
                    b"bounded",
                )
                .unwrap(),
            };
            if !controller.client.raw(7, &[record]) {
                rejected += 1;
            }
        }
        assert!(start.elapsed() < Duration::from_millis(100));
        assert!(rejected > 0);
        controller
            .note(None, "Priority annotation survives a full data queue")
            .unwrap();
        let start = Instant::now();
        controller.stop();
        assert!(start.elapsed() < Duration::from_millis(100));
        assert_eq!(controller.client.token(), 0);
        assert!(controller.busy());
        assert!(
            controller
                .start(
                    path.to_str().unwrap(),
                    Limits::default(),
                    Arc::new(ProjectConfig::default()),
                    vec![]
                )
                .is_err()
        );
        assert!(controller.recover(path.to_str().unwrap()).is_err());
        assert!(
            !controller
                .client
                .admit(token, Input::Note(None, "stale".into()), 1)
        );
        drop(guard);
        let end = Instant::now() + Duration::from_secs(4);
        loop {
            if controller
                .snapshot()
                .is_some_and(|s| s.state == State::Stopped)
            {
                break;
            }
            assert!(Instant::now() < end);
            thread::sleep(Duration::from_millis(2));
        }
        let recovered = Recovery::open(&path).unwrap();
        assert!(recovered.summary.closed);
        assert_eq!(recovered.summary.dropped, rejected);
        assert_eq!(recovered.summary.last_loss.unwrap().generation, 7);
        assert!(recovered.page(0).unwrap().iter().any(|e|matches!(e.event.as_ref(),Event::Annotation{text,..}if text=="Priority annotation survives a full data queue")));
        drop(recovered);
        drop(controller);
        std::fs::remove_file(path).unwrap();
    }
}
