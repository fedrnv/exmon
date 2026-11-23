use exmon_domain::{
    checksum::{self, ByteRange, Checksum},
    config::{Endian, Protocol},
    fields::{Literal, Number},
};
use exmon_protocol::{
    checksum::{Algorithm, Failure, Input, Issue, Registry},
    codec::{Codec, Edit, Error},
    fields::Issue as FieldIssue,
};
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

fn protocol(fields: Value, checksums: Value) -> Protocol {
    serde_json::from_value(json!({"id":"p","name":"Synthetic","framing":{"kind":"datagram"},"fields":fields,"checksums":checksums})).unwrap()
}
fn field(id: &str, offset: u32, length: u32, representation: &str) -> Value {
    json!({"id":id,"offset":offset,"length":length,"representation":representation,"endian":"big"})
}
fn checksum(id: &str, algorithm: &str, offset: u32, length: u32, ranges: &[(u32, u32)]) -> Value {
    json!({"id":id,"algorithm":algorithm,"offset":offset,"length":length,"endian":"big","ranges":ranges.iter().map(|(offset,length)|json!({"offset":offset,"length":length})).collect::<Vec<_>>()})
}
fn unsigned(value: u64) -> Literal {
    Literal::Unsigned { value }
}

#[test]
fn unchanged_raw_message_is_exact_across_types_unknown_bytes_and_invalid_data() {
    let mut bits = field("bits", 2, 2, "signed");
    bits["bits"] = json!({"offset":3,"width":7});
    let fields = json!([
        field("u", 0, 2, "unsigned"),
        bits,
        field("f32", 4, 4, "float"),
        field("f64", 8, 8, "float"),
        field("bytes", 16, 2, "bytes"),
        field("ascii", 18, 2, "ascii"),
        field("text", 20, 2, "utf8")
    ]);
    let p = protocol(fields, json!([]));
    let codec = Codec::new(&p, &Registry::default()).unwrap();
    let mut source = vec![0xff, 0xfe, 0xfc, 0xf0];
    source.extend(0x7fc12345u32.to_be_bytes());
    source.extend(0x8000000000000000u64.to_be_bytes());
    source.extend([0xab, 0xcd, b'A', 0, 0xc3, 0xa9, 0x99, 0x88]);
    let snapshot = source.clone();
    let draft = codec.decode(&source).unwrap();
    assert!(draft.inspect().is_valid());
    assert_eq!(draft.inspect().fields[2].number, Err(FieldIssue::NonFinite));
    assert_eq!(draft.encode(), snapshot);
    assert_eq!(draft.prepare().unwrap().bytes(), snapshot);
    source.fill(0);
    assert_eq!(draft.original(), snapshot);
    assert_eq!(draft.bytes(), snapshot);
    for len in 0..snapshot.len() {
        let draft = codec.decode(&snapshot[..len]).unwrap();
        assert_eq!(draft.encode(), &snapshot[..len]);
        if len < 22 {
            assert!(!draft.inspect().is_valid());
            assert!(draft.prepare().is_err());
        }
    }
    let mut invalid = snapshot.clone();
    invalid[18] = 0xff;
    invalid[20] = 0xff;
    let draft = codec.decode(&invalid).unwrap();
    assert_eq!(draft.encode(), invalid);
    let report = draft.prepare().unwrap_err().report;
    assert_eq!(
        report.fields[4].raw,
        Some(Literal::Bytes {
            value: vec![0xab, 0xcd]
        })
    );
    assert_eq!(report.fields[5].violations, [FieldIssue::Text]);
    assert_eq!(report.fields[6].violations, [FieldIssue::Text]);
}

