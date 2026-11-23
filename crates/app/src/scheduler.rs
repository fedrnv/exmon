//! Bounded deadline arithmetic and activation grants, never transport I/O.
use exmon_capture::adapter::Cancellation;
pub use exmon_domain::schedule::{Drift, Missed, Options};
use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeError {
    Exhausted,
}
/// Trusted nonblocking monotonic source. The worker detects backward values.
pub trait Clock: Send + Sync {
    fn now_ns(&self) -> Result<u64, TimeError>;
}
pub struct MonotonicClock(Instant);
impl Default for MonotonicClock {
    fn default() -> Self {
        Self(Instant::now())
    }
}
impl Clock for MonotonicClock {
    fn now_ns(&self) -> Result<u64, TimeError> {
        u64::try_from(self.0.elapsed().as_nanos()).map_err(|_| TimeError::Exhausted)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u64)]
pub enum Reason {
    User = 1,
    Replaced,
    Finished,
    Stale,
    Invalid,
    Missed,
    Busy,
    Partial,
    Backend,
    Clock,
    Exhausted,
    Shutdown,
    OwnerDropped,
}
fn reason(value: u64) -> Reason {
    match value {
        1 => Reason::User,
        2 => Reason::Replaced,
        3 => Reason::Finished,
        4 => Reason::Stale,
        5 => Reason::Invalid,
        6 => Reason::Missed,
        7 => Reason::Busy,
        8 => Reason::Partial,
        9 => Reason::Backend,
        10 => Reason::Clock,
        11 => Reason::Exhausted,
        12 => Reason::Shutdown,
        _ => Reason::OwnerDropped,
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Disabled,
    Enabled,
    Reserved,
    InFlight,
    Stopped(Reason),
}
#[derive(Debug, Clone, Default)]
pub struct Stats {
    pub attempts: u64,
    pub missed: u64,
    pub deadline_ns: Option<u64>,
    pub decision_ns: Option<u64>,
    pub submitted_ns: Option<u64>,
    pub completed_ns: Option<u64>,
    pub max_lateness_ns: u64,
}
#[derive(Debug, Clone, Copy)]
pub struct Pulse {
    pub deadline_ns: u64,
    pub decision_ns: u64,
}
pub struct Engine {
    options: Options,
    state: State,
    next: Option<u64>,
    reserved: Option<Pulse>,
    last: Option<u64>,
    stats: Stats,
}
impl Engine {
    pub fn new(options: Options) -> Result<Self, &'static str> {
        options.validate()?;
        Ok(Self {
            options,
            state: State::Disabled,
            next: None,
            reserved: None,
            last: None,
            stats: Stats::default(),
        })
    }
    pub fn state(&self) -> State {
        self.state
    }
    pub fn stats(&self) -> &Stats {
        &self.stats
    }
    pub fn next_ns(&self) -> Option<u64> {
        self.next
    }
    pub fn options(&self) -> Options {
        self.options
    }
    pub fn stop(&mut self, why: Reason) {
        if !matches!(self.state, State::Stopped(_)) {
            self.state = State::Stopped(why);
        }
        self.reserved = None;
        self.next = None;
    }
    fn observe(&mut self, now: u64) -> bool {
        if self.last.is_some_and(|last| now < last) {
            self.stop(Reason::Clock);
            return false;
        }
        self.last = Some(now);
        true
    }
    pub fn arm(&mut self, now: u64) {
        if self.state != State::Disabled {
            return;
        }
        self.last = Some(now);
        self.next = now.checked_add(self.options.interval_ns());
        if self.next.is_none() {
            self.stop(Reason::Exhausted)
        } else {
            self.state = State::Enabled
        }
    }
    pub fn offer(&mut self, now: u64) -> Option<Pulse> {
        if !self.observe(now) || self.state != State::Enabled {
            return None;
        }
        let next = self.next?;
        if now < next {
            return None;
        }
        let interval = self.options.interval_ns();
        let periods = (now - next) / interval + 1;
        let latest = next.checked_add((periods - 1).checked_mul(interval)?)?;
        let late = now - latest;
        if (periods > 1 || late > self.options.lateness_ns()) && self.options.missed == Missed::Stop
        {
            self.stats.missed = self.stats.missed.saturating_add(periods);
            self.stop(Reason::Missed);
            return None;
        }
        self.stats.missed = self.stats.missed.saturating_add(periods - 1);
        self.next = latest.checked_add(interval);
        if self.next.is_none() {
            self.stop(Reason::Exhausted);
            return None;
        }
        if late > self.options.lateness_ns() {
            self.stats.missed = self.stats.missed.saturating_add(1);
            if self.options.drift == Drift::FixedDelay {
                self.next = now.checked_add(interval);
                if self.next.is_none() {
                    self.stop(Reason::Exhausted)
                }
            }
            return None;
        }
        let pulse = Pulse {
            deadline_ns: latest,
            decision_ns: now,
        };
        self.reserved = Some(pulse);
        self.state = State::Reserved;
        self.stats.deadline_ns = Some(latest);
        self.stats.decision_ns = Some(now);
        Some(pulse)
    }
    pub fn skip(&mut self, now: u64) {
        if !self.observe(now) || self.state != State::Reserved {
            return;
        }
        self.stats.missed = self.stats.missed.saturating_add(1);
        self.reserved = None;
        if self.options.missed == Missed::Stop {
            self.stop(Reason::Missed)
        } else {
            self.state = State::Enabled;
            if self.options.drift == Drift::FixedDelay {
                self.next = now.checked_add(self.options.interval_ns());
                if self.next.is_none() {
                    self.stop(Reason::Exhausted)
                }
            }
        }
    }
    /// Called only after the final grant/connection fence, adjacent to source submission.
    pub fn commit(&mut self, now: u64) -> bool {
        if !self.observe(now) || self.state != State::Reserved {
            return false;
        }
        let Some(pulse) = self.reserved else {
            return false;
        };
        if now - pulse.deadline_ns > self.options.lateness_ns() {
            self.skip(now);
            return false;
        }
        let Some(attempts) = self.stats.attempts.checked_add(1) else {
            self.stop(Reason::Exhausted);
            return false;
        };
        self.stats.attempts = attempts;
        self.stats.submitted_ns = Some(now);
        self.stats.max_lateness_ns = self.stats.max_lateness_ns.max(now - pulse.deadline_ns);
        self.reserved = None;
        self.state = State::InFlight;
        true
    }
    pub fn finish(&mut self, now: u64) {
        if !matches!(self.state, State::InFlight | State::Stopped(_)) {
            return;
        }
        if !self.observe(now) {
            return;
        }
        self.stats.completed_ns = Some(now);
        if matches!(self.state, State::Stopped(_)) {
            return;
        }
        if self
            .options
            .count
            .is_some_and(|count| self.stats.attempts >= count)
        {
            self.stop(Reason::Finished);
            return;
        }
        self.state = State::Enabled;
        if self.options.drift == Drift::FixedDelay {
            self.next = now.checked_add(self.options.interval_ns());
            if self.next.is_none() {
                self.stop(Reason::Exhausted)
            }
        }
    }
    pub(crate) fn tick(&mut self, now: Result<u64, TimeError>) {
        if let Ok(now) = now {
            self.observe(now);
        } else {
            self.stop(Reason::Clock)
        }
    }
}

