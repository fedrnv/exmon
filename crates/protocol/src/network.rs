//! Bounded, allocation-light inspection of Ethernet II frames. No reassembly or I/O.
use exmon_domain::observation::{Metadata, Observation};
use std::{
    fmt,
    net::{Ipv4Addr, Ipv6Addr},
    ops::Range,
};

pub const MAX_FRAME_BYTES: usize = 65_536;
pub const MAX_VLANS: usize = 8;
pub const MAX_EXTENSIONS: usize = 16;
pub const MAX_HEADER_BYTES: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub frame_bytes: usize,
    pub vlan_tags: usize,
    pub ipv6_extensions: usize,
    pub header_bytes: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            frame_bytes: MAX_FRAME_BYTES,
            vlan_tags: MAX_VLANS,
            ipv6_extensions: MAX_EXTENSIONS,
            header_bytes: MAX_HEADER_BYTES,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    Link,
    Ethernet,
    Vlan,
    Ipv4,
    Ipv6,
    Extension,
    Tcp,
    Udp,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Limit {
    FrameBytes,
    VlanTags,
    Extensions,
    HeaderBytes,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Diagnostic {
    Truncated {
        layer: Layer,
        offset: usize,
        needed: usize,
        available: usize,
    },
    Malformed {
        layer: Layer,
        offset: usize,
        reason: &'static str,
    },
    Limited {
        limit: Limit,
        offset: usize,
    },
    Unsupported {
        layer: Layer,
        protocol: u32,
    },
    Fragmented {
        layer: Layer,
    },
    Encrypted,
    JumbogramUnsupported,
}
/// A checksum describes captured bytes, not necessarily the bytes on the wire:
/// host offload can leave a checksum unfilled in a capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Checksum {
    Valid,
    Invalid,
    Unavailable,
    NotPresent,
}

