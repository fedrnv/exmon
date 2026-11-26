//! Manual deterministic mutation campaign with replayable case indices.
use exmon_domain::{
    config::{Endian, Escape, Framing, Protocol},
    fields::Literal,
};
use exmon_protocol::{
    checksum::Registry,
    codec::{Codec, Edit},
    framing::{Event, Framer, Source},
    network::Decoder,
};
use exmon_storage::pcap::{Limits, Reader};
use serde_json::{Value, json};
use std::{
    io::Cursor,
    time::{Duration, Instant},
};

fn mutate(seed: &[u8], case: u64) -> Vec<u8> {
    let mut bytes = seed.to_vec();
    let mut state = case.wrapping_add(0x034c_a5e5);
    for _ in 0..1 + case % 8 {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        if bytes.is_empty() {
            bytes.push(state as u8);
        }
        let i = state as usize % bytes.len();
        match state % 4 {
            0 => bytes[i] ^= 1 << (state % 8),
            1 => bytes[i] = (state >> 24) as u8,
            2 => bytes.truncate(i),
            _ if bytes.len() < 4096 => bytes.extend_from_within(..bytes.len().min(32)),
            _ => {}
        }
    }
    bytes
}

fn network(bytes: &[u8], _: u64) {
    let packet = Decoder::default().decode(bytes);
    assert_eq!(packet.raw(), bytes);
    let range = packet.payload_range();
    assert!(range.start <= range.end && range.end <= bytes.len());
}

fn capture(bytes: &[u8], _: u64) {
    let limits = Limits {
        block_bytes: 8192,
        interfaces: 4,
        total_bytes: 65536,
        entries: 64,
    };
    if let Ok(mut reader) = Reader::new(Cursor::new(bytes), limits) {
        for _ in 0..66 {
            let before = reader.position();
            match reader.next_entry() {
                Ok(Some(entry)) => {
                    assert!(reader.position() > before);
                    assert!(entry.raw().len() <= 8192);
                    if let Some(data) = entry.payload() {
                        assert!(data.len() <= 8192);
                    }
                }
                _ => return,
            }
        }
        panic!("capture entry budget bypass");
    }
}

fn framing(bytes: &[u8], case: u64) {
    let rules = [
        Framing::Datagram {},
        Framing::Fixed { length: 8 },
        Framing::Delimiter {
            bytes: vec![10],
            max_length: 64,
        },
        Framing::LengthPrefix {
            offset: 0,
            width: 2,
            endian: Endian::Little,
            max_length: 64,
        },
        Framing::Markers {
            start: vec![165],
            end: vec![90],
            max_length: 64,
        },
        Framing::IdleGap {
            gap_ns: 10,
            max_length: 64,
        },
    ];
    for (i, rule) in rules.into_iter().enumerate() {
        let escape = (case % 2 == 1 && i != 0).then_some(Escape { byte: 125, xor: 32 });
        let mut framer = Framer::new(rule, escape).unwrap();
        for (n, chunk) in bytes.chunks(17).enumerate() {
            let source = Source {
                observation: n as u64,
                offset: 0,
            };
            let events = if i == 0 {
                framer.push_frame(source, n as u64, chunk)
            } else {
                framer.push(source, n as u64, chunk)
            };
            for event in events.unwrap() {
                if let Event::Message(message) = event {
                    assert!(message.bytes().len() <= 65536);
                    assert!(message.wire.start <= message.wire.end);
                }
            }
            assert!(framer.stats().pending_decoded <= 65536);
            assert!(framer.stats().pending_source_bytes <= 131072);
        }
        let _ = framer.finish(bytes.len() as u64 + 100);
    }
}

fn codec(bytes: &[u8], case: u64) {
    let mut spec = json!({
        "id":"synthetic","name":"Synthetic","framing":{"kind":"datagram"},
        "fields":[{"id":"value","offset":0,"length":2,"representation":"unsigned","endian":"big"}],
        "checksums":[{"id":"sum","algorithm":"sum8","offset":2,"length":1,"endian":"big","ranges":[{"offset":0,"length":2}]}]
    });
    if case.is_multiple_of(3) {
        spec["fields"][0]["offset"] = json!(case as u32);
        spec["fields"][0]["length"] = json!(bytes.first().copied().unwrap_or(0));
        spec["checksums"][0]["ranges"][0]["length"] = json!(case as u32);
    }
    if let Ok(p) = serde_json::from_value::<Protocol>(spec)
        && let Ok(codec) = Codec::new(&p, &Registry::default())
        && let Ok(mut draft) = codec.decode(bytes)
    {
        assert_eq!(draft.encode(), bytes);
        let _ = draft.inspect();
        let before = draft.encode();
        let literal = Literal::Unsigned { value: case };
        if draft
            .edit(&[Edit::Field {
                index: 0,
                value: &literal,
            }])
            .is_err()
        {
            assert_eq!(draft.encode(), before);
        }
        let _ = draft.repair_checksums();
        let _ = draft.prepare();
        assert_eq!(draft.encode().len(), bytes.len());
    }
}

