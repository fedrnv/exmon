use exmon_capture::{adapter::Error, usb_transfers::*};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
struct Control {
    claims: usize,
    releases: usize,
    submitted: [bool; 2],
    done: [Option<Completion>; 2],
    busy: bool,
    kind: Kind,
}
impl Default for Control {
    fn default() -> Self {
        Self {
            claims: 0,
            releases: 0,
            submitted: [false; 2],
            done: [None, None],
            busy: false,
            kind: Kind::Bulk,
        }
    }
}
struct Mock(Arc<Mutex<Control>>);
fn descriptors(kind: Kind) -> Descriptors {
    Descriptors {
        active: 1,
        configurations: vec![Configuration {
            value: 1,
            interfaces: vec![Interface {
                number: 2,
                alternate: 0,
                endpoints: vec![
                    Endpoint {
                        address: 0x81,
                        kind,
                        packet_size: 8,
                    },
                    Endpoint {
                        address: 2,
                        kind,
                        packet_size: 8,
                    },
                ],
            }],
        }],
    }
}
fn selection() -> Selection {
    Selection {
        configuration: 1,
        interface: 2,
        alternate: 0,
        input: Some(0x81),
        output: Some(2),
    }
}
impl Source for Mock {
    fn enumerate(&mut self, _: usize) -> Result<Vec<Device>, Error> {
        Ok(vec![Device {
            vendor_id: 0x1209,
            product_id: 1,
        }])
    }
    fn inspect(&mut self, index: usize) -> Result<Descriptors, Error> {
        assert_eq!(index, 0);
        Ok(descriptors(self.0.lock().unwrap().kind))
    }
    fn claim(&mut self, _: Selection) -> Result<(), Error> {
        let mut c = self.0.lock().unwrap();
        if c.busy {
            return Err(Error::Busy);
        }
        c.claims += 1;
        Ok(())
    }
    fn submit(
        &mut self,
        dir: Direction,
        endpoint: Endpoint,
        bytes: &[u8],
        length: usize,
    ) -> Result<(), Error> {
        let mut c = self.0.lock().unwrap();
        assert_eq!(endpoint.kind, c.kind);
        assert!(!c.submitted[dir.index()]);
        assert!(length <= 65536);
        if dir == Direction::Out {
            assert_eq!(bytes, [1, 2, 3]);
        }
        c.submitted[dir.index()] = true;
        Ok(())
    }
    fn poll(&mut self, dir: Direction) -> Result<Option<Completion>, Error> {
        let mut c = self.0.lock().unwrap();
        let done = c.done[dir.index()].take();
        if done.is_some() {
            c.submitted[dir.index()] = false;
        }
        Ok(done)
    }
    fn release(&mut self) {
        let mut c = self.0.lock().unwrap();
        c.submitted = [false; 2];
        c.releases += 1;
    }
}
fn setup() -> (Usb<Mock>, Arc<Mutex<Control>>) {
    let c = Arc::new(Mutex::new(Control::default()));
    (
        Usb::new(Mock(c.clone()), Duration::from_secs(1)).unwrap(),
        c,
    )
}
fn start(usb: &mut Usb<Mock>) {
    usb.enumerate().unwrap();
    usb.inspect(0).unwrap();
    usb.claim(selection()).unwrap();
}
fn completion(dir: Direction, status: Status, bytes: Vec<u8>) -> Completion {
    Completion {
        direction: dir,
        endpoint: if dir == Direction::In { 0x81 } else { 2 },
        actual_length: Some(bytes.len()),
        bytes,
        status,
    }
}

