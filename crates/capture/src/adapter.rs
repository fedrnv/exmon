//! Background-worker API. No method may run on the GUI thread.
use exmon_domain::{
    metrics::{Counter, Metrics},
    observation::{MAX_PAYLOAD, Observation, TransportKind},
};
use std::{
    collections::VecDeque,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    InvalidState,
    InvalidConfig,
    Unsupported,
    PermissionDenied,
    Disconnected,
    Cancelled,
    Backend,
    Exhausted,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::InvalidState => "operation requires a different adapter lifecycle state",
            Self::InvalidConfig => "capture limits or configuration are invalid",
            Self::Unsupported => "adapter capability is unsupported",
            Self::PermissionDenied => "adapter access denied; configure permissions manually",
            Self::Disconnected => "interface disconnected; explicit reconnect required",
            Self::Cancelled => "capture cancelled",
            Self::Backend => "adapter failed or violated its bounded event contract",
            Self::Exhausted => "capture epoch or sequence exhausted",
        })
    }
}
impl std::error::Error for Error {}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Availability {
    Available,
    Unavailable(Error),
    Unsupported,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Descriptor {
    pub id: u64,
    pub kind: TransportKind,
    pub rx: Availability,
    pub tx: Availability,
}

#[derive(Clone, Default)]
pub struct Cancellation(Arc<AtomicBool>);
impl Cancellation {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub queue_items: usize,
    pub queue_bytes: usize,
    pub batch_items: usize,
    pub batch_bytes: usize,
}
impl Limits {
    pub fn validate(self) -> Result<(), Error> {
        if self.queue_items == 0
            || self.queue_items > 4096
            || self.queue_bytes == 0
            || self.queue_bytes > 16 * 1024 * 1024
            || self.batch_items == 0
            || self.batch_items > 64
            || self.batch_bytes == 0
            || self.batch_bytes > MAX_PAYLOAD
        {
            Err(Error::InvalidConfig)
        } else {
            Ok(())
        }
    }
}

/// Trusted adapters must honor bounds before allocation and cancellation during
/// finite polling. Enum/discovery may return at most the requested limit (<=64).
pub trait Adapter: Send {
    fn enumerate(&mut self, limit: usize) -> Result<Vec<Descriptor>, Error>;
    fn open(&mut self, id: u64) -> Result<Descriptor, Error>;
    fn configure(&mut self, snapshot_length: usize) -> Result<(), Error>;
    fn start(&mut self) -> Result<(), Error>;
    fn poll(
        &mut self,
        cancellation: &Cancellation,
        max_items: usize,
        max_bytes: usize,
    ) -> Result<Vec<Observation>, Error>;
    fn stop(&mut self) -> Result<(), Error>;
    fn close(&mut self) -> Result<(), Error>;
    /// Coalesced report: None = no new report; Some(None) = unknown lost count.
    fn take_backend_loss(&mut self) -> Option<Option<u64>> {
        None
    }
    /// Only a future explicitly authorized TX worker may call this hook.
    /// Never called by Session lifecycle. Implementations default RX-only.
    fn transmit(&mut self, _bytes: &[u8]) -> Result<usize, Error> {
        Err(Error::Unsupported)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Closed,
    Open,
    Running,
    Stopped,
    Disconnected,
    Failed,
}
pub struct Captured {
    pub interface_id: u64,
    pub epoch: u64,
    pub sequence: u64,
    pub observation: Observation,
}

pub struct Session<A: Adapter> {
    adapter: A,
    limits: Limits,
    state: State,
    descriptor: Option<Descriptor>,
    configured: bool,
    snapshot_length: usize,
    epoch: u64,
    sequence: u64,
    cancellation: Cancellation,
    queue: VecDeque<Captured>,
    bytes: usize,
    metrics: Arc<Metrics>,
    last_error: Option<Error>,
}
impl<A: Adapter> Session<A> {
    pub fn new(adapter: A, limits: Limits) -> Result<Self, Error> {
        limits.validate()?;
        Ok(Self {
            adapter,
            limits,
            state: State::Closed,
            descriptor: None,
            configured: false,
            snapshot_length: 0,
            epoch: 0,
            sequence: 0,
            cancellation: Cancellation::default(),
            queue: VecDeque::new(),
            bytes: 0,
            metrics: Arc::new(Metrics::default()),
            last_error: None,
        })
    }
    pub fn state(&self) -> State {
        self.state
    }
    pub fn last_error(&self) -> Option<Error> {
        self.last_error
    }
    pub fn metrics(&self) -> Arc<Metrics> {
        self.metrics.clone()
    }
    pub fn cancellation(&self) -> Cancellation {
        self.cancellation.clone()
    }
    pub fn queued_bytes(&self) -> usize {
        self.bytes
    }
    pub fn queued_items(&self) -> usize {
        self.queue.len()
    }
    pub fn enumerate(&mut self) -> Result<Vec<Descriptor>, Error> {
        let items = self.adapter.enumerate(64)?;
        if items.len() > 64 {
            return Err(Error::Backend);
        }
        Ok(items)
    }
    pub fn open(&mut self, id: u64) -> Result<Descriptor, Error> {
        if self.state != State::Closed {
            return Err(Error::InvalidState);
        }
        let descriptor = self.adapter.open(id)?;
        if descriptor.id != id {
            let _ = self.adapter.close();
            return Err(Error::Backend);
        }
        self.descriptor = Some(descriptor);
        self.state = State::Open;
        Ok(descriptor)
    }
    pub fn configure(&mut self, snapshot_length: usize) -> Result<(), Error> {
        if !matches!(self.state, State::Open | State::Stopped) {
            return Err(Error::InvalidState);
        }
        self.configured = false;
        if snapshot_length == 0 || snapshot_length > self.limits.batch_bytes {
            return Err(Error::InvalidConfig);
        }
        self.adapter.configure(snapshot_length)?;
        self.snapshot_length = snapshot_length;
        self.configured = true;
        Ok(())
    }
    pub fn start(&mut self) -> Result<(), Error> {
        if !matches!(self.state, State::Open | State::Stopped) || !self.configured {
            return Err(Error::InvalidState);
        }
        match self.descriptor.ok_or(Error::InvalidState)?.rx {
            Availability::Available => (),
            Availability::Unavailable(e) => return Err(e),
            Availability::Unsupported => return Err(Error::Unsupported),
        }
        let epoch = self.epoch.checked_add(1).ok_or(Error::Exhausted)?;
        self.adapter.start().map_err(|e| self.fail(e))?;
        self.clear();
        self.epoch = epoch;
        self.sequence = 0;
        self.cancellation = Cancellation::default();
        self.metrics = Arc::new(Metrics::default());
        self.metrics.set_connected(true);
        self.last_error = None;
        self.state = State::Running;
        Ok(())
    }
    fn clear(&mut self) {
        self.metrics
            .add(Counter::ApplicationDrops, self.queue.len() as u64);
        self.queue.clear();
        self.bytes = 0;
        self.metrics.set_queue_depth(0);
    }
    fn fail(&mut self, error: Error) -> Error {
        self.configured = false;
        self.cancellation.cancel();
        self.metrics.set_connected(false);
        self.last_error = Some(error);
        self.state = if error == Error::Disconnected {
            State::Disconnected
        } else {
            State::Failed
        };
        let _ = self.adapter.stop();
        self.clear();
        error
    }
    pub fn pump(&mut self) -> Result<(), Error> {
        if self.state != State::Running {
            return Err(Error::InvalidState);
        }
        if self.cancellation.is_cancelled() {
            self.stop()?;
            return Err(Error::Cancelled);
        }
        let batch = self
            .adapter
            .poll(
                &self.cancellation,
                self.limits.batch_items,
                self.limits.batch_bytes,
            )
            .map_err(|e| self.fail(e))?;
        if self.cancellation.is_cancelled() {
            self.stop()?;
            return Err(Error::Cancelled);
        }
        if batch.len() > self.limits.batch_items {
            return Err(self.fail(Error::Backend));
        }
        let size: usize = batch.iter().map(|o| o.bytes().len()).sum();
        if size > self.limits.batch_bytes
            || batch.iter().any(|o| o.bytes().len() > self.snapshot_length)
        {
            return Err(self.fail(Error::Backend));
        }
        if let Some(loss) = self.adapter.take_backend_loss() {
            self.metrics.backend_loss(loss);
        }
        for observation in batch {
            if Some(observation.metadata().kind()) != self.descriptor.map(|d| d.kind) {
                return Err(self.fail(Error::Backend));
            }
            let sequence = self.sequence;
            self.sequence = self
                .sequence
                .checked_add(1)
                .ok_or_else(|| self.fail(Error::Exhausted))?;
            let length = observation.bytes().len();
            self.metrics.add(Counter::RxPackets, 1);
            self.metrics.add(Counter::RxBytes, length as u64);
            if self.queue.len() == self.limits.queue_items
                || length > self.limits.queue_bytes - self.bytes
            {
                self.metrics.add(Counter::ApplicationDrops, 1);
                continue;
            }
            self.queue.push_back(Captured {
                interface_id: self.descriptor.ok_or(Error::InvalidState)?.id,
                epoch: self.epoch,
                sequence,
                observation,
            });
            self.bytes += length;
        }
        self.metrics.set_queue_depth(self.queue.len() as u64);
        Ok(())
    }
    pub fn drain(&mut self, maximum: usize) -> Vec<Captured> {
        let count = maximum.min(self.limits.batch_items).min(self.queue.len());
        let items: Vec<_> = self.queue.drain(..count).collect();
        self.bytes -= items
            .iter()
            .map(|i| i.observation.bytes().len())
            .sum::<usize>();
        self.metrics.set_queue_depth(self.queue.len() as u64);
        items
    }
    pub fn stop(&mut self) -> Result<(), Error> {
        self.cancellation.cancel();
        self.metrics.set_connected(false);
        if self.state == State::Closed {
            return Ok(());
        }
        if let Err(e) = self.adapter.stop() {
            return Err(self.fail(e));
        }
        if matches!(self.state, State::Open | State::Running | State::Stopped) {
            self.state = State::Stopped;
        }
        Ok(())
    }
    pub fn close(&mut self) -> Result<(), Error> {
        let stopped = self.stop();
        let closed = self.adapter.close();
        self.clear();
        self.configured = false;
        if let Err(error) = closed {
            self.state = State::Failed;
            self.last_error = Some(error);
            return Err(error);
        }
        self.descriptor = None;
        self.state = State::Closed;
        stopped.and(closed)
    }
    /// Explicit reconnect closes first; never restarts capture or TX implicitly.
    pub fn reconnect(&mut self, id: u64) -> Result<Descriptor, Error> {
        self.close()?;
        self.open(id)
    }
}
impl<A: Adapter> Drop for Session<A> {
    fn drop(&mut self) {
        let _ = self.close();
    }
}
