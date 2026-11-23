use exmon_app::{scheduler::*, transmission::*};
use exmon_capture::{
    adapter::{Availability, Cancellation, Error},
    transmit::{Completion, Identity, Info, Kind, Packet, Port, Progress},
};
use exmon_domain::observation::Metadata;
use std::{
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

const MS: u64 = 1_000_000;
#[derive(Default)]
struct Virtual(AtomicU64);
impl Virtual {
    fn set(&self, ms: u64) {
        self.0.store(ms * MS, Ordering::Release)
    }
}
impl Clock for Virtual {
    fn now_ns(&self) -> Result<u64, TimeError> {
        let n = self.0.load(Ordering::Acquire);
        if n == u64::MAX {
            Err(TimeError::Exhausted)
        } else {
            Ok(n)
        }
    }
}
fn options() -> Options {
    Options {
        interval_ms: 10,
        count: None,
        drift: Drift::FixedRate,
        missed: Missed::Skip,
        max_lateness_ms: None,
    }
}
fn spec(id: u64) -> Spec {
    Spec {
        id,
        revision: 1,
        template: 1,
        template_revision: 1,
        options: options(),
    }
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
fn identity() -> Identity {
    Identity::Serial {
        device: 1,
        revision: 1,
    }
}
fn wait(mut f: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(3);
    while !f() {
        assert!(Instant::now() < end, "scheduler fixture timeout");
        thread::sleep(Duration::from_millis(1));
    }
}
fn status(w: &TxWorker, id: u64) -> Option<Status> {
    snapshot(w).schedules.into_iter().find(|s| s.id == id)
}
fn snapshot(w: &TxWorker) -> Snapshot {
    let mut result = None;
    wait(|| {
        result = w.snapshot();
        result.is_some()
    });
    result.unwrap()
}
fn enabled(w: &TxWorker, id: u64) {
    wait(|| status(w, id).is_some_and(|s| s.state == State::Enabled));
}
fn stopped(w: &TxWorker, id: u64, why: Reason) {
    wait(|| status(w, id).is_some_and(|s| s.state == State::Stopped(why)));
}
fn all(w: &TxWorker) -> Vec<Attempt> {
    let mut out = vec![];
    loop {
        let after = out.last().map_or(0, |a: &Attempt| a.sequence);
        let batch = w.history(after, 4);
        if batch.is_empty() {
            return out;
        }
        out.extend(batch);
    }
}
struct Gate {
    entered: AtomicBool,
    freed: Mutex<bool>,
    cv: Condvar,
}
impl Gate {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            entered: AtomicBool::new(false),
            freed: Mutex::new(false),
            cv: Condvar::new(),
        })
    }
    fn block(&self) {
        self.entered.store(true, Ordering::Release);
        let g = self
            .cv
            .wait_timeout_while(self.freed.lock().unwrap(), Duration::from_secs(2), |v| !*v)
            .unwrap();
        assert!(*g.0, "bounded test gate");
    }
    fn free(&self) {
        *self.freed.lock().unwrap() = true;
        self.cv.notify_all();
    }
}
struct Audit {
    submit_gate: Option<Arc<Gate>>,
    writes: u64,
    result: Result<Progress, Error>,
    completion: Progress,
    info: Info,
    gate: Option<Arc<Gate>>,
    cancel: Option<Cancellation>,
    complete_advance: Option<(Arc<Virtual>, u64)>,
}
impl Default for Audit {
    fn default() -> Self {
        Self {
            submit_gate: None,
            writes: 0,
            result: Ok(Progress::accepted(3)),
            completion: Progress::Pending,
            info: Info {
                identity: identity(),
                writable: Availability::Available,
                max_bytes: 65536,
            },
            gate: None,
            cancel: None,
            complete_advance: None,
        }
    }
}
struct Fake(Arc<Mutex<Audit>>);
impl Port for Fake {
    fn info(&self) -> Result<Info, Error> {
        Ok(self.0.lock().unwrap().info.clone())
    }
    fn metadata(&self, _: &Packet<'_>) -> Result<Metadata, Error> {
        let gate = self.0.lock().unwrap().gate.take();
        if let Some(g) = gate {
            g.block();
        }
        Ok(Metadata::Serial {
            configuration_revision: 1,
        })
    }
    fn submit(&mut self, p: &Packet<'_>, c: &Cancellation) -> Result<Progress, Error> {
        let gate = self.0.lock().unwrap().submit_gate.take();
        if let Some(gate) = gate {
            gate.block();
        }
        assert_eq!(p.bytes(), &[1, 2, 3]);
        let mut a = self.0.lock().unwrap();
        a.writes += 1;
        a.cancel = Some(c.clone());
        if let Some((clock, n)) = a.complete_advance.take() {
            clock.set(n)
        }
        a.result
    }
    fn completion(&mut self) -> Result<Progress, Error> {
        Ok(self.0.lock().unwrap().completion)
    }
    fn close(&mut self) -> Result<(), Error> {
        Ok(())
    }
}
fn bind(w: &TxWorker, a: &Arc<Mutex<Audit>>) -> Ticket {
    let a = a.clone();
    w.bind(key(), Expected::Exact(identity()), true, move || {
        Ok(Box::new(Fake(a)))
    })
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
            .is_some_and(|s| s.ticket.is_some() && s.templates == vec![(1, 1)])
    });
    snapshot(w).ticket.unwrap()
}
fn setup() -> (TxWorker, Arc<Virtual>, Arc<Mutex<Audit>>, Ticket) {
    let clock = Arc::new(Virtual::default());
    let w = TxWorker::spawn_with_clock(clock.clone()).unwrap();
    let a = Arc::new(Mutex::new(Audit::default()));
    let t = bind(&w, &a);
    (w, clock, a, t)
}

