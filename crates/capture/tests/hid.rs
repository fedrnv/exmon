use exmon_capture::{adapter::*, hid::*};
use exmon_domain::{Direction, observation::Metadata};
use std::sync::{Arc, Mutex};

#[cfg(all(target_os = "linux", feature = "native-hid", feature = "native-usbmon"))]
mod native_virtual {
    use super::*;
    use exmon_capture::{native_hid::NativeHid, native_usbmon::NativeUsbmon};
    use std::{
        fs,
        io::Write,
        process::Command,
        time::{Duration, Instant},
    };

    fn text(path: impl AsRef<std::path::Path>) -> String {
        fs::read_to_string(path).unwrap().trim().to_owned()
    }
    fn fixture() -> (u16, u8) {
        assert_eq!(
            std::env::var("EXMON_TEST_HID").as_deref(),
            Ok("qemu-keyboard")
        );
        assert_eq!(text("/sys/class/dmi/id/product_name"), "EXMON-HID-FIXTURE");
        assert_eq!(text("/sys/class/dmi/id/sys_vendor"), "EXMON-QEMU");
        let entries: Vec<_> = fs::read_dir("/sys/class/hidraw").unwrap().take(4).collect();
        assert_eq!(
            entries.len(),
            1,
            "fixture must contain exactly one emulated HID collection"
        );
        let path = fs::canonicalize(entries[0].as_ref().unwrap().path().join("device")).unwrap();
        let usb = path
            .ancestors()
            .find(|p| p.join("busnum").is_file())
            .unwrap();
        assert_eq!(text(usb.join("idVendor")), "0627");
        assert_eq!(text(usb.join("idProduct")), "0001");
        assert_eq!(text(usb.join("serial")), "EXMON-SYNTHETIC-HID");
        let bus = text(usb.join("busnum")).parse().unwrap();
        let device = text(usb.join("devnum")).parse().unwrap();
        assert_ne!(bus, 0, "never capture all buses");
        (bus, device)
    }
    fn selected() -> Hid<NativeHid> {
        let mut adapter = Hid::new(NativeHid::default(), ReportMode::Unnumbered);
        let devices = adapter.enumerate(64).unwrap();
        assert_eq!(devices.len(), 1);
        let id = devices[0].id;
        let d = adapter.device(id).unwrap();
        assert_eq!(
            (d.vendor_id, d.product_id, d.usage_page, d.usage),
            (0x0627, 1, 1, 6)
        );
        adapter.open(id).unwrap();
        adapter.configure(8).unwrap();
        adapter
    }
    #[test]
    #[ignore = "requires isolated EXMON QEMU USB keyboard and monitor-injected synthetic key"]
    fn native_qemu_keyboard_input_and_output_fidelity() {
        let (bus, device) = fixture();
        let mut adapter = selected();
        adapter.start().unwrap();
        let mut spy = NativeUsbmon::default();
        spy.open(bus).unwrap();
        assert_eq!(adapter.transmit(&[0, 2]), Ok(2));
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut submission = None;
        let mut completed = false;
        while Instant::now() < deadline {
            if let Some(record) = spy.poll().unwrap() {
                let m = record.metadata;
                if m.device == device && m.transfer == b'C' && m.endpoint == 0 {
                    if m.event == b'S' && m.setup == Some([0x21, 9, 0, 2, 0, 0, 1, 0]) {
                        assert_eq!(record.bytes, [2]);
                        submission = Some(m.tag);
                    }
                    if m.event == b'C' && submission == Some(m.tag) {
                        assert_eq!(m.status, Some(0));
                        completed = true;
                        break;
                    }
                }
            } else {
                std::thread::sleep(Duration::from_millis(1));
            }
        }
        spy.stop();
        assert!(
            completed,
            "expected synthetic LED SET_REPORT and successful callback"
        );
        println!("EXMON_HID_READY");
        std::io::stdout().flush().unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut received = false;
        while Instant::now() < deadline {
            for record in adapter.poll(&Cancellation::default(), 1, 8).unwrap() {
                if record.bytes() == [0, 0, 4, 0, 0, 0, 0, 0] {
                    assert_eq!(record.direction(), Direction::Rx);
                    assert!(matches!(
                        record.metadata(),
                        Metadata::Hid {
                            vendor_id: 0x0627,
                            product_id: 1,
                            usage_page: 1,
                            usage: 6,
                            report_id: 0
                        }
                    ));
                    assert_eq!(record.original_length(), None);
                    received = true;
                }
            }
            if received {
                break;
            }
        }
        assert!(received, "expected synthetic unnumbered keyboard report");
        let token = Cancellation::default();
        token.cancel();
        assert_eq!(adapter.poll(&token, 1, 8).err(), Some(Error::Cancelled));
        println!("EXMON_HID_REMOVE");
        std::io::stdout().flush().unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut removed = false;
        while Instant::now() < deadline {
            match adapter.poll(&Cancellation::default(), 1, 8) {
                Err(Error::Disconnected) => {
                    removed = true;
                    break;
                }
                Ok(_) => {}
                Err(other) => panic!("unexpected removal result: {other:?}"),
            }
        }
        assert!(
            removed,
            "expected emulated removal to revoke selected handle"
        );
        assert_eq!(adapter.transmit(&[0, 2]), Err(Error::InvalidState));
        adapter.close().unwrap();
        assert_eq!(adapter.transmit(&[0, 2]), Err(Error::InvalidState));
    }
    #[test]
    #[ignore = "requires unprivileged account in isolated EXMON QEMU USB keyboard fixture"]
    fn native_qemu_keyboard_permission_denied_is_explicit() {
        fixture();
        let uid = Command::new("id").arg("-u").output().unwrap();
        assert!(uid.status.success());
        assert_ne!(String::from_utf8(uid.stdout).unwrap().trim(), "0");
        let mut adapter = selected();
        assert_eq!(adapter.start(), Err(Error::PermissionDenied));
        assert_eq!(adapter.transmit(&[0, 2]), Err(Error::InvalidState));
        adapter.close().unwrap();
    }
}

