//! Shared raw-packet inspection. Parsing, search and local file I/O run on a bounded worker.
use exmon_domain::{
    Direction,
    observation::{Metadata, Observation},
};
use exmon_protocol::network::{Decoder, Ip, Transport};
use std::{
    collections::VecDeque,
    io::Read,
    net::IpAddr,
    ops::Range,
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        mpsc::{self, SyncSender},
    },
    thread,
    time::Duration,
};
pub const FRAMES: usize = 512;
pub const BYTES: usize = 8 * 1024 * 1024;
pub const PAGE: usize = 256;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Node {
    pub label: String,
    pub range: Option<Range<usize>>,
    pub parent: Option<usize>,
}
pub struct Frame {
    pub observation: Arc<Observation>,
    pub nodes: Vec<Node>,
    pub protocol: &'static str,
    pub diagnostic: bool,
    pub source: Option<IpAddr>,
    pub destination: Option<IpAddr>,
    pub ports: Option<(u16, u16)>,
}
impl Frame {
    pub fn prepare(observation: Arc<Observation>) -> Self {
        let packet = Decoder::default().decode_observation(&observation);
        let mut nodes = Vec::new();
        let len = observation.bytes().len();
        let add = |nodes: &mut Vec<Node>, label: String, range: Option<Range<usize>>, parent| {
            // Only validated captured bytes can be selected, including malformed input.
            let range = range.filter(|r| r.start <= r.end && r.end <= len);
            let index = nodes.len();
            if index < 256 {
                nodes.push(Node {
                    label,
                    range,
                    parent,
                });
            }
            index
        };
        add(
            &mut nodes,
            format!(
                "Frame: {len} captured bytes / {:?} original",
                observation.original_length()
            ),
            Some(0..len),
            None,
        );
        let mut protocol = "Raw";
        let (mut source, mut destination, mut ports) = (None, None, None);
        if let Some(eth) = &packet.ethernet {
            protocol = "Ethernet";
            let n = add(
                &mut nodes,
                "Ethernet II".into(),
                Some(0..14 + 4 * packet.vlans.len()),
                Some(0),
            );
            let mac = |v: &[u8]| {
                v.iter()
                    .map(|b| format!("{b:02X}"))
                    .collect::<Vec<_>>()
                    .join(":")
            };
            add(
                &mut nodes,
                format!("Destination MAC: {}", mac(&eth.destination)),
                Some(0..6),
                Some(n),
            );
            add(
                &mut nodes,
                format!("Source MAC: {}", mac(&eth.source)),
                Some(6..12),
                Some(n),
            );
            for (i, tag) in packet.vlans.iter().enumerate() {
                add(
                    &mut nodes,
                    format!("VLAN {} (priority {})", tag.id(), tag.priority()),
                    Some(12 + 4 * i..16 + 4 * i),
                    Some(n),
                );
            }
            let at = 12 + 4 * packet.vlans.len();
            add(
                &mut nodes,
                format!("EtherType: 0x{:04X}", eth.ether_type),
                Some(at..at + 2),
                Some(n),
            );
        }
        match &packet.ip {
            Some(Ip::V4(ip)) => {
                protocol = "IPv4";
                source = Some(ip.source.into());
                destination = Some(ip.destination.into());
                let n = add(&mut nodes, "IPv4".into(), Some(ip.header.clone()), Some(0));
                let at = ip.header.start;
                for (label, r) in [
                    (format!("Source IP: {}", ip.source), at + 12..at + 16),
                    (
                        format!("Destination IP: {}", ip.destination),
                        at + 16..at + 20,
                    ),
                    (format!("TTL: {}", ip.ttl), at + 8..at + 9),
                    (format!("IP protocol: {}", ip.protocol), at + 9..at + 10),
                    (format!("Total length: {}", ip.total_length), at + 2..at + 4),
                    (
                        format!(
                            "Fragment offset: {} / more {}",
                            ip.fragment_offset, ip.more_fragments
                        ),
                        at + 6..at + 8,
                    ),
                    (
                        format!(
                            "Header checksum: {:?} (capture/offload dependent)",
                            ip.checksum
                        ),
                        at + 10..at + 12,
                    ),
                ] {
                    add(&mut nodes, label, Some(r), Some(n));
                }
                if ip.header.end > at + 20 {
                    add(
                        &mut nodes,
                        "IPv4 options".into(),
                        Some(at + 20..ip.header.end),
                        Some(n),
                    );
                }
            }
            Some(Ip::V6(ip)) => {
                protocol = "IPv6";
                source = Some(ip.source.into());
                destination = Some(ip.destination.into());
                let n = add(&mut nodes, "IPv6".into(), Some(ip.header.clone()), Some(0));
                let at = ip.header.start;
                for (label, r) in [
                    (format!("Source IP: {}", ip.source), at + 8..at + 24),
                    (
                        format!("Destination IP: {}", ip.destination),
                        at + 24..at + 40,
                    ),
                    (format!("Next header: {}", ip.next_header), at + 6..at + 7),
                    (format!("Hop limit: {}", ip.hop_limit), at + 7..at + 8),
                    (
                        format!("Payload length: {}", ip.payload_length),
                        at + 4..at + 6,
                    ),
                ] {
                    add(&mut nodes, label, Some(r), Some(n));
                }
            }
            None => {}
        }
        for ext in &packet.extensions {
            add(
                &mut nodes,
                format!("IPv6 extension {} / next {}", ext.kind, ext.next_header),
                Some(ext.range.clone()),
                Some(0),
            );
        }
        match &packet.transport {
            Some(Transport::Tcp(tcp)) => {
                protocol = "TCP";
                ports = Some((tcp.source_port, tcp.destination_port));
                let n = add(&mut nodes, "TCP".into(), Some(tcp.header.clone()), Some(0));
                let at = tcp.header.start;
                for (label, r) in [
                    (format!("Source port: {}", tcp.source_port), at..at + 2),
                    (
                        format!("Destination port: {}", tcp.destination_port),
                        at + 2..at + 4,
                    ),
                    (format!("Sequence: {}", tcp.sequence), at + 4..at + 8),
                    (
                        format!("Acknowledgement: {}", tcp.acknowledgement),
                        at + 8..at + 12,
                    ),
                    (format!("Flags: 0x{:03X}", tcp.flags), at + 12..at + 14),
                    (format!("Window: {}", tcp.window), at + 14..at + 16),
                    (
                        format!("Checksum: {:?} (capture/offload dependent)", tcp.checksum),
                        at + 16..at + 18,
                    ),
                ] {
                    add(&mut nodes, label, Some(r), Some(n));
                }
                if !tcp.options.is_empty() {
                    add(
                        &mut nodes,
                        "TCP options".into(),
                        Some(tcp.options.clone()),
                        Some(n),
                    );
                }
            }
            Some(Transport::Udp(udp)) => {
                protocol = "UDP";
                ports = Some((udp.source_port, udp.destination_port));
                let n = add(&mut nodes, "UDP".into(), Some(udp.header.clone()), Some(0));
                let at = udp.header.start;
                for (label, r) in [
                    (format!("Source port: {}", udp.source_port), at..at + 2),
                    (
                        format!("Destination port: {}", udp.destination_port),
                        at + 2..at + 4,
                    ),
                    (format!("UDP length: {}", udp.length), at + 4..at + 6),
                    (
                        format!("Checksum: {:?} (capture/offload dependent)", udp.checksum),
                        at + 6..at + 8,
                    ),
                ] {
                    add(&mut nodes, label, Some(r), Some(n));
                }
            }
            None => {}
        }
        match observation.metadata() {
            Metadata::Tcp { local, remote, .. } | Metadata::Udp { local, remote } => {
                protocol = if matches!(observation.metadata(), Metadata::Tcp { .. }) {
                    "TCP socket"
                } else {
                    "UDP socket"
                };
                add(
                    &mut nodes,
                    format!(
                        "Socket metadata: local {local}, remote {remote}; headers absent from captured bytes"
                    ),
                    None,
                    Some(0),
                );
            }
            _ => {}
        }
        add(
            &mut nodes,
            "Payload / undecoded bytes".into(),
            Some(packet.payload_range()),
            Some(0),
        );
        if let Some(d) = &packet.diagnostic {
            add(
                &mut nodes,
                format!("Decoder diagnostic: {d:?}"),
                None,
                Some(0),
            );
        }
        if packet.capture_truncated {
            add(
                &mut nodes,
                "Capture truncated: original bytes unavailable".into(),
                None,
                Some(0),
            );
        }
        let diagnostic = packet.diagnostic.is_some() || packet.capture_truncated;
        Self {
            observation,
            nodes,
            protocol,
            diagnostic,
            source,
            destination,
            ports,
        }
    }
    pub fn bytes(&self) -> &[u8] {
        self.observation.bytes()
    }
    pub fn selection(&self, r: Range<usize>) -> Result<&[u8], &'static str> {
        if r.is_empty() {
            return Err("Select at least one captured byte");
        }
        self.bytes()
            .get(r)
            .ok_or("Selection is outside captured bytes")
    }
    fn charge(&self) -> usize {
        self.bytes().len()
            + self
                .nodes
                .iter()
                .map(|n| 128 + n.label.len())
                .sum::<usize>()
            + 1024
    }
}
/// Linear-time bounded search, including overlapping matches.
#[derive(Clone)]
pub struct Pattern {
    bytes: Vec<u8>,
    prefix: Vec<usize>,
}
impl Pattern {
    pub fn parse(text: &str, hex: bool) -> Result<Self, &'static str> {
        if text.len() > 768 {
            return Err("Pattern exceeds 256 bytes");
        }
        let bytes = if hex {
            let digits: Vec<_> = text.bytes().filter(|c| !c.is_ascii_whitespace()).collect();
            if !digits.len().is_multiple_of(2) {
                return Err("HEX requires complete byte pairs");
            }
            let mut bytes = Vec::new();
            for pair in digits.as_chunks::<2>().0 {
                let hi = (pair[0] as char).to_digit(16).ok_or("Invalid HEX digit")?;
                let lo = (pair[1] as char).to_digit(16).ok_or("Invalid HEX digit")?;
                bytes.push((hi * 16 + lo) as u8);
            }
            bytes
        } else {
            if !text.is_ascii() {
                return Err("ASCII search accepts ASCII only");
            }
            text.as_bytes().to_vec()
        };
        if bytes.is_empty() || bytes.len() > 256 {
            return Err("Pattern must contain 1..256 bytes");
        }
        let mut prefix = vec![0; bytes.len()];
        for i in 1..bytes.len() {
            let mut j = prefix[i - 1];
            while j > 0 && bytes[i] != bytes[j] {
                j = prefix[j - 1];
            }
            if bytes[i] == bytes[j] {
                j += 1;
            }
            prefix[i] = j;
        }
        Ok(Self { bytes, prefix })
    }
    pub fn find(&self, input: &[u8], limit: usize) -> (Vec<Range<usize>>, bool) {
        let mut out = Vec::new();
        let mut j = 0;
        for (i, b) in input.iter().enumerate() {
            while j > 0 && *b != self.bytes[j] {
                j = self.prefix[j - 1];
            }
            if *b == self.bytes[j] {
                j += 1;
            }
            if j == self.bytes.len() {
                if out.len() == limit.min(256) {
                    return (out, true);
                }
                out.push(i + 1 - j..i + 1);
                j = self.prefix[j - 1];
            }
        }
        (out, false)
    }
}
enum Term {
    Protocol(String),
    Diagnostic,
    Ip(IpAddr, u8),
    Port(u16),
    Min(usize),
    Max(usize),
    Contains(Pattern),
}
#[derive(Default)]
pub struct Filter(Vec<Term>);
impl Filter {
    pub fn parse(text: &str) -> Result<Self, &'static str> {
        if text.len() > 1024 {
            return Err("Filter exceeds 1024 bytes");
        }
        let mut terms = Vec::new();
        for term in text.split_whitespace() {
            if terms.len() == 8 {
                return Err("At most eight AND terms");
            }
            terms.push(
                if ["ethernet", "ipv4", "ipv6", "tcp", "udp", "raw"].contains(&term) {
                    Term::Protocol(term.into())
                } else if term == "diagnostic" {
                    Term::Diagnostic
                } else if let Some(v) = term.strip_prefix("port=") {
                    Term::Port(v.parse().map_err(|_| "Invalid port")?)
                } else if let Some(v) = term.strip_prefix("len>=") {
                    Term::Min(v.parse().map_err(|_| "Invalid length")?)
                } else if let Some(v) = term.strip_prefix("len<=") {
                    Term::Max(v.parse().map_err(|_| "Invalid length")?)
                } else if let Some(v) = term.strip_prefix("contains=") {
                    Term::Contains(Pattern::parse(v, true)?)
                } else {
                    let (kind, v) = term.split_once('=').ok_or("Unknown display filter term")?;
                    let mode = match kind {
                        "host" => 0,
                        "src" => 1,
                        "dst" => 2,
                        _ => return Err("Unknown display filter term"),
                    };
                    Term::Ip(v.parse().map_err(|_| "Invalid IP address")?, mode)
                },
            );
        }
        Ok(Self(terms))
    }
    pub fn matches(&self, frame: &Frame) -> bool {
        self.0.iter().all(|t| match t {
            Term::Protocol(p) => match p.as_str() {
                "ethernet" => frame.nodes.iter().any(|n| n.label == "Ethernet II"),
                "ipv4" => frame.source.is_some_and(|a| a.is_ipv4()),
                "ipv6" => frame.source.is_some_and(|a| a.is_ipv6()),
                _ => frame.protocol.eq_ignore_ascii_case(p),
            },
            Term::Diagnostic => frame.diagnostic,
            Term::Ip(ip, mode) => {
                (*mode != 2 && frame.source == Some(*ip))
                    || (*mode != 1 && frame.destination == Some(*ip))
            }
            Term::Port(port) => frame.ports.is_some_and(|(a, b)| a == *port || b == *port),
            Term::Min(n) => frame.bytes().len() >= *n,
            Term::Max(n) => frame.bytes().len() <= *n,
            Term::Contains(p) => !p.find(frame.bytes(), 1).0.is_empty(),
        })
    }
}
#[derive(Clone)]
pub struct Record {
    pub id: u64,
    pub frame: Arc<Frame>,
}
#[derive(Clone, Default)]
pub struct Snapshot {
    pub revision: u64,
    pub generation: u64,
    pub context: u64,
    pub records: Vec<Record>,
    pub hidden: usize,
    pub file_mode: bool,
    pub synthetic: bool,
    pub evicted: u64,
    pub ignored_live: u64,
    pub input_dropped: u64,
    pub notice: Option<&'static str>,
    pub filter: String,
}
pub struct History {
    records: VecDeque<Record>,
    bytes: usize,
    next: u64,
    filter: Filter,
    state: Snapshot,
}
impl Default for History {
    fn default() -> Self {
        Self {
            records: VecDeque::new(),
            bytes: 0,
            next: 1,
            filter: Filter::default(),
            state: Snapshot::default(),
        }
    }
}
impl History {
    fn reset(&mut self) {
        self.records.clear();
        self.bytes = 0;
        self.state.evicted = 0;
        self.state.ignored_live = 0;
    }
    pub fn push(&mut self, observation: Arc<Observation>) -> Result<(), &'static str> {
        let id = self.next;
        self.next = self
            .next
            .checked_add(1)
            .ok_or("Packet identity exhausted")?;
        let frame = Arc::new(Frame::prepare(observation));
        self.bytes += frame.charge();
        self.records.push_back(Record { id, frame });
        while self.records.len() > FRAMES || self.bytes > BYTES {
            if let Some(old) = self.records.pop_front() {
                self.bytes -= old.frame.charge();
                self.state.evicted = self.state.evicted.saturating_add(1);
            }
        }
        Ok(())
    }
    pub fn set_filter(&mut self, text: &str) -> Result<(), &'static str> {
        let filter = Filter::parse(text)?;
        self.filter = filter;
        self.state.filter = text.into();
        Ok(())
    }
    pub fn snapshot(&self) -> Snapshot {
        let mut s = self.state.clone();
        s.records = self
            .records
            .iter()
            .filter(|r| self.filter.matches(&r.frame))
            .cloned()
            .collect();
        s.hidden = self.records.len() - s.records.len();
        s
    }
    /// Transactional import: malformed records never replace committed history.
    pub fn import(&mut self, input: impl Read) -> Result<(), &'static str> {
        use exmon_storage::pcap::{Content, Limits, Reader};
        let mut reader = Reader::new(
            input,
            Limits {
                total_bytes: exmon_storage::inspection_files::MAX_BYTES,
                entries: 10_000,
                ..Limits::default()
            },
        )
        .map_err(|_| "Invalid or oversized capture")?;
        let mut candidate = Self {
            next: self.next,
            ..Self::default()
        };
        while let Some(entry) = reader
            .next_entry()
            .map_err(|_| "Invalid or oversized capture")?
        {
            if let Content::Packet(p) = entry.content() {
                if !p.issues.is_empty() {
                    return Err("Invalid capture record; previous view retained");
                }
                let bytes = entry.payload().ok_or("Capture has no packet bytes")?;
                let mut obs = Observation::new(
                    0,
                    Direction::Unknown,
                    Metadata::Ethernet {
                        link_type: p.link_type,
                    },
                    Some(p.original_length as usize),
                    bytes,
                )
                .map_err(|_| "Unsupported capture record length")?;
                if p.timestamp.is_some() {
                    obs = entry
                        .observation(0)
                        .map_err(|_| "Invalid source timestamp or metadata")?;
                }
                candidate.push(Arc::new(obs))?;
            }
        }
        let context = self
            .state
            .context
            .checked_add(1)
            .ok_or("Inspection context exhausted")?;
        self.records = candidate.records;
        self.bytes = candidate.bytes;
        self.next = candidate.next;
        self.state.context = context;
        self.state.evicted = candidate.state.evicted;
        self.state.file_mode = true;
        self.state.synthetic = false;
        self.state.notice =
            Some("Imported capture; live packets are ignored until Live inspection");
        Ok(())
    }
}
pub enum Action {
    Filter(String),
    Import(String),
    Live,
    Sample,
    Search {
        frame: Arc<Frame>,
        text: String,
        hex: bool,
    },
    Copy {
        frame: Arc<Frame>,
        range: Range<usize>,
        ascii: bool,
    },
    Export {
        frame: Arc<Frame>,
        range: Range<usize>,
        path: String,
    },
}
pub enum Reply {
    Notice(&'static str),
    Search(Vec<Range<usize>>, bool),
    Copy(String),
}
impl History {
    fn action(&mut self, action: Action) -> Reply {
        match action {
            Action::Filter(text) => Reply::Notice(
                self.set_filter(&text)
                    .err()
                    .unwrap_or("Display filter applied; capture is unchanged"),
            ),
            Action::Import(path) => Reply::Notice(
                exmon_storage::inspection_files::open(Path::new(&path))
                    .and_then(|f| self.import(f))
                    .err()
                    .unwrap_or("Capture imported"),
            ),
            Action::Live | Action::Sample => {
                let synthetic = matches!(action, Action::Sample);
                let Some(context) = self.state.context.checked_add(1) else {
                    return Reply::Notice("Inspection context exhausted");
                };
                self.reset();
                self.state.context = context;
                self.state.file_mode = false;
                self.state.synthetic = synthetic;
                if synthetic {
                    Reply::Notice(
                        self.push(Arc::new(sample()))
                            .err()
                            .unwrap_or("Synthetic Ethernet/IPv4/UDP fixture; no transmission"),
                    )
                } else {
                    Reply::Notice("Live inspection; waiting for captured packets")
                }
            }
            Action::Search { frame, text, hex } => match Pattern::parse(&text, hex) {
                Ok(pattern) => {
                    let (ranges, more) = pattern.find(frame.bytes(), 256);
                    Reply::Search(ranges, more)
                }
                Err(e) => Reply::Notice(e),
            },
            Action::Copy {
                frame,
                range,
                ascii,
            } => match frame.selection(range) {
                Ok(bytes) => Reply::Copy(if ascii {
                    bytes
                        .iter()
                        .map(|b| {
                            if b.is_ascii_graphic() || *b == b' ' {
                                *b as char
                            } else {
                                '.'
                            }
                        })
                        .collect()
                } else {
                    bytes
                        .iter()
                        .map(|b| format!("{b:02X}"))
                        .collect::<Vec<_>>()
                        .join(" ")
                }),
                Err(e) => Reply::Notice(e),
            },
            Action::Export { frame, range, path } => Reply::Notice(
                frame
                    .selection(range)
                    .and_then(|bytes| {
                        exmon_storage::settings_files::create(Path::new(&path), bytes).map_err(
                            |e| match e {
                                exmon_storage::settings_files::Error::Exists => {
                                    "Destination exists; nothing overwritten"
                                }
                                _ => "Export failed; check destination for an incomplete file",
                            },
                        )
                    })
                    .err()
                    .unwrap_or("Selected raw bytes exported"),
            ),
        }
    }
}
struct Batch {
    generation: u64,
    rows: Vec<Arc<Observation>>,
}
pub struct Worker {
    data: SyncSender<Batch>,
    commands: SyncSender<Action>,
    replies: mpsc::Receiver<Reply>,
    snapshot: Arc<Mutex<Arc<Snapshot>>>,
    generation: Arc<AtomicU64>,
    dropped: Arc<AtomicU64>,
    quit: Arc<AtomicBool>,
    outstanding: AtomicUsize,
}
impl Worker {
    pub fn new() -> Result<Self, &'static str> {
        let (data, rx) = mpsc::sync_channel::<Batch>(8);
        let (commands, cmd) = mpsc::sync_channel::<Action>(8);
        let (tx, replies) = mpsc::sync_channel(8);
        let snapshot = Arc::new(Mutex::new(Arc::new(Snapshot::default())));
        let output = snapshot.clone();
        let generation = Arc::new(AtomicU64::new(0));
        let gen_id = generation.clone();
        let dropped = Arc::new(AtomicU64::new(0));
        let losses = dropped.clone();
        let quit = Arc::new(AtomicBool::new(false));
        let stop = quit.clone();
        thread::Builder::new()
            .name("packet-inspection".into())
            .spawn(move || {
                let mut history = History::default();
                let mut revision = 0u64;
                while !stop.load(Ordering::Acquire) {
                    let mut changed = false;
                    let generation = gen_id.load(Ordering::Acquire);
                    if generation != history.state.generation {
                        history.state.generation = generation;
                        if !history.state.file_mode && !history.state.synthetic {
                            history.reset();
                        }
                        changed = true;
                    }
                    if let Ok(action) = cmd.try_recv() {
                        if matches!(action, Action::Live) {
                            for _ in rx.try_iter() {}
                        }
                        let reply = history.action(action);
                        history.state.notice = match &reply {
                            Reply::Notice(n) => Some(*n),
                            _ => None,
                        };
                        // A single UI request may be pending; bounded replies never grow without a consumer.
                        let _ = tx.try_send(reply);
                        changed = true;
                    }
                    if let Ok(batch) = rx.try_recv()
                        && batch.generation == history.state.generation
                    {
                        if history.state.file_mode || history.state.synthetic {
                            history.state.ignored_live = history
                                .state
                                .ignored_live
                                .saturating_add(batch.rows.len() as u64);
                        } else {
                            for obs in batch.rows {
                                if let Err(e) = history.push(obs) {
                                    history.state.notice = Some(e);
                                }
                            }
                        }
                        changed = true;
                    }
                    if changed {
                        revision = revision.saturating_add(1);
                        history.state.revision = revision;
                        history.state.input_dropped = losses.load(Ordering::Relaxed);
                        if let Ok(mut out) = output.lock() {
                            *out = Arc::new(history.snapshot());
                        }
                    } else {
                        thread::sleep(Duration::from_millis(5));
                    }
                }
            })
            .map_err(|_| "Inspection worker unavailable")?;
        Ok(Self {
            data,
            commands,
            replies,
            snapshot,
            generation,
            dropped,
            quit,
            outstanding: AtomicUsize::new(0),
        })
    }
    pub fn begin(&self, generation: u64) {
        self.generation.store(generation, Ordering::Release);
    }
    pub fn submit(&self, batch: &crate::incoming::PreparedBatch) -> Result<(), &'static str> {
        if batch.rows.len() > 96
            || batch
                .rows
                .iter()
                .map(|r| r.observation.bytes().len())
                .sum::<usize>()
                > 1024 * 1024
        {
            return Err("Inspection batch exceeds bounds");
        }
        let rows = batch
            .rows
            .iter()
            .filter(|r| r.raw())
            .map(|r| r.observation.clone())
            .collect::<Vec<_>>();
        let count = rows.len() as u64;
        self.data
            .try_send(Batch {
                generation: batch.generation,
                rows,
            })
            .map_err(|_| {
                self.dropped.fetch_add(count, Ordering::Relaxed);
                "Inspection input full"
            })
    }
    pub fn request(&self, action: Action) -> Result<(), &'static str> {
        let valid = match &action {
            Action::Filter(s) => s.len() <= 1024,
            Action::Import(s) => s.len() <= 4096,
            Action::Search { text, .. } => text.len() <= 768,
            Action::Export { path, .. } => path.len() <= 4096,
            _ => true,
        };
        if !valid {
            return Err("Inspection command exceeds bounds");
        }
        self.outstanding
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                (n < 8).then_some(n + 1)
            })
            .map_err(|_| "Inspection command queue full")?;
        self.commands.try_send(action).map_err(|_| {
            self.outstanding.fetch_sub(1, Ordering::AcqRel);
            "Inspection command queue full"
        })
    }
    pub fn snapshot(&self) -> Option<Arc<Snapshot>> {
        self.snapshot.try_lock().ok().map(|s| s.clone())
    }
    pub fn reply(&self) -> Option<Reply> {
        self.replies.try_recv().ok().inspect(|_| {
            self.outstanding.fetch_sub(1, Ordering::AcqRel);
        })
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.quit.store(true, Ordering::Release);
    }
}
/// Deterministic local prototype fixture. No adapter, device or transmission involved.
pub fn sample() -> Observation {
    let mut bytes = vec![
        0x02, 0, 0, 0, 0, 1, 0x02, 0, 0, 0, 0, 2, 0x08, 0, 0x45, 0, 0, 36, 0, 1, 0, 0, 64, 17, 0,
        0, 127, 0, 0, 1, 127, 0, 0, 2, 0x2e, 0xe0, 0x2e, 0xe1, 0, 16, 0, 0,
    ];
    let mut sum = bytes[14..34]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| u16::from_be_bytes([p[0], p[1]]) as u32)
        .sum::<u32>();
    while sum > 0xffff {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    let checksum = (!(sum as u16)).to_be_bytes();
    bytes[24..26].copy_from_slice(&checksum);
    bytes.extend_from_slice(b"EXMON030");
    Observation::new(
        0,
        Direction::Rx,
        Metadata::Ethernet { link_type: 1 },
        Some(bytes.len()),
        &bytes,
    )
    .expect("valid bounded fixture")
}
