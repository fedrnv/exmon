//! Fixture integrity only; this is not the production protocol implementation.
use serde_json::{Value, json};
use std::collections::BTreeSet;

fn bytes(value: &Value) -> Vec<u8> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|n| u8::try_from(n.as_u64().unwrap()).unwrap())
        .collect()
}

#[test]
fn corpus_is_synthetic_complete_bounded_and_ordered() {
    let corpus: Value =
        serde_json::from_str(include_str!("../../../fixtures/traffic/corpus.json")).unwrap();
    assert_eq!(corpus["schema_version"], 1);
    assert_eq!(corpus["provenance"], "generated_synthetic_constants");
    assert_eq!(corpus["license"], "CC0-1.0");
    let cases = corpus["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 132);
    let mut ids = BTreeSet::new();
    for case in cases {
        assert!(ids.insert(case["id"].as_str().unwrap()));
        let observations = case["observations"].as_array().unwrap();
        assert!(!observations.is_empty() && observations.len() <= 8);
        for (index, observation) in observations.iter().enumerate() {
            assert_eq!(observation["sequence"], index);
            assert_eq!(observation["direction"], "Rx");
            assert!(bytes(&observation["bytes"]).len() <= 4);
            assert_eq!(
                observation["timestamp_ns"],
                if case["scenario"] == "timestamped" {
                    123456789
                } else {
                    index * 1000
                }
            );
        }
    }
    for transport in ["ethernet", "tcp", "udp", "serial", "usb", "can"] {
        for framing in ["datagram", "fixed", "delimiter", "length_prefix"] {
            if framing == "datagram" && ["tcp", "serial"].contains(&transport) {
                continue;
            }
            for scenario in [
                "normal",
                "fragmented",
                "malformed",
                "truncated",
                "burst",
                "timestamped",
            ] {
                assert!(ids.contains(format!("{transport}-{framing}-{scenario}").as_str()));
            }
        }
    }
}

#[test]
fn independent_reference_matches_all_expected_messages_and_fields() {
    let corpus: Value =
        serde_json::from_str(include_str!("../../../fixtures/traffic/corpus.json")).unwrap();
    for case in corpus["cases"].as_array().unwrap() {
        let mut pending = Vec::new();
        let mut candidates = Vec::new();
        let mut truncated = false;
        let mut incomplete_datagrams = false;
        let mode = case["framing"].as_str().unwrap();
        for observation in case["observations"].as_array().unwrap() {
            let chunk = bytes(&observation["bytes"]);
            if observation["original_length"].as_u64().unwrap() > chunk.len() as u64 {
                truncated = true;
                pending.clear();
                continue;
            }
            if mode == "datagram" {
                incomplete_datagrams |= chunk.len() != 3;
                candidates.push(chunk);
                continue;
            }
            pending.extend(chunk);
            loop {
                let range = match mode {
                    "fixed" if pending.len() >= 3 => Some((0, 3, 3)),
                    "delimiter" => pending
                        .iter()
                        .position(|b| *b == 10)
                        .map(|end| (0, end, end + 1)),
                    "length_prefix"
                        if !pending.is_empty() && pending.len() > usize::from(pending[0]) =>
                    {
                        let end = 1 + usize::from(pending[0]);
                        Some((1, end, end))
                    }
                    _ => None,
                };
                let Some((start, end, consumed)) = range else {
                    break;
                };
                candidates.push(pending[start..end].to_vec());
                pending.drain(..consumed);
            }
        }
        let expected: Vec<_> = candidates
            .into_iter()
            .filter(|m| m.len() == 3 && m[0] == 165)
            .map(|m| {
                let value = u16::from_be_bytes([m[1], m[2]]);
                json!({"type_id":"synthetic_value", "bytes":m, "fields":{"value":value}})
            })
            .collect();
        assert_eq!(json!(expected), case["expected"], "{}", case["id"]);
        let outcome = if truncated {
            "capture_truncated"
        } else if incomplete_datagrams {
            "incomplete_datagrams"
        } else if expected.is_empty() {
            "unclassified"
        } else {
            "complete"
        };
        assert_eq!(outcome, case["outcome"].as_str().unwrap(), "{}", case["id"]);
    }
}