#[test]
fn engine_exact_grid_delay_skips_and_count() {
    for drift in [Drift::FixedRate, Drift::FixedDelay] {
        let mut o = options();
        o.drift = drift;
        o.count = Some(2);
        let mut e = Engine::new(o).unwrap();
        assert_eq!(e.state(), State::Disabled);
        assert!(e.offer(100 * MS).is_none());
        e.arm(100 * MS);
        assert!(e.offer(109 * MS).is_none());
        assert_eq!(e.offer(110 * MS).unwrap().deadline_ns, 110 * MS);
        assert!(e.commit(111 * MS));
        e.finish(117 * MS);
        assert_eq!(
            e.next_ns(),
            Some(if drift == Drift::FixedRate {
                120 * MS
            } else {
                127 * MS
            })
        );
        let due = e.next_ns().unwrap();
        assert!(e.offer(due).is_some());
        assert!(e.commit(due));
        e.finish(due);
        assert_eq!(e.state(), State::Stopped(Reason::Finished));
        assert_eq!(e.stats().attempts, 2);
    }
    let mut e = Engine::new(options()).unwrap();
    e.arm(0);
    assert_eq!(e.offer(95 * MS).unwrap().deadline_ns, 90 * MS);
    assert_eq!(e.stats().missed, 8);
    assert!(e.commit(95 * MS));
    e.finish(95 * MS);
    assert!(e.offer(95 * MS).is_none());
    assert_eq!(e.next_ns(), Some(100 * MS));
}
#[test]
fn engine_lateness_stop_backward_overflow_and_invalid_states() {
    let mut o = options();
    o.max_lateness_ms = Some(1);
    let mut e = Engine::new(o).unwrap();
    e.finish(0);
    assert_eq!(e.state(), State::Disabled);
    e.arm(0);
    assert!(e.offer(12 * MS).is_none());
    assert_eq!(e.stats().missed, 1);
    assert!(e.offer(20 * MS).is_some());
    assert!(!e.commit(22 * MS));
    assert_eq!(e.stats().attempts, 0);
    e.stop(Reason::User);
    e.skip(30 * MS);
    assert_eq!(e.state(), State::Stopped(Reason::User));
    o.missed = Missed::Stop;
    let mut e = Engine::new(o).unwrap();
    e.arm(0);
    assert!(e.offer(20 * MS).is_none());
    assert_eq!(e.state(), State::Stopped(Reason::Missed));
    let mut e = Engine::new(options()).unwrap();
    e.arm(20);
    assert!(e.offer(19).is_none());
    assert_eq!(e.state(), State::Stopped(Reason::Clock));
    let mut e = Engine::new(options()).unwrap();
    e.arm(u64::MAX - 1);
    assert_eq!(e.state(), State::Stopped(Reason::Exhausted));
    let mut o = options();
    o.count = Some(0);
    assert!(Engine::new(o).is_err());
    o.count = None;
    o.interval_ms = 0;
    assert!(Engine::new(o).is_err());
}
#[test]
fn worker_starts_only_on_explicit_activation_and_counts_measured_attempts() {
    let (w, c, a, t) = setup();
    c.set(100);
    thread::sleep(Duration::from_millis(20));
    assert_eq!(a.lock().unwrap().writes, 0);
    assert!(snapshot(&w).schedules.is_empty());
    let mut s = spec(1);
    s.options.count = Some(2);
    let h = w.enable_schedule(&t, s).unwrap();
    enabled(&w, 1);
    c.set(110);
    wait(|| a.lock().unwrap().writes == 1);
    enabled(&w, 1);
    c.set(120);
    stopped(&w, 1, Reason::Finished);
    assert!(!h.is_enabled());
    assert_eq!(a.lock().unwrap().writes, 2);
    let s = status(&w, 1).unwrap();
    assert_eq!(s.stats.attempts, 2);
    assert_eq!(s.stats.completed_ns, Some(120 * MS));
    let events = all(&w);
    assert_eq!(events.len(), 2);
    for (i, e) in events.iter().enumerate() {
        let timing = e.scheduled.as_ref().unwrap();
        assert_eq!(timing.deadline_ns, (110 + i as u64 * 10) * MS);
        assert_eq!(timing.submitted_ns, Some(timing.deadline_ns));
        assert_eq!(timing.completed_ns, timing.submitted_ns);
        assert_eq!(e.observation.as_ref().unwrap().bytes(), &[1, 2, 3]);
    }
    c.set(1000);
    thread::sleep(Duration::from_millis(15));
    assert_eq!(a.lock().unwrap().writes, 2);
}
#[test]
fn load_jumps_skip_without_burst_stop_and_fixed_delay_use_completion() {
    let (w, c, a, t) = setup();
    let h = w.enable_schedule(&t, spec(1)).unwrap();
    enabled(&w, 1);
    c.set(95);
    wait(|| a.lock().unwrap().writes == 1);
    enabled(&w, 1);
    wait(|| status(&w, 1).is_some_and(|s| s.stats.missed == 8));
    thread::sleep(Duration::from_millis(15));
    assert_eq!(a.lock().unwrap().writes, 1);
    h.stop();
    stopped(&w, 1, Reason::User);
    let mut s = spec(2);
    s.options.missed = Missed::Stop;
    let _h = w.enable_schedule(&t, s).unwrap();
    enabled(&w, 2);
    c.set(125);
    stopped(&w, 2, Reason::Missed);
    assert_eq!(a.lock().unwrap().writes, 1);
    let mut s = spec(3);
    s.options.drift = Drift::FixedDelay;
    let _h = w.enable_schedule(&t, s).unwrap();
    enabled(&w, 3);
    a.lock().unwrap().complete_advance = Some((c.clone(), 147));
    c.set(135);
    wait(|| status(&w, 3).is_some_and(|s| s.stats.attempts == 1 && s.next_ns == Some(157 * MS)));
    c.set(156);
    thread::sleep(Duration::from_millis(10));
    assert_eq!(a.lock().unwrap().writes, 2);
    c.set(157);
    wait(|| a.lock().unwrap().writes == 3);
}
#[test]
fn stop_before_final_fence_is_immediate_even_with_full_queue_and_clock_load() {
    let (w, c, a, t) = setup();
    let g = Gate::new();
    a.lock().unwrap().gate = Some(g.clone());
    let h = w.enable_schedule(&t, spec(1)).unwrap();
    enabled(&w, 1);
    c.set(10);
    wait(|| g.entered.load(Ordering::Acquire));
    for _ in 0..4 {
        w.send_once(&t, 1, 1).unwrap();
    }
    assert_eq!(w.send_once(&t, 1, 1), Err(Failure::Queue));
    let start = Instant::now();
    h.stop();
    assert!(start.elapsed() < Duration::from_millis(100));
    assert!(!h.is_enabled());
    g.free();
    stopped(&w, 1, Reason::User);
    wait(|| a.lock().unwrap().writes == 4);
    assert!(all(&w).iter().all(|a| {
        a.scheduled
            .as_ref()
            .is_none_or(|t| t.submitted_ns.is_none())
    }));
    // Late preparation is rechecked at the final source fence.
    let g = Gate::new();
    a.lock().unwrap().gate = Some(g.clone());
    let mut s = spec(2);
    s.options.max_lateness_ms = Some(1);
    let _h = w.enable_schedule(&t, s).unwrap();
    enabled(&w, 2);
    c.set(20);
    wait(|| g.entered.load(Ordering::Acquire));
    c.set(25);
    g.free();
    wait(|| status(&w, 2).is_some_and(|s| s.stats.missed == 1));
    assert_eq!(a.lock().unwrap().writes, 4);
}
#[test]
fn pending_cancellation_reaches_source_and_requires_rebind() {
    let (w, c, a, t) = setup();
    a.lock().unwrap().result = Ok(Progress::Pending);
    let h = w.enable_schedule(&t, spec(1)).unwrap();
    enabled(&w, 1);
    c.set(10);
    wait(|| w.snapshot().is_some_and(|s| s.pending));
    h.stop();
    assert!(a.lock().unwrap().cancel.as_ref().unwrap().is_cancelled());
    stopped(&w, 1, Reason::User);
    wait(|| {
        w.snapshot()
            .is_some_and(|s| s.identity.is_none() && !s.pending)
    });
    assert_eq!(a.lock().unwrap().writes, 1);
    assert_eq!(w.enable_schedule(&t, spec(2)).unwrap_err(), Failure::Stale);
    let events = all(&w);
    assert!(events.iter().any(|a| a.scheduled.is_some()
        && a.outcome
            == Outcome::Complete {
                accepted: None,
                status: Completion::Cancelled
            }));
}
#[test]
fn replacement_owner_drop_old_handles_and_template_edit_revoke() {
    let (w, c, a, t) = setup();
    let old = w.enable_schedule(&t, spec(1)).unwrap();
    enabled(&w, 1);
    let new = w.enable_schedule(&t, spec(1)).unwrap();
    wait(|| {
        status(&w, 1).is_some_and(|s| s.activation == new.activation() && s.state == State::Enabled)
    });
    old.stop();
    assert!(new.is_enabled());
    let clone = new.clone();
    drop(new);
    assert!(clone.is_enabled());
    drop(clone);
    stopped(&w, 1, Reason::OwnerDropped);
    c.set(100);
    thread::sleep(Duration::from_millis(10));
    assert_eq!(a.lock().unwrap().writes, 0);
    let _h = w.enable_schedule(&t, spec(2)).unwrap();
    enabled(&w, 2);
    w.register(TemplateSpec {
        id: 1,
        revision: 2,
        protocol_revision: 1,
        format: Format::Bytes(Kind::Serial),
        body: vec![1, 2, 3],
        structured: None,
        source: None,
    })
    .unwrap();
    stopped(&w, 2, Reason::Stale);
    assert_eq!(a.lock().unwrap().writes, 0);
}
#[test]
fn busy_attempts_count_partial_unknown_and_capability_loss_disable() {
    for (result, policy, reason) in [
        (Err(Error::Busy), Missed::Skip, Reason::Finished),
        (Err(Error::Busy), Missed::Stop, Reason::Busy),
        (Ok(Progress::accepted(1)), Missed::Skip, Reason::Partial),
        (
            Ok(Progress::Complete {
                accepted: None,
                status: Completion::Timeout,
            }),
            Missed::Skip,
            Reason::Backend,
        ),
    ] {
        let (w, c, a, t) = setup();
        a.lock().unwrap().result = result;
        let mut s = spec(1);
        s.options.count = Some(1);
        s.options.missed = policy;
        let _h = w.enable_schedule(&t, s).unwrap();
        enabled(&w, 1);
        c.set(10);
        stopped(&w, 1, reason);
        assert_eq!(status(&w, 1).unwrap().stats.attempts, 1);
        assert_eq!(a.lock().unwrap().writes, 1);
    }
    let (w, _c, a, t) = setup();
    let h = w.enable_schedule(&t, spec(1)).unwrap();
    enabled(&w, 1);
    a.lock().unwrap().info.writable = Availability::Unavailable(Error::Disconnected);
    wait(|| !h.is_enabled());
    stopped(&w, 1, Reason::Stale);
    assert_eq!(a.lock().unwrap().writes, 0);
}
#[test]
fn clock_failure_or_backward_disables_without_zero_fallback() {
    for failure in [0, u64::MAX] {
        let (w, c, a, t) = setup();
        c.set(100);
        let h = w.enable_schedule(&t, spec(1)).unwrap();
        enabled(&w, 1);
        c.0.store(failure, Ordering::Release);
        stopped(&w, 1, Reason::Clock);
        assert!(!h.is_enabled());
        assert_eq!(a.lock().unwrap().writes, 0);
    }
}
#[test]
fn bounded_schedules_are_fair_and_stop_all_has_no_queue_dependency() {
    let (w, c, a, t) = setup();
    let mut handles = vec![];
    for id in 1..=64 {
        let mut s = spec(id);
        s.options.count = Some(1);
        handles.push(w.enable_schedule(&t, s).unwrap());
        enabled(&w, id);
    }
    assert_eq!(
        w.enable_schedule(&t, spec(65)).unwrap_err(),
        Failure::Capacity
    );
    c.set(10);
    wait(|| a.lock().unwrap().writes == 64);
    wait(|| {
        w.snapshot().is_some_and(|s| {
            s.schedules
                .iter()
                .all(|s| s.state == State::Stopped(Reason::Finished))
        })
    });
    assert_eq!(snapshot(&w).schedules.len(), 64);
    let h = w.enable_schedule(&t, spec(65)).unwrap();
    enabled(&w, 65);
    w.stop_schedules();
    assert!(!h.is_enabled());
    stopped(&w, 65, Reason::User);
}

