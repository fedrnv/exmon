//! Deterministic bounded worker-side classification. No capture, history, or TX authority.
use crate::framing::Message;
use exmon_domain::{
    Direction,
    classification::MetadataMatch,
    config::{Classification, ProjectConfig},
    observation::{MAX_PAYLOAD, Metadata, Observation, TransportKind},
};
use std::{collections::BTreeMap, fmt};
pub const MAX_PATTERN_BYTES: usize = 1_048_576;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Configuration,
    PatternBudget,
    Input,
    NeedsFraming,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "classification: {self:?}")
    }
}
impl std::error::Error for Error {}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quality {
    Complete,
    Unknown,
    Incomplete,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Matched,
    Ambiguous,
    Unmatched,
    Incomplete,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Predicate {
    IncompleteInput,
    Transport,
    Direction,
    Length,
    HeaderMissing,
    HeaderMismatch,
}
#[derive(Clone, PartialEq, Eq)]
pub enum GroupId {
    Rule(String),
    Auto(String),
}
impl fmt::Debug for GroupId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("GroupId([REDACTED])")
    }
}
#[derive(Clone, PartialEq, Eq)]
pub struct RuleCheck {
    pub id: String,
    pub priority: i32,
    pub failed: Option<Predicate>,
    pub selected: bool,
    pub criteria: u8,
}
impl RuleCheck {
    pub fn explanation(&self) -> &'static str {
        match self.failed {
            Some(Predicate::IncompleteInput) => {
                "Incomplete capture: no rule may claim a complete match."
            }
            Some(Predicate::Transport) => "Transport metadata does not satisfy the rule.",
            Some(Predicate::Direction) => "Direction does not satisfy the rule.",
            Some(Predicate::Length) => "Message length does not satisfy the rule.",
            Some(Predicate::HeaderMissing) => {
                "Message is too short for the configured header range."
            }
            Some(Predicate::HeaderMismatch) => {
                "Header bytes do not satisfy the configured prefix or mask."
            }
            None if self.selected => {
                "All configured predicates match at the highest matching priority."
            }
            None => "All configured predicates match, but a higher-priority rule wins.",
        }
    }
}
impl fmt::Debug for RuleCheck {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuleCheck")
            .field("priority", &self.priority)
            .field("failed", &self.failed)
            .field("selected", &self.selected)
            .finish()
    }
}
#[derive(Clone, PartialEq, Eq)]
pub struct Decision {
    pub revision: u64,
    pub outcome: Outcome,
    pub quality: Quality,
    pub group: GroupId,
    pub name: String,
    pub contenders: Vec<String>,
    pub checks: Vec<RuleCheck>,
}
impl fmt::Debug for Decision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClassificationDecision")
            .field("revision", &self.revision)
            .field("outcome", &self.outcome)
            .field("quality", &self.quality)
            .field("rules", &self.checks.len())
            .finish()
    }
}
struct Rule {
    id: String,
    name: String,
    condition: Classification,
}
pub struct Classifier {
    revision: u64,
    rules: Vec<Rule>,
    header: u8,
    names: BTreeMap<String, String>,
    pattern_bytes: usize,
}
impl fmt::Debug for Classifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Classifier")
            .field("revision", &self.revision)
            .field("rules", &self.rules.len())
            .finish()
    }
}
impl Classifier {
    pub fn new(config: &ProjectConfig, revision: u64) -> Result<Self, Error> {
        config.validate().map_err(|_| Error::Configuration)?;
        let mut total = 0usize;
        for p in &config.protocols {
            total = total
                .checked_add(p.classification.prefix.len())
                .and_then(|n| n.checked_add(p.classification.mask.len()))
                .filter(|n| *n <= MAX_PATTERN_BYTES)
                .ok_or(Error::PatternBudget)?;
        }
        let mut rules: Vec<_> = config
            .protocols
            .iter()
            .map(|p| Rule {
                id: p.id.clone(),
                name: p.name.clone(),
                condition: p.classification.clone(),
            })
            .collect();
        rules.sort_by(|a, b| {
            b.condition
                .priority
                .cmp(&a.condition.priority)
                .then(a.id.cmp(&b.id))
        });
        Ok(Self {
            revision,
            rules,
            header: config.grouping.header_bytes,
            names: config
                .grouping
                .names
                .iter()
                .map(|n| (n.id.clone(), n.name.clone()))
                .collect(),
            pattern_bytes: total,
        })
    }
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }
    pub fn pattern_bytes(&self) -> usize {
        self.pattern_bytes
    }
    /// Explicit inspection of the exact immutable rule used by this revision.
    pub fn condition(&self, id: &str) -> Option<&Classification> {
        self.rules.iter().find(|r| r.id == id).map(|r| &r.condition)
    }
    pub fn classify_message(
        &self,
        message: &Message,
        metadata: &Metadata,
        direction: Direction,
        quality: Quality,
    ) -> Result<Decision, Error> {
        self.classify(metadata, direction, message.bytes(), quality)
    }
    /// Native records only; arbitrary TCP/serial read chunks must pass EXM019 framing first.
    pub fn classify_observation(&self, o: &Observation) -> Result<Decision, Error> {
        if matches!(
            o.metadata().kind(),
            TransportKind::Tcp | TransportKind::Serial
        ) {
            return Err(Error::NeedsFraming);
        }
        let quality = if o.original_length().is_some_and(|n| n > o.bytes().len())
            || matches!(
                o.metadata(),
                Metadata::Usb {
                    completed: false,
                    ..
                }
            ) {
            Quality::Incomplete
        } else if o.original_length().is_none() {
            Quality::Unknown
        } else {
            Quality::Complete
        };
        self.classify(o.metadata(), o.direction(), o.bytes(), quality)
    }
    pub fn classify(
        &self,
        metadata: &Metadata,
        direction: Direction,
        bytes: &[u8],
        quality: Quality,
    ) -> Result<Decision, Error> {
        if bytes.len() > MAX_PAYLOAD
            || Observation::new(0, direction, metadata.clone(), Some(0), &[]).is_err()
        {
            return Err(Error::Input);
        }
        let mut checks = Vec::with_capacity(self.rules.len());
        let mut best = None;
        let mut contenders = vec![];
        for rule in &self.rules {
            let c = &rule.condition;
            let failed = if quality == Quality::Incomplete {
                Some(Predicate::IncompleteInput)
            } else {
                failure(c, metadata, direction, bytes)
            };
            let selected = failed.is_none() && best.is_none_or(|p| p == c.priority);
            if selected {
                best = Some(c.priority);
                contenders.push(rule.id.clone());
            }
            let criteria = u8::from(c.transport.is_some())
                | u8::from(c.direction.is_some()) << 1
                | u8::from(c.length.is_some()) << 2
                | u8::from(!c.prefix.is_empty()) << 3
                | u8::from(!c.mask.is_empty()) << 4;
            checks.push(RuleCheck {
                id: rule.id.clone(),
                priority: c.priority,
                failed,
                selected,
                criteria,
            });
        }
        let outcome = if quality == Quality::Incomplete {
            Outcome::Incomplete
        } else {
            match contenders.len() {
                0 => Outcome::Unmatched,
                1 => Outcome::Matched,
                _ => Outcome::Ambiguous,
            }
        };
        let (group, name) = if outcome == Outcome::Matched {
            let id = &contenders[0];
            let rule = self
                .rules
                .iter()
                .find(|r| &r.id == id)
                .expect("matching rule retained");
            (GroupId::Rule(id.clone()), rule.name.clone())
        } else {
            let (id, label) = automatic(metadata, direction, bytes, outcome, self.header);
            let name = self.names.get(&id).cloned().unwrap_or_else(|| {
                format!(
                    "{} {label}; {} bytes; {}",
                    match outcome {
                        Outcome::Ambiguous => "Ambiguous",
                        Outcome::Incomplete => "Incomplete",
                        _ => "Unmatched",
                    },
                    bytes.len(),
                    hex(&bytes[..bytes.len().min(self.header as usize)])
                )
            });
            (GroupId::Auto(id), name)
        };
        Ok(Decision {
            revision: self.revision,
            outcome,
            quality,
            group,
            name,
            contenders,
            checks,
        })
    }
}
fn failure(c: &Classification, m: &Metadata, d: Direction, bytes: &[u8]) -> Option<Predicate> {
    if c.transport
        .as_ref()
        .is_some_and(|s| !metadata_matches(s, m))
    {
        return Some(Predicate::Transport);
    }
    if c.direction.is_some_and(|value| value != d) {
        return Some(Predicate::Direction);
    }
    if c.length.is_some_and(|n| n as usize != bytes.len()) {
        return Some(Predicate::Length);
    }
    if !c.prefix.is_empty() {
        let start = c.offset as usize;
        let Some(actual) = bytes.get(start..start + c.prefix.len()) else {
            return Some(Predicate::HeaderMissing);
        };
        if actual.iter().zip(&c.prefix).enumerate().any(|(i, (a, b))| {
            let mask = c.mask.get(i).copied().unwrap_or(0xff);
            a & mask != b & mask
        }) {
            return Some(Predicate::HeaderMismatch);
        }
    }
    None
}
fn is<T: PartialEq>(selector: &Option<T>, actual: T) -> bool {
    selector.as_ref().is_none_or(|s| *s == actual)
}
fn metadata_matches(s: &MetadataMatch, m: &Metadata) -> bool {
    match (s, m) {
        (MetadataMatch::Ethernet { link_type }, Metadata::Ethernet { link_type: a }) => {
            is(link_type, *a)
        }
        (
            MetadataMatch::Tcp {
                local,
                remote,
                local_port,
                remote_port,
            },
            Metadata::Tcp {
                local: a,
                remote: b,
                ..
            },
        )
        | (
            MetadataMatch::Udp {
                local,
                remote,
                local_port,
                remote_port,
            },
            Metadata::Udp {
                local: a,
                remote: b,
            },
        ) => {
            is(local, *a) && is(remote, *b) && is(local_port, a.port()) && is(remote_port, b.port())
        }
        (
            MetadataMatch::Serial {
                configuration_revision,
            },
            Metadata::Serial {
                configuration_revision: a,
            },
        ) => is(configuration_revision, *a),
        (MetadataMatch::Usb { endpoint }, Metadata::Usb { endpoint: a, .. }) => is(endpoint, *a),
        (
            MetadataMatch::Hid {
                vendor_id,
                product_id,
                usage_page,
                usage,
                report_id,
            },
            Metadata::Hid {
                vendor_id: a,
                product_id: b,
                usage_page: c,
                usage: d,
                report_id: e,
            },
        ) => {
            is(vendor_id, *a)
                && is(product_id, *b)
                && is(usage_page, *c)
                && is(usage, *d)
                && is(report_id, *e)
        }
        (
            MetadataMatch::UsbMon {
                event,
                transfer,
                endpoint,
                bus,
                device,
            },
            Metadata::UsbMon(a),
        ) => {
            is(event, a.event)
                && is(transfer, a.transfer)
                && is(endpoint, a.endpoint)
                && is(bus, a.bus)
                && is(device, a.device)
        }
        (
            MetadataMatch::Can {
                channel,
                id,
                extended,
                fd,
                remote,
                error,
                dlc,
            },
            Metadata::Can {
                channel: a,
                id: b,
                extended: c,
                fd: d,
                remote: e,
                error: f,
                dlc: g,
            },
        ) => {
            is(channel, *a)
                && is(id, *b)
                && is(extended, *c)
                && is(fd, *d)
                && is(remote, *e)
                && is(error, *f)
                && is(dlc, *g)
        }
        _ => false,
    }
}
fn be16(out: &mut Vec<u8>, n: u16) {
    out.extend_from_slice(&n.to_be_bytes());
}
fn be32(out: &mut Vec<u8>, n: u32) {
    out.extend_from_slice(&n.to_be_bytes());
}
fn automatic(
    m: &Metadata,
    d: Direction,
    bytes: &[u8],
    outcome: Outcome,
    header: u8,
) -> (String, String) {
    let mut key = vec![
        match outcome {
            Outcome::Ambiguous => 1,
            Outcome::Incomplete => 2,
            _ => 0,
        },
        match d {
            Direction::Rx => 0,
            Direction::Tx => 1,
            Direction::Unknown => 2,
        },
    ];
    let label = match m {
        Metadata::Ethernet { link_type } => {
            key.push(0);
            be32(&mut key, *link_type);
            format!("Ethernet/link{link_type}")
        }
        Metadata::Tcp { local, remote, .. } | Metadata::Udp { local, remote } => {
            let tcp = matches!(m, Metadata::Tcp { .. });
            key.push(if tcp { 1 } else { 2 });
            be16(&mut key, local.port());
            be16(&mut key, remote.port());
            format!(
                "{} {}:{}",
                if tcp { "TCP" } else { "UDP" },
                local.port(),
                remote.port()
            )
        }
        Metadata::Serial { .. } => {
            key.push(3);
            "Serial".to_owned()
        }
        Metadata::Usb { endpoint, .. } => {
            key.extend([4, *endpoint]);
            format!("USB endpoint{endpoint:02X}")
        }
        Metadata::Hid {
            vendor_id,
            product_id,
            usage_page,
            usage,
            report_id,
        } => {
            key.push(5);
            for n in [vendor_id, product_id, usage_page, usage] {
                be16(&mut key, *n);
            }
            key.push(*report_id);
            format!(
                "HID {vendor_id:04X}:{product_id:04X}/{usage_page:04X}:{usage:04X}/report{report_id}"
            )
        }
        Metadata::UsbMon(a) => {
            key.extend([6, a.event, a.transfer, a.endpoint]);
            format!("USB trace {}/{}/ep{:02X}", a.event, a.transfer, a.endpoint)
        }
        Metadata::Can {
            channel,
            id,
            extended,
            fd,
            remote,
            error,
            dlc,
        } => {
            key.push(7);
            be32(&mut key, *channel);
            be32(&mut key, *id);
            key.push(
                u8::from(*extended)
                    | u8::from(*fd) << 1
                    | u8::from(*remote) << 2
                    | u8::from(*error) << 3,
            );
            key.push(*dlc);
            format!(
                "CAN ch{channel}/id{id:X}/flags{:X}/dlc{dlc}",
                key[key.len() - 2]
            )
        }
    };
    be32(&mut key, bytes.len() as u32);
    let len = bytes.len().min(header as usize);
    key.push(len as u8);
    key.extend_from_slice(&bytes[..len]);
    (format!("auto-v1-{}", hex(&key)), label)
}
fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(DIGITS[(b >> 4) as usize] as char);
        out.push(DIGITS[(b & 15) as usize] as char);
    }
    out
}