#[derive(Default)]
struct Control {
    opens: Vec<usize>,
    writes: Vec<Vec<u8>>,
    selected: Option<usize>,
    input: Vec<u8>,
    denied: bool,
    removed: bool,
    partial: bool,
}
struct Mock(Arc<Mutex<Control>>);
fn info(id: u16) -> Device {
    Device {
        vendor_id: 0x1209,
        product_id: id,
        usage_page: 0xff00,
        usage: 1,
    }
}
impl Source for Mock {
    fn enumerate(&mut self, _: usize) -> Result<Vec<Device>, Error> {
        Ok(vec![info(1), info(2)])
    }
    fn open_selected(&mut self, id: usize) -> Result<(), Error> {
        let mut c = self.0.lock().unwrap();
        if c.denied {
            return Err(Error::PermissionDenied);
        }
        c.opens.push(id);
        c.selected = Some(id);
        Ok(())
    }
    fn read(&mut self, bytes: &mut [u8], timeout: i32) -> Result<usize, Error> {
        assert_eq!(timeout, 10);
        let mut c = self.0.lock().unwrap();
        if c.removed {
            return Err(Error::Disconnected);
        }
        assert!(c.selected.is_some());
        let length = c.input.len();
        if length > bytes.len() {
            return Ok(length);
        }
        bytes[..length].copy_from_slice(&c.input);
        c.input.clear();
        Ok(length)
    }
    fn write(&mut self, bytes: &[u8]) -> Result<usize, Error> {
        let mut c = self.0.lock().unwrap();
        if c.removed {
            return Err(Error::Disconnected);
        }
        assert!(c.selected.is_some());
        assert!(c.writes.len() < 16);
        c.writes.push(bytes.to_vec());
        Ok(if c.partial { 1 } else { bytes.len() })
    }
    fn close(&mut self) {
        self.0.lock().unwrap().selected = None;
    }
}
fn setup(mode: ReportMode) -> (Hid<Mock>, Arc<Mutex<Control>>) {
    let c = Arc::new(Mutex::new(Control::default()));
    (Hid::new(Mock(c.clone()), mode), c)
}
fn start(adapter: &mut Hid<Mock>, id: u64) {
    adapter.enumerate(64).unwrap();
    adapter.open(id).unwrap();
    adapter.configure(64).unwrap();
    adapter.start().unwrap();
}

#[test]
fn report_mode_must_be_fixed_before_selecting_a_collection() {
    let (mut adapter, c) = setup(ReportMode::Numbered);
    adapter.set_report_mode(ReportMode::Unnumbered).unwrap();
    adapter.enumerate(2).unwrap();
    adapter.open(1).unwrap();
    assert_eq!(
        adapter.set_report_mode(ReportMode::Numbered),
        Err(Error::InvalidState)
    );
    adapter.configure(64).unwrap();
    adapter.start().unwrap();
    c.lock().unwrap().input = vec![165, 0, 42];
    let report = adapter.poll(&Cancellation::default(), 1, 64).unwrap();
    assert_eq!(report[0].bytes(), [165, 0, 42]);
    assert!(matches!(
        report[0].metadata(),
        Metadata::Hid { report_id: 0, .. }
    ));
    adapter.close().unwrap();
    adapter.set_report_mode(ReportMode::Numbered).unwrap();
    assert!(c.lock().unwrap().writes.is_empty());
}

