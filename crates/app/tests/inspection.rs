use exmon_app::{
    incoming::{Key, PreparedBatch, Row},
    inspection::*,
};
use exmon_domain::{
    Direction,
    observation::{Metadata, Observation},
};
use exmon_storage::pcap::{Encoder, Encoding};
use std::{
    io::Cursor,
    ops::Range,
    sync::Arc,
    time::{Duration, Instant},
};
fn obs(bytes: &[u8], link_type: u32) -> Observation {
    Observation::new(
        0,
        Direction::Rx,
        Metadata::Ethernet { link_type },
        Some(bytes.len()),
        bytes,
    )
    .unwrap()
}
fn frame(bytes: &[u8]) -> Frame {
    Frame::prepare(Arc::new(obs(bytes, 1)))
}
fn range(f: &Frame, label: &str) -> Range<usize> {
    f.nodes
        .iter()
        .find(|n| n.label == label)
        .unwrap_or_else(|| panic!("Missing {label}"))
        .range
        .clone()
        .unwrap()
}
fn encoded(observation: &Observation, encoding: Encoding) -> Vec<u8> {
    let observation = Observation::new(
        0,
        Direction::Unknown,
        observation.metadata().clone(),
        observation.original_length(),
        observation.bytes(),
    )
    .unwrap()
    .with_source_timestamp(observation.source_timestamp_ns().unwrap_or(0));
    let mut e = Encoder::new(Vec::new(), encoding).unwrap();
    e.write_observation(0, &observation).unwrap();
    e.finish().unwrap()
}
fn batch(generation: u64, observation: Observation) -> PreparedBatch {
    PreparedBatch {
        generation,
        rows: vec![Arc::new(Row {
            quality: None,
            key: Key { generation, id: 1 },
            interface: 1,
            epoch: 1,
            sequence: 1,
            observation: Arc::new(observation),
            normalized: None,
            group: None,
            name: "raw".into(),
            revision: 0,
            protocol: None,
            fields: vec![],
            status: "Raw",
            source: String::new(),
            destination: String::new(),
            sources: vec![],
        })],
    }
}
fn wait(worker: &Worker, mut f: impl FnMut(&Snapshot) -> bool) -> Arc<Snapshot> {
    let end = Instant::now() + Duration::from_secs(4);
    loop {
        if let Some(s) = worker.snapshot()
            && f(&s)
        {
            return s;
        }
        assert!(Instant::now() < end, "inspection worker timeout");
        std::thread::sleep(Duration::from_millis(2));
    }
}
fn reply(worker: &Worker) -> Reply {
    let end = Instant::now() + Duration::from_secs(4);
    loop {
        if let Some(r) = worker.reply() {
            return r;
        }
        assert!(Instant::now() < end);
        std::thread::sleep(Duration::from_millis(2));
    }
}
#[test]
fn ethernet_ipv4_udp_layers_and_exact_fields() {
    let f = Frame::prepare(Arc::new(sample()));
    assert_eq!(f.protocol, "UDP");
    assert!(!f.diagnostic);
    for (label, expected) in [
        ("Ethernet II", 0..14),
        ("IPv4", 14..34),
        ("UDP", 34..42),
        ("Payload / undecoded bytes", 42..50),
        ("Source port: 12000", 34..36),
        ("Destination IP: 127.0.0.2", 30..34),
    ] {
        assert_eq!(range(&f, label), expected);
    }
    assert_eq!(f.selection(42..50).unwrap(), b"EXMON030");
    assert!(f.selection(0..51).is_err());
    assert!(f.selection(4..4).is_err());
}
#[test]
fn vlan_and_ipv4_options_shift_transport_without_guessing() {
    let original = sample();
    let mut bytes = original.bytes().to_vec();
    bytes.splice(12..14, [0x81, 0, 0, 7, 0x08, 0]);
    bytes[18] = 0x46;
    bytes[20..22].copy_from_slice(&40u16.to_be_bytes());
    bytes.splice(38..38, [1, 1, 0, 0]);
    let f = frame(&bytes);
    assert_eq!(range(&f, "VLAN 7 (priority 0)"), 12..16);
    assert_eq!(range(&f, "IPv4"), 18..42);
    assert_eq!(range(&f, "IPv4 options"), 38..42);
    assert_eq!(range(&f, "UDP"), 42..50);
    assert_eq!(f.selection(50..58).unwrap(), b"EXMON030");
}
#[test]
fn ipv6_extension_and_tcp_options_have_independent_offsets() {
    let mut bytes = vec![0; 14 + 40 + 8 + 24 + 3];
    bytes[12..14].copy_from_slice(&[0x86, 0xdd]);
    bytes[14] = 0x60;
    bytes[18..20].copy_from_slice(&35u16.to_be_bytes());
    bytes[20] = 0;
    bytes[21] = 64;
    bytes[37] = 1;
    bytes[53] = 2;
    bytes[54] = 6;
    bytes[55] = 0;
    bytes[62..66].copy_from_slice(&[0, 80, 1, 187]);
    bytes[74] = 0x60;
    bytes[75] = 0x12;
    bytes[82..86].copy_from_slice(&[1, 1, 1, 1]);
    bytes[86..89].copy_from_slice(b"xyz");
    let f = frame(&bytes);
    assert_eq!(f.protocol, "TCP");
    assert_eq!(range(&f, "IPv6"), 14..54);
    assert_eq!(range(&f, "IPv6 extension 0 / next 6"), 54..62);
    assert_eq!(range(&f, "TCP"), 62..86);
    assert_eq!(range(&f, "TCP options"), 82..86);
    assert_eq!(range(&f, "Payload / undecoded bytes"), 86..89);
}
#[test]
fn every_truncated_prefix_preserves_raw_and_checks_all_node_ranges() {
    let observation = sample();
    for length in 0..=50 {
        let input = &observation.bytes()[..length];
        let f = frame(input);
        assert_eq!(f.bytes(), input);
        assert!(f.nodes.len() <= 256);
        for node in &f.nodes {
            if let Some(r) = &node.range {
                assert!(r.start <= r.end && r.end <= length);
            }
        }
        if length < 42 {
            assert!(f.diagnostic);
        }
    }
}
#[test]
fn unknown_link_encrypted_fragments_and_capture_truncation_remain_inspectable() {
    let original = sample();
    let unknown = Frame::prepare(Arc::new(obs(original.bytes(), 147)));
    assert_eq!(unknown.bytes(), original.bytes());
    assert_eq!(unknown.protocol, "Raw");
    assert!(unknown.diagnostic);
    let mut fragment = original.bytes().to_vec();
    fragment[20..22].copy_from_slice(&[0x20, 0]);
    let f = frame(&fragment);
    assert!(f.diagnostic);
    assert!(!f.nodes.iter().any(|n| n.label == "UDP"));
    assert_eq!(f.bytes(), fragment);
    let truncated = Observation::new(
        0,
        Direction::Unknown,
        Metadata::Ethernet { link_type: 1 },
        Some(70),
        original.bytes(),
    )
    .unwrap();
    assert!(Frame::prepare(Arc::new(truncated)).diagnostic);
    let mut encrypted = vec![0; 54];
    encrypted[12..14].copy_from_slice(&[0x86, 0xdd]);
    encrypted[14] = 0x60;
    encrypted[20] = 50;
    let f = frame(&encrypted);
    assert!(f.diagnostic);
    assert_eq!(f.bytes(), encrypted);
}
#[test]
fn socket_metadata_never_invents_packet_header_ranges() {
    let observation = Observation::new(
        8,
        Direction::Rx,
        Metadata::Tcp {
            local: "127.0.0.1:80".parse().unwrap(),
            remote: "127.0.0.2:5000".parse().unwrap(),
            connection: 1,
        },
        None,
        b"payload",
    )
    .unwrap();
    let f = Frame::prepare(Arc::new(observation));
    assert_eq!(f.protocol, "TCP socket");
    assert_eq!(range(&f, "Payload / undecoded bytes"), 0..7);
    assert!(
        f.nodes
            .iter()
            .find(|n| n.label.starts_with("Socket metadata"))
            .unwrap()
            .range
            .is_none()
    );
    assert!(!Filter::parse("tcp port=80").unwrap().matches(&f));
}
#[test]
fn display_filters_are_bounded_and_invalid_filter_keeps_last_valid() {
    let f = Frame::prepare(Arc::new(sample()));
    for text in [
        "",
        "ethernet ipv4 udp port=12001",
        "src=127.0.0.1 dst=127.0.0.2 len>=50 len<=50 contains=45584D",
    ] {
        assert!(Filter::parse(text).unwrap().matches(&f), "{text}");
    }
    for text in [
        "tcp",
        "ipv6",
        "port=80",
        "host=127.0.0.3",
        "diagnostic",
        "len<=49",
        "contains=FFFF",
    ] {
        assert!(!Filter::parse(text).unwrap().matches(&f), "{text}");
    }
    for text in [
        "udp or tcp",
        "port=65536",
        "src=bad",
        "contains=F",
        "udp udp udp udp udp udp udp udp udp",
    ] {
        assert!(Filter::parse(text).is_err());
    }
    let mut history = History::default();
    history.push(Arc::new(sample())).unwrap();
    history.set_filter("tcp").unwrap();
    assert_eq!(history.snapshot().hidden, 1);
    assert!(history.set_filter("bad").is_err());
    assert_eq!(history.snapshot().filter, "tcp");
    assert!(history.snapshot().records.is_empty());
}
#[test]
fn linear_search_overlaps_limits_and_rejects_bad_patterns() {
    assert_eq!(
        Pattern::parse("ABA", false).unwrap().find(b"ABABABA", 256),
        (vec![0..3, 2..5, 4..7], false)
    );
    assert_eq!(
        Pattern::parse("45 58", true).unwrap().find(b"EXEX", 256),
        (vec![0..2, 2..4], false)
    );
    let (found, more) = Pattern::parse("a", false)
        .unwrap()
        .find(&[b'a'; 65_536], 256);
    assert_eq!(found.len(), 256);
    assert!(more);
    for (s, hex) in [("", false), ("F", true), ("GG", true), ("é", false)] {
        assert!(Pattern::parse(s, hex).is_err());
    }
    assert!(Pattern::parse(&"a".repeat(257), false).is_err());
    assert!(Pattern::parse(&"AA".repeat(257), true).is_err());
}
#[test]
fn pcap_and_pcapng_import_have_identical_live_layers_values_and_raw() {
    let original = sample().with_source_timestamp(123_456_000);
    let live = Frame::prepare(Arc::new(original.clone()));
    for encoding in [Encoding::Pcap { link_type: 1 }, Encoding::PcapNg] {
        let mut history = History::default();
        history
            .import(Cursor::new(encoded(&original, encoding)))
            .unwrap();
        let s = history.snapshot();
        assert!(s.file_mode);
        let imported = &s.records[0].frame;
        assert_eq!(imported.nodes, live.nodes);
        assert_eq!(imported.bytes(), live.bytes());
        assert_eq!(imported.ports, live.ports);
        assert_eq!(
            imported.observation.source_timestamp_ns(),
            Some(123_456_000)
        );
    }
}
#[test]
fn malformed_import_is_atomic_and_unknown_link_import_stays_raw() {
    let mut history = History::default();
    history.push(Arc::new(sample())).unwrap();
    let pinned = history.snapshot().records[0].clone();
    let bytes = encoded(&sample(), Encoding::Pcap { link_type: 1 });
    for length in [0, 10, 25, 39, bytes.len() - 1] {
        assert!(history.import(Cursor::new(&bytes[..length])).is_err());
        assert_eq!(history.snapshot().records[0].id, pinned.id);
        assert!(!history.snapshot().file_mode);
    }
    let unknown = obs(&[1, 2, 3], 147);
    history
        .import(Cursor::new(encoded(
            &unknown,
            Encoding::Pcap { link_type: 147 },
        )))
        .unwrap();
    assert_eq!(history.snapshot().records[0].frame.bytes(), [1, 2, 3]);
    assert!(history.snapshot().records[0].frame.diagnostic);
}
#[test]
fn import_record_limit_and_history_count_and_byte_bounds_are_explicit() {
    let mut history = History::default();
    for _ in 0..600 {
        history.push(Arc::new(sample())).unwrap();
    }
    let s = history.snapshot();
    assert_eq!(s.records.len(), FRAMES);
    assert_eq!(s.evicted, 88);
    let pinned = s.records[0].clone();
    for _ in 0..200 {
        history.push(Arc::new(obs(&[0; 65_536], 147))).unwrap();
    }
    let current = history.snapshot();
    assert!(current.records.len() < 128);
    assert_eq!(pinned.frame.bytes(), sample().bytes());
    assert!(!current.records.iter().any(|r| r.id == pinned.id));
    let mut encoder = Encoder::new(Vec::new(), Encoding::Pcap { link_type: 1 }).unwrap();
    for _ in 0..10_001 {
        encoder
            .write_observation(
                0,
                &Observation::new(
                    0,
                    Direction::Unknown,
                    Metadata::Ethernet { link_type: 1 },
                    Some(50),
                    sample().bytes(),
                )
                .unwrap()
                .with_source_timestamp(0),
            )
            .unwrap();
    }
    let bytes = encoder.finish().unwrap();
    assert!(history.import(Cursor::new(bytes)).is_err());
    assert_eq!(
        history.snapshot().records.last().unwrap().id,
        current.records.last().unwrap().id
    );
}
#[test]
fn worker_generation_file_isolation_raw_only_and_selection_actions() {
    let worker = Worker::new().unwrap();
    worker.begin(7);
    wait(&worker, |s| s.generation == 7);
    worker.submit(&batch(7, sample())).unwrap();
    let s = wait(&worker, |s| s.records.len() == 1);
    let f = s.records[0].frame.clone();
    worker
        .request(Action::Search {
            frame: f.clone(),
            text: "EXMON".into(),
            hex: false,
        })
        .unwrap();
    match reply(&worker) {
        Reply::Search(r, more) => {
            assert_eq!(r, vec![42..47]);
            assert!(!more);
        }
        _ => panic!("search reply"),
    }
    worker
        .request(Action::Copy {
            frame: f.clone(),
            range: 42..44,
            ascii: false,
        })
        .unwrap();
    match reply(&worker) {
        Reply::Copy(s) => assert_eq!(s, "45 58"),
        _ => panic!("copy reply"),
    }
    worker
        .request(Action::Copy {
            frame: f.clone(),
            range: 42..50,
            ascii: true,
        })
        .unwrap();
    match reply(&worker) {
        Reply::Copy(s) => assert_eq!(s, "EXMON030"),
        _ => panic!("copy reply"),
    }
    let path = std::env::temp_dir().join(format!(
        "exmon030-{}-{}.bin",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    worker
        .request(Action::Export {
            frame: f.clone(),
            range: 42..50,
            path: path.to_str().unwrap().into(),
        })
        .unwrap();
    assert!(matches!(
        reply(&worker),
        Reply::Notice("Selected raw bytes exported")
    ));
    assert_eq!(std::fs::read(&path).unwrap(), b"EXMON030");
    worker
        .request(Action::Export {
            frame: f,
            range: 0..1,
            path: path.to_str().unwrap().into(),
        })
        .unwrap();
    assert!(matches!(
        reply(&worker),
        Reply::Notice("Destination exists; nothing overwritten")
    ));
    assert_eq!(std::fs::read(&path).unwrap(), b"EXMON030");
    std::fs::remove_file(path).unwrap();
    worker.begin(8);
    wait(&worker, |s| s.generation == 8 && s.records.is_empty());
    worker.submit(&batch(7, sample())).unwrap();
    let mut normalized = batch(8, sample());
    Arc::get_mut(&mut normalized.rows[0]).unwrap().normalized = Some(Arc::from(&b"decoded"[..]));
    worker.submit(&normalized).unwrap();
    worker.request(Action::Filter("".into())).unwrap();
    reply(&worker);
    assert!(wait(&worker, |s| s.generation == 8).records.is_empty());
    worker.request(Action::Sample).unwrap();
    reply(&worker);
    let s = wait(&worker, |s| s.synthetic && s.records.len() == 1);
    let id = s.records[0].id;
    worker.submit(&batch(8, sample())).unwrap();
    assert_eq!(wait(&worker, |s| s.ignored_live > 0).records[0].id, id);
    worker.request(Action::Live).unwrap();
    reply(&worker);
    assert!(wait(&worker, |s| !s.synthetic).records.is_empty());
    assert!(worker.request(Action::Filter("x".repeat(1025))).is_err());
    let invalid = PreparedBatch {
        generation: 8,
        rows: (0..97).map(|_| batch(8, sample()).rows.remove(0)).collect(),
    };
    assert!(worker.submit(&invalid).is_err());
}

#[test]
fn simple_packet_without_timestamp_and_ng_direction_are_preserved() {
    let input = sample().with_source_timestamp(123);
    let mut encoder = Encoder::new(Vec::new(), Encoding::PcapNg).unwrap();
    encoder.write_observation(1, &input).unwrap();
    let bytes = encoder.finish().unwrap();
    let mut history = History::default();
    history.import(Cursor::new(&bytes)).unwrap();
    assert_eq!(
        history.snapshot().records[0].frame.observation.direction(),
        Direction::Rx
    );
    // Keep the independently recognized section and interface blocks, replace EPB by SPB.
    let section_len = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
    let interface_len =
        u32::from_le_bytes(bytes[section_len + 4..section_len + 8].try_into().unwrap()) as usize;
    let mut simple = bytes[..section_len + interface_len].to_vec();
    for value in [3u32, 68, 50] {
        simple.extend(value.to_le_bytes());
    }
    simple.extend(input.bytes());
    simple.extend([0, 0]);
    simple.extend(68u32.to_le_bytes());
    history.import(Cursor::new(simple)).unwrap();
    let s = history.snapshot();
    assert_eq!(s.records[0].frame.bytes(), input.bytes());
    assert_eq!(s.records[0].frame.observation.source_timestamp_ns(), None);
    assert_eq!(
        s.records[0].frame.nodes,
        Frame::prepare(Arc::new(input)).nodes
    );
}

#[test]
fn worker_keeps_file_context_across_capture_generation_and_drains_bounded_replies() {
    let path = std::env::temp_dir().join(format!("exmon030-import-{}.pcap", std::process::id()));
    std::fs::write(&path, encoded(&sample(), Encoding::Pcap { link_type: 1 })).unwrap();
    let worker = Worker::new().unwrap();
    worker.begin(1);
    wait(&worker, |s| s.generation == 1);
    worker
        .request(Action::Import(path.to_str().unwrap().into()))
        .unwrap();
    reply(&worker);
    let imported = wait(&worker, |s| s.file_mode);
    let id = imported.records[0].id;
    worker.begin(2);
    wait(&worker, |s| s.generation == 2);
    worker.submit(&batch(2, sample())).unwrap();
    let s = wait(&worker, |s| s.ignored_live == 1);
    assert_eq!(s.records.len(), 1);
    assert_eq!(s.records[0].id, id);
    for _ in 0..8 {
        worker.request(Action::Filter(String::new())).unwrap();
    }
    assert!(worker.request(Action::Live).is_err());
    for _ in 0..8 {
        assert!(matches!(reply(&worker), Reply::Notice(_)));
    }
    worker.request(Action::Live).unwrap();
    reply(&worker);
    wait(&worker, |s| !s.file_mode && s.records.is_empty());
    worker.submit(&batch(2, sample())).unwrap();
    wait(&worker, |s| s.records.len() == 1);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn invalid_capture_record_original_length_never_replaces_committed_view() {
    let mut bytes = encoded(&sample(), Encoding::Pcap { link_type: 1 });
    bytes[36..40].copy_from_slice(&1u32.to_le_bytes());
    let mut history = History::default();
    history.push(Arc::new(sample())).unwrap();
    assert!(history.import(Cursor::new(bytes)).is_err());
    assert!(!history.snapshot().file_mode);
    assert_eq!(history.snapshot().records.len(), 1);
}

#[test]
fn saturated_input_is_nonblocking_reports_drops_and_does_not_block_commands() {
    let worker = Worker::new().unwrap();
    worker.begin(1);
    wait(&worker, |s| s.generation == 1);
    let mut input = batch(1, obs(&[0; 65_536], 147));
    input.rows = vec![input.rows[0].clone(); 16];
    let start = Instant::now();
    let mut rejected = 0;
    for _ in 0..100 {
        if worker.submit(&input).is_err() {
            rejected += 1;
        }
    }
    assert!(start.elapsed() < Duration::from_millis(500));
    assert!(rejected > 0);
    worker.request(Action::Filter("raw".into())).unwrap();
    reply(&worker);
    wait(&worker, |s| s.input_dropped > 0 && s.filter == "raw");
}
