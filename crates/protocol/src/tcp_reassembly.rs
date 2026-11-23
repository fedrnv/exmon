//! Passive bounded reconstruction, not an endpoint TCP implementation. Call on a worker.
use crate::network::{Decoder, Ip, Transport};
use exmon_domain::observation::{Metadata, Observation};
use std::{collections::BTreeMap, fmt, net::SocketAddr, ops::Range};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub flows: usize,
    pub total_bytes: usize,
    pub flow_bytes: usize,
    pub total_chunks: usize,
    pub direction_chunks: usize,
    pub input_bytes: usize,
    pub sequence_window: u32,
    pub gap_timeout_ns: u64,
    pub idle_timeout_ns: u64,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            flows: 256,
            total_bytes: 4 * 1024 * 1024,
            flow_bytes: 128 * 1024,
            total_chunks: 2048,
            direction_chunks: 64,
            input_bytes: 65536,
            sequence_window: 1024 * 1024,
            gap_timeout_ns: 2_000_000_000,
            idle_timeout_ns: 60_000_000_000,
        }
    }
}
impl Limits {
    fn validate(self) -> Result<(), Error> {
        if !(1..=1024).contains(&self.flows)
            || !(1..=8 * 1024 * 1024).contains(&self.total_bytes)
            || !(1..=1024 * 1024).contains(&self.flow_bytes)
            || self.flow_bytes > self.total_bytes
            || !(1..=4096).contains(&self.total_chunks)
            || !(1..=128).contains(&self.direction_chunks)
            || self.direction_chunks > self.total_chunks
            || !(1..=65536).contains(&self.input_bytes)
            || self.sequence_window > 1 << 30
            || self.sequence_window < u32::try_from(self.input_bytes).unwrap()
            || self.gap_timeout_ns == 0
            || self.idle_timeout_ns == 0
        {
            return Err(Error::InvalidLimits);
        }
        Ok(())
    }
    /// Conservative bound for a single push/tick/finish result, not an internal queue.
    pub fn output_event_bound(self) -> usize {
        self.total_chunks
            .saturating_mul(2)
            .saturating_add(self.flows.saturating_mul(8))
            .saturating_add(self.direction_chunks.saturating_mul(2))
            .saturating_add(32)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    InvalidLimits,
    InvalidSegment,
    UnusablePacket,
    ClockWentBackwards,
    Exhausted,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TCP reconstruction: {self:?}")
    }
}
impl std::error::Error for Error {}
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Context {
    pub interface: u64,
    pub epoch: u64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Vlan {
    pub tpid: u16,
    pub id: u16,
}
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct FlowKey {
    pub context: Context,
    pub a: SocketAddr,
    pub b: SocketAddr,
    pub vlans: [Option<Vlan>; 8],
}
impl fmt::Debug for FlowKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("FlowKey([REDACTED])")
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    AtoB,
    BtoA,
}
impl Side {
    fn index(self) -> usize {
        match self {
            Self::AtoB => 0,
            Self::BtoA => 1,
        }
    }
    fn from_index(index: usize) -> Self {
        if index == 0 { Self::AtoB } else { Self::BtoA }
    }
}
/// original_payload_len excludes SYN/FIN and includes bytes missing due to snaplen.
pub struct Segment<'a> {
    pub source: SocketAddr,
    pub destination: SocketAddr,
    pub vlans: &'a [Vlan],
    pub sequence: u32,
    pub acknowledgement: Option<u32>,
    pub syn: bool,
    pub fin: bool,
    pub rst: bool,
    pub payload: &'a [u8],
    pub original_payload_len: usize,
}
impl fmt::Debug for Segment<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Segment([REDACTED])")
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseReason {
    Fin,
    Reset,
    Idle,
    Capacity,
    Replaced,
    CaptureEnded,
    ControlConflict,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GapReason {
    Deadline,
    Idle,
    Capacity,
    Reset,
    Replaced,
    CaptureEnded,
    ControlConflict,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overlap {
    IdenticalBuffered,
    ConflictingBuffered,
    UnverifiablePast,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rejection {
    SequenceWindow,
    BufferBytes,
    BufferChunks,
    AfterFin,
    AckWindow,
}
#[derive(Clone, PartialEq, Eq)]
pub enum EventKind {
    Opened {
        key: FlowKey,
    },
    UnknownPrefix,
    Data {
        sequence: Range<u64>,
        bytes: Vec<u8>,
    },
    /// A permanent boundary: consumers MUST reset framing across this range.
    Gap {
        sequence: Range<u64>,
        reason: GapReason,
    },
    /// Packet loss may still be recovered before a permanent Gap is emitted.
    Truncated {
        sequence: Range<u64>,
    },
    Overlap {
        sequence: Range<u64>,
        policy: Overlap,
    },
    Rejected {
        sequence: Option<Range<u64>>,
        reason: Rejection,
    },
    HalfClosed {
        sequence: u64,
    },
    Closed {
        reason: CloseReason,
        incomplete: bool,
        ambiguous: bool,
    },
}
impl fmt::Debug for EventKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Opened { .. } => "Opened([REDACTED])",
            Self::Data { .. } => "Data([REDACTED])",
            Self::Gap { .. } => "Gap",
            Self::Truncated { .. } => "Truncated",
            Self::Overlap { .. } => "Overlap",
            Self::Rejected { .. } => "Rejected",
            Self::HalfClosed { .. } => "HalfClosed",
            Self::Closed { .. } => "Closed",
            Self::UnknownPrefix => "UnknownPrefix",
        })
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub flow: u64,
    pub side: Option<Side>,
    pub kind: EventKind,
}
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Stats {
    pub flows: usize,
    pub buffered_bytes: usize,
    pub buffered_chunks: usize,
    pub emitted_bytes: u64,
    pub gaps: u64,
    pub overlaps: u64,
    pub rejected_segments: u64,
    pub evictions: u64,
    pub ignored_controls: u64,
    pub unusable_packets: u64,
}
#[derive(Default)]
struct Direction {
    next: Option<u64>,
    origin: Option<u64>,
    syn: Option<u32>,
    fin: Option<u64>,
    closed: bool,
    known_end: u64,
    payload_end: u64,
    pending: BTreeMap<u64, Vec<u8>>,
    bytes: usize,
    gap_since: Option<u64>,
}
struct Flow {
    id: u64,
    directions: [Direction; 2],
    last_seen: u64,
    unknown_prefix: bool,
    gapped: bool,
    ambiguous: bool,
}
fn emit(events: &mut Vec<Event>, id: u64, side: Side, kind: EventKind) {
    events.push(Event {
        flow: id,
        side: Some(side),
        kind,
    });
}
fn serial(sequence: u32, anchor: u64) -> Option<u64> {
    let delta = sequence.wrapping_sub(anchor as u32);
    if delta == 1 << 31 {
        return None;
    }
    anchor.checked_add_signed(i64::from(delta as i32))
}
fn has_gap(d: &Direction) -> bool {
    d.next.is_some_and(|next| !d.closed && d.known_end > next)
}
fn update_gap(d: &mut Direction, now: u64) {
    if has_gap(d) {
        d.gap_since.get_or_insert(now);
    } else {
        d.gap_since = None;
    }
}

