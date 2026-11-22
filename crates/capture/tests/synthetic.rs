use exmon_capture::{adapter::*, synthetic::Synthetic};
use exmon_domain::observation::TransportKind;
#[test]
fn synthetic_transports_are_bounded_rx_only_and_poll_on_the_owning_thread() {
    for kind in [
        TransportKind::Ethernet,
        TransportKind::Tcp,
        TransportKind::Udp,
        TransportKind::Serial,
        TransportKind::Usb,
        TransportKind::Can,
    ] {
        let mut adapter = Synthetic::new(kind);
        assert_eq!(adapter.enumerate(2).unwrap().len(), 2);
        assert_eq!(adapter.open(1).unwrap().tx, Availability::Unsupported);
        adapter.configure(64).unwrap();
        adapter.start().unwrap();
        let batch = adapter.poll(&Cancellation::default(), 32, 2048).unwrap();
        assert_eq!(batch.len(), 32);
        assert!(
            batch
                .iter()
                .all(|o| o.metadata().kind() == kind && o.bytes().len() <= 32)
        );
        assert_eq!(adapter.transmit(&[1]), Err(Error::Unsupported));
        let error = std::thread::spawn(move || adapter.poll(&Cancellation::default(), 1, 64).err())
            .join()
            .unwrap();
        assert_eq!(error, Some(Error::Backend));
    }
}
