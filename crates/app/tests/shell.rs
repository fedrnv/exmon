use exmon_app::shell::*;
use exmon_domain::observation::TransportKind;
use std::time::{Duration, Instant};
fn until(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while !condition() {
        assert!(Instant::now() < deadline, "worker deadline");
        std::thread::sleep(Duration::from_millis(2));
    }
}
#[test]
fn sustained_capture_is_off_thread_bounded_and_stop_survives_full_results() {
    let mut worker = CaptureWorker::spawn().unwrap();
    assert!(!worker.status().requested);
    worker.start(TransportKind::Serial, 1).unwrap();
    until(|| worker.status().packets >= 1024);
    assert_ne!(worker.worker_thread().unwrap(), std::thread::current().id());
    assert!(worker.status().display_decimation > 0);
    let batches = worker.drain(usize::MAX);
    assert!(batches.len() <= 8);
    assert!(
        batches.iter().all(|b| b.records.len() <= 32
            && b.records.iter().all(|r| r.observation.bytes().len() <= 64))
    );
    worker.stop();
    until(|| !worker.status().running);
    let packets = worker.status().packets;
    std::thread::sleep(Duration::from_millis(30));
    assert_eq!(worker.status().packets, packets);
    assert!(!worker.status().requested);
    let generation = worker.generation();
    worker.start(TransportKind::Can, 2).unwrap();
    until(|| worker.status().running);
    assert!(worker.generation() > generation);
    drop(worker);
    let idle = CaptureWorker::spawn().unwrap();
    assert!(!idle.status().requested);
    assert_eq!(idle.status().packets, 0);
}
#[test]
fn preferences_are_loaded_and_saved_on_distinct_worker_without_capture_state() {
    let path = std::env::temp_dir().join(format!(
        "exmon-worker-layout-{}-{}.json",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let worker = PreferencesWorker::spawn(Some(path.clone())).unwrap();
    let mut initial = None;
    until(|| {
        initial = worker.load_result();
        initial.is_some()
    });
    assert_eq!(initial, Some(Ok(None)));
    assert_ne!(worker.worker_thread().unwrap(), std::thread::current().id());
    let layout = Layout {
        theme: Theme::Light,
        split: 0.65,
        ..Layout::default()
    };
    worker.save(layout).unwrap();
    drop(worker);
    let restored = PreferencesWorker::spawn(Some(path.clone())).unwrap();
    let mut loaded = None;
    until(|| {
        loaded = restored.load_result();
        loaded.is_some()
    });
    assert_eq!(loaded, Some(Ok(Some(layout))));
    drop(restored);
    std::fs::remove_file(path).unwrap();
}
