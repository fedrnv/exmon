//! Bounded streaming capture files. Raw records are inspection data, never TX authority.
use exmon_domain::{
    Direction,
    observation::{MAX_PAYLOAD, Metadata, Observation},
};
use std::{
    collections::BTreeMap,
    fmt,
    io::{self, Read, Write},
    ops::Range,
};

const SHB: u32 = 0x0a0d0d0a;
pub const MAX_BLOCK: usize = 1024 * 1024;
const MAX_OPTIONS: usize = 1024;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Endian {
    Little,
    Big,
}
impl Endian {
    fn u16(self, b: &[u8]) -> u16 {
        let a = [b[0], b[1]];
        match self {
            Self::Little => u16::from_le_bytes(a),
            Self::Big => u16::from_be_bytes(a),
        }
    }
    fn u32(self, b: &[u8]) -> u32 {
        let a = [b[0], b[1], b[2], b[3]];
        match self {
            Self::Little => u32::from_le_bytes(a),
            Self::Big => u32::from_be_bytes(a),
        }
    }
    fn i64(self, b: &[u8]) -> i64 {
        let a: [u8; 8] = b[..8].try_into().expect("validated fixed field");
        match self {
            Self::Little => i64::from_le_bytes(a),
            Self::Big => i64::from_be_bytes(a),
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Code {
    Io,
    Truncated,
    Magic,
    Version,
    Length,
    Trailer,
    Options,
    Interface,
    Section,
    Limit,
    Timestamp,
    Record,
    Unsupported,
    State,
}
pub struct Error {
    pub offset: u64,
    pub code: Code,
    raw: Box<[u8]>,
}
impl Error {
    pub fn raw(&self) -> &[u8] {
        &self.raw
    }
}
impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CaptureFileError")
            .field("offset", &self.offset)
            .field("code", &self.code)
            .finish()
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "capture file {:?} at byte {}", self.code, self.offset)
    }
}
impl std::error::Error for Error {}
fn error(offset: u64, code: Code) -> Error {
    Error {
        offset,
        code,
        raw: Box::new([]),
    }
}
type Fault = (usize, Code);
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub block_bytes: usize,
    pub interfaces: usize,
    pub total_bytes: u64,
    pub entries: u64,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            block_bytes: MAX_BLOCK,
            interfaces: 64,
            total_bytes: 1 << 40,
            entries: 100_000_000,
        }
    }
}
impl Limits {
    fn validate(self) -> Result<Self, Error> {
        if !(28..=MAX_BLOCK).contains(&self.block_bytes)
            || !(1..=256).contains(&self.interfaces)
            || self.total_bytes < 24
            || self.total_bytes > u64::MAX - MAX_BLOCK as u64
            || self.entries == 0
            || self.entries > 1_000_000_000
        {
            Err(error(0, Code::Limit))
        } else {
            Ok(self)
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Clock {
    pub resolution: u8,
    pub offset_seconds: i64,
}
impl Default for Clock {
    fn default() -> Self {
        Self {
            resolution: 6,
            offset_seconds: 0,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Timestamp {
    Pcap {
        seconds: u32,
        fraction: u32,
        nanos: bool,
    },
    Ng {
        ticks: u64,
        clock: Clock,
    },
}
impl Timestamp {
    /// Exact conversion only. Native ticks always remain available, including sub-ns times.
    pub fn nanoseconds(self) -> Option<u64> {
        match self {
            Self::Pcap {
                seconds,
                fraction,
                nanos,
            } => {
                let scale = if nanos { 1_000_000_000 } else { 1_000_000 };
                if fraction >= scale {
                    return None;
                }
                Some(
                    u64::from(seconds) * 1_000_000_000
                        + u64::from(fraction) * if nanos { 1 } else { 1000 },
                )
            }
            Self::Ng { ticks, clock } => {
                if ticks == 0 {
                    return u64::try_from(i128::from(clock.offset_seconds) * 1_000_000_000).ok();
                }
                let exponent = u32::from(clock.resolution & 0x7f);
                let denominator = if clock.resolution & 0x80 == 0 {
                    10u128.checked_pow(exponent)?
                } else {
                    1u128.checked_shl(exponent)?
                };
                let numerator = u128::from(ticks) * 1_000_000_000;
                if numerator % denominator != 0 {
                    return None;
                }
                let ns = i128::try_from(numerator / denominator).ok()?
                    + i128::from(clock.offset_seconds) * 1_000_000_000;
                u64::try_from(ns).ok()
            }
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    pub endian: Endian,
    pub nanos: bool,
    pub snaplen: u32,
    pub link_info: u32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Interface {
    pub id: u32,
    pub link_type: u16,
    pub snaplen: u32,
    pub clock: Clock,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Issue {
    CapturedExceedsOriginal,
    CapturedExceedsSnapshot,
    PayloadLimit,
    InvalidTimestamp,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Packet {
    pub interface: u32,
    pub link_type: u32,
    pub timestamp: Option<Timestamp>,
    pub original_length: u32,
    pub direction: Direction,
    pub data: Range<usize>,
    pub issues: Vec<Issue>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Content {
    Header(Header),
    Section { endian: Endian, length: i64 },
    Interface(Interface),
    Packet(Packet),
    Opaque { block_type: u32 },
}
pub struct Entry {
    offset: u64,
    section: u64,
    content: Content,
    raw: Box<[u8]>,
}
impl fmt::Debug for Entry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CaptureFileEntry")
            .field("offset", &self.offset)
            .field("section", &self.section)
            .field(
                "kind",
                &match self.content {
                    Content::Header(_) => "header",
                    Content::Section { .. } => "section",
                    Content::Interface(_) => "interface",
                    Content::Packet(_) => "packet",
                    Content::Opaque { .. } => "opaque",
                },
            )
            .finish()
    }
}
impl Entry {
    pub fn offset(&self) -> u64 {
        self.offset
    }
    pub fn section(&self) -> u64 {
        self.section
    }
    pub fn content(&self) -> &Content {
        &self.content
    }
    pub fn raw(&self) -> &[u8] {
        &self.raw
    }
    pub fn payload(&self) -> Option<&[u8]> {
        if let Content::Packet(p) = &self.content {
            Some(&self.raw[p.data.clone()])
        } else {
            None
        }
    }
    pub fn observation(&self, monotonic_ns: u64) -> Result<Observation, Error> {
        let Content::Packet(p) = &self.content else {
            return Err(error(self.offset, Code::Unsupported));
        };
        if !p.issues.is_empty() {
            return Err(error(self.offset, Code::Record));
        }
        let time = p
            .timestamp
            .and_then(Timestamp::nanoseconds)
            .ok_or_else(|| error(self.offset, Code::Timestamp))?;
        Observation::new(
            monotonic_ns,
            p.direction,
            Metadata::Ethernet {
                link_type: p.link_type,
            },
            Some(p.original_length as usize),
            &self.raw[p.data.clone()],
        )
        .map(|o| o.with_source_timestamp(time))
        .map_err(|_| error(self.offset, Code::Record))
    }
}
#[derive(Clone, Copy)]
enum Format {
    Pcap(Header),
    Ng(Endian),
}
pub struct Reader<R> {
    input: R,
    limits: Limits,
    format: Option<Format>,
    offset: u64,
    entries: u64,
    section: u64,
    section_end: Option<u64>,
    interfaces: Vec<Interface>,
    raw: Vec<u8>,
    ended: bool,
    failed: bool,
}
impl<R: Read> Reader<R> {
    pub fn new(input: R, limits: Limits) -> Result<Self, Error> {
        Ok(Self {
            input,
            limits: limits.validate()?,
            format: None,
            offset: 0,
            entries: 0,
            section: 0,
            section_end: None,
            interfaces: vec![],
            raw: vec![],
            ended: false,
            failed: false,
        })
    }
    pub fn position(&self) -> u64 {
        self.offset
    }
    pub fn interface_count(&self) -> usize {
        self.interfaces.len()
    }
    fn read_more(&mut self, count: usize, eof: bool) -> Result<bool, Error> {
        let old = self.raw.len();
        let length = old
            .checked_add(count)
            .filter(|n| *n <= self.limits.block_bytes)
            .ok_or_else(|| error(self.offset, Code::Limit))?;
        self.raw.resize(length, 0);
        let mut filled = old;
        let mut interruptions = 0;
        while filled < length {
            // One extra byte is permitted only to distinguish exact-limit EOF from more input.
            let remaining = self
                .limits
                .total_bytes
                .saturating_sub(self.offset)
                .max(1)
                .min((length - filled) as u64) as usize;
            match self.input.read(&mut self.raw[filled..filled + remaining]) {
                Ok(0) => {
                    self.raw.truncate(filled);
                    return if eof && filled == 0 {
                        Ok(false)
                    } else {
                        Err(error(self.offset, Code::Truncated))
                    };
                }
                Ok(n) => {
                    filled += n;
                    self.offset += n as u64;
                    if self.offset > self.limits.total_bytes {
                        self.raw.truncate(filled);
                        return Err(error(self.offset - 1, Code::Limit));
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted && interruptions < 8 => {
                    interruptions += 1
                }
                Err(_) => {
                    self.raw.truncate(filled);
                    return Err(error(self.offset, Code::Io));
                }
            }
        }
        Ok(true)
    }
    fn fault(&self, start: u64, fault: Fault) -> Error {
        error(start + fault.0 as u64, fault.1)
    }
    pub fn next_entry(&mut self) -> Result<Option<Entry>, Error> {
        if self.failed {
            return Err(error(self.offset, Code::State));
        }
        if self.ended {
            return Ok(None);
        }
        let start = self.offset;
        match self.next_content(start) {
            Ok(Some(content)) if self.entries < self.limits.entries => {
                self.entries += 1;
                Ok(Some(Entry {
                    offset: start,
                    section: self.section,
                    content,
                    raw: std::mem::take(&mut self.raw).into_boxed_slice(),
                }))
            }
            Ok(None) => {
                self.ended = true;
                Ok(None)
            }
            result => {
                self.failed = true;
                let mut e = match result {
                    Err(e) => e,
                    _ => error(start, Code::Limit),
                };
                e.raw = std::mem::take(&mut self.raw).into_boxed_slice();
                Err(e)
            }
        }
    }
    fn next_content(&mut self, start: u64) -> Result<Option<Content>, Error> {
        match self.format {
            None => {
                self.read_more(4, false)?;
                if self.raw == SHB.to_le_bytes() {
                    return self.ng(start, true).map(Some);
                }
                let (endian, nanos) = match self.raw.as_slice() {
                    [0xd4, 0xc3, 0xb2, 0xa1] => (Endian::Little, false),
                    [0xa1, 0xb2, 0xc3, 0xd4] => (Endian::Big, false),
                    [0x4d, 0x3c, 0xb2, 0xa1] => (Endian::Little, true),
                    [0xa1, 0xb2, 0x3c, 0x4d] => (Endian::Big, true),
                    _ => return Err(error(start, Code::Magic)),
                };
                self.read_more(20, false)?;
                if endian.u16(&self.raw[4..]) != 2 || endian.u16(&self.raw[6..]) != 4 {
                    return Err(error(start + 4, Code::Version));
                }
                let header = Header {
                    endian,
                    nanos,
                    snaplen: endian.u32(&self.raw[16..]),
                    link_info: endian.u32(&self.raw[20..]),
                };
                if header.snaplen == 0 {
                    return Err(error(start + 16, Code::Length));
                }
                self.format = Some(Format::Pcap(header));
                Ok(Some(Content::Header(header)))
            }
            Some(Format::Pcap(header)) => {
                if !self.read_more(16, true)? {
                    return Ok(None);
                }
                let e = header.endian;
                let cap = e.u32(&self.raw[8..]) as usize;
                if cap > MAX_PAYLOAD {
                    return Err(error(start + 8, Code::Limit));
                }
                self.read_more(cap, false)?;
                let time = Timestamp::Pcap {
                    seconds: e.u32(&self.raw),
                    fraction: e.u32(&self.raw[4..]),
                    nanos: header.nanos,
                };
                let packet = packet(
                    0,
                    header.link_info & 0xffff,
                    Some(time),
                    e.u32(&self.raw[12..]),
                    Direction::Unknown,
                    16..16 + cap,
                    header.snaplen,
                );
                Ok(Some(Content::Packet(packet)))
            }
            Some(Format::Ng(_)) => {
                if !self.read_more(8, true)? {
                    if self.section_end.is_some_and(|end| end != self.offset) {
                        return Err(error(self.offset, Code::Section));
                    }
                    return Ok(None);
                }
                self.ng(start, false).map(Some)
            }
        }
    }
    fn ng(&mut self, start: u64, first: bool) -> Result<Content, Error> {
        let section = self.raw[..4] == SHB.to_le_bytes();
        if first {
            self.read_more(4, false)?;
        }
        let e = if section {
            self.read_more(4, false)?;
            match &self.raw[8..12] {
                [0x4d, 0x3c, 0x2b, 0x1a] => Endian::Little,
                [0x1a, 0x2b, 0x3c, 0x4d] => Endian::Big,
                _ => return Err(error(start + 8, Code::Magic)),
            }
        } else {
            match self.format {
                Some(Format::Ng(e)) => e,
                _ => return Err(error(start, Code::Magic)),
            }
        };
        let len = e.u32(&self.raw[4..]) as usize;
        if len < if section { 28 } else { 12 } || !len.is_multiple_of(4) {
            return Err(error(start + 4, Code::Length));
        }
        if len > self.limits.block_bytes {
            return Err(error(start + 4, Code::Limit));
        }
        if let Some(end) = self.section_end
            && ((section && start != end) || (!section && start + len as u64 > end))
        {
            return Err(error(start, Code::Section));
        }
        self.read_more(len - self.raw.len(), false)?;
        if e.u32(&self.raw[len - 4..]) != len as u32 {
            return Err(error(start + len as u64 - 4, Code::Trailer));
        }
        if section {
            if e.u16(&self.raw[12..]) != 1 || ![0, 2].contains(&e.u16(&self.raw[14..])) {
                return Err(error(start + 12, Code::Version));
            }
            let length = e.i64(&self.raw[16..]);
            if length < -1 || (length >= 0 && length % 4 != 0) {
                return Err(error(start + 16, Code::Section));
            }
            options(&self.raw, 24..len - 4, e).map_err(|f| self.fault(start, f))?;
            self.section_end = if length == -1 {
                None
            } else {
                Some(
                    self.offset
                        .checked_add(length as u64)
                        .filter(|v| *v <= self.limits.total_bytes)
                        .ok_or_else(|| error(start + 16, Code::Limit))?,
                )
            };
            self.section = self
                .section
                .checked_add(1)
                .ok_or_else(|| error(start, Code::Limit))?;
            self.interfaces.clear();
            self.format = Some(Format::Ng(e));
            return Ok(Content::Section { endian: e, length });
        }
        let kind = e.u32(&self.raw);
        let content = match kind {
            1 => {
                if len < 20 {
                    return Err(error(start + 4, Code::Length));
                }
                if self.interfaces.len() == self.limits.interfaces {
                    return Err(error(start, Code::Limit));
                }
                let mut interface = Interface {
                    id: self.interfaces.len() as u32,
                    link_type: e.u16(&self.raw[8..]),
                    snaplen: e.u32(&self.raw[12..]),
                    clock: Clock::default(),
                };
                let mut seen = 0;
                for (code, span) in
                    options(&self.raw, 16..len - 4, e).map_err(|f| self.fault(start, f))?
                {
                    if code == 9 || code == 14 {
                        let bit = if code == 9 { 1 } else { 2 };
                        if seen & bit != 0 || span.len() != if code == 9 { 1 } else { 8 } {
                            return Err(error(start + span.start as u64, Code::Options));
                        }
                        seen |= bit;
                        if code == 9 {
                            interface.clock.resolution = self.raw[span.start];
                        } else {
                            interface.clock.offset_seconds = e.i64(&self.raw[span]);
                        }
                    }
                }
                self.interfaces.push(interface);
                Content::Interface(interface)
            }
            6 | 2 => {
                if len < 32 {
                    return Err(error(start + 4, Code::Length));
                }
                let id = if kind == 2 {
                    u32::from(e.u16(&self.raw[8..]))
                } else {
                    e.u32(&self.raw[8..])
                };
                let interface = *self
                    .interfaces
                    .get(id as usize)
                    .ok_or_else(|| error(start + 8, Code::Interface))?;
                let cap = e.u32(&self.raw[20..]) as usize;
                let end = 28usize
                    .checked_add(cap)
                    .filter(|end| *end <= len - 4)
                    .ok_or_else(|| error(start + 20, Code::Length))?;
                let opt = aligned(end)
                    .filter(|end| *end <= len - 4)
                    .ok_or_else(|| error(start + 20, Code::Length))?;
                let mut direction = Direction::Unknown;
                let mut flags_seen = false;
                for (code, span) in
                    options(&self.raw, opt..len - 4, e).map_err(|f| self.fault(start, f))?
                {
                    if code == 2 {
                        if flags_seen || span.len() != 4 {
                            return Err(error(start + span.start as u64, Code::Options));
                        }
                        flags_seen = true;
                        direction = match e.u32(&self.raw[span]) & 3 {
                            1 => Direction::Rx,
                            2 => Direction::Tx,
                            _ => Direction::Unknown,
                        };
                    }
                }
                let ticks =
                    (u64::from(e.u32(&self.raw[12..])) << 32) | u64::from(e.u32(&self.raw[16..]));
                Content::Packet(packet(
                    id,
                    u32::from(interface.link_type),
                    Some(Timestamp::Ng {
                        ticks,
                        clock: interface.clock,
                    }),
                    e.u32(&self.raw[24..]),
                    direction,
                    28..end,
                    interface.snaplen,
                ))
            }
            3 => {
                if len < 16 {
                    return Err(error(start + 4, Code::Length));
                }
                let interface = *self
                    .interfaces
                    .first()
                    .ok_or_else(|| error(start, Code::Interface))?;
                let original = e.u32(&self.raw[8..]);
                let cap = if interface.snaplen == 0 {
                    original
                } else {
                    original.min(interface.snaplen)
                } as usize;
                if aligned(cap) != Some(len - 16) {
                    return Err(error(start + 8, Code::Length));
                }
                Content::Packet(packet(
                    0,
                    u32::from(interface.link_type),
                    None,
                    original,
                    Direction::Unknown,
                    12..12 + cap,
                    interface.snaplen,
                ))
            }
            _ => Content::Opaque { block_type: kind },
        };
        Ok(content)
    }
}
fn aligned(n: usize) -> Option<usize> {
    n.checked_add(3).map(|v| v & !3)
}
fn options(raw: &[u8], span: Range<usize>, e: Endian) -> Result<Vec<(u16, Range<usize>)>, Fault> {
    let mut cursor = span.start;
    let mut out = vec![];
    while cursor < span.end {
        if span.end - cursor < 4 {
            return Err((cursor, Code::Options));
        }
        let code = e.u16(&raw[cursor..]);
        let len = e.u16(&raw[cursor + 2..]) as usize;
        let begin = cursor + 4;
        let end = begin
            .checked_add(len)
            .filter(|v| *v <= span.end)
            .ok_or((cursor + 2, Code::Options))?;
        if code == 0 {
            return if len == 0 && raw[begin..span.end].iter().all(|b| *b == 0) {
                Ok(out)
            } else {
                Err((cursor, Code::Options))
            };
        }
        if out.len() == MAX_OPTIONS {
            return Err((cursor, Code::Limit));
        }
        out.push((code, begin..end));
        cursor = aligned(end)
            .filter(|v| *v <= span.end)
            .ok_or((cursor, Code::Options))?;
    }
    Ok(out)
}
fn packet(
    interface: u32,
    link_type: u32,
    timestamp: Option<Timestamp>,
    original_length: u32,
    direction: Direction,
    data: Range<usize>,
    snaplen: u32,
) -> Packet {
    let mut issues = vec![];
    if data.len() > original_length as usize {
        issues.push(Issue::CapturedExceedsOriginal);
    }
    if snaplen != 0 && data.len() > snaplen as usize {
        issues.push(Issue::CapturedExceedsSnapshot);
    }
    if data.len() > MAX_PAYLOAD {
        issues.push(Issue::PayloadLimit);
    }
    if matches!(timestamp, Some(Timestamp::Pcap { .. }))
        && timestamp.and_then(Timestamp::nanoseconds).is_none()
    {
        issues.push(Issue::InvalidTimestamp);
    }
    Packet {
        interface,
        link_type,
        timestamp,
        original_length,
        direction,
        data,
        issues,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    Pcap { link_type: u16 },
    PcapNg,
}
pub struct Encoder<W> {
    output: W,
    position: u64,
    failed: bool,
    encoding: Encoding,
    interfaces: BTreeMap<u32, (u32, u16)>,
}
impl<W: Write> Encoder<W> {
    pub fn new(output: W, encoding: Encoding) -> Result<Self, Error> {
        let mut writer = Self {
            output,
            position: 0,
            failed: false,
            encoding,
            interfaces: BTreeMap::new(),
        };
        let bytes = match encoding {
            Encoding::Pcap { link_type } => {
                let mut b = vec![];
                for n in [
                    0xa1b23c4d,
                    0x00040002,
                    0,
                    0,
                    MAX_PAYLOAD as u32,
                    u32::from(link_type),
                ] {
                    le32(&mut b, n);
                }
                b
            }
            Encoding::PcapNg => block(SHB, &[0x1a2b3c4d, 1, u32::MAX, u32::MAX]),
        };
        writer.write(&bytes)?;
        Ok(writer)
    }
    fn write(&mut self, bytes: &[u8]) -> Result<(), Error> {
        if self.failed {
            return Err(error(self.position, Code::State));
        }
        if self.position.checked_add(bytes.len() as u64).is_none() {
            self.failed = true;
            return Err(error(self.position, Code::Limit));
        }
        let mut done = 0;
        let mut interruptions = 0;
        while done < bytes.len() {
            match self.output.write(&bytes[done..]) {
                Ok(0) => {
                    self.failed = true;
                    return Err(error(self.position, Code::Io));
                }
                Ok(n) => {
                    done += n;
                    self.position += n as u64;
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted && interruptions < 8 => {
                    interruptions += 1
                }
                Err(_) => {
                    self.failed = true;
                    return Err(error(self.position, Code::Io));
                }
            }
        }
        Ok(())
    }
    /// Caller supplies explicit source identity; no device address or binding is exported.
    pub fn write_observation(
        &mut self,
        source: u32,
        observation: &Observation,
    ) -> Result<(), Error> {
        if self.failed {
            return Err(error(self.position, Code::State));
        }
        let Metadata::Ethernet { link_type } = observation.metadata() else {
            return Err(error(self.position, Code::Unsupported));
        };
        let link =
            u16::try_from(*link_type).map_err(|_| error(self.position, Code::Unsupported))?;
        let original = observation
            .original_length()
            .and_then(|v| u32::try_from(v).ok())
            .ok_or_else(|| error(self.position, Code::Record))?;
        let time = observation
            .source_timestamp_ns()
            .ok_or_else(|| error(self.position, Code::Timestamp))?;
        let cap = observation.bytes().len() as u32;
        let mut bytes = vec![];
        match self.encoding {
            Encoding::Pcap { link_type } => {
                if link != link_type
                    || observation.direction() != Direction::Unknown
                    || self.interfaces.keys().any(|key| *key != source)
                {
                    return Err(error(self.position, Code::Unsupported));
                }
                let seconds = u32::try_from(time / 1_000_000_000)
                    .map_err(|_| error(self.position, Code::Timestamp))?;
                for n in [seconds, (time % 1_000_000_000) as u32, cap, original] {
                    le32(&mut bytes, n);
                }
                bytes.extend_from_slice(observation.bytes());
                self.interfaces.insert(source, (0, link));
            }
            Encoding::PcapNg => {
                let id = if let Some(&(id, old_link)) = self.interfaces.get(&source) {
                    if old_link != link {
                        return Err(error(self.position, Code::Interface));
                    }
                    id
                } else {
                    if self.interfaces.len() == 256 {
                        return Err(error(self.position, Code::Limit));
                    }
                    let id = self.interfaces.len() as u32;
                    let idb = block(1, &[u32::from(link), MAX_PAYLOAD as u32, 0x00010009, 9, 0]);
                    // The first option value is one byte (9) followed by three zero padding bytes.
                    self.write(&idb)?;
                    self.interfaces.insert(source, (id, link));
                    id
                };
                for n in [6, 0, id, (time >> 32) as u32, time as u32, cap, original] {
                    le32(&mut bytes, n);
                }
                bytes.extend_from_slice(observation.bytes());
                bytes.resize(aligned(bytes.len()).expect("bounded payload"), 0);
                for n in [
                    0x00040002,
                    match observation.direction() {
                        Direction::Rx => 1,
                        Direction::Tx => 2,
                        _ => 0,
                    },
                    0,
                ] {
                    le32(&mut bytes, n);
                }
                let len = bytes.len() as u32 + 4;
                bytes[4..8].copy_from_slice(&len.to_le_bytes());
                le32(&mut bytes, len);
            }
        }
        self.write(&bytes)
    }
    pub fn finish(mut self) -> Result<W, Error> {
        if self.failed {
            return Err(error(self.position, Code::State));
        }
        self.output
            .flush()
            .map_err(|_| error(self.position, Code::Io))?;
        Ok(self.output)
    }
}
fn le32(bytes: &mut Vec<u8>, n: u32) {
    bytes.extend_from_slice(&n.to_le_bytes());
}
fn block(kind: u32, body: &[u32]) -> Vec<u8> {
    let len = (body.len() * 4 + 12) as u32;
    let mut b = vec![];
    for n in [kind, len]
        .into_iter()
        .chain(body.iter().copied())
        .chain([len])
    {
        le32(&mut b, n);
    }
    b
}
/// Complete copies are byte-exact, including opaque metadata and malformed packet semantics.
pub fn copy_all<R: Read, W: Write>(reader: &mut Reader<R>, output: W) -> Result<W, Error> {
    copy(reader, output, None::<fn(&Entry) -> bool>)
}
/// Retains metadata except explicitly non-copyable custom data; caller authorizes metadata too.
pub fn copy_selected<R: Read, W: Write, F: FnMut(&Entry) -> bool>(
    reader: &mut Reader<R>,
    output: W,
    select: F,
) -> Result<W, Error> {
    copy(reader, output, Some(select))
}
fn copy<R: Read, W: Write, F: FnMut(&Entry) -> bool>(
    reader: &mut Reader<R>,
    output: W,
    mut select: Option<F>,
) -> Result<W, Error> {
    if reader.position() != 0 {
        return Err(error(reader.position(), Code::State));
    }
    // Reuse the terminal partial-write contract without writing a new file header.
    let mut writer = Encoder {
        output,
        position: 0,
        failed: false,
        encoding: Encoding::PcapNg,
        interfaces: BTreeMap::new(),
    };
    let mut endian = None;
    while let Some(entry) = reader.next_entry()? {
        if matches!(entry.content, Content::Packet(_))
            && select.as_mut().is_some_and(|f| !f(&entry))
        {
            continue;
        }
        if select.is_some() {
            if let Content::Section { endian: order, .. } = entry.content {
                endian = Some(order);
            }
            if let Some(raw) = selected_raw(&entry, endian)? {
                writer.write(&raw)?;
            }
        } else {
            writer.write(&entry.raw)?;
        }
    }
    writer.finish()
}
fn selected_raw(entry: &Entry, endian: Option<Endian>) -> Result<Option<Vec<u8>>, Error> {
    if matches!(entry.content, Content::Header(_)) {
        return Ok(Some(entry.raw.to_vec()));
    }
    let e = endian.ok_or_else(|| error(entry.offset, Code::State))?;
    let kind = e.u32(&entry.raw);
    if kind == 0x40000bad {
        return Ok(None);
    }
    let opt_start = match &entry.content {
        Content::Section { .. } => Some(24),
        Content::Interface(_) => Some(16),
        Content::Packet(p) if kind != 3 => aligned(p.data.end),
        _ => None,
    };
    let Some(opt_start) = opt_start else {
        return Ok(Some(entry.raw.to_vec()));
    };
    let mut raw = entry.raw[..opt_start].to_vec();
    for (code, span) in options(&entry.raw, opt_start..entry.raw.len() - 4, e)
        .map_err(|(at, code)| error(entry.offset + at as u64, code))?
    {
        if code != 19372 && code != 19373 {
            raw.extend_from_slice(
                &entry.raw[span.start - 4..aligned(span.end).expect("validated option padding")],
            );
        }
    }
    if kind == SHB {
        raw[14..16].fill(0);
        raw[16..24].fill(0xff);
    }
    if kind == 2 {
        // A selected export is a rewrite: upgrade obsolete PB to EPB and retain its drop count.
        let id = u32::from(e.u16(&entry.raw[8..]));
        raw[..4].copy_from_slice(&match e {
            Endian::Little => 6u32.to_le_bytes(),
            Endian::Big => 6u32.to_be_bytes(),
        });
        raw[8..12].copy_from_slice(&match e {
            Endian::Little => id.to_le_bytes(),
            Endian::Big => id.to_be_bytes(),
        });
        let drops = e.u16(&entry.raw[10..]);
        if drops != u16::MAX {
            let header = match e {
                Endian::Little => 0x00080004u32.to_le_bytes(),
                Endian::Big => 0x00040008u32.to_be_bytes(),
            };
            raw.extend_from_slice(&header);
            raw.extend_from_slice(&match e {
                Endian::Little => u64::from(drops).to_le_bytes(),
                Endian::Big => u64::from(drops).to_be_bytes(),
            });
        }
    }
    raw.extend_from_slice(&[0; 4]);
    if raw.len() > MAX_BLOCK - 4 {
        return Err(error(entry.offset, Code::Limit));
    }
    let length = raw.len() as u32 + 4;
    let bytes = match e {
        Endian::Little => length.to_le_bytes(),
        Endian::Big => length.to_be_bytes(),
    };
    raw[4..8].copy_from_slice(&bytes);
    raw.extend_from_slice(&bytes);
    Ok(Some(raw))
}
