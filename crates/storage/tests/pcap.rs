use exmon_domain::{
    Direction,
    observation::{Metadata, Observation},
};
use exmon_storage::pcap::*;
use std::io::{self, Cursor, Read, Write};

fn n16(b: &mut Vec<u8>, n: u16, e: Endian) {
    b.extend_from_slice(&match e {
        Endian::Little => n.to_le_bytes(),
        Endian::Big => n.to_be_bytes(),
    });
}
fn n32(b: &mut Vec<u8>, n: u32, e: Endian) {
    b.extend_from_slice(&match e {
        Endian::Little => n.to_le_bytes(),
        Endian::Big => n.to_be_bytes(),
    });
}
fn n64(b: &mut Vec<u8>, n: i64, e: Endian) {
    b.extend_from_slice(&match e {
        Endian::Little => n.to_le_bytes(),
        Endian::Big => n.to_be_bytes(),
    });
}
fn ng_block(kind: u32, body: &[u8], e: Endian) -> Vec<u8> {
    let mut raw = vec![];
    n32(&mut raw, kind, e);
    n32(&mut raw, (body.len() + 12) as u32, e);
    raw.extend_from_slice(body);
    n32(&mut raw, (body.len() + 12) as u32, e);
    raw
}
fn shb(e: Endian, length: i64) -> Vec<u8> {
    let mut b = vec![];
    n32(&mut b, 0x1a2b3c4d, e);
    n16(&mut b, 1, e);
    n16(&mut b, 0, e);
    n64(&mut b, length, e);
    ng_block(0x0a0d0d0a, &b, e)
}
fn option(b: &mut Vec<u8>, code: u16, value: &[u8], e: Endian) {
    n16(b, code, e);
    n16(b, value.len() as u16, e);
    b.extend_from_slice(value);
    while !b.len().is_multiple_of(4) {
        b.push(0);
    }
}
fn idb(e: Endian, link: u16, snap: u32, resolution: u8, offset: i64) -> Vec<u8> {
    let mut b = vec![];
    n16(&mut b, link, e);
    n16(&mut b, 0, e);
    n32(&mut b, snap, e);
    option(&mut b, 9, &[resolution], e);
    let mut seconds = vec![];
    n64(&mut seconds, offset, e);
    option(&mut b, 14, &seconds, e);
    option(&mut b, 3000, b"opaque-option", e);
    option(&mut b, 0, &[], e);
    ng_block(1, &b, e)
}
fn epb(e: Endian, id: u32, ticks: u64, payload: &[u8], original: u32, flags: u32) -> Vec<u8> {
    let mut b = vec![];
    for n in [
        id,
        (ticks >> 32) as u32,
        ticks as u32,
        payload.len() as u32,
        original,
    ] {
        n32(&mut b, n, e);
    }
    b.extend_from_slice(payload);
    while !b.len().is_multiple_of(4) {
        b.push(0x5a);
    }
    let mut f = vec![];
    n32(&mut f, flags, e);
    option(&mut b, 2, &f, e);
    option(&mut b, 0, &[], e);
    ng_block(6, &b, e)
}
fn pcap(
    e: Endian,
    nanos: bool,
    link: u32,
    snap: u32,
    records: &[(u32, u32, &[u8], u32)],
) -> Vec<u8> {
    let mut b = vec![];
    n32(&mut b, if nanos { 0xa1b23c4d } else { 0xa1b2c3d4 }, e);
    n16(&mut b, 2, e);
    n16(&mut b, 4, e);
    n32(&mut b, 0, e);
    n32(&mut b, 0, e);
    n32(&mut b, snap, e);
    n32(&mut b, link, e);
    for (seconds, frac, payload, original) in records {
        for n in [*seconds, *frac, payload.len() as u32, *original] {
            n32(&mut b, n, e);
        }
        b.extend_from_slice(payload);
    }
    b
}
fn entries(bytes: &[u8]) -> Vec<Entry> {
    let mut reader = Reader::new(Cursor::new(bytes), Limits::default()).unwrap();
    let mut out = vec![];
    while let Some(entry) = reader.next_entry().unwrap() {
        out.push(entry);
    }
    out
}
fn fail(bytes: &[u8], code: Code, offset: u64) -> Error {
    let mut reader = Reader::new(Cursor::new(bytes), Limits::default()).unwrap();
    let failure = loop {
        match reader.next_entry() {
            Err(e) => break e,
            Ok(Some(_)) => {}
            Ok(None) => panic!("expected failure"),
        }
    };
    assert_eq!((failure.code, failure.offset), (code, offset));
    assert_eq!(reader.next_entry().unwrap_err().code, Code::State);
    failure
}
fn obs(link: u32, time: u64, direction: Direction, payload: &[u8]) -> Observation {
    Observation::new(
        42,
        direction,
        Metadata::Ethernet { link_type: link },
        Some(payload.len() + 2),
        payload,
    )
    .unwrap()
    .with_source_timestamp(time)
}

