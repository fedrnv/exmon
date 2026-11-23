#[path = "support/filter_wire.rs"]
mod wire_fixture;
use exmon_domain::{
    Direction,
    observation::{Metadata, Observation},
};
use exmon_protocol::tcp_reassembly::*;
use std::{net::SocketAddr, ops::Range};
const BASE: u64 = 1 << 32;
fn endpoints() -> (SocketAddr, SocketAddr) {
    (
        "192.0.2.1:12345".parse().unwrap(),
        "198.51.100.2:443".parse().unwrap(),
    )
}
fn segment(sequence: u32, bytes: &[u8]) -> Segment<'_> {
    let (source, destination) = endpoints();
    Segment {
        source,
        destination,
        vlans: &[],
        sequence,
        acknowledgement: None,
        syn: false,
        fin: false,
        rst: false,
        payload: bytes,
        original_payload_len: bytes.len(),
    }
}
fn reverse(mut s: Segment<'_>) -> Segment<'_> {
    std::mem::swap(&mut s.source, &mut s.destination);
    s
}
fn limits() -> Limits {
    Limits {
        flows: 8,
        total_bytes: 256,
        flow_bytes: 128,
        total_chunks: 16,
        direction_chunks: 8,
        input_bytes: 64,
        sequence_window: 1024,
        gap_timeout_ns: 10,
        idle_timeout_ns: 100,
    }
}
fn engine() -> Reassembler {
    Reassembler::new(limits()).unwrap()
}
fn assert_bounds(r: &Reassembler, events: &[Event]) {
    let l = r.limits();
    let stats = r.stats();
    assert!(stats.flows <= l.flows);
    assert!(stats.buffered_bytes <= l.total_bytes);
    assert!(stats.buffered_chunks <= l.total_chunks);
    assert!(events.len() <= l.output_event_bound());
    let bytes = events
        .iter()
        .map(|e| match &e.kind {
            EventKind::Data { bytes, .. } => bytes.len(),
            _ => 0,
        })
        .sum::<usize>();
    assert!(bytes <= l.total_bytes + l.input_bytes);
    for e in events {
        match &e.kind {
            EventKind::Data { sequence, bytes } => {
                assert_eq!(sequence.end - sequence.start, bytes.len() as u64);
                assert!(!bytes.is_empty());
            }
            EventKind::Gap { sequence, .. }
            | EventKind::Truncated { sequence }
            | EventKind::Overlap { sequence, .. } => assert!(sequence.start < sequence.end),
            _ => {}
        }
    }
}
fn push(r: &mut Reassembler, now: u64, s: Segment<'_>) -> Vec<Event> {
    let events = r.push(Context::default(), now, s).unwrap();
    assert_bounds(r, &events);
    events
}
fn syn(r: &mut Reassembler, now: u64, seq: u32, back: bool) -> Vec<Event> {
    let mut s = segment(seq, b"");
    s.syn = true;
    push(r, now, if back { reverse(s) } else { s })
}
fn data(events: &[Event], side: Side) -> Vec<u8> {
    events
        .iter()
        .filter(|e| e.side == Some(side))
        .flat_map(|e| match &e.kind {
            EventKind::Data { bytes, .. } => bytes.as_slice(),
            _ => &[],
        })
        .copied()
        .collect()
}
fn ranges(events: &[Event]) -> Vec<Range<u64>> {
    events
        .iter()
        .filter_map(|e| match &e.kind {
            EventKind::Gap { sequence, .. } => Some(sequence.clone()),
            _ => None,
        })
        .collect()
}
fn one(range: Range<u64>) -> Vec<Range<u64>> {
    std::iter::once(range).collect()
}
fn closed(events: &[Event]) -> Option<(CloseReason, bool, bool)> {
    events.iter().find_map(|e| match e.kind {
        EventKind::Closed {
            reason,
            incomplete,
            ambiguous,
        } => Some((reason, incomplete, ambiguous)),
        _ => None,
    })
}

#[test]
fn bidirectional_order_fragmentation_and_fin_are_exact() {
    let mut r = engine();
    let opened = syn(&mut r, 0, 100, false);
    assert_eq!(opened.len(), 1);
    assert!(matches!(opened[0].kind, EventKind::Opened { .. }));
    syn(&mut r, 1, 500, true);
    assert!(push(&mut r, 2, segment(104, b"def")).is_empty());
    assert_eq!(r.stats().buffered_bytes, 3);
    let events = push(&mut r, 3, reverse(segment(501, b"reply")));
    assert_eq!(data(&events, Side::BtoA), b"reply");
    let events = push(&mut r, 4, segment(101, b"abc"));
    assert_eq!(data(&events, Side::AtoB), b"abcdef");
    assert!(ranges(&events).is_empty());
    assert_eq!(r.stats().buffered_bytes, 0);
    let mut fin = segment(107, b"");
    fin.fin = true;
    let events = push(&mut r, 5, fin);
    assert!(matches!(events[0].kind,EventKind::HalfClosed {sequence} if sequence==BASE+107));
    assert_eq!(r.stats().flows, 1);
    let mut fin = reverse(segment(506, b""));
    fin.fin = true;
    let events = push(&mut r, 6, fin);
    assert_eq!(closed(&events), Some((CloseReason::Fin, false, false)));
    assert_eq!(r.stats().flows, 0);
    assert_eq!(r.stats().emitted_bytes, 11);
}
#[test]
fn buffered_overlaps_first_seen_wins_and_emitted_history_is_not_fabricated() {
    let mut r = engine();
    syn(&mut r, 0, 100, false);
    push(&mut r, 1, segment(104, b"def"));
    let exact = push(&mut r, 2, segment(104, b"def"));
    assert!(exact.iter().any(|e| matches!(
        e.kind,
        EventKind::Overlap {
            policy: Overlap::IdenticalBuffered,
            ..
        }
    )));
    let conflict = push(&mut r, 3, segment(103, b"XQQQY"));
    assert!(conflict.iter().any(|e| matches!(
        e.kind,
        EventKind::Overlap {
            policy: Overlap::ConflictingBuffered,
            ..
        }
    )));
    let events = push(&mut r, 4, segment(101, b"ab"));
    assert_eq!(data(&events, Side::AtoB), b"abXdefY");
    assert_eq!(r.stats().buffered_chunks, 0);
    let late = push(&mut r, 5, segment(101, b"different"));
    assert_eq!(data(&late, Side::AtoB), b"nt");
    assert!(late.iter().any(|e| matches!(
        e.kind,
        EventKind::Overlap {
            policy: Overlap::UnverifiablePast,
            ..
        }
    )));
    let end = r.finish(6).unwrap();
    assert_eq!(closed(&end), Some((CloseReason::CaptureEnded, true, true)));
}
#[test]
fn holes_recover_before_deadline_and_permanent_gap_precedes_data() {
    let mut r = engine();
    syn(&mut r, 0, 100, false);
    push(&mut r, 1, segment(104, b"def"));
    assert!(r.tick(10).unwrap().is_empty());
    let events = push(&mut r, 10, segment(101, b"abc"));
    assert_eq!(data(&events, Side::AtoB), b"abcdef");
    assert!(r.tick(11).unwrap().is_empty());
    push(&mut r, 12, segment(111, b"tail"));
    let events = r.tick(22).unwrap();
    assert_bounds(&r, &events);
    assert_eq!(ranges(&events), one(BASE + 107..BASE + 111));
    assert!(matches!(
        events[0].kind,
        EventKind::Gap {
            reason: GapReason::Deadline,
            ..
        }
    ));
    assert_eq!(data(&events, Side::AtoB), b"tail");
    let late = push(&mut r, 23, segment(107, b"lost"));
    assert!(data(&late, Side::AtoB).is_empty());
}
#[test]
fn peer_ack_and_capture_truncation_create_precise_recoverable_gaps() {
    let mut r = engine();
    syn(&mut r, 0, 100, false);
    let mut truncated = segment(101, b"ab");
    truncated.original_payload_len = 5;
    truncated.fin = true;
    let events = push(&mut r, 1, truncated);
    assert_eq!(data(&events, Side::AtoB), b"ab");
    assert!(events.iter().any(
        |e| matches!(&e.kind,EventKind::Truncated {sequence} if sequence==&(BASE+103..BASE+106))
    ));
    let events = push(&mut r, 2, segment(103, b"cde"));
    assert!(
        events
            .iter()
            .any(|e| matches!(e.kind,EventKind::HalfClosed {sequence} if sequence==BASE+106))
    );
    assert!(ranges(&events).is_empty());
    let mut r = engine();
    syn(&mut r, 0, 100, false);
    push(&mut r, 1, segment(101, b"ab"));
    let mut ack = reverse(segment(999, b""));
    ack.acknowledgement = Some(106);
    push(&mut r, 2, ack);
    let events = r.tick(12).unwrap();
    assert_eq!(ranges(&events), one(BASE + 103..BASE + 106));
    assert!(data(&events, Side::BtoA).is_empty());
    let events = push(&mut r, 13, segment(106, b"z"));
    assert_eq!(data(&events, Side::AtoB), b"z");
}
#[test]
fn fin_waits_for_missing_bytes_reset_timeout_and_finish_flush_explicit_boundaries() {
    let mut r = engine();
    syn(&mut r, 0, 100, false);
    syn(&mut r, 0, 500, true);
    let mut tail = segment(104, b"de");
    tail.fin = true;
    let events = push(&mut r, 1, tail);
    assert!(events.is_empty());
    let events = r.tick(11).unwrap();
    assert_eq!(ranges(&events), one(BASE + 101..BASE + 104));
    assert_eq!(data(&events, Side::AtoB), b"de");
    assert!(
        matches!(events.last().unwrap().kind,EventKind::HalfClosed {sequence} if sequence==BASE+106)
    );
    let mut fin = segment(106, b"");
    fin.fin = true;
    assert!(push(&mut r, 12, fin).is_empty());
    let rejected = push(&mut r, 13, segment(107, b"bad"));
    assert!(rejected.iter().any(|e| matches!(
        e.kind,
        EventKind::Rejected {
            reason: Rejection::AfterFin,
            ..
        }
    )));
    push(&mut r, 14, reverse(segment(505, b"tail")));
    let mut reset = segment(107, b"");
    reset.rst = true;
    let events = push(&mut r, 15, reset);
    assert_eq!(ranges(&events), one(BASE + 501..BASE + 505));
    assert_eq!(data(&events, Side::BtoA), b"tail");
    assert_eq!(closed(&events).unwrap().0, CloseReason::Reset);
    assert_eq!(r.stats().buffered_bytes, 0);
    let mut r = engine();
    syn(&mut r, 0, 100, false);
    push(&mut r, 1, segment(110, b"x"));
    let events = r.tick(101).unwrap();
    assert_eq!(closed(&events).unwrap().0, CloseReason::Idle);
    assert_eq!(ranges(&events), one(BASE + 101..BASE + 110));
    let mut r = engine();
    syn(&mut r, 0, 100, false);
    push(&mut r, 1, segment(110, b"x"));
    let events = r.finish(2).unwrap();
    assert_eq!(closed(&events).unwrap().0, CloseReason::CaptureEnded);
    assert_eq!(r.stats().flows, 0);
}
#[test]
fn tuple_reuse_delayed_syn_midstream_and_conflicting_fin_are_unambiguous() {
    let mut r = engine();
    let events = push(&mut r, 0, segment(101, b"abc"));
    assert!(
        events
            .iter()
            .any(|e| matches!(e.kind, EventKind::UnknownPrefix))
    );
    let id = events[0].flow;
    let events = syn(&mut r, 1, 100, false);
    assert!(events.iter().all(|e| e.flow == id));
    assert!(closed(&events).is_none());
    push(&mut r, 2, segment(110, b"tail"));
    let events = syn(&mut r, 3, 1000, false);
    assert_eq!(closed(&events).unwrap().0, CloseReason::Replaced);
    assert!(
        events
            .iter()
            .any(|e| matches!(e.kind, EventKind::Opened { .. }) && e.flow != id)
    );
    assert_eq!(ranges(&events), one(BASE + 104..BASE + 110));
    push(&mut r, 4, segment(1001, b"abc"));
    let mut fin = segment(1002, b"");
    fin.fin = true;
    let events = push(&mut r, 5, fin);
    assert_eq!(
        closed(&events),
        Some((CloseReason::ControlConflict, true, true))
    );
    assert_eq!(r.stats().flows, 0);
}
#[test]
fn serial_wrap_half_space_and_outside_window_are_bounded() {
    let mut r = engine();
    syn(&mut r, 0, u32::MAX - 2, false);
    push(&mut r, 1, segment(1, b"de"));
    let events = push(&mut r, 2, segment(u32::MAX - 1, b"abc"));
    assert_eq!(data(&events, Side::AtoB), b"abcde");
    let ranges: Vec<_> = events
        .iter()
        .filter_map(|e| match &e.kind {
            EventKind::Data { sequence, .. } => Some(sequence.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(
        ranges,
        [2 * BASE - 2..2 * BASE + 1, 2 * BASE + 1..2 * BASE + 3]
    );
    for seq in [3u32.wrapping_add(1 << 31), 5000] {
        let events = push(&mut r, 3, segment(seq, b"bad"));
        assert!(events.iter().any(|e| matches!(
            e.kind,
            EventKind::Rejected {
                reason: Rejection::SequenceWindow,
                ..
            }
        )));
    }
    assert_eq!(r.stats().buffered_bytes, 0);
}
#[test]
fn interface_epoch_vlan_and_reverse_endpoints_isolate_flows() {
    let mut r = engine();
    for (interface, epoch, vlan) in [(1, 0, 1), (2, 0, 1), (1, 1, 1), (1, 0, 2)] {
        let tags = [Vlan {
            tpid: 0x8100,
            id: vlan,
        }];
        let mut s = segment(100, b"");
        s.syn = true;
        s.vlans = &tags;
        let events = r.push(Context { interface, epoch }, 0, s).unwrap();
        assert!(
            events
                .iter()
                .any(|e| matches!(e.kind, EventKind::Opened { .. }))
        );
    }
    assert_eq!(r.stats().flows, 4);
    let tags = [Vlan {
        tpid: 0x8100,
        id: 1,
    }];
    let mut s = reverse(segment(500, b""));
    s.syn = true;
    s.vlans = &tags;
    let events = r
        .push(
            Context {
                interface: 1,
                epoch: 0,
            },
            1,
            s,
        )
        .unwrap();
    assert!(
        !events
            .iter()
            .any(|e| matches!(e.kind, EventKind::Opened { .. }))
    );
    assert_eq!(r.stats().flows, 4);
    assert!(!format!("{events:?}").contains("192.0.2"));
}
#[test]
fn global_per_flow_byte_chunk_and_lru_limits_leave_explicit_loss() {
    let mut l = limits();
    l.total_bytes = 5;
    l.flow_bytes = 4;
    l.flows = 2;
    l.total_chunks = 2;
    l.direction_chunks = 1;
    let mut r = Reassembler::new(l).unwrap();
    syn(&mut r, 0, 100, false);
    push(&mut r, 1, segment(110, b"abcd"));
    let events = push(&mut r, 2, segment(120, b"x"));
    assert!(events.iter().any(|e| matches!(
        e.kind,
        EventKind::Rejected {
            reason: Rejection::BufferBytes,
            ..
        }
    )));
    let mut s = reverse(segment(500, b""));
    s.syn = true;
    push(&mut r, 3, s);
    let events = push(&mut r, 4, reverse(segment(510, b"zz")));
    assert!(events.iter().any(|e| matches!(
        e.kind,
        EventKind::Rejected {
            reason: Rejection::BufferBytes,
            ..
        }
    )));
    let events = push(&mut r, 5, segment(101, b"123456789"));
    assert_eq!(data(&events, Side::AtoB), b"123456789abcd");
    assert_eq!(r.stats().buffered_bytes, 0);
    let mut other = segment(100, b"");
    other.syn = true;
    other.source.set_port(12346);
    push(&mut r, 6, other);
    let mut another = segment(100, b"");
    another.syn = true;
    another.source.set_port(12347);
    let events = push(&mut r, 7, another);
    assert_eq!(closed(&events).unwrap().0, CloseReason::Capacity);
    assert_eq!(r.stats().flows, 2);
    assert_eq!(r.stats().evictions, 1);
    let mut l = limits();
    l.total_chunks = 1;
    l.direction_chunks = 1;
    let mut r = Reassembler::new(l).unwrap();
    syn(&mut r, 0, 100, false);
    push(&mut r, 1, segment(110, b"a"));
    let events = push(&mut r, 2, segment(112, b"b"));
    assert!(events.iter().any(|e| matches!(
        e.kind,
        EventKind::Rejected {
            reason: Rejection::BufferChunks,
            ..
        }
    )));
    let events = r.tick(11).unwrap();
    assert_eq!(
        ranges(&events),
        [BASE + 101..BASE + 110, BASE + 111..BASE + 113]
    );
    assert_eq!(data(&events, Side::AtoB), b"a");
    assert_eq!(r.stats().buffered_chunks, 0);
}
#[test]
fn invalid_limits_inputs_and_backward_clock_do_not_damage_state() {
    let l = limits();
    for invalid in [
        Limits { flows: 0, ..l },
        Limits { flows: 1025, ..l },
        Limits {
            total_bytes: usize::MAX,
            ..l
        },
        Limits {
            flow_bytes: 257,
            ..l
        },
        Limits {
            total_chunks: 4097,
            ..l
        },
        Limits {
            direction_chunks: 129,
            ..l
        },
        Limits {
            input_bytes: 65537,
            ..l
        },
        Limits {
            sequence_window: 1 << 31,
            ..l
        },
        Limits {
            gap_timeout_ns: 0,
            ..l
        },
        Limits {
            idle_timeout_ns: 0,
            ..l
        },
    ] {
        assert!(matches!(
            Reassembler::new(invalid),
            Err(Error::InvalidLimits)
        ));
    }
    let mut r = engine();
    syn(&mut r, 10, 100, false);
    assert_eq!(r.tick(9), Err(Error::ClockWentBackwards));
    let before = r.stats();
    let bytes = [0; 65];
    assert_eq!(
        r.push(Context::default(), 11, segment(101, &bytes)),
        Err(Error::InvalidSegment)
    );
    let mut invalid = segment(101, b"abcd");
    invalid.original_payload_len = 2;
    assert_eq!(
        r.push(Context::default(), 11, invalid),
        Err(Error::InvalidSegment)
    );
    let mut invalid = segment(101, b"");
    invalid.syn = true;
    invalid.fin = true;
    assert_eq!(
        r.push(Context::default(), 11, invalid),
        Err(Error::InvalidSegment)
    );
    assert_eq!(r.stats(), before);
    let events = push(&mut r, 11, segment(101, b"still valid"));
    assert_eq!(data(&events, Side::AtoB), b"still valid");
}

fn wire(
    sequence: u32,
    bytes: &[u8],
    v6: bool,
    tags: usize,
    extensions: bool,
    flags: u8,
) -> Vec<u8> {
    let mut raw = wire_fixture::wire(v6, true, 12345, 443, tags, extensions);
    let ip = 14 + 4 * tags;
    let transport = ip
        + if v6 {
            40 + if extensions { 16 } else { 0 }
        } else if extensions {
            24
        } else {
            20
        };
    raw.truncate(transport + 20);
    raw[transport + 4..transport + 8].copy_from_slice(&sequence.to_be_bytes());
    raw[transport + 13] = flags;
    raw.extend(bytes);
    if v6 {
        let len = (raw.len() - ip - 40) as u16;
        raw[ip + 4..ip + 6].copy_from_slice(&len.to_be_bytes());
    } else {
        let len = (raw.len() - ip) as u16;
        raw[ip + 2..ip + 4].copy_from_slice(&len.to_be_bytes());
    }
    raw
}
#[test]
fn actual_wire_decoder_handles_ipv4_ipv6_options_extensions_vlan_and_snaplen() {
    for v6 in [false, true] {
        for extensions in [false, true] {
            for tags in [0, 2, 8] {
                let mut r = engine();
                let syn = wire(100, b"", v6, tags, extensions, 2);
                let first = wire_fixture::observation(&syn, Direction::Unknown);
                let events = r.push_observation(Context::default(), 0, &first).unwrap();
                assert_eq!(events.len(), 1);
                let raw = wire(104, b"def", v6, tags, extensions, 0x18);
                let observation = wire_fixture::observation(&raw, Direction::Unknown);
                r.push_observation(Context::default(), 1, &observation)
                    .unwrap();
                let raw = wire(101, b"abc", v6, tags, extensions, 0x18);
                let observation = wire_fixture::observation(&raw, Direction::Unknown);
                let events = r
                    .push_observation(Context::default(), 2, &observation)
                    .unwrap();
                assert_eq!(data(&events, Side::AtoB), b"abcdef");
                assert_eq!(observation.bytes(), raw);
                assert_eq!(observation.source_timestamp_ns(), Some(99));
                assert_bounds(&r, &events);
                let raw = wire(107, b"ghijk", v6, tags, extensions, 0x19);
                let len = raw.len() - 3;
                let observation = Observation::new(
                    5,
                    Direction::Unknown,
                    Metadata::Ethernet { link_type: 1 },
                    Some(raw.len()),
                    &raw[..len],
                )
                .unwrap();
                let events = r
                    .push_observation(Context::default(), 3, &observation)
                    .unwrap();
                assert_eq!(data(&events, Side::AtoB), b"gh");
                let events = r.tick(13).unwrap();
                assert_eq!(ranges(&events), one(BASE + 109..BASE + 112));
                assert!(
                    matches!(events.last().unwrap().kind,EventKind::HalfClosed {sequence} if sequence==BASE+112)
                );
            }
        }
    }
    let mut r = engine();
    let raw = wire(100, b"payload", true, 2, true, 0x18);
    for length in 0..=raw.len() {
        let mut r = engine();
        let observation = wire_fixture::observation(&raw[..length], Direction::Unknown);
        if let Ok(events) = r.push_observation(Context::default(), 0, &observation) {
            assert_bounds(&r, &events);
        }
    }
    let observation = Observation::new(
        0,
        Direction::Rx,
        Metadata::Serial {
            configuration_revision: 1,
        },
        None,
        b"not wire",
    )
    .unwrap();
    assert_eq!(
        r.push_observation(Context::default(), 0, &observation),
        Err(Error::UnusablePacket)
    );
    assert_eq!(r.stats().flows, 0);
}

#[test]
fn deterministic_segment_and_wire_mutations_stay_bounded_without_panics() {
    let mut r = engine();
    let mut random = 0x9e3779b9u32;
    for now in 0..8000u64 {
        random ^= random << 13;
        random ^= random >> 17;
        random ^= random << 5;
        let bytes = random.to_le_bytes();
        let mut s = segment(random % 1200, &bytes[..(random % 5) as usize]);
        s.source.set_port(10000 + (random % 16) as u16);
        s.syn = random & 31 == 0;
        s.fin = random & 31 == 1;
        s.rst = random & 31 == 2;
        s.acknowledgement = (random & 3 == 0).then_some(random.rotate_left(9));
        if random & 1 != 0 {
            s = reverse(s);
        }
        let events = r
            .push(
                Context {
                    interface: random as u64 % 2,
                    epoch: 0,
                },
                now,
                s,
            )
            .unwrap();
        assert_bounds(&r, &events);
    }
    let events = r.finish(8000).unwrap();
    assert_bounds(&r, &events);
    assert_eq!(r.stats().buffered_bytes, 0);
    assert_eq!(r.stats().buffered_chunks, 0);
    assert_eq!(r.stats().flows, 0);
    let raw = wire(100, b"data", true, 2, true, 0x18);
    for i in 0..raw.len() {
        for value in [0, 1, 127, 255] {
            let mut bytes = raw.clone();
            bytes[i] = value;
            let observation = wire_fixture::observation(&bytes, Direction::Unknown);
            let mut r = engine();
            if let Ok(events) = r.push_observation(Context::default(), 0, &observation) {
                assert_bounds(&r, &events);
            }
        }
    }
}

#[test]
fn independent_per_byte_first_seen_reference_matches_many_overlap_orders() {
    let mut random = 0x243f6a88u32;
    for _case in 0..128 {
        let mut l = limits();
        l.total_chunks = 128;
        l.direction_chunks = 64;
        l.gap_timeout_ns = 10000;
        l.idle_timeout_ns = 100000;
        let mut r = Reassembler::new(l).unwrap();
        let isn = u32::MAX - 10;
        syn(&mut r, 0, isn, false);
        let mut expected = [None; 64];
        let mut delivered = 0;
        for now in 1..=96 {
            random ^= random << 13;
            random ^= random >> 17;
            random ^= random << 5;
            let start = (random % 64) as usize;
            let length = ((random >> 8) % 8 + 1) as usize;
            let length = length.min(64 - start);
            let bytes: Vec<u8> = (0..length)
                .map(|i| random.wrapping_add(i as u32) as u8)
                .collect();
            for (target, byte) in expected[start..start + length].iter_mut().zip(&bytes) {
                target.get_or_insert(*byte);
            }
            let before = delivered;
            while delivered < 64 && expected[delivered].is_some() {
                delivered += 1;
            }
            let events = push(
                &mut r,
                now,
                segment(isn.wrapping_add(1 + start as u32), &bytes),
            );
            let oracle: Vec<_> = expected[before..delivered]
                .iter()
                .map(|b| b.unwrap())
                .collect();
            assert_eq!(data(&events, Side::AtoB), oracle);
        }
        let fill = [0xee; 64];
        for target in &mut expected {
            target.get_or_insert(0xee);
        }
        let events = push(&mut r, 97, segment(isn.wrapping_add(1), &fill));
        let oracle: Vec<_> = expected[delivered..].iter().map(|b| b.unwrap()).collect();
        assert_eq!(data(&events, Side::AtoB), oracle);
        assert_eq!(r.stats().emitted_bytes, 64);
        assert_eq!(r.stats().buffered_bytes, 0);
    }
}

#[test]
fn hard_global_and_per_flow_byte_caps_are_reached_without_silent_loss() {
    let bytes = vec![0x5a; 65536];
    {
        let l = Limits {
            flows: 16,
            total_bytes: 8 * 1024 * 1024,
            flow_bytes: 1024 * 1024,
            sequence_window: 2 * 1024 * 1024,
            ..Default::default()
        };
        let mut r = Reassembler::new(l).unwrap();
        for flow in 0..16 {
            let mut s = segment(100, b"");
            s.source.set_port(12000 + flow);
            s.syn = true;
            push(&mut r, 0, s);
            for chunk in 0..8 {
                let mut s = segment(102 + chunk * 65536, &bytes);
                s.source.set_port(12000 + flow);
                push(&mut r, 0, s);
            }
        }
        assert_eq!(r.stats().buffered_bytes, 8 * 1024 * 1024);
        let mut s = segment(102 + 8 * 65536, &bytes);
        s.source.set_port(12000);
        let events = push(&mut r, 0, s);
        assert!(events.iter().any(|e| matches!(
            e.kind,
            EventKind::Rejected {
                reason: Rejection::BufferBytes,
                ..
            }
        )));
        assert_eq!(r.stats().buffered_bytes, 8 * 1024 * 1024);
        let events = r.finish(1).unwrap();
        assert_bounds(&r, &events);
        assert_eq!(r.stats().buffered_bytes, 0);
        assert_eq!(r.stats().buffered_chunks, 0);
    }
    {
        let l = Limits {
            total_bytes: 8 * 1024 * 1024,
            flow_bytes: 1024 * 1024,
            sequence_window: 2 * 1024 * 1024,
            ..Default::default()
        };
        let mut r = Reassembler::new(l).unwrap();
        syn(&mut r, 0, 100, false);
        for chunk in 0..16 {
            push(&mut r, 0, segment(102 + chunk * 65536, &bytes));
        }
        let events = push(&mut r, 0, segment(102 + 16 * 65536, &bytes));
        assert!(events.iter().any(|e| matches!(
            e.kind,
            EventKind::Rejected {
                reason: Rejection::BufferBytes,
                ..
            }
        )));
        assert_eq!(r.stats().buffered_bytes, 1024 * 1024);
        let events = r.finish(1).unwrap();
        assert_bounds(&r, &events);
        assert_eq!(r.stats().buffered_bytes, 0);
    }
}

#[test]
fn hard_chunk_and_flow_caps_bound_metadata_and_flush_output() {
    {
        let l = Limits {
            flows: 32,
            total_chunks: 4096,
            direction_chunks: 128,
            gap_timeout_ns: 1,
            ..Default::default()
        };
        let mut r = Reassembler::new(l).unwrap();
        for flow in 0..32 {
            for reverse_direction in [false, true] {
                let mut s = segment(100, b"");
                s.source.set_port(12000 + flow);
                s.syn = true;
                if reverse_direction {
                    s = reverse(s);
                }
                push(&mut r, 0, s);
                for chunk in 0..64 {
                    let mut s = segment(102 + chunk * 2, b"x");
                    s.source.set_port(12000 + flow);
                    if reverse_direction {
                        s = reverse(s);
                    }
                    push(&mut r, 0, s);
                }
            }
        }
        assert_eq!(r.stats().buffered_chunks, 4096);
        let mut s = segment(102 + 64 * 2, b"x");
        s.source.set_port(12000);
        let events = push(&mut r, 0, s);
        assert!(events.iter().any(|e| matches!(
            e.kind,
            EventKind::Rejected {
                reason: Rejection::BufferChunks,
                ..
            }
        )));
        assert_eq!(r.stats().buffered_chunks, 4096);
        let events = r.tick(1).unwrap();
        assert_bounds(&r, &events);
        assert!(events.len() >= 8192);
        assert_eq!(r.stats().buffered_chunks, 0);
    }
    {
        let l = Limits {
            flows: 1024,
            ..Default::default()
        };
        let mut r = Reassembler::new(l).unwrap();
        for interface in 0..1024 {
            let mut s = segment(100, b"");
            s.syn = true;
            let events = r
                .push(
                    Context {
                        interface,
                        epoch: 0,
                    },
                    0,
                    s,
                )
                .unwrap();
            assert_bounds(&r, &events);
        }
        assert_eq!(r.stats().flows, 1024);
        let mut s = segment(100, b"");
        s.syn = true;
        let events = r
            .push(
                Context {
                    interface: 1024,
                    epoch: 0,
                },
                0,
                s,
            )
            .unwrap();
        assert_eq!(events[0].flow, 1);
        assert_eq!(closed(&events).unwrap().0, CloseReason::Capacity);
        assert_eq!(r.stats().flows, 1024);
        let events = r.finish(1).unwrap();
        assert_bounds(&r, &events);
        assert_eq!(r.stats().flows, 0);
    }
}

#[test]
fn control_edge_cases_and_deadline_order_preserve_boundaries() {
    let mut r = engine();
    let mut start = segment(u32::MAX, b"syn data");
    start.syn = true;
    let events = push(&mut r, 0, start);
    assert_eq!(data(&events, Side::AtoB), b"syn data");
    let events = push(&mut r, 1, segment(8, b"next"));
    assert_eq!(data(&events, Side::AtoB), b"next");
    let mut r = engine();
    syn(&mut r, 0, 100, false);
    push(&mut r, 1, segment(104, b"def"));
    let events = push(&mut r, 11, segment(101, b"abc"));
    assert!(matches!(events[0].kind, EventKind::Gap { .. }));
    assert_eq!(data(&events, Side::AtoB), b"def");
    for second in [102, 106] {
        let mut r = engine();
        syn(&mut r, 0, 100, false);
        let mut fin = segment(104, b"");
        fin.fin = true;
        push(&mut r, 1, fin);
        let mut fin = segment(second, b"");
        fin.fin = true;
        let events = push(&mut r, 2, fin);
        assert_eq!(
            closed(&events),
            Some((CloseReason::ControlConflict, true, true))
        );
        assert_eq!(r.stats().flows, 0);
    }
    let mut r = engine();
    assert!(push(&mut r, 0, segment(100, b"")).is_empty());
    assert_eq!(r.stats().flows, 0);
    syn(&mut r, 1, 100, false);
    push(&mut r, 2, segment(104, b"data"));
    let events = r.tick(u64::MAX).unwrap();
    assert_eq!(closed(&events).unwrap().0, CloseReason::Idle);
    assert_eq!(r.stats().buffered_bytes, 0);
    assert_eq!(r.finish(u64::MAX).unwrap(), vec![]);
}

#[test]
fn very_large_acknowledged_hole_allocates_no_payload() {
    let mut l = limits();
    l.sequence_window = 1 << 30;
    l.gap_timeout_ns = 1;
    let mut r = Reassembler::new(l).unwrap();
    syn(&mut r, 0, 100, false);
    let mut ack = reverse(segment(500, b""));
    ack.acknowledgement = Some(101 + (1 << 30));
    push(&mut r, 0, ack);
    let events = r.tick(1).unwrap();
    assert_bounds(&r, &events);
    assert_eq!(ranges(&events), one(BASE + 101..BASE + 101 + (1 << 30)));
    assert!(data(&events, Side::AtoB).is_empty());
    assert_eq!(r.stats().buffered_bytes, 0);
    assert_eq!(r.stats().emitted_bytes, 0);
}

#[test]
fn production_debug_redacts_actual_opened_endpoints_and_data() {
    let mut r = engine();
    let s = segment(100, b"private fixture");
    let debug = format!("{s:?}");
    assert!(!debug.contains("private"));
    assert!(!debug.contains("192.0.2"));
    let events = push(&mut r, 0, s);
    assert!(
        events
            .iter()
            .any(|e| matches!(e.kind, EventKind::Opened { .. }))
    );
    assert_eq!(data(&events, Side::AtoB), b"private fixture");
    let debug = format!("{events:?}");
    assert!(!debug.contains("private"));
    assert!(!debug.contains("192.0.2"));
    assert!(!debug.contains("112, 114, 105"));
    assert!(debug.contains("[REDACTED]"));
}

#[test]
fn half_closed_ack_controls_and_fin_against_prior_extent_are_distinguished() {
    let mut r = engine();
    syn(&mut r, 0, 100, false);
    syn(&mut r, 0, 500, true);
    push(&mut r, 1, segment(101, b"abc"));
    let mut fin = segment(104, b"");
    fin.fin = true;
    push(&mut r, 2, fin);
    let mut ack = segment(105, b"");
    ack.acknowledgement = Some(501);
    let events = push(&mut r, 3, ack);
    assert!(events.is_empty(), "ACK at FIN+1 is valid control traffic");
    let mut fin = reverse(segment(501, b""));
    fin.fin = true;
    let events = push(&mut r, 4, fin);
    assert_eq!(closed(&events), Some((CloseReason::Fin, false, false)));
    let mut r = engine();
    syn(&mut r, 0, 100, false);
    let mut partial = segment(101, b"ab");
    partial.original_payload_len = 5;
    push(&mut r, 1, partial);
    let mut fin = segment(104, b"");
    fin.fin = true;
    let events = push(&mut r, 2, fin);
    assert_eq!(
        closed(&events),
        Some((CloseReason::ControlConflict, true, true))
    );
    assert_eq!(ranges(&events), one(BASE + 103..BASE + 106));
    for ack_end in [105, 200] {
        let mut r = engine();
        syn(&mut r, 0, 100, false);
        syn(&mut r, 0, 500, true);
        push(&mut r, 1, segment(101, b"abc"));
        let mut ack = reverse(segment(501, b""));
        ack.acknowledgement = Some(ack_end);
        push(&mut r, 2, ack);
        let mut fin = segment(104, b"");
        fin.fin = true;
        let events = push(&mut r, 3, fin);
        if ack_end == 105 {
            assert!(ranges(&events).is_empty());
            assert!(
                events.iter().any(
                    |e| matches!(e.kind,EventKind::HalfClosed {sequence} if sequence==BASE+104)
                )
            );
        } else {
            assert_eq!(
                closed(&events),
                Some((CloseReason::ControlConflict, true, true))
            );
        }
    }
}
