//! Nonblocking desktop controllers. Capture and filesystem work stay in workers.
pub use exmon_capture::adapter::Captured;
use exmon_capture::{
    adapter::{Limits, Session},
    synthetic::Synthetic,
};
use exmon_domain::{metrics::Counter, observation::TransportKind};
pub use exmon_storage::layout::{Layout, LayoutError, Theme};
use std::{
    path::PathBuf,
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread::{self, JoinHandle, ThreadId},
    time::Duration,
};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellError {
    Busy,
    Unavailable,
    Invalid,
}
impl std::fmt::Display for ShellError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Busy => "worker command queue is busy",
            Self::Unavailable => "background worker unavailable",
            Self::Invalid => "invalid shell settings",
        })
    }
}
impl std::error::Error for ShellError {}
struct Control {
    quit: AtomicBool,
    desired: AtomicBool,
    generation: AtomicU64,
    running: AtomicBool,
    packets: AtomicU64,
    bytes: AtomicU64,
    decimated: AtomicU64,
    backend_loss: AtomicU64,
    application_drops: AtomicU64,
    failed: AtomicBool,
    owner: OnceLock<ThreadId>,
}
impl Default for Control {
    fn default() -> Self {
        Self {
            quit: AtomicBool::new(false),
            desired: AtomicBool::new(false),
            generation: AtomicU64::new(0),
            running: AtomicBool::new(false),
            packets: AtomicU64::new(0),
            bytes: AtomicU64::new(0),
            decimated: AtomicU64::new(0),
            backend_loss: AtomicU64::new(0),
            application_drops: AtomicU64::new(0),
            failed: AtomicBool::new(false),
            owner: OnceLock::new(),
        }
    }
}
struct Start {
    kind: TransportKind,
    interface: u64,
    generation: u64,
}
pub struct Batch {
    pub generation: u64,
    pub records: Vec<Captured>,
}
#[derive(Clone, Copy, Debug)]
pub struct CaptureStatus {
    pub requested: bool,
    pub running: bool,
    pub packets: u64,
    pub bytes: u64,
    pub display_decimation: u64,
    pub backend_loss: u64,
    pub application_drops: u64,
    pub failed: bool,
}
pub struct CaptureWorker {
    control: Arc<Control>,
    commands: SyncSender<Start>,
    results: Receiver<Batch>,
    thread: Option<JoinHandle<()>>,
}
impl CaptureWorker {
    pub fn spawn() -> Result<Self, ShellError> {
        let control = Arc::new(Control::default());
        let worker = control.clone();
        let (commands, rx) = mpsc::sync_channel::<Start>(4);
        let (tx, results) = mpsc::sync_channel(8);
        let handle = thread::Builder::new()
            .name("exmon-capture".into())
            .spawn(move || {
                let _ = worker.owner.set(thread::current().id());
                let mut session: Option<Session<Synthetic>> = None;
                let mut active = 0;
                while !worker.quit.load(Ordering::Acquire) {
                    if !worker.desired.load(Ordering::Acquire)
                        || worker.generation.load(Ordering::Acquire) != active
                    {
                        session.take();
                        worker.running.store(false, Ordering::Release);
                    }
                    if let Ok(start) = rx.try_recv()
                        && worker.desired.load(Ordering::Acquire)
                        && start.generation == worker.generation.load(Ordering::Acquire)
                    {
                        session.take();
                        let mut adapter = Synthetic::new(start.kind);
                        // The source itself rejects poll calls on a different thread.
                        debug_assert_eq!(adapter.owner(), thread::current().id());
                        use exmon_capture::adapter::Adapter;
                        let result = (|| {
                            adapter.enumerate(2)?;
                            let mut capture = Session::new(
                                adapter,
                                Limits {
                                    queue_items: 64,
                                    queue_bytes: 4096,
                                    batch_items: 32,
                                    batch_bytes: 2048,
                                },
                            )?;
                            capture.open(start.interface)?;
                            capture.configure(64)?;
                            capture.start()?;
                            Ok::<_, exmon_capture::adapter::Error>(capture)
                        })();
                        match result {
                            Ok(capture) => {
                                active = start.generation;
                                session = Some(capture);
                                worker.packets.store(0, Ordering::Relaxed);
                                worker.bytes.store(0, Ordering::Relaxed);
                                worker.decimated.store(0, Ordering::Relaxed);
                                worker.failed.store(false, Ordering::Relaxed);
                                worker.running.store(true, Ordering::Release);
                            }
                            Err(_) => {
                                worker.failed.store(true, Ordering::Relaxed);
                                worker.desired.store(false, Ordering::Release);
                            }
                        }
                    }
                    if let Some(capture) = session.as_mut() {
                        if capture.pump().is_err() {
                            worker.failed.store(true, Ordering::Relaxed);
                            worker.desired.store(false, Ordering::Release);
                        } else {
                            let metrics = capture.metrics().snapshot();
                            worker
                                .packets
                                .store(metrics.count(Counter::RxPackets), Ordering::Relaxed);
                            worker
                                .bytes
                                .store(metrics.count(Counter::RxBytes), Ordering::Relaxed);
                            worker
                                .backend_loss
                                .store(metrics.count(Counter::BackendLoss), Ordering::Relaxed);
                            worker
                                .application_drops
                                .store(metrics.count(Counter::ApplicationDrops), Ordering::Relaxed);
                            let records = capture.drain(32);
                            let count = records.len() as u64;
                            if tx
                                .try_send(Batch {
                                    generation: active,
                                    records,
                                })
                                .is_err()
                            {
                                let _ = worker.decimated.fetch_update(
                                    Ordering::Relaxed,
                                    Ordering::Relaxed,
                                    |n| Some(n.saturating_add(count)),
                                );
                            }
                        }
                    }
                    thread::park_timeout(Duration::from_millis(if session.is_some() {
                        2
                    } else {
                        100
                    }));
                }
                session.take();
                worker.running.store(false, Ordering::Release);
            })
            .map_err(|_| ShellError::Unavailable)?;
        Ok(Self {
            control,
            commands,
            results,
            thread: Some(handle),
        })
    }
    pub fn start(&self, kind: TransportKind, interface: u64) -> Result<(), ShellError> {
        if !(1..=2).contains(&interface) {
            return Err(ShellError::Invalid);
        }
        if self.control.desired.swap(true, Ordering::AcqRel) {
            return Err(ShellError::Busy);
        }
        let Some(generation) = self
            .control
            .generation
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| n.checked_add(1))
            .ok()
            .and_then(|n| n.checked_add(1))
        else {
            self.control.desired.store(false, Ordering::Release);
            return Err(ShellError::Unavailable);
        };
        if self
            .commands
            .try_send(Start {
                kind,
                interface,
                generation,
            })
            .is_err()
        {
            self.control.desired.store(false, Ordering::Release);
            return Err(ShellError::Busy);
        }
        if let Some(thread) = &self.thread {
            thread.thread().unpark();
        }
        Ok(())
    }
    pub fn stop(&self) {
        self.control.desired.store(false, Ordering::Release);
        if let Some(thread) = &self.thread {
            thread.thread().unpark();
        }
    }
    pub fn status(&self) -> CaptureStatus {
        CaptureStatus {
            requested: self.control.desired.load(Ordering::Acquire),
            running: self.control.running.load(Ordering::Acquire),
            packets: self.control.packets.load(Ordering::Relaxed),
            bytes: self.control.bytes.load(Ordering::Relaxed),
            display_decimation: self.control.decimated.load(Ordering::Relaxed),
            backend_loss: self.control.backend_loss.load(Ordering::Relaxed),
            application_drops: self.control.application_drops.load(Ordering::Relaxed),
            failed: self.control.failed.load(Ordering::Relaxed),
        }
    }
    pub fn generation(&self) -> u64 {
        self.control.generation.load(Ordering::Acquire)
    }
    pub fn worker_thread(&self) -> Option<ThreadId> {
        self.control.owner.get().copied()
    }
    /// Caller budget is hard-capped: never drain an unbounded producer queue.
    pub fn drain(&mut self, maximum: usize) -> Vec<Batch> {
        (0..maximum.min(8))
            .filter_map(|_| self.results.try_recv().ok())
            .collect()
    }
}
impl Drop for CaptureWorker {
    fn drop(&mut self) {
        self.control.quit.store(true, Ordering::Release);
        self.stop();
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }
}

