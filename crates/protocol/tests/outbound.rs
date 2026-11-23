use exmon_domain::config::{Endian, Escape, Framing};
use exmon_protocol::outbound::{Error, encode};
#[test]
fn exact_framing_vectors_and_preserved_headers() {
    assert_eq!(encode(&Framing::Datagram {}, None, &[], b""), Ok(vec![]));
    assert_eq!(
        encode(&Framing::Fixed { length: 3 }, None, &[], b"abc"),
        Ok(b"abc".to_vec())
    );
    assert_eq!(
        encode(&Framing::Fixed { length: 3 }, None, &[], b"ab"),
        Err(Error::Bounds)
    );
    let delimiter = Framing::Delimiter {
        bytes: b"ABA".to_vec(),
        max_length: 64,
    };
    assert_eq!(
        encode(&delimiter, None, &[], b"xyz"),
        Ok(b"xyzABA".to_vec())
    );
    assert_eq!(encode(&delimiter, None, &[], b"AB"), Err(Error::Ambiguous));
    let markers = Framing::Markers {
        start: vec![0xaa],
        end: vec![0xbb],
        max_length: 64,
    };
    assert_eq!(
        encode(&markers, None, &[], &[1, 2]),
        Ok(vec![0xaa, 1, 2, 0xbb])
    );
    assert_eq!(encode(&markers, None, &[], &[0xaa]), Err(Error::Ambiguous));
    for width in [1, 2, 4] {
        for endian in [Endian::Little, Endian::Big] {
            let rule = Framing::LengthPrefix {
                offset: 2,
                width,
                endian,
                max_length: 64,
            };
            let mut header = vec![0xa5, 0x55];
            header.resize(2 + usize::from(width), 0xff);
            let wire = encode(&rule, None, &header, &[9, 8, 7]).unwrap();
            let mut expected = vec![0xa5, 0x55];
            let mut length = vec![0; usize::from(width)];
            length[if endian == Endian::Little {
                0
            } else {
                usize::from(width) - 1
            }] = 3;
            expected.extend(length);
            expected.extend([9, 8, 7]);
            assert_eq!(wire, expected);
        }
    }
    assert_eq!(
        encode(
            &Framing::IdleGap {
                gap_ns: 10,
                max_length: 64
            },
            None,
            &[],
            b"abc"
        ),
        Ok(b"abc".to_vec())
    );
}
#[test]
fn escaping_limits_ambiguous_empty_and_malformed_inputs_fail_closed() {
    let rule = Framing::Delimiter {
        bytes: vec![10],
        max_length: 65536,
    };
    let escape = Some(Escape {
        byte: 0x7d,
        xor: 0x20,
    });
    assert_eq!(
        encode(&rule, escape, &[], &[10, 0x7d, 1]),
        Ok(vec![0x7d, 0x2a, 0x7d, 0x5d, 1, 10])
    );
    assert_eq!(encode(&rule, None, &[], &[10]), Err(Error::Ambiguous));
    assert_eq!(
        encode(&rule, escape, &[], &vec![0x7d; 32768]),
        Err(Error::Bounds)
    );
    assert!(encode(&Framing::Fixed { length: 0 }, None, &[], &[]).is_err());
    assert_eq!(
        encode(
            &Framing::LengthPrefix {
                offset: 0,
                width: 1,
                endian: Endian::Big,
                max_length: 65536
            },
            None,
            &[0],
            &vec![1; 256]
        ),
        Err(Error::Bounds)
    );
    assert_eq!(
        encode(
            &Framing::LengthPrefix {
                offset: 1,
                width: 2,
                endian: Endian::Big,
                max_length: 64
            },
            None,
            &[],
            &[]
        ),
        Err(Error::Header)
    );
    assert_eq!(
        encode(&Framing::Datagram {}, None, &[0], &[]),
        Err(Error::Header)
    );
    assert_eq!(
        encode(
            &Framing::IdleGap {
                gap_ns: 1,
                max_length: 64
            },
            None,
            &[],
            &[]
        ),
        Err(Error::Ambiguous)
    );
    assert_eq!(
        encode(&Framing::Datagram {}, None, &[], &vec![0; 65537]),
        Err(Error::Bounds)
    );
    let p: Vec<_> = (0..=255).collect();
    assert!(encode(&rule, escape, &[], &p).is_ok());
}

#[test]
fn independent_escape_reference_and_roundtrip_cover_control_bytes_and_unknown_length_header() {
    let rule = Framing::Markers {
        start: vec![0xaa, 0x55],
        end: vec![0xbb, 0x66],
        max_length: 65536,
    };
    let escape = Some(Escape {
        byte: 0x7d,
        xor: 0x20,
    });
    let mut random = 0x12345678u32;
    for length in 0..128 {
        let mut body = vec![];
        for _ in 0..length {
            random ^= random << 13;
            random ^= random >> 17;
            random ^= random << 5;
            body.push(random as u8)
        }
        let wire = encode(&rule, escape, &[], &body).unwrap();
        let mut expected = vec![0xaa, 0x55];
        for b in &body {
            if [0xaa, 0xbb, 0x7d].contains(b) {
                expected.push(0x7d);
                expected.push(b ^ 0x20)
            } else {
                expected.push(*b)
            }
        }
        expected.extend([0xbb, 0x66]);
        assert_eq!(wire, expected);
    }
    let prefix = Framing::LengthPrefix {
        offset: 2,
        width: 2,
        endian: Endian::Little,
        max_length: 65536,
    };
    let body = vec![0x11; 125];
    let wire = encode(&prefix, escape, &[0x7d, 0x99, 0, 0], &body).unwrap();
    assert_eq!(&wire[..6], &[0x7d, 0x5d, 0x99, 0x7d, 0x5d, 0]);
    assert_eq!(
        encode(&Framing::Datagram {}, None, &[], &vec![0; 65536])
            .unwrap()
            .len(),
        65536
    );
}