#[test]
fn actual_windows_udp_loopback_measures_five_periodic_submissions() {
    use exmon_capture::{
        adapter::Adapter,
        sockets::{Mode, Settings, Sockets},
        transmit::{SocketPort, SocketTarget},
    };
    use std::net::UdpSocket;
    let receiver = UdpSocket::bind("127.0.0.1:0").unwrap();
    receiver
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let remote = receiver.local_addr().unwrap();
    let w = TxWorker::spawn().unwrap();
    w.bind(key(), Expected::Udp { remote }, true, move || {
        let mut port = Sockets::new(Settings::new(Mode::UdpBound {
            local: "127.0.0.1:0".parse().unwrap(),
        }))?;
        port.enumerate(1)?;
        port.open(0)?;
        port.configure(65536)?;
        port.start()?;
        Ok(Box::new(SocketPort::new(port, SocketTarget::Udp(remote))?))
    })
    .unwrap();
    wait(|| snapshot(&w).ticket.is_some());
    w.register(TemplateSpec {
        id: 1,
        revision: 1,
        protocol_revision: 1,
        format: Format::Bytes(Kind::Udp),
        body: vec![1, 2, 3],
        structured: None,
        source: None,
    })
    .unwrap();
    wait(|| snapshot(&w).ticket.is_some() && snapshot(&w).templates.len() == 1);
    let mut s = spec(1);
    s.options.interval_ms = 50;
    s.options.count = Some(5);
    s.options.max_lateness_ms = Some(250);
    let start = Instant::now();
    let _h = w.enable_schedule(&snapshot(&w).ticket.unwrap(), s).unwrap();
    for _ in 0..5 {
        let mut bytes = [0; 8];
        let (n, _) = receiver.recv_from(&mut bytes).unwrap();
        assert_eq!(&bytes[..n], &[1, 2, 3]);
        assert!(start.elapsed() < Duration::from_secs(3));
    }
    stopped(&w, 1, Reason::Finished);
    let stats = status(&w, 1).unwrap().stats;
    assert_eq!(stats.attempts, 5);
    assert_eq!(stats.missed, 0);
    assert!(stats.max_lateness_ns <= 250 * MS);
    let events = all(&w);
    assert_eq!(events.len(), 5);
    for event in events {
        let time = event.scheduled.unwrap();
        assert!(time.completed_ns.unwrap() >= time.submitted_ns.unwrap());
        assert!(time.submitted_ns.unwrap() >= time.deadline_ns);
    }
    println!(
        "Windows loopback: attempts={}, missed={}, max_lateness_ns={}, elapsed_ms={}",
        stats.attempts,
        stats.missed,
        stats.max_lateness_ns,
        start.elapsed().as_millis()
    );
}