#[test]
fn ordered_edits_preserve_undeclared_bytes_bits_and_validated_snapshots() {
    let mut bits = field("bits", 1, 2, "unsigned");
    bits["bits"] = json!({"offset":4,"width":5});
    let mut physical = field("physical", 3, 2, "signed");
    physical["scale"] = json!(0.5);
    physical["bias"] = json!(-1.0);
    let mut label = field("state", 5, 1, "unsigned");
    label["enumeration"] = json!([{"value":{"kind":"unsigned","value":1},"name":"Ready"}]);
    let p = protocol(json!([bits, physical, label]), json!([]));
    let codec = Codec::new(&p, &Registry::default()).unwrap();
    let original = [0xa5, 0xf0, 0x0f, 0, 0, 0, 0x77];
    let mut draft = codec.decode(&original).unwrap();
    let snapshot = draft.prepare().unwrap();
    draft
        .edit(&[
            Edit::Field {
                index: 0,
                value: &unsigned(17),
            },
            Edit::Number {
                index: 1,
                value: Number::Real(2.0),
                tolerance: 0.0,
            },
            Edit::Label {
                index: 2,
                label: "Ready",
            },
        ])
        .unwrap();
    assert_eq!(draft.bytes(), [0xa5, 0xf1, 0x1f, 0, 6, 1, 0x77]);
    assert_eq!(draft.original(), original);
    assert_eq!(snapshot.bytes(), original);
    assert_eq!(snapshot.revision(), 0);
    assert_eq!(draft.revision(), 1);
    let expected = draft.encode();
    draft.edit(&[]).unwrap();
    draft
        .edit(&[Edit::Hex {
            offset: 6,
            text: "77",
        }])
        .unwrap();
    assert_eq!(draft.revision(), 1);
    draft
        .edit(&[
            Edit::Hex {
                offset: 0,
                text: "01",
            },
            Edit::Hex {
                offset: 0,
                text: "a5",
            },
        ])
        .unwrap();
    assert_eq!(draft.encode(), expected);
    assert_eq!(draft.revision(), 1);
    assert_eq!(draft.prepare().unwrap().revision(), 1);
}

#[test]
fn hex_is_strict_and_invalid_field_drafts_require_repair_before_preparation() {
    let mut constrained = field("u", 1, 1, "unsigned");
    constrained["constraints"] = json!({"max":{"kind":"unsigned","value":10}});
    let p = protocol(json!([constrained, field("text", 2, 2, "utf8")]), json!([]));
    let codec = Codec::new(&p, &Registry::default()).unwrap();
    let mut draft = codec.decode(&[0xa5, 1, 0xc3, 0xa9, 0x55]).unwrap();
    let report = draft
        .edit(&[Edit::Hex {
            offset: 1,
            text: "FF  ff 00",
        }])
        .unwrap();
    assert!(!report.is_valid());
    assert_eq!(report.fields[0].violations, [FieldIssue::Range]);
    assert_eq!(report.fields[1].violations, [FieldIssue::Text]);
    assert!(draft.prepare().is_err());
    assert_eq!(draft.encode(), [0xa5, 0xff, 0xff, 0, 0x55]);
    draft
        .edit(&[Edit::Hex {
            offset: 1,
            text: "0a\tC3a9\r\n",
        }])
        .unwrap();
    assert!(draft.prepare().is_ok());
    let saved = draft.encode();
    let revision = draft.revision();
    for text in [
        "",
        " ",
        "0",
        "0 1",
        "0x01",
        "01:02",
        "gg",
        "é",
        "00\u{a0}01",
    ] {
        assert_eq!(
            draft.edit(&[Edit::Hex { offset: 0, text }]).unwrap_err(),
            Error::Hex
        );
        assert_eq!(draft.bytes(), saved);
        assert_eq!(draft.revision(), revision);
    }
    for offset in [5, u32::MAX] {
        assert_eq!(
            draft.edit(&[Edit::Hex { offset, text: "01" }]).unwrap_err(),
            Error::Bounds
        );
        assert_eq!(draft.bytes(), saved);
    }
    let result = draft.edit(&[
        Edit::Hex {
            offset: 0,
            text: "12",
        },
        Edit::Field {
            index: 0,
            value: &unsigned(11),
        },
    ]);
    assert!(matches!(result, Err(Error::Field(e)) if e.issue==FieldIssue::Range));
    assert_eq!(draft.bytes(), saved);
    assert_eq!(draft.revision(), revision);
}

