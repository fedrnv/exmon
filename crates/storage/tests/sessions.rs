use exmon_domain::{
    Direction,
    observation::{Metadata, Observation},
};
use exmon_storage::{config::load_project, sessions::*};
use std::{fs, path::PathBuf};
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "exmon031-storage-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn path(&self) -> PathBuf {
        self.0.join("session.sqlite")
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        assert_eq!(self.0.parent(), Some(std::env::temp_dir().as_path()));
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn source() -> Source {
    Source {
        generation: 9,
        interface: 2,
        epoch: 3,
        sequence: u64::MAX,
    }
}
fn fixture() -> Vec<Event> {
    let observation = Observation::new(
        u64::MAX,
        Direction::Rx,
        Metadata::Udp {
            local: "127.0.0.1:8000".parse().unwrap(),
            remote: "127.0.0.2:8001".parse().unwrap(),
        },
        Some(8),
        b"\0\x01\xFE\xFF",
    )
    .unwrap()
    .with_source_timestamp(u64::MAX);
    let config=load_project(br#"{"schema_version":1,"protocols":[{"id":"voltage","name":"Saved voltage","framing":{"kind":"datagram"},"classification":{},"fields":[{"id":"level","offset":0,"length":2,"representation":"unsigned","endian":"big","unit":"V"}]}]}"#).unwrap();
    vec![
        Event::Raw(Raw::new(source(), &observation)),
        Event::Definitions {
            generation: 9,
            revision: 7,
            config,
        },
        Event::Annotation {
            source: Some(source()),
            text: "Synthetic annotation\nline two".into(),
        },
        Event::Message {
            timestamp_ns: u64::MAX,
            source_timestamp_ns: Some(u64::MAX),
            source: source(),
            revision: 7,
            protocol: "voltage".into(),
            name: "Saved voltage".into(),
            group: "rule:voltage".into(),
            bytes: vec![1, 2],
            samples: vec![
                Sample {
                    field: "level".into(),
                    unit: "V".into(),
                    value: Some(25.5),
                },
                Sample {
                    field: "gap".into(),
                    unit: "V".into(),
                    value: None,
                },
            ],
            spans: vec![Span {
                observation: 4,
                offsets: 0..2,
                stream: 0..2,
            }],
        },
        Event::Charts {
            definitions: serde_json::json!([{"id":1,"name":"Voltage","samples":4096,"average":8,"series":[{"protocol":"voltage","field":"level","label":"Level","unit":"V"}]}]),
        },
    ]
}
#[test]
fn exact_committed_data_definitions_names_annotations_and_samples_recover() {
    let root = Temp::new();
    let path = root.path();
    let expected = fixture();
    let mut writer = Writer::create(&path, Limits::default()).unwrap();
    writer.append(&expected).unwrap();
    writer.finish(0).unwrap();
    let summary = writer.summary();
    drop(writer);
    let recovered = Recovery::open(&path).unwrap();
    assert_eq!(recovered.summary, summary);
    let page = recovered.page(0).unwrap();
    assert_eq!(page.len(), expected.len());
    for (i, (entry, expected)) in page.iter().zip(expected.iter()).enumerate() {
        assert_eq!(entry.id, i as u64 + 1);
        assert_eq!(entry.event.encode().unwrap(), expected.encode().unwrap());
    }
    let Event::Raw(raw) = page[0].event.as_ref() else {
        panic!("raw")
    };
    let o = raw.observation().unwrap();
    assert_eq!(o.bytes(), [0, 1, 254, 255]);
    assert_eq!(o.timestamp_ns(), u64::MAX);
    assert_eq!(o.source_timestamp_ns(), Some(u64::MAX));
    assert_eq!(raw.source.sequence, u64::MAX);
}
#[test]
fn invalid_batch_is_atomic_and_drop_without_finish_is_interrupted() {
    let root = Temp::new();
    let mut w = Writer::create(&root.path(), Limits::default()).unwrap();
    w.append(&fixture()).unwrap();
    let before = w.summary();
    let bad = Event::Raw(Raw {
        source: source(),
        timestamp_ns: 0,
        source_timestamp_ns: None,
        direction: Direction::Rx,
        metadata: Metadata::Usb {
            endpoint: 0x70,
            completed: true,
        },
        original: Some(1),
        bytes: vec![1, 2],
    });
    assert!(w.append(&[fixture().remove(0), bad]).is_err());
    assert_eq!(w.summary(), before);
    drop(w);
    let r = Recovery::open(&root.path()).unwrap();
    assert!(!r.summary.closed);
    assert_eq!(r.summary.records, 5);
    assert_eq!(r.page(0).unwrap().len(), 5);
}
#[test]
fn count_and_disk_limits_preserve_previous_commit_and_bound_file_size() {
    let root = Temp::new();
    let path = root.path();
    let limits = Limits {
        bytes: 1024 * 1024,
        records: 6,
    };
    let mut w = Writer::create(&path, limits).unwrap();
    w.append(&fixture()).unwrap();
    assert_eq!(w.append(&fixture()), Err(Error::Limit));
    assert_eq!(w.summary().records, 5);
    w.finish(12).unwrap();
    drop(w);
    let r = Recovery::open(&path).unwrap();
    assert_eq!(r.summary.dropped, 12);
    assert!(fs::metadata(path).unwrap().len() <= limits.bytes);
    drop(r);
    let path = root.0.join("byte-limit.sqlite");
    let mut w = Writer::create(
        &path,
        Limits {
            bytes: 1024 * 1024,
            records: 10000,
        },
    )
    .unwrap();
    let big = Event::Raw(Raw::new(
        source(),
        &Observation::new(
            0,
            Direction::Unknown,
            Metadata::Ethernet { link_type: 147 },
            None,
            &[255; 65536],
        )
        .unwrap(),
    ));
    let mut accepted = 0;
    loop {
        match w.append(std::slice::from_ref(&big)) {
            Ok(()) => accepted += 1,
            Err(Error::Limit) => break,
            Err(e) => panic!("unexpected {e:?}"),
        }
    }
    assert!(accepted > 0 && accepted < 8);
    assert_eq!(w.summary().records, accepted);
    w.finish(1).unwrap();
    drop(w);
    let r = Recovery::open(&path).unwrap();
    assert_eq!(r.summary.records, accepted);
    assert!(fs::metadata(path).unwrap().len() <= 1024 * 1024);
}
#[test]
fn bounded_recovery_pages_preserve_order_and_all_bytes() {
    let root = Temp::new();
    let mut w = Writer::create(&root.path(), Limits::default()).unwrap();
    for n in 0..300 {
        w.append(&[Event::Annotation {
            source: None,
            text: format!("entry {n}"),
        }])
        .unwrap();
    }
    w.finish(0).unwrap();
    drop(w);
    let r = Recovery::open(&root.path()).unwrap();
    let mut after = 0;
    let mut count = 0;
    loop {
        let page = r.page(after).unwrap();
        if page.is_empty() {
            break;
        }
        assert!(page.len() <= 64);
        let cost: usize = page.iter().map(|e| e.event.encode().unwrap().len()).sum();
        assert!(cost <= MAX_RECORD);
        for e in page {
            count += 1;
            assert_eq!(e.id, count);
            after = e.id;
        }
    }
    assert_eq!(count, 300);
    assert!(r.page(u64::MAX).is_err());
}
#[test]
fn new_file_only_schema_version_corruption_and_local_path_validation() {
    let root = Temp::new();
    let path = root.path();
    let w = Writer::create(&path, Limits::default()).unwrap();
    drop(w);
    let original = fs::read(&path).unwrap();
    assert!(matches!(
        Writer::create(&path, Limits::default()),
        Err(Error::Exists)
    ));
    assert_eq!(fs::read(&path).unwrap(), original);
    let mut future = original.clone();
    future[60..64].copy_from_slice(&2u32.to_be_bytes());
    fs::write(&path, future).unwrap();
    assert!(matches!(Recovery::open(&path), Err(Error::Version)));
    fs::write(&path, b"wrong").unwrap();
    assert!(Recovery::open(&path).is_err());
    fs::write(&path, original).unwrap();
    let c = rusqlite::Connection::open(&path).unwrap();
    c.execute_batch(
        "CREATE TRIGGER forbidden AFTER INSERT ON entries BEGIN DELETE FROM entries; END",
    )
    .unwrap();
    drop(c);
    assert!(matches!(Recovery::open(&path), Err(Error::Invalid)));
    for path in ["bad\npath", ""] {
        assert!(Writer::create(std::path::Path::new(path), Limits::default()).is_err());
    }
    #[cfg(windows)]
    for path in [
        r"\\server\share\db",
        "NUL",
        "file:stream",
        r"\\.\PhysicalDrive0",
    ] {
        assert!(Recovery::open(std::path::Path::new(path)).is_err());
    }
}
#[test]
fn unsafe_configuration_nonfinite_samples_and_unknown_fields_are_rejected() {
    assert!(
        Event::decode(br#"{"kind":"annotation","source":null,"text":"ok","unknown":1}"#).is_err()
    );
    assert!(Event::decode(&vec![0; MAX_RECORD + 1]).is_err());
    let mut events = fixture();
    let Event::Message { samples, .. } = &mut events[3] else {
        panic!()
    };
    samples[0].value = Some(f64::INFINITY);
    assert!(events[3].encode().is_err());
    let config = exmon_domain::config::ProjectConfig {
        schema_version: 99,
        ..Default::default()
    };
    assert!(
        Event::Definitions {
            generation: 0,
            revision: 0,
            config
        }
        .encode()
        .is_err()
    );
    assert!(
        Event::Charts {
            definitions: serde_json::json!([{"unexpected":"run"}])
        }
        .encode()
        .is_err()
    );
}
#[test]
fn recorded_loss_keeps_known_source_scope_and_no_identity_is_logged() {
    let root = Temp::new();
    let mut w = Writer::create(&root.path(), Limits::default()).unwrap();
    w.append(&fixture()).unwrap();
    w.finish_with_loss(7, Some(source())).unwrap();
    drop(w);
    let r = Recovery::open(&root.path()).unwrap();
    assert_eq!(r.summary.last_loss, Some(source()));
    assert_eq!(r.summary.dropped, 7);
    let Event::Raw(raw) = &fixture()[0] else {
        panic!()
    };
    assert!(!format!("{:?}", raw.observation().unwrap()).contains("127.0.0.1"));
}
#[test]
fn active_writer_is_exclusive_and_recovery_wait_is_bounded() {
    let root = Temp::new();
    let mut w = Writer::create(&root.path(), Limits::default()).unwrap();
    w.append(&fixture()).unwrap();
    let start = std::time::Instant::now();
    assert!(matches!(Recovery::open(&root.path()), Err(Error::Busy)));
    assert!(start.elapsed() < std::time::Duration::from_secs(2));
    drop(w);
    assert_eq!(Recovery::open(&root.path()).unwrap().summary.records, 5);
}
#[test]
fn all_transport_metadata_roundtrip_through_checked_observation_boundary() {
    let trace:Metadata=serde_json::from_value(serde_json::json!({"UsbMon":{"tag":1,"event":67,"transfer":66,"bus":1,"device":2,"endpoint":129,"iso_count":0,"iso":[null,null,null,null,null],"reported_length":2,"data_tag":61}})).unwrap();
    let metadata = [
        Metadata::Ethernet { link_type: 147 },
        Metadata::Tcp {
            local: "127.0.0.1:123".parse().unwrap(),
            remote: "127.0.0.2:456".parse().unwrap(),
            connection: u64::MAX,
        },
        Metadata::Udp {
            local: "[::1]:123".parse().unwrap(),
            remote: "[::1]:456".parse().unwrap(),
        },
        Metadata::Serial {
            configuration_revision: 9,
        },
        Metadata::Usb {
            endpoint: 129,
            completed: false,
        },
        Metadata::Hid {
            vendor_id: 0x1234,
            product_id: 1,
            usage_page: 0xff00,
            usage: 1,
            report_id: 2,
        },
        Metadata::Can {
            channel: 1,
            id: 0x123,
            extended: false,
            fd: false,
            remote: false,
            error: false,
            dlc: 8,
        },
        trace,
    ];
    for metadata in metadata {
        let observation =
            Observation::new(7, Direction::Unknown, metadata.clone(), Some(3), b"abc").unwrap();
        let event = Event::Raw(Raw::new(source(), &observation));
        let encoded = event.encode().unwrap();
        let decoded = Event::decode(&encoded).unwrap();
        let Event::Raw(raw) = decoded else { panic!() };
        let restored = raw.observation().unwrap();
        assert_eq!(
            serde_json::to_value(restored.metadata()).unwrap(),
            serde_json::to_value(metadata).unwrap()
        );
        assert_eq!(restored.bytes(), b"abc");
    }
}
#[test]
fn excess_schema_objects_are_rejected_before_unbounded_catalog_collection() {
    let root = Temp::new();
    let writer = Writer::create(&root.path(), Limits::default()).unwrap();
    drop(writer);
    let c = rusqlite::Connection::open(root.path()).unwrap();
    for n in 0..16 {
        c.execute_batch(&format!("CREATE TABLE extra_{n}(data BLOB)"))
            .unwrap();
    }
    drop(c);
    assert!(Recovery::open(&root.path()).is_err());
}
#[test]
fn super_journal_references_are_rejected_without_changing_any_file() {
    let root = Temp::new();
    let path = root.path();
    let mut writer = Writer::create(&path, Limits::default()).unwrap();
    writer.append(&fixture()).unwrap();
    writer.finish(0).unwrap();
    drop(writer);
    let original = fs::read(&path).unwrap();
    let external = root.0.join("unrelated-mj0123459ab");
    fs::write(&external, b"untouched").unwrap();
    let name = external.to_str().unwrap().as_bytes();
    let magic = [0xd9, 0xd5, 0x05, 0xf9, 0x20, 0xa1, 0x63, 0xd7];
    let mut journal = vec![0; 512];
    journal[..8].copy_from_slice(&magic);
    journal.extend(name);
    journal.extend((name.len() as u32).to_be_bytes());
    journal.extend(
        name.iter()
            .map(|b| u32::from(*b))
            .sum::<u32>()
            .to_be_bytes(),
    );
    journal.extend(magic);
    let sidecar = PathBuf::from(format!("{}-journal", path.display()));
    fs::write(&sidecar, &journal).unwrap();
    assert!(matches!(Recovery::open(&path), Err(Error::Invalid)));
    assert_eq!(fs::read(&path).unwrap(), original);
    assert_eq!(fs::read(&external).unwrap(), b"untouched");
    assert_eq!(fs::read(&sidecar).unwrap(), journal);
}
#[test]
fn restart_after_abrupt_exit_recovers_only_committed_data_deterministically() {
    let root = Temp::new();
    let path = root.path();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", "abrupt_child"])
        .env("EXMON031_CRASH_FILE", &path)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(23));
    let journal = PathBuf::from(format!("{}-journal", path.display()));
    assert!(
        journal.exists(),
        "test must actually leave a hot rollback journal"
    );
    let expected = fixture();
    let first = {
        let r = Recovery::open(&path).unwrap();
        assert!(!r.summary.closed);
        assert_eq!(r.summary.records, 5);
        r.page(0)
            .unwrap()
            .iter()
            .map(|e| e.event.encode().unwrap())
            .collect::<Vec<_>>()
    };
    let second = {
        let r = Recovery::open(&path).unwrap();
        r.page(0)
            .unwrap()
            .iter()
            .map(|e| e.event.encode().unwrap())
            .collect::<Vec<_>>()
    };
    assert_eq!(first, second);
    assert_eq!(
        first,
        expected
            .iter()
            .map(|e| e.encode().unwrap())
            .collect::<Vec<_>>()
    );
}
#[test]
#[ignore = "subprocess-only abrupt transaction fixture"]
fn abrupt_child() {
    let Some(path) = std::env::var_os("EXMON031_CRASH_FILE") else {
        panic!("subprocess path required")
    };
    let path = PathBuf::from(path);
    let mut writer = Writer::create(&path, Limits::default()).unwrap();
    writer.append(&fixture()).unwrap();
    drop(writer);
    let c = rusqlite::Connection::open(path).unwrap();
    c.execute_batch("PRAGMA cache_size=8; BEGIN IMMEDIATE")
        .unwrap();
    let data = Event::Annotation {
        source: None,
        text: "uncommitted".repeat(250),
    }
    .encode()
    .unwrap();
    for id in 6..100 {
        c.execute(
            "INSERT INTO entries VALUES(?1,?2)",
            rusqlite::params![id, &data],
        )
        .unwrap();
    }
    std::process::exit(23);
}
