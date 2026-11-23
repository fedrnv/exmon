use exmon_app::{incoming::Worker as Incoming, sessions::*, shell::CaptureWorker};
use exmon_domain::{
    Direction,
    config::ProjectConfig,
    observation::{Metadata, Observation, TransportKind},
};
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "exmon031-app-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        Self(root)
    }
    fn path(&self) -> PathBuf {
        self.0.join("session.sqlite")
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        assert_eq!(self.0.parent(), Some(std::env::temp_dir().as_path()));
        let end = Instant::now() + Duration::from_secs(3);
        loop {
            match std::fs::remove_dir_all(&self.0) {
                Ok(()) => break,
                Err(e) => {
                    assert!(Instant::now() < end, "owned session cleanup: {e}");
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
        }
    }
}
#[track_caller]
fn wait(controller: &Controller, mut f: impl FnMut(&Snapshot) -> bool) -> Arc<Snapshot> {
    let end = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(s) = controller.snapshot()
            && f(&s)
        {
            return s;
        }
        assert!(
            Instant::now() < end,
            "session wait {:?}",
            controller.snapshot().map(|s| (s.state, s.notice))
        );
        std::thread::sleep(Duration::from_millis(2));
    }
}
fn all(path: &std::path::Path) -> Vec<Entry> {
    let r = Recovery::open(path).unwrap();
    let mut result = Vec::new();
    let mut after = 0;
    loop {
        let p = r.page(after).unwrap();
        if p.is_empty() {
            break;
        }
        after = p.last().unwrap().id;
        result.extend(p);
    }
    result
}
fn raw(sequence: u64) -> exmon_app::shell::Captured {
    exmon_app::shell::Captured {
        interface_id: 3,
        epoch: 2,
        sequence,
        observation: Observation::new(
            sequence,
            Direction::Rx,
            Metadata::Serial {
                configuration_revision: 1,
            },
            Some(4),
            &[0, 1, 2, 255],
        )
        .unwrap(),
    }
}
#[test]
fn explicit_start_stop_and_recovery_preserve_records_without_resuming() {
    let root = Temp::new();
    let c = Controller::new().unwrap();
    assert!(!c.requested());
    assert!(!c.client().raw(1, &[raw(1)]));
    assert!(!root.path().exists());
    c.start(
        root.path().to_str().unwrap(),
        Limits::default(),
        Arc::new(ProjectConfig::default()),
        vec![],
    )
    .unwrap();
    wait(&c, |s| s.state == State::Recording);
    assert!(c.client().raw(8, &[raw(1)]));
    c.client()
        .note(
            Some(Source {
                generation: 8,
                interface: 3,
                epoch: 2,
                sequence: 1,
            }),
            "Recorded note",
        )
        .unwrap();
    wait(&c, |s| s.summary.as_ref().is_some_and(|s| s.records >= 4));
    c.stop();
    assert!(!c.requested());
    assert!(!c.client().raw(8, &[raw(2)]));
    wait(&c, |s| s.state == State::Stopped);
    let original = std::fs::read(root.path()).unwrap();
    c.recover(root.path().to_str().unwrap()).unwrap();
    let s = wait(&c, |s| s.state == State::Recovered);
    assert_eq!(s.summary.as_ref().unwrap().records, 4);
    assert_eq!(c.client().token(), 0);
    assert!(!c.requested());
    assert!(
        s.entries
            .iter()
            .any(|e| matches!(e.event.as_ref(),Event::Annotation{text,..}if text=="Recorded note"))
    );
    c.start(
        root.path().to_str().unwrap(),
        Limits::default(),
        Arc::new(ProjectConfig::default()),
        vec![],
    )
    .unwrap();
    wait(&c, |s| s.state == State::Failed);
    assert_eq!(std::fs::read(root.path()).unwrap(), original);
}
#[test]
fn raw_recording_continues_when_gui_capture_results_are_never_drained() {
    let root = Temp::new();
    let recorder = Controller::new().unwrap();
    let capture = CaptureWorker::spawn().unwrap();
    capture.record_to(recorder.client());
    recorder
        .start(
            root.path().to_str().unwrap(),
            Limits::default(),
            Arc::new(ProjectConfig::default()),
            vec![],
        )
        .unwrap();
    wait(&recorder, |s| s.state == State::Recording);
    capture.start(TransportKind::Ethernet, 1).unwrap();
    let end = Instant::now() + Duration::from_secs(3);
    loop {
        let record_count = recorder
            .snapshot()
            .and_then(|s| s.summary.as_ref().map(|s| s.records))
            .unwrap_or(0);
        // The initial definitions and chart specification are two non-raw records.
        if record_count > 1026 && capture.status().display_decimation > 0 {
            break;
        }
        assert!(Instant::now() < end);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(capture.status().running);
    recorder.stop();
    wait(&recorder, |s| {
        matches!(s.state, State::Stopped | State::Failed)
    });
    let before = capture.status().packets;
    std::thread::sleep(Duration::from_millis(25));
    assert!(capture.status().packets > before);
    capture.stop();
    let captured = capture.status().packets;
    let events = all(&root.path());
    let raw: Vec<_> = events
        .iter()
        .filter_map(|e| {
            if let Event::Raw(r) = e.event.as_ref() {
                Some(r)
            } else {
                None
            }
        })
        .collect();
    assert!(raw.len() > 512, "raw={}, all={}", raw.len(), events.len());
    assert_eq!(events.len(), raw.len() + 2);
    assert!(captured >= raw.len() as u64);
    assert!(
        raw.iter()
            .all(|r| r.source.interface == 1 && r.source.generation > 0)
    );
    assert!(
        raw.windows(2)
            .all(|r| r[0].source.sequence < r[1].source.sequence)
    );
}
#[test]
fn storage_budget_stops_only_recording_and_reports_reason() {
    let root = Temp::new();
    let c = Controller::new().unwrap();
    let capture = CaptureWorker::spawn().unwrap();
    capture.record_to(c.client());
    c.start(
        root.path().to_str().unwrap(),
        Limits {
            bytes: 1024 * 1024,
            records: 10,
        },
        Arc::new(ProjectConfig::default()),
        vec![],
    )
    .unwrap();
    wait(&c, |s| s.state == State::Recording);
    capture.start(TransportKind::Udp, 2).unwrap();
    let s = wait(&c, |s| {
        s.state == State::Failed && s.summary.as_ref().is_some_and(|s| s.closed)
    });
    assert!(s.notice.unwrap().contains("budget"));
    assert!(s.dropped > 0);
    assert!(!c.requested());
    let before = capture.status().packets;
    std::thread::sleep(Duration::from_millis(25));
    assert!(capture.status().packets > before);
    capture.stop();
    let r = Recovery::open(&root.path()).unwrap();
    assert!(r.summary.records <= 10);
    assert!(r.summary.dropped > 0);
}
#[test]
fn processing_definitions_renames_and_engineering_samples_are_recorded_before_display() {
    let root = Temp::new();
    let c = Controller::new().unwrap();
    let incoming = Incoming::prototype().unwrap();
    incoming.record_to(c.client());
    let end = Instant::now() + Duration::from_secs(3);
    let config = loop {
        if let Some(config) = incoming.configuration() {
            break config;
        }
        assert!(Instant::now() < end);
        std::thread::sleep(Duration::from_millis(2));
    };
    let specs = vec![exmon_app::charts::Spec {
        id: 1,
        name: "Recorded samples".into(),
        samples: 128,
        average: 8,
        series: vec![exmon_app::charts::Selection {
            protocol: "sample32".into(),
            field: "sequence".into(),
            label: "Sequence".into(),
            unit: String::new(),
        }],
    }];
    c.start(
        root.path().to_str().unwrap(),
        Limits::default(),
        config,
        specs,
    )
    .unwrap();
    wait(&c, |s| s.state == State::Recording);
    incoming.begin(9).unwrap();
    let end = Instant::now() + Duration::from_secs(3);
    while !incoming.status().is_some_and(|s| s.generation == 9) {
        assert!(Instant::now() < end);
        std::thread::sleep(Duration::from_millis(2));
    }
    let mut bytes = [0u8; 32];
    bytes[0] = 165;
    bytes[1] = 1;
    bytes[2..10].copy_from_slice(&42u64.to_le_bytes());
    incoming
        .submit(exmon_app::shell::Batch {
            generation: 9,
            records: vec![exmon_app::shell::Captured {
                interface_id: 1,
                epoch: 1,
                sequence: 0,
                observation: Observation::new(
                    100,
                    Direction::Rx,
                    Metadata::Serial {
                        configuration_revision: 1,
                    },
                    Some(32),
                    &bytes,
                )
                .unwrap(),
            }],
        })
        .unwrap();
    let end = Instant::now() + Duration::from_secs(3);
    while !incoming.status().is_some_and(|s| !s.groups.is_empty()) {
        assert!(Instant::now() < end);
        std::thread::sleep(Duration::from_millis(2));
    }
    incoming.rename("rule:sample32", "Stored name").unwrap();
    let end = Instant::now() + Duration::from_secs(3);
    while !incoming
        .status()
        .is_some_and(|s| s.groups.iter().any(|g| g.name == "Stored name"))
    {
        assert!(Instant::now() < end);
        std::thread::sleep(Duration::from_millis(2));
    }
    c.client().note(None, "Idle rename persisted").unwrap();
    wait(&c, |s| s.summary.as_ref().is_some_and(|s| s.records >= 5));
    c.stop();
    wait(&c, |s| s.state == State::Stopped);
    let entries = all(&root.path());
    assert!(entries.iter().any(|e|matches!(e.event.as_ref(),Event::Message{source,timestamp_ns,samples,..}if source.generation==9&&*timestamp_ns==100&&samples.iter().any(|s|s.field=="sequence"&&s.value==Some(42.0)))));
    assert!(entries.iter().any(|e|matches!(e.event.as_ref(),Event::Definitions{config,..}if config.protocols.iter().any(|p|p.name=="Stored name"))));
    assert!(entries.iter().any(|e|matches!(e.event.as_ref(),Event::Charts{definitions}if definitions[0]["name"]=="Recorded samples")));
}
#[test]
fn malformed_recovery_preserves_previous_committed_view_and_paging_is_bounded() {
    let root = Temp::new();
    let mut w = exmon_storage::sessions::Writer::create(&root.path(), Limits::default()).unwrap();
    for i in 0..150 {
        w.append(&[Event::Annotation {
            source: None,
            text: format!("record {i}"),
        }])
        .unwrap();
    }
    w.finish(0).unwrap();
    drop(w);
    let c = Controller::new().unwrap();
    c.recover(root.path().to_str().unwrap()).unwrap();
    let s = wait(&c, |s| s.state == State::Recovered);
    assert_eq!(s.entries.len(), 64);
    assert!(s.more);
    c.page(64).unwrap();
    let s = wait(&c, |s| s.after == 64);
    assert_eq!(s.entries[0].id, 65);
    let invalid = root.0.join("invalid.sqlite");
    std::fs::write(&invalid, b"invalid").unwrap();
    let rev = s.revision;
    c.recover(invalid.to_str().unwrap()).unwrap();
    let s = wait(&c, |s| s.revision > rev);
    assert_eq!(s.state, State::Recovered);
    assert_eq!(s.after, 64);
    assert_eq!(s.entries[0].id, 65);
    assert_eq!(c.client().token(), 0);
}