#[test]
fn builtin_checksum_known_vectors_split_coverage_and_endian_match_independent_bytes() {
    for endian in ["little", "big"] {
        let mut crc = checksum("crc", "crc16_ccitt_false", 10, 2, &[(0, 4), (4, 5)]);
        crc["endian"] = json!(endian);
        let p = protocol(
            json!([]),
            json!([
                checksum("sum", "sum8", 9, 1, &[(0, 9)]),
                crc,
                checksum("xor", "xor8", 12, 1, &[(0, 9)])
            ]),
        );
        let codec = Codec::new(&p, &Registry::default()).unwrap();
        let mut bytes = b"123456789".to_vec();
        bytes.extend([0; 4]);
        let mut draft = codec.decode(&bytes).unwrap();
        assert!(draft.prepare().is_err());
        assert_eq!(draft.encode(), bytes);
        draft.repair_checksums().unwrap();
        let expected = if endian == "big" {
            [0xdd, 0x29, 0xb1, 0x31]
        } else {
            [0xdd, 0xb1, 0x29, 0x31]
        };
        assert_eq!(&draft.bytes()[9..], expected);
        assert!(draft.prepare().is_ok());
        draft.repair_checksums().unwrap();
        assert_eq!(draft.revision(), 1);
    }
}

#[test]
fn dependency_order_updates_only_affected_checksum_outputs_and_redecodes_fields() {
    let mut derived = field("derived", 0, 0, "derived");
    derived["derived"] = json!([{"op":"field","id":"stored"},{"op":"literal","value":{"kind":"unsigned","value":2}},{"op":"mul"}]);
    let p = protocol(
        json!([
            field("value", 1, 2, "unsigned"),
            field("stored", 3, 1, "unsigned"),
            derived
        ]),
        json!([
            checksum("outer", "sum8", 4, 1, &[(0, 4)]),
            checksum("inner", "sum8", 3, 1, &[(0, 3)]),
            checksum("other", "xor8", 5, 1, &[(6, 1)])
        ]),
    );
    let codec = Codec::new(&p, &Registry::default()).unwrap();
    let mut draft = codec.decode(&[0xa5, 1, 2, 0xa8, 0x50, 0x77, 0x77]).unwrap();
    assert!(draft.prepare().is_ok());
    let report = draft
        .edit(&[Edit::Field {
            index: 0,
            value: &unsigned(0x0304),
        }])
        .unwrap();
    assert_eq!(draft.bytes(), [0xa5, 3, 4, 0xac, 0x58, 0x77, 0x77]);
    assert_eq!(report.fields[1].number, Ok(Number::Integer(172)));
    assert_eq!(report.fields[2].number, Ok(Number::Integer(344)));
    assert!(report.is_valid());
    for edit in [
        Edit::Hex {
            offset: 3,
            text: "00",
        },
        Edit::Hex {
            offset: 2,
            text: "0000",
        },
        Edit::Field {
            index: 1,
            value: &unsigned(0),
        },
    ] {
        let saved = draft.encode();
        assert_eq!(draft.edit(&[edit]).unwrap_err(), Error::ReadOnly);
        assert_eq!(draft.bytes(), saved);
    }
    let mut bad = codec.decode(&[0xa5, 1, 2, 0, 0, 0x77, 0x77]).unwrap();
    bad.edit(&[Edit::Hex {
        offset: 6,
        text: "66",
    }])
    .unwrap();
    assert_eq!(bad.bytes(), [0xa5, 1, 2, 0, 0, 0x66, 0x66]);
    assert!(bad.prepare().is_err());
    bad.repair_checksums().unwrap();
    assert_eq!(bad.bytes(), [0xa5, 1, 2, 0xa8, 0x50, 0x66, 0x66]);
    assert!(bad.prepare().is_ok());
    let saved = draft.encode();
    assert!(
        matches!(draft.edit(&[Edit::Field { index:2,value:&unsigned(0) }]),Err(Error::Field(e)) if e.issue==FieldIssue::ReadOnly)
    );
    assert_eq!(draft.bytes(), saved);
}