#[test]
fn selection_and_numbered_report_fidelity_are_explicit() {
    let (mut adapter, c) = setup(ReportMode::Numbered);
    assert_eq!(adapter.start(), Err(Error::InvalidState));
    adapter.enumerate(64).unwrap();
    assert!(c.lock().unwrap().opens.is_empty());
    assert_eq!(adapter.open(99), Err(Error::InvalidConfig));
    adapter.open(1).unwrap();
    adapter.configure(64).unwrap();
    assert!(c.lock().unwrap().opens.is_empty());
    assert_eq!(adapter.transmit(&[3, 9]), Err(Error::InvalidState));
    adapter.start().unwrap();
    assert_eq!(c.lock().unwrap().opens, [1]);
    assert!(c.lock().unwrap().writes.is_empty());
    c.lock().unwrap().input = vec![3, 165, 0, 42];
    let batch = adapter.poll(&Cancellation::default(), 1, 64).unwrap();
    assert_eq!(batch[0].bytes(), [3, 165, 0, 42]);
    assert_eq!(batch[0].direction(), Direction::Rx);
    assert_eq!(
        batch[0].metadata(),
        &Metadata::Hid {
            vendor_id: 0x1209,
            product_id: 2,
            usage_page: 0xff00,
            usage: 1,
            report_id: 3
        }
    );
    assert_eq!(adapter.transmit(&[3, 4, 5]), Ok(3));
    assert_eq!(c.lock().unwrap().writes, [vec![3, 4, 5]]);
    assert!(adapter.transmit(&[0, 4]).is_err());
    assert!(adapter.transmit(&[1; 65]).is_err());
    assert!(adapter.enumerate(64).is_err());
    adapter.close().unwrap();
    assert!(c.lock().unwrap().selected.is_none());
}

#[test]
fn unnumbered_input_is_not_stripped_and_output_zero_id_is_not_duplicated() {
    let (mut adapter, c) = setup(ReportMode::Unnumbered);
    start(&mut adapter, 0);
    c.lock().unwrap().input = vec![165, 0, 42];
    let batch = adapter.poll(&Cancellation::default(), 2, 64).unwrap();
    assert_eq!(batch[0].bytes(), [165, 0, 42]);
    assert!(matches!(
        batch[0].metadata(),
        Metadata::Hid { report_id: 0, .. }
    ));
    assert_eq!(adapter.transmit(&[0, 9, 8]), Ok(3));
    assert_eq!(c.lock().unwrap().writes, [vec![0, 9, 8]]);
    assert!(adapter.transmit(&[9, 8]).is_err());
    c.lock().unwrap().partial = true;
    assert_eq!(adapter.transmit(&[0, 7, 6]), Ok(1));
    assert_eq!(c.lock().unwrap().writes.len(), 2);
}

#[test]
fn removal_permission_cancellation_and_size_errors_do_not_open_other_devices() {
    let (mut adapter, c) = setup(ReportMode::Numbered);
    adapter.enumerate(64).unwrap();
    adapter.open(0).unwrap();
    adapter.configure(64).unwrap();
    c.lock().unwrap().denied = true;
    assert_eq!(adapter.start(), Err(Error::PermissionDenied));
    assert!(c.lock().unwrap().opens.is_empty());
    c.lock().unwrap().denied = false;
    adapter.start().unwrap();
    let token = Cancellation::default();
    token.cancel();
    assert_eq!(adapter.poll(&token, 1, 64).err(), Some(Error::Cancelled));
    c.lock().unwrap().removed = true;
    assert_eq!(
        adapter.poll(&Cancellation::default(), 1, 64).err(),
        Some(Error::Disconnected)
    );
    assert_eq!(adapter.transmit(&[1]), Err(Error::InvalidState));
    assert!(c.lock().unwrap().selected.is_none());
    adapter.close().unwrap();
    c.lock().unwrap().removed = false;
    start(&mut adapter, 0);
    c.lock().unwrap().input = vec![1; 65];
    assert_eq!(
        adapter.poll(&Cancellation::default(), 1, 64).err(),
        Some(Error::Backend)
    );
    assert_eq!(c.lock().unwrap().opens, [0, 0]);
    assert!(c.lock().unwrap().writes.is_empty());
}