#[test]
fn independent_literal_pcap_and_four_endian_precision_variants_are_exact() {
    // Literal reference is independent of both fixture builder and production encoder.
    let literal = [
        0xd4, 0xc3, 0xb2, 0xa1, 2, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xff, 0xff, 0, 0, 1, 0, 0, 0,
        1, 0, 0, 0, 0x40, 0xe2, 1, 0, 3, 0, 0, 0, 5, 0, 0, 0, 0, 0xff, 0x41,
    ];
    let parsed = entries(&literal);
    let o = parsed[1].observation(7).unwrap();
    assert_eq!(o.bytes(), [0, 255, 65]);
    assert_eq!(o.original_length(), Some(5));
    assert_eq!(o.source_timestamp_ns(), Some(1_123_456_000));
    assert_eq!(o.timestamp_ns(), 7);
    for e in [Endian::Little, Endian::Big] {
        for nanos in [false, true] {
            let raw = pcap(
                e,
                nanos,
                0x2400_ffff,
                65536,
                &[(u32::MAX, 123456, b"\0\xffPAYLOAD", 20), (0, 0, b"", 0)],
            );
            let parsed = entries(&raw);
            assert_eq!(parsed.len(), 3);
            assert!(
                matches!(parsed[0].content(),Content::Header(h) if h.link_info==0x2400ffff && h.endian==e && h.nanos==nanos)
            );
            let p = parsed[1].observation(0).unwrap();
            assert!(matches!(
                p.metadata(),
                Metadata::Ethernet { link_type: 65535 }
            ));
            assert_eq!(
                p.source_timestamp_ns(),
                Some(u64::from(u32::MAX) * 1_000_000_000 + 123456 * if nanos { 1 } else { 1000 })
            );
            let mut r = Reader::new(Cursor::new(&raw), Limits::default()).unwrap();
            assert_eq!(copy_all(&mut r, vec![]).unwrap(), raw);
        }
    }
}
#[test]
fn ng_sections_interfaces_endianness_options_native_clocks_and_selection() {
    let mut raw = vec![];
    for e in [Endian::Little, Endian::Big] {
        let mut body = idb(e, 1, 65536, 9, 0);
        body.extend(idb(e, 65535, 5, 0x8a, -2));
        body.extend(ng_block(0x12345678, b"metadata", e));
        body.extend(epb(e, 0, 123456789, b"\0\xfffirst", 12, 1));
        body.extend(epb(e, 1, 3072, b"last", 4, 2));
        raw.extend(shb(e, body.len() as i64));
        raw.extend(body);
    }
    let parsed = entries(&raw);
    assert_eq!(parsed.len(), 12);
    let packets: Vec<_> = parsed
        .iter()
        .filter(|e| matches!(e.content(), Content::Packet(_)))
        .collect();
    assert_eq!(packets.len(), 4);
    for p in &packets {
        let o = p.observation(999).unwrap();
        if o.bytes() == b"last" {
            assert_eq!(o.direction(), Direction::Tx);
            assert_eq!(o.source_timestamp_ns(), Some(1_000_000_000));
            assert!(matches!(
                o.metadata(),
                Metadata::Ethernet { link_type: 65535 }
            ));
        } else {
            assert_eq!(o.direction(), Direction::Rx);
            assert_eq!(o.source_timestamp_ns(), Some(123456789));
        }
    }
    assert_eq!(packets[0].section(), 1);
    assert_eq!(packets[2].section(), 2);
    let mut r = Reader::new(Cursor::new(&raw), Limits::default()).unwrap();
    assert_eq!(copy_all(&mut r, vec![]).unwrap(), raw);
    let mut r = Reader::new(Cursor::new(&raw), Limits::default()).unwrap();
    let selected =
        copy_selected(&mut r, vec![], |e| e.payload() == Some(b"last".as_slice())).unwrap();
    let selected = entries(&selected);
    assert_eq!(selected.len(), 10);
    assert_eq!(
        selected
            .iter()
            .filter(|e| matches!(e.content(), Content::Packet(_)))
            .count(),
        2
    );
    assert!(
        selected
            .iter()
            .filter_map(Entry::payload)
            .all(|p| p == b"last")
    );
    assert!(
        selected
            .iter()
            .filter_map(|e| if let Content::Section { length, .. } = e.content() {
                Some(*length)
            } else {
                None
            })
            .all(|n| n == -1)
    );
    assert_eq!(
        selected
            .iter()
            .filter(|e| matches!(e.content(), Content::Opaque { .. }))
            .count(),
        2
    );
}
#[test]
fn simple_and_obsolete_packets_keep_missing_time_and_padding_honest() {
    for e in [Endian::Little, Endian::Big] {
        let mut raw = shb(e, -1);
        raw.extend(idb(e, 1, 3, 6, 0));
        let mut b = vec![];
        n32(&mut b, 5, e);
        b.extend_from_slice(&[1, 2, 3, 0x7e]);
        raw.extend(ng_block(3, &b, e));
        let mut b = vec![];
        n16(&mut b, 0, e);
        n16(&mut b, 0xffff, e);
        for n in [0, 1, 3, 5] {
            n32(&mut b, n, e);
        }
        b.extend_from_slice(&[4, 5, 6, 0x7e]);
        raw.extend(ng_block(2, &b, e));
        let parsed = entries(&raw);
        assert_eq!(parsed[2].payload(), Some([1, 2, 3].as_slice()));
        assert_eq!(parsed[2].observation(0).unwrap_err().code, Code::Timestamp);
        assert_eq!(parsed[3].payload(), Some([4, 5, 6].as_slice()));
        assert_eq!(
            parsed[3].observation(0).unwrap().source_timestamp_ns(),
            Some(1000)
        );
    }
}
#[test]
fn semantic_malformations_are_preserved_and_never_become_valid_observations() {
    let raw = pcap(
        Endian::Little,
        false,
        1,
        2,
        &[(1, 1_000_000, b"malformed", 1)],
    );
    let parsed = entries(&raw);
    assert_eq!(parsed[1].payload(), Some(b"malformed".as_slice()));
    let Content::Packet(p) = parsed[1].content() else {
        panic!()
    };
    assert_eq!(
        p.issues,
        [
            Issue::CapturedExceedsOriginal,
            Issue::CapturedExceedsSnapshot,
            Issue::InvalidTimestamp
        ]
    );
    assert_eq!(parsed[1].observation(0).unwrap_err().code, Code::Record);
    let mut r = Reader::new(Cursor::new(&raw), Limits::default()).unwrap();
    assert_eq!(copy_all(&mut r, vec![]).unwrap(), raw);
    let mut raw = shb(Endian::Little, -1);
    raw.extend(idb(Endian::Little, 1, 1, 9, 0));
    raw.extend(epb(Endian::Little, 0, 0, &vec![5; 65537], 2, 0));
    let parsed = entries(&raw);
    let Content::Packet(p) = parsed[2].content() else {
        panic!()
    };
    assert!(p.issues.contains(&Issue::PayloadLimit));
    assert_eq!(parsed[2].payload().unwrap().len(), 65537);
    assert_eq!(parsed[2].observation(0).unwrap_err().code, Code::Record);
}
#[test]
fn native_timestamp_extremes_are_exact_or_explicitly_unrepresentable() {
    for (clock, ticks, expected) in [
        (
            Clock {
                resolution: 9,
                offset_seconds: 0,
            },
            u64::MAX,
            Some(u64::MAX),
        ),
        (
            Clock {
                resolution: 0x80 | 10,
                offset_seconds: -2,
            },
            3072,
            Some(1_000_000_000),
        ),
        (
            Clock {
                resolution: 10,
                offset_seconds: 0,
            },
            1,
            None,
        ),
        (
            Clock {
                resolution: 127,
                offset_seconds: 0,
            },
            0,
            Some(0),
        ),
        (
            Clock {
                resolution: 127,
                offset_seconds: 0,
            },
            1,
            None,
        ),
        (
            Clock {
                resolution: 0xff,
                offset_seconds: 0,
            },
            u64::MAX,
            None,
        ),
        (
            Clock {
                resolution: 0,
                offset_seconds: i64::MAX,
            },
            u64::MAX,
            None,
        ),
        (
            Clock {
                resolution: 9,
                offset_seconds: -1,
            },
            0,
            None,
        ),
    ] {
        assert_eq!(Timestamp::Ng { ticks, clock }.nanoseconds(), expected);
    }
    let mut raw = shb(Endian::Big, -1);
    raw.extend(idb(Endian::Big, 1, 0, 10, 0));
    raw.extend(epb(Endian::Big, 0, 1, b"subnanosecond", 13, 0));
    let parsed = entries(&raw);
    assert_eq!(parsed[2].observation(0).unwrap_err().code, Code::Timestamp);
    assert!(matches!(
        parsed[2].content(),
        Content::Packet(Packet {
            timestamp: Some(Timestamp::Ng { ticks: 1, .. }),
            ..
        })
    ));
    let mut reader = Reader::new(Cursor::new(&raw), Limits::default()).unwrap();
    assert_eq!(copy_all(&mut reader, vec![]).unwrap(), raw);
}
#[test]
fn encoder_streams_network_observations_and_rejects_lossy_projection() {
    for encoding in [Encoding::Pcap { link_type: 65535 }, Encoding::PcapNg] {
        let mut w = Encoder::new(vec![], encoding).unwrap();
        for (i, payload) in [b"binary\0\xff".as_slice(), b"", b"selected"]
            .into_iter()
            .enumerate()
        {
            w.write_observation(
                100,
                &obs(65535, 1_123_456_789 + i as u64, Direction::Unknown, payload),
            )
            .unwrap();
        }
        let bytes = w.finish().unwrap();
        let parsed = entries(&bytes);
        let packets: Vec<_> = parsed
            .iter()
            .filter_map(|e| e.payload().map(|_| e.observation(9).unwrap()))
            .collect();
        assert_eq!(packets.len(), 3);
        assert_eq!(packets[0].bytes(), b"binary\0\xff");
        assert_eq!(packets[1].bytes(), b"");
        assert_eq!(packets[2].source_timestamp_ns(), Some(1_123_456_791));
        assert_eq!(packets[2].original_length(), Some(10));
    }
    let mut w = Encoder::new(vec![], Encoding::PcapNg).unwrap();
    w.write_observation(1000, &obs(1, u64::MAX, Direction::Rx, b"one"))
        .unwrap();
    w.write_observation(42, &obs(101, 1, Direction::Tx, b"two"))
        .unwrap();
    assert_eq!(
        w.write_observation(42, &obs(1, 1, Direction::Rx, b"wrong"))
            .unwrap_err()
            .code,
        Code::Interface
    );
    let parsed = entries(&w.finish().unwrap());
    let p: Vec<_> = parsed.iter().filter(|e| e.payload().is_some()).collect();
    assert_eq!(
        p[0].observation(0).unwrap().source_timestamp_ns(),
        Some(u64::MAX)
    );
    assert_eq!(p[1].observation(0).unwrap().direction(), Direction::Tx);
    let mut w = Encoder::new(vec![], Encoding::Pcap { link_type: 1 }).unwrap();
    assert_eq!(
        w.write_observation(0, &obs(1, 1, Direction::Rx, b"x"))
            .unwrap_err()
            .code,
        Code::Unsupported
    );
    w.write_observation(0, &obs(1, 1, Direction::Unknown, b"x"))
        .unwrap();
    for o in [
        obs(101, 1, Direction::Unknown, b"x"),
        obs(1, u64::MAX, Direction::Unknown, b"x"),
    ] {
        assert!(w.write_observation(0, &o).is_err());
    }
    assert_eq!(
        w.write_observation(1, &obs(1, 1, Direction::Unknown, b"x"))
            .unwrap_err()
            .code,
        Code::Unsupported
    );
    let missing = Observation::new(
        1,
        Direction::Unknown,
        Metadata::Ethernet { link_type: 1 },
        None,
        b"x",
    )
    .unwrap();
    assert_eq!(
        w.write_observation(0, &missing).unwrap_err().code,
        Code::Record
    );
    let missing = Observation::new(
        1,
        Direction::Unknown,
        Metadata::Ethernet { link_type: 1 },
        Some(1),
        b"x",
    )
    .unwrap();
    assert_eq!(
        w.write_observation(0, &missing).unwrap_err().code,
        Code::Timestamp
    );
    let wrong = Observation::new(
        1,
        Direction::Unknown,
        Metadata::Serial {
            configuration_revision: 1,
        },
        Some(1),
        b"x",
    )
    .unwrap()
    .with_source_timestamp(1);
    assert_eq!(
        w.write_observation(0, &wrong).unwrap_err().code,
        Code::Unsupported
    );
}
#[test]
fn malformed_container_errors_have_exact_locations_and_consumed_source_bytes() {
    let e = Endian::Little;
    assert_eq!(fail(b"bad!", Code::Magic, 0).raw(), b"bad!");
    let mut raw = pcap(e, false, 1, 64, &[]);
    raw[4] = 3;
    assert_eq!(fail(&raw, Code::Version, 4).raw(), raw);
    let mut raw = shb(e, -1);
    let last = raw.len() - 4;
    raw[last] = 0;
    assert_eq!(fail(&raw, Code::Trailer, last as u64).raw(), raw);
    let raw = ng_block(0x0a0d0d0a, &[0; 16], e);
    fail(&raw, Code::Magic, 8);
    let mut raw = shb(e, -1);
    raw[4..8].copy_from_slice(&29u32.to_le_bytes());
    fail(&raw, Code::Length, 4);
    let mut raw = shb(e, -1);
    raw[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
    fail(&raw, Code::Length, 4);
    let mut raw = shb(e, -1);
    raw[4..8].copy_from_slice(&(MAX_BLOCK as u32 + 4).to_le_bytes());
    let err = fail(&raw, Code::Limit, 4);
    assert_eq!(err.raw().len(), 12);
    let mut raw = shb(e, -1);
    raw.extend(epb(e, 0, 0, b"x", 1, 0));
    fail(&raw, Code::Interface, 36);
    let mut raw = shb(e, 4);
    raw.extend(idb(e, 1, 64, 9, 0));
    fail(&raw, Code::Section, 28);
    let raw = shb(e, 4);
    fail(&raw, Code::Section, 28);
    let mut raw = shb(e, 4);
    raw.extend(shb(e, -1));
    fail(&raw, Code::Section, 28);
    let mut raw = shb(e, -1);
    let mut bad = idb(e, 1, 64, 9, 0);
    bad[18..20].copy_from_slice(&u16::MAX.to_le_bytes());
    raw.extend(bad);
    fail(&raw, Code::Options, 46);
    let mut raw = pcap(e, false, 1, 64, &[(0, 0, b"x", 1)]);
    raw[32..36].copy_from_slice(&65537u32.to_le_bytes());
    assert_eq!(fail(&raw, Code::Limit, 32).raw().len(), 16);
}
#[test]
fn every_prefix_and_deterministic_hostile_mutation_remains_bounded() {
    let mut ng = shb(Endian::Big, -1);
    ng.extend(idb(Endian::Big, 65535, 0, 0xff, i64::MIN));
    ng.extend(epb(Endian::Big, 0, u64::MAX, b"opaque", 8, 1));
    for raw in [pcap(Endian::Little, true, 1, 64, &[(1, 2, b"abc", 5)]), ng] {
        let boundaries: std::collections::BTreeSet<_> = entries(&raw)
            .iter()
            .map(|e| e.offset() as usize)
            .chain([raw.len()])
            .filter(|n| *n > 0)
            .collect();
        for end in 0..raw.len() {
            let mut reader = Reader::new(Cursor::new(&raw[..end]), Limits::default()).unwrap();
            let result = loop {
                match reader.next_entry() {
                    Ok(Some(_)) => {}
                    other => break other,
                }
            };
            if boundaries.contains(&end) {
                assert!(matches!(result, Ok(None)));
            } else {
                let err = result.unwrap_err();
                assert_eq!(err.code, Code::Truncated);
                assert_eq!(err.offset, end as u64);
                assert!(err.raw().len() <= MAX_BLOCK);
            }
        }
        for index in 0..raw.len() {
            for mask in [1, 0x80, 0xff] {
                let mut bytes = raw.clone();
                bytes[index] ^= mask;
                let mut reader = Reader::new(
                    Cursor::new(&bytes),
                    Limits {
                        block_bytes: 4096,
                        interfaces: 4,
                        ..Limits::default()
                    },
                )
                .unwrap();
                for _ in 0..bytes.len() + 1 {
                    match reader.next_entry() {
                        Ok(Some(e)) => {
                            assert!(e.raw().len() <= 4096);
                            if e.payload().is_some() {
                                let _ = e.observation(0);
                            }
                        }
                        Ok(None) => break,
                        Err(err) => {
                            assert!(err.raw().len() <= 4096);
                            break;
                        }
                    }
                }
            }
        }
    }
}
struct OneByte<R>(R);
impl<R: Read> Read for OneByte<R> {
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        let n = b.len().min(1);
        self.0.read(&mut b[..n])
    }
}
struct Fails {
    remaining: usize,
}
impl Read for Fails {
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        if self.remaining == 0 {
            return Err(io::Error::other("private path and bytes"));
        }
        let n = b.len().min(self.remaining);
        b[..n].fill(0);
        self.remaining -= n;
        Ok(n)
    }
}
struct PartialWrite {
    limit: usize,
    bytes: Vec<u8>,
}
impl Write for PartialWrite {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        if self.bytes.len() == self.limit {
            return Err(io::Error::other("sensitive backend"));
        }
        let n = b.len().min(3).min(self.limit - self.bytes.len());
        self.bytes.extend_from_slice(&b[..n]);
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
#[test]
fn limits_short_io_and_partial_write_failures_are_explicit() {
    let raw = pcap(Endian::Big, true, 1, 64, &[(1, 2, b"abc", 3)]);
    let mut r = Reader::new(OneByte(Cursor::new(&raw)), Limits::default()).unwrap();
    assert_eq!(copy_all(&mut r, vec![]).unwrap(), raw);
    let mut r = Reader::new(
        Cursor::new(&raw),
        Limits {
            total_bytes: raw.len() as u64,
            entries: 2,
            ..Limits::default()
        },
    )
    .unwrap();
    assert_eq!(copy_all(&mut r, vec![]).unwrap(), raw);
    let mut r = Reader::new(
        Cursor::new(&raw),
        Limits {
            total_bytes: 24,
            entries: 2,
            ..Limits::default()
        },
    )
    .unwrap();
    r.next_entry().unwrap();
    assert_eq!(r.next_entry().unwrap_err().code, Code::Limit);
    let mut r = Reader::new(
        Cursor::new(&raw),
        Limits {
            entries: 1,
            ..Limits::default()
        },
    )
    .unwrap();
    r.next_entry().unwrap();
    assert_eq!(r.next_entry().unwrap_err().code, Code::Limit);
    let mut raw = shb(Endian::Little, -1);
    raw.extend(idb(Endian::Little, 1, 64, 9, 0));
    raw.extend(idb(Endian::Little, 1, 64, 9, 0));
    let mut r = Reader::new(
        Cursor::new(&raw),
        Limits {
            interfaces: 1,
            ..Limits::default()
        },
    )
    .unwrap();
    r.next_entry().unwrap();
    r.next_entry().unwrap();
    let error = r.next_entry().unwrap_err();
    assert_eq!(error.code, Code::Limit);
    assert!(r.interface_count() <= 1);
    let mut r = Reader::new(Fails { remaining: 2 }, Limits::default()).unwrap();
    let e = r.next_entry().unwrap_err();
    assert_eq!((e.code, e.offset), (Code::Io, 2));
    assert_eq!(e.raw(), [0, 0]);
    assert!(!format!("{e:?} {e}").contains("private"));
    let mut out = PartialWrite {
        limit: 30,
        bytes: vec![],
    };
    let mut encoder = Encoder::new(&mut out, Encoding::Pcap { link_type: 1 }).unwrap();
    let o = obs(1, 1, Direction::Unknown, b"abcdef");
    let e = encoder.write_observation(0, &o).unwrap_err();
    assert_eq!((e.code, e.offset), (Code::Io, 30));
    assert_eq!(
        encoder.write_observation(0, &o).unwrap_err().code,
        Code::State
    );
    assert!(encoder.finish().is_err());
    assert_eq!(out.bytes.len(), 30);
    let mut r = Reader::new(Cursor::new(raw), Limits::default()).unwrap();
    r.next_entry().unwrap();
    assert_eq!(copy_all(&mut r, vec![]).unwrap_err().code, Code::State);
}
struct Repeated {
    header: Vec<u8>,
    record: Vec<u8>,
    position: u64,
    count: u64,
    max_request: std::rc::Rc<std::cell::Cell<usize>>,
}
impl Read for Repeated {
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        self.max_request.set(self.max_request.get().max(b.len()));
        let total = self.header.len() as u64 + self.record.len() as u64 * self.count;
        if self.position == total {
            return Ok(0);
        }
        let n = b.len().min((total - self.position) as usize);
        for slot in &mut b[..n] {
            *slot = if self.position < (self.header.len() as u64) {
                self.header[self.position as usize]
            } else {
                self.record[((self.position - self.header.len() as u64) % self.record.len() as u64)
                    as usize]
            };
            self.position += 1;
        }
        Ok(n)
    }
}
#[test]
fn large_generated_stream_has_no_history_and_debug_redacts_capture_data() {
    let bytes = pcap(
        Endian::Little,
        false,
        1,
        65536,
        &[(123, 456, b"sensitive-payload", 20)],
    );
    let max_request = std::rc::Rc::new(std::cell::Cell::new(0));
    let input = Repeated {
        header: bytes[..24].to_vec(),
        record: bytes[24..].to_vec(),
        position: 0,
        count: 1_000_000,
        max_request: max_request.clone(),
    };
    let mut r = Reader::new(input, Limits::default()).unwrap();
    let mut count = 0;
    while let Some(e) = r.next_entry().unwrap() {
        assert!(e.raw().len() < 64);
        assert!(!format!("{e:?}").contains("sensitive"));
        count += 1;
    }
    assert_eq!(count, 1_000_001);
    assert_eq!(r.position(), 24 + 1_000_000 * bytes[24..].len() as u64);
    assert_eq!(r.interface_count(), 0);
    assert!(max_request.get() <= 20);
}