struct Exact {
    expected: Vec<u8>,
    value: u64,
    calls: Arc<AtomicUsize>,
}
impl Algorithm for Exact {
    fn calculate(&self, input: Input<'_>) -> Result<u64, Failure> {
        assert_eq!(input.len(), self.expected.len());
        assert!(!input.is_empty());
        assert_eq!(input.bytes().collect::<Vec<_>>(), self.expected);
        assert_eq!(input.bytes().collect::<Vec<_>>(), self.expected);
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.value)
    }
}
struct Failed;
impl Algorithm for Failed {
    fn calculate(&self, _: Input<'_>) -> Result<u64, Failure> {
        Err(Failure)
    }
}
struct Panicked;
impl Algorithm for Panicked {
    fn calculate(&self, _: Input<'_>) -> Result<u64, Failure> {
        panic!("synthetic trusted extension panic")
    }
}
struct Constant(u64);
impl Algorithm for Constant {
    fn calculate(&self, _: Input<'_>) -> Result<u64, Failure> {
        Ok(self.0)
    }
}

#[test]
fn trusted_extensions_receive_only_declared_bytes_and_support_all_host_widths() {
    for (width, value) in [(1, 0xff), (2, 0x1234), (4, 0xabcdef01), (8, u64::MAX)] {
        for endian in ["little", "big"] {
            let calls = Arc::new(AtomicUsize::new(0));
            let mut registry = Registry::empty();
            registry
                .register(
                    "exact",
                    width,
                    Arc::new(Exact {
                        expected: vec![1, 2, 4],
                        value,
                        calls: calls.clone(),
                    }),
                )
                .unwrap();
            let mut c = checksum("c", "exact", 5, width, &[(1, 2), (4, 1)]);
            c["endian"] = json!(endian);
            let p = protocol(json!([]), json!([c]));
            let codec = Codec::new(&p, &registry).unwrap();
            drop(registry);
            let mut bytes = vec![0x99, 1, 2, 0x88, 4];
            bytes.resize((5 + width) as usize, 0);
            let mut draft = codec.decode(&bytes).unwrap();
            draft.repair_checksums().unwrap();
            let encoded = if endian == "little" {
                value.to_le_bytes()[..width as usize].to_vec()
            } else {
                value.to_be_bytes()[8 - width as usize..].to_vec()
            };
            assert_eq!(&draft.bytes()[5..], encoded);
            assert_eq!(&draft.bytes()[..5], &bytes[..5]);
            assert!(draft.prepare().is_ok());
            assert!(calls.load(Ordering::SeqCst) >= 3);
            assert!(!format!("{draft:?} {:?}", draft.prepare().unwrap()).contains("153"));
        }
    }
}

#[test]
fn unknown_mismatched_failing_panicking_and_oversize_extensions_fail_closed() {
    let p = protocol(json!([]), json!([checksum("c", "custom", 1, 1, &[(0, 1)])]));
    assert!(
        matches!(Codec::new(&p,&Registry::default()),Err(Error::Checksum(e)) if e.issue==Issue::UnknownAlgorithm)
    );
    let mut wrong = Registry::empty();
    wrong.register("custom", 2, Arc::new(Constant(1))).unwrap();
    assert!(matches!(Codec::new(&p,&wrong),Err(Error::Checksum(e)) if e.issue==Issue::Width));
    for (algorithm, expected) in [
        (Arc::new(Failed) as Arc<dyn Algorithm>, Issue::Algorithm),
        (Arc::new(Panicked), Issue::Panic),
        (Arc::new(Constant(256)), Issue::ResultWidth),
    ] {
        let mut registry = Registry::empty();
        registry.register("custom", 1, algorithm).unwrap();
        let codec = Codec::new(&p, &registry).unwrap();
        let mut draft = codec.decode(&[1, 0]).unwrap();
        assert_eq!(draft.inspect().checksums[0].result, Err(expected));
        assert!(draft.prepare().is_err());
        assert!(matches!(draft.repair_checksums(),Err(Error::Checksum(e)) if e.issue==expected));
        assert!(
            matches!(draft.edit(&[Edit::Hex { offset:0,text:"02" }]),Err(Error::Checksum(e)) if e.issue==expected)
        );
        assert_eq!(draft.bytes(), [1, 0]);
        assert_eq!(draft.original(), [1, 0]);
        assert_eq!(draft.revision(), 0);
    }
}

