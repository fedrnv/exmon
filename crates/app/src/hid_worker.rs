//! Explicit selected-collection HID commands, always executed off the UI thread.
pub use exmon_capture::{
    adapter::Error,
    hid::{Device, MAX_REPORT, ReportMode, Source},
};
use exmon_capture::{
    adapter::{Adapter, Cancellation},
    hid::Hid,
};
use exmon_domain::observation::Observation;
use std::{
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread::{self, JoinHandle, ThreadId},
    time::Duration,
};
const REVOKED: u64 = 1;
const SUBMITTED: u64 = 2;
#[derive(Clone)]
pub struct Collection {
    pub id: u64,
    pub device: Device,
}
#[derive(Clone, Default)]
pub struct Snapshot {
    pub collections: Vec<Collection>,
    pub enumeration: u64,
    pub selected: Option<u64>,
    pub epoch: u64,
    pub open: bool,
    pub report_mode: Option<ReportMode>,
    pub capacity: usize,
    pub received: u64,
    pub dropped: u64,
    pub last_sent: Option<usize>,
    pub error: Option<Error>,
    pub failures: u64,
}
pub struct Report {
    pub id: u64,
    pub epoch: u64,
    pub observation: Observation,
}
enum Command {
    Discover,
    Open {
        id: u64,
        enumeration: u64,
        epoch: u64,
        mode: ReportMode,
        maximum: usize,
    },
    Send {
        id: u64,
        epoch: u64,
        hex: String,
    },
}
struct Shared {
    snapshot: Mutex<Snapshot>,
    authority: AtomicU64,
    next: AtomicU64,
    quit: AtomicBool,
    pending: AtomicBool,
    owner: OnceLock<ThreadId>,
}
impl Shared {
    fn update(&self, f: impl FnOnce(&mut Snapshot)) {
        if let Ok(mut state) = self.snapshot.lock() {
            f(&mut state);
        }
    }
    fn error(&self, error: Error) {
        self.update(|s| {
            s.error = Some(error);
            s.failures = s.failures.saturating_add(1);
        });
    }
}
pub struct HidWorker {
    shared: Arc<Shared>,
    commands: SyncSender<Command>,
    reports: Receiver<Report>,
    thread: Option<JoinHandle<()>>,
}
impl HidWorker {
    /// Factory runs inside the worker; it must not discover/open during construction.
    pub fn spawn<S: Source + 'static>(
        factory: impl FnOnce() -> S + Send + 'static,
    ) -> Result<Self, Error> {
        let shared = Arc::new(Shared {
            snapshot: Mutex::new(Snapshot::default()),
            authority: AtomicU64::new(REVOKED),
            next: AtomicU64::new(0),
            quit: AtomicBool::new(false),
            pending: AtomicBool::new(false),
            owner: OnceLock::new(),
        });
        let worker = shared.clone();
        let (commands, rx) = mpsc::sync_channel(4);
        let (tx, reports) = mpsc::sync_channel(8);
        let handle = thread::Builder::new()
            .name("exmon-hid".into())
            .spawn(move || {
                let _ = worker.owner.set(thread::current().id());
                let mut hid = Hid::new(factory(), ReportMode::Numbered);
                let mut active: Option<(u64, u64, usize)> = None;
                let mut enumeration = 0u64;
                let mut known = Vec::new();
                while !worker.quit.load(Ordering::Acquire) {
                    if worker.authority.load(Ordering::Acquire) & REVOKED != 0
                        && active.take().is_some()
                    {
                        let _ = hid.close();
                        worker.update(|s| {
                            s.open = false;
                            s.selected = None;
                        });
                    }
                    if let Ok(command) = rx.try_recv() {
                        match command {
                            Command::Discover => {
                                if active.is_some() {
                                    worker.error(Error::InvalidState);
                                } else {
                                    known.clear();
                                    let Some(next) = enumeration.checked_add(1) else {
                                        worker.error(Error::Exhausted);
                                        worker.pending.store(false, Ordering::Release);
                                        continue;
                                    };
                                    enumeration = next;
                                    worker.update(|s| {
                                        s.collections.clear();
                                        s.enumeration = enumeration;
                                        s.selected = None;
                                    });
                                    let _ = hid.close();
                                    match hid.enumerate(64) {
                                        Ok(descriptors) => {
                                            known = descriptors.iter().map(|d| d.id).collect();
                                            let collections = descriptors
                                                .into_iter()
                                                .filter_map(|d| {
                                                    hid.device(d.id).map(|device| Collection {
                                                        id: d.id,
                                                        device,
                                                    })
                                                })
                                                .collect();
                                            worker.update(|s| {
                                                s.collections = collections;
                                                s.enumeration = enumeration;
                                                s.selected = None;
                                                s.error = None;
                                            });
                                        }
                                        Err(error) => {
                                            worker.update(|s| s.collections.clear());
                                            worker.error(error);
                                        }
                                    }
                                }
                                worker.pending.store(false, Ordering::Release);
                            }
                            Command::Open {
                                id,
                                enumeration: expected,
                                epoch,
                                mode,
                                maximum,
                            } => {
                                let result = (|| {
                                    if active.is_some()
                                        || expected != enumeration
                                        || !known.contains(&id)
                                    {
                                        return Err(Error::InvalidState);
                                    }
                                    // The CAS is the submission boundary; Close can revoke queued operations.
                                    worker
                                        .authority
                                        .compare_exchange(
                                            epoch,
                                            epoch | SUBMITTED,
                                            Ordering::AcqRel,
                                            Ordering::Acquire,
                                        )
                                        .map_err(|_| Error::Cancelled)?;
                                    hid.set_report_mode(mode)?;
                                    hid.open(id)?;
                                    hid.configure(maximum)?;
                                    hid.start()
                                })();
                                let _ = worker.authority.compare_exchange(
                                    epoch | SUBMITTED,
                                    epoch,
                                    Ordering::AcqRel,
                                    Ordering::Acquire,
                                );
                                match result {
                                    Ok(()) if worker.authority.load(Ordering::Acquire) == epoch => {
                                        active = Some((id, epoch, maximum));
                                        worker.update(|s| {
                                            s.selected = Some(id);
                                            s.epoch = epoch;
                                            s.open = true;
                                            s.report_mode = Some(mode);
                                            s.capacity = maximum;
                                            s.error = None;
                                            s.last_sent = None;
                                            s.received = 0;
                                            s.dropped = 0;
                                        });
                                    }
                                    Ok(()) => {
                                        let _ = hid.close();
                                        worker.error(Error::Cancelled);
                                    }
                                    Err(error) => {
                                        let _ = hid.close();
                                        worker.error(error);
                                    }
                                }
                                worker.pending.store(false, Ordering::Release);
                            }
                            Command::Send { id, epoch, hex } => {
                                let result = (|| {
                                    if active.map(|a| (a.0, a.1)) != Some((id, epoch)) {
                                        return Err(Error::InvalidState);
                                    }
                                    let bytes =
                                        parse_report(&hex, active.ok_or(Error::InvalidState)?.2)?;
                                    worker
                                        .authority
                                        .compare_exchange(
                                            epoch,
                                            epoch | SUBMITTED,
                                            Ordering::AcqRel,
                                            Ordering::Acquire,
                                        )
                                        .map_err(|_| Error::Cancelled)?;
                                    hid.transmit(&bytes)
                                })();
                                let _ = worker.authority.compare_exchange(
                                    epoch | SUBMITTED,
                                    epoch,
                                    Ordering::AcqRel,
                                    Ordering::Acquire,
                                );
                                match result {
                                    Ok(count) => worker.update(|s| {
                                        s.last_sent = Some(count);
                                        s.error = None;
                                    }),
                                    Err(error) => {
                                        worker.error(error);
                                        if matches!(
                                            error,
                                            Error::Disconnected
                                                | Error::PermissionDenied
                                                | Error::Backend
                                        ) {
                                            worker.authority.fetch_or(REVOKED, Ordering::AcqRel);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if let Some((id, epoch, maximum)) = active
                        && worker.authority.load(Ordering::Acquire) == epoch
                    {
                        match hid.poll(&Cancellation::default(), 1, maximum) {
                            Ok(batch) => {
                                for observation in batch {
                                    if worker.authority.load(Ordering::Acquire) != epoch {
                                        break;
                                    }
                                    worker.update(|s| s.received = s.received.saturating_add(1));
                                    if tx
                                        .try_send(Report {
                                            id,
                                            epoch,
                                            observation,
                                        })
                                        .is_err()
                                    {
                                        worker.update(|s| s.dropped = s.dropped.saturating_add(1));
                                    }
                                }
                            }
                            Err(error) => {
                                worker.error(error);
                                worker.authority.fetch_or(REVOKED, Ordering::AcqRel);
                            }
                        }
                    }
                    thread::park_timeout(Duration::from_millis(if active.is_some() {
                        2
                    } else {
                        20
                    }));
                }
                let _ = hid.close();
                worker.update(|s| {
                    s.open = false;
                    s.selected = None;
                });
            })
            .map_err(|_| Error::Backend)?;
        Ok(Self {
            shared,
            commands,
            reports,
            thread: Some(handle),
        })
    }
    pub fn native() -> Result<Self, Error> {
        #[cfg(feature = "native-hid")]
        {
            Self::spawn(exmon_capture::native_hid::NativeHid::default)
        }
        #[cfg(not(feature = "native-hid"))]
        {
            Self::spawn(|| Unsupported)
        }
    }
    fn enqueue(&self, command: Command) -> Result<(), Error> {
        self.commands.try_send(command).map_err(|_| Error::Busy)?;
        if let Some(t) = &self.thread {
            t.thread().unpark();
        }
        Ok(())
    }
    pub fn discover(&self) -> Result<(), Error> {
        if self.snapshot().is_none_or(|s| s.open)
            || self.shared.pending.swap(true, Ordering::AcqRel)
        {
            return Err(Error::Busy);
        }
        if let Err(error) = self.enqueue(Command::Discover) {
            self.shared.pending.store(false, Ordering::Release);
            return Err(error);
        }
        Ok(())
    }
    pub fn open_selected(
        &self,
        id: u64,
        enumeration: u64,
        mode: ReportMode,
        maximum: usize,
    ) -> Result<(), Error> {
        if maximum == 0 || maximum > MAX_REPORT {
            return Err(Error::InvalidConfig);
        }
        if self.snapshot().is_none_or(|s| s.open)
            || self.shared.pending.swap(true, Ordering::AcqRel)
        {
            return Err(Error::Busy);
        }
        let next = self
            .shared
            .next
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                n.checked_add(1).filter(|n| *n <= u64::MAX >> 2)
            })
            .map_err(|_| {
                self.shared.pending.store(false, Ordering::Release);
                Error::Exhausted
            })?
            + 1;
        let epoch = next << 2;
        self.shared.authority.store(epoch, Ordering::Release);
        if let Err(error) = self.enqueue(Command::Open {
            id,
            enumeration,
            epoch,
            mode,
            maximum,
        }) {
            self.close();
            self.shared.pending.store(false, Ordering::Release);
            return Err(error);
        }
        Ok(())
    }
    /// Explicit report only; selected identity and generation are rechecked by worker.
    pub fn send_selected(&self, id: u64, epoch: u64, hex: String) -> Result<(), Error> {
        if hex.len() > MAX_REPORT * 3 {
            return Err(Error::InvalidConfig);
        }
        if self.shared.authority.load(Ordering::Acquire) != epoch {
            return Err(Error::Cancelled);
        }
        self.enqueue(Command::Send { id, epoch, hex })
    }
    pub fn close(&self) {
        self.shared.authority.fetch_or(REVOKED, Ordering::AcqRel);
        if let Some(t) = &self.thread {
            t.thread().unpark();
        }
    }
    pub fn pending(&self) -> bool {
        self.shared.pending.load(Ordering::Acquire)
    }
    pub fn revoked(&self) -> bool {
        self.shared.authority.load(Ordering::Acquire) & REVOKED != 0
    }
    pub fn snapshot(&self) -> Option<Snapshot> {
        self.shared.snapshot.try_lock().ok().map(|s| s.clone())
    }
    pub fn drain(&mut self, maximum: usize) -> Vec<Report> {
        (0..maximum.min(4))
            .filter_map(|_| self.reports.try_recv().ok())
            .collect()
    }
    pub fn worker_thread(&self) -> Option<ThreadId> {
        self.shared.owner.get().copied()
    }
}
impl Drop for HidWorker {
    fn drop(&mut self) {
        self.shared.quit.store(true, Ordering::Release);
        self.close();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}
fn parse_report(hex: &str, maximum: usize) -> Result<Vec<u8>, Error> {
    if hex.is_empty() || hex.len() > MAX_REPORT * 3 || !hex.is_ascii() {
        return Err(Error::InvalidConfig);
    }
    let mut bytes = Vec::new();
    let mut high = None;
    for b in hex.bytes() {
        if b.is_ascii_whitespace() {
            if high.is_some() {
                return Err(Error::InvalidConfig);
            }
            continue;
        }
        let n = match b {
            b'0'..=b'9' => b - b'0',
            b'a'..=b'f' => b - b'a' + 10,
            b'A'..=b'F' => b - b'A' + 10,
            _ => return Err(Error::InvalidConfig),
        };
        if let Some(h) = high.take() {
            if bytes.len() == maximum {
                return Err(Error::InvalidConfig);
            }
            bytes.push(h * 16 + n);
        } else {
            high = Some(n);
        }
    }
    if high.is_some() || bytes.is_empty() {
        Err(Error::InvalidConfig)
    } else {
        Ok(bytes)
    }
}
#[cfg(not(feature = "native-hid"))]
struct Unsupported;
#[cfg(not(feature = "native-hid"))]
impl Source for Unsupported {
    fn enumerate(&mut self, _: usize) -> Result<Vec<Device>, Error> {
        Err(Error::Unsupported)
    }
    fn open_selected(&mut self, _: usize) -> Result<(), Error> {
        Err(Error::Unsupported)
    }
    fn read(&mut self, _: &mut [u8], _: i32) -> Result<usize, Error> {
        Err(Error::Unsupported)
    }
    fn write(&mut self, _: &[u8]) -> Result<usize, Error> {
        Err(Error::Unsupported)
    }
    fn close(&mut self) {}
}
