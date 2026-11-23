use exmon_domain::{
    Direction,
    config::{Endian, Escape, Framing, ProjectConfig},
    observation::{Metadata, Observation},
};
use exmon_protocol::framing::*;
use serde_json::Value;
fn source(offset: usize) -> Source {
    Source {
        observation: 7,
        offset: 100 + offset as u32,
    }
}
fn messages(events: &[Event]) -> Vec<&Message> {
    events
        .iter()
        .filter_map(|e| {
            if let Event::Message(m) = e {
                Some(m)
            } else {
                None
            }
        })
        .collect()
}
fn payloads(events: &[Event]) -> Vec<Vec<u8>> {
    messages(events)
        .into_iter()
        .map(|m| m.bytes().to_vec())
        .collect()
}
fn reasons(events: &[Event]) -> Vec<Reason> {
    events
        .iter()
        .filter_map(|e| {
            if let Event::Diagnostic { reason, .. } = e {
                Some(*reason)
            } else {
                None
            }
        })
        .collect()
}
fn delimiter(max: u32) -> Framing {
    Framing::Delimiter {
        bytes: vec![10],
        max_length: max,
    }
}
fn run(rule: Framing, escape: Option<Escape>, wire: &[u8], mask: usize) -> Vec<Event> {
    let mut f = Framer::new(rule, escape).unwrap();
    let mut out = vec![];
    let mut start = 0;
    for end in 1..=wire.len() {
        if end == wire.len() || mask & (1 << (end - 1)) != 0 {
            out.extend(f.push(source(start), 0, &wire[start..end]).unwrap());
            start = end;
        }
    }
    out.extend(f.finish(10).unwrap());
    out
}
fn partitions(rule: Framing, escape: Option<Escape>, wire: &[u8], expected: &[&[u8]]) {
    assert!(wire.len() <= 15);
    let reference = run(rule.clone(), escape, wire, 0);
    assert_eq!(payloads(&reference), expected);
    for mask in 0..(1usize << wire.len().saturating_sub(1)) {
        assert_eq!(
            run(rule.clone(), escape, wire, mask),
            reference,
            "partition {mask}"
        );
    }
    for m in messages(&reference) {
        assert_eq!(m.sources.len(), 1);
        assert_eq!(m.sources[0].observation, 7);
        assert_eq!(m.sources[0].stream, m.wire);
        assert_eq!(
            m.sources[0].offsets,
            (100 + m.wire.start as u32)..(100 + m.wire.end as u32)
        );
        assert_eq!(m.bytes().len(), m.byte_ranges.len());
    }
}
#[test]
fn exhaustive_read_splits_preserve_all_stream_grammars_and_provenance() {
    partitions(
        Framing::Fixed { length: 3 },
        None,
        b"abcdefg",
        &[b"abc", b"def"],
    );
    partitions(delimiter(16), None, b"ab\ncd\n\n", &[b"ab", b"cd", b""]);
    partitions(
        Framing::Delimiter {
            bytes: b"aba".to_vec(),
            max_length: 16,
        },
        None,
        b"xabazaba",
        &[b"x", b"z"],
    );
    partitions(
        Framing::LengthPrefix {
            offset: 0,
            width: 1,
            endian: Endian::Big,
            max_length: 16,
        },
        None,
        &[2, 65, 66, 1, 67, 0],
        &[b"AB", b"C", b""],
    );
    partitions(
        Framing::Markers {
            start: b"<".to_vec(),
            end: b">".to_vec(),
            max_length: 16,
        },
        None,
        b"x<ab><><c>y",
        &[b"ab", b"", b"c"],
    );
    partitions(
        Framing::IdleGap {
            gap_ns: 100,
            max_length: 16,
        },
        None,
        b"arbitrary",
        &[b"arbitrary"],
    );
    let escape = Some(Escape {
        byte: 0x7d,
        xor: 0x20,
    });
    partitions(
        delimiter(16),
        escape,
        &[65, 0x7d, 10 ^ 0x20, 66, 10, 67, 10],
        &[b"A\nB", b"C"],
    );
    partitions(
        Framing::Fixed { length: 2 },
        escape,
        &[65, 0x7d, 0x7d ^ 0x20, 66, 67],
        &[b"A}", b"BC"],
    );
    partitions(
        Framing::Markers {
            start: b"<".to_vec(),
            end: b">".to_vec(),
            max_length: 16,
        },
        escape,
        &[b'<', 0x7d, b'>' ^ 0x20, b'x', b'>'],
        &[b">x"],
    );
    partitions(
        Framing::LengthPrefix {
            offset: 0,
            width: 1,
            endian: Endian::Little,
            max_length: 16,
        },
        escape,
        &[1, 0x7d, 0x7d ^ 0x20, 1, 65],
        &[b"}", b"A"],
    );
}
#[test]
fn multi_byte_length_offsets_endianness_and_zero_payloads_are_explicit() {
    for width in [1, 2, 4] {
        for endian in [Endian::Little, Endian::Big] {
            let mut wire = vec![];
            for payload in [b"hello".as_slice(), b""] {
                wire.extend_from_slice(&[0xaa, 0x55]);
                let count = payload.len() as u32;
                let value = match endian {
                    Endian::Little => count.to_le_bytes(),
                    Endian::Big => count.to_be_bytes(),
                };
                if endian == Endian::Little {
                    wire.extend_from_slice(&value[..width as usize]);
                } else {
                    wire.extend_from_slice(&value[4 - width as usize..]);
                }
                wire.extend_from_slice(payload);
            }
            let rule = Framing::LengthPrefix {
                offset: 2,
                width,
                endian,
                max_length: 32,
            };
            let mut f = Framer::new(rule, None).unwrap();
            let mut out = vec![];
            for (index, b) in wire.iter().enumerate() {
                out.extend(f.push(source(index), 0, std::slice::from_ref(b)).unwrap());
            }
            out.extend(f.finish(1).unwrap());
            assert_eq!(payloads(&out), [b"hello".to_vec(), vec![]]);
            assert!(reasons(&out).is_empty());
        }
    }
}
#[test]
fn marker_prefixes_do_not_restart_inside_a_possible_end_marker() {
    partitions(
        Framing::Markers {
            start: b"<".to_vec(),
            end: b"</>".to_vec(),
            max_length: 16,
        },
        None,
        b"<X</>",
        &[b"X"],
    );
    partitions(
        Framing::Markers {
            start: b"b".to_vec(),
            end: b"abc".to_vec(),
            max_length: 16,
        },
        None,
        b"bXabc",
        &[b"X"],
    );
    partitions(
        Framing::Markers {
            start: b"<".to_vec(),
            end: b"</>".to_vec(),
            max_length: 16,
        },
        None,
        b"<old<Y</>",
        &[b"Y"],
    );
}
#[test]
fn all_short_binary_markers_accept_an_independent_constructed_frame() {
    let mut patterns = vec![];
    for length in 1..=3 {
        for bits in 0..(1 << length) {
            patterns.push(
                (0..length)
                    .map(|i| if bits & (1 << i) == 0 { b'A' } else { b'B' })
                    .collect::<Vec<_>>(),
            );
        }
    }
    for start in &patterns {
        for end in &patterns {
            let mut wire = start.clone();
            wire.push(0xfe);
            wire.extend_from_slice(end);
            partitions(
                Framing::Markers {
                    start: start.clone(),
                    end: end.clone(),
                    max_length: 16,
                },
                None,
                &wire,
                &[&[0xfe]],
            );
        }
    }
}
#[test]
fn escape_straddling_observations_has_exact_raw_and_decoded_byte_ranges() {
    let mut f = Framer::new(
        delimiter(16),
        Some(Escape {
            byte: 0x7d,
            xor: 0x20,
        }),
    )
    .unwrap();
    assert!(
        f.push(
            Source {
                observation: 10,
                offset: 3
            },
            0,
            &[65, 0x7d]
        )
        .unwrap()
        .is_empty()
    );
    let out = f
        .push(
            Source {
                observation: 11,
                offset: 0,
            },
            1,
            &[10 ^ 0x20, 66, 10],
        )
        .unwrap();
    let m = messages(&out)[0];
    assert_eq!(m.bytes(), b"A\nB");
    assert_eq!(m.wire, 0..5);
    assert_eq!(m.byte_ranges, vec![0..1, 1..3, 3..4]);
    assert_eq!(
        m.sources,
        vec![
            SourceSpan {
                observation: 10,
                offsets: 3..5,
                stream: 0..2
            },
            SourceSpan {
                observation: 11,
                offsets: 0..3,
                stream: 2..5
            }
        ]
    );
    assert!(!format!("{m:?} {f:?}").contains("A\\nB"));
    let out = f.push(source(5), 2, &[0x7d]).unwrap();
    assert!(out.is_empty());
    let out = f.finish(3).unwrap();
    assert!(reasons(&out).contains(&Reason::DanglingEscape));
    assert!(payloads(&out).is_empty());
    assert_eq!(f.stats().pending_source_bytes, 0);
    assert_eq!(f.push(source(0), 3, b"a").unwrap_err(), Error::Finished);
}
#[test]
fn delimiter_and_marker_overflow_nested_start_and_length_resync_recover() {
    let raw = b"ABCDE\nOK\n";
    let events = run(delimiter(4), None, raw, 0);
    assert_eq!(payloads(&events), [b"OK".to_vec()]);
    assert_eq!(
        reasons(&events),
        [Reason::Oversized, Reason::DiscardedUntilBoundary]
    );
    assert!(matches!(&events[0],Event::Diagnostic{wire,..}if *wire==(0..5)));
    assert_eq!(messages(&events)[0].wire, 6..9);
    partitions(delimiter(4), None, raw, &[b"OK"]);
    let rule = Framing::Markers {
        start: b"<".to_vec(),
        end: b">".to_vec(),
        max_length: 5,
    };
    let events = run(rule, None, b"x<ab<z><toolong><q>", 0);
    assert_eq!(payloads(&events), [b"z".to_vec(), b"q".to_vec()]);
    assert_eq!(
        reasons(&events),
        [
            Reason::Noise,
            Reason::Restarted,
            Reason::Oversized,
            Reason::DiscardedUntilBoundary
        ]
    );
    assert_eq!(messages(&events)[0].wire, 4..7);
    partitions(
        Framing::Markers {
            start: b"aa".to_vec(),
            end: b"bb".to_vec(),
            max_length: 8,
        },
        None,
        b"zaaXbb",
        &[b"X"],
    );
    let wire = [255, 2, 0xaa, 0xbb, 1, 0xcc, 0];
    let events = run(
        Framing::LengthPrefix {
            offset: 0,
            width: 1,
            endian: Endian::Big,
            max_length: 8,
        },
        None,
        &wire,
        0,
    );
    assert_eq!(payloads(&events), [vec![0xaa, 0xbb], vec![0xcc], vec![]]);
    assert_eq!(reasons(&events), [Reason::InvalidLength]);
    partitions(
        Framing::LengthPrefix {
            offset: 0,
            width: 1,
            endian: Endian::Big,
            max_length: 8,
        },
        None,
        &wire,
        &[&[0xaa, 0xbb], &[0xcc], b""],
    );
}
#[test]
fn idle_gap_deadlines_empty_reads_overflow_and_clock_edges() {
    let rule = Framing::IdleGap {
        gap_ns: 10,
        max_length: 3,
    };
    let mut f = Framer::new(rule.clone(), None).unwrap();
    assert!(f.push(source(0), 5, b"ab").unwrap().is_empty());
    assert!(f.push(source(2), 14, b"").unwrap().is_empty());
    let out = f.tick(15).unwrap();
    assert_eq!(payloads(&out), [b"ab".to_vec()]);
    assert_eq!(messages(&out)[0].completed_ns, 15);
    f.push(source(2), 20, b"c").unwrap();
    let out = f.push(source(3), 30, b"d").unwrap();
    assert_eq!(payloads(&out), [b"c".to_vec()]);
    assert_eq!(payloads(&f.finish(31).unwrap()), [b"d".to_vec()]);
    let mut f = Framer::new(rule, None).unwrap();
    let out = f.push(source(0), 0, b"abcd").unwrap();
    assert_eq!(reasons(&out), [Reason::Oversized]);
    assert!(f.stats().discarding);
    let out = f.tick(10).unwrap();
    assert!(payloads(&out).is_empty());
    assert_eq!(reasons(&out), [Reason::DiscardedUntilBoundary]);
    f.push(source(4), 11, b"ok").unwrap();
    assert_eq!(payloads(&f.finish(12).unwrap()), [b"ok".to_vec()]);
    let mut f = Framer::new(
        Framing::IdleGap {
            gap_ns: 10,
            max_length: 8,
        },
        None,
    )
    .unwrap();
    f.push(source(0), u64::MAX - 5, b"end").unwrap();
    assert!(f.tick(u64::MAX).unwrap().is_empty());
    let before = f.stats();
    assert_eq!(f.tick(u64::MAX - 1).unwrap_err(), Error::ClockWentBackwards);
    assert_eq!(f.stats(), before);
    assert_eq!(payloads(&f.finish(u64::MAX).unwrap()), [b"end".to_vec()]);
}
#[test]
fn native_empty_frames_incomplete_capture_loss_and_metadata_boundaries() {
    let mut f = Framer::new(Framing::Datagram {}, None).unwrap();
    let out = f.push_frame(source(0), 0, b"").unwrap();
    let m = messages(&out)[0];
    assert!(m.bytes().is_empty());
    assert_eq!(m.sources[0].observation, 7);
    assert_eq!(m.sources[0].offsets, 100..100);
    assert_eq!(f.push(source(0), 0, b"x").unwrap_err(), Error::WrongMode);
    let mut f = Framer::new(delimiter(16), None).unwrap();
    let observation = |time, original, bytes: &[u8]| {
        Observation::new(
            time,
            Direction::Rx,
            Metadata::Serial {
                configuration_revision: 1,
            },
            original,
            bytes,
        )
        .unwrap()
    };
    f.observe(0, &observation(0, Some(2), b"AB")).unwrap();
    let out = f.observe(1, &observation(1, Some(2), b"C")).unwrap();
    assert_eq!(reasons(&out), [Reason::Gap, Reason::IncompleteCapture]);
    let out = f.observe(2, &observation(2, None, b"D\n")).unwrap();
    assert_eq!(payloads(&out), [b"D".to_vec()]);
    assert_eq!(messages(&out)[0].wire, 3..5);
    let mut f = Framer::new(Framing::Datagram {}, None).unwrap();
    assert_eq!(
        f.observe(0, &observation(0, Some(1), b"x")).unwrap_err(),
        Error::WrongMode
    );
    let o = Observation::new(
        0,
        Direction::Rx,
        Metadata::Usb {
            endpoint: 129,
            completed: false,
        },
        Some(1),
        b"x",
    )
    .unwrap();
    assert_eq!(
        reasons(&f.observe(0, &o).unwrap()),
        [Reason::Gap, Reason::IncompleteCapture]
    );
    let o = Observation::new(
        1,
        Direction::Rx,
        Metadata::Ethernet { link_type: 1 },
        None,
        b"unknown",
    )
    .unwrap();
    let out = f.observe(1, &o).unwrap();
    assert_eq!(reasons(&out), [Reason::UnknownOriginalLength]);
    assert_eq!(payloads(&out), [b"unknown".to_vec()]);
}
#[test]
fn actual_tcp_reconstruction_data_and_permanent_gap_never_join_messages() {
    use exmon_protocol::tcp_reassembly as tcp;
    let mut tcp = tcp::Reassembler::new(tcp::Limits {
        gap_timeout_ns: 10,
        ..Default::default()
    })
    .unwrap();
    let segment = |sequence, payload| tcp::Segment {
        source: "192.0.2.1:1234".parse().unwrap(),
        destination: "198.51.100.2:4321".parse().unwrap(),
        vlans: &[],
        sequence,
        acknowledgement: None,
        syn: false,
        fin: false,
        rst: false,
        payload,
        original_payload_len: payload.len(),
    };
    let mut f = Framer::new(delimiter(16), None).unwrap();
    let mut output = vec![];
    let mut id = 0;
    let initial = tcp
        .push(tcp::Context::default(), 0, segment(100, b"A"))
        .unwrap();
    assert!(!initial.is_empty());
    let queued = tcp
        .push(tcp::Context::default(), 1, segment(103, b"B\n"))
        .unwrap();
    let expired = tcp.tick(11).unwrap();
    for (time, events) in [(0, initial), (1, queued), (11, expired)] {
        for e in events {
            match e.kind {
                tcp::EventKind::Data { bytes, .. } => {
                    output.extend(
                        f.push(
                            Source {
                                observation: id,
                                offset: 0,
                            },
                            time,
                            &bytes,
                        )
                        .unwrap(),
                    );
                    id += 1;
                }
                tcp::EventKind::Gap { .. } => output.extend(f.gap(time).unwrap()),
                _ => {}
            }
        }
    }
    assert_eq!(payloads(&output), [b"B".to_vec()]);
    assert!(reasons(&output).contains(&Reason::Gap));
}
#[test]
fn all_132_accepted_traffic_cases_use_the_production_framer() {
    let corpus: Value =
        serde_json::from_str(include_str!("../../../fixtures/traffic/corpus.json")).unwrap();
    let cases = corpus["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 132);
    for case in cases {
        let mode = case["framing"].as_str().unwrap();
        let rule = match mode {
            "datagram" => Framing::Datagram {},
            "fixed" => Framing::Fixed { length: 3 },
            "delimiter" => delimiter(64),
            "length_prefix" => Framing::LengthPrefix {
                offset: 0,
                width: 1,
                endian: Endian::Big,
                max_length: 64,
            },
            _ => panic!(),
        };
        let mut f = Framer::new(rule, None).unwrap();
        let mut out = vec![];
        let mut time = 0;
        for (id, o) in case["observations"].as_array().unwrap().iter().enumerate() {
            let bytes: Vec<u8> = o["bytes"]
                .as_array()
                .unwrap()
                .iter()
                .map(|b| b.as_u64().unwrap() as u8)
                .collect();
            time = o["timestamp_ns"].as_u64().unwrap();
            let metadata = if mode == "datagram" {
                Metadata::Ethernet { link_type: 1 }
            } else {
                Metadata::Serial {
                    configuration_revision: 1,
                }
            };
            let o = Observation::new(
                time,
                Direction::Rx,
                metadata,
                o["original_length"].as_u64().map(|n| n as usize),
                &bytes,
            )
            .unwrap();
            out.extend(f.observe(id as u64, &o).unwrap());
        }
        out.extend(f.finish(time).unwrap());
        let actual: Vec<_> = payloads(&out)
            .into_iter()
            .filter(|m| m.len() == 3 && m[0] == 165)
            .collect();
        let expected: Vec<Vec<u8>> = case["expected"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| {
                m["bytes"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_u64().unwrap() as u8)
                    .collect()
            })
            .collect();
        assert_eq!(actual, expected, "{}", case["id"]);
    }
}
#[test]
fn shared_configuration_validation_and_runtime_entry_points_agree() {
    let value = serde_json::json!({"schema_version":1,"protocols":[{"id":"p","name":"P","framing":{"kind":"markers","start":[60],"end":[62],"max_length":16},"escape":{"byte":125,"xor":32}}]});
    let config: ProjectConfig = serde_json::from_value(value).unwrap();
    config.validate().unwrap();
    let mut f = Framer::from_protocol(&config.protocols[0]).unwrap();
    assert_eq!(
        payloads(&f.push(source(0), 0, b"<hello>").unwrap()),
        [b"hello".to_vec()]
    );
    for rule in [
        Framing::Fixed { length: 0 },
        Framing::Fixed { length: 65537 },
        Framing::Delimiter {
            bytes: vec![],
            max_length: 5,
        },
        Framing::Markers {
            start: vec![0; 17],
            end: vec![2],
            max_length: 64,
        },
        Framing::LengthPrefix {
            offset: u32::MAX,
            width: 4,
            endian: Endian::Big,
            max_length: 64,
        },
        Framing::IdleGap {
            gap_ns: 0,
            max_length: 1,
        },
    ] {
        assert!(rule.limit(None).is_err());
        assert_eq!(Framer::new(rule, None).unwrap_err(), Error::Configuration);
    }
    assert!(Framer::new(delimiter(5), Some(Escape { byte: 10, xor: 0 })).is_err());
    let mut f = Framer::new(Framing::Fixed { length: 2 }, None).unwrap();
    let before = f.stats();
    assert_eq!(
        f.push(source(0), 0, &vec![0; 65537]).unwrap_err(),
        Error::Input
    );
    assert_eq!(
        f.push(
            Source {
                observation: 0,
                offset: u32::MAX
            },
            0,
            b"x"
        )
        .unwrap_err(),
        Error::Input
    );
    assert_eq!(f.stats(), before);
}
#[test]
fn maximum_partial_state_output_and_deterministic_hostile_streams_stay_bounded() {
    let escape = Some(Escape {
        byte: 0x7d,
        xor: 0x20,
    });
    let mut f = Framer::new(Framing::Fixed { length: 65536 }, escape).unwrap();
    let half = [0x7d, 0x61 ^ 0x20].repeat(32768);
    assert!(f.push(source(0), 0, &half).unwrap().is_empty());
    assert_eq!(f.stats().pending_decoded, 32768);
    assert_eq!(f.stats().pending_source_bytes, 65536);
    let out = f
        .push(
            Source {
                observation: 8,
                offset: 0,
            },
            1,
            &half,
        )
        .unwrap();
    let m = messages(&out)[0];
    assert_eq!(m.bytes(), vec![0x61; 65536]);
    assert_eq!(m.wire, 0..131072);
    assert_eq!(m.sources.len(), 2);
    assert_eq!(f.stats().pending_decoded, 0);
    let mut f = Framer::new(Framing::Fixed { length: 1 }, None).unwrap();
    let out = f
        .push(
            Source {
                observation: 1,
                offset: 0,
            },
            0,
            &vec![1; 65536],
        )
        .unwrap();
    assert_eq!(messages(&out).len(), 65536);
    for rule in [
        delimiter(32),
        Framing::Markers {
            start: b"AB".to_vec(),
            end: b"BA".to_vec(),
            max_length: 32,
        },
        Framing::LengthPrefix {
            offset: 1,
            width: 2,
            endian: Endian::Little,
            max_length: 32,
        },
        Framing::IdleGap {
            gap_ns: 5,
            max_length: 32,
        },
    ] {
        let mut f = Framer::new(rule, escape).unwrap();
        let mut seed = 12345u32;
        let mut all = vec![];
        for id in 0..512 {
            let mut bytes = [0; 47];
            for b in &mut bytes {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                *b = seed as u8;
            }
            all.extend_from_slice(&bytes);
            let out = f
                .push(
                    Source {
                        observation: id,
                        offset: 0,
                    },
                    id,
                    &bytes,
                )
                .unwrap();
            assert!(out.len() <= 2 * bytes.len() + 4);
            let stats = f.stats();
            assert!(stats.pending_decoded <= 32);
            assert!(stats.pending_source_bytes <= 65);
            for m in messages(&out) {
                assert!(m.bytes().len() <= 32);
                assert!(m.wire.end <= all.len() as u64);
                assert_eq!(m.bytes().len(), m.byte_ranges.len());
                for (&value, r) in m.bytes().iter().zip(&m.byte_ranges) {
                    let raw = &all[r.start as usize..r.end as usize];
                    assert_eq!(
                        value,
                        if raw.len() == 2 {
                            assert_eq!(raw[0], 0x7d);
                            raw[1] ^ 0x20
                        } else {
                            assert_eq!(raw.len(), 1);
                            raw[0]
                        }
                    );
                }
            }
        }
        let out = f.finish(1000).unwrap();
        assert!(out.len() <= 4);
        assert_eq!(f.stats().pending_source_bytes, 0);
    }
}