#[test]
fn every_truncated_checksum_prefix_is_preserved_and_touched_coverage_fails_atomically() {
    let p = protocol(
        json!([field("u", 0, 1, "unsigned")]),
        json!([checksum("c", "sum8", 2, 1, &[(0, 2)])]),
    );
    let codec = Codec::new(&p, &Registry::default()).unwrap();
    for len in 0..3 {
        let bytes = &[1, 2, 3][..len];
        let mut draft = codec.decode(bytes).unwrap();
        assert_eq!(draft.encode(), bytes);
        assert!(draft.prepare().is_err());
        assert_eq!(draft.inspect().checksums[0].result, Err(Issue::Bounds));
        assert!(
            matches!(draft.repair_checksums(),Err(Error::Checksum(e)) if e.issue==Issue::Bounds)
        );
        if len > 0 {
            assert!(
                matches!(draft.edit(&[Edit::Hex { offset:0,text:"02" }]),Err(Error::Checksum(e)) if e.issue==Issue::Bounds)
            );
        }
        assert_eq!(draft.bytes(), bytes);
        assert_eq!(draft.revision(), 0);
    }
}

#[test]
fn checksum_definition_geometry_dependency_and_resource_admission_are_shared() {
    let c = checksum("a", "sum8", 3, 1, &[(0, 3)]);
    for (path, bad) in [
        ("/offset", json!(u32::MAX)),
        ("/length", json!(0)),
        ("/length", json!(3)),
        ("/id", json!("")),
        ("/algorithm", json!("../code")),
        ("/ranges", json!([])),
        ("/ranges/0/length", json!(0)),
        ("/ranges/0/offset", json!(u32::MAX)),
        ("/ranges/0/length", json!(4)),
    ] {
        let mut bad_c = c.clone();
        *bad_c.pointer_mut(path).unwrap() = bad;
        let p = protocol(json!([]), json!([bad_c]));
        assert!(checksum::validate(&p.checksums, &p.fields, 65536).is_err());
        assert!(Codec::new(&p, &Registry::default()).is_err());
    }
    for checksums in [
        json!([c.clone(), c.clone()]),
        json!([c.clone(), checksum("b", "sum8", 3, 1, &[(0, 1)])]),
        json!([
            checksum("a", "sum8", 0, 1, &[(1, 1)]),
            checksum("b", "sum8", 1, 1, &[(0, 1)])
        ]),
        json!([checksum("a", "sum8", 4, 1, &[(2, 1), (0, 1)])]),
        json!([checksum("a", "sum8", 4, 1, &[(0, 3), (2, 1)])]),
    ] {
        let p = protocol(json!([]), checksums);
        assert!(Codec::new(&p, &Registry::default()).is_err());
    }
    let p = protocol(json!([field("straddle", 2, 2, "unsigned")]), json!([c]));
    assert!(Codec::new(&p, &Registry::default()).is_err());
    let mut p = protocol(json!([]), json!([]));
    p.checksums = (0..32)
        .map(|i| Checksum {
            id: format!("c{i}"),
            algorithm: "sum8".into(),
            offset: i,
            length: 1,
            endian: Endian::Big,
            ranges: vec![ByteRange {
                offset: 32,
                length: 1,
            }],
        })
        .collect();
    assert_eq!(
        checksum::validate(&p.checksums, &[], 65536).unwrap().len(),
        32
    );
    let codec = Codec::new(&p, &Registry::default()).unwrap();
    let mut draft = codec.decode(&[1; 33]).unwrap();
    assert!(draft.repair_checksums().unwrap().is_valid());
    p.checksums.push(Checksum {
        id: "extra".into(),
        offset: 33,
        ..p.checksums[0].clone()
    });
    assert_eq!(
        checksum::validate(&p.checksums, &[], 65536)
            .unwrap_err()
            .code,
        checksum::Code::Count
    );
    p.checksums.pop();
    for c in &mut p.checksums {
        c.ranges[0].length = 65504;
    }
    assert_eq!(
        checksum::validate(&p.checksums, &[], 65536)
            .unwrap_err()
            .code,
        checksum::Code::Coverage
    );
    let mut registry = Registry::empty();
    for i in 0..64 {
        registry
            .register(&format!("a{i}"), 1, Arc::new(Constant(0)))
            .unwrap();
    }
    assert!(
        registry
            .register("extra", 1, Arc::new(Constant(0)))
            .is_err()
    );
    assert!(registry.register("a0", 1, Arc::new(Constant(0))).is_err());
    for (key, width) in [("", 1), ("space key", 1), ("valid", 3)] {
        assert!(
            Registry::empty()
                .register(key, width, Arc::new(Constant(0)))
                .is_err()
        );
    }
}

