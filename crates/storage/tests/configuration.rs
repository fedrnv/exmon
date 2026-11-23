use exmon_domain::config::*;
use exmon_storage::config::*;
use serde_json::{Value, json};

fn fixture() -> Value {
    json!({"schema_version":1,
        "protocols":[{"id":"sample","name":"Synthetic","framing":{"kind":"fixed","length":8},
            "fields":[{"id":"value","offset":0,"length":2,"representation":"unsigned","endian":"big"}]}],
        "interfaces":[
            {"id":"ethernet","protocol_id":"sample","transport":{"kind":"ethernet"},"tx_allowed":true},
            {"id":"tcp","protocol_id":"sample","transport":{"kind":"tcp"},"filter":{"ports":[80]}},
            {"id":"udp","protocol_id":"sample","transport":{"kind":"udp"}},
            {"id":"serial","protocol_id":"sample","transport":{"kind":"serial","baud":9600,"data_bits":8,"stop_bits":1,"parity":"none"}},
            {"id":"usb","protocol_id":"sample","transport":{"kind":"usb","endpoint":129,"transfer":"hid"}},
            {"id":"can","protocol_id":"sample","transport":{"kind":"can","bitrate":500000,"fd":false},"filter":{"can_ids":[291]}}
        ],
        "charts":[{"id":"chart","protocol_id":"sample","field_id":"value","samples":100,"moving_average":2}],
        "templates":[{"id":"request","interface_id":"ethernet","bytes":[1,2]}],
        "schedules":[{"id":"poll","template_id":"request","interval_ms":1000}]
    })
}
fn load(value: &Value) -> Result<ProjectConfig, LoadError> {
    load_project(&serde_json::to_vec(value).unwrap())
}
#[test]
fn schedule_options_roundtrip_disabled_defaults_and_reject_invalid_imports() {
    use exmon_domain::schedule::{Drift, Missed};
    let old = load(&fixture()).unwrap();
    let s = &old.schedules[0];
    assert_eq!(s.count, None);
    assert_eq!(s.drift, Drift::FixedRate);
    assert_eq!(s.missed, Missed::Skip);
    assert_eq!(s.max_lateness_ms, None);
    let saved = String::from_utf8(save_project(&old).unwrap()).unwrap();
    for name in [
        "\"count\"",
        "\"drift\"",
        "\"missed\"",
        "\"max_lateness_ms\"",
    ] {
        assert!(!saved.contains(name));
    }
    let mut value = fixture();
    value["schedules"][0]["count"] = json!(7);
    value["schedules"][0]["drift"] = json!("fixed_delay");
    value["schedules"][0]["missed"] = json!("stop");
    value["schedules"][0]["max_lateness_ms"] = json!(0);
    let config = load(&value).unwrap();
    assert!(!config.schedules[0].enabled);
    assert_eq!(
        config,
        load_project(&save_project(&config).unwrap()).unwrap()
    );
    for (key, bad) in [
        ("enabled", json!(true)),
        ("count", json!(0)),
        ("count", json!(-1)),
        ("count", json!(1.5)),
        ("drift", json!("fast")),
        ("missed", json!("retry")),
        ("max_lateness_ms", json!(86400001)),
        ("interval_ms", json!(0)),
        ("interval_ms", json!(86400001)),
        ("extra", json!(0)),
    ] {
        let mut invalid = value.clone();
        invalid["schedules"][0][key] = bad;
        assert!(load(&invalid).is_err(), "{key}");
    }
}

