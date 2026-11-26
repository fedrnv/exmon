use exmon_app::transmission::*;
use exmon_capture::{
    adapter::{Availability, Cancellation, Error},
    sockets::{Mode, Settings, Sockets},
    transmit::{
        Completion, Identity, Info, Kind, Packet, Port, Progress, SocketPort, SocketTarget,
    },
};
use exmon_domain::{Direction, config::Protocol, observation::Metadata};
use exmon_protocol::checksum::Registry;
use std::{
    net::UdpSocket,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

fn key() -> Binding {
    Binding {
        session: 1,
        interface: 2,
        epoch: 3,
        configuration: 4,
        protocol: 5,
    }
}
fn identity() -> Identity {
    Identity::Serial {
        device: 1,
        revision: 1,
    }
}
fn spec(id: u64, revision: u64, body: &[u8]) -> TemplateSpec {
    TemplateSpec {
        id,
        revision,
        protocol_revision: 5,
        format: Format::Bytes(Kind::Serial),
        body: body.to_vec(),
        structured: None,
        source: None,
    }
}
fn until(mut f: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !f() {
        assert!(Instant::now() < deadline, "TX fixture deadline");
        thread::sleep(Duration::from_millis(1));
    }
}
fn snapshot(w: &TxWorker) -> Snapshot {
    w.snapshot().unwrap_or_default()
}
fn ticket(w: &TxWorker) -> Ticket {
    until(|| snapshot(w).ticket.is_some());
    snapshot(w).ticket.unwrap()
}
fn register(w: &TxWorker, s: TemplateSpec) -> Ticket {
    let id = s.id;
    let revision = s.revision;
    w.register(s).unwrap();
    until(|| {
        let s = snapshot(w);
        s.ticket.is_some() && s.templates.contains(&(id, revision))
    });
    ticket(w)
}
fn history(w: &TxWorker) -> Vec<Attempt> {
    let mut all = vec![];
    let mut after = 0;
    loop {
        let batch = w.history(after, 4);
        if batch.is_empty() {
            return all;
        }
        after = batch.last().unwrap().sequence;
        all.extend(batch);
    }
}
fn outcome(w: &TxWorker, id: u64) -> Attempt {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(attempt) = history(w)
            .into_iter()
            .find(|a| a.id == id && a.outcome != Outcome::Pending)
        {
            return attempt;
        }
        assert!(Instant::now() < deadline, "TX outcome fixture deadline");
        thread::sleep(Duration::from_millis(1));
    }
}
struct Gate {
    armed: AtomicBool,
    entered: AtomicBool,
    release: Mutex<bool>,
    cv: Condvar,
}
impl Gate {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            armed: AtomicBool::new(true),
            entered: AtomicBool::new(false),
            release: Mutex::new(false),
            cv: Condvar::new(),
        })
    }
    fn wait(&self) -> Result<(), Error> {
        if !self.armed.swap(false, Ordering::AcqRel) {
            return Ok(());
        }
        self.entered.store(true, Ordering::Release);
        let done = self
            .cv
            .wait_timeout_while(self.release.lock().unwrap(), Duration::from_secs(2), |v| {
                !*v
            })
            .unwrap();
        if !*done.0 {
            return Err(Error::Backend);
        }
        Ok(())
    }
    fn free(&self) {
        *self.release.lock().unwrap() = true;
        self.cv.notify_all();
    }
}
struct Audit {
    info: Info,
    writes: Vec<Vec<u8>>,
    closes: usize,
    result: Result<Progress, Error>,
    completion: Progress,
    metadata_gate: Option<Arc<Gate>>,
    submit_gate: Option<Arc<Gate>>,
    owner: Option<thread::ThreadId>,
}
impl Default for Audit {
    fn default() -> Self {
        Self {
            info: Info {
                identity: identity(),
                writable: Availability::Available,
                max_bytes: 65536,
            },
            writes: vec![],
            closes: 0,
            result: Ok(Progress::accepted(3)),
            completion: Progress::Pending,
            metadata_gate: None,
            submit_gate: None,
            owner: None,
        }
    }
}
struct Fixture(Arc<Mutex<Audit>>);
impl Port for Fixture {
    fn info(&self) -> Result<Info, Error> {
        Ok(self.0.lock().unwrap().info.clone())
    }
    fn metadata(&self, p: &Packet<'_>) -> Result<Metadata, Error> {
        let gate = self.0.lock().unwrap().metadata_gate.clone();
        if let Some(g) = gate {
            g.wait()?;
        }
        if !matches!(p, Packet::Bytes(_)) {
            return Err(Error::InvalidConfig);
        }
        let a = self.0.lock().unwrap();
        Ok(match a.info.identity {
            Identity::Serial { revision, .. } => Metadata::Serial {
                configuration_revision: revision,
            },
            Identity::Usb { endpoint, .. } => Metadata::Usb {
                endpoint,
                completed: false,
            },
            _ => return Err(Error::Unsupported),
        })
    }
    fn submit(&mut self, p: &Packet<'_>, _: &Cancellation) -> Result<Progress, Error> {
        let gate = self.0.lock().unwrap().submit_gate.clone();
        if let Some(g) = gate {
            g.wait()?;
        }
        let mut a = self.0.lock().unwrap();
        assert_eq!(a.owner, Some(thread::current().id()));
        assert!(a.writes.len() < 256);
        a.writes.push(p.bytes().to_vec());
        a.result
    }
    fn completion(&mut self) -> Result<Progress, Error> {
        Ok(self.0.lock().unwrap().completion)
    }
    fn close(&mut self) -> Result<(), Error> {
        self.0.lock().unwrap().closes += 1;
        Ok(())
    }
}
fn bound(a: Arc<Mutex<Audit>>) -> TxWorker {
    let w = TxWorker::spawn().unwrap();
    let expected = a.lock().unwrap().info.identity.clone();
    w.bind(key(), Expected::Exact(expected), true, move || {
        a.lock().unwrap().owner = Some(thread::current().id());
        Ok(Box::new(Fixture(a)))
    })
    .unwrap();
    ticket(&w);
    w
}
fn protocol() -> Protocol {
    let mut p=exmon_storage::config::load_project(br#"{"schema_version":1,"protocols":[{"id":"synthetic","name":"Synthetic","framing":{"kind":"fixed","length":3},"fields":[{"id":"value","offset":0,"length":2,"representation":"unsigned","endian":"big"}],"checksums":[{"id":"sum","algorithm":"sum8","offset":2,"length":1,"endian":"big","ranges":[{"offset":0,"length":2}]}]}]}"#).unwrap();
    p.protocols.remove(0)
}

#[test]
fn explicit_clone_registration_and_one_shot_preserve_session_provenance_and_do_not_auto_send() {
    let a = Arc::new(Mutex::new(Audit::default()));
    let w = bound(a.clone());
    let source = SourceRef {
        session: 10,
        interface: 11,
        epoch: 12,
        observation: 13,
    };
    let s = TemplateSpec::incoming(
        1,
        1,
        5,
        Format::Bytes(Kind::Serial),
        &[1, 2, 3],
        Structured {
            protocol: Arc::new(protocol()),
            registry: Registry::default(),
            header: vec![],
        },
        source,
    )
    .unwrap();
    let t = register(&w, s);
    assert!(a.lock().unwrap().writes.is_empty());
    let id = w.send_once(&t, 1, 1).unwrap();
    let attempt = outcome(&w, id);
    assert_eq!(attempt.binding, key());
    assert_eq!(attempt.source, Some(source));
    assert_eq!(attempt.requested, Some(3));
    assert_eq!(
        attempt.outcome,
        Outcome::Complete {
            accepted: Some(3),
            status: Completion::Accepted
        }
    );
    let observation = attempt.observation.as_ref().unwrap();
    assert_eq!(observation.direction(), Direction::Tx);
    assert_eq!(observation.bytes(), [1, 2, 3]);
    assert!(attempt.timestamp_ns.is_some());
    assert_ne!(a.lock().unwrap().owner, Some(thread::current().id()));
    assert!(!format!("{attempt:?} {t:?}").contains("[1, 2, 3]"));
    assert_eq!(a.lock().unwrap().writes, [vec![1, 2, 3]]);
}
#[test]
fn revocation_before_submission_lease_defeats_queued_requests_and_full_queue() {
    let gate = Gate::new();
    let a = Arc::new(Mutex::new(Audit {
        metadata_gate: Some(gate.clone()),
        ..Default::default()
    }));
    let w = bound(a.clone());
    let t = register(&w, spec(1, 1, &[1, 2, 3]));
    let first = w.send_once(&t, 1, 1).unwrap();
    until(|| gate.entered.load(Ordering::Acquire));
    let mut ids = vec![first];
    for _ in 0..4 {
        ids.push(w.send_once(&t, 1, 1).unwrap());
    }
    assert_eq!(w.send_once(&t, 1, 1), Err(Failure::Queue));
    let start = Instant::now();
    w.close();
    assert!(start.elapsed() < Duration::from_millis(100));
    gate.free();
    for id in ids {
        assert_eq!(outcome(&w, id).outcome, Outcome::Rejected(Failure::Stale));
    }
    assert!(a.lock().unwrap().writes.is_empty());
    assert!(snapshot(&w).queue_rejected >= 1);
    assert_eq!(w.send_once(&t, 1, 1), Err(Failure::Stale));
    until(|| a.lock().unwrap().closes > 0);
}
#[test]
fn invalidation_revokes_queued_sends_and_revalidates_without_reopening_idle_port() {
    let gate = Gate::new();
    let a = Arc::new(Mutex::new(Audit {
        metadata_gate: Some(gate.clone()),
        ..Default::default()
    }));
    let w = bound(a.clone());
    let t = register(&w, spec(1, 1, &[1, 2, 3]));
    let first = w.send_once(&t, 1, 1).unwrap();
    until(|| gate.entered.load(Ordering::Acquire));
    let mut ids = vec![first];
    for _ in 0..4 {
        ids.push(w.send_once(&t, 1, 1).unwrap());
    }
    let start = Instant::now();
    w.invalidate().unwrap();
    assert!(start.elapsed() < Duration::from_millis(100));
    gate.free();
    for id in ids {
        assert_eq!(outcome(&w, id).outcome, Outcome::Rejected(Failure::Stale));
    }
    assert!(a.lock().unwrap().writes.is_empty());
    assert!(snapshot(&w).identity.is_some());
    assert_eq!(a.lock().unwrap().closes, 0);
    let t = register(&w, spec(1, 2, &[4, 5, 6]));
    let sent = w.send_once(&t, 1, 2).unwrap();
    assert!(matches!(
        outcome(&w, sent).outcome,
        Outcome::Complete {
            accepted: Some(3),
            status: Completion::Accepted
        }
    ));
    assert_eq!(a.lock().unwrap().writes, [vec![4, 5, 6]]);
}
#[test]
fn already_leased_submission_reports_actual_acceptance_after_close_without_recall_or_retry() {
    let gate = Gate::new();
    let a = Arc::new(Mutex::new(Audit {
        submit_gate: Some(gate.clone()),
        ..Default::default()
    }));
    let w = bound(a.clone());
    let t = register(&w, spec(1, 1, &[1, 2, 3]));
    let first = w.send_once(&t, 1, 1).unwrap();
    until(|| gate.entered.load(Ordering::Acquire));
    let queued = w.send_once(&t, 1, 1).unwrap();
    w.close();
    gate.free();
    assert_eq!(
        outcome(&w, first).outcome,
        Outcome::Complete {
            accepted: Some(3),
            status: Completion::Accepted
        }
    );
    assert_eq!(
        outcome(&w, queued).outcome,
        Outcome::Rejected(Failure::Stale)
    );
    assert_eq!(a.lock().unwrap().writes.len(), 1);
}
#[test]
fn partial_zero_backend_error_and_impossible_results_are_distinct_and_never_retried() {
    for result in [
        Ok(Progress::accepted(1)),
        Ok(Progress::accepted(0)),
        Err(Error::Disconnected),
        Ok(Progress::accepted(4)),
        Ok(Progress::Complete {
            accepted: None,
            status: Completion::Accepted,
        }),
    ] {
        let a = Arc::new(Mutex::new(Audit {
            result,
            ..Default::default()
        }));
        let w = bound(a.clone());
        let t = register(&w, spec(1, 1, &[1, 2, 3]));
        let attempt = outcome(&w, w.send_once(&t, 1, 1).unwrap());
        match result {
            Ok(Progress::Complete {
                accepted: Some(n), ..
            }) if n <= 3 => {
                assert_eq!(
                    attempt.outcome,
                    Outcome::Complete {
                        accepted: Some(n),
                        status: Completion::Accepted
                    }
                );
                assert_eq!(attempt.observation.unwrap().bytes(), &[1, 2, 3][..n]);
            }
            Err(error) => assert_eq!(
                attempt.outcome,
                Outcome::Failed {
                    error,
                    accepted: None
                }
            ),
            _ => {
                assert!(matches!(
                    attempt.outcome,
                    Outcome::Failed {
                        error: Error::Backend,
                        ..
                    }
                ));
                until(|| snapshot(&w).ticket.is_none());
            }
        }
        assert_eq!(a.lock().unwrap().writes.len(), 1);
    }
}
#[test]
fn asynchronous_completion_pending_failure_unknown_cancellation_and_known_partial_are_honest() {
    for completion in [
        Progress::accepted(3),
        Progress::Complete {
            accepted: Some(1),
            status: Completion::Stall,
        },
        Progress::Complete {
            accepted: None,
            status: Completion::Timeout,
        },
    ] {
        let a = Arc::new(Mutex::new(Audit {
            result: Ok(Progress::Pending),
            ..Default::default()
        }));
        a.lock().unwrap().info.identity = Identity::Usb {
            device: 1,
            configuration: 1,
            interface: 2,
            alternate: 0,
            endpoint: 1,
            interrupt: false,
            packet_size: 64,
        };
        let w = bound(a.clone());
        let mut s = spec(1, 1, &[1, 2, 3]);
        s.format = Format::Bytes(Kind::Usb);
        let t = register(&w, s);
        let id = w.send_once(&t, 1, 1).unwrap();
        until(|| snapshot(&w).pending);
        assert_eq!(history(&w)[0].outcome, Outcome::Pending);
        a.lock().unwrap().completion = completion;
        let attempt = outcome(&w, id);
        let Progress::Complete { accepted, status } = completion else {
            unreachable!()
        };
        assert_eq!(attempt.outcome, Outcome::Complete { accepted, status });
        assert_eq!(
            attempt.observation.as_ref().map(|o| o.bytes().len()),
            accepted
        );
        if let Some(o) = &attempt.observation {
            assert_eq!(
                o.metadata(),
                &Metadata::Usb {
                    endpoint: 1,
                    completed: status == Completion::Accepted
                }
            );
        }
        assert_eq!(a.lock().unwrap().writes.len(), 1);
    }
    let a = Arc::new(Mutex::new(Audit {
        result: Ok(Progress::Pending),
        ..Default::default()
    }));
    let w = bound(a.clone());
    let t = register(&w, spec(1, 1, &[1, 2, 3]));
    let id = w.send_once(&t, 1, 1).unwrap();
    until(|| snapshot(&w).pending);
    w.close();
    assert_eq!(
        outcome(&w, id).outcome,
        Outcome::Complete {
            accepted: None,
            status: Completion::Cancelled
        }
    );
}
#[test]
fn wrong_binding_capability_template_and_revisions_fail_before_backend_submission() {
    let opened = Arc::new(AtomicBool::new(false));
    let flag = opened.clone();
    let w = TxWorker::spawn().unwrap();
    assert_eq!(
        w.bind(key(), Expected::Exact(identity()), false, move || {
            flag.store(true, Ordering::Release);
            Err(Error::Backend)
        }),
        Err(Failure::Permission)
    );
    assert!(!opened.load(Ordering::Acquire));
    let a = Arc::new(Mutex::new(Audit::default()));
    let w = bound(a.clone());
    let t = register(&w, spec(1, 1, &[1, 2, 3]));
    assert_eq!(
        outcome(&w, w.send_once(&t, 1, 2).unwrap()).outcome,
        Outcome::Rejected(Failure::Template)
    );
    let mut new = spec(1, 2, &[1, 2, 3]);
    new.protocol_revision = 6;
    let next = register(&w, new);
    assert_eq!(w.send_once(&t, 1, 1), Err(Failure::Stale));
    assert_eq!(
        outcome(&w, w.send_once(&next, 1, 2).unwrap()).outcome,
        Outcome::Rejected(Failure::Binding)
    );
    assert!(a.lock().unwrap().writes.is_empty());
    let a = Arc::new(Mutex::new(Audit::default()));
    let w = bound(a.clone());
    let mut s = spec(1, 1, &[1, 2, 3]);
    s.format = Format::Bytes(Kind::Udp);
    let t = register(&w, s);
    assert_eq!(
        outcome(&w, w.send_once(&t, 1, 1).unwrap()).outcome,
        Outcome::Rejected(Failure::Binding)
    );
    assert!(a.lock().unwrap().writes.is_empty());
    let a = Arc::new(Mutex::new(Audit::default()));
    let w = bound(a.clone());
    let t = register(&w, spec(1, 1, &[1, 2, 3]));
    a.lock().unwrap().info.identity = Identity::Serial {
        device: 99,
        revision: 1,
    };
    assert_eq!(
        outcome(&w, w.send_once(&t, 1, 1).unwrap()).outcome,
        Outcome::Rejected(Failure::Binding)
    );
    assert!(a.lock().unwrap().writes.is_empty());
}
#[test]
fn structured_validation_returns_field_and_checksum_errors_then_accepts_explicit_repair() {
    let a = Arc::new(Mutex::new(Audit::default()));
    let w = bound(a.clone());
    let mut p = protocol();
    p.fields[0].constant = Some(exmon_domain::fields::Literal::Unsigned { value: 258 });
    let mut s = spec(1, 1, &[0, 0, 7]);
    s.structured = Some(Structured {
        protocol: Arc::new(p),
        registry: Registry::default(),
        header: vec![],
    });
    w.register(s).unwrap();
    until(|| snapshot(&w).last_error == Some(Failure::Validation));
    let state = snapshot(&w);
    assert_eq!(state.validation.fields[0].0, 0);
    assert_eq!(state.validation.checksums[0].0, 0);
    assert!(a.lock().unwrap().writes.is_empty());
    let t = register(&w, spec(1, 2, &[1, 2, 3]));
    assert!(matches!(
        outcome(&w, w.send_once(&t, 1, 2).unwrap()).outcome,
        Outcome::Complete { .. }
    ));
    assert_eq!(
        w.register(spec(1, 3, &vec![0; 65537])),
        Err(Failure::Payload)
    );
    assert_eq!(w.send_once(&t, 1, 2), Err(Failure::Stale));
}
#[test]
fn actual_udp_loopback_from_worker_records_exact_target_and_tx_bytes() {
    use exmon_capture::adapter::Adapter;
    let receiver = UdpSocket::bind("127.0.0.1:0").unwrap();
    receiver
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let remote = receiver.local_addr().unwrap();
    let w = TxWorker::spawn().unwrap();
    w.bind(key(), Expected::Udp { remote }, true, move || {
        let mut a = Sockets::new(Settings::new(Mode::UdpBound {
            local: "127.0.0.1:0".parse().unwrap(),
        }))?;
        a.enumerate(1)?;
        a.open(0)?;
        a.configure(65536)?;
        a.start()?;
        Ok(Box::new(SocketPort::new(a, SocketTarget::Udp(remote))?))
    })
    .unwrap();
    ticket(&w);
    let mut s = spec(1, 1, b"abc");
    s.format = Format::Bytes(Kind::Udp);
    let t = register(&w, s);
    let id = w.send_once(&t, 1, 1).unwrap();
    let mut bytes = [0; 8];
    let (n, peer) = receiver.recv_from(&mut bytes).unwrap();
    assert_eq!(&bytes[..n], b"abc");
    let attempt = outcome(&w, id);
    assert!(
        matches!(attempt.observation.unwrap().metadata(),Metadata::Udp{local,remote:r} if *local==peer&&*r==remote)
    );
}
#[test]
fn history_byte_and_count_evictions_are_explicit_and_keep_each_retained_session_outcome() {
    let a = Arc::new(Mutex::new(Audit {
        result: Ok(Progress::accepted(65536)),
        ..Default::default()
    }));
    let w = bound(a.clone());
    let t = register(&w, spec(1, 1, &vec![0x5a; 65536]));
    for _ in 0..20 {
        let id = w.send_once(&t, 1, 1).unwrap();
        outcome(&w, id);
    }
    let h = history(&w);
    assert!(h.len() < 20);
    assert!(
        h.iter()
            .map(|a| a.observation.as_ref().unwrap().bytes().len() + 256)
            .sum::<usize>()
            <= 1_048_576
    );
    assert!(snapshot(&w).history_evicted > 0);
    assert_eq!(snapshot(&w).history_lost, 0);
    for _ in 0..140 {
        assert_eq!(
            w.send_once(&t, 999, 1).map(|id| outcome(&w, id).outcome),
            Ok(Outcome::Rejected(Failure::Template))
        );
    }
    assert!(history(&w).len() <= 128);
    assert!(snapshot(&w).history_evicted >= 32);
}

#[test]
fn ticket_namespace_survives_worker_recreation_and_native_permission_loss_revokes_binding() {
    let a = Arc::new(Mutex::new(Audit::default()));
    let first = bound(a);
    let stale = register(&first, spec(1, 1, &[1, 2, 3]));
    drop(first);
    let a = Arc::new(Mutex::new(Audit::default()));
    let next = bound(a.clone());
    let fresh = register(&next, spec(1, 1, &[1, 2, 3]));
    assert_eq!(next.send_once(&stale, 1, 1), Err(Failure::Stale));
    assert!(a.lock().unwrap().writes.is_empty());
    a.lock().unwrap().info.writable = Availability::Unavailable(Error::PermissionDenied);
    assert_eq!(
        outcome(&next, next.send_once(&fresh, 1, 1).unwrap()).outcome,
        Outcome::Rejected(Failure::Backend(Error::PermissionDenied))
    );
    until(|| snapshot(&next).ticket.is_none());
    assert!(a.lock().unwrap().writes.is_empty());
}
#[test]
fn changed_template_cancels_queued_old_revision_before_any_write() {
    let gate = Gate::new();
    let a = Arc::new(Mutex::new(Audit {
        metadata_gate: Some(gate.clone()),
        ..Default::default()
    }));
    let w = bound(a.clone());
    let old = register(&w, spec(1, 1, &[1, 2, 3]));
    let id = w.send_once(&old, 1, 1).unwrap();
    until(|| gate.entered.load(Ordering::Acquire));
    w.register(spec(1, 2, &[4, 5, 6])).unwrap();
    gate.free();
    assert_eq!(outcome(&w, id).outcome, Outcome::Rejected(Failure::Stale));
    until(|| snapshot(&w).templates.contains(&(1, 2)) && snapshot(&w).ticket.is_some());
    let next = ticket(&w);
    let id = w.send_once(&next, 1, 2).unwrap();
    assert!(matches!(
        outcome(&w, id).outcome,
        Outcome::Complete {
            accepted: Some(3),
            ..
        }
    ));
    assert_eq!(a.lock().unwrap().writes, [vec![4, 5, 6]]);
}
#[test]
fn factory_lease_rechecks_revocation_and_never_rearms_a_canceled_bind() {
    let gate = Gate::new();
    let a = Arc::new(Mutex::new(Audit::default()));
    let w = TxWorker::spawn().unwrap();
    let g = gate.clone();
    let source = a.clone();
    w.bind(key(), Expected::Exact(identity()), true, move || {
        g.wait()?;
        Ok(Box::new(Fixture(source)))
    })
    .unwrap();
    until(|| gate.entered.load(Ordering::Acquire));
    w.close();
    gate.free();
    until(|| a.lock().unwrap().closes > 0);
    assert!(snapshot(&w).ticket.is_none());
    assert!(a.lock().unwrap().writes.is_empty());
}
#[test]
fn registry_count_definition_byte_caps_and_invalid_template_ids_do_not_pin_unbounded_data() {
    let a = Arc::new(Mutex::new(Audit::default()));
    let w = bound(a.clone());
    for id in 1..=64 {
        register(&w, spec(id, 1, &[]));
    }
    w.register(spec(65, 1, &[])).unwrap();
    until(|| snapshot(&w).last_error == Some(Failure::Capacity));
    assert_eq!(snapshot(&w).templates.len(), 64);
    let mut bad = spec(1, 2, &[0; 3]);
    let mut p = protocol();
    p.fields[0].constant = Some(exmon_domain::fields::Literal::Bytes {
        value: vec![0; 1_048_577],
    });
    bad.structured = Some(Structured {
        protocol: Arc::new(p),
        registry: Registry::default(),
        header: vec![],
    });
    assert_eq!(w.register(bad), Err(Failure::Capacity));
    assert!(a.lock().unwrap().writes.is_empty());
    let a = Arc::new(Mutex::new(Audit::default()));
    let w = bound(a);
    w.register(spec(0, 1, &[1])).unwrap();
    until(|| snapshot(&w).last_error == Some(Failure::Template));
}

#[test]
fn worker_watchdog_bounds_missing_async_completion_without_fabricating_zero_acceptance() {
    let a = Arc::new(Mutex::new(Audit {
        result: Ok(Progress::Pending),
        ..Default::default()
    }));
    let w = bound(a.clone());
    let t = register(&w, spec(1, 1, &[1, 2, 3]));
    let id = w.send_once(&t, 1, 1).unwrap();
    assert_eq!(
        outcome(&w, id).outcome,
        Outcome::Complete {
            accepted: None,
            status: Completion::Timeout
        }
    );
    assert_eq!(a.lock().unwrap().writes.len(), 1);
    until(|| snapshot(&w).ticket.is_none());
}
#[test]
fn edited_structured_fields_and_checksums_are_framed_before_the_only_submission() {
    use exmon_protocol::codec::{Codec, Edit};
    let mut p = protocol();
    p.framing = exmon_domain::config::Framing::Delimiter {
        bytes: vec![10],
        max_length: 64,
    };
    let original = [1, 2, 3];
    let codec = Codec::new(&p, &Registry::default()).unwrap();
    let mut draft = codec.decode(&original).unwrap();
    draft
        .edit(&[Edit::Field {
            index: 0,
            value: &exmon_domain::fields::Literal::Unsigned { value: 0x0304 },
        }])
        .unwrap();
    assert_eq!(draft.bytes(), [3, 4, 7]);
    assert_eq!(draft.original(), original);
    let a = Arc::new(Mutex::new(Audit {
        result: Ok(Progress::accepted(4)),
        ..Default::default()
    }));
    let w = bound(a.clone());
    let mut s = spec(1, 1, draft.prepare().unwrap().bytes());
    s.structured = Some(Structured {
        protocol: Arc::new(p),
        registry: Registry::default(),
        header: vec![],
    });
    let t = register(&w, s);
    let attempt = outcome(&w, w.send_once(&t, 1, 1).unwrap());
    assert_eq!(attempt.observation.unwrap().bytes(), [3, 4, 7, 10]);
    assert_eq!(a.lock().unwrap().writes, [vec![3, 4, 7, 10]]);
}
#[test]
fn worker_failure_revokes_tickets_and_exposes_unknown_outcome_loss() {
    struct PanicPort;
    impl Port for PanicPort {
        fn info(&self) -> Result<Info, Error> {
            Ok(Info {
                identity: identity(),
                writable: Availability::Available,
                max_bytes: 65536,
            })
        }
        fn metadata(&self, _: &Packet<'_>) -> Result<Metadata, Error> {
            panic!("synthetic trusted worker failure")
        }
        fn submit(&mut self, _: &Packet<'_>, _: &Cancellation) -> Result<Progress, Error> {
            panic!("must never submit")
        }
        fn close(&mut self) -> Result<(), Error> {
            Ok(())
        }
    }
    let w = TxWorker::spawn().unwrap();
    w.bind(key(), Expected::Exact(identity()), true, || {
        Ok(Box::new(PanicPort))
    })
    .unwrap();
    ticket(&w);
    let t = register(&w, spec(1, 1, &[1, 2, 3]));
    w.send_once(&t, 1, 1).unwrap();
    until(|| snapshot(&w).last_error == Some(Failure::Disconnected));
    assert!(snapshot(&w).ticket.is_none());
    assert_eq!(snapshot(&w).history_lost, 1);
    assert!(history(&w).is_empty());
    assert_eq!(w.send_once(&t, 1, 1), Err(Failure::Stale));
}

#[test]
fn idle_gap_rejects_early_next_message_without_scheduling_and_resets_on_explicit_new_binding() {
    let a = Arc::new(Mutex::new(Audit::default()));
    let w = bound(a.clone());
    let mut p = protocol();
    p.framing = exmon_domain::config::Framing::IdleGap {
        gap_ns: 60_000_000_000,
        max_length: 64,
    };
    let mut s = spec(1, 1, &[1, 2, 3]);
    s.structured = Some(Structured {
        protocol: Arc::new(p),
        registry: Registry::default(),
        header: vec![],
    });
    register(&w, s);
    let t = register(&w, spec(2, 1, &[1, 2, 3]));
    outcome(&w, w.send_once(&t, 1, 1).unwrap());
    assert_eq!(
        outcome(&w, w.send_once(&t, 2, 1).unwrap()).outcome,
        Outcome::Rejected(Failure::Backend(Error::Busy))
    );
    assert_eq!(a.lock().unwrap().writes.len(), 1);
    let source = a.clone();
    let mut next = key();
    next.epoch += 1;
    w.bind(next, Expected::Exact(identity()), true, move || {
        Ok(Box::new(Fixture(source)))
    })
    .unwrap();
    let t = ticket(&w);
    assert_eq!(t.binding(), next);
    assert!(matches!(
        outcome(&w, w.send_once(&t, 2, 1).unwrap()).outcome,
        Outcome::Complete {
            accepted: Some(3),
            ..
        }
    ));
    assert_eq!(a.lock().unwrap().writes.len(), 2);
}

#[test]
fn actual_hid_bridge_applies_explicit_unnumbered_convention_once_and_rejects_wrong_mode() {
    use exmon_capture::{adapter::Adapter, hid};
    struct Source(Arc<Mutex<Vec<Vec<u8>>>>);
    impl hid::Source for Source {
        fn enumerate(&mut self, _: usize) -> Result<Vec<hid::Device>, Error> {
            Ok(vec![hid::Device {
                vendor_id: 0x1209,
                product_id: 1,
                usage_page: 0xff00,
                usage: 1,
            }])
        }
        fn open_selected(&mut self, index: usize) -> Result<(), Error> {
            assert_eq!(index, 0);
            Ok(())
        }
        fn read(&mut self, _: &mut [u8], _: i32) -> Result<usize, Error> {
            Ok(0)
        }
        fn write(&mut self, b: &[u8]) -> Result<usize, Error> {
            self.0.lock().unwrap().push(b.to_vec());
            Ok(b.len())
        }
        fn close(&mut self) {}
    }
    for (numbered, format, body, expected) in [
        (
            false,
            Format::HidUnnumbered,
            vec![0xa5, 0],
            Some(vec![0, 0xa5, 0]),
        ),
        (
            false,
            Format::Bytes(Kind::Hid),
            vec![0, 0xa5, 0],
            Some(vec![0, 0xa5, 0]),
        ),
        (
            true,
            Format::Bytes(Kind::Hid),
            vec![3, 0xa5, 0],
            Some(vec![3, 0xa5, 0]),
        ),
        (true, Format::HidUnnumbered, vec![0xa5, 0], None),
    ] {
        let audit = Arc::new(Mutex::new(vec![]));
        let source = audit.clone();
        let w = TxWorker::spawn().unwrap();
        let identity = Identity::Hid {
            collection: 0,
            vendor: 0x1209,
            product: 1,
            page: 0xff00,
            usage: 1,
            numbered,
            capacity: 8,
        };
        w.bind(key(), Expected::Exact(identity), true, move || {
            let mut port = hid::Hid::new(
                Source(source),
                if numbered {
                    hid::ReportMode::Numbered
                } else {
                    hid::ReportMode::Unnumbered
                },
            );
            port.enumerate(1)?;
            port.open(0)?;
            port.configure(8)?;
            port.start()?;
            Ok(Box::new(port))
        })
        .unwrap();
        ticket(&w);
        let mut s = spec(1, 1, &body);
        s.format = format;
        let t = register(&w, s);
        let attempt = outcome(&w, w.send_once(&t, 1, 1).unwrap());
        if let Some(expected) = expected {
            assert_eq!(
                audit.lock().unwrap().as_slice(),
                std::slice::from_ref(&expected)
            );
            assert_eq!(attempt.observation.unwrap().bytes(), expected);
        } else {
            assert_eq!(attempt.outcome, Outcome::Rejected(Failure::Payload));
            assert!(audit.lock().unwrap().is_empty());
        }
    }
}

#[test]
fn transient_busy_preserves_unchanged_live_connection_but_never_retries_automatically() {
    let a = Arc::new(Mutex::new(Audit {
        result: Err(Error::Busy),
        ..Default::default()
    }));
    let w = bound(a.clone());
    let t = register(&w, spec(1, 1, &[1, 2, 3]));
    assert_eq!(
        outcome(&w, w.send_once(&t, 1, 1).unwrap()).outcome,
        Outcome::Failed {
            error: Error::Busy,
            accepted: None
        }
    );
    assert_eq!(a.lock().unwrap().writes.len(), 1);
    assert!(snapshot(&w).ticket.is_some());
    a.lock().unwrap().result = Ok(Progress::accepted(3));
    assert!(matches!(
        outcome(&w, w.send_once(&t, 1, 1).unwrap()).outcome,
        Outcome::Complete {
            accepted: Some(3),
            ..
        }
    ));
    assert_eq!(a.lock().unwrap().writes.len(), 2);
}