#[test]
fn message_hex_batch_and_replacement_budgets_are_enforced_before_mutation() {
    let p = protocol(json!([]), json!([]));
    let codec = Codec::new(&p, &Registry::default()).unwrap();
    assert!(matches!(codec.decode(&vec![0; 65537]), Err(Error::Input)));
    let mut draft = codec.decode(&vec![0; 65536]).unwrap();
    let hex = "ab".repeat(65536);
    draft
        .edit(&[Edit::Hex {
            offset: 0,
            text: &hex,
        }])
        .unwrap();
    assert_eq!(draft.bytes(), vec![0xab; 65536]);
    let saved = draft.encode();
    let edits: Vec<_> = (0..257)
        .map(|_| Edit::Hex {
            offset: 0,
            text: "00",
        })
        .collect();
    assert_eq!(draft.edit(&edits).unwrap_err(), Error::EditLimit);
    assert_eq!(
        draft
            .edit(&[
                Edit::Hex {
                    offset: 0,
                    text: &hex
                },
                Edit::Hex {
                    offset: 0,
                    text: "00"
                }
            ])
            .unwrap_err(),
        Error::EditLimit
    );
    assert_eq!(
        draft
            .edit(&[Edit::Hex {
                offset: 0,
                text: &" ".repeat(3 * 65536 + 1)
            }])
            .unwrap_err(),
        Error::EditLimit
    );
    assert_eq!(draft.bytes(), saved);
    assert_eq!(draft.revision(), 1);
    let many: Vec<_> = (0..256)
        .map(|_| Edit::Hex {
            offset: 0,
            text: "ab",
        })
        .collect();
    draft.edit(&many).unwrap();
    assert_eq!(draft.revision(), 1);
}

