use exmon_capture::{adapter::*, hid::*};
use exmon_domain::{Direction, observation::Metadata};
use std::sync::{Arc, Mutex};

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
