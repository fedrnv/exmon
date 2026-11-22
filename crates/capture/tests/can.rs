use exmon_capture::{adapter::*, can::*, native_can, virtual_can::VirtualBus};
use exmon_domain::{
    Direction,
    observation::{Metadata, Observation},
};
fn open<S: Source>(source: S, options: Options) -> Can<S> {
    let mut adapter = Can::new(source, options).unwrap();
    let channels = adapter.enumerate(64).unwrap();
    adapter.open(channels[0].id).unwrap();
    adapter.configure(8).unwrap();
    adapter.start().unwrap();
    adapter
}
fn data(id: u32) -> Frame {
    Frame::new(id, 3, &[165, 0, 42]).unwrap()
}
#[test]
fn virtual_exchange_filters_standard_extended_remote_and_preserves_metadata() {
    let bus = VirtualBus::new(8).unwrap();
    let mut sender = open(bus.endpoint(), Options::default());
    let mut receiver = open(
        bus.endpoint(),
        Options {
            filters: vec![Filter {
                id: 0x123,
                mask: EFF | RTR | ID_MASK,
            }],
            ..Options::default()
        },
    );
    let cancellation = Cancellation::default();
    for frame in [
        data(0x122),
        data(EFF | 0x123),
        Frame::new(RTR | 0x123, 7, &[]).unwrap(),
        data(0x123),
    ] {
        sender.send_frame(&frame, &cancellation).unwrap();
    }
    let frames = receiver.poll(&cancellation, 8, 64).unwrap();
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].bytes(), [165, 0, 42]);
    assert_eq!(frames[0].source_timestamp_ns(), Some(4));
    assert_eq!(frames[0].direction(), Direction::Rx);
    assert_eq!(
        frames[0].metadata(),
        &Metadata::Can {
            channel: 1,
            id: 0x123,
            extended: false,
            remote: false,
            error: false,
            fd: false,
            dlc: 3
        }
    );
    assert!(sender.poll(&cancellation, 8, 64).unwrap().is_empty());
    let mut all = open(bus.endpoint(), Options::default());
    for frame in [
        Frame::new(EFF | 0x123, 15, &[1; 8]).unwrap(),
        Frame::new(RTR | 0x321, 8, &[]).unwrap(),
    ] {
        sender.send_frame(&frame, &cancellation).unwrap();
    }
    let frames = all.poll(&cancellation, 8, 64).unwrap();
    assert_eq!(frames.len(), 2);
    assert!(matches!(
        frames[0].metadata(),
        Metadata::Can {
            extended: true,
            dlc: 15,
            ..
        }
    ));
    assert_eq!(frames[0].bytes(), [1; 8]);
    assert!(matches!(
        frames[1].metadata(),
        Metadata::Can {
            remote: true,
            dlc: 8,
            ..
        }
    ));
    assert!(frames[1].bytes().is_empty());
    assert_eq!(frames[1].original_length(), Some(0));
}
#[test]
fn bounded_bus_loss_session_limits_cancel_and_reconnect_are_explicit() {
    let bus = VirtualBus::new(2).unwrap();
    let mut sender = open(bus.endpoint(), Options::default());
    let mut receiver = Can::new(bus.endpoint(), Options::default()).unwrap();
    receiver.enumerate(1).unwrap();
    let mut session = Session::new(
        receiver,
        Limits {
            queue_items: 2,
            queue_bytes: 4,
            batch_items: 2,
            batch_bytes: 4,
        },
    )
    .unwrap();
    session.open(1).unwrap();
    session.configure(2).unwrap();
    session.start().unwrap();
    for _ in 0..4 {
        sender
            .send_frame(&data(0x100), &Cancellation::default())
            .unwrap();
    }
    session.pump().unwrap();
    let captured = session.drain(2);
    assert_eq!(captured.len(), 2);
    assert_eq!(captured[0].observation.bytes(), [165, 0]);
    assert_eq!(captured[0].observation.original_length(), Some(3));
    assert_eq!(
        session
            .metrics()
            .snapshot()
            .count(exmon_domain::metrics::Counter::BackendLoss),
        2
    );
    let cancel = Cancellation::default();
    cancel.cancel();
    assert_eq!(sender.send_frame(&data(1), &cancel), Err(Error::Cancelled));
    session.cancellation().cancel();
    assert_eq!(session.pump(), Err(Error::Cancelled));
    session.reconnect(1).unwrap();
    assert_eq!(session.state(), State::Open);
    assert_eq!(session.pump(), Err(Error::InvalidState));
    session.configure(2).unwrap();
    session.start().unwrap();
    session.pump().unwrap();
    assert!(session.drain(2).is_empty());
    sender.stop().unwrap();
    assert_eq!(
        sender.send_frame(&data(1), &Cancellation::default()),
        Err(Error::InvalidState)
    );
}
#[test]
fn invalid_can_frames_modes_filters_and_raw_byte_tx_are_rejected() {
    for (id, dlc, bytes) in [
        (0x800, 0, &[][..]),
        (1, 16, &[]),
        (1, 1, &[]),
        (RTR | 1, 1, &[1]),
        (ERR | RTR, 8, &[]),
        (ERR | EFF, 8, &[0; 8]),
    ] {
        assert_eq!(Frame::new(id, dlc, bytes).err(), Some(Error::InvalidConfig));
    }
    assert!(Frame::new(EFF | ID_MASK, 8, &[0; 8]).is_ok());
    assert_eq!(
        Options {
            mode: Mode::Fd,
            ..Options::default()
        }
        .validate(),
        Err(Error::Unsupported)
    );
    assert_eq!(
        Options {
            filters: vec![Filter { id: ERR, mask: 0 }],
            ..Options::default()
        }
        .validate(),
        Err(Error::InvalidConfig)
    );
    assert_eq!(
        Options {
            filters: vec![Filter { id: 0, mask: 0 }; 65],
            ..Options::default()
        }
        .validate(),
        Err(Error::InvalidConfig)
    );
    let bus = VirtualBus::new(1).unwrap();
    let mut sender = open(bus.endpoint(), Options::default());
    let mut none = open(
        bus.endpoint(),
        Options {
            filters: vec![],
            ..Options::default()
        },
    );
    assert_eq!(sender.transmit(&[1, 2]), Err(Error::Unsupported));
    let error = Frame::new(ERR | 4, 8, &[0; 8]).unwrap();
    assert_eq!(
        sender.send_frame(&error, &Cancellation::default()),
        Err(Error::Unsupported)
    );
    sender
        .send_frame(&data(1), &Cancellation::default())
        .unwrap();
    assert!(
        none.poll(&Cancellation::default(), 1, 8)
            .unwrap()
            .is_empty()
    );
    assert!(!Options::default().accepts(&error));
    assert!(
        Options {
            receive_errors: true,
            ..Options::default()
        }
        .accepts(&error)
    );
    let bad_meta = Metadata::Can {
        channel: 1,
        id: 1,
        extended: false,
        remote: true,
        error: false,
        fd: false,
        dlc: 8,
    };
    assert!(Observation::new(0, Direction::Rx, bad_meta, None, &[1]).is_err());
}
#[test]
fn unavailable_backends_and_activation_failure_do_not_send() {
    if !cfg!(all(target_os = "linux", feature = "native-can")) {
        assert_eq!(native_can::capability(), Availability::Unsupported);
        let mut native = native_can::NativeCan::default();
        assert_eq!(native.list(1).err(), Some(Error::Unsupported));
        assert_eq!(
            native.activate(1, &Options::default()),
            Err(Error::Unsupported)
        );
        assert_eq!(native.send(&data(1)), Err(Error::Unsupported));
    }
    assert!(!native_can::requirements().is_empty());
    struct Denied;
    impl Source for Denied {
        fn list(&mut self, _: usize) -> Result<Vec<Channel>, Error> {
            Ok(vec![Channel {
                id: 1,
                name: "synthetic-denied".into(),
                tx: Availability::Available,
            }])
        }
        fn activate(&mut self, _: u32, _: &Options) -> Result<(), Error> {
            Err(Error::PermissionDenied)
        }
        fn next(&mut self) -> Result<Option<Received>, Error> {
            panic!("must not poll failed source")
        }
        fn send(&mut self, _: &Frame) -> Result<(), Error> {
            panic!("must not send failed source")
        }
        fn close(&mut self) {}
    }
    let mut adapter = Can::new(Denied, Options::default()).unwrap();
    adapter.enumerate(1).unwrap();
    adapter.open(1).unwrap();
    adapter.configure(8).unwrap();
    assert_eq!(adapter.start(), Err(Error::PermissionDenied));
    assert_eq!(
        adapter.send_frame(&data(1), &Cancellation::default()),
        Err(Error::InvalidState)
    );
}

