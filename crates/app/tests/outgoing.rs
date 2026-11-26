use exmon_app::{incoming, outgoing::*, transmission::Outcome};
use exmon_capture::transmit::Completion;
use std::{
    sync::Arc,
    thread,
    time::{Duration, Instant},
};
fn wait(mut f: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(4);
    while !f() {
        assert!(Instant::now() < end, "outgoing fixture timeout");
        thread::sleep(Duration::from_millis(1));
    }
}
fn snapshot(e: &Editor) -> Snapshot {
    let mut s = None;
    wait(|| {
        s = e.snapshot();
        s.is_some()
    });
    s.unwrap()
}
fn template(e: &Editor, id: u64) -> Arc<Template> {
    snapshot(e)
        .templates
        .into_iter()
        .find(|t| t.id == id)
        .unwrap()
}
fn created(e: &Editor, id: u64) {
    wait(|| snapshot(e).templates.iter().any(|t| t.id == id));
}
fn ready(e: &Editor, id: u64) {
    wait(|| e.can_send(id));
}
#[test]
fn manual_raw_edit_bind_and_explicit_send_are_separate_actions() {
    let e = Editor::spawn().unwrap();
    let id = e.create(Create::raw()).unwrap();
    created(&e, id);
    assert_eq!(e.mock_status().unwrap().calls, 0);
    assert!(!e.can_send(id));
    e.edit(id, Operation::Hex("01 02 A5".into())).unwrap();
    wait(|| template(&e, id).bytes.as_ref() == [1, 2, 165]);
    assert_eq!(e.mock_status().unwrap().calls, 0);
    e.bind_mock(1, true).unwrap();
    ready(&e, id);
    assert_eq!(e.mock_status().unwrap().calls, 0);
    e.send_once(id).unwrap();
    wait(|| template(&e, id).tx_count == 1);
    assert_eq!(e.mock_status().unwrap().last.as_ref(), &[1, 2, 165]);
    assert!(matches!(
        template(&e, id).last,
        Some(Outcome::Complete {
            status: Completion::Accepted,
            ..
        })
    ));
}
#[test]
fn dirty_invalid_hex_and_permission_loss_prevent_old_payload_submission() {
    let e = Editor::spawn().unwrap();
    let id = e.create(Create::raw()).unwrap();
    created(&e, id);
    e.bind_mock(1, true).unwrap();
    ready(&e, id);
    e.invalidate(id).unwrap();
    assert!(!e.can_send(id));
    assert!(e.send_once(id).is_err());
    e.edit(id, Operation::Hex("GG".into())).unwrap();
    wait(|| snapshot(&e).error.is_some());
    assert!(!e.can_send(id));
    assert_eq!(e.mock_status().unwrap().calls, 0);
    e.edit(id, Operation::Validate).unwrap();
    ready(&e, id);
    assert!(e.bind_mock(2, false).is_err());
    assert!(e.send_once(id).is_err());
    assert_eq!(e.mock_status().unwrap().calls, 0);
}
#[test]
fn multiple_queued_creations_publish_every_template_without_auto_send() {
    let e = Editor::spawn().unwrap();
    let a = e.create(Create::raw()).unwrap();
    let b = e.create(Create::raw()).unwrap();
    created(&e, a);
    created(&e, b);
    e.bind_mock(2, true).unwrap();
    ready(&e, a);
    ready(&e, b);
    e.send_once(a).unwrap();
    wait(|| template(&e, a).tx_count == 1);
    e.send_once(b).unwrap();
    wait(|| template(&e, b).tx_count == 1);
    assert_eq!(e.mock_status().unwrap().calls, 2);
}
#[test]
fn scheduled_count_stop_option_failure_and_edit_revocation_use_real_tx_worker() {
    let e = Editor::spawn().unwrap();
    let id = e.create(Create::raw()).unwrap();
    created(&e, id);
    e.bind_mock(1, true).unwrap();
    ready(&e, id);
    e.start_schedule(
        id,
        ScheduleInput {
            interval: "10".into(),
            count: "2".into(),
            ..Default::default()
        },
    )
    .unwrap();
    wait(|| template(&e, id).tx_count == 2);
    thread::sleep(Duration::from_millis(30));
    assert_eq!(e.mock_status().unwrap().calls, 2);
    e.start_schedule(
        id,
        ScheduleInput {
            interval: "10".into(),
            ..Default::default()
        },
    )
    .unwrap();
    wait(|| e.mock_status().is_some_and(|s| s.calls > 2));
    e.stop_schedule(id);
    let n = e.mock_status().unwrap().calls;
    thread::sleep(Duration::from_millis(30));
    assert!(e.mock_status().unwrap().calls <= n + 1);
    e.start_schedule(
        id,
        ScheduleInput {
            interval: "invalid".into(),
            ..Default::default()
        },
    )
    .unwrap();
    wait(|| snapshot(&e).error.is_some());
    let n = e.mock_status().unwrap().calls;
    thread::sleep(Duration::from_millis(20));
    assert_eq!(e.mock_status().unwrap().calls, n);
    e.start_schedule(
        id,
        ScheduleInput {
            interval: "10".into(),
            ..Default::default()
        },
    )
    .unwrap();
    wait(|| e.mock_status().is_some_and(|s| s.calls > n));
    e.invalidate(id).unwrap();
    let n = e.mock_status().unwrap().calls;
    thread::sleep(Duration::from_millis(30));
    assert!(e.mock_status().unwrap().calls <= n + 1);
    assert!(!e.can_send(id));
}
#[test]
fn received_structured_copy_edits_exact_bytes_and_preserves_original() {
    use exmon_app::shell::{Batch, Captured};
    use exmon_domain::{
        Direction,
        observation::{Metadata, Observation},
    };
    let mut source = incoming::Worker::prototype().unwrap();
    source.begin(1).unwrap();
    let mut original = vec![0; 32];
    original[0] = 165;
    original[1] = 1;
    original[2] = 7;
    source
        .submit(Batch {
            generation: 1,
            records: vec![Captured {
                interface_id: 1,
                epoch: 1,
                sequence: 0,
                observation: Observation::new(
                    1,
                    Direction::Rx,
                    Metadata::Udp {
                        local: "127.0.0.1:1".parse().unwrap(),
                        remote: "127.0.0.1:2".parse().unwrap(),
                    },
                    Some(32),
                    &original,
                )
                .unwrap(),
            }],
        })
        .unwrap();
    let mut row = None;
    wait(|| {
        for b in source.drain() {
            row = b.rows.into_iter().find(|r| !r.raw());
        }
        row.is_some()
    });
    let row = row.unwrap();
    let e = Editor::spawn().unwrap();
    let id = e.clone_received(&row).unwrap();
    created(&e, id);
    e.edit(
        id,
        Operation::Field {
            index: 0,
            text: "42".into(),
            engineering: false,
            label: false,
        },
    )
    .unwrap();
    wait(|| template(&e, id).bytes[2] == 42);
    assert_eq!(row.bytes(), original);
    e.bind_mock(1, true).unwrap();
    ready(&e, id);
    e.send_once(id).unwrap();
    wait(|| e.mock_status().is_some_and(|s| s.calls == 1));
    let mut expected = original;
    expected[2] = 42;
    assert_eq!(e.mock_status().unwrap().last.as_ref(), expected);
    assert_eq!(template(&e, id).source.unwrap().observation, 0);
}
#[test]
fn checksums_engineering_enum_validation_and_readonly_hex_use_actual_codec() {
    let mut config=exmon_storage::config::load_project(br#"{"schema_version":1,"protocols":[{"id":"p","name":"Checked","framing":{"kind":"fixed","length":3},"fields":[{"id":"value","offset":0,"length":1,"representation":"unsigned","endian":"big","scale":2,"bias":1},{"id":"mode","offset":1,"length":1,"representation":"unsigned","endian":"big","enumeration":[{"name":"One","value":{"kind":"unsigned","value":1}},{"name":"Two","value":{"kind":"unsigned","value":2}}],"constraints":{"enum_closed":true}}],"checksums":[{"id":"sum","algorithm":"sum8","offset":2,"length":1,"endian":"big","ranges":[{"offset":0,"length":2}]}]}]}"#).unwrap();
    let e = Editor::spawn().unwrap();
    let mut value = Create::raw();
    value.protocol = Some(Arc::new(config.protocols.remove(0)));
    value.body = Some(vec![1, 1, 0]);
    let id = e.create(value).unwrap();
    created(&e, id);
    assert!(!template(&e, id).valid);
    e.bind_mock(1, true).unwrap();
    wait(|| snapshot(&e).connected);
    assert!(e.send_once(id).is_err());
    e.edit(id, Operation::Repair).unwrap();
    wait(|| template(&e, id).bytes.as_ref() == [1, 1, 2]);
    ready(&e, id);
    e.edit(
        id,
        Operation::Field {
            index: 0,
            text: "9".into(),
            engineering: true,
            label: false,
        },
    )
    .unwrap();
    wait(|| template(&e, id).bytes.as_ref() == [4, 1, 5]);
    e.edit(
        id,
        Operation::Field {
            index: 1,
            text: "Two".into(),
            engineering: false,
            label: true,
        },
    )
    .unwrap();
    wait(|| template(&e, id).bytes.as_ref() == [4, 2, 6]);
    ready(&e, id);
    e.send_once(id).unwrap();
    wait(|| e.mock_status().is_some_and(|s| s.calls == 1));
    e.edit(id, Operation::Hex("04 02 FF".into())).unwrap();
    wait(|| template(&e, id).error == Some(Error::Codec));
    assert_eq!(template(&e, id).bytes.as_ref(), &[4, 2, 6]);
    assert!(e.send_once(id).is_err());
    e.edit(id, Operation::Validate).unwrap();
    ready(&e, id);
    e.edit(
        id,
        Operation::Field {
            index: 0,
            text: "8".into(),
            engineering: true,
            label: false,
        },
    )
    .unwrap();
    wait(|| template(&e, id).error == Some(Error::Codec));
    assert!(e.send_once(id).is_err());
    assert_eq!(e.mock_status().unwrap().calls, 1);
}
#[test]
fn missing_preserved_header_is_not_inferred_and_explicit_header_rebuilds_exact_wire() {
    let mut config=exmon_storage::config::load_project(br#"{"schema_version":1,"protocols":[{"id":"p","name":"Length","framing":{"kind":"length_prefix","offset":1,"width":1,"endian":"big","max_length":16}}]}"#).unwrap();
    let e = Editor::spawn().unwrap();
    let mut value = Create::raw();
    value.protocol = Some(Arc::new(config.protocols.remove(0)));
    value.body = Some(vec![7, 8]);
    let id = e.create(value).unwrap();
    created(&e, id);
    assert!(!template(&e, id).valid);
    e.bind_mock(1, true).unwrap();
    wait(|| snapshot(&e).connected);
    assert!(e.send_once(id).is_err());
    e.edit(id, Operation::Header("A5 00".into())).unwrap();
    ready(&e, id);
    e.send_once(id).unwrap();
    wait(|| e.mock_status().is_some_and(|s| s.calls == 1));
    assert_eq!(e.mock_status().unwrap().last.as_ref(), &[165, 2, 7, 8]);
}
#[test]
fn bounded_template_count_and_byte_admission_fail_without_sending() {
    let e = Editor::spawn().unwrap();
    for _ in 0..64 {
        let id = e.create(Create::raw()).unwrap();
        created(&e, id);
    }
    let _ = e.create(Create::raw()).unwrap();
    wait(|| snapshot(&e).error == Some(Error::Capacity));
    assert_eq!(snapshot(&e).templates.len(), 64);
    assert_eq!(e.mock_status().unwrap().calls, 0);
    let mut huge = Create::raw();
    huge.body = Some(vec![0; 65537]);
    assert_eq!(e.create(huge), Err(Error::Invalid));
    let e = Editor::spawn().unwrap();
    for _ in 0..2 {
        let mut value = Create::raw();
        value.body = Some(vec![0; 65536]);
        let id = e.create(value).unwrap();
        created(&e, id);
    }
    let mut value = Create::raw();
    value.body = Some(vec![0; 65536]);
    e.create(value).unwrap();
    wait(|| snapshot(&e).error == Some(Error::Capacity));
    assert_eq!(snapshot(&e).templates.len(), 2);
    assert_eq!(e.mock_status().unwrap().calls, 0);
}
struct Audit {
    calls: u64,
    info: exmon_capture::transmit::Info,
    result: Result<exmon_capture::transmit::Progress, exmon_capture::adapter::Error>,
}
struct Port(std::sync::Arc<std::sync::Mutex<Audit>>);
impl exmon_capture::transmit::Port for Port {
    fn info(&self) -> Result<exmon_capture::transmit::Info, exmon_capture::adapter::Error> {
        Ok(self.0.lock().unwrap().info.clone())
    }
    fn metadata(
        &self,
        _: &exmon_capture::transmit::Packet<'_>,
    ) -> Result<exmon_domain::observation::Metadata, exmon_capture::adapter::Error> {
        Ok(exmon_domain::observation::Metadata::Serial {
            configuration_revision: 1,
        })
    }
    fn submit(
        &mut self,
        _: &exmon_capture::transmit::Packet<'_>,
        _: &exmon_capture::adapter::Cancellation,
    ) -> Result<exmon_capture::transmit::Progress, exmon_capture::adapter::Error> {
        let mut a = self.0.lock().unwrap();
        a.calls += 1;
        a.result
    }
    fn completion(
        &mut self,
    ) -> Result<exmon_capture::transmit::Progress, exmon_capture::adapter::Error> {
        Ok(exmon_capture::transmit::Progress::Pending)
    }
    fn close(&mut self) -> Result<(), exmon_capture::adapter::Error> {
        Ok(())
    }
}
fn audited(
    result: Result<exmon_capture::transmit::Progress, exmon_capture::adapter::Error>,
) -> (Editor, u64, std::sync::Arc<std::sync::Mutex<Audit>>) {
    use exmon_app::transmission::{Binding, Expected};
    use exmon_capture::{
        adapter::Availability,
        transmit::{Identity, Info},
    };
    let e = Editor::spawn().unwrap();
    let id = e.create(Create::raw()).unwrap();
    created(&e, id);
    let a = std::sync::Arc::new(std::sync::Mutex::new(Audit {
        calls: 0,
        info: Info {
            identity: Identity::Serial {
                device: 1,
                revision: 1,
            },
            writable: Availability::Available,
            max_bytes: 65536,
        },
        result,
    }));
    let port = a.clone();
    e.bind_port(
        Binding {
            session: 1,
            interface: 1,
            epoch: 1,
            configuration: 1,
            protocol: 1,
        },
        Expected::Exact(a.lock().unwrap().info.identity.clone()),
        true,
        move || Ok(Box::new(Port(port))),
    )
    .unwrap();
    ready(&e, id);
    (e, id, a)
}
#[test]
fn actual_partial_unknown_busy_and_interface_loss_are_not_successful_tx_counts() {
    use exmon_capture::{
        adapter::{Availability, Error as Backend},
        transmit::Progress,
    };
    for result in [
        Ok(Progress::accepted(1)),
        Ok(Progress::Complete {
            accepted: None,
            status: Completion::Timeout,
        }),
        Err(Backend::Busy),
    ] {
        let (e, id, a) = audited(result);
        e.send_once(id).unwrap();
        wait(|| template(&e, id).last.is_some());
        assert_eq!(template(&e, id).tx_count, 0);
        assert_eq!(a.lock().unwrap().calls, 1);
        if matches!(result, Err(Backend::Busy)) {
            a.lock().unwrap().result = Ok(Progress::accepted(3));
            e.send_once(id).unwrap();
            wait(|| template(&e, id).tx_count == 1);
            assert_eq!(a.lock().unwrap().calls, 2);
        }
    }
    let (e, id, a) = audited(Ok(Progress::accepted(3)));
    a.lock().unwrap().info.writable = Availability::Unavailable(Backend::Disconnected);
    e.send_once(id).unwrap();
    wait(|| template(&e, id).last.is_some());
    assert_eq!(a.lock().unwrap().calls, 0);
    assert!(!e.can_send(id));
    let (e, id, a) = audited(Err(Backend::Disconnected));
    e.send_once(id).unwrap();
    wait(|| template(&e, id).last.is_some());
    assert_eq!(a.lock().unwrap().calls, 1);
    assert!(!e.can_send(id));
}
#[test]
fn oversized_edit_revokes_active_schedule_before_returning_admission_error() {
    let e = Editor::spawn().unwrap();
    let id = e.create(Create::raw()).unwrap();
    created(&e, id);
    e.bind_mock(1, true).unwrap();
    ready(&e, id);
    e.start_schedule(
        id,
        ScheduleInput {
            interval: "10".into(),
            ..Default::default()
        },
    )
    .unwrap();
    wait(|| e.mock_status().is_some_and(|s| s.calls > 0));
    assert_eq!(
        e.edit(id, Operation::Hex("0".repeat(3 * 65536 + 1))),
        Err(Error::Capacity)
    );
    let calls = e.mock_status().unwrap().calls;
    thread::sleep(Duration::from_millis(30));
    assert!(e.mock_status().unwrap().calls <= calls + 1);
    assert!(e.send_once(id).is_err());
}
#[test]
fn variable_length_structured_hex_preserves_checksum_ownership_and_unknown_tails() {
    let mut config=exmon_storage::config::load_project(br#"{"schema_version":1,"protocols":[{"id":"p","name":"Variable","framing":{"kind":"datagram"},"fields":[{"id":"value","offset":0,"length":1,"representation":"unsigned","endian":"big"}],"checksums":[{"id":"sum","algorithm":"sum8","offset":1,"length":1,"endian":"big","ranges":[{"offset":0,"length":1}]}]}]}"#).unwrap();
    let e = Editor::spawn().unwrap();
    let mut value = Create::raw();
    value.protocol = Some(Arc::new(config.protocols.remove(0)));
    value.body = Some(vec![1, 1, 153]);
    let id = e.create(value).unwrap();
    created(&e, id);
    e.edit(id, Operation::Hex("02 01 99 88".into())).unwrap();
    wait(|| template(&e, id).bytes.as_ref() == [2, 2, 153, 136]);
    assert!(template(&e, id).valid);
    e.bind_mock(1, true).unwrap();
    ready(&e, id);
    e.send_once(id).unwrap();
    wait(|| e.mock_status().is_some_and(|s| s.calls == 1));
    assert_eq!(e.mock_status().unwrap().last.as_ref(), &[2, 2, 153, 136]);
    e.edit(id, Operation::Hex("02".into())).unwrap();
    wait(|| template(&e, id).error == Some(Error::Codec));
    assert_eq!(template(&e, id).bytes.as_ref(), &[2, 2, 153, 136]);
    assert!(e.send_once(id).is_err());
}
#[test]
fn independent_template_enables_do_not_supersede_each_other() {
    let e = Editor::spawn().unwrap();
    let a = e.create(Create::raw()).unwrap();
    let b = e.create(Create::raw()).unwrap();
    created(&e, a);
    created(&e, b);
    e.bind_mock(1, true).unwrap();
    ready(&e, a);
    ready(&e, b);
    e.start_schedule(
        a,
        ScheduleInput {
            interval: "50".into(),
            count: "1".into(),
            ..Default::default()
        },
    )
    .unwrap();
    e.start_schedule(
        b,
        ScheduleInput {
            interval: "50".into(),
            count: "1".into(),
            ..Default::default()
        },
    )
    .unwrap();
    wait(|| template(&e, a).tx_count == 1 && template(&e, b).tx_count == 1);
    assert_eq!(e.mock_status().unwrap().calls, 2);
}
