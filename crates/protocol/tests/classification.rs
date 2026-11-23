use exmon_domain::{
    Direction,
    classification::{MetadataMatch, valid_auto_id},
    config::ProjectConfig,
    observation::{Metadata, Observation, UsbMonMetadata},
};
use exmon_protocol::{
    classification::*,
    framing::{Event, Framer, Source},
};
use serde_json::{Value, json};
fn rule(id: &str, priority: i32, condition: Value) -> Value {
    let mut c = condition;
    c["priority"] = json!(priority);
    json!({"id":id,"name":format!("Name {id}"),"framing":{"kind":"fixed","length":65536},"classification":c})
}
fn project(rules: Vec<Value>) -> ProjectConfig {
    serde_json::from_value(json!({"schema_version":1,"protocols":rules})).unwrap()
}
fn ethernet() -> Metadata {
    Metadata::Ethernet { link_type: 1 }
}
fn classify(c: &Classifier, bytes: &[u8]) -> Decision {
    c.classify(&ethernet(), Direction::Rx, bytes, Quality::Complete)
        .unwrap()
}
fn trace() -> UsbMonMetadata {
    UsbMonMetadata {
        tag: 1,
        event: b'C',
        transfer: b'I',
        bus: 1,
        device: 2,
        endpoint: 129,
        status: Some(0),
        interval: None,
        start_frame: None,
        errors: None,
        setup: None,
        setup_tag: None,
        iso_count: 0,
        iso: [None; 5],
        reported_length: 3,
        data_tag: b'=',
    }
}
#[test]
fn independent_prefix_mask_offset_priority_and_explanations() {
    let cfg = project(vec![
        rule("low", i32::MIN, json!({})),
        rule(
            "specific",
            i32::MAX,
            json!({"length":3,"offset":1,"prefix":[160,5],"mask":[240,255],"direction":"Rx","transport":{"kind":"ethernet","link_type":1}}),
        ),
    ]);
    let c = Classifier::new(&cfg, 7).unwrap();
    assert_eq!(c.condition("specific").unwrap().offset, 1);
    assert_eq!(c.condition("specific").unwrap().mask, [240, 255]);
    assert!(c.condition("absent").is_none());
    for ignored in 0..16 {
        assert_eq!(
            classify(&c, &[ignored, 160 + ignored, 5]).group,
            GroupId::Rule("specific".into())
        );
    }
    let decision = classify(&c, &[0, 175, 5]);
    assert_eq!(decision.outcome, Outcome::Matched);
    assert_eq!(decision.group, GroupId::Rule("specific".into()));
    assert_eq!(decision.name, "Name specific");
    assert_eq!(decision.revision, 7);
    assert_eq!(decision.contenders, ["specific"]);
    assert!(decision.checks[0].selected);
    assert_eq!(decision.checks[0].criteria, 31);
    assert_eq!(decision.checks[1].failed, None);
    assert!(!decision.checks[1].selected);
    assert!(decision.checks[1].explanation().contains("higher-priority"));
    let decision = classify(&c, &[0, 159, 5]);
    assert_eq!(decision.group, GroupId::Rule("low".into()));
    assert_eq!(decision.checks[0].failed, Some(Predicate::HeaderMismatch));
    let decision = classify(&c, &[0, 175]);
    assert_eq!(decision.checks[0].failed, Some(Predicate::Length));
    let decision = c
        .classify(&ethernet(), Direction::Tx, &[0, 175, 5], Quality::Complete)
        .unwrap();
    assert_eq!(decision.checks[0].failed, Some(Predicate::Direction));
    let decision = c
        .classify(
            &Metadata::Serial {
                configuration_revision: 1,
            },
            Direction::Rx,
            &[0, 175, 5],
            Quality::Complete,
        )
        .unwrap();
    assert_eq!(decision.checks[0].failed, Some(Predicate::Transport));
    let c = Classifier::new(
        &project(vec![rule("header", 0, json!({"offset":4,"prefix":[1]}))]),
        0,
    )
    .unwrap();
    let d = classify(&c, &[1]);
    assert_eq!(d.outcome, Outcome::Unmatched);
    assert_eq!(d.checks[0].failed, Some(Predicate::HeaderMissing));
    assert!(!d.checks[0].explanation().is_empty());
}
#[test]
fn tied_rules_remain_ambiguous_and_config_and_observation_order_do_not_choose_winner() {
    let rules = vec![
        rule("z", 10, json!({"prefix":[1]})),
        rule("a", 10, json!({"prefix":[1],"mask":[255]})),
        rule("lower", 9, json!({})),
    ];
    let cfg = project(rules.clone());
    let c = Classifier::new(&cfg, 1).unwrap();
    let expected = classify(&c, &[1, 2, 3]);
    assert_eq!(expected.outcome, Outcome::Ambiguous);
    assert_eq!(expected.contenders, ["a", "z"]);
    assert!(matches!(expected.group, GroupId::Auto(_)));
    assert_eq!(expected.checks.iter().filter(|r| r.selected).count(), 2);
    assert_eq!(expected.checks[2].failed, None);
    assert!(!expected.checks[2].selected);
    for order in [
        vec![rules[2].clone(), rules[1].clone(), rules[0].clone()],
        vec![rules[1].clone(), rules[0].clone(), rules[2].clone()],
    ] {
        let other = Classifier::new(&project(order), 1).unwrap();
        assert_eq!(classify(&other, &[1, 2, 3]), expected);
    }
    for _ in 0..100 {
        let _ = classify(&c, &[9, 8]);
        assert_eq!(classify(&c, &[1, 2, 3]), expected);
    }
    let d = classify(&c, b"");
    assert_eq!(d.group, GroupId::Rule("lower".into()));
}
#[test]
fn every_metadata_variant_and_selector_field_uses_typed_equality() {
    let tcp = Metadata::Tcp {
        local: "192.0.2.1:1234".parse().unwrap(),
        remote: "198.51.100.1:80".parse().unwrap(),
        connection: 9,
    };
    let udp = Metadata::Udp {
        local: "[::1]:1234".parse().unwrap(),
        remote: "[::1]:80".parse().unwrap(),
    };
    let cases = vec![
        (ethernet(), json!({"kind":"ethernet","link_type":1})),
        (
            tcp.clone(),
            json!({"kind":"tcp","local":"192.0.2.1:1234","remote":"198.51.100.1:80","local_port":1234,"remote_port":80}),
        ),
        (
            udp,
            json!({"kind":"udp","local":"[::1]:1234","remote":"[::1]:80","local_port":1234,"remote_port":80}),
        ),
        (
            Metadata::Serial {
                configuration_revision: 23,
            },
            json!({"kind":"serial","configuration_revision":23}),
        ),
        (
            Metadata::Usb {
                endpoint: 129,
                completed: true,
            },
            json!({"kind":"usb","endpoint":129}),
        ),
        (
            Metadata::Hid {
                vendor_id: 1,
                product_id: 2,
                usage_page: 3,
                usage: 4,
                report_id: 5,
            },
            json!({"kind":"hid","vendor_id":1,"product_id":2,"usage_page":3,"usage":4,"report_id":5}),
        ),
        (
            Metadata::UsbMon(trace()),
            json!({"kind":"usbmon","event":67,"transfer":73,"endpoint":129,"bus":1,"device":2}),
        ),
        (
            Metadata::Can {
                channel: 2,
                id: 291,
                extended: false,
                fd: false,
                remote: false,
                error: false,
                dlc: 3,
            },
            json!({"kind":"can","channel":2,"id":291,"extended":false,"fd":false,"remote":false,"error":false,"dlc":3}),
        ),
    ];
    for (metadata, selector) in cases {
        let cfg = project(vec![rule(
            "type",
            1,
            json!({"transport":selector.clone(),"length":3,"prefix":[1]}),
        )]);
        let c = Classifier::new(&cfg, 0).unwrap();
        assert_eq!(
            c.classify(&metadata, Direction::Rx, &[1, 2, 3], Quality::Complete)
                .unwrap()
                .outcome,
            Outcome::Matched
        );
        for (field, value) in selector.as_object().unwrap() {
            if field == "kind" {
                continue;
            }
            let mut other = selector.clone();
            other[field] = match value {
                Value::Bool(b) => json!(!b),
                Value::Number(n) => json!(n.as_u64().unwrap() + 1),
                Value::String(s) => json!(s.replace("1234", "1235").replace(":80", ":81")),
                _ => panic!(),
            };
            // Change one predicate without making the independent endpoint/port constraints contradictory.
            if matches!(field.as_str(), "local" | "remote") {
                other
                    .as_object_mut()
                    .unwrap()
                    .remove(&format!("{field}_port"));
            }
            if matches!(field.as_str(), "local_port" | "remote_port") {
                other
                    .as_object_mut()
                    .unwrap()
                    .remove(field.trim_end_matches("_port"));
            }
            let cfg = project(vec![rule("type", 1, json!({"transport":other}))]);
            if cfg.validate().is_err() {
                continue;
            }
            let c = Classifier::new(&cfg, 0).unwrap();
            assert_eq!(
                c.classify(&metadata, Direction::Rx, &[1, 2, 3], Quality::Complete)
                    .unwrap()
                    .outcome,
                Outcome::Unmatched,
                "{field}"
            );
        }
        let c = Classifier::new(&project(vec![]), 0).unwrap();
        let d = c
            .classify(&metadata, Direction::Rx, &[1, 2, 3], Quality::Complete)
            .unwrap();
        let GroupId::Auto(key) = d.group else {
            panic!()
        };
        assert!(valid_auto_id(&key));
    }
    let c = Classifier::new(
        &project(vec![rule("udp", 0, json!({"transport":{"kind":"udp"}}))]),
        0,
    )
    .unwrap();
    assert_eq!(
        c.classify(&tcp, Direction::Rx, &[], Quality::Complete)
            .unwrap()
            .outcome,
        Outcome::Unmatched
    );
}
#[test]
fn automatic_keys_are_full_canonical_features_and_ignore_only_declared_volatile_fields() {
    let mut cfg = project(vec![]);
    let c = Classifier::new(&cfg, 1).unwrap();
    let d = classify(&c, &[1, 2, 3]);
    assert_eq!(
        d.group,
        GroupId::Auto("auto-v1-000000000000010000000303010203".into())
    );
    assert_ne!(d.group, classify(&c, &[1, 2, 4]).group);
    assert_ne!(d.group, classify(&c, &[1, 2, 3, 0]).group);
    assert_ne!(
        d.group,
        c.classify(&ethernet(), Direction::Tx, &[1, 2, 3], Quality::Complete)
            .unwrap()
            .group
    );
    assert_ne!(
        d.group,
        c.classify(
            &Metadata::Ethernet { link_type: 2 },
            Direction::Rx,
            &[1, 2, 3],
            Quality::Complete
        )
        .unwrap()
        .group
    );
    cfg.grouping.header_bytes = 0;
    let c = Classifier::new(&cfg, 1).unwrap();
    assert_eq!(
        classify(&c, &[1, 2, 3]).group,
        classify(&c, &[9, 8, 7]).group
    );
    let a = Metadata::Tcp {
        local: "192.0.2.1:10".parse().unwrap(),
        remote: "198.51.100.1:20".parse().unwrap(),
        connection: 1,
    };
    let b = Metadata::Tcp {
        local: "203.0.113.2:10".parse().unwrap(),
        remote: "203.0.113.3:20".parse().unwrap(),
        connection: 999,
    };
    assert_eq!(
        c.classify(&a, Direction::Rx, b"x", Quality::Complete)
            .unwrap()
            .group,
        c.classify(&b, Direction::Rx, b"y", Quality::Complete)
            .unwrap()
            .group
    );
    let a = Metadata::Serial {
        configuration_revision: 1,
    };
    let b = Metadata::Serial {
        configuration_revision: 999,
    };
    assert_eq!(
        c.classify(&a, Direction::Rx, b"x", Quality::Complete)
            .unwrap()
            .group,
        c.classify(&b, Direction::Rx, b"y", Quality::Complete)
            .unwrap()
            .group
    );
    let mut a = trace();
    let mut b = a.clone();
    b.tag = 999;
    b.bus = 25;
    b.device = 42;
    b.status = Some(-1);
    assert_eq!(
        c.classify(
            &Metadata::UsbMon(a.clone()),
            Direction::Rx,
            b"x",
            Quality::Complete
        )
        .unwrap()
        .group,
        c.classify(&Metadata::UsbMon(b), Direction::Rx, b"y", Quality::Complete)
            .unwrap()
            .group
    );
    a.endpoint = 130;
    assert_ne!(
        c.classify(&Metadata::UsbMon(a), Direction::Rx, b"x", Quality::Complete)
            .unwrap()
            .group,
        c.classify(
            &Metadata::UsbMon(trace()),
            Direction::Rx,
            b"x",
            Quality::Complete
        )
        .unwrap()
        .group
    );
}
#[test]
fn explicit_names_persist_after_reload_and_old_decisions_keep_their_revision_and_name() {
    let mut cfg = project(vec![rule("typed", 5, json!({"prefix":[165]}))]);
    let original = Classifier::new(&cfg, 1).unwrap();
    let typed = classify(&original, &[165, 0]);
    let fallback = classify(&original, &[5, 6]);
    let GroupId::Auto(key) = &fallback.group else {
        panic!()
    };
    cfg.rename_type("typed", "Temperature").unwrap();
    cfg.rename_auto_group(key, "Unrecognized status").unwrap();
    let serialized = serde_json::to_vec(&cfg).unwrap();
    let restored: ProjectConfig = serde_json::from_slice(&serialized).unwrap();
    restored.validate().unwrap();
    let renamed = Classifier::new(&restored, 2).unwrap();
    assert_eq!(classify(&renamed, &[165, 0]).name, "Temperature");
    assert_eq!(classify(&renamed, &[5, 6]).name, "Unrecognized status");
    assert_eq!(classify(&renamed, &[5, 6]).group, fallback.group);
    assert_eq!(typed.name, "Name typed");
    assert_eq!(typed.revision, 1);
    assert_eq!(classify(&original, &[165, 0]), typed);
    for name in ["", "bad\nname", &"x".repeat(129)] {
        let before = cfg.clone();
        assert!(cfg.rename_auto_group(key, name).is_err());
        assert_eq!(cfg, before);
        assert!(cfg.rename_type("typed", name).is_err());
        assert_eq!(cfg, before);
    }
    let before = cfg.clone();
    assert!(cfg.rename_type("missing", "Name").is_err());
    assert!(cfg.rename_auto_group("auto-v1-bad", "Name").is_err());
    assert_eq!(cfg, before);
}
#[test]
fn unknown_incomplete_and_native_stream_boundaries_are_visible_without_fabricated_matches() {
    let c = Classifier::new(&project(vec![rule("catch", 0, json!({}))]), 1).unwrap();
    let o = Observation::new(0, Direction::Rx, ethernet(), Some(10), b"short").unwrap();
    let d = c.classify_observation(&o).unwrap();
    assert_eq!(d.outcome, Outcome::Incomplete);
    assert_eq!(d.quality, Quality::Incomplete);
    assert_eq!(d.checks[0].failed, Some(Predicate::IncompleteInput));
    assert!(d.contenders.is_empty());
    assert!(matches!(d.group, GroupId::Auto(_)));
    assert_eq!(o.bytes(), b"short");
    let o = Observation::new(0, Direction::Rx, ethernet(), None, b"unknown").unwrap();
    let d = c.classify_observation(&o).unwrap();
    assert_eq!(d.outcome, Outcome::Matched);
    assert_eq!(d.quality, Quality::Unknown);
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
        c.classify_observation(&o).unwrap().outcome,
        Outcome::Incomplete
    );
    let o = Observation::new(
        0,
        Direction::Rx,
        Metadata::Serial {
            configuration_revision: 1,
        },
        None,
        b"x",
    )
    .unwrap();
    assert_eq!(c.classify_observation(&o).unwrap_err(), Error::NeedsFraming);
    let mut f = Framer::new(exmon_domain::config::Framing::Fixed { length: 1 }, None).unwrap();
    let out = f
        .push(
            Source {
                observation: 0,
                offset: 0,
            },
            0,
            b"x",
        )
        .unwrap();
    let Event::Message(message) = &out[0] else {
        panic!()
    };
    assert_eq!(
        c.classify_message(message, o.metadata(), Direction::Rx, Quality::Complete)
            .unwrap()
            .outcome,
        Outcome::Matched
    );
    let c = Classifier::new(&project(vec![]), 0).unwrap();
    let a = c
        .classify(&ethernet(), Direction::Rx, b"", Quality::Complete)
        .unwrap();
    let b = c
        .classify(&ethernet(), Direction::Rx, b"", Quality::Incomplete)
        .unwrap();
    assert_eq!(a.outcome, Outcome::Unmatched);
    assert_eq!(b.outcome, Outcome::Incomplete);
    assert_ne!(a.group, b.group);
}
#[test]
fn accepted_132_case_framing_corpus_classifies_expected_messages_in_production() {
    let corpus: Value =
        serde_json::from_str(include_str!("../../../fixtures/traffic/corpus.json")).unwrap();
    let cases = corpus["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 132);
    let cfg = project(vec![rule(
        "synthetic_value",
        0,
        json!({"length":3,"prefix":[165]}),
    )]);
    let classifier = Classifier::new(&cfg, 1).unwrap();
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
        let mut f = Framer::new(serde_json::from_value(framing).unwrap(), None).unwrap();
        let mut events = vec![];
        let mut time = 0;
        for (id, o) in case["observations"].as_array().unwrap().iter().enumerate() {
            let data: Vec<u8> = o["bytes"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as u8)
                .collect();
            time = o["timestamp_ns"].as_u64().unwrap();
            let metadata = if mode == "datagram" {
                ethernet()
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
                &data,
            )
            .unwrap();
            events.extend(f.observe(id as u64, &o).unwrap());
        }
        events.extend(f.finish(time).unwrap());
        let mut recognized = vec![];
        for e in &events {
            if let Event::Message(m) = e {
                let d = classifier
                    .classify_message(m, &ethernet(), Direction::Rx, Quality::Complete)
                    .unwrap();
                if d.outcome == Outcome::Matched {
                    assert_eq!(d.group, GroupId::Rule("synthetic_value".into()));
                    recognized.push(m.bytes().to_vec());
                } else {
                    assert_eq!(d.outcome, Outcome::Unmatched);
                    assert!(matches!(d.group, GroupId::Auto(_)));
                    assert!(!d.checks[0].explanation().is_empty());
                }
            }
        }
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
        assert_eq!(recognized, expected, "{}", case["id"]);
    }
}
#[test]
fn hard_rule_pattern_input_and_name_budgets_and_redacted_debug() {
    let cfg = project(
        (0..256)
            .map(|i| rule(&format!("r{i:03}"), 0, json!({})))
            .collect(),
    );
    let c = Classifier::new(&cfg, 0).unwrap();
    let d = classify(&c, b"\xde\xad\xbe\xef");
    assert_eq!(d.outcome, Outcome::Ambiguous);
    assert_eq!(d.contenders.len(), 256);
    assert_eq!(d.checks.len(), 256);
    assert!(d.name.len() <= 128);
    let cfg = project(
        (0..257)
            .map(|i| rule(&format!("r{i:03}"), 0, json!({})))
            .collect(),
    );
    assert_eq!(Classifier::new(&cfg, 0).unwrap_err(), Error::Configuration);
    let mut cfg = project(
        (0..16)
            .map(|i| rule(&format!("r{i}"), 0, json!({})))
            .collect(),
    );
    for p in &mut cfg.protocols {
        p.classification.prefix = vec![0; 65536];
    }
    assert_eq!(
        Classifier::new(&cfg, 0).unwrap().pattern_bytes(),
        MAX_PATTERN_BYTES
    );
    cfg.protocols.push(cfg.protocols[0].clone());
    cfg.protocols.last_mut().unwrap().id = "extra".into();
    assert_eq!(Classifier::new(&cfg, 0).unwrap_err(), Error::PatternBudget);
    let c = Classifier::new(&project(vec![]), 0).unwrap();
    assert_eq!(
        c.classify(
            &ethernet(),
            Direction::Rx,
            &vec![0; 65537],
            Quality::Complete
        )
        .unwrap_err(),
        Error::Input
    );
    assert_eq!(
        c.classify(
            &Metadata::Usb {
                endpoint: 0x70,
                completed: true
            },
            Direction::Rx,
            b"",
            Quality::Complete
        )
        .unwrap_err(),
        Error::Input
    );
    let mut cfg = project(vec![]);
    cfg.grouping.header_bytes = 16;
    for i in 0..256u16 {
        let d = classify(&Classifier::new(&cfg, 0).unwrap(), &i.to_be_bytes());
        let GroupId::Auto(key) = d.group else {
            panic!()
        };
        cfg.rename_auto_group(&key, "Saved name").unwrap();
    }
    let before = cfg.clone();
    let d = classify(&Classifier::new(&cfg, 0).unwrap(), b"new");
    let GroupId::Auto(key) = d.group else {
        panic!()
    };
    assert!(cfg.rename_auto_group(&key, "Overflow").is_err());
    assert_eq!(cfg, before);
    let mut cfg = project(vec![rule("SECRET", 0, json!({"prefix":[222,173,190,239]}))]);
    cfg.protocols[0].name = "PRIVATE NAME".into();
    let c = Classifier::new(&cfg, 0).unwrap();
    let d = classify(&c, b"\xde\xad\xbe\xef");
    let debug = format!("{c:?} {d:?} {:?} {:?}", d.group, d.checks);
    for secret in ["SECRET", "PRIVATE", "222, 173, 190, 239"] {
        assert!(!debug.contains(secret));
    }
}
#[test]
fn malformed_selectors_and_key_prefixes_are_rejected_without_panic() {
    for selector in [
        json!({"kind":"can","id":536870912}),
        json!({"kind":"can","id":2048,"extended":false,"error":false}),
        json!({"kind":"can","dlc":16}),
        json!({"kind":"can","fd":true,"remote":true}),
        json!({"kind":"usb","endpoint":112}),
        json!({"kind":"tcp","local":"127.0.0.1:1","local_port":2}),
        json!({"kind":"udp","local":"127.0.0.1:1","remote":"[::1]:2"}),
    ] {
        let cfg = project(vec![rule("invalid", 0, json!({"transport":selector}))]);
        assert_eq!(Classifier::new(&cfg, 0).unwrap_err(), Error::Configuration);
    }
    for bad in [
        json!({"offset":65536,"prefix":[1]}),
        json!({"offset":u32::MAX,"prefix":[1]}),
        json!({"prefix":[1],"mask":[255,255]}),
        json!({"prefix":[],"mask":[0]}),
        json!({"length":0}),
    ] {
        assert_eq!(
            Classifier::new(&project(vec![rule("invalid", 0, bad)]), 0).unwrap_err(),
            Error::Configuration
        );
    }
    let d = classify(&Classifier::new(&project(vec![]), 0).unwrap(), b"abc");
    let GroupId::Auto(key) = d.group else {
        panic!()
    };
    assert!(valid_auto_id(&key));
    for end in 0..key.len() {
        assert!(!valid_auto_id(&key[..end]));
    }
    for i in 0..key.len() {
        let mut b = key.clone().into_bytes();
        b[i] = b'!';
        assert!(!valid_auto_id(std::str::from_utf8(&b).unwrap()));
    }
    assert!(!valid_auto_id(&"auto-v1-".repeat(100)));
    assert!(
        serde_json::from_value::<MetadataMatch>(json!({"kind":"ethernet","unknown":1})).is_err()
    );
}
#[test]
fn wildcard_error_flag_accepts_valid_nonextended_can_error_identifiers() {
    let metadata = Metadata::Can {
        channel: 0,
        id: 0x10000,
        extended: false,
        fd: false,
        remote: false,
        error: true,
        dlc: 8,
    };
    let observation = Observation::new(0, Direction::Rx, metadata, Some(8), b"err-data").unwrap();
    let config = project(vec![rule(
        "error",
        0,
        json!({"transport":{"kind":"can","extended":false,"id":65536}}),
    )]);
    let classifier = Classifier::new(&config, 1).unwrap();
    assert_eq!(
        classifier.classify_observation(&observation).unwrap().group,
        GroupId::Rule("error".into())
    );
    let unmatched = Classifier::new(&project(vec![]), 1)
        .unwrap()
        .classify_observation(&observation)
        .unwrap();
    let GroupId::Auto(id) = unmatched.group else {
        panic!()
    };
    assert!(valid_auto_id(&id));
}
#[test]
fn independent_slow_match_reference_and_high_cardinality_input_leave_no_history() {
    let rules: Vec<_> = (0..32)
        .map(|i| {
            rule(
                &format!("r{i:02}"),
                i % 3,
                json!({"length":4,"offset":1,"prefix":[i*3],"mask":[15]}),
            )
        })
        .collect();
    let cfg = project(rules);
    let c = Classifier::new(&cfg, 1).unwrap();
    let initial = classify(&c, &[1, 2, 3]);
    for value in 0..4096u32 {
        let storage = [
            (value >> 8) as u8,
            value as u8,
            (value >> 4) as u8,
            (value >> 12) as u8,
        ];
        let input = if value % 7 == 0 {
            &storage[..3]
        } else {
            &storage[..]
        };
        let mut matches: Vec<_> = (0..32)
            .filter(|i| input.len() == 4 && (input[1] & 15) == ((i * 3) & 15) as u8)
            .map(|i| (i % 3, format!("r{i:02}")))
            .collect();
        let highest = matches.iter().map(|(p, _)| *p).max();
        matches.retain(|(p, _)| Some(*p) == highest);
        matches.sort();
        let expected: Vec<_> = matches.iter().map(|(_, id)| id.clone()).collect();
        let actual = classify(&c, input);
        assert_eq!(actual.contenders, expected);
        assert_eq!(
            actual.outcome,
            match expected.len() {
                0 => Outcome::Unmatched,
                1 => Outcome::Matched,
                _ => Outcome::Ambiguous,
            }
        );
        assert_eq!(c.rule_count(), 32);
        assert_eq!(c.pattern_bytes(), 64);
    }
    assert_eq!(classify(&c, &[1, 2, 3]), initial);
    let c = Classifier::new(&project(vec![]), 1).unwrap();
    let mut keys = std::collections::BTreeSet::new();
    for n in 0..4096u32 {
        let d = classify(&c, &n.to_be_bytes());
        let GroupId::Auto(key) = d.group else {
            panic!()
        };
        assert!(valid_auto_id(&key));
        assert!(keys.insert(key));
    }
    assert_eq!(c.rule_count(), 0);
}
