use exmon_app::hid_worker::*;
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
#[derive(Default)]
struct Audit {
    opens: Vec<usize>,
    writes: Vec<(usize, Vec<u8>)>,
    selected: Option<usize>,
    removed: bool,
    denied: bool,
    enumeration_fails: bool,
}
struct Fixture(Arc<Mutex<Audit>>);
impl Source for Fixture {
    fn enumerate(&mut self, _: usize) -> Result<Vec<Device>, Error> {
        if self.0.lock().unwrap().enumeration_fails {
            return Err(Error::PermissionDenied);
        }
        Ok((1..=2)
            .map(|product_id| Device {
                vendor_id: 0x1209,
                product_id,
                usage_page: 0xff00,
                usage: 1,
            })
            .collect())
    }
    fn open_selected(&mut self, id: usize) -> Result<(), Error> {
        let mut a = self.0.lock().unwrap();
        if a.denied {
            return Err(Error::PermissionDenied);
        }
        assert!(id < 2);
        a.opens.push(id);
        a.selected = Some(id);
        Ok(())
    }
    fn read(&mut self, bytes: &mut [u8], _: i32) -> Result<usize, Error> {
        let a = self.0.lock().unwrap();
        if a.removed {
            return Err(Error::Disconnected);
        }
        assert!(a.selected.is_some());
        bytes[..4].copy_from_slice(&[3, 165, 0, 42]);
        Ok(4)
    }
    fn write(&mut self, bytes: &[u8]) -> Result<usize, Error> {
        let mut a = self.0.lock().unwrap();
        let id = a.selected.ok_or(Error::InvalidState)?;
        if a.removed {
            return Err(Error::Disconnected);
        }
        assert!(a.writes.len() < 64);
        a.writes.push((id, bytes.to_vec()));
        Ok(bytes.len())
    }
    fn close(&mut self) {
        self.0.lock().unwrap().selected = None;
    }
}
fn until(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while !condition() {
        assert!(Instant::now() < deadline, "HID worker deadline");
        std::thread::sleep(Duration::from_millis(2));
    }
}
fn setup() -> (HidWorker, Arc<Mutex<Audit>>) {
    let a = Arc::new(Mutex::new(Audit::default()));
    let source = a.clone();
    (HidWorker::spawn(move || Fixture(source)).unwrap(), a)
}
fn snapshot(w: &HidWorker) -> Snapshot {
    w.snapshot().unwrap_or_default()
}
#[test]
fn exact_selected_worker_reports_and_stale_generations_never_fall_back() {
    let (mut worker, a) = setup();
    worker.discover().unwrap();
    until(|| !worker.pending() && snapshot(&worker).collections.len() == 2);
    assert!(a.lock().unwrap().opens.is_empty());
    let enumeration = snapshot(&worker).enumeration;
    worker
        .open_selected(1, enumeration, ReportMode::Numbered, 64)
        .unwrap();
    until(|| snapshot(&worker).open);
    assert_ne!(worker.worker_thread().unwrap(), std::thread::current().id());
    assert_eq!(a.lock().unwrap().opens, [1]);
    assert!(a.lock().unwrap().writes.is_empty());
    let epoch = snapshot(&worker).epoch;
    worker.send_selected(0, epoch, "03 01".into()).unwrap();
    until(|| snapshot(&worker).error == Some(Error::InvalidState));
    assert!(a.lock().unwrap().writes.is_empty());
    for invalid in ["0 3", "03 z0", "03 💣", ""] {
        let failures = snapshot(&worker).failures;
        worker.send_selected(1, epoch, invalid.into()).unwrap();
        until(|| snapshot(&worker).failures > failures);
        assert_eq!(snapshot(&worker).error, Some(Error::InvalidConfig));
    }
    assert!(a.lock().unwrap().writes.is_empty());
    worker
        .send_selected(1, epoch, "03 A5 00 2A".into())
        .unwrap();
    until(|| snapshot(&worker).last_sent == Some(4));
    assert_eq!(a.lock().unwrap().writes, [(1, vec![3, 165, 0, 42])]);
    until(|| snapshot(&worker).dropped > 0);
    let reports = worker.drain(999);
    assert!(reports.len() <= 4);
    assert!(
        reports
            .iter()
            .all(|r| r.id == 1 && r.epoch == epoch && r.observation.bytes() == [3, 165, 0, 42])
    );
    worker.close();
    assert_eq!(
        worker.send_selected(1, epoch, "03 01".into()),
        Err(Error::Cancelled)
    );
    until(|| !snapshot(&worker).open);
    worker
        .open_selected(0, enumeration, ReportMode::Unnumbered, 64)
        .unwrap();
    until(|| snapshot(&worker).open);
    let next = snapshot(&worker).epoch;
    assert_ne!(epoch, next);
    assert_eq!(
        worker.send_selected(1, epoch, "03 01".into()),
        Err(Error::Cancelled)
    );
    worker.send_selected(0, next, "00 01".into()).unwrap();
    until(|| snapshot(&worker).last_sent == Some(2));
    assert_eq!(a.lock().unwrap().opens, [1, 0]);
    assert_eq!(a.lock().unwrap().writes[1], (0, vec![0, 1]));
    a.lock().unwrap().removed = true;
    until(|| !snapshot(&worker).open);
    assert_eq!(snapshot(&worker).error, Some(Error::Disconnected));
    assert_eq!(
        worker.send_selected(0, next, "00 02".into()),
        Err(Error::Cancelled)
    );
}
#[test]
fn refresh_failure_invalidates_old_selection_and_permission_denial_is_recoverable() {
    let (worker, a) = setup();
    worker.discover().unwrap();
    until(|| !worker.pending() && snapshot(&worker).collections.len() == 2);
    let old = snapshot(&worker).enumeration;
    a.lock().unwrap().enumeration_fails = true;
    worker.discover().unwrap();
    until(|| !worker.pending() && snapshot(&worker).enumeration > old);
    assert!(snapshot(&worker).collections.is_empty());
    worker
        .open_selected(0, old, ReportMode::Numbered, 64)
        .unwrap();
    until(|| !worker.pending());
    assert_eq!(snapshot(&worker).error, Some(Error::InvalidState));
    assert!(a.lock().unwrap().opens.is_empty());
    {
        let mut audit = a.lock().unwrap();
        audit.enumeration_fails = false;
        audit.denied = true;
    }
    worker.discover().unwrap();
    until(|| !worker.pending() && snapshot(&worker).collections.len() == 2);
    let fresh = snapshot(&worker).enumeration;
    worker
        .open_selected(1, fresh, ReportMode::Numbered, 64)
        .unwrap();
    until(|| !worker.pending());
    assert_eq!(snapshot(&worker).error, Some(Error::PermissionDenied));
    assert!(a.lock().unwrap().opens.is_empty());
    assert!(a.lock().unwrap().writes.is_empty());
}