#[test]
fn all_132_corpus_cases_pass_real_framing_classification_lossless_codec_and_guarded_editing() {
    use exmon_domain::{
        Direction,
        config::ProjectConfig,
        observation::{Metadata, Observation},
    };
    use exmon_protocol::{
        classification::{Classifier, Outcome, Quality},
        framing::{Event, Framer},
    };
    let corpus: Value =
        serde_json::from_str(include_str!("../../../fixtures/traffic/corpus.json")).unwrap();
    let cases = corpus["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 132);
    let config:ProjectConfig=serde_json::from_value(json!({"schema_version":1,"protocols":[{"id":"value","name":"value","framing":{"kind":"fixed","length":3},"classification":{"length":3,"prefix":[165]},"fields":[field("value",1,2,"unsigned")]}]})).unwrap();
    config.validate().unwrap();
    let classifier = Classifier::new(&config, 1).unwrap();
    let codec = Codec::new(&config.protocols[0], &Registry::default()).unwrap();
    for case in cases {
        let mode = case["framing"].as_str().unwrap();
        let framing = match mode {
            "datagram" => json!({"kind":"datagram"}),
            "fixed" => json!({"kind":"fixed","length":3}),
            "delimiter" => json!({"kind":"delimiter","bytes":[10],"max_length":64}),
            "length_prefix" => {
                json!({"kind":"length_prefix","offset":0,"width":1,"endian":"big","max_length":64})
            }
            _ => panic!(),
        };
        let mut framer = Framer::new(serde_json::from_value(framing).unwrap(), None).unwrap();
        let mut events = vec![];
        let mut time = 0;
        for (id, input) in case["observations"].as_array().unwrap().iter().enumerate() {
            let bytes: Vec<u8> = serde_json::from_value(input["bytes"].clone()).unwrap();
            time = input["timestamp_ns"].as_u64().unwrap();
            let metadata = if mode == "datagram" {
                Metadata::Ethernet { link_type: 1 }
            } else {
                Metadata::Serial {
                    configuration_revision: 1,
                }
            };
            let observation = Observation::new(
                time,
                Direction::Rx,
                metadata,
                input["original_length"].as_u64().map(|n| n as usize),
                &bytes,
            )
            .unwrap();
            events.extend(framer.observe(id as u64, &observation).unwrap());
        }
        events.extend(framer.finish(time).unwrap());
        let mut values = vec![];
        for event in &events {
            if let Event::Message(message) = event {
                let decision = classifier
                    .classify_message(
                        message,
                        &Metadata::Ethernet { link_type: 1 },
                        Direction::Rx,
                        Quality::Complete,
                    )
                    .unwrap();
                if decision.outcome != Outcome::Matched {
                    continue;
                }
                let source = message.bytes().to_vec();
                let mut draft = codec.decode(&source).unwrap();
                assert_eq!(draft.encode(), source);
                assert_eq!(draft.prepare().unwrap().bytes(), source);
                let report = draft.inspect();
                let Some(Literal::Unsigned { value }) = report.fields[0].raw else {
                    panic!()
                };
                values.push(value);
                let edited = value ^ 0x5aa5;
                draft
                    .edit(&[Edit::Field {
                        index: 0,
                        value: &unsigned(edited),
                    }])
                    .unwrap();
                assert_eq!(draft.bytes(), [0xa5, (edited >> 8) as u8, edited as u8]);
                assert_eq!(draft.original(), source);
                assert_eq!(message.bytes(), source);
            }
        }
        let expected: Vec<_> = case["expected"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["fields"]["value"].as_u64().unwrap())
            .collect();
        assert_eq!(values, expected, "{}", case["id"]);
    }
}

#[test]
fn deterministic_mutations_preserve_exact_source_and_independent_checksum_edit_reference() {
    let mut p = protocol(
        json!([field("u", 1, 2, "unsigned")]),
        json!([checksum("sum", "sum8", 4, 1, &[(0, 4)])]),
    );
    let codec = Codec::new(&p, &Registry::default()).unwrap();
    // Later protocol edits cannot change the compiled interpretation of a draft.
    p.fields[0].offset = 0;
    p.checksums.clear();
    let mut random = 0x91e10da5u64;
    for _ in 0..4096 {
        random ^= random << 13;
        random ^= random >> 7;
        random ^= random << 17;
        let b = random.to_be_bytes();
        let source = [b[0], b[1], b[2], b[3], b[4], b[5]];
        let mut draft = codec.decode(&source).unwrap();
        assert_eq!(draft.encode(), source);
        let edited = u64::from(u16::from_be_bytes([b[6], b[7]]));
        draft
            .edit(&[Edit::Field {
                index: 0,
                value: &unsigned(edited),
            }])
            .unwrap();
        let mut expected = source;
        expected[1] = b[6];
        expected[2] = b[7];
        if expected[1..3] != source[1..3] {
            // Independent widened sum reference, rather than builtin wrapping fold.
            expected[4] = (expected[..4].iter().map(|v| u32::from(*v)).sum::<u32>() % 256) as u8;
        }
        assert_eq!(draft.bytes(), expected);
        assert_eq!(draft.original(), source);
        let saved = draft.encode();
        let rev = draft.revision();
        assert_eq!(
            draft
                .edit(&[Edit::Hex {
                    offset: u32::MAX,
                    text: "00"
                }])
                .unwrap_err(),
            Error::Bounds
        );
        assert_eq!(draft.bytes(), saved);
        assert_eq!(draft.revision(), rev);
    }
    for _ in 0..2048 {
        random ^= random << 13;
        random ^= random >> 7;
        random ^= random << 17;
        let mut p = protocol(json!([]), json!([checksum("c", "sum8", 4, 1, &[(0, 4)])]));
        p.checksums[0].offset = random as u32;
        p.checksums[0].ranges[0].length = ((random >> 32) & 0xffff) as u32;
        if let Ok(codec) = Codec::new(&p, &Registry::default()) {
            for len in 0..8 {
                let mut draft = codec.decode(&vec![0; len]).unwrap();
                assert_eq!(draft.encode(), vec![0; len]);
                let _ = draft.repair_checksums();
            }
        }
    }
}

#[test]
fn late_checksum_failure_rolls_back_earlier_checksum_and_field_writes() {
    let mut registry = Registry::default();
    registry.register("fail", 1, Arc::new(Failed)).unwrap();
    let p = protocol(
        json!([field("value", 0, 1, "unsigned")]),
        json!([
            checksum("a", "sum8", 1, 1, &[(0, 1)]),
            checksum("b", "fail", 2, 1, &[(0, 2)])
        ]),
    );
    let codec = Codec::new(&p, &registry).unwrap();
    let mut draft = codec.decode(&[1, 1, 0]).unwrap();
    assert!(
        matches!(draft.edit(&[Edit::Field { index:0,value:&unsigned(2) }]),Err(Error::Checksum(e)) if e.checksum==Some(1) && e.issue==Issue::Algorithm)
    );
    assert_eq!(draft.bytes(), [1, 1, 0]);
    assert_eq!(draft.revision(), 0);
    assert!(!format!("{draft:?} {:?}", draft.inspect()).contains("Synthetic"));
    assert!(
        !ByteRange {
            offset: 1,
            length: 0
        }
        .overlaps(ByteRange {
            offset: 0,
            length: 2
        })
    );
    assert!(
        !ByteRange {
            offset: u32::MAX,
            length: 1
        }
        .overlaps(ByteRange {
            offset: 0,
            length: 2
        })
    );
}

#[test]
fn preparation_reports_every_invalid_field_and_derived_dependency_without_replacing_values() {
    let mut constant = field("constant", 0, 1, "unsigned");
    constant["constant"] = json!({"kind":"unsigned","value":1});
    let mut enumeration = field("enum", 1, 1, "unsigned");
    enumeration["enumeration"] = json!([{"value":{"kind":"unsigned","value":2},"name":"Ready"}]);
    enumeration["constraints"] = json!({"enum_closed":true});
    let mut float = field("float", 2, 4, "float");
    float["constraints"] = json!({"finite":true});
    let mut bytes = field("bytes", 6, 1, "bytes");
    bytes["constant"] = json!({"kind":"bytes","value":[170]});
    let mut derived = field("derived", 0, 0, "derived");
    derived["derived"] = json!([{"op":"literal","value":{"kind":"unsigned","value":1}},{"op":"field","id":"enum"},{"op":"div"}]);
    let p = protocol(
        json!([constant, enumeration, float, bytes, derived]),
        json!([]),
    );
    let codec = Codec::new(&p, &Registry::default()).unwrap();
    let original = [0, 3, 0x7f, 0x80, 0, 0, 0];
    let mut draft = codec.decode(&original).unwrap();
    let report = draft.prepare().unwrap_err().report;
    assert_eq!(report.fields[0].violations, [FieldIssue::Constant]);
    assert_eq!(report.fields[1].violations, [FieldIssue::Enum]);
    assert_eq!(report.fields[2].violations, [FieldIssue::NonFinite]);
    assert_eq!(report.fields[3].violations, [FieldIssue::Constant]);
    assert_eq!(report.fields[4].violations, [FieldIssue::Dependency]);
    assert_eq!(report.fields[0].raw, Some(unsigned(0)));
    assert_eq!(report.fields[1].raw, Some(unsigned(3)));
    assert_eq!(draft.encode(), original);
    let report = draft
        .edit(&[Edit::Hex {
            offset: 0,
            text: "01 02 3f800000 aa",
        }])
        .unwrap();
    assert!(report.is_valid());
    assert_eq!(report.fields[4].number, Ok(Number::Real(0.5)));
    assert!(draft.prepare().is_ok());
    let mut at_limit = protocol(json!([]), json!([]));
    at_limit.checksums = (0..32)
        .map(|i| Checksum {
            id: format!("c{i}"),
            algorithm: "sum8".into(),
            offset: i,
            length: 1,
            endian: Endian::Big,
            ranges: vec![ByteRange {
                offset: 32,
                length: 32768,
            }],
        })
        .collect();
    assert_eq!(
        checksum::validate(&at_limit.checksums, &[], 65536)
            .unwrap()
            .len(),
        32
    );
    at_limit.checksums[0].ranges[0].length += 1;
    assert_eq!(
        checksum::validate(&at_limit.checksums, &[], 65536)
            .unwrap_err()
            .code,
        checksum::Code::Coverage
    );
}