fn seeds() -> Vec<Vec<u8>> {
    let corpus: Value =
        serde_json::from_str(include_str!("../../../fixtures/traffic/corpus.json")).unwrap();
    let mut result: Vec<Vec<u8>> = corpus["cases"]
        .as_array()
        .unwrap()
        .iter()
        .take(32)
        .flat_map(|c| c["observations"].as_array().unwrap())
        .map(|o| {
            o["bytes"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as u8)
                .collect()
        })
        .collect();
    let mut ethernet = vec![0; 64];
    ethernet[12..16].copy_from_slice(&[8, 0, 0x45, 0]);
    ethernet[16..18].copy_from_slice(&50u16.to_be_bytes());
    ethernet[23] = 17;
    result.push(ethernet);
    let mut tcp = result.last().unwrap().clone();
    tcp[23] = 6;
    tcp[46] = 0x50;
    result.push(tcp);
    let mut ipv6 = vec![0; 80];
    ipv6[12..14].copy_from_slice(&[0x86, 0xdd]);
    ipv6[14] = 0x60;
    ipv6[18..20].copy_from_slice(&26u16.to_be_bytes());
    ipv6[20] = 17;
    result.push(ipv6);
    result.push(vec![0xff; 4096]);
    result.push(Vec::new());
    result
}

fn capture_seeds() -> Vec<Vec<u8>> {
    let mut pcap = vec![
        0xd4, 0xc3, 0xb2, 0xa1, 2, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xff, 0xff, 0, 0, 1, 0, 0, 0,
    ];
    pcap.extend([0, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 3, 0, 0, 0, 165, 0, 42]);
    let ng = vec![
        10, 13, 13, 10, 28, 0, 0, 0, 77, 60, 43, 26, 1, 0, 0, 0, 255, 255, 255, 255, 255, 255, 255,
        255, 28, 0, 0, 0, 1, 0, 0, 0, 20, 0, 0, 0, 1, 0, 0, 0, 0xff, 0xff, 0, 0, 20, 0, 0, 0,
    ];
    vec![pcap, ng]
}

#[test]
fn deterministic_parser_smoke() {
    for (i, seed) in seeds().into_iter().enumerate() {
        network(&seed, i as u64);
        framing(&seed, i as u64);
        codec(&seed, i as u64);
    }
    for seed in capture_seeds() {
        capture(&seed, 0);
    }
    let protocol: Protocol = serde_json::from_value(json!({
        "id":"p","name":"Synthetic","framing":{"kind":"datagram"},
        "fields":[{"id":"value","offset":0,"length":2,"representation":"unsigned","endian":"big"}],
        "checksums":[{"id":"sum","algorithm":"sum8","offset":2,"length":1,"endian":"big","ranges":[{"offset":0,"length":2}]}]
    })).unwrap();
    let engine = Codec::new(&protocol, &Registry::default()).unwrap();
    let mut draft = engine.decode(&[0, 42, 42]).unwrap();
    assert!(draft.inspect().is_valid());
    draft
        .edit(&[Edit::Field {
            index: 0,
            value: &Literal::Unsigned { value: 256 },
        }])
        .unwrap();
    draft.repair_checksums().unwrap();
    assert_eq!(draft.prepare().unwrap().bytes(), &[1, 0, 1]);
}

#[test]
#[ignore = "manual supervised mutation campaign"]
fn mutation_campaign() {
    type Target = fn(&[u8], u64);
    let families: [(&str, Target); 4] = [
        ("network", network),
        ("capture", capture),
        ("framing", framing),
        ("codec", codec),
    ];
    for (name, target) in families {
        let seeds = if name == "capture" {
            capture_seeds()
        } else {
            seeds()
        };
        let begin = Instant::now();
        let mut case = 0;
        while begin.elapsed() < Duration::from_secs(15) || case < 256 {
            let seed = &seeds[case as usize % seeds.len()];
            let bytes = mutate(seed, case);
            if let Err(panic) = std::panic::catch_unwind(|| target(&bytes, case)) {
                eprintln!(
                    "failure family={name} case={case} seed_index={}",
                    case as usize % seeds.len()
                );
                std::panic::resume_unwind(panic);
            }
            case += 1;
        }
        println!(
            "family={name} seconds={:.3} cases={case} pass=true",
            begin.elapsed().as_secs_f64()
        );
    }
}
