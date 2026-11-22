use exmon_capture::{adapter::*, ethernet::*};
use exmon_domain::{Direction, observation::Metadata};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Control {
    active: bool,
    activates: usize,
    reads: usize,
    snapshot: usize,
    promisc: bool,
    filter: String,
    link: u32,
    denied: bool,
}
struct Fixture(Arc<Mutex<Control>>);
fn frame() -> Vec<u8> {
    // Synthetic locally administered addresses, experimental EtherType 0x88B5.
    vec![2, 0, 0, 0, 0, 1, 2, 0, 0, 0, 0, 2, 0x88, 0xb5, 0xa5, 0, 42]
}
impl Source for Fixture {
    fn list(&mut self, maximum: usize) -> Result<Vec<String>, Error> {
        Ok(vec!["synthetic-ethernet".into()]
            .into_iter()
            .take(maximum)
            .collect())
    }
    fn activate(&mut self, name: &str, snapshot: usize, options: &Options) -> Result<u32, Error> {
        assert_eq!(name, "synthetic-ethernet");
        let mut c = self.0.lock().unwrap();
        if c.denied {
            return Err(Error::PermissionDenied);
        }
        c.active = true;
        c.activates += 1;
        c.snapshot = snapshot;
        c.promisc = options.promiscuous;
        c.filter = options.filter.clone();
        Ok(c.link)
    }
    fn next(&mut self) -> Result<Option<RawFrame>, Error> {
        let mut c = self.0.lock().unwrap();
        assert!(c.active);
        c.reads += 1;
        let full = frame();
        let bytes = full[..full.len().min(c.snapshot)].to_vec();
        Ok(Some(RawFrame {
            timestamp_ns: 123_000 + c.reads as u64,
            original_length: full.len(),
            bytes,
        }))
    }
    fn close(&mut self) {
        self.0.lock().unwrap().active = false;
    }
}
fn setup(snapshot: usize) -> (Ethernet<Fixture>, Arc<Mutex<Control>>) {
    let control = Arc::new(Mutex::new(Control {
        link: 1,
        ..Control::default()
    }));
    let mut adapter = Ethernet::new(
        Fixture(control.clone()),
        Options {
            promiscuous: true,
            timeout_ms: 20,
            filter: "ether proto 0x88b5".into(),
        },
    )
    .unwrap();
    let descriptors = adapter.enumerate(64).unwrap();
    assert_eq!(descriptors.len(), 1);
    assert_eq!(descriptors[0].tx, Availability::Unsupported);
    assert_eq!(control.lock().unwrap().activates, 0);
    adapter.open(0).unwrap();
    adapter.configure(snapshot).unwrap();
    assert_eq!(control.lock().unwrap().activates, 0);
    (adapter, control)
}

#[test]
fn exact_bytes_capture_time_truncation_and_options() {
    for snapshot in [12, 17] {
        let (mut adapter, control) = setup(snapshot);
        adapter.start().unwrap();
        let batch = adapter.poll(&Cancellation::default(), 4, 64).unwrap();
        assert_eq!(batch.len(), (64 / snapshot).min(4));
        assert_eq!(batch[0].bytes(), &frame()[..snapshot]);
        assert_eq!(batch[0].original_length(), Some(17));
        assert_eq!(batch[0].source_timestamp_ns(), Some(123001));
        assert_eq!(batch[0].direction(), Direction::Unknown);
        assert_eq!(batch[0].metadata(), &Metadata::Ethernet { link_type: 1 });
        let c = control.lock().unwrap();
        assert!(c.promisc);
        assert_eq!(c.filter, "ether proto 0x88b5");
    }
}

#[test]
fn stop_under_synthetic_load_is_finite_and_does_not_read_after_cancel() {
    let (mut adapter, control) = setup(17);
    adapter.start().unwrap();
    let token = Cancellation::default();
    for _ in 0..100 {
        assert_eq!(adapter.poll(&token, 4, 68).unwrap().len(), 4);
    }
    token.cancel();
    let before = control.lock().unwrap().reads;
    assert_eq!(adapter.poll(&token, 4, 68).err(), Some(Error::Cancelled));
    assert_eq!(control.lock().unwrap().reads, before);
    adapter.stop().unwrap();
    assert!(!control.lock().unwrap().active);
    assert_eq!(adapter.poll(&token, 4, 68).err(), Some(Error::InvalidState));
}

#[test]
fn unsupported_links_and_permission_denial_remain_explicit() {
    let (mut adapter, control) = setup(17);
    control.lock().unwrap().link = 113;
    assert_eq!(adapter.start(), Err(Error::Unsupported));
    assert!(!control.lock().unwrap().active);
    control.lock().unwrap().denied = true;
    assert_eq!(adapter.start(), Err(Error::PermissionDenied));
    assert!(
        Ethernet::new(
            Fixture(control),
            Options {
                timeout_ms: 0,
                ..Options::default()
            }
        )
        .is_err()
    );
}