#[test]
fn all_transports_and_defaults_roundtrip_deterministically() {
    let config = load(&fixture()).unwrap();
    assert!(!config.schedules[0].enabled);
    assert!(!config.interfaces[1].tx_allowed);
    assert_eq!(config.protocols[0].fields[0].scale, 1.0);
    let bytes = save_project(&config).unwrap();
    let restored = load_project(&bytes).unwrap();
    assert_eq!(config, restored);
    assert_eq!(bytes, save_project(&restored).unwrap());
    assert_eq!(
        load_project(br#"{"schema_version":1}"#).unwrap(),
        ProjectConfig::default()
    );
}

#[test]
fn framing_and_numeric_variants_roundtrip() {
    for framing in [
        json!({"kind":"datagram"}),
        json!({"kind":"fixed","length":16}),
        json!({"kind":"delimiter","bytes":[13,10],"max_length":16}),
        json!({"kind":"length_prefix","offset":0,"width":2,"endian":"little","max_length":16}),
        json!({"kind":"markers","start":[60],"end":[62],"max_length":16}),
        json!({"kind":"idle_gap","gap_ns":1000,"max_length":16}),
    ] {
        let mut value = fixture();
        value["interfaces"]
            .as_array_mut()
            .unwrap()
            .retain(|i| i["id"] != "tcp" && i["id"] != "serial");
        value["protocols"][0]["framing"] = framing;
        for representation in ["signed", "unsigned", "float"] {
            value["protocols"][0]["fields"][0]["representation"] = json!(representation);
            value["protocols"][0]["fields"][0]["length"] = json!(4);
            let config = load(&value).unwrap();
            assert_eq!(
                config,
                load_project(&save_project(&config).unwrap()).unwrap()
            );
        }
    }
}
#[test]
fn checksum_schema_roundtrip_ownership_and_coverage_validation_preserve_launch_safety() {
    let old = String::from_utf8(save_project(&load(&fixture()).unwrap()).unwrap()).unwrap();
    assert!(!old.contains("\"checksums\""));
    let mut value = fixture();
    value["protocols"][0]["fields"] = json!([
        {"id":"value","offset":0,"length":1,"representation":"unsigned","endian":"big"},
        {"id":"checksum","offset":3,"length":1,"representation":"unsigned","endian":"big"}
    ]);
    value["protocols"][0]["checksums"] = json!([
        {"id":"sum","algorithm":"sum8","offset":3,"length":1,"endian":"big","ranges":[{"offset":0,"length":3}]}
    ]);
    let config = load(&value).unwrap();
    let bytes = save_project(&config).unwrap();
    let loaded = load_project(&bytes).unwrap();
    assert_eq!(loaded, config);
    assert_eq!(save_project(&loaded).unwrap(), bytes);
    assert!(!loaded.schedules[0].enabled);
    assert!(!loaded.interfaces[1].tx_allowed);
    for (path, bad) in [
        ("/protocols/0/checksums/0/offset", json!(u32::MAX)),
        ("/protocols/0/checksums/0/length", json!(3)),
        ("/protocols/0/checksums/0/algorithm", json!("../plugin")),
        ("/protocols/0/checksums/0/ranges/0/length", json!(4)),
        ("/protocols/0/checksums/0/ranges", json!([])),
        ("/protocols/0/fields/1/offset", json!(2)),
    ] {
        let mut invalid = value.clone();
        *invalid.pointer_mut(path).unwrap() = bad;
        if path.ends_with("fields/1/offset") {
            invalid["protocols"][0]["fields"][1]["length"] = json!(2);
        }
        assert!(load(&invalid).is_err(), "{path}");
    }
    let mut invalid = value.clone();
    invalid["protocols"][0]["checksums"][0]["script"] = json!("execute");
    assert!(load(&invalid).is_err());
    // Portable definitions may name trusted application extensions; only codec
    // registry resolution can establish actual algorithm availability.
    value["protocols"][0]["checksums"][0]["algorithm"] = json!("trusted_custom");
    assert!(load(&value).is_ok());
}
#[test]
fn binary_field_schema_extensions_roundtrip_and_reject_unsafe_definitions() {
    let old = String::from_utf8(save_project(&load(&fixture()).unwrap()).unwrap()).unwrap();
    for new in ["\"bits\"", "\"constant\"", "\"constraints\"", "\"derived\""] {
        assert!(!old.contains(new));
    }
    let mut value = fixture();
    value["protocols"][0]["fields"] = json!([
        {"id":"a","offset":0,"length":2,"representation":"unsigned","endian":"little","bits":{"offset":0,"width":4},"constant":{"kind":"unsigned","value":1},"enumeration":[{"value":{"kind":"unsigned","value":1},"name":"Ready"},{"value":{"kind":"unsigned","value":2},"name":"Busy"}],"constraints":{"min":{"kind":"unsigned","value":0},"max":{"kind":"unsigned","value":15},"enum_closed":true}},
        {"id":"b","offset":0,"length":2,"representation":"unsigned","endian":"big","bits":{"offset":0,"width":4},"scale":0.5,"bias":-1.0,"unit":"V"},
        {"id":"float","offset":2,"length":4,"representation":"float","endian":"little","constant":{"kind":"float32","bits":1065353216},"constraints":{"finite":true}},
        {"id":"d","offset":0,"length":0,"representation":"derived","endian":"little","derived":[{"op":"field","id":"a"},{"op":"literal","value":{"kind":"unsigned","value":2}},{"op":"mul"}],"unit":"counts"}
    ]);
    value["charts"][0]["field_id"] = json!("d");
    let config = load(&value).unwrap();
    let bytes = save_project(&config).unwrap();
    assert_eq!(load_project(&bytes).unwrap(), config);
    assert!(!config.schedules[0].enabled);
    assert!(!config.interfaces[1].tx_allowed);
    for (path, bad) in [
        ("/protocols/0/fields/1/bits/offset", json!(8)),
        ("/protocols/0/fields/0/bits/width", json!(0)),
        ("/protocols/0/fields/0/constant/value", json!(16)),
        ("/protocols/0/fields/0/enumeration/1/name", json!("Ready")),
        ("/protocols/0/fields/0/constraints/min/value", json!(16)),
        ("/protocols/0/fields/3/derived/0/id", json!("d")),
        ("/protocols/0/fields/3/derived/0/id", json!("missing")),
        ("/protocols/0/fields/3/derived/2/op", json!("execute")),
        ("/protocols/0/fields/3/length", json!(1)),
    ] {
        let mut invalid = value.clone();
        *invalid.pointer_mut(path).unwrap() = bad;
        assert!(load(&invalid).is_err(), "{path}");
    }
    let mut invalid = value.clone();
    invalid["protocols"][0]["fields"][1]["bits"]["unknown"] = json!(true);
    assert!(load(&invalid).is_err());
}
#[test]
fn classification_rules_and_explicit_names_roundtrip_without_changing_tx_defaults() {
    let old = load(&fixture()).unwrap();
    let encoded = String::from_utf8(save_project(&old).unwrap()).unwrap();
    assert!(!encoded.contains("\"grouping\""));
    assert!(!encoded.contains("\"priority\""));
    let mut value = fixture();
    value["protocols"][0]["classification"] = json!({"priority":-7,"length":4,"offset":1,"prefix":[160,1],"mask":[240,255],"direction":"Rx","transport":{"kind":"tcp","remote_port":80}});
    value["grouping"] = json!({"header_bytes":0,"names":[]});
    let mut config = load(&value).unwrap();
    let key = "auto-v1-0000030000000300";
    config.rename_type("sample", "Temperature").unwrap();
    config.rename_auto_group(key, "Unknown packet").unwrap();
    let bytes = save_project(&config).unwrap();
    let restored = load_project(&bytes).unwrap();
    assert_eq!(restored, config);
    assert_eq!(restored.protocols[0].name, "Temperature");
    assert_eq!(restored.grouping.names[0].name, "Unknown packet");
    assert!(!restored.schedules[0].enabled);
    assert!(!restored.interfaces[1].tx_allowed);
    let mut duplicate = config.clone();
    duplicate
        .grouping
        .names
        .push(duplicate.grouping.names[0].clone());
    assert!(save_project(&duplicate).is_err());
    let before = config.clone();
    assert!(config.rename_auto_group(key, "bad\nname").is_err());
    assert!(config.rename_type("absent", "Name").is_err());
    assert_eq!(config, before);
    for bad in [
        json!({"header_bytes":17}),
        json!({"names":[{"id":"auto-v1-00","name":"N"}]}),
        json!({"names":[{"id":key,"name":""}]}),
        json!({"names":[{"id":key,"name":"N","unknown":1}]}),
    ] {
        let mut value = fixture();
        value["grouping"] = bad;
        assert!(load(&value).is_err());
    }
    for bad in [
        json!({"prefix":[1],"mask":[255,255]}),
        json!({"prefix":[1],"offset":u32::MAX}),
        json!({"transport":{"kind":"usb","endpoint":112}}),
        json!({"transport":{"kind":"ethernet","unknown":1}}),
        json!({"priority":2147483648u64}),
    ] {
        let mut value = fixture();
        value["protocols"][0]["classification"] = bad;
        assert!(load(&value).is_err());
    }
}

#[test]
fn framing_escape_defaults_extensions_and_negative_inputs_preserve_launch_safety() {
    let old = load(&fixture()).unwrap();
    assert_eq!(old.protocols[0].escape, None);
    assert!(
        !String::from_utf8(save_project(&old).unwrap())
            .unwrap()
            .contains("\"escape\"")
    );
    for framing in [
        json!({"kind":"markers","start":[60],"end":[62],"max_length":16}),
        json!({"kind":"idle_gap","gap_ns":1,"max_length":16}),
        json!({"kind":"delimiter","bytes":[10],"max_length":16}),
    ] {
        let mut value = fixture();
        value["protocols"][0]["framing"] = framing;
        value["protocols"][0]["escape"] = json!({"byte":125,"xor":32});
        let config = load(&value).unwrap();
        assert_eq!(
            config.protocols[0].escape,
            Some(Escape { byte: 125, xor: 32 })
        );
        assert_eq!(
            load_project(&save_project(&config).unwrap()).unwrap(),
            config
        );
        assert!(!config.schedules[0].enabled);
        assert!(!config.interfaces[1].tx_allowed);
    }
    for (path, bad) in [
        (
            "/protocols/0/framing",
            json!({"kind":"idle_gap","gap_ns":0,"max_length":16}),
        ),
        (
            "/protocols/0/framing",
            json!({"kind":"markers","start":[],"end":[62],"max_length":16}),
        ),
        (
            "/protocols/0/framing",
            json!({"kind":"markers","start":[60],"end":[62],"max_length":1}),
        ),
        (
            "/protocols/0/framing",
            json!({"kind":"markers","start":[60],"end":[62],"max_length":65537}),
        ),
        (
            "/protocols/0/framing",
            json!({"kind":"delimiter","bytes":[125],"max_length":16}),
        ),
        ("/protocols/0/escape", json!({"byte":256,"xor":32})),
        (
            "/protocols/0/escape",
            json!({"byte":125,"xor":32,"unknown":true}),
        ),
    ] {
        let mut value = fixture();
        value["protocols"][0]["escape"] = json!({"byte":125,"xor":32});
        *value.pointer_mut(path).unwrap() = bad;
        assert!(load(&value).is_err(), "{path}");
    }
}

#[test]
fn machine_bindings_are_separate_and_reference_checked() {
    let config = load(&fixture()).unwrap();
    let bindings = load_bindings(
        br#"{"schema_version":1,"bindings":[{"interface_id":"serial","selector":"virtual-port"}]}"#,
        &config,
    )
    .unwrap();
    let bytes = save_bindings(&bindings, &config).unwrap();
    assert_eq!(bindings, load_bindings(&bytes, &config).unwrap());
    assert!(
        !String::from_utf8(save_project(&config).unwrap())
            .unwrap()
            .contains("virtual-port")
    );
    let mut invalid = bindings.clone();
    invalid.bindings[0].interface_id = "absent".into();
    assert!(save_bindings(&invalid, &config).is_err());
    invalid = bindings.clone();
    invalid.bindings[0].selector = "\n".into();
    assert!(save_bindings(&invalid, &config).is_err());
    invalid = bindings.clone();
    invalid.bindings.push(bindings.bindings[0].clone());
    assert!(save_bindings(&invalid, &config).is_err());
}

#[test]
fn version_and_syntax_errors_are_explicit_bounded_and_redacted() {
    for input in [
        b"{}".as_slice(),
        br#"{"schema_version":1,"secret-device":0}"#,
        br#"{"schema_version":1,"schema_version":1}"#,
        br#"{"schema_version":1,"interfaces":null}"#,
        br#"{"schema_version":1,"protocols":[{"secret-device":1}]}"#,
        b"\xff",
        b"[]",
    ] {
        let error = load_project(input).unwrap_err().to_string();
        assert!(error.len() < 256);
        assert!(!error.contains("secret-device"));
    }
    for version in [0, 2, u32::MAX] {
        assert!(
            matches!(load(&json!({"schema_version":version})), Err(LoadError::Version(v)) if v == version)
        );
    }
    assert!(matches!(
        load_project(&vec![b' '; MAX_DOCUMENT_BYTES + 1]),
        Err(LoadError::TooLarge)
    ));
    let nested = format!(
        "{{\"schema_version\":1,\"protocols\":{}0{}}}",
        "[".repeat(200),
        "]".repeat(200)
    );
    assert!(load_project(nested.as_bytes()).is_err());
}

#[test]
fn unsafe_and_invalid_values_are_rejected() {
    // Mutations exercise independent semantic boundaries, not only syntax.
    let mutations = [
        ("/schedules/0/enabled", json!(true)),
        ("/schedules/0/interval_ms", json!(0)),
        ("/schedules/0/template_id", json!("absent")),
        ("/templates/0/interface_id", json!("tcp")),
        ("/templates/0/bytes", json!([])),
        ("/interfaces/0/protocol_id", json!("absent")),
        ("/interfaces/0/filter", json!({"ports":[80]})),
        ("/interfaces/3/transport/baud", json!(0)),
        ("/interfaces/4/transport/endpoint", json!(0)),
        ("/interfaces/5/transport/bitrate", json!(0)),
        ("/interfaces/5/filter/can_ids", json!([0x20000000_u32])),
        ("/charts/0/field_id", json!("absent")),
        ("/charts/0/samples", json!(0)),
        ("/charts/0/moving_average", json!(101)),
        ("/protocols/0/fields/0/offset", json!(u32::MAX)),
        ("/protocols/0/fields/0/length", json!(3)),
        ("/protocols/0/name", json!("\n")),
        ("/protocols/0/id", json!("")),
        ("/protocols/0/framing", json!({"kind":"fixed","length":0})),
        (
            "/protocols/0/framing",
            json!({"kind":"length_prefix","offset":u32::MAX,"width":4,"endian":"big","max_length":16}),
        ),
        (
            "/protocols/0/framing",
            json!({"kind":"delimiter","bytes":[],"max_length":8}),
        ),
        (
            "/protocols/0/classification",
            json!({"length":1,"prefix":[1,2]}),
        ),
    ];
    for (pointer, replacement) in mutations {
        let mut value = fixture();
        // Some optional members are absent in the fixture, so set via parent.
        let (parent, key) = pointer.rsplit_once('/').unwrap();
        value.pointer_mut(parent).unwrap()[key] = replacement;
        let error = load(&value).expect_err(pointer).to_string();
        assert!(error.len() < 256, "{pointer}: {error}");
    }
    let mut config = load(&fixture()).unwrap();
    config.protocols[0].fields[0].scale = f64::INFINITY;
    assert!(save_project(&config).is_err());
    config = load(&fixture()).unwrap();
    config.protocols.push(config.protocols[0].clone());
    assert!(save_project(&config).is_err());
    config = load(&fixture()).unwrap();
    config.protocols[0].fields[0].unit = "x".repeat(33);
    assert!(save_project(&config).is_err());
    config = load(&fixture()).unwrap();
    config.interfaces[0].filter.ports = vec![1; MAX_ITEMS + 1];
    assert!(save_project(&config).is_err());
}

#[test]
fn collection_payload_and_output_caps_are_enforced() {
    let mut config = load(&fixture()).unwrap();
    config.templates[0].bytes = vec![255; MAX_BYTES as usize];
    assert!(save_project(&config).is_ok());
    config.templates[0].bytes.push(0);
    assert!(save_project(&config).is_err());
    config = load(&fixture()).unwrap();
    let protocol = config.protocols[0].clone();
    config.protocols = (0..MAX_ITEMS)
        .map(|n| Protocol {
            id: format!("p{n}"),
            ..protocol.clone()
        })
        .collect();
    config.interfaces.clear();
    config.charts.clear();
    config.templates.clear();
    config.schedules.clear();
    assert!(save_project(&config).is_ok());
    config.protocols.push(Protocol {
        id: "one_too_many".into(),
        ..protocol
    });
    assert!(save_project(&config).is_err());
    config = load(&fixture()).unwrap();
    let template = config.templates[0].clone();
    config.templates = (0..10)
        .map(|n| Template {
            id: format!("t{n}"),
            bytes: vec![255; MAX_BYTES as usize],
            ..template.clone()
        })
        .collect();
    config.schedules.clear();
    assert!(matches!(save_project(&config), Err(LoadError::TooLarge)));
}

#[test]
fn remaining_schema_variants_and_nested_unknown_fields() {
    for transfer in ["hid", "bulk", "interrupt"] {
        for parity in ["none", "odd", "even"] {
            let mut value = fixture();
            value["interfaces"][4]["transport"]["transfer"] = json!(transfer);
            value["interfaces"][3]["transport"]["parity"] = json!(parity);
            value["interfaces"][5]["transport"]["fd"] = json!(true);
            value["interfaces"][2]["filter"] = json!({"direction":"Unknown","ports":[0,65535]});
            assert!(load(&value).is_ok());
        }
    }
    for representation in ["bytes", "ascii"] {
        let mut value = fixture();
        value["charts"] = json!([]);
        value["protocols"][0]["fields"][0]["representation"] = json!(representation);
        assert!(load(&value).is_ok());
    }
    for pointer in [
        "/interfaces/0/transport",
        "/protocols/0/framing",
        "/protocols/0/fields/0",
    ] {
        let mut value = fixture();
        value.pointer_mut(pointer).unwrap()["unexpected"] = json!(true);
        assert!(load(&value).is_err(), "{pointer}");
    }
    let project = load(&fixture()).unwrap();
    assert!(matches!(
        load_bindings(br#"{"schema_version":2}"#, &project),
        Err(LoadError::Version(2))
    ));
    assert!(load_bindings(br#"{"schema_version":1,"bindings":[{"interface_id":"serial","selector":"safe","extra":0}]}"#, &project).is_err());
}

#[test]
fn documented_examples_are_loadable() {
    let document = include_str!("../../../docs/configuration.md");
    let examples: Vec<_> = document
        .split("```json\n")
        .skip(1)
        .map(|part| part.split("```").next().unwrap().as_bytes())
        .collect();
    assert_eq!(examples.len(), 2);
    let project = load_project(examples[0]).unwrap();
    assert_eq!(
        load_bindings(examples[1], &project).unwrap().bindings.len(),
        1
    );
}