pub(crate) struct Grant {
    state: AtomicU64,
    pub(crate) cancel: Cancellation,
    pub(crate) activation: u64,
}
impl Grant {
    pub(crate) fn new(activation: u64) -> Arc<Self> {
        Arc::new(Self {
            state: AtomicU64::new(0),
            cancel: Cancellation::default(),
            activation,
        })
    }
    pub(crate) fn activate(&self) -> bool {
        self.state
            .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }
    pub(crate) fn acquire(&self) -> bool {
        self.state
            .compare_exchange(1, 2, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }
    pub(crate) fn release(&self) {
        let _ = self
            .state
            .compare_exchange(2, 1, Ordering::AcqRel, Ordering::Acquire);
    }
    pub(crate) fn stop(&self, why: Reason) {
        let _ = self
            .state
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |old| {
                if old & 3 == 3 {
                    None
                } else {
                    Some(3 | ((why as u64) << 2))
                }
            });
        self.cancel.cancel();
    }
    pub(crate) fn stopped(&self) -> Option<Reason> {
        let value = self.state.load(Ordering::Acquire);
        if value & 3 == 3 {
            Some(reason(value >> 2))
        } else {
            None
        }
    }
    pub(crate) fn enabled(&self) -> bool {
        matches!(self.state.load(Ordering::Acquire), 1 | 2)
    }
}
struct Control(Arc<Grant>);
impl Drop for Control {
    fn drop(&mut self) {
        self.0.stop(Reason::OwnerDropped)
    }
}
#[derive(Clone)]
pub struct Handle {
    control: Arc<Control>,
}
impl Handle {
    pub(crate) fn new(grant: Arc<Grant>) -> Self {
        Self {
            control: Arc::new(Control(grant)),
        }
    }
    pub fn stop(&self) {
        self.control.0.stop(Reason::User)
    }
    pub fn is_enabled(&self) -> bool {
        self.control.0.enabled()
    }
    pub fn activation(&self) -> u64 {
        self.control.0.activation
    }
}
impl std::fmt::Debug for Handle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScheduleHandle")
            .field("enabled", &self.is_enabled())
            .finish()
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spec {
    pub id: u64,
    pub revision: u64,
    pub template: u64,
    pub template_revision: u64,
    pub options: Options,
}
impl Spec {
    pub fn from_config(
        id: u64,
        revision: u64,
        template: u64,
        template_revision: u64,
        config: &exmon_domain::config::Schedule,
    ) -> Result<Self, &'static str> {
        if config.enabled {
            return Err("imported schedule cannot be enabled");
        }
        let spec = Self {
            id,
            revision,
            template,
            template_revision,
            options: config.options(),
        };
        spec.validate()?;
        Ok(spec)
    }
    pub fn validate(self) -> Result<(), &'static str> {
        if self.id == 0 || self.revision == 0 || self.template == 0 || self.template_revision == 0 {
            return Err("schedule identity/revision must be nonzero");
        }
        self.options.validate()
    }
}
#[derive(Debug, Clone)]
pub struct Status {
    pub id: u64,
    pub revision: u64,
    pub activation: u64,
    pub state: State,
    pub next_ns: Option<u64>,
    pub stats: Stats,
}
#[derive(Debug, Clone)]
pub struct Timing {
    pub id: u64,
    pub activation: u64,
    pub deadline_ns: u64,
    pub decision_ns: u64,
    pub submitted_ns: Option<u64>,
    pub completed_ns: Option<u64>,
}
