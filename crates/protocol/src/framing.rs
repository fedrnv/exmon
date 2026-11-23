//! Incremental worker-owned framing. One instance per ordered stream/direction/epoch.
use exmon_domain::{
    config::{Endian, Escape, Framing, Protocol},
    observation::{Metadata, Observation, TransportKind},
};
use std::{collections::VecDeque, fmt, ops::Range};
pub const MAX_INPUT: usize = 65_536;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Configuration,
    Input,
    ClockWentBackwards,
    Exhausted,
    WrongMode,
    Finished,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "framing: {self:?}")
    }
}
impl std::error::Error for Error {}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Source {
    pub observation: u64,
    pub offset: u32,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceSpan {
    pub observation: u64,
    pub offsets: Range<u32>,
    pub stream: Range<u64>,
}
#[derive(Clone, PartialEq, Eq)]
pub struct Message {
    bytes: Box<[u8]>,
    pub wire: Range<u64>,
    pub sources: Vec<SourceSpan>,
    pub byte_ranges: Vec<Range<u64>>,
    pub completed_ns: u64,
}
impl Message {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}
impl fmt::Debug for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FramedMessage")
            .field("wire", &self.wire)
            .field("bytes", &"[REDACTED]")
            .finish()
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    IncompleteCapture,
    UnknownOriginalLength,
    Noise,
    InvalidLength,
    Oversized,
    DiscardedUntilBoundary,
    Restarted,
    Gap,
    Incomplete,
    DanglingEscape,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Message(Message),
    Diagnostic { reason: Reason, wire: Range<u64> },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stats {
    pub position: u64,
    pub pending_decoded: usize,
    pub pending_source_bytes: usize,
    pub discarding: bool,
}
#[derive(Clone, Copy)]
struct Position {
    source: Source,
    stream: u64,
}
#[derive(Clone, Copy)]
struct Cell {
    value: u8,
    first: Position,
    second: Option<Position>,
}
impl Cell {
    fn wire(self) -> Range<u64> {
        self.first.stream..self.second.unwrap_or(self.first).stream + 1
    }
}
pub struct Framer {
    rule: Framing,
    escape: Option<Escape>,
    limit: usize,
    pending: VecDeque<Cell>,
    escaped: Option<Position>,
    position: u64,
    now: Option<u64>,
    last_input: Option<u64>,
    active: bool,
    discarding: Option<u64>,
    noise_start: u64,
    finished: bool,
}
impl fmt::Debug for Framer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Framer([REDACTED])")
    }
}
impl Framer {
    pub fn new(rule: Framing, escape: Option<Escape>) -> Result<Self, Error> {
        let limit = rule.limit(escape).map_err(|_| Error::Configuration)? as usize;
        Ok(Self {
            rule,
            escape,
            limit,
            pending: VecDeque::new(),
            escaped: None,
            position: 0,
            now: None,
            last_input: None,
            active: false,
            discarding: None,
            noise_start: 0,
            finished: false,
        })
    }
    pub fn from_protocol(protocol: &Protocol) -> Result<Self, Error> {
        Self::new(protocol.framing.clone(), protocol.escape)
    }
    /// Direct domain boundary; raw observations remain immutable. Loss invalidates pending framing.
    pub fn observe(&mut self, id: u64, observation: &Observation) -> Result<Vec<Event>, Error> {
        let native = matches!(self.rule, Framing::Datagram {});
        if native
            && matches!(
                observation.metadata().kind(),
                TransportKind::Tcp | TransportKind::Serial
            )
        {
            return Err(Error::WrongMode);
        }
        let source = Source {
            observation: id,
            offset: 0,
        };
        let now = observation.timestamp_ns();
        let bytes = observation.bytes();
        self.input(source, now, bytes)?;
        if observation
            .original_length()
            .is_some_and(|n| n > bytes.len())
            || matches!(
                observation.metadata(),
                Metadata::Usb {
                    completed: false,
                    ..
                }
            )
        {
            let mut out = self.gap(now)?;
            let start = self.position;
            self.position += bytes.len() as u64;
            diagnostic(&mut out, Reason::IncompleteCapture, start..self.position);
            self.noise_start = self.position;
            return Ok(out);
        }
        if native {
            let start = self.position;
            let mut out = self.push_frame(source, now, bytes)?;
            if observation.original_length().is_none() {
                out.insert(
                    0,
                    Event::Diagnostic {
                        reason: Reason::UnknownOriginalLength,
                        wire: start..self.position,
                    },
                );
            }
            Ok(out)
        } else {
            self.push(source, now, bytes)
        }
    }
    pub fn stats(&self) -> Stats {
        Stats {
            position: self.position,
            pending_decoded: self.pending.len(),
            pending_source_bytes: self
                .pending
                .iter()
                .map(|c| 1 + usize::from(c.second.is_some()))
                .sum::<usize>()
                + usize::from(self.escaped.is_some()),
            discarding: self.discarding.is_some(),
        }
    }
    fn validate(&self, now: u64) -> Result<(), Error> {
        if self.finished {
            return Err(Error::Finished);
        }
        if self.now.is_some_and(|old| now < old) {
            return Err(Error::ClockWentBackwards);
        }
        Ok(())
    }
    fn input(&self, source: Source, now: u64, bytes: &[u8]) -> Result<(), Error> {
        self.validate(now)?;
        if bytes.len() > MAX_INPUT || source.offset.checked_add(bytes.len() as u32).is_none() {
            return Err(Error::Input);
        }
        self.position
            .checked_add(bytes.len() as u64)
            .ok_or(Error::Exhausted)?;
        Ok(())
    }
    fn decode(&mut self, source: Source, value: u8) -> Option<Cell> {
        let p = Position {
            source,
            stream: self.position,
        };
        self.position += 1;
        if let Some(first) = self.escaped.take() {
            return Some(Cell {
                value: value ^ self.escape.expect("escape prefix is configured").xor,
                first,
                second: Some(p),
            });
        }
        if self.escape.is_some_and(|e| e.byte == value) {
            self.escaped = Some(p);
            None
        } else {
            Some(Cell {
                value,
                first: p,
                second: None,
            })
        }
    }
    /// Arbitrary read fragments of one ordered stream. Native boundaries use push_frame instead.
    pub fn push(&mut self, source: Source, now: u64, bytes: &[u8]) -> Result<Vec<Event>, Error> {
        if matches!(self.rule, Framing::Datagram {}) {
            return Err(Error::WrongMode);
        }
        self.input(source, now, bytes)?;
        let mut out = self.tick(now)?;
        for (index, &value) in bytes.iter().enumerate() {
            if let Some(cell) = self.decode(
                Source {
                    observation: source.observation,
                    offset: source.offset + index as u32,
                },
                value,
            ) {
                self.consume(cell, now, &mut out);
            }
        }
        if !bytes.is_empty() {
            self.last_input = Some(now);
        }
        Ok(out)
    }
    /// Caller supplies a complete logical native frame, never a truncated capture/read fragment.
    pub fn push_frame(
        &mut self,
        source: Source,
        now: u64,
        bytes: &[u8],
    ) -> Result<Vec<Event>, Error> {
        if !matches!(self.rule, Framing::Datagram {}) {
            return Err(Error::WrongMode);
        }
        self.input(source, now, bytes)?;
        self.now = Some(now);
        let mut out = vec![];
        let begin = self.position;
        for (i, &b) in bytes.iter().enumerate() {
            if let Some(cell) = self.decode(
                Source {
                    observation: source.observation,
                    offset: source.offset + i as u32,
                },
                b,
            ) {
                self.pending.push_back(cell);
            }
        }
        if let Some(p) = self.escaped.take() {
            diagnostic(&mut out, Reason::DanglingEscape, p.stream..self.position);
            diagnostic(&mut out, Reason::Incomplete, begin..self.position);
            self.pending.clear();
        } else if bytes.is_empty() {
            out.push(Event::Message(Message {
                bytes: Box::new([]),
                wire: begin..begin,
                sources: vec![SourceSpan {
                    observation: source.observation,
                    offsets: source.offset..source.offset,
                    stream: begin..begin,
                }],
                byte_ranges: vec![],
                completed_ns: now,
            }));
        } else {
            let n = self.pending.len();
            self.emit(0..n, now, &mut out);
        }
        Ok(out)
    }
    pub fn tick(&mut self, now: u64) -> Result<Vec<Event>, Error> {
        self.validate(now)?;
        self.now = Some(now);
        let mut out = vec![];
        if let Framing::IdleGap { gap_ns, .. } = self.rule
            && let Some(last) = self.last_input
            && now - last >= gap_ns
        {
            // The predicate proves this addition is <=now, even close to u64::MAX.
            self.end_idle(last + gap_ns, &mut out);
            self.last_input = None;
        }
        Ok(out)
    }
    /// Application/reassembly loss is an explicit reset; no message spans the gap.
    pub fn gap(&mut self, now: u64) -> Result<Vec<Event>, Error> {
        self.validate(now)?;
        self.now = Some(now);
        let mut out = vec![];
        let start = self
            .pending
            .front()
            .map(|c| c.wire().start)
            .or(self.discarding)
            .or(self.escaped.map(|p| p.stream))
            .unwrap_or(self.position);
        diagnostic(&mut out, Reason::Gap, start..self.position);
        self.clear();
        Ok(out)
    }
    pub fn finish(&mut self, now: u64) -> Result<Vec<Event>, Error> {
        self.validate(now)?;
        let mut out = self.tick(now)?;
        if matches!(self.rule, Framing::IdleGap { .. }) {
            self.end_idle(now, &mut out);
        } else {
            if let Some(p) = self.escaped {
                diagnostic(&mut out, Reason::DanglingEscape, p.stream..self.position);
            }
            if let Some(start) = self.discarding {
                diagnostic(
                    &mut out,
                    Reason::DiscardedUntilBoundary,
                    start..self.position,
                );
            } else if let Some(first) = self.pending.front() {
                diagnostic(
                    &mut out,
                    Reason::Incomplete,
                    first.wire().start..self.position,
                );
            }
            if matches!(self.rule, Framing::Markers { .. })
                && !self.active
                && self.noise_start < self.position
            {
                diagnostic(&mut out, Reason::Noise, self.noise_start..self.position);
            }
        }
        self.clear();
        self.finished = true;
        Ok(out)
    }
    fn clear(&mut self) {
        self.pending.clear();
        self.escaped = None;
        self.active = false;
        self.discarding = None;
        self.last_input = None;
        self.noise_start = self.position;
    }
    fn end_idle(&mut self, now: u64, out: &mut Vec<Event>) {
        if let Some(start) = self.discarding {
            diagnostic(out, Reason::DiscardedUntilBoundary, start..self.position);
        } else if let Some(p) = self.escaped {
            diagnostic(out, Reason::DanglingEscape, p.stream..self.position);
            if let Some(first) = self.pending.front() {
                diagnostic(out, Reason::Incomplete, first.wire().start..self.position);
            }
        } else if !self.pending.is_empty() {
            let n = self.pending.len();
            self.emit(0..n, now, out);
        }
        self.clear();
    }
    fn emit(&mut self, payload: Range<usize>, now: u64, out: &mut Vec<Event>) {
        let first = self
            .pending
            .front()
            .expect("nonempty complete frame")
            .wire()
            .start;
        let last = self
            .pending
            .back()
            .expect("nonempty complete frame")
            .wire()
            .end;
        let mut bytes = Vec::with_capacity(payload.len());
        let mut byte_ranges = Vec::with_capacity(payload.len());
        let mut sources = vec![];
        for (index, c) in self.pending.drain(..).enumerate() {
            append_source(&mut sources, c.first);
            if let Some(p) = c.second {
                append_source(&mut sources, p);
            }
            if payload.contains(&index) {
                bytes.push(c.value);
                byte_ranges.push(c.wire());
            }
        }
        out.push(Event::Message(Message {
            bytes: bytes.into_boxed_slice(),
            wire: first..last,
            sources,
            byte_ranges,
            completed_ns: now,
        }));
        self.noise_start = last;
    }
    fn consume(&mut self, cell: Cell, now: u64, out: &mut Vec<Event>) {
        if matches!(self.rule, Framing::IdleGap { .. }) && self.discarding.is_some() {
            return;
        }
        self.pending.push_back(cell);
        match &self.rule {
            Framing::Datagram {} => unreachable!("native frames use separate entry point"),
            Framing::Fixed { length } => {
                let n = *length as usize;
                if self.pending.len() == n {
                    self.emit(0..n, now, out);
                }
            }
            Framing::LengthPrefix {
                offset,
                width,
                endian,
                ..
            } => {
                let offset = *offset as usize;
                let width = *width as usize;
                let header = offset + width;
                if self.pending.len() < header {
                    return;
                }
                let mut value = 0u64;
                for i in 0..width {
                    let index = if *endian == Endian::Big {
                        offset + i
                    } else {
                        offset + width - 1 - i
                    };
                    value = (value << 8) | u64::from(self.pending[index].value);
                }
                let total = header as u64 + value;
                if total > self.limit as u64 {
                    let c = self.pending.pop_front().expect("complete length header");
                    diagnostic(out, Reason::InvalidLength, c.wire());
                } else if self.pending.len() == total as usize {
                    self.emit(header..total as usize, now, out);
                }
            }
            Framing::Delimiter { bytes, .. } => {
                let n = bytes.len();
                let matched = ends(&self.pending, bytes);
                if let Some(start) = self.discarding {
                    if matched {
                        diagnostic(out, Reason::DiscardedUntilBoundary, start..cell.wire().end);
                        self.pending.clear();
                        self.discarding = None;
                    } else {
                        keep_last(&mut self.pending, n - 1);
                    }
                } else if self.pending.len() > self.limit {
                    diagnostic(
                        out,
                        Reason::Oversized,
                        self.pending.front().unwrap().wire().start..cell.wire().end,
                    );
                    if matched {
                        self.pending.clear();
                    } else {
                        self.discarding = Some(cell.wire().end);
                        keep_last(&mut self.pending, n - 1);
                    }
                } else if matched {
                    let len = self.pending.len();
                    self.emit(0..len - n, now, out);
                }
            }
            Framing::Markers { start, end, .. } => {
                let start_len = start.len();
                let end_len = end.len();
                let restart = restart_index(&self.pending, start, end, self.discarding.is_some());
                let complete = ends(&self.pending, end);
                if !self.active {
                    while !prefix(&self.pending, start) {
                        self.pending.pop_front();
                    }
                    if self.pending.len() == start_len {
                        let begin = self.pending.front().unwrap().wire().start;
                        if begin > self.noise_start {
                            diagnostic(out, Reason::Noise, self.noise_start..begin);
                        }
                        self.active = true;
                    }
                } else if let Some(discard) = self.discarding {
                    if complete {
                        diagnostic(
                            out,
                            Reason::DiscardedUntilBoundary,
                            discard..cell.wire().end,
                        );
                        self.clear();
                    } else if let Some(index) = restart {
                        let begin = self.pending[index].wire().start;
                        if begin > discard {
                            diagnostic(out, Reason::DiscardedUntilBoundary, discard..begin);
                        }
                        let retain = self.pending.len() - index;
                        keep_last(&mut self.pending, retain);
                        self.discarding = None;
                    } else {
                        keep_last(&mut self.pending, start_len + end_len - 1);
                    }
                } else if complete && self.pending.len() >= start_len + end_len {
                    let len = self.pending.len();
                    if len > self.limit {
                        diagnostic(
                            out,
                            Reason::Oversized,
                            self.pending.front().unwrap().wire().start..cell.wire().end,
                        );
                        self.clear();
                    } else {
                        self.emit(start_len..len - end_len, now, out);
                        self.active = false;
                    }
                } else if let Some(index) = restart {
                    let begin = self.pending[index].wire().start;
                    diagnostic(
                        out,
                        Reason::Restarted,
                        self.pending.front().unwrap().wire().start..begin,
                    );
                    let retain = self.pending.len() - index;
                    keep_last(&mut self.pending, retain);
                } else if self.pending.len() > self.limit {
                    diagnostic(
                        out,
                        Reason::Oversized,
                        self.pending.front().unwrap().wire().start..cell.wire().end,
                    );
                    self.discarding = Some(cell.wire().end);
                    keep_last(&mut self.pending, start_len + end_len - 1);
                }
            }
            Framing::IdleGap { .. } => {
                if self.pending.len() > self.limit {
                    diagnostic(
                        out,
                        Reason::Oversized,
                        self.pending.front().unwrap().wire().start..cell.wire().end,
                    );
                    self.pending.clear();
                    self.discarding = Some(cell.wire().end);
                }
            }
        }
    }
}
fn append_source(spans: &mut Vec<SourceSpan>, p: Position) {
    if let Some(last) = spans.last_mut()
        && last.observation == p.source.observation
        && last.offsets.end == p.source.offset
        && last.stream.end == p.stream
    {
        last.offsets.end += 1;
        last.stream.end += 1;
    } else {
        spans.push(SourceSpan {
            observation: p.source.observation,
            offsets: p.source.offset..p.source.offset + 1,
            stream: p.stream..p.stream + 1,
        });
    }
}
fn diagnostic(out: &mut Vec<Event>, reason: Reason, wire: Range<u64>) {
    out.push(Event::Diagnostic { reason, wire });
}
fn ends(pending: &VecDeque<Cell>, pattern: &[u8]) -> bool {
    pending.len() >= pattern.len()
        && pending
            .iter()
            .skip(pending.len() - pattern.len())
            .zip(pattern)
            .all(|(c, b)| c.value == *b && c.second.is_none())
}
fn prefix(pending: &VecDeque<Cell>, pattern: &[u8]) -> bool {
    pending.len() <= pattern.len()
        && pending
            .iter()
            .zip(pattern)
            .all(|(c, b)| c.value == *b && c.second.is_none())
}
fn keep_last(pending: &mut VecDeque<Cell>, n: usize) {
    while pending.len() > n {
        pending.pop_front();
    }
}
fn at(pending: &VecDeque<Cell>, offset: usize, pattern: &[u8]) -> bool {
    pattern
        .iter()
        .enumerate()
        .all(|(i, b)| pending[offset + i].value == *b && pending[offset + i].second.is_none())
}
fn restart_index(
    pending: &VecDeque<Cell>,
    start: &[u8],
    end: &[u8],
    discarding: bool,
) -> Option<usize> {
    let n = pending.len();
    if n < start.len() {
        return None;
    }
    let oldest = n
        .saturating_sub(start.len() + end.len())
        .max(usize::from(!discarding));
    let candidate = (oldest..=n - start.len())
        .rev()
        .find(|i| at(pending, *i, start))?;
    // A nested start may itself belong to a longer end marker. Decide only after
    // the bounded end-prefix lookahead resolves, always preferring a complete end.
    let end_prefix = (1..=n.min(end.len() - 1))
        .rev()
        .find(|k| at(pending, n - *k, &end[..*k]));
    if end_prefix.is_some_and(|k| candidate >= n - k) {
        None
    } else {
        Some(candidate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn final_source_and_stream_coordinates_never_wrap_or_mutate_on_error() {
        let mut f = Framer::new(Framing::Fixed { length: 1 }, None).unwrap();
        f.position = u64::MAX - 1;
        let events = f
            .push(
                Source {
                    observation: 1,
                    offset: u32::MAX - 1,
                },
                5,
                b"x",
            )
            .unwrap();
        let Event::Message(message) = &events[0] else {
            panic!()
        };
        assert_eq!(message.wire, (u64::MAX - 1)..u64::MAX);
        assert_eq!(message.sources[0].offsets, (u32::MAX - 1)..u32::MAX);
        let before = f.stats();
        assert_eq!(
            f.push(
                Source {
                    observation: 2,
                    offset: 0
                },
                6,
                b"y"
            )
            .unwrap_err(),
            Error::Exhausted
        );
        assert_eq!(f.stats(), before);
        assert!(f.tick(5).unwrap().is_empty());
    }
}
