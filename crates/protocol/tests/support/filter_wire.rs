//! Shared synthetic wire inputs for filter contract tests; checksums deliberately
//! unfilled. Filtering must not silently exclude a capture with checksum offload.
#![allow(dead_code)]
use exmon_domain::{
    Direction,
    observation::{Metadata, Observation},
};
pub fn wire(
    v6: bool,
    tcp: bool,
    source_port: u16,
    destination_port: u16,
    tags: usize,
    extensions: bool,
) -> Vec<u8> {
    let mut transport = vec![0u8; if tcp { 20 } else { 8 }];
    transport[..2].copy_from_slice(&source_port.to_be_bytes());
    transport[2..4].copy_from_slice(&destination_port.to_be_bytes());
    if tcp {
        transport[12] = 0x50;
    } else {
        transport[4..6].copy_from_slice(&15u16.to_be_bytes());
    }
    transport.extend(b"payload");
    let protocol = if tcp { 6 } else { 17 };
    let mut ip = if v6 {
        let mut ip = vec![0; 40];
        ip[0] = 0x60;
        ip[6] = if extensions { 0 } else { protocol };
        ip[7] = 64;
        ip[8..24].copy_from_slice(
            &"2001:db8::1"
                .parse::<std::net::Ipv6Addr>()
                .unwrap()
                .octets(),
        );
        ip[24..40].copy_from_slice(
            &"2001:db8::2"
                .parse::<std::net::Ipv6Addr>()
                .unwrap()
                .octets(),
        );
        if extensions {
            ip.extend([60, 0, 0, 0, 0, 0, 0, 0, protocol, 0, 0, 0, 0, 0, 0, 0]);
        }
        let length = (ip.len() - 40 + transport.len()) as u16;
        ip[4..6].copy_from_slice(&length.to_be_bytes());
        ip
    } else {
        let mut ip = vec![0; if extensions { 24 } else { 20 }];
        ip[0] = if extensions { 0x46 } else { 0x45 };
        ip[8] = 64;
        ip[9] = protocol;
        let length = (ip.len() + transport.len()) as u16;
        ip[2..4].copy_from_slice(&length.to_be_bytes());
        ip[12..16].copy_from_slice(&[192, 0, 2, 1]);
        ip[16..20].copy_from_slice(&[198, 51, 100, 2]);
        if extensions {
            ip[20..24].copy_from_slice(&[1, 1, 0, 0]);
        }
        ip
    };
    ip.extend(transport);
    let mut raw = vec![2, 0, 0, 0, 0, 2, 2, 0, 0, 0, 0, 1];
    for i in 0..tags {
        raw.extend(if i == 0 { 0x88a8u16 } else { 0x8100u16 }.to_be_bytes());
        raw.extend((i as u16).to_be_bytes());
    }
    raw.extend(if v6 { 0x86ddu16 } else { 0x0800u16 }.to_be_bytes());
    raw.extend(ip);
    raw
}
pub fn observation(raw: &[u8], direction: Direction) -> Observation {
    Observation::new(
        42,
        direction,
        Metadata::Ethernet { link_type: 1 },
        Some(raw.len()),
        raw,
    )
    .unwrap()
    .with_source_timestamp(99)
}