pub struct PreferencesWorker {
    sender: Option<SyncSender<Layout>>,
    loaded: Receiver<Result<Option<Layout>, LayoutError>>,
    status: Arc<AtomicU8>,
    owner: Arc<OnceLock<ThreadId>>,
    thread: Option<JoinHandle<()>>,
}
impl PreferencesWorker {
    pub fn spawn(path: Option<PathBuf>) -> Result<Self, ShellError> {
        let (sender, receiver) = mpsc::sync_channel::<Layout>(1);
        let (loaded_tx, loaded) = mpsc::sync_channel(1);
        let status = Arc::new(AtomicU8::new(0));
        let worker_status = status.clone();
        let owner = Arc::new(OnceLock::new());
        let worker_owner = owner.clone();
        let handle = thread::Builder::new()
            .name("exmon-layout".into())
            .spawn(move || {
                let _ = worker_owner.set(thread::current().id());
                let initial = path
                    .as_ref()
                    .map_or(Err(LayoutError::Io), |p| exmon_storage::layout::load(p));
                if initial.is_err() {
                    worker_status.store(2, Ordering::Release);
                }
                let _ = loaded_tx.try_send(initial);
                while let Ok(mut layout) = receiver.recv() {
                    // Coalesce drag/resize events until quiet, bounded to one pending value.
                    while let Ok(next) = receiver.recv_timeout(Duration::from_millis(250)) {
                        layout = next;
                    }
                    let saved = path
                        .as_ref()
                        .is_some_and(|p| exmon_storage::layout::save(p, layout).is_ok());
                    worker_status.store(if saved { 1 } else { 2 }, Ordering::Release);
                }
            })
            .map_err(|_| ShellError::Unavailable)?;
        Ok(Self {
            sender: Some(sender),
            loaded,
            status,
            owner,
            thread: Some(handle),
        })
    }
    pub fn load_result(&self) -> Option<Result<Option<Layout>, LayoutError>> {
        self.loaded.try_recv().ok()
    }
    pub fn save(&self, layout: Layout) -> Result<(), ShellError> {
        if !layout.valid() {
            return Err(ShellError::Invalid);
        }
        self.sender
            .as_ref()
            .ok_or(ShellError::Unavailable)?
            .try_send(layout)
            .map_err(|_| ShellError::Busy)
    }
    pub fn failed(&self) -> bool {
        self.status.load(Ordering::Acquire) == 2
    }
    pub fn worker_thread(&self) -> Option<ThreadId> {
        self.owner.get().copied()
    }
}
impl Drop for PreferencesWorker {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }
}
pub fn default_layout_path() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    let root = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    #[cfg(target_os = "macos")]
    let root =
        std::env::var_os("HOME").map(|p| PathBuf::from(p).join("Library/Application Support"));
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let root = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".config")));
    root.filter(|p| p.is_absolute())
        .map(|p| p.join("ExchangeMonitor/layout-v1.json"))
}