#[derive(Clone, PartialEq, Eq)]
pub struct Ethernet {
    pub source: [u8; 6],
    pub destination: [u8; 6],
    /// Inner type after VLAN tags; values <=1500 are IEEE 802.3 lengths.
    pub ether_type: u16,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Vlan {
    pub tpid: u16,
    pub tci: u16,
}
impl Vlan {
    pub fn id(self) -> u16 {
        self.tci & 0x0fff
    }
    pub fn priority(self) -> u8 {
        (self.tci >> 13) as u8
    }
    pub fn drop_eligible(self) -> bool {
        self.tci & 0x1000 != 0
    }
}
#[derive(Clone, PartialEq, Eq)]
pub struct Ipv4 {
    pub source: Ipv4Addr,
    pub destination: Ipv4Addr,
    pub protocol: u8,
    pub dscp_ecn: u8,
    pub ttl: u8,
    pub identification: u16,
    pub dont_fragment: bool,
    pub more_fragments: bool,
    pub fragment_offset: usize,
    pub total_length: usize,
    pub header: Range<usize>,
    pub checksum: Checksum,
}
#[derive(Clone, PartialEq, Eq)]
pub struct Ipv6 {
    pub source: Ipv6Addr,
    pub destination: Ipv6Addr,
    pub traffic_class: u8,
    pub flow_label: u32,
    pub hop_limit: u8,
    pub next_header: u8,
    pub payload_length: usize,
    pub header: Range<usize>,
}
#[derive(Clone, PartialEq, Eq)]
pub enum Ip {
    V4(Ipv4),
    V6(Ipv6),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Extension {
    pub kind: u8,
    pub next_header: u8,
    pub range: Range<usize>,
    pub fragment: Option<Fragment>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fragment {
    pub offset: usize,
    pub more: bool,
    pub identification: u32,
}
#[derive(Clone, PartialEq, Eq)]
pub struct Tcp {
    pub source_port: u16,
    pub destination_port: u16,
    pub sequence: u32,
    pub acknowledgement: u32,
    pub flags: u16,
    pub window: u16,
    pub urgent_pointer: u16,
    pub header: Range<usize>,
    pub options: Range<usize>,
    pub checksum: Checksum,
}
#[derive(Clone, PartialEq, Eq)]
pub struct Udp {
    pub source_port: u16,
    pub destination_port: u16,
    pub length: usize,
    pub header: Range<usize>,
    pub checksum: Checksum,
}
#[derive(Clone, PartialEq, Eq)]
pub enum Transport {
    Tcp(Tcp),
    Udp(Udp),
}
// Debug output must not accidentally expose captured endpoints or contents.
macro_rules! redacted_debug {
    ($($t:ty),+ $(,)?) => { $(impl fmt::Debug for $t {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str(concat!(stringify!($t), "([REDACTED])"))
        }
    })+ };
}
redacted_debug!(Ethernet, Ipv4, Ipv6, Ip, Tcp, Udp, Transport);

pub struct Packet<'a> {
    raw: &'a [u8],
    payload: Range<usize>,
    pub ethernet: Option<Ethernet>,
    pub vlans: Vec<Vlan>,
    pub ip: Option<Ip>,
    pub extensions: Vec<Extension>,
    pub transport: Option<Transport>,
    pub diagnostic: Option<Diagnostic>,
    /// Includes truncation known only from the capture's original-length metadata.
    pub capture_truncated: bool,
}
impl fmt::Debug for Packet<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Packet")
            .field("captured_length", &self.raw.len())
            .field("diagnostic", &self.diagnostic)
            .field("capture_truncated", &self.capture_truncated)
            .finish()
    }
}
impl<'a> Packet<'a> {
    /// The complete unmodified input, including unknown data and Ethernet padding.
    pub fn raw(&self) -> &'a [u8] {
        self.raw
    }
    /// Deepest safely identified payload; inspect `diagnostic` before interpreting it.
    pub fn payload(&self) -> &'a [u8] {
        &self.raw[self.payload.clone()]
    }
    pub fn payload_range(&self) -> Range<usize> {
        self.payload.clone()
    }
    fn new(raw: &'a [u8]) -> Self {
        Self {
            raw,
            payload: 0..raw.len(),
            ethernet: None,
            vlans: Vec::new(),
            ip: None,
            extensions: Vec::new(),
            transport: None,
            diagnostic: None,
            capture_truncated: false,
        }
    }
}
#[derive(Debug, Clone, Copy, Default)]
pub struct Decoder {
    limits: Limits,
}
impl Decoder {
    pub fn new(limits: Limits) -> Result<Self, &'static str> {
        if !(1..=MAX_FRAME_BYTES).contains(&limits.frame_bytes)
            || limits.vlan_tags > MAX_VLANS
            || limits.ipv6_extensions > MAX_EXTENSIONS
            || !(14..=MAX_HEADER_BYTES).contains(&limits.header_bytes)
        {
            return Err("network decoder limits exceed hard bounds");
        }
        Ok(Self { limits })
    }
    pub fn limits(&self) -> Limits {
        self.limits
    }
    pub fn decode<'a>(&self, raw: &'a [u8]) -> Packet<'a> {
        let mut packet = Packet::new(raw);
        packet.diagnostic = self.parse(&mut packet).err();
        packet
    }
    pub fn decode_observation<'a>(&self, observation: &'a Observation) -> Packet<'a> {
        let mut packet = match observation.metadata() {
            Metadata::Ethernet { link_type: 1 } => self.decode(observation.bytes()),
            metadata => {
                let mut packet = Packet::new(observation.bytes());
                packet.diagnostic = Some(Diagnostic::Unsupported {
                    layer: Layer::Link,
                    protocol: match metadata {
                        Metadata::Ethernet { link_type } => *link_type,
                        _ => u32::MAX,
                    },
                });
                packet
            }
        };
        packet.capture_truncated |= observation
            .original_length()
            .is_some_and(|n| n > packet.raw.len());
        packet
    }
    fn header(
        &self,
        raw: &[u8],
        offset: usize,
        length: usize,
        end: usize,
        layer: Layer,
    ) -> Result<(), Diagnostic> {
        if length > end.saturating_sub(offset) {
            return Err(Diagnostic::Malformed {
                layer,
                offset,
                reason: "header exceeds declared PDU",
            });
        }
        if offset + length > self.limits.header_bytes {
            return Err(Diagnostic::Limited {
                limit: Limit::HeaderBytes,
                offset,
            });
        }
        if length > raw.len().saturating_sub(offset) {
            return Err(Diagnostic::Truncated {
                layer,
                offset,
                needed: length,
                available: raw.len().saturating_sub(offset),
            });
        }
        Ok(())
    }
    fn parse(&self, p: &mut Packet<'_>) -> Result<(), Diagnostic> {
        let b = p.raw;
        if b.len() > self.limits.frame_bytes {
            return Err(Diagnostic::Limited {
                limit: Limit::FrameBytes,
                offset: 0,
            });
        }
        // Link-layer length is unknown until its fixed header is available.
        self.header(b, 0, 14, usize::MAX, Layer::Ethernet)?;
        let mut ether_type = u16_at(b, 12);
        p.ethernet = Some(Ethernet {
            destination: b[0..6].try_into().unwrap(),
            source: b[6..12].try_into().unwrap(),
            ether_type,
        });
        let mut offset = 14;
        p.payload = offset..b.len();
        while matches!(ether_type, 0x8100 | 0x88a8) {
            if p.vlans.len() == self.limits.vlan_tags {
                return Err(Diagnostic::Limited {
                    limit: Limit::VlanTags,
                    offset,
                });
            }
            self.header(b, offset, 4, usize::MAX, Layer::Vlan)?;
            p.vlans.push(Vlan {
                tpid: ether_type,
                tci: u16_at(b, offset),
            });
            ether_type = u16_at(b, offset + 2);
            offset += 4;
            p.ethernet.as_mut().unwrap().ether_type = ether_type;
            p.payload = offset..b.len();
        }
        match ether_type {
            0x0800 => self.ipv4(p, offset),
            0x86dd => self.ipv6(p, offset),
            other => Err(Diagnostic::Unsupported {
                layer: Layer::Ethernet,
                protocol: other.into(),
            }),
        }
    }
    fn ipv4(&self, p: &mut Packet<'_>, offset: usize) -> Result<(), Diagnostic> {
        let b = p.raw;
        self.header(b, offset, 20, usize::MAX, Layer::Ipv4)?;
        let size = usize::from(b[offset] & 15) * 4;
        let total = usize::from(u16_at(b, offset + 2));
        let fragment = u16_at(b, offset + 6);
        if b[offset] >> 4 != 4 || size < 20 || total < size || fragment & 0x8000 != 0 {
            return Err(malformed(
                Layer::Ipv4,
                offset,
                "invalid version, length or reserved flag",
            ));
        }
        let end = offset + total;
        self.header(b, offset, size, end, Layer::Ipv4)?;
        let ambiguous = options(b, offset + 20..offset + size, Layer::Ipv4)?;
        p.ip = Some(Ip::V4(Ipv4 {
            source: Ipv4Addr::from(<[u8; 4]>::try_from(&b[offset + 12..offset + 16]).unwrap()),
            destination: Ipv4Addr::from(<[u8; 4]>::try_from(&b[offset + 16..offset + 20]).unwrap()),
            protocol: b[offset + 9],
            dscp_ecn: b[offset + 1],
            ttl: b[offset + 8],
            identification: u16_at(b, offset + 4),
            dont_fragment: fragment & 0x4000 != 0,
            more_fragments: fragment & 0x2000 != 0,
            fragment_offset: usize::from(fragment & 0x1fff) * 8,
            total_length: total,
            header: offset..offset + size,
            checksum: checksum(&b[offset..offset + size], 0),
        }));
        p.capture_truncated |= end > b.len();
        p.payload = offset + size..end.min(b.len());
        if fragment & 0x3fff != 0 {
            if fragment & 0x2000 != 0 && !(total - size).is_multiple_of(8) {
                return Err(malformed(
                    Layer::Ipv4,
                    offset + 6,
                    "non-final fragment length is not a multiple of eight",
                ));
            }
            return Err(Diagnostic::Fragmented { layer: Layer::Ipv4 });
        }
        self.transport(p, offset + size, end, b[offset + 9], !ambiguous)
    }
    fn ipv6(&self, p: &mut Packet<'_>, offset: usize) -> Result<(), Diagnostic> {
        let b = p.raw;
        self.header(b, offset, 40, usize::MAX, Layer::Ipv6)?;
        if b[offset] >> 4 != 6 {
            return Err(malformed(Layer::Ipv6, offset, "invalid version"));
        }
        let length = usize::from(u16_at(b, offset + 4));
        let mut next = b[offset + 6];
        let end = offset + 40 + length;
        p.ip = Some(Ip::V6(Ipv6 {
            source: Ipv6Addr::from(<[u8; 16]>::try_from(&b[offset + 8..offset + 24]).unwrap()),
            destination: Ipv6Addr::from(
                <[u8; 16]>::try_from(&b[offset + 24..offset + 40]).unwrap(),
            ),
            traffic_class: ((b[offset] & 15) << 4) | (b[offset + 1] >> 4),
            flow_label: u32_at(b, offset) & 0xfffff,
            hop_limit: b[offset + 7],
            next_header: next,
            payload_length: length,
            header: offset..offset + 40,
        }));
        p.capture_truncated |= end > b.len();
        let mut cursor = offset + 40;
        p.payload = cursor..end.min(b.len());
        if length == 0 && next == 0 {
            return Err(Diagnostic::JumbogramUnsupported);
        }
        let mut ambiguous = false;
        let mut fragment_seen = false;
        while matches!(next, 0 | 43 | 44 | 51 | 60) {
            if p.extensions.len() == self.limits.ipv6_extensions {
                return Err(Diagnostic::Limited {
                    limit: Limit::Extensions,
                    offset: cursor,
                });
            }
            self.header(b, cursor, 2, end, Layer::Extension)?;
            let size = match next {
                44 => 8,
                51 => (usize::from(b[cursor + 1]) + 2) * 4,
                _ => (usize::from(b[cursor + 1]) + 1) * 8,
            };
            self.header(b, cursor, size, end, Layer::Extension)?;
            let mut fragment = None;
            match next {
                0 | 60 => {
                    if next == 0 && cursor != offset + 40 {
                        return Err(malformed(
                            Layer::Extension,
                            cursor,
                            "hop-by-hop header is not first",
                        ));
                    }
                    ambiguous |= ipv6_options(b, cursor + 2..cursor + size)?;
                }
                43 => ambiguous |= b[cursor + 3] != 0,
                51 if size < 12 || !size.is_multiple_of(8) => {
                    return Err(malformed(
                        Layer::Extension,
                        cursor,
                        "invalid IPv6 AH length or alignment",
                    ));
                }
                44 => {
                    let bits = u16_at(b, cursor + 2);
                    if fragment_seen || b[cursor + 1] != 0 || bits & 6 != 0 {
                        return Err(malformed(
                            Layer::Extension,
                            cursor,
                            "duplicate fragment or reserved bits",
                        ));
                    }
                    fragment_seen = true;
                    fragment = Some(Fragment {
                        offset: usize::from(bits & 0xfff8),
                        more: bits & 1 != 0,
                        identification: u32_at(b, cursor + 4),
                    });
                }
                _ => {}
            }
            p.extensions.push(Extension {
                kind: next,
                next_header: b[cursor],
                range: cursor..cursor + size,
                fragment,
            });
            next = b[cursor];
            cursor += size;
            p.payload = cursor..end.min(b.len());
            if let Some(fragment) = fragment
                && (fragment.more || fragment.offset != 0)
            {
                if fragment.more && !(end - cursor).is_multiple_of(8) {
                    return Err(malformed(
                        Layer::Extension,
                        cursor - size,
                        "non-final fragment length is not a multiple of eight",
                    ));
                }
                return Err(Diagnostic::Fragmented { layer: Layer::Ipv6 });
            }
        }
        if next == 50 {
            return Err(Diagnostic::Encrypted);
        }
        if next == 59 {
            return complete(p, cursor, end, Layer::Ipv6);
        }
        self.transport(p, cursor, end, next, !ambiguous)
    }
    fn transport(
        &self,
        p: &mut Packet<'_>,
        offset: usize,
        end: usize,
        protocol: u8,
        checksum_known: bool,
    ) -> Result<(), Diagnostic> {
        let b = p.raw;
        match protocol {
            6 => {
                self.header(b, offset, 20, end, Layer::Tcp)?;
                let size = usize::from(b[offset + 12] >> 4) * 4;
                if size < 20 {
                    return Err(malformed(
                        Layer::Tcp,
                        offset + 12,
                        "data offset shorter than fixed header",
                    ));
                }
                self.header(b, offset, size, end, Layer::Tcp)?;
                options(b, offset + 20..offset + size, Layer::Tcp)?;
                p.transport = Some(Transport::Tcp(Tcp {
                    source_port: u16_at(b, offset),
                    destination_port: u16_at(b, offset + 2),
                    sequence: u32_at(b, offset + 4),
                    acknowledgement: u32_at(b, offset + 8),
                    flags: u16_at(b, offset + 12) & 0x0fff,
                    window: u16_at(b, offset + 14),
                    urgent_pointer: u16_at(b, offset + 18),
                    header: offset..offset + size,
                    options: offset + 20..offset + size,
                    checksum: transport_checksum(p, offset, end, protocol, checksum_known),
                }));
                p.payload = offset + size..end.min(b.len());
                complete(p, offset, end, Layer::Tcp)
            }
            17 => {
                self.header(b, offset, 8, end, Layer::Udp)?;
                let size = usize::from(u16_at(b, offset + 4));
                if size < 8 || size > end - offset {
                    return Err(malformed(
                        Layer::Udp,
                        offset + 4,
                        "UDP length outside IP payload",
                    ));
                }
                let checksum = if u16_at(b, offset + 6) == 0 {
                    if matches!(p.ip, Some(Ip::V4(_))) {
                        Checksum::NotPresent
                    } else {
                        Checksum::Invalid
                    }
                } else {
                    transport_checksum(p, offset, offset + size, protocol, checksum_known)
                };
                p.transport = Some(Transport::Udp(Udp {
                    source_port: u16_at(b, offset),
                    destination_port: u16_at(b, offset + 2),
                    length: size,
                    header: offset..offset + 8,
                    checksum,
                }));
                p.payload = offset + 8..(offset + size).min(b.len());
                complete(p, offset, end, Layer::Udp)
            }
            other => Err(Diagnostic::Unsupported {
                layer: if matches!(p.ip, Some(Ip::V4(_))) {
                    Layer::Ipv4
                } else {
                    Layer::Ipv6
                },
                protocol: other.into(),
            }),
        }
    }
}
fn complete(p: &Packet<'_>, offset: usize, end: usize, layer: Layer) -> Result<(), Diagnostic> {
    if end > p.raw.len() {
        Err(Diagnostic::Truncated {
            layer,
            offset,
            needed: end - offset,
            available: p.raw.len().saturating_sub(offset),
        })
    } else {
        Ok(())
    }
}
fn malformed(layer: Layer, offset: usize, reason: &'static str) -> Diagnostic {
    Diagnostic::Malformed {
        layer,
        offset,
        reason,
    }
}
fn u16_at(b: &[u8], i: usize) -> u16 {
    u16::from_be_bytes([b[i], b[i + 1]])
}
fn u32_at(b: &[u8], i: usize) -> u32 {
    u32::from_be_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}