#[test]
fn error_metadata_is_receive_only_and_send_disconnect_revokes_running_source() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct Fault {
        closes: Arc<AtomicUsize>,
        delivered: bool,
    }
    impl Source for Fault {
        fn list(&mut self, _: usize) -> Result<Vec<Channel>, Error> {
            Ok(vec![Channel {
                id: 7,
                name: "synthetic-errors".into(),
                tx: Availability::Available,
            }])
        }
        fn activate(&mut self, _: u32, _: &Options) -> Result<(), Error> {
            Ok(())
        }
        fn next(&mut self) -> Result<Option<Received>, Error> {
            if self.delivered {
                return Ok(None);
            }
            self.delivered = true;
            Ok(Some(Received {
                frame: Frame::new(ERR | 4, 8, &[0, 1, 2, 3, 4, 5, 6, 7]).unwrap(),
                direction: Direction::Unknown,
                source_timestamp_ns: None,
            }))
        }
        fn send(&mut self, _: &Frame) -> Result<(), Error> {
            Err(Error::Disconnected)
        }
        fn close(&mut self) {
            self.closes.fetch_add(1, Ordering::SeqCst);
        }
    }
    let closes = Arc::new(AtomicUsize::new(0));
    let mut adapter = open(
        Fault {
            closes: closes.clone(),
            delivered: false,
        },
        Options {
            receive_errors: true,
            ..Options::default()
        },
    );
    let received = adapter.poll(&Cancellation::default(), 4, 32).unwrap();
    assert_eq!(received.len(), 1);
    assert_eq!(received[0].bytes(), [0, 1, 2, 3, 4, 5, 6, 7]);
    assert_eq!(received[0].direction(), Direction::Unknown);
    assert_eq!(received[0].source_timestamp_ns(), None);
    assert!(matches!(
        received[0].metadata(),
        Metadata::Can {
            channel: 7,
            id: 4,
            error: true,
            dlc: 8,
            ..
        }
    ));
    assert_eq!(
        adapter.send_frame(&data(1), &Cancellation::default()),
        Err(Error::Disconnected)
    );
    assert_eq!(closes.load(Ordering::SeqCst), 1);
    assert_eq!(
        adapter.send_frame(&data(1), &Cancellation::default()),
        Err(Error::InvalidState)
    );
}

