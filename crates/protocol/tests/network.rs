//! Synthetic wire fixtures use documentation addresses and locally administered MACs.
//! These are distinct from EXM-005's application payload corpus.
use exmon_domain::{
    Direction,
    observation::{Metadata, Observation},
};
use exmon_protocol::network::*;
use std::net::{Ipv4Addr, Ipv6Addr};

const V4_SOURCE: [u8; 4] = [192, 0, 2, 1];
const V4_DEST: [u8; 4] = [198, 51, 100, 2];
const V6_SOURCE: [u8; 16] = [0x20, 1, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1];
const V6_DEST: [u8; 16] = [0x20, 1, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2];
fn put16(b: &mut [u8], offset: usize, value: u16) {
    b[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
}
// Independent reference: accumulate bytes by their alternating significance,
// rather than using the decoder's array-chunk implementation.
fn reference_checksum(bytes: &[u8]) -> u16 {
    let total = bytes.iter().enumerate().fold(0u64, |sum, (i, b)| {
        sum + (u64::from(*b) << if i % 2 == 0 { 8 } else { 0 })
    });
    let mut folded = total;
    while folded > 0xffff {
        folded = (folded & 0xffff) + (folded >> 16);
    }
    !(folded as u16)
}
fn ethernet(ip: &[u8], kind: u16, tags: &[(u16, u16)]) -> Vec<u8> {
    let mut frame = vec![2, 0, 0, 0, 0, 2, 2, 0, 0, 0, 0, 1];
    for (tpid, tci) in tags {
        frame.extend(tpid.to_be_bytes());
        frame.extend(tci.to_be_bytes());
    }
    frame.extend(kind.to_be_bytes());
    frame.extend(ip);
    frame
}
fn pseudo(v6: bool, protocol: u8, segment: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    if v6 {
        bytes.extend(V6_SOURCE);
        bytes.extend(V6_DEST);
        bytes.extend((segment.len() as u32).to_be_bytes());
        bytes.extend([0, 0, 0, protocol]);
    } else {
        bytes.extend(V4_SOURCE);
        bytes.extend(V4_DEST);
        bytes.extend([0, protocol]);
        bytes.extend((segment.len() as u16).to_be_bytes());
    }
    bytes.extend(segment);
    bytes
}
fn segment(v6: bool, tcp: bool, payload: &[u8], options: &[u8]) -> Vec<u8> {
    assert!(options.len().is_multiple_of(4));
    let header = if tcp { 20 + options.len() } else { 8 };
    let mut bytes = vec![0; header];
    put16(&mut bytes, 0, 12345);
    put16(&mut bytes, 2, 443);
    if tcp {
        bytes[4..8].copy_from_slice(&0x12345678u32.to_be_bytes());
        bytes[8..12].copy_from_slice(&0xabcdef01u32.to_be_bytes());
        bytes[12] = ((header / 4) as u8) << 4;
        bytes[13] = 0x18;
        put16(&mut bytes, 14, 4096);
        bytes[20..header].copy_from_slice(options);
    } else {
        put16(&mut bytes, 4, (header + payload.len()) as u16);
    }
    bytes.extend(payload);
    let check = reference_checksum(&pseudo(v6, if tcp { 6 } else { 17 }, &bytes));
    put16(
        &mut bytes,
        if tcp { 16 } else { 6 },
        if check == 0 { 0xffff } else { check },
    );
    bytes
}
fn ipv4(protocol: u8, segment: &[u8], options: &[u8], fragment: u16) -> Vec<u8> {
    assert!(options.len().is_multiple_of(4));
    let header = 20 + options.len();
    let mut bytes = vec![0; header];
    bytes[0] = 0x40 | (header / 4) as u8;
    bytes[1] = 0x2e;
    put16(&mut bytes, 2, (header + segment.len()) as u16);
    put16(&mut bytes, 4, 0x6789);
    put16(&mut bytes, 6, fragment);
    bytes[8] = 63;
    bytes[9] = protocol;
    bytes[12..16].copy_from_slice(&V4_SOURCE);
    bytes[16..20].copy_from_slice(&V4_DEST);
    bytes[20..header].copy_from_slice(options);
    let check = reference_checksum(&bytes);
    put16(&mut bytes, 10, check);
    bytes.extend(segment);
    bytes
}
fn ipv6(next: u8, payload: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0; 40];
    bytes[0..4].copy_from_slice(&0x6ab12345u32.to_be_bytes());
    put16(&mut bytes, 4, payload.len() as u16);
    bytes[6] = next;
    bytes[7] = 42;
    bytes[8..24].copy_from_slice(&V6_SOURCE);
    bytes[24..40].copy_from_slice(&V6_DEST);
    bytes.extend(payload);
    bytes
}
fn wire(v6: bool, tcp: bool, payload: &[u8]) -> Vec<u8> {
    let segment = segment(v6, tcp, payload, &[]);
    ethernet(
        &if v6 {
            ipv6(if tcp { 6 } else { 17 }, &segment)
        } else {
            ipv4(if tcp { 6 } else { 17 }, &segment, &[], 0x4000)
        },
        if v6 { 0x86dd } else { 0x0800 },
        &[],
    )
}
fn checked<'a>(decoder: &Decoder, raw: &'a [u8]) -> Packet<'a> {
    let p = decoder.decode(raw);
    assert_eq!(p.raw(), raw);
    assert_eq!(p.raw().as_ptr(), raw.as_ptr());
    assert!(p.payload_range().start <= p.payload_range().end);
    assert!(p.payload_range().end <= raw.len());
    assert!(p.vlans.len() <= decoder.limits().vlan_tags);
    assert!(p.extensions.len() <= decoder.limits().ipv6_extensions);
    let range = |r: &std::ops::Range<usize>| {
        assert!(r.start <= r.end && r.end <= raw.len() && r.end <= decoder.limits().header_bytes);
    };
    match &p.ip {
        Some(Ip::V4(ip)) => range(&ip.header),
        Some(Ip::V6(ip)) => range(&ip.header),
        _ => {}
    }
    for ext in &p.extensions {
        range(&ext.range);
    }
    match &p.transport {
        Some(Transport::Tcp(t)) => {
            range(&t.header);
            range(&t.options);
        }
        Some(Transport::Udp(t)) => range(&t.header),
        _ => {}
    }
    p
}