#[test]
fn selected_rewrite_respects_custom_copy_semantics_and_upgrades_legacy_packet_blocks() {
    for e in [Endian::Little, Endian::Big] {
        let mut section = shb(e, -1);
        let minor = match e {
            Endian::Little => 2u16.to_le_bytes(),
            Endian::Big => 2u16.to_be_bytes(),
        };
        section[14..16].copy_from_slice(&minor);
        let mut body = vec![];
        n16(&mut body, 1, e);
        n16(&mut body, 0, e);
        n32(&mut body, 64, e);
        option(&mut body, 19373, b"PEN!noncopy", e);
        option(&mut body, 2989, b"PEN!copy", e);
        option(&mut body, 0, &[], e);
        section.extend(ng_block(1, &body, e));
        section.extend(ng_block(0x40000bad, b"PEN!omit", e));
        section.extend(ng_block(0xbad, b"PEN!keep", e));
        let mut body = vec![];
        n16(&mut body, 0, e);
        n16(&mut body, 7, e);
        for n in [0, 1, 3, 5] {
            n32(&mut body, n, e);
        }
        body.extend_from_slice(&[1, 2, 3, 0]);
        section.extend(ng_block(2, &body, e));
        let mut reader = Reader::new(Cursor::new(&section), Limits::default()).unwrap();
        assert_eq!(copy_all(&mut reader, vec![]).unwrap(), section);
        let mut reader = Reader::new(Cursor::new(&section), Limits::default()).unwrap();
        let rewritten = copy_selected(&mut reader, vec![], |_| true).unwrap();
        let parsed = entries(&rewritten);
        assert_eq!(parsed.len(), 4);
        assert_eq!(&parsed[0].raw()[14..16], &[0, 0]);
        assert!(!rewritten.windows(7).any(|w| w == b"noncopy"));
        assert!(rewritten.windows(8).any(|w| w == b"PEN!copy"));
        assert!(matches!(
            parsed[2].content(),
            Content::Opaque { block_type: 0xbad }
        ));
        assert_eq!(parsed[3].payload(), Some([1, 2, 3].as_slice()));
        assert_eq!(parsed[3].observation(0).unwrap().original_length(), Some(5));
        let epb = parsed[3].raw();
        assert_eq!(
            &epb[..4],
            &match e {
                Endian::Little => 6u32.to_le_bytes(),
                Endian::Big => 6u32.to_be_bytes(),
            }
        );
        assert!(epb.windows(8).any(|w| w
            == match e {
                Endian::Little => 7u64.to_le_bytes(),
                Endian::Big => 7u64.to_be_bytes(),
            }));
    }
}
#[test]
fn hard_interface_option_and_block_caps_are_exercised() {
    let mut writer = Encoder::new(vec![], Encoding::PcapNg).unwrap();
    for id in 0..256 {
        writer
            .write_observation(id, &obs(1, 0, Direction::Unknown, b""))
            .unwrap();
    }
    assert_eq!(
        writer
            .write_observation(256, &obs(1, 0, Direction::Unknown, b""))
            .unwrap_err()
            .code,
        Code::Limit
    );
    let raw = writer.finish().unwrap();
    let mut reader = Reader::new(
        Cursor::new(raw),
        Limits {
            interfaces: 256,
            ..Limits::default()
        },
    )
    .unwrap();
    while reader.next_entry().unwrap().is_some() {}
    assert_eq!(reader.interface_count(), 256);
    let e = Endian::Little;
    let mut body = vec![];
    n32(&mut body, 1, e);
    n32(&mut body, 64, e);
    for _ in 0..1024 {
        option(&mut body, 3000, &[], e);
    }
    let mut raw = shb(e, -1);
    raw.extend(ng_block(1, &body, e));
    assert_eq!(entries(&raw).len(), 2);
    option(&mut body, 3000, &[], e);
    let mut raw = shb(e, -1);
    raw.extend(ng_block(1, &body, e));
    fail(&raw, Code::Limit, 28 + 16 + 1024 * 4);
    let mut raw = shb(e, -1);
    raw.extend(ng_block(0x1234, &vec![0; MAX_BLOCK - 12], e));
    assert_eq!(entries(&raw)[1].raw().len(), MAX_BLOCK);
    assert!(
        Reader::new(
            Cursor::new([]),
            Limits {
                interfaces: 257,
                ..Limits::default()
            }
        )
        .is_err()
    );
}