/// Explicit opt-in only; verifies link kind before any native socket/TX.
#[cfg(all(target_os = "linux", feature = "native-can"))]
#[test]
#[ignore = "requires an externally prepared vcan and EXMON_TEST_VCAN; never physical CAN"]
fn native_vcan_rx_tx_and_filtering() {
    use std::{
        process::Command,
        time::{Duration, Instant},
    };
    let name = std::env::var("EXMON_TEST_VCAN")
        .expect("set EXMON_TEST_VCAN to approved virtual interface");
    assert!(
        !name.is_empty()
            && name.len() <= 15
            && name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    );
    let output = Command::new("ip")
        .args(["-details", "-json", "link", "show", "dev", &name])
        .output()
        .expect("iproute2 required");
    assert!(output.status.success());
    let info: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        info[0]["linkinfo"]["info_kind"], "vcan",
        "refuse non-vcan interface before opening socket"
    );
    let options = Options {
        filters: vec![Filter {
            id: 0x123,
            mask: EFF | RTR | ID_MASK,
        }],
        ..Options::default()
    };
    fn selected(name: &str, options: Options) -> Can<native_can::NativeCan> {
        let mut a = Can::new(native_can::NativeCan::default(), options).unwrap();
        let channels = a.enumerate(64).unwrap();
        let id = channels
            .iter()
            .find(|c| a.channel_name(c.id) == Some(name))
            .expect("virtual CAN not discovered")
            .id;
        a.open(id).unwrap();
        a.configure(8).unwrap();
        a.start().unwrap();
        a
    }
    let mut receiver = selected(&name, options);
    let mut sender = selected(&name, Options::default());
    sender
        .send_frame(&data(0x122), &Cancellation::default())
        .unwrap();
    sender
        .send_frame(&data(0x123), &Cancellation::default())
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(1);
    let mut received = vec![];
    while Instant::now() < deadline {
        received.extend(receiver.poll(&Cancellation::default(), 8, 64).unwrap());
        if !received.is_empty() {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(received.len(), 1);
    assert_eq!(received[0].bytes(), [165, 0, 42]);
    assert!(received[0].source_timestamp_ns().is_some());
    assert!(matches!(
        received[0].metadata(),
        Metadata::Can { id: 0x123, .. }
    ));
    receiver.stop().unwrap();
    sender.stop().unwrap();
}
