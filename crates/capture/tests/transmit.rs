use exmon_capture::{
    adapter::{Adapter, Availability, Cancellation, Error},
    can, hid, serial,
    sockets::{Mode, Settings, Sockets},
    transmit::{Completion, Identity, Packet, Port, Progress, SocketPort, SocketTarget},
    usb_transfers as usb,
};
use exmon_domain::observation::Metadata;
use std::{
    io::Read,
    net::{TcpListener, UdpSocket},
    sync::{Arc, Mutex},
    time::Duration,
};
#[derive(Default)]
struct Audit {
    selected: Option<usize>,
    writes: Vec<Vec<u8>>,
    partial: Option<usize>,
    removed: bool,
}
struct Fixture(Arc<Mutex<Audit>>);
impl hid::Source for Fixture {
    fn enumerate(&mut self, _: usize) -> Result<Vec<hid::Device>, Error> {
        Ok((0..2)
            .map(|id| hid::Device {
                vendor_id: 0x1209,
                product_id: id,
                usage_page: 0xff00,
                usage: 1,
            })
            .collect())
    }
    fn open_selected(&mut self, index: usize) -> Result<(), Error> {
        self.0.lock().unwrap().selected = Some(index);
        Ok(())
    }
    fn read(&mut self, _: &mut [u8], _: i32) -> Result<usize, Error> {
        Ok(0)
    }
    fn write(&mut self, bytes: &[u8]) -> Result<usize, Error> {
        let mut a = self.0.lock().unwrap();
        if a.removed {
            return Err(Error::Disconnected);
        }
        a.writes.push(bytes.to_vec());
        Ok(a.partial.unwrap_or(bytes.len()))
    }
    fn close(&mut self) {
        self.0.lock().unwrap().selected = None;
    }
}
impl serial::Source for Fixture {
    fn list(&mut self, _: usize) -> Result<Vec<String>, Error> {
        Ok(vec!["synthetic0".into(), "synthetic1".into()])
    }
    fn open(&mut self, name: &str, _: serial::Settings) -> Result<(), Error> {
        self.0.lock().unwrap().selected = Some(if name == "synthetic1" { 1 } else { 0 });
        Ok(())
    }
    fn read(&mut self, _: &mut [u8]) -> Result<usize, Error> {
        Ok(0)
    }
    fn write(&mut self, b: &[u8]) -> Result<usize, Error> {
        hid::Source::write(self, b)
    }
    fn close(&mut self) {
        hid::Source::close(self)
    }
}
fn active<A: Adapter>(a: &mut A, id: u64, size: usize) {
    a.enumerate(64).unwrap();
    a.open(id).unwrap();
    a.configure(size).unwrap();
    a.start().unwrap();
}
#[test]
fn hid_and_serial_bridge_keep_selected_identity_validate_and_report_partial_counts() {
    let audit = Arc::new(Mutex::new(Audit::default()));
    let mut hid = hid::Hid::new(Fixture(audit.clone()), hid::ReportMode::Unnumbered);
    assert!(Port::info(&hid).is_err());
    active(&mut hid, 1, 64);
    assert!(matches!(
        Port::info(&hid).unwrap().identity,
        Identity::Hid {
            collection: 1,
            numbered: false,
            ..
        }
    ));
    assert_eq!(
        Port::submit(&mut hid, &Packet::Bytes(&[1, 2]), &Cancellation::default()),
        Err(Error::InvalidConfig)
    );
    assert!(audit.lock().unwrap().writes.is_empty());
    assert!(matches!(
        Port::metadata(&hid, &Packet::Bytes(&[0, 2])).unwrap(),
        Metadata::Hid {
            product_id: 1,
            report_id: 0,
            ..
        }
    ));
    let cancel = Cancellation::default();
    cancel.cancel();
    assert_eq!(
        Port::submit(&mut hid, &Packet::Bytes(&[0, 2]), &cancel),
        Err(Error::Cancelled)
    );
    audit.lock().unwrap().partial = Some(1);
    assert_eq!(
        Port::submit(&mut hid, &Packet::Bytes(&[0, 2]), &Cancellation::default()),
        Ok(Progress::accepted(1))
    );
    Port::close(&mut hid).unwrap();
    assert!(Port::info(&hid).is_err());
    let mut serial =
        serial::Serial::new(Fixture(audit.clone()), serial::Settings::default()).unwrap();
    active(&mut serial, 1, 64);
    assert!(matches!(
        Port::info(&serial).unwrap().identity,
        Identity::Serial { device: 1, .. }
    ));
    assert_eq!(
        Port::submit(&mut serial, &Packet::Bytes(&[]), &Cancellation::default()),
        Err(Error::InvalidConfig)
    );
    audit.lock().unwrap().partial = Some(0);
    assert_eq!(
        Port::submit(
            &mut serial,
            &Packet::Bytes(&[1, 2]),
            &Cancellation::default()
        ),
        Ok(Progress::accepted(0))
    );
    audit.lock().unwrap().removed = true;
    assert_eq!(
        Port::submit(&mut serial, &Packet::Bytes(&[1]), &Cancellation::default()),
        Err(Error::Disconnected)
    );
    assert!(Port::info(&serial).is_err());
    assert_eq!(audit.lock().unwrap().writes, [vec![0, 2], vec![1, 2]]);
}
struct CanFixture {
    sent: Arc<Mutex<Vec<u32>>>,
    writable: Availability,
}
impl can::Source for CanFixture {
    fn list(&mut self, _: usize) -> Result<Vec<can::Channel>, Error> {
        Ok(vec![can::Channel {
            id: 7,
            name: "synthetic".into(),
            tx: self.writable,
        }])
    }
    fn activate(&mut self, id: u32, _: &can::Options) -> Result<(), Error> {
        assert_eq!(id, 7);
        Ok(())
    }
    fn next(&mut self) -> Result<Option<can::Received>, Error> {
        Ok(None)
    }
    fn send(&mut self, f: &can::Frame) -> Result<(), Error> {
        self.sent.lock().unwrap().push(f.id_word());
        Ok(())
    }
    fn close(&mut self) {}
}
#[test]
fn can_requires_typed_frame_permission_and_preserves_rtr_dlc_without_fake_payload() {
    let sent = Arc::new(Mutex::new(vec![]));
    for writable in [
        Availability::Available,
        Availability::Unavailable(Error::PermissionDenied),
    ] {
        let mut a = can::Can::new(
            CanFixture {
                sent: sent.clone(),
                writable,
            },
            can::Options {
                filters: vec![],
                receive_errors: false,
                mode: can::Mode::Classic,
            },
        )
        .unwrap();
        active(&mut a, 7, 8);
        let frame = can::Frame::new(can::RTR | 0x123, 8, &[]).unwrap();
        let meta = Port::metadata(&a, &Packet::Can(&frame)).unwrap();
        assert!(matches!(
            meta,
            Metadata::Can {
                channel: 7,
                id: 0x123,
                remote: true,
                dlc: 8,
                ..
            }
        ));
        assert_eq!(
            Port::submit(&mut a, &Packet::Bytes(&[1]), &Cancellation::default()),
            Err(if writable == Availability::Available {
                Error::InvalidConfig
            } else {
                Error::PermissionDenied
            })
        );
        let result = Port::submit(&mut a, &Packet::Can(&frame), &Cancellation::default());
        if writable == Availability::Available {
            assert_eq!(result, Ok(Progress::accepted(0)))
        } else {
            assert_eq!(result, Err(Error::PermissionDenied))
        }
        let error = can::Frame::new(can::ERR | 0x123, 8, &[0; 8]).unwrap();
        assert_eq!(
            Port::metadata(&a, &Packet::Can(&error)),
            Err(Error::Unsupported)
        );
    }
    assert_eq!(*sent.lock().unwrap(), [can::RTR | 0x123]);
}
struct UsbFixture {
    pending: Option<Vec<u8>>,
    delay: bool,
    device: Arc<Mutex<Option<usize>>>,
}
impl usb::Source for UsbFixture {
    fn enumerate(&mut self, _: usize) -> Result<Vec<usb::Device>, Error> {
        Ok(vec![
            usb::Device {
                vendor_id: 0x1209,
                product_id: 1
            };
            2
        ])
    }
    fn inspect(&mut self, index: usize) -> Result<usb::Descriptors, Error> {
        *self.device.lock().unwrap() = Some(index);
        Ok(usb::Descriptors {
            active: 1,
            configurations: vec![usb::Configuration {
                value: 1,
                interfaces: vec![usb::Interface {
                    number: 2,
                    alternate: 0,
                    endpoints: vec![usb::Endpoint {
                        address: 1,
                        kind: usb::Kind::Bulk,
                        packet_size: 64,
                    }],
                }],
            }],
        })
    }
    fn claim(&mut self, s: usb::Selection) -> Result<(), Error> {
        assert_eq!(s.output, Some(1));
        Ok(())
    }
    fn submit(
        &mut self,
        d: usb::Direction,
        e: usb::Endpoint,
        b: &[u8],
        n: usize,
    ) -> Result<(), Error> {
        assert_eq!(d, usb::Direction::Out);
        assert_eq!(e.address, 1);
        assert_eq!(b.len(), n);
        self.pending = Some(b.to_vec());
        self.delay = true;
        Ok(())
    }
    fn poll(&mut self, _: usb::Direction) -> Result<Option<usb::Completion>, Error> {
        if self.delay {
            self.delay = false;
            return Ok(None);
        }
        Ok(self.pending.take().map(|bytes| usb::Completion {
            direction: usb::Direction::Out,
            endpoint: 1,
            actual_length: Some(bytes.len()),
            bytes,
            status: usb::Status::Success,
        }))
    }
    fn release(&mut self) {
        self.pending = None;
    }
}
#[test]
fn usb_submission_is_pending_until_matching_completion_and_close_keeps_unknown_cancel_count() {
    let device = Arc::new(Mutex::new(None));
    let mut a = usb::Usb::new(
        UsbFixture {
            pending: None,
            delay: false,
            device: device.clone(),
        },
        Duration::from_millis(100),
    )
    .unwrap();
    a.enumerate().unwrap();
    a.inspect(1).unwrap();
    a.claim(usb::Selection {
        configuration: 1,
        interface: 2,
        alternate: 0,
        input: None,
        output: Some(1),
    })
    .unwrap();
    assert_eq!(*device.lock().unwrap(), Some(1));
    assert!(matches!(
        Port::info(&a).unwrap().identity,
        Identity::Usb {
            device: 1,
            endpoint: 1,
            ..
        }
    ));
    assert_eq!(
        Port::submit(&mut a, &Packet::Bytes(&[1, 2, 3]), &Cancellation::default()),
        Ok(Progress::Pending)
    );
    assert_eq!(Port::completion(&mut a), Ok(Progress::Pending));
    assert_eq!(Port::completion(&mut a), Ok(Progress::accepted(3)));
    Port::submit(&mut a, &Packet::Bytes(&[4]), &Cancellation::default()).unwrap();
    Port::close(&mut a).unwrap();
    assert_eq!(
        Port::completion(&mut a),
        Ok(Progress::Complete {
            accepted: None,
            status: Completion::Cancelled
        })
    );
    assert!(Port::info(&a).is_err());
}
fn socket(mode: Mode) -> Sockets {
    let mut a = Sockets::new(Settings::new(mode)).unwrap();
    active(&mut a, 0, 65536);
    a
}
#[test]
fn actual_windows_tcp_udp_loopback_bridges_target_exact_connection_and_no_fallback() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let a = socket(Mode::TcpClient {
        local: "127.0.0.1:0".parse().unwrap(),
        remote: listener.local_addr().unwrap(),
    });
    let (mut receiver, _) = listener.accept().unwrap();
    receiver
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let peer = a.peers()[0].connection;
    let mut port = SocketPort::new(a, SocketTarget::Tcp(peer)).unwrap();
    assert_eq!(
        port.submit(&Packet::Bytes(b"abc"), &Cancellation::default()),
        Ok(Progress::accepted(3))
    );
    let mut bytes = [0; 3];
    receiver.read_exact(&mut bytes).unwrap();
    assert_eq!(&bytes, b"abc");
    port.close().unwrap();
    assert!(port.info().is_err());
    let receiver = UdpSocket::bind("127.0.0.1:0").unwrap();
    receiver
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let remote = receiver.local_addr().unwrap();
    let a = socket(Mode::UdpConnected {
        local: "127.0.0.1:0".parse().unwrap(),
        remote,
    });
    assert!(SocketPort::new(a, SocketTarget::Udp("127.0.0.1:1".parse().unwrap())).is_err());
    let a = socket(Mode::UdpBound {
        local: "127.0.0.1:0".parse().unwrap(),
    });
    let mut port = SocketPort::new(a, SocketTarget::Udp(remote)).unwrap();
    assert_eq!(
        port.submit(&Packet::Bytes(&[]), &Cancellation::default()),
        Ok(Progress::accepted(0))
    );
    assert_eq!(receiver.recv_from(&mut [0; 8]).unwrap().0, 0);
    assert_eq!(
        port.submit(&Packet::Bytes(&vec![0; 65508]), &Cancellation::default()),
        Err(Error::InvalidConfig)
    );
    assert!(!format!("{:?}", port.info().unwrap()).contains("127.0.0.1"));
}
