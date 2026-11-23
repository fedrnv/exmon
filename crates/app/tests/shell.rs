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
#[test]
fn portable_safety_limits_reach_actual_capture_and_invalidation_fences_old_batches() {
    use exmon_app::network_filters::NetworkFilter;
    use exmon_app::settings::Safety;
    let mut w = CaptureWorker::spawn().unwrap();
    let limits = Safety {
        queue_items: 4,
        queue_bytes: 32,
        batch_items: 2,
        batch_bytes: 8,
        snapshot_bytes: 4,
    };
    w.start_bounded(TransportKind::Udp, 1, NetworkFilter::default(), limits)
        .unwrap();
    until(|| w.status().packets > 4);
    let old = w.generation();
    let batches = w.drain(4);
    assert!(batches.iter().all(
        |b| b.records.len() <= 2 && b.records.iter().all(|r| r.observation.bytes().len() == 4)
    ));
    let next = w.invalidate().unwrap();
    assert!(next > old);
    assert!(!w.status().requested);
    let mut invalid = limits;
    invalid.snapshot_bytes = 65;
    assert!(
        w.start_bounded(TransportKind::Udp, 1, NetworkFilter::default(), invalid)
            .is_err()
    );
    assert_eq!(w.generation(), next);
    assert!(!w.status().requested);
}