// Options are scanned only within a previously validated, budgeted header.
fn options(b: &[u8], range: Range<usize>, layer: Layer) -> Result<bool, Diagnostic> {
    let mut cursor = range.start;
    let mut ambiguous = false;
    while cursor < range.end {
        let kind = b[cursor];
        if kind == 0 {
            break;
        }
        if kind == 1 {
            cursor += 1;
            continue;
        }
        if cursor + 2 > range.end {
            return Err(malformed(layer, cursor, "missing option length"));
        }
        let length = usize::from(b[cursor + 1]);
        if length < 2 || length > range.end - cursor {
            return Err(malformed(layer, cursor, "invalid option length"));
        }
        if layer == Layer::Tcp
            && match kind {
                2 => length != 4,
                3 => length != 3,
                4 => length != 2,
                5 => length < 10 || !(length - 2).is_multiple_of(8),
                8 => length != 10,
                _ => false,
            }
        {
            return Err(malformed(
                layer,
                cursor,
                "invalid standard TCP option length",
            ));
        }
        ambiguous |= layer == Layer::Ipv4 && matches!(kind, 131 | 137);
        cursor += length;
    }
    Ok(ambiguous)
}
fn ipv6_options(b: &[u8], range: Range<usize>) -> Result<bool, Diagnostic> {
    let mut cursor = range.start;
    let mut ambiguous = false;
    while cursor < range.end {
        if b[cursor] == 0 {
            cursor += 1;
            continue;
        }
        if cursor + 2 > range.end {
            return Err(malformed(
                Layer::Extension,
                cursor,
                "missing IPv6 option length",
            ));
        }
        let length = usize::from(b[cursor + 1]) + 2;
        if length > range.end - cursor {
            return Err(malformed(
                Layer::Extension,
                cursor,
                "IPv6 option exceeds header",
            ));
        }
        if b[cursor] == 0xc2 {
            return Err(Diagnostic::JumbogramUnsupported);
        }
        // Mobile IPv6 Home Address changes the pseudoheader source (RFC 6275).
        ambiguous |= b[cursor] == 0xc9;
        cursor += length;
    }
    Ok(ambiguous)
}
fn sum(b: &[u8]) -> u32 {
    let (chunks, remainder) = b.as_chunks::<2>();
    let mut total: u32 = chunks
        .iter()
        .map(|pair| u32::from(u16::from_be_bytes([pair[0], pair[1]])))
        .sum();
    if let [last] = remainder {
        total += u32::from(*last) << 8;
    }
    total
}
fn checksum(b: &[u8], seed: u32) -> Checksum {
    let mut total = seed + sum(b);
    while total >> 16 != 0 {
        total = (total & 0xffff) + (total >> 16);
    }
    if total == 0xffff {
        Checksum::Valid
    } else {
        Checksum::Invalid
    }
}
fn transport_checksum(
    p: &Packet<'_>,
    start: usize,
    end: usize,
    protocol: u8,
    known: bool,
) -> Checksum {
    if !known || end > p.raw.len() {
        return Checksum::Unavailable;
    }
    let seed = match p.ip.as_ref().unwrap() {
        Ip::V4(ip) => {
            sum(&ip.source.octets())
                + sum(&ip.destination.octets())
                + u32::from(protocol)
                + (end - start) as u32
        }
        Ip::V6(ip) => {
            sum(&ip.source.octets())
                + sum(&ip.destination.octets())
                + u32::from(protocol)
                + (end - start) as u32
        }
    };
    checksum(&p.raw[start..end], seed)
}
