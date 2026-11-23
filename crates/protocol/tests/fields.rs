use exmon_domain::{
    config::{Field, ProjectConfig},
    fields::{
        Bits, Constraints, DefCode, EnumEntry, Literal as V, Number as N, NumericError,
        Operation as Op,
    },
};
use exmon_protocol::fields::*;
use serde_json::{Value, json};
fn field(id: &str, offset: u32, length: u32, representation: &str, endian: &str) -> Field {
    serde_json::from_value(json!({"id":id,"offset":offset,"length":length,"representation":representation,"endian":endian})).unwrap()
}
fn unsigned(v: u64) -> V {
    V::Unsigned { value: v }
}
fn signed(v: i64) -> V {
    V::Signed { value: v }
}
fn derived(id: &str, ops: Vec<Op>) -> Field {
    let mut f = field(id, 0, 0, "derived", "little");
    f.derived = Some(ops);
    f
}
fn reference(id: &str) -> Op {
    Op::Field { id: id.into() }
}
fn literal(v: u64) -> Op {
    Op::Literal { value: unsigned(v) }
}
#[test]
fn exact_integer_vectors_all_widths_orders_and_guard_bytes() {
    for width in [1u32, 2, 4, 8] {
        for endian in ["little", "big"] {
            let bits = width * 8;
            let max = if bits == 64 {
                u64::MAX
            } else {
                (1u64 << bits) - 1
            };
            let f = field("u", 1, width, "unsigned", endian);
            let schema = Schema::new(&[f], width + 2).unwrap();
            for value in [0, 1, max, 0xfedcba9876543210 & max] {
                let all = if endian == "little" {
                    value.to_le_bytes()
                } else {
                    value.to_be_bytes()
                };
                let expected = if endian == "little" {
                    &all[..width as usize]
                } else {
                    &all[8 - width as usize..]
                };
                let mut wire = vec![0xcc];
                wire.extend_from_slice(expected);
                wire.push(0xdd);
                let report = schema.decode(&wire).unwrap();
                assert_eq!(report[0].raw, Some(unsigned(value)));
                assert_eq!(report[0].number, Ok(N::Integer(i128::from(value))));
                let mut encoded = vec![0xaa; wire.len()];
                schema
                    .encode_field(0, &report[0].raw.clone().unwrap(), &mut encoded)
                    .unwrap();
                assert_eq!(&encoded[1..1 + width as usize], expected);
                assert_eq!(encoded[0], 0xaa);
                assert_eq!(*encoded.last().unwrap(), 0xaa);
                let before = wire.clone();
                schema.encode_field(0, &unsigned(value), &mut wire).unwrap();
                assert_eq!(wire, before);
            }
            let f = field("i", 0, width, "signed", endian);
            let schema = Schema::new(&[f], width).unwrap();
            let min = -(1i128 << (bits - 1));
            let max = (1i128 << (bits - 1)) - 1;
            for value in [min as i64, -1, 0, 1, max as i64] {
                let raw = if endian == "little" {
                    value.to_le_bytes()
                } else {
                    value.to_be_bytes()
                };
                let wire = if endian == "little" {
                    raw[..width as usize].to_vec()
                } else {
                    raw[8 - width as usize..].to_vec()
                };
                let report = schema.decode(&wire).unwrap();
                assert_eq!(report[0].raw, Some(signed(value)));
                let mut out = vec![0; wire.len()];
                schema.encode_field(0, &signed(value), &mut out).unwrap();
                assert_eq!(out, wire);
            }
        }
    }
}
#[test]
fn ieee_patterns_negative_zero_nan_payloads_and_nonfinite_projection_are_preserved() {
    for endian in ["little", "big"] {
        let schema = Schema::new(&[field("f", 0, 4, "float", endian)], 4).unwrap();
        for bits in [
            0u32,
            0x80000000,
            0x3f800000,
            0x3dcccccd,
            0x7f800000,
            0xff800000,
            0x7fc12345,
            0x7f812345,
            1,
            u32::MAX,
        ] {
            let raw = if endian == "little" {
                bits.to_le_bytes()
            } else {
                bits.to_be_bytes()
            };
            let report = schema.decode(&raw).unwrap();
            assert_eq!(report[0].raw, Some(V::Float32 { bits }));
            if !f32::from_bits(bits).is_finite() {
                assert_eq!(report[0].number, Err(Issue::NonFinite));
            }
            let mut out = [0; 4];
            schema
                .encode_field(0, &report[0].raw.clone().unwrap(), &mut out)
                .unwrap();
            assert_eq!(out, raw);
        }
        let schema = Schema::new(&[field("f", 0, 8, "float", endian)], 8).unwrap();
        for bits in [
            0u64,
            0x8000000000000000,
            0x3ff0000000000000,
            0x7ff0000000000000,
            0xfff0000000000000,
            0x7ff8123456789abc,
            0x7ff0123456789abc,
            1,
            u64::MAX,
        ] {
            let raw = if endian == "little" {
                bits.to_le_bytes()
            } else {
                bits.to_be_bytes()
            };
            let report = schema.decode(&raw).unwrap();
            assert_eq!(report[0].raw, Some(V::Float64 { bits }));
            let mut out = [0; 8];
            schema
                .encode_field(0, &report[0].raw.clone().unwrap(), &mut out)
                .unwrap();
            assert_eq!(out, raw);
        }
    }
    let mut f = field("finite", 0, 4, "float", "little");
    f.constraints.finite = true;
    let schema = Schema::new(&[f], 4).unwrap();
    let bits = 0x7fc12345u32;
    let report = schema.decode(&bits.to_le_bytes()).unwrap();
    assert!(report[0].violations.contains(&Issue::NonFinite));
    let mut out = [0xaa; 4];
    let before = out;
    assert_eq!(
        schema
            .encode_field(0, &V::Float32 { bits }, &mut out)
            .unwrap_err()
            .issue,
        Issue::NonFinite
    );
    assert_eq!(out, before);
}
#[test]
fn exhaustive_bit_widths_and_offsets_match_an_independent_physical_bit_reference() {
    for endian in ["little", "big"] {
        for width in 1u8..=64 {
            for shift in 0..=64 - width {
                let mask = if width == 64 {
                    u64::MAX
                } else {
                    (1u64 << width) - 1
                };
                let mut f = field("bits", 1, 8, "unsigned", endian);
                f.bits = Some(Bits {
                    offset: shift,
                    width,
                });
                let schema = Schema::new(&[f], 10).unwrap();
                for value in [0, mask, 0xaaaaaaaaaaaaaaaa & mask] {
                    let mut wire = [0x5a; 10];
                    let mut expected = wire;
                    for bit in 0..width {
                        let logical = shift + bit;
                        let index = 1 + if endian == "little" {
                            usize::from(logical / 8)
                        } else {
                            7 - usize::from(logical / 8)
                        };
                        let flag = 1u8 << (logical % 8);
                        if value & (1u64 << bit) == 0 {
                            expected[index] &= !flag;
                        } else {
                            expected[index] |= flag;
                        }
                    }
                    schema.encode_field(0, &unsigned(value), &mut wire).unwrap();
                    assert_eq!(wire, expected);
                    assert_eq!(schema.decode(&wire).unwrap()[0].raw, Some(unsigned(value)));
                }
            }
        }
    }
    for width in [1, 3, 12, 63, 64] {
        let mut f = field("signed_bits", 0, 8, "signed", "big");
        f.bits = Some(Bits { offset: 0, width });
        let schema = Schema::new(&[f], 8).unwrap();
        for value in [-(1i128 << (width - 1)), -1, 0, (1i128 << (width - 1)) - 1] {
            let mut wire = [0xa5; 8];
            schema
                .encode_field(0, &signed(value as i64), &mut wire)
                .unwrap();
            assert_eq!(
                schema.decode(&wire).unwrap()[0].raw,
                Some(signed(value as i64))
            );
        }
    }
}
#[test]
fn physical_overlap_validation_allows_disjoint_bits_across_endian_containers_only() {
    let mut a = field("a", 0, 2, "unsigned", "little");
    a.bits = Some(Bits {
        offset: 0,
        width: 4,
    });
    let mut b = field("b", 0, 2, "unsigned", "big");
    b.bits = Some(Bits {
        offset: 0,
        width: 4,
    });
    let schema = Schema::new(&[a.clone(), b.clone()], 2).unwrap();
    let mut wire = [0xab, 0xcd];
    let decoded = schema.decode(&wire).unwrap();
    assert_eq!(decoded[0].raw, Some(unsigned(11)));
    assert_eq!(decoded[1].raw, Some(unsigned(13)));
    schema.encode_field(1, &unsigned(2), &mut wire).unwrap();
    assert_eq!(wire, [0xab, 0xc2]);
    b.bits = Some(Bits {
        offset: 8,
        width: 4,
    });
    assert_eq!(
        Schema::new(&[a.clone(), b], 2).unwrap_err().code,
        DefCode::Overlap
    );
    assert_eq!(
        Schema::new(&[a, field("full", 0, 1, "unsigned", "little")], 2)
            .unwrap_err()
            .code,
        DefCode::Overlap
    );
}
#[test]
fn byte_ascii_utf8_values_are_exact_and_invalid_text_is_never_replaced() {
    for (kind, wire, value) in [
        (
            "bytes",
            vec![0, 255, 1],
            V::Bytes {
                value: vec![0, 255, 1],
            },
        ),
        (
            "ascii",
            b"A\0Z".to_vec(),
            V::Text {
                value: "A\0Z".into(),
            },
        ),
        (
            "utf8",
            "é€".as_bytes().to_vec(),
            V::Text {
                value: "é€".into()
            },
        ),
    ] {
        let schema = Schema::new(
            &[field("value", 0, wire.len() as u32, kind, "little")],
            wire.len() as u32,
        )
        .unwrap();
        let report = schema.decode(&wire).unwrap();
        assert_eq!(report[0].raw, Some(value.clone()));
        assert_eq!(report[0].number, Err(Issue::NotNumeric));
        let mut out = vec![0; wire.len()];
        schema.encode_field(0, &value, &mut out).unwrap();
        assert_eq!(out, wire);
        let before = out.clone();
        assert!(
            schema
                .encode_field(
                    0,
                    &V::Text {
                        value: "wrong length".into()
                    },
                    &mut out
                )
                .is_err()
        );
        assert_eq!(out, before);
    }
    for kind in ["ascii", "utf8"] {
        let schema = Schema::new(&[field("text", 0, 2, kind, "big")], 2).unwrap();
        let report = schema.decode(&[0xff, 0xfe]).unwrap();
        assert_eq!(report[0].raw, None);
        assert_eq!(report[0].number, Err(Issue::Text));
        assert_eq!(report[0].violations, [Issue::Text]);
    }
}
#[test]
fn constants_enum_labels_and_raw_ranges_enforce_transactional_edits() {
    let mut f = field("state", 0, 1, "unsigned", "little");
    f.constant = Some(unsigned(5));
    f.enumeration = vec![
        EnumEntry {
            value: unsigned(5),
            name: "OK".into(),
        },
        EnumEntry {
            value: unsigned(10),
            name: "WARN".into(),
        },
    ];
    f.constraints = Constraints {
        min: Some(unsigned(0)),
        max: Some(unsigned(10)),
        enum_closed: true,
        ..Default::default()
    };
    let schema = Schema::new(&[f], 1).unwrap();
    let result = schema.decode(&[5]).unwrap();
    assert_eq!(result[0].label.as_deref(), Some("OK"));
    assert!(result[0].violations.is_empty());
    let result = schema.decode(&[10]).unwrap();
    assert_eq!(result[0].label.as_deref(), Some("WARN"));
    assert_eq!(result[0].number, Err(Issue::Constant));
    let mut out = [9];
    let before = out;
    assert_eq!(
        schema.encode_label(0, "WARN", &mut out).unwrap_err().issue,
        Issue::Constant
    );
    assert_eq!(out, before);
    schema.encode_label(0, "OK", &mut out).unwrap();
    assert_eq!(out, [5]);
    assert_eq!(
        schema
            .encode_field(0, &signed(5), &mut out)
            .unwrap_err()
            .issue,
        Issue::Type
    );
    let mut f = field("range", 0, 8, "unsigned", "big");
    f.constraints.min = Some(unsigned(u64::MAX - 1));
    f.constraints.max = Some(unsigned(u64::MAX));
    let schema = Schema::new(&[f], 8).unwrap();
    assert_eq!(
        schema.decode(&u64::MAX.to_be_bytes()).unwrap()[0].number,
        Ok(N::Integer(i128::from(u64::MAX)))
    );
    assert_eq!(
        schema.decode(&(u64::MAX - 2).to_be_bytes()).unwrap()[0].number,
        Err(Issue::Range)
    );
}
#[test]
fn engineering_scaling_quantization_and_large_integers_are_explicit() {
    let mut f = field("temperature", 0, 2, "signed", "little");
    f.scale = 0.1;
    f.bias = -40.0;
    f.unit = "°C".into();
    let schema = Schema::new(&[f], 2).unwrap();
    assert_eq!(
        schema.decode(&300i16.to_le_bytes()).unwrap()[0].number,
        Ok(N::Real(-10.0))
    );
    let mut out = [0; 2];
    assert_eq!(
        schema
            .encode_number(0, N::Real(-10.0), 0.0, &mut out)
            .unwrap(),
        N::Real(-10.0)
    );
    assert_eq!(out, 300i16.to_le_bytes());
    assert_eq!(schema.field(0).unwrap().unit, "°C");
    let mut f = field("scaled", 0, 1, "unsigned", "little");
    f.scale = 0.1;
    let schema = Schema::new(&[f], 1).unwrap();
    let mut out = [0xaa];
    assert_eq!(
        schema
            .encode_number(0, N::Real(0.3), 0.0, &mut out)
            .unwrap_err()
            .issue,
        Issue::Quantization
    );
    assert_eq!(out, [0xaa]);
    let actual = schema
        .encode_number(0, N::Real(0.3), 1e-12, &mut out)
        .unwrap();
    assert_eq!(out, [3]);
    assert_eq!(actual, N::Real(0.30000000000000004));
    let schema = Schema::new(&[field("single", 0, 4, "float", "big")], 4).unwrap();
    let mut out = [0; 4];
    assert_eq!(
        schema
            .encode_number(0, N::Real(0.1), 0.0, &mut out)
            .unwrap_err()
            .issue,
        Issue::Quantization
    );
    let actual = schema
        .encode_number(0, N::Real(0.1), 1e-8, &mut out)
        .unwrap();
    assert_eq!(out, 0x3dcccccdu32.to_be_bytes());
    assert_eq!(actual, N::Real(f64::from(0.1f32)));
    let schema = Schema::new(&[field("counter", 0, 8, "unsigned", "little")], 8).unwrap();
    let mut out = [0; 8];
    assert_eq!(
        schema
            .encode_number(0, N::Integer(i128::from(u64::MAX)), 0.0, &mut out)
            .unwrap(),
        N::Integer(i128::from(u64::MAX))
    );
    assert_eq!(out, [255; 8]);
    let before = out;
    assert_eq!(
        schema
            .encode_number(0, N::Real(18446744073709551616.0), 0.0, &mut out)
            .unwrap_err()
            .issue,
        Issue::Range
    );
    assert_eq!(out, before);
    let mut f = field("zero", 0, 8, "unsigned", "little");
    f.scale = 0.0;
    f.bias = 12.5;
    let schema = Schema::new(&[f], 8).unwrap();
    assert_eq!(
        schema.decode(&[255; 8]).unwrap()[0].number,
        Ok(N::Real(12.5))
    );
    assert_eq!(
        schema
            .encode_number(0, N::Real(12.5), 0.0, &mut out)
            .unwrap_err()
            .issue,
        Issue::NonInvertible
    );
}
#[test]
fn derived_forward_references_arithmetic_validity_and_failures_propagate_without_zero() {
    let sum = derived("sum", vec![reference("counter"), literal(1), Op::Add]);
    let count = field("counter", 0, 8, "unsigned", "little");
    let schema = Schema::new(&[sum.clone(), count.clone()], 8).unwrap();
    let report = schema.decode(&[255; 8]).unwrap();
    assert_eq!(report[0].number, Ok(N::Integer(i128::from(u64::MAX) + 1)));
    assert_eq!(report[1].raw, Some(unsigned(u64::MAX)));
    let report = schema.decode(&[255; 7]).unwrap();
    assert_eq!(report[1].number, Err(Issue::Bounds));
    assert_eq!(report[0].number, Err(Issue::Dependency));
    let mut out = [0xaa; 8];
    let before = out;
    assert_eq!(
        schema
            .encode_field(0, &unsigned(1), &mut out)
            .unwrap_err()
            .issue,
        Issue::ReadOnly
    );
    assert_eq!(out, before);
    let overflow = derived(
        "overflow",
        vec![reference("counter"), reference("counter"), Op::Mul],
    );
    let zero = derived("zero", vec![literal(1), literal(0), Op::Div]);
    let fraction = derived("fraction", vec![literal(1), literal(2), Op::Div]);
    let schema = Schema::new(&[count.clone(), overflow, zero, fraction], 8).unwrap();
    let report = schema.decode(&[255; 8]).unwrap();
    assert_eq!(report[1].number, Err(Issue::Arithmetic));
    assert_eq!(report[2].number, Err(Issue::Arithmetic));
    assert_eq!(report[3].number, Ok(N::Real(0.5)));
    let mut restricted = sum;
    restricted.constraints.max = Some(unsigned(2));
    let schema = Schema::new(&[count, restricted], 8).unwrap();
    assert_eq!(
        schema.decode(&[255; 8]).unwrap()[1].number,
        Err(Issue::Range)
    );
    let a = derived("a", vec![reference("b")]);
    let b = derived("b", vec![reference("a")]);
    assert_eq!(Schema::new(&[a, b], 8).unwrap_err().code, DefCode::Cycle);
    assert_eq!(
        Schema::new(&[derived("missing", vec![reference("absent")])], 8)
            .unwrap_err()
            .code,
        DefCode::Reference
    );
    assert_eq!(
        Schema::new(&[derived("bad", vec![Op::Add])], 8)
            .unwrap_err()
            .code,
        DefCode::Expression
    );
}
#[test]
fn numeric_comparison_and_conversion_never_saturate_or_round_large_integers_silently() {
    use std::cmp::Ordering::*;
    for (a, b, order) in [
        (
            N::Integer(i128::from(u64::MAX)),
            N::Real(18446744073709551616.0),
            Less,
        ),
        (N::Integer(i128::MAX), N::Real(i128::MAX as f64), Less),
        (N::Integer(i128::MIN), N::Real(i128::MIN as f64), Equal),
        (N::Integer(-1), N::Real(-0.5), Less),
        (N::Integer(0), N::Real(-0.5), Greater),
        (N::Integer(1), N::Real(1.5), Less),
    ] {
        assert_eq!(a.compare(b), Ok(order));
        assert_eq!(b.compare(a), Ok(order.reverse()));
    }
    assert_eq!(
        N::Integer(i128::from(u64::MAX)).to_f64(),
        Err(NumericError::PrecisionLoss)
    );
    assert_eq!(
        N::Integer(i128::MAX).to_f64(),
        Err(NumericError::PrecisionLoss)
    );
    assert_eq!(N::Integer(i128::MIN).to_f64(), Ok(i128::MIN as f64));
    assert_eq!(
        N::Real(f64::NAN).project(1.0, 0.0),
        Err(NumericError::NonFinite)
    );
}
#[test]
fn every_derived_operator_scaled_dependencies_and_enum_constraints_are_defined() {
    let expression = derived(
        "operations",
        vec![
            Op::Literal { value: signed(-7) },
            Op::Abs,
            literal(2),
            Op::Sub,
            literal(4),
            Op::Max,
            literal(10),
            Op::Min,
            Op::Neg,
        ],
    );
    let mut native = field("source", 0, 1, "unsigned", "little");
    native.scale = 0.5;
    native.bias = 1.0;
    let mut physical = derived("physical", vec![reference("source"), literal(2), Op::Mul]);
    physical.scale = 2.0;
    physical.unit = "V".into();
    physical.enumeration = vec![EnumEntry {
        value: unsigned(6),
        name: "Nominal".into(),
    }];
    physical.constraints.enum_closed = true;
    let schema = Schema::new(&[expression, native, physical], 1).unwrap();
    let report = schema.decode(&[4]).unwrap();
    assert_eq!(report[0].number, Ok(N::Integer(-5)));
    assert_eq!(report[1].number, Ok(N::Real(3.0)));
    assert_eq!(report[2].raw_number, Some(N::Real(6.0)));
    assert_eq!(report[2].number, Ok(N::Real(12.0)));
    assert_eq!(report[2].label.as_deref(), Some("Nominal"));
    assert_eq!(schema.decode(&[2]).unwrap()[2].number, Err(Issue::Enum));
    let mut bad = field("closed", 0, 1, "unsigned", "little");
    bad.enumeration = vec![EnumEntry {
        value: unsigned(10),
        name: "Impossible".into(),
    }];
    bad.constraints = Constraints {
        max: Some(unsigned(5)),
        enum_closed: true,
        ..Default::default()
    };
    assert_eq!(
        Schema::new(&[bad], 1).unwrap_err().code,
        DefCode::Constraint
    );
}
#[test]
fn accepted_corpus_runs_through_framing_classification_and_exact_field_values() {
    use exmon_domain::{
        Direction,
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
    let config:ProjectConfig=serde_json::from_value(json!({"schema_version":1,"protocols":[{"id":"synthetic_value","name":"Synthetic value","framing":{"kind":"fixed","length":3},"classification":{"length":3,"prefix":[165]},"fields":[{"id":"value","offset":1,"length":2,"representation":"unsigned","endian":"big"}]}]})).unwrap();
    config.validate().unwrap();
    let classifier = Classifier::new(&config, 1).unwrap();
    let schema = Schema::for_protocol(&config.protocols[0]).unwrap();
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
            let bytes: Vec<u8> = input["bytes"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as u8)
                .collect();
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
        let mut decoded = vec![];
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
                if decision.outcome == Outcome::Matched {
                    let report = schema.decode(message.bytes()).unwrap();
                    let Some(V::Unsigned { value }) = report[0].raw else {
                        panic!()
                    };
                    decoded.push(value);
                    assert_eq!(report[0].number, Ok(N::Integer(i128::from(value))));
                    let mut encoded = message.bytes().to_vec();
                    schema
                        .encode_field(0, &unsigned(value), &mut encoded)
                        .unwrap();
                    assert_eq!(encoded, message.bytes());
                    assert_eq!(encoded[0], 165);
                }
            }
        }
        let expected: Vec<_> = case["expected"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["fields"]["value"].as_u64().unwrap())
            .collect();
        assert_eq!(decoded, expected, "{}", case["id"]);
    }
}
#[test]
fn bounds_types_constraints_and_all_message_prefixes_fail_safely() {
    let fields = vec![
        field("i", 0, 4, "signed", "big"),
        field("f", 4, 8, "float", "little"),
        field("text", 12, 4, "utf8", "big"),
    ];
    let schema = Schema::new(&fields, 16).unwrap();
    let input = [0u8; 16];
    for end in 0..16 {
        let report = schema.decode(&input[..end]).unwrap();
        for (f, r) in fields.iter().zip(report) {
            if f.offset as usize + f.length as usize > end {
                assert_eq!(r.raw, None);
                assert_eq!(r.number, Err(Issue::Bounds));
            }
        }
    }
    assert_eq!(schema.decode(&[0; 17]).unwrap_err().issue, Issue::Input);
    for mut bad in [
        field("bad", u32::MAX, 1, "unsigned", "big"),
        field("bad", 0, u32::MAX, "bytes", "big"),
        field("bad", 0, 0, "unsigned", "big"),
        field("bad", 0, 3, "signed", "big"),
    ] {
        assert!(Schema::new(&[bad.clone()], 16).is_err());
        bad.bits = Some(Bits {
            offset: 255,
            width: 255,
        });
        assert!(!exmon_domain::fields::literal_fits(&bad, &unsigned(1)));
        assert!(Schema::new(&[bad], 16).is_err());
    }
    for bits in [
        Bits {
            offset: 0,
            width: 0,
        },
        Bits {
            offset: 64,
            width: 1,
        },
        Bits {
            offset: 1,
            width: 64,
        },
        Bits {
            offset: 255,
            width: 255,
        },
    ] {
        let mut f = field("bad", 0, 8, "unsigned", "little");
        f.bits = Some(bits);
        assert!(Schema::new(&[f], 8).is_err());
    }
    let mut f = field("constant", 0, 1, "unsigned", "big");
    f.constant = Some(unsigned(256));
    assert_eq!(Schema::new(&[f], 1).unwrap_err().code, DefCode::Constant);
    let mut f = field("range", 0, 1, "unsigned", "big");
    f.constraints.min = Some(unsigned(9));
    f.constraints.max = Some(unsigned(1));
    assert_eq!(Schema::new(&[f], 1).unwrap_err().code, DefCode::Constraint);
    let mut f = field("enum", 0, 1, "unsigned", "big");
    f.enumeration = vec![
        EnumEntry {
            value: unsigned(0),
            name: "A".into(),
        },
        EnumEntry {
            value: unsigned(0),
            name: "B".into(),
        },
    ];
    assert_eq!(Schema::new(&[f], 1).unwrap_err().code, DefCode::Enumeration);
}
#[test]
fn resource_limits_and_deterministic_hostile_wire_values_are_bounded_and_redacted() {
    let definitions: Vec<_> = (0..256)
        .map(|i| field(&format!("f{i}"), i, 1, "unsigned", "big"))
        .collect();
    let schema = Schema::new(&definitions, 256).unwrap();
    assert_eq!(schema.len(), 256);
    assert_eq!(schema.decode(&[0; 256]).unwrap().len(), 256);
    let mut too_many = definitions.clone();
    too_many.push(field("extra", 256, 1, "unsigned", "big"));
    assert_eq!(
        Schema::new(&too_many, 257).unwrap_err().code,
        DefCode::Count
    );
    let mut f = field("large", 0, 65536, "bytes", "little");
    let schema = Schema::new(&[f.clone()], 65536).unwrap();
    let report = schema.decode(&vec![0x5a; 65536]).unwrap();
    assert!(matches!(&report[0].raw,Some(V::Bytes{value})if value.len()==65536));
    f.constant = Some(V::Bytes {
        value: vec![0; 1_048_576],
    });
    assert_eq!(Schema::new(&[f], 65536).unwrap_err().code, DefCode::Budget);
    let mut f = field("enum", 0, 2, "unsigned", "big");
    f.enumeration = (0..257)
        .map(|n| EnumEntry {
            value: unsigned(n),
            name: format!("v{n}"),
        })
        .collect();
    assert_eq!(Schema::new(&[f], 2).unwrap_err().code, DefCode::Enumeration);
    let expression = derived("many", vec![literal(1); 65]);
    assert_eq!(
        Schema::new(&[expression], 1).unwrap_err().code,
        DefCode::Expression
    );
    let schema = Schema::new(&[field("PRIVATE_ID", 0, 8, "unsigned", "big")], 8).unwrap();
    let mut state = 12345u64;
    for _ in 0..4096 {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let data = state.to_be_bytes();
        let result = schema.decode(&data).unwrap();
        assert_eq!(result[0].raw, Some(unsigned(state)));
        let mut out = [0; 8];
        schema
            .encode_field(0, &result[0].raw.clone().unwrap(), &mut out)
            .unwrap();
        assert_eq!(out, data);
        let debug = format!("{schema:?} {:?}", result);
        assert!(!debug.contains("PRIVATE_ID"));
        assert!(!debug.contains(&state.to_string()));
    }
}
#[test]
fn aggregate_definition_budget_graph_stack_and_checked_i128_edges_are_exercised() {
    let mut definitions = vec![];
    for i in 0..256 {
        let mut f = field(&format!("f{i}"), i * 256, 256, "bytes", "little");
        f.enumeration = (0..16)
            .map(|n| {
                let mut value = vec![0; 256];
                value[0] = n;
                EnumEntry {
                    value: V::Bytes { value },
                    name: format!("v{n}"),
                }
            })
            .collect();
        definitions.push(f);
    }
    assert_eq!(
        Schema::new(&definitions, 65536).unwrap_err().code,
        DefCode::Budget
    );
    let base = vec![
        literal(1u64 << 63),
        literal(1u64 << 63),
        Op::Mul,
        Op::Literal { value: signed(-2) },
        Op::Mul,
    ];
    for op in [Op::Neg, Op::Abs] {
        let mut expr = base.clone();
        expr.push(op);
        let schema = Schema::new(&[derived("overflow", expr)], 1).unwrap();
        assert_eq!(
            schema.decode(b"").unwrap()[0].number,
            Err(Issue::Arithmetic)
        );
    }
    let mut expr = base;
    expr.push(Op::Literal { value: signed(-1) });
    expr.push(Op::Div);
    let schema = Schema::new(&[derived("overflow", expr)], 1).unwrap();
    assert_eq!(
        schema.decode(b"").unwrap()[0].number,
        Err(Issue::Arithmetic)
    );
    let mut chain = vec![field("f0", 0, 1, "unsigned", "little")];
    for i in 1..256 {
        chain.push(derived(
            &format!("f{i}"),
            vec![reference(&format!("f{}", i - 1)), literal(1), Op::Add],
        ));
    }
    let schema = Schema::new(&chain, 1).unwrap();
    assert_eq!(
        schema.decode(&[1]).unwrap()[255].number,
        Ok(N::Integer(256))
    );
    let mut wide = vec![literal(1); 32];
    wide.extend(vec![Op::Add; 31]);
    assert_eq!(wide.len(), 63);
    let schema = Schema::new(&[derived("stack", wide)], 1).unwrap();
    assert_eq!(schema.decode(b"").unwrap()[0].number, Ok(N::Integer(32)));
}
#[test]
fn hostile_definition_mutations_and_failed_write_boundaries_never_read_or_change_outside_fields() {
    let baseline = serde_json::to_value(field("n", 1, 8, "unsigned", "little")).unwrap();
    let mut seed = 0x12345678u32;
    for _ in 0..2048 {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        let mut value = baseline.clone();
        value["offset"] = json!(seed);
        value["length"] = json!((seed >> 4) & 15);
        if seed & 1 != 0 {
            value["bits"] = json!({"offset":(seed>>16)&255,"width":(seed>>24)&255});
        }
        let f: Field = serde_json::from_value(value).unwrap();
        if let Ok(schema) = Schema::new(&[f], 16) {
            for end in 0..=16 {
                let input = vec![0xa5; end];
                let _ = schema.decode(&input).unwrap();
                let mut output = input.clone();
                let result = schema.encode_field(0, &unsigned(u64::MAX), &mut output);
                if result.is_err() {
                    assert_eq!(output, input);
                }
            }
        }
    }
    let schema = Schema::new(&[field("n", 1, 2, "unsigned", "big")], 4).unwrap();
    for length in 0..3 {
        let mut out = vec![0xaa; length];
        let before = out.clone();
        assert_eq!(
            schema
                .encode_field(0, &unsigned(1), &mut out)
                .unwrap_err()
                .issue,
            Issue::Bounds
        );
        assert_eq!(out, before);
    }
    let mut out = [0xaa; 4];
    let before = out;
    assert!(
        schema
            .encode_number(0, N::Real(f64::NAN), 0.0, &mut out)
            .is_err()
    );
    assert!(
        schema
            .encode_number(0, N::Integer(1), -1.0, &mut out)
            .is_err()
    );
    assert!(
        schema
            .encode_number(0, N::Integer(1), f64::INFINITY, &mut out)
            .is_err()
    );
    assert!(
        schema
            .encode_field(usize::MAX, &unsigned(1), &mut out)
            .is_err()
    );
    assert_eq!(out, before);
}