#[test]
fn exact_known_ethernet_ipv4_ipv6_tcp_udp_and_odd_checksums() {
    assert_eq!(
        reference_checksum(&[0, 1, 0xf2, 3, 0xf4, 0xf5, 0xf6, 0xf7]),
        0x220d
    ); // RFC 1071 example.
    for v6 in [false, true] {
        for tcp in [false, true] {
            for payload in [&b""[..], &b"abc"[..], &b"abcd"[..]] {
                let raw = wire(v6, tcp, payload);
                let p = checked(&Decoder::default(), &raw);
                assert_eq!(p.diagnostic, None);
                assert!(!p.capture_truncated);
                assert_eq!(p.payload(), payload);
                assert_eq!(p.ethernet.as_ref().unwrap().destination, [2, 0, 0, 0, 0, 2]);
                assert_eq!(p.ethernet.as_ref().unwrap().source, [2, 0, 0, 0, 0, 1]);
                assert_eq!(
                    p.ethernet.as_ref().unwrap().ether_type,
                    if v6 { 0x86dd } else { 0x0800 }
                );
                match p.ip.as_ref().unwrap() {
                    Ip::V4(ip) => {
                        assert!(!v6);
                        assert_eq!(ip.source, Ipv4Addr::from(V4_SOURCE));
                        assert_eq!(ip.destination, Ipv4Addr::from(V4_DEST));
                        assert_eq!(ip.protocol, if tcp { 6 } else { 17 });
                        assert_eq!(ip.dscp_ecn, 0x2e);
                        assert_eq!(ip.ttl, 63);
                        assert_eq!(ip.identification, 0x6789);
                        assert!(ip.dont_fragment);
                        assert!(!ip.more_fragments);
                        assert_eq!(ip.fragment_offset, 0);
                        assert_eq!(ip.total_length, raw.len() - 14);
                        assert_eq!(ip.header, 14..34);
                        assert_eq!(ip.checksum, Checksum::Valid);
                    }
                    Ip::V6(ip) => {
                        assert!(v6);
                        assert_eq!(ip.source, Ipv6Addr::from(V6_SOURCE));
                        assert_eq!(ip.destination, Ipv6Addr::from(V6_DEST));
                        assert_eq!(ip.next_header, if tcp { 6 } else { 17 });
                        assert_eq!(ip.traffic_class, 0xab);
                        assert_eq!(ip.flow_label, 0x12345);
                        assert_eq!(ip.hop_limit, 42);
                        assert_eq!(ip.payload_length, raw.len() - 54);
                        assert_eq!(ip.header, 14..54);
                    }
                }
                let start = if v6 { 54 } else { 34 };
                match p.transport.as_ref().unwrap() {
                    Transport::Tcp(t) => {
                        assert!(tcp);
                        assert_eq!((t.source_port, t.destination_port), (12345, 443));
                        assert_eq!(t.sequence, 0x12345678);
                        assert_eq!(t.acknowledgement, 0xabcdef01);
                        assert_eq!(t.flags, 0x18);
                        assert_eq!(t.window, 4096);
                        assert_eq!(t.urgent_pointer, 0);
                        assert_eq!(t.header, start..start + 20);
                        assert_eq!(t.options, start + 20..start + 20);
                        assert_eq!(t.checksum, Checksum::Valid);
                    }
                    Transport::Udp(t) => {
                        assert!(!tcp);
                        assert_eq!((t.source_port, t.destination_port), (12345, 443));
                        assert_eq!(t.length, payload.len() + 8);
                        assert_eq!(t.header, start..start + 8);
                        assert_eq!(t.checksum, Checksum::Valid);
                    }
                }
            }
        }
    }
}
#[test]
fn stacked_vlans_ipv4_tcp_options_and_padding() {
    let segment = segment(false, true, b"odd", &[2, 4, 0x05, 0xb4, 1, 1, 1, 0]);
    let ip = ipv4(6, &segment, &[1, 1, 0, 0], 0);
    let mut raw = ethernet(&ip, 0x0800, &[(0x88a8, 0xb123), (0x8100, 0x0456)]);
    raw.extend([0xaa; 32]);
    let p = checked(&Decoder::default(), &raw);
    assert_eq!(p.diagnostic, None);
    assert_eq!(p.payload(), b"odd");
    assert_eq!(p.vlans.len(), 2);
    assert_eq!(
        p.vlans[0],
        Vlan {
            tpid: 0x88a8,
            tci: 0xb123
        }
    );
    assert_eq!(p.vlans[0].id(), 0x123);
    assert_eq!(p.vlans[0].priority(), 5);
    assert!(p.vlans[0].drop_eligible());
    assert_eq!(p.vlans[1].id(), 0x456);
    assert!(!p.vlans[1].drop_eligible());
    assert_eq!(p.ethernet.unwrap().ether_type, 0x0800);
    let Some(Ip::V4(ip)) = p.ip else { panic!() };
    assert_eq!(ip.header, 22..46);
    assert_eq!(ip.checksum, Checksum::Valid);
    let Some(Transport::Tcp(t)) = p.transport else {
        panic!()
    };
    assert_eq!(t.header, 46..74);
    assert_eq!(t.options, 66..74);
    assert_eq!(t.checksum, Checksum::Valid);
}
#[test]
fn ipv6_extensions_and_atomic_fragment_preserve_upper_layer_checksum() {
    // Hop-by-hop, destination options, inactive routing, atomic fragment, AH, UDP.
    let mut ext = vec![
        60, 0, 0, 0, 0, 0, 0, 0, 43, 0, 1, 4, 0, 0, 0, 0, 44, 0, 0, 0, 0, 0, 0, 0, 51, 0, 0, 0,
        0x12, 0x34, 0x56, 0x78, 17, 2, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0,
    ];
    ext.extend(segment(true, false, b"hello", &[]));
    let raw = ethernet(&ipv6(0, &ext), 0x86dd, &[]);
    let p = checked(&Decoder::default(), &raw);
    assert_eq!(p.diagnostic, None);
    assert_eq!(p.payload(), b"hello");
    assert_eq!(
        p.extensions.iter().map(|e| e.kind).collect::<Vec<_>>(),
        [0, 60, 43, 44, 51]
    );
    assert_eq!(
        p.extensions[3].fragment,
        Some(Fragment {
            offset: 0,
            more: false,
            identification: 0x12345678
        })
    );
    assert_eq!(p.extensions[4].range, 86..102);
    let Some(Transport::Udp(udp)) = p.transport else {
        panic!()
    };
    assert_eq!(udp.checksum, Checksum::Valid);
}
#[test]
fn checksum_corruption_zero_udp_truncation_and_source_routing_are_explicit() {
    for v6 in [false, true] {
        for tcp in [false, true] {
            let mut raw = wire(v6, tcp, b"abc");
            let start = if v6 { 54 } else { 34 };
            *raw.last_mut().unwrap() ^= 1;
            let p = Decoder::default().decode(&raw);
            assert_eq!(p.diagnostic, None);
            match p.transport.unwrap() {
                Transport::Tcp(t) => assert_eq!(t.checksum, Checksum::Invalid),
                Transport::Udp(t) => assert_eq!(t.checksum, Checksum::Invalid),
            }
            if !tcp {
                put16(&mut raw, start + 6, 0);
                let p = Decoder::default().decode(&raw);
                let Some(Transport::Udp(udp)) = p.transport else {
                    panic!()
                };
                assert_eq!(
                    udp.checksum,
                    if v6 {
                        Checksum::Invalid
                    } else {
                        Checksum::NotPresent
                    }
                );
            }
            let raw = wire(v6, tcp, b"abc");
            let p = Decoder::default().decode(&raw[..raw.len() - 1]);
            assert!(matches!(p.diagnostic, Some(Diagnostic::Truncated { .. })));
            assert!(p.capture_truncated);
            assert_eq!(p.payload(), b"ab");
            match p.transport.unwrap() {
                Transport::Tcp(t) => assert_eq!(t.checksum, Checksum::Unavailable),
                Transport::Udp(t) => assert_eq!(t.checksum, Checksum::Unavailable),
            }
        }
    }
    let mut raw = wire(false, false, b"abc");
    raw[24] ^= 1;
    let p = Decoder::default().decode(&raw);
    let Some(Ip::V4(ip)) = p.ip else { panic!() };
    assert_eq!(ip.checksum, Checksum::Invalid);
    let raw = ethernet(
        &ipv4(17, &segment(false, false, b"abc", &[]), &[131, 3, 4, 0], 0),
        0x0800,
        &[],
    );
    let p = Decoder::default().decode(&raw);
    let Some(Transport::Udp(udp)) = p.transport else {
        panic!()
    };
    assert_eq!(udp.checksum, Checksum::Unavailable);
    // Routing with segments left, or Home Address, makes pseudoheader endpoints ambiguous.
    for extension in [
        vec![17, 0, 0, 1, 0, 0, 0, 0],
        vec![
            17, 2, 0xc9, 16, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ],
    ] {
        let kind = if extension.len() == 8 { 43 } else { 60 };
        let mut ext = extension;
        ext.extend(segment(true, false, b"abc", &[]));
        let raw = ethernet(&ipv6(kind, &ext), 0x86dd, &[]);
        let p = Decoder::default().decode(&raw);
        assert_eq!(p.diagnostic, None);
        let Some(Transport::Udp(udp)) = p.transport else {
            panic!()
        };
        assert_eq!(udp.checksum, Checksum::Unavailable);
    }
}
#[test]
fn fragments_unknown_encrypted_jumbo_and_non_ethernet_keep_original_bytes() {
    for bits in [0x2000, 1] {
        let raw = ethernet(&ipv4(17, &[0x11; 8], &[], bits), 0x0800, &[]);
        let p = checked(&Decoder::default(), &raw);
        assert_eq!(
            p.diagnostic,
            Some(Diagnostic::Fragmented { layer: Layer::Ipv4 })
        );
        assert!(p.transport.is_none());
        let Some(Ip::V4(ip)) = p.ip else { panic!() };
        assert_eq!(ip.fragment_offset, if bits == 1 { 8 } else { 0 });
    }
    for bits in [1u16, 8] {
        let mut fragment = vec![17, 0];
        fragment.extend(bits.to_be_bytes());
        fragment.extend([0, 0, 0, 1]);
        fragment.extend([0x11; 8]);
        let raw = ethernet(&ipv6(44, &fragment), 0x86dd, &[]);
        let p = checked(&Decoder::default(), &raw);
        assert_eq!(
            p.diagnostic,
            Some(Diagnostic::Fragmented { layer: Layer::Ipv6 })
        );
        assert!(p.transport.is_none());
        assert_eq!(
            p.extensions[0].fragment.unwrap().offset,
            usize::from(bits & 0xfff8)
        );
    }
    for (raw, expected) in [
        (
            ethernet(b"opaque", 0x88b5, &[]),
            Diagnostic::Unsupported {
                layer: Layer::Ethernet,
                protocol: 0x88b5,
            },
        ),
        (
            ethernet(b"LLC", 3, &[]),
            Diagnostic::Unsupported {
                layer: Layer::Ethernet,
                protocol: 3,
            },
        ),
        (
            ethernet(&ipv4(1, b"ICMP", &[], 0), 0x0800, &[]),
            Diagnostic::Unsupported {
                layer: Layer::Ipv4,
                protocol: 1,
            },
        ),
        (
            ethernet(&ipv6(58, b"ICMPv6"), 0x86dd, &[]),
            Diagnostic::Unsupported {
                layer: Layer::Ipv6,
                protocol: 58,
            },
        ),
        (
            ethernet(&ipv6(50, b"encrypted"), 0x86dd, &[]),
            Diagnostic::Encrypted,
        ),
        (
            ethernet(&ipv6(0, &[]), 0x86dd, &[]),
            Diagnostic::JumbogramUnsupported,
        ),
    ] {
        let p = checked(&Decoder::default(), &raw);
        assert_eq!(p.diagnostic, Some(expected));
    }
    let empty = ethernet(&ipv6(59, &[]), 0x86dd, &[]);
    assert_eq!(Decoder::default().decode(&empty).diagnostic, None);
    let raw = wire(false, false, b"abc");
    for metadata in [
        Metadata::Ethernet { link_type: 0 },
        Metadata::Ethernet { link_type: 101 },
        Metadata::Serial {
            configuration_revision: 1,
        },
    ] {
        let observation = Observation::new(42, Direction::Rx, metadata, Some(raw.len() + 1), &raw)
            .unwrap()
            .with_source_timestamp(99);
        let p = Decoder::default().decode_observation(&observation);
        assert_eq!(p.raw(), raw);
        assert!(p.capture_truncated);
        assert!(matches!(
            p.diagnostic,
            Some(Diagnostic::Unsupported {
                layer: Layer::Link,
                ..
            })
        ));
        assert!(p.ip.is_none());
        assert_eq!(
            (
                observation.timestamp_ns(),
                observation.source_timestamp_ns()
            ),
            (42, Some(99))
        );
    }
    let observation = Observation::new(
        42,
        Direction::Rx,
        Metadata::Ethernet { link_type: 1 },
        Some(raw.len() + 4),
        &raw,
    )
    .unwrap();
    let p = Decoder::default().decode_observation(&observation);
    assert_eq!(p.diagnostic, None);
    assert!(p.capture_truncated);
    assert_eq!(p.raw().as_ptr(), observation.bytes().as_ptr());
    assert!(!format!("{p:?}").contains("abc"));
    assert!(!format!("{:?}", p.ip).contains("192.0.2"));
}
#[test]
fn invalid_lengths_options_and_reserved_fields_are_rejected() {
    let mut cases = Vec::new();
    for (index, value) in [(14, 0x65), (14, 0x44), (17, 19), (20, 0x80), (34 + 5, 7)] {
        let mut raw = wire(false, false, b"abc");
        raw[index] = value;
        cases.push(raw);
    }
    for offset in [0x40, 0xf0] {
        let mut raw = wire(false, true, b"abc");
        raw[46] = offset;
        cases.push(raw);
    }
    for options in [[2, 0, 0, 0], [2, 5, 0, 0], [2, 3, 0, 0], [1, 1, 1, 2]] {
        cases.push(ethernet(
            &ipv4(6, &segment(false, true, b"", &options), &[], 0),
            0x0800,
            &[],
        ));
    }
    cases.push(ethernet(
        &ipv4(17, &segment(false, false, b"", &[]), &[2, 0, 0, 0], 0),
        0x0800,
        &[],
    ));
    cases.push(ethernet(&ipv4(17, &[0; 9], &[], 0x2000), 0x0800, &[]));
    for (next, ext) in [
        (0, vec![17, 0, 2, 9, 0, 0, 0, 0]),
        (44, vec![17, 1, 0, 0, 0, 0, 0, 0]),
        (44, vec![17, 0, 0, 2, 0, 0, 0, 0]),
        (51, vec![17, 0, 0, 0, 0, 0, 0, 0]),
        (51, vec![17, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
        (60, vec![0, 0, 0, 0, 0, 0, 0, 0, 17, 0, 0, 0, 0, 0, 0, 0]),
        (44, vec![44, 0, 0, 0, 0, 0, 0, 0, 17, 0, 0, 0, 0, 0, 0, 0]),
        (44, vec![17, 0, 0, 1, 0, 0, 0, 0, 1]),
    ] {
        cases.push(ethernet(&ipv6(next, &ext), 0x86dd, &[]));
    }
    let mut v6 = wire(true, false, b"abc");
    v6[14] = 0x40;
    cases.push(v6);
    for raw in cases {
        let p = checked(&Decoder::default(), &raw);
        assert!(
            matches!(p.diagnostic, Some(Diagnostic::Malformed { .. })),
            "{p:?}"
        );
    }
}
#[test]
fn hard_and_configured_limits_stop_before_allocating_or_scanning_excess_headers() {
    let default = Limits::default();
    for invalid in [
        Limits {
            frame_bytes: 0,
            ..default
        },
        Limits {
            frame_bytes: MAX_FRAME_BYTES + 1,
            ..default
        },
        Limits {
            vlan_tags: MAX_VLANS + 1,
            ..default
        },
        Limits {
            ipv6_extensions: MAX_EXTENSIONS + 1,
            ..default
        },
        Limits {
            header_bytes: 13,
            ..default
        },
        Limits {
            header_bytes: MAX_HEADER_BYTES + 1,
            ..default
        },
    ] {
        assert!(Decoder::new(invalid).is_err());
    }
    let raw = vec![0; MAX_FRAME_BYTES + 1];
    let p = checked(&Decoder::default(), &raw);
    assert_eq!(
        p.diagnostic,
        Some(Diagnostic::Limited {
            limit: Limit::FrameBytes,
            offset: 0
        })
    );
    assert!(p.ethernet.is_none());
    let ip = ipv4(17, &segment(false, false, b"abc", &[]), &[], 0);
    for count in 0..=MAX_VLANS {
        let tags = vec![(0x8100, 1); count + 1];
        let raw = ethernet(&ip, 0x0800, &tags);
        let decoder = Decoder::new(Limits {
            vlan_tags: count,
            ..default
        })
        .unwrap();
        let p = checked(&decoder, &raw);
        assert_eq!(
            p.diagnostic,
            Some(Diagnostic::Limited {
                limit: Limit::VlanTags,
                offset: 14 + 4 * count
            })
        );
        assert_eq!(p.vlans.len(), count);
    }
    for count in 0..=MAX_EXTENSIONS {
        let mut extensions = Vec::new();
        for _ in 0..count {
            extensions.extend([60, 0, 0, 0, 0, 0, 0, 0]);
        }
        extensions.extend([17, 0, 0, 0, 0, 0, 0, 0]);
        extensions.extend(segment(true, false, b"abc", &[]));
        let raw = ethernet(&ipv6(60, &extensions), 0x86dd, &[]);
        let decoder = Decoder::new(Limits {
            ipv6_extensions: count,
            ..default
        })
        .unwrap();
        let p = checked(&decoder, &raw);
        assert_eq!(
            p.diagnostic,
            Some(Diagnostic::Limited {
                limit: Limit::Extensions,
                offset: 54 + 8 * count
            })
        );
        assert_eq!(p.extensions.len(), count);
    }
    let raw = wire(false, true, b"abc");
    let p = checked(
        &Decoder::new(Limits {
            frame_bytes: raw.len() - 1,
            ..default
        })
        .unwrap(),
        &raw,
    );
    assert!(matches!(
        p.diagnostic,
        Some(Diagnostic::Limited {
            limit: Limit::FrameBytes,
            ..
        })
    ));
    let p = checked(
        &Decoder::new(Limits {
            header_bytes: 34,
            ..default
        })
        .unwrap(),
        &raw,
    );
    assert_eq!(
        p.diagnostic,
        Some(Diagnostic::Limited {
            limit: Limit::HeaderBytes,
            offset: 34
        })
    );
    let mut options = vec![0; 2048];
    options[0] = 60;
    options[1] = 255;
    let mut second = options.clone();
    second[0] = 59;
    options.extend(second);
    let raw = ethernet(&ipv6(60, &options), 0x86dd, &[]);
    let p = checked(&Decoder::default(), &raw);
    assert_eq!(p.extensions.len(), 1);
    assert!(matches!(
        p.diagnostic,
        Some(Diagnostic::Limited {
            limit: Limit::HeaderBytes,
            ..
        })
    ));
    let payload = vec![0xab; MAX_FRAME_BYTES - 14 - 20 - 8];
    let raw = wire(false, false, &payload);
    let p = checked(&Decoder::default(), &raw);
    assert_eq!(p.diagnostic, None);
    assert_eq!(p.payload(), payload);
}
#[test]
fn all_fixture_prefixes_mutations_and_deterministic_hostile_noise_stay_bounded() {
    let decoder = Decoder::default();
    let mut fixtures = vec![
        wire(false, false, b"abcdefg"),
        wire(false, true, b"abcdefg"),
        wire(true, false, b"abcdefg"),
        wire(true, true, b"abcdefg"),
    ];
    let mut ext = vec![44, 0, 0, 0, 0, 0, 0, 0, 17, 0, 0, 0, 1, 2, 3, 4];
    ext.extend(segment(true, false, b"hello", &[]));
    fixtures.push(ethernet(
        &ipv6(0, &ext),
        0x86dd,
        &[(0x88a8, 1), (0x8100, 2)],
    ));
    fixtures.push(ethernet(
        &ipv4(
            6,
            &segment(false, true, b"xyz", &[1, 2, 4, 5, 0xb4, 0, 0, 0]),
            &[1, 1, 0, 0],
            0,
        ),
        0x0800,
        &[],
    ));
    for raw in fixtures {
        for length in 0..=raw.len() {
            checked(&decoder, &raw[..length]);
        }
        for index in 0..raw.len() {
            for value in [0, 1, 15, 127, 255] {
                let mut mutated = raw.clone();
                mutated[index] = value;
                checked(&decoder, &mutated);
            }
        }
    }
    let mut state = 0x9e3779b9u32;
    for iteration in 0..4000 {
        let mut raw = vec![0; iteration % 300];
        for byte in &mut raw {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            *byte = state as u8;
        }
        if raw.len() >= 14 {
            let kind = match iteration % 3 {
                0 => 0x0800u16,
                1 => 0x86dd,
                _ => 0x8100,
            };
            put16(&mut raw, 12, kind);
        }
        checked(&decoder, &raw);
        checked(
            &Decoder::new(Limits {
                frame_bytes: 128,
                header_bytes: 64,
                vlan_tags: 1,
                ipv6_extensions: 1,
            })
            .unwrap(),
            &raw,
        );
    }
}

#[test]
fn existing_application_payload_corpus_is_retained_without_claiming_wire_decoding() {
    let corpus: serde_json::Value =
        serde_json::from_str(include_str!("../../../fixtures/traffic/corpus.json")).unwrap();
    let cases = corpus["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 132);
    for case in cases {
        for observation in case["observations"].as_array().unwrap() {
            let raw: Vec<u8> = observation["bytes"]
                .as_array()
                .unwrap()
                .iter()
                .map(|b| u8::try_from(b.as_u64().unwrap()).unwrap())
                .collect();
            // This proves retention and bounded error handling only. These bytes
            // have no Ethernet/IP wire-header acceptance assertions.
            checked(&Decoder::default(), &raw);
        }
    }
}