pub struct Reassembler {
    limits: Limits,
    flows: BTreeMap<FlowKey, Flow>,
    next_id: u64,
    last_now: Option<u64>,
    stats: Stats,
}
impl Reassembler {
    pub fn new(limits: Limits) -> Result<Self, Error> {
        limits.validate()?;
        Ok(Self {
            limits,
            flows: BTreeMap::new(),
            next_id: 0,
            last_now: None,
            stats: Stats::default(),
        })
    }
    pub fn limits(&self) -> Limits {
        self.limits
    }
    pub fn stats(&self) -> Stats {
        Stats {
            flows: self.flows.len(),
            ..self.stats
        }
    }
    fn clock(&mut self, now: u64) -> Result<(), Error> {
        if self.last_now.is_some_and(|old| now < old) {
            return Err(Error::ClockWentBackwards);
        }
        self.last_now = Some(now);
        Ok(())
    }
    fn key(&self, context: Context, s: &Segment<'_>) -> Result<(FlowKey, Side), Error> {
        if s.source == s.destination
            || s.source.is_ipv4() != s.destination.is_ipv4()
            || s.payload.len() > s.original_payload_len
            || s.original_payload_len > self.limits.input_bytes
            || s.vlans.len() > 8
            || s.vlans
                .iter()
                .any(|v| v.id > 4095 || !matches!(v.tpid, 0x8100 | 0x88a8))
            || (s.syn && (s.fin || s.rst))
            || (s.fin && s.rst)
        {
            return Err(Error::InvalidSegment);
        }
        let mut vlans = [None; 8];
        for (target, vlan) in vlans.iter_mut().zip(s.vlans) {
            *target = Some(*vlan);
        }
        let (a, b, side) = if s.source < s.destination {
            (s.source, s.destination, Side::AtoB)
        } else {
            (s.destination, s.source, Side::BtoA)
        };
        Ok((
            FlowKey {
                context,
                a,
                b,
                vlans,
            },
            side,
        ))
    }
    pub fn push_observation(
        &mut self,
        context: Context,
        now: u64,
        observation: &Observation,
    ) -> Result<Vec<Event>, Error> {
        if !matches!(observation.metadata(), Metadata::Ethernet { link_type: 1 }) {
            self.stats.unusable_packets = self.stats.unusable_packets.saturating_add(1);
            return Err(Error::UnusablePacket);
        }
        let packet = Decoder::default().decode_observation(observation);
        let Some(Transport::Tcp(tcp)) = packet.transport.as_ref() else {
            self.stats.unusable_packets = self.stats.unusable_packets.saturating_add(1);
            return Err(Error::UnusablePacket);
        };
        let (source, destination, length) = match packet.ip.as_ref().unwrap() {
            Ip::V4(ip) => (
                ip.source.into(),
                ip.destination.into(),
                ip.total_length.checked_sub(ip.header.len()),
            ),
            Ip::V6(ip) => (
                ip.source.into(),
                ip.destination.into(),
                ip.payload_length
                    .checked_sub(packet.extensions.iter().map(|e| e.range.len()).sum()),
            ),
        };
        let length = length
            .and_then(|n| n.checked_sub(tcp.header.len()))
            .ok_or(Error::UnusablePacket)?;
        let vlans: Vec<_> = packet
            .vlans
            .iter()
            .map(|v| Vlan {
                tpid: v.tpid,
                id: v.id(),
            })
            .collect();
        self.push(
            context,
            now,
            Segment {
                source: SocketAddr::new(source, tcp.source_port),
                destination: SocketAddr::new(destination, tcp.destination_port),
                vlans: &vlans,
                sequence: tcp.sequence,
                acknowledgement: (tcp.flags & 0x10 != 0).then_some(tcp.acknowledgement),
                syn: tcp.flags & 2 != 0,
                fin: tcp.flags & 1 != 0,
                rst: tcp.flags & 4 != 0,
                payload: packet.payload(),
                original_payload_len: length,
            },
        )
    }
    pub fn push(
        &mut self,
        context: Context,
        now: u64,
        s: Segment<'_>,
    ) -> Result<Vec<Event>, Error> {
        let (key, side) = self.key(context, &s)?;
        if self.next_id == u64::MAX {
            return Err(Error::Exhausted);
        }
        self.clock(now)?;
        let mut events = Vec::new();
        self.expire(now, &mut events);
        let mut existing = self.flows.remove(&key);
        if s.syn
            && existing.as_ref().is_some_and(|flow| {
                let d = &flow.directions[side.index()];
                d.origin.is_some()
                    && d.syn != Some(s.sequence)
                    && !(d.syn.is_none()
                        && serial(s.sequence, d.origin.unwrap()).and_then(|n| n.checked_add(1))
                            == d.origin)
            })
        {
            self.close(existing.take().unwrap(), CloseReason::Replaced, &mut events);
        }
        if s.rst {
            if let Some(flow) = existing {
                self.close(flow, CloseReason::Reset, &mut events);
            } else {
                self.stats.ignored_controls = self.stats.ignored_controls.saturating_add(1);
            }
            return Ok(events);
        }
        if existing.is_none() && !s.syn && !s.fin && s.original_payload_len == 0 {
            self.stats.ignored_controls = self.stats.ignored_controls.saturating_add(1);
            return Ok(events);
        }
        let mut flow = if let Some(flow) = existing {
            flow
        } else {
            if self.flows.len() == self.limits.flows {
                let oldest = self
                    .flows
                    .iter()
                    .min_by_key(|(_, f)| (f.last_seen, f.id))
                    .unwrap()
                    .0
                    .clone();
                let old = self.flows.remove(&oldest).unwrap();
                self.stats.evictions = self.stats.evictions.saturating_add(1);
                self.close(old, CloseReason::Capacity, &mut events);
            }
            self.next_id += 1;
            events.push(Event {
                flow: self.next_id,
                side: None,
                kind: EventKind::Opened { key: key.clone() },
            });
            Flow {
                id: self.next_id,
                directions: Default::default(),
                last_seen: now,
                unknown_prefix: false,
                gapped: false,
                ambiguous: false,
            }
        };
        if let Some(reason) = self.process(&mut flow, side, now, &s, &mut events) {
            self.close(flow, reason, &mut events);
        } else if flow.directions.iter().all(|d| d.closed) {
            self.close(flow, CloseReason::Fin, &mut events);
        } else {
            self.flows.insert(key, flow);
        }
        Ok(events)
    }
    fn process(
        &mut self,
        f: &mut Flow,
        side: Side,
        now: u64,
        s: &Segment<'_>,
        out: &mut Vec<Event>,
    ) -> Option<CloseReason> {
        let index = side.index();
        if let Some(ack) = s.acknowledgement {
            let peer = &mut f.directions[1 - index];
            if let Some(next) = peer.next
                && !peer.closed
            {
                if let Some(ack) = serial(ack, next)
                    .filter(|n| n.abs_diff(next) <= u64::from(self.limits.sequence_window))
                {
                    peer.known_end = peer.known_end.max(ack.min(peer.fin.unwrap_or(u64::MAX)));
                    update_gap(peer, now);
                } else {
                    emit(
                        out,
                        f.id,
                        Side::from_index(1 - index),
                        EventKind::Rejected {
                            sequence: None,
                            reason: Rejection::AckWindow,
                        },
                    );
                }
            }
        }
        let total_flow_bytes = f.directions.iter().map(|d| d.bytes).sum::<usize>();
        let d = &mut f.directions[index];
        if d.next.is_none() {
            if !s.syn && !s.fin && s.original_payload_len == 0 {
                f.last_seen = now;
                return None;
            }
            let origin = (1u64 << 32) + u64::from(s.sequence) + u64::from(s.syn);
            d.next = Some(origin);
            d.origin = Some(origin);
            d.known_end = origin;
            d.payload_end = origin;
            if s.syn {
                d.syn = Some(s.sequence);
            } else {
                f.unknown_prefix = true;
                emit(out, f.id, side, EventKind::UnknownPrefix);
            }
        }
        let next = d.next.unwrap();
        let positions = serial(s.sequence, next)
            .and_then(|n| n.checked_add(u64::from(s.syn)))
            .and_then(|start| {
                let end = start.checked_add(s.original_payload_len as u64)?;
                (end < u64::MAX
                    && start.abs_diff(next) <= u64::from(self.limits.sequence_window)
                    && end.saturating_sub(next) <= u64::from(self.limits.sequence_window))
                .then_some((start, end))
            });
        let Some((start, end)) = positions else {
            self.stats.rejected_segments = self.stats.rejected_segments.saturating_add(1);
            emit(
                out,
                f.id,
                side,
                EventKind::Rejected {
                    sequence: None,
                    reason: Rejection::SequenceWindow,
                },
            );
            return None;
        };
        if s.syn {
            d.syn = Some(s.sequence);
        }
        if s.fin
            && (d.fin.is_some_and(|fin| fin != end) || d.payload_end > end || d.known_end > end + 1)
        {
            f.ambiguous = true;
            return Some(CloseReason::ControlConflict);
        }
        if d.fin.is_some_and(|fin| {
            end > fin
                && !(end == fin + 1
                    && s.original_payload_len == 0
                    && s.acknowledgement.is_some()
                    && !s.syn
                    && !s.fin)
        }) {
            self.stats.rejected_segments = self.stats.rejected_segments.saturating_add(1);
            emit(
                out,
                f.id,
                side,
                EventKind::Rejected {
                    sequence: Some(start..end),
                    reason: Rejection::AfterFin,
                },
            );
            return None;
        }
        if s.fin && (end < next || d.pending.iter().any(|(n, b)| n + b.len() as u64 > end)) {
            // A repeated already-consumed FIN is harmless.
            if !(d.closed && d.fin == Some(end)) {
                f.ambiguous = true;
                return Some(CloseReason::ControlConflict);
            }
        }
        f.last_seen = now;
        d.known_end = d.known_end.max(end.min(d.fin.unwrap_or(u64::MAX)));
        if s.original_payload_len > 0 {
            d.payload_end = d.payload_end.max(end);
        }
        if s.fin {
            d.fin = Some(end);
            // ACK or zero-length sequence evidence may have included this FIN's
            // consumed sequence number before its capture. It is not a data gap.
            d.known_end = end;
        }
        let captured_end = start + s.payload.len() as u64;
        if captured_end < end {
            emit(
                out,
                f.id,
                side,
                EventKind::Truncated {
                    sequence: captured_end..end,
                },
            );
        }
        if start < next && captured_end > start {
            emit(
                out,
                f.id,
                side,
                EventKind::Overlap {
                    sequence: start..captured_end.min(next),
                    policy: Overlap::UnverifiablePast,
                },
            );
            self.stats.overlaps = self.stats.overlaps.saturating_add(1);
            f.ambiguous = true;
        }
        let mut holes = Vec::new();
        let mut cursor = start.max(next);
        for (&queued, bytes) in &d.pending {
            let queued_end = queued + bytes.len() as u64;
            if queued_end <= cursor {
                continue;
            }
            if queued >= captured_end {
                break;
            }
            if cursor < queued {
                holes.push(cursor..queued.min(captured_end));
            }
            let a = start.max(queued).max(next);
            let b = captured_end.min(queued_end);
            if a < b {
                let same = bytes[(a - queued) as usize..(b - queued) as usize]
                    == s.payload[(a - start) as usize..(b - start) as usize];
                emit(
                    out,
                    f.id,
                    side,
                    EventKind::Overlap {
                        sequence: a..b,
                        policy: if same {
                            Overlap::IdenticalBuffered
                        } else {
                            Overlap::ConflictingBuffered
                        },
                    },
                );
                self.stats.overlaps = self.stats.overlaps.saturating_add(1);
                f.ambiguous |= !same;
            }
            cursor = cursor.max(queued_end);
        }
        if cursor < captured_end {
            holes.push(cursor..captured_end);
        }
        if start <= next && captured_end > next {
            // Deliver input around first-seen queued spans without temporarily
            // charging in-order data against pending-buffer limits.
            while d.next.unwrap() < captured_end {
                Self::drain(d, f.id, side, &mut self.stats, out);
                let cursor = d.next.unwrap();
                if cursor >= captured_end {
                    break;
                }
                let end = d
                    .pending
                    .first_key_value()
                    .map_or(captured_end, |(&n, _)| n.min(captured_end));
                let bytes = s.payload[(cursor - start) as usize..(end - start) as usize].to_vec();
                Self::data(d, f.id, side, cursor, bytes, &mut self.stats, out);
            }
        } else if !holes.is_empty() {
            let bytes = holes
                .iter()
                .map(|r| (r.end - r.start) as usize)
                .sum::<usize>();
            let rejection = if total_flow_bytes + bytes > self.limits.flow_bytes
                || self.stats.buffered_bytes + bytes > self.limits.total_bytes
            {
                Some(Rejection::BufferBytes)
            } else if d.pending.len() + holes.len() > self.limits.direction_chunks
                || self.stats.buffered_chunks + holes.len() > self.limits.total_chunks
            {
                Some(Rejection::BufferChunks)
            } else {
                None
            };
            if let Some(reason) = rejection {
                self.stats.rejected_segments = self.stats.rejected_segments.saturating_add(1);
                emit(
                    out,
                    f.id,
                    side,
                    EventKind::Rejected {
                        sequence: Some(start..captured_end),
                        reason,
                    },
                );
            } else {
                for range in holes {
                    let bytes = s.payload
                        [(range.start - start) as usize..(range.end - start) as usize]
                        .to_vec();
                    d.bytes += bytes.len();
                    self.stats.buffered_bytes += bytes.len();
                    self.stats.buffered_chunks += 1;
                    d.pending.insert(range.start, bytes);
                }
            }
        }
        Self::drain(d, f.id, side, &mut self.stats, out);
        Self::fin(d, f.id, side, out);
        update_gap(d, now);
        None
    }
    fn data(
        d: &mut Direction,
        id: u64,
        side: Side,
        start: u64,
        bytes: Vec<u8>,
        stats: &mut Stats,
        out: &mut Vec<Event>,
    ) {
        let end = start + bytes.len() as u64;
        d.next = Some(end);
        stats.emitted_bytes = stats.emitted_bytes.saturating_add(bytes.len() as u64);
        emit(
            out,
            id,
            side,
            EventKind::Data {
                sequence: start..end,
                bytes,
            },
        );
    }
    fn drain(d: &mut Direction, id: u64, side: Side, stats: &mut Stats, out: &mut Vec<Event>) {
        while d
            .pending
            .first_key_value()
            .is_some_and(|(n, _)| Some(*n) == d.next)
        {
            let (start, bytes) = d.pending.pop_first().unwrap();
            d.bytes -= bytes.len();
            stats.buffered_bytes -= bytes.len();
            stats.buffered_chunks -= 1;
            Self::data(d, id, side, start, bytes, stats, out);
        }
    }
    fn fin(d: &mut Direction, id: u64, side: Side, out: &mut Vec<Event>) {
        if !d.closed
            && let Some(fin) = d.fin
            && d.next == Some(fin)
        {
            d.next = Some(fin + 1);
            d.closed = true;
            d.gap_since = None;
            emit(out, id, side, EventKind::HalfClosed { sequence: fin });
        }
    }
    fn flush(
        d: &mut Direction,
        id: u64,
        side: Side,
        reason: GapReason,
        stats: &mut Stats,
        out: &mut Vec<Event>,
    ) -> bool {
        let mut gapped = false;
        while let Some(next) = d.next {
            Self::drain(d, id, side, stats, out);
            let next = d.next.unwrap().max(next);
            let target = d
                .pending
                .first_key_value()
                .map_or(d.known_end, |(&n, _)| n)
                .min(d.fin.unwrap_or(u64::MAX));
            if next >= target {
                break;
            }
            emit(
                out,
                id,
                side,
                EventKind::Gap {
                    sequence: next..target,
                    reason,
                },
            );
            stats.gaps = stats.gaps.saturating_add(1);
            gapped = true;
            d.next = Some(target);
        }
        Self::fin(d, id, side, out);
        d.gap_since = None;
        gapped
    }
    fn close(&mut self, mut f: Flow, reason: CloseReason, out: &mut Vec<Event>) {
        let gap_reason = match reason {
            CloseReason::Fin => GapReason::Deadline,
            CloseReason::Reset => GapReason::Reset,
            CloseReason::Idle => GapReason::Idle,
            CloseReason::Capacity => GapReason::Capacity,
            CloseReason::Replaced => GapReason::Replaced,
            CloseReason::CaptureEnded => GapReason::CaptureEnded,
            CloseReason::ControlConflict => GapReason::ControlConflict,
        };
        for (index, d) in f.directions.iter_mut().enumerate() {
            f.gapped |= Self::flush(
                d,
                f.id,
                Side::from_index(index),
                gap_reason,
                &mut self.stats,
                out,
            );
        }
        out.push(Event {
            flow: f.id,
            side: None,
            kind: EventKind::Closed {
                reason,
                incomplete: f.unknown_prefix || f.gapped || reason != CloseReason::Fin,
                ambiguous: f.ambiguous,
            },
        });
    }
    fn expire(&mut self, now: u64, out: &mut Vec<Event>) {
        let mut keys: Vec<_> = self
            .flows
            .iter()
            .filter(|(_, f)| {
                now - f.last_seen >= self.limits.idle_timeout_ns
                    || f.directions.iter().any(|d| {
                        d.gap_since
                            .is_some_and(|since| now - since >= self.limits.gap_timeout_ns)
                    })
            })
            .map(|(key, f)| (f.id, key.clone()))
            .collect();
        keys.sort_unstable_by_key(|(id, _)| *id);
        for (_, key) in keys {
            let mut f = self.flows.remove(&key).unwrap();
            if now - f.last_seen >= self.limits.idle_timeout_ns {
                self.close(f, CloseReason::Idle, out);
                continue;
            }
            for (index, d) in f.directions.iter_mut().enumerate() {
                if d.gap_since
                    .is_some_and(|since| now - since >= self.limits.gap_timeout_ns)
                {
                    f.gapped |= Self::flush(
                        d,
                        f.id,
                        Side::from_index(index),
                        GapReason::Deadline,
                        &mut self.stats,
                        out,
                    );
                }
            }
            if f.directions.iter().all(|d| d.closed) {
                self.close(f, CloseReason::Fin, out);
            } else {
                self.flows.insert(key, f);
            }
        }
    }
    pub fn tick(&mut self, now: u64) -> Result<Vec<Event>, Error> {
        self.clock(now)?;
        let mut out = Vec::new();
        self.expire(now, &mut out);
        Ok(out)
    }
    pub fn finish(&mut self, now: u64) -> Result<Vec<Event>, Error> {
        self.clock(now)?;
        let mut out = Vec::new();
        let mut flows: Vec<_> = std::mem::take(&mut self.flows).into_values().collect();
        flows.sort_unstable_by_key(|f| f.id);
        for f in flows {
            self.close(f, CloseReason::CaptureEnded, &mut out);
        }
        Ok(out)
    }
}