#[test]
fn imported_definition_and_recreated_worker_never_rearm_old_authority() {
    let config=exmon_storage::config::load_project(br#"{"schema_version":1,"protocols":[{"id":"p","name":"p","framing":{"kind":"datagram"}}],"interfaces":[{"id":"i","protocol_id":"p","transport":{"kind":"udp"},"tx_allowed":true}],"templates":[{"id":"t","interface_id":"i","bytes":[1,2,3]}],"schedules":[{"id":"s","template_id":"t","interval_ms":10}]}"#).unwrap();
    let imported = Spec::from_config(1, 1, 1, 1, &config.schedules[0]).unwrap();
    let mut invalid = config.schedules[0].clone();
    invalid.enabled = true;
    assert!(Spec::from_config(1, 1, 1, 1, &invalid).is_err());
    let (w, c, a, t) = setup();
    c.set(100);
    thread::sleep(Duration::from_millis(10));
    assert_eq!(a.lock().unwrap().writes, 0);
    let h = w.enable_schedule(&t, imported).unwrap();
    enabled(&w, 1);
    drop(w);
    assert!(!h.is_enabled());
    let (w, c, a, new) = setup();
    assert_eq!(w.enable_schedule(&t, imported).unwrap_err(), Failure::Stale);
    c.set(100);
    thread::sleep(Duration::from_millis(10));
    assert_eq!(a.lock().unwrap().writes, 0);
    let h = w.enable_schedule(&new, imported).unwrap();
    enabled(&w, 1);
    assert_eq!(w.enable_schedule(&t, imported).unwrap_err(), Failure::Stale);
    assert!(h.is_enabled());
    w.bind(key(), Expected::Exact(identity()), false, || {
        panic!("permission denied factory")
    })
    .unwrap_err();
    stopped(&w, 1, Reason::Stale);
    assert_eq!(a.lock().unwrap().writes, 0);
}

#[test]
fn synchronous_lease_reports_known_acceptance_after_stop_and_watchdog_stops_pending() {
    let (w, c, a, t) = setup();
    let g = Gate::new();
    a.lock().unwrap().submit_gate = Some(g.clone());
    let h = w.enable_schedule(&t, spec(1)).unwrap();
    enabled(&w, 1);
    c.set(10);
    wait(|| g.entered.load(Ordering::Acquire));
    h.stop();
    assert!(!h.is_enabled());
    g.free();
    stopped(&w, 1, Reason::User);
    wait(|| {
        all(&w).iter().any(|a| {
            a.outcome
                == Outcome::Complete {
                    accepted: Some(3),
                    status: Completion::Accepted,
                }
        })
    });
    assert_eq!(a.lock().unwrap().writes, 1);
    assert_eq!(status(&w, 1).unwrap().stats.attempts, 1);
    let (w, c, a, t) = setup();
    a.lock().unwrap().result = Ok(Progress::Pending);
    let _h = w.enable_schedule(&t, spec(1)).unwrap();
    enabled(&w, 1);
    c.set(10);
    wait(|| snapshot(&w).pending);
    stopped(&w, 1, Reason::Backend);
    wait(|| !snapshot(&w).pending);
    assert_eq!(a.lock().unwrap().writes, 1);
    assert!(all(&w).iter().any(|a| a.outcome
        == Outcome::Complete {
            accepted: None,
            status: Completion::Timeout
        }));
}

#[test]
fn busy_skip_waits_for_next_period_and_async_completion_obeys_count() {
    let (w, c, a, t) = setup();
    a.lock().unwrap().result = Err(Error::Busy);
    let h = w.enable_schedule(&t, spec(1)).unwrap();
    enabled(&w, 1);
    c.set(10);
    wait(|| status(&w, 1).is_some_and(|s| s.state == State::Enabled && s.stats.attempts == 1));
    assert!(h.is_enabled());
    thread::sleep(Duration::from_millis(10));
    assert_eq!(a.lock().unwrap().writes, 1);
    a.lock().unwrap().result = Ok(Progress::Pending);
    c.set(20);
    wait(|| snapshot(&w).pending);
    c.set(200);
    thread::sleep(Duration::from_millis(10));
    assert_eq!(a.lock().unwrap().writes, 2);
    a.lock().unwrap().completion = Progress::accepted(3);
    wait(|| {
        status(&w, 1)
            .is_some_and(|s| s.state == State::Enabled && s.stats.completed_ns == Some(200 * MS))
    });
    // The worker can select one current opportunity after completion, never a queue of 18 writes.
    wait(|| a.lock().unwrap().writes == 3);
    h.stop();
    wait(|| !snapshot(&w).pending);
    assert_eq!(a.lock().unwrap().writes, 3);
}

#[test]
fn capability_loss_while_preparing_and_invalid_replacement_prevent_source_calls() {
    let (w, c, a, t) = setup();
    let g = Gate::new();
    a.lock().unwrap().gate = Some(g.clone());
    let h = w.enable_schedule(&t, spec(1)).unwrap();
    enabled(&w, 1);
    c.set(10);
    wait(|| g.entered.load(Ordering::Acquire));
    a.lock().unwrap().info.max_bytes = 2;
    g.free();
    wait(|| !h.is_enabled());
    assert_eq!(a.lock().unwrap().writes, 0);
    let (w, _c, a, t) = setup();
    let h = w.enable_schedule(&t, spec(1)).unwrap();
    enabled(&w, 1);
    let mut invalid = spec(1);
    invalid.options.interval_ms = 0;
    assert_eq!(
        w.enable_schedule(&t, invalid).unwrap_err(),
        Failure::Validation
    );
    assert!(!h.is_enabled());
    stopped(&w, 1, Reason::Replaced);
    assert_eq!(a.lock().unwrap().writes, 0);
}