#[test]
fn asynchronous_in_and_explicit_out_have_separate_bounded_completions() {
    let (mut usb, c) = setup();
    usb.enumerate().unwrap();
    assert_eq!(c.lock().unwrap().claims, 0);
    usb.inspect(0).unwrap();
    assert_eq!(c.lock().unwrap().claims, 0);
    assert!(usb.submit_in(8).is_err());
    usb.claim(selection()).unwrap();
    assert_eq!(c.lock().unwrap().submitted, [false; 2]);
    assert!(usb.submit_in(7).is_err());
    usb.submit_in(16).unwrap();
    assert!(usb.submit_in(8).is_err());
    assert!(usb.poll(Direction::In).unwrap().is_none());
    usb.submit_out(&[1, 2, 3]).unwrap();
    assert!(usb.submit_out(&[1, 2, 3]).is_err());
    c.lock().unwrap().done[0] = Some(completion(Direction::In, Status::Success, vec![165, 0, 42]));
    let done = usb.poll(Direction::In).unwrap().unwrap();
    assert_eq!(done.bytes, [165, 0, 42]);
    assert_eq!(done.actual_length, Some(3));
    c.lock().unwrap().done[1] = Some(completion(Direction::Out, Status::Success, vec![1, 2]));
    let done = usb.poll(Direction::Out).unwrap().unwrap();
    assert_eq!(done.bytes, [1, 2]);
    assert_eq!(done.actual_length, Some(2));
}

#[test]
fn interrupt_completion_preserves_bytes_and_endpoint_kind() {
    let (mut usb, c) = setup();
    c.lock().unwrap().kind = Kind::Interrupt;
    start(&mut usb);
    usb.submit_in(8).unwrap();
    c.lock().unwrap().done[0] = Some(completion(Direction::In, Status::Success, vec![4, 5]));
    let done = usb.poll(Direction::In).unwrap().unwrap();
    assert_eq!(done.bytes, [4, 5]);
    usb.submit_out(&[1, 2, 3]).unwrap();
    usb.stop();
    assert_eq!(
        usb.poll(Direction::Out).unwrap().unwrap().actual_length,
        None
    );
}

#[test]
fn descriptor_fixtures_validate_bulk_interrupt_and_bad_selection() {
    for kind in [Kind::Bulk, Kind::Interrupt] {
        assert!(descriptors(kind).validate().is_ok());
    }
    let mut d = descriptors(Kind::Bulk);
    d.configurations[0].interfaces[0].endpoints[0].address = 0;
    assert!(d.validate().is_err());
    d = descriptors(Kind::Interrupt);
    d.configurations.push(d.configurations[0].clone());
    assert!(d.validate().is_err());
    let (mut usb, _) = setup();
    usb.enumerate().unwrap();
    assert!(usb.inspect(1).is_err());
    usb.inspect(0).unwrap();
    assert_eq!(
        usb.claim(Selection {
            configuration: 2,
            ..selection()
        }),
        Err(Error::Unsupported)
    );
    assert_eq!(
        usb.claim(Selection {
            input: Some(2),
            ..selection()
        }),
        Err(Error::InvalidConfig)
    );
    assert_eq!(
        usb.claim(Selection {
            interface: 3,
            ..selection()
        }),
        Err(Error::InvalidConfig)
    );
    let (mut unsupported, control) = setup();
    control.lock().unwrap().kind = Kind::Isochronous;
    unsupported.enumerate().unwrap();
    unsupported.inspect(0).unwrap();
    assert_eq!(unsupported.claim(selection()), Err(Error::Unsupported));
}

#[test]
fn stall_disconnect_and_cancel_keep_partial_or_unknown_results_honest() {
    for status in [
        Status::Stall,
        Status::Disconnected,
        Status::Cancelled,
        Status::Fault,
    ] {
        let (mut usb, c) = setup();
        start(&mut usb);
        usb.submit_in(8).unwrap();
        usb.submit_out(&[1, 2, 3]).unwrap();
        c.lock().unwrap().done[0] = Some(completion(Direction::In, status, vec![9]));
        let done = usb.poll(Direction::In).unwrap().unwrap();
        assert_eq!(done.status, status);
        assert_eq!(done.bytes, [9]);
        let other = usb.poll(Direction::Out).unwrap().unwrap();
        assert_eq!(other.status, Status::Cancelled);
        assert_eq!(other.actual_length, None);
        assert!(usb.submit_in(8).is_err());
        assert!(c.lock().unwrap().releases > 0);
    }
    let (mut usb, c) = setup();
    usb.enumerate().unwrap();
    usb.inspect(0).unwrap();
    c.lock().unwrap().busy = true;
    assert_eq!(usb.claim(selection()), Err(Error::Busy));
    assert_eq!(c.lock().unwrap().claims, 0);
}
