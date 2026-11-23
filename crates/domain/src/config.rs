//! Portable configuration contract. Loading never authorizes physical I/O.
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const SCHEMA_VERSION: u32 = 1;
pub const MAX_ITEMS: usize = 256;
pub const MAX_BYTES: u32 = 65_536;

/// Fixed diagnostics deliberately exclude untrusted identifiers and payloads.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{section}: {reason}")]
pub struct ConfigError {
    pub section: &'static str,
    pub reason: &'static str,
}
fn check(ok: bool, section: &'static str, reason: &'static str) -> Result<(), ConfigError> {
    if ok {
        Ok(())
    } else {
        Err(ConfigError { section, reason })
    }
}
fn text(value: &str) -> bool {
    !value.is_empty() && value.len() <= 128 && !value.chars().any(char::is_control)
}
fn id(value: &str) -> bool {
    text(value)
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c))
}
fn ids<'a>(
    values: impl Iterator<Item = &'a str>,
    section: &'static str,
) -> Result<(), ConfigError> {
    let mut seen = BTreeSet::new();
    for value in values {
        check(
            seen.len() < MAX_ITEMS && id(value) && seen.insert(value),
            section,
            "use at most 256 unique nonempty ASCII identifiers (maximum 128 bytes)",
        )?;
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectConfig {
    pub schema_version: u32,
    #[serde(default)]
    pub protocols: Vec<Protocol>,
    #[serde(default)]
    pub interfaces: Vec<Interface>,
    #[serde(default)]
    pub charts: Vec<Chart>,
    #[serde(default)]
    pub templates: Vec<Template>,
    #[serde(default)]
    pub schedules: Vec<Schedule>,
}
impl Default for ProjectConfig {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            protocols: vec![],
            interfaces: vec![],
            charts: vec![],
            templates: vec![],
            schedules: vec![],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Protocol {
    pub id: String,
    pub name: String,
    pub framing: Framing,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub escape: Option<Escape>,
    #[serde(default)]
    pub classification: Classification,
    #[serde(default)]
    pub fields: Vec<Field>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Framing {
    Datagram {},
    Fixed {
        length: u32,
    },
    Delimiter {
        bytes: Vec<u8>,
        max_length: u32,
    },
    LengthPrefix {
        offset: u32,
        width: u8,
        endian: Endian,
        max_length: u32,
    },
    Markers {
        start: Vec<u8>,
        end: Vec<u8>,
        max_length: u32,
    },
    IdleGap {
        gap_ns: u64,
        max_length: u32,
    },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Escape {
    pub byte: u8,
    pub xor: u8,
}

impl Framing {
    /// Shared validation for stored configuration and incremental framing entry points.
    pub fn limit(&self, escape: Option<Escape>) -> Result<u32, ConfigError> {
        let control = |b: &[u8]| {
            !b.is_empty() && b.len() <= 16 && escape.is_none_or(|e| !b.contains(&e.byte))
        };
        let limit = match self {
            Self::Datagram {} => MAX_BYTES,
            Self::Fixed { length } => *length,
            Self::Delimiter { bytes, max_length } => {
                check(
                    control(bytes) && bytes.len() <= *max_length as usize,
                    "framing.delimiter",
                    "use 1..16 unescaped delimiter bytes within frame limit",
                )?;
                *max_length
            }
            Self::LengthPrefix {
                offset,
                width,
                max_length,
                ..
            } => {
                check(
                    [1, 2, 4].contains(width)
                        && offset
                            .checked_add(u32::from(*width))
                            .is_some_and(|end| end <= *max_length),
                    "framing.length_prefix",
                    "use width 1, 2 or 4 and header within frame limit",
                )?;
                *max_length
            }
            Self::Markers {
                start,
                end,
                max_length,
            } => {
                check(
                    control(start)
                        && control(end)
                        && start.len() + end.len() <= *max_length as usize,
                    "framing.markers",
                    "use 1..16 unescaped marker bytes within frame limit",
                )?;
                *max_length
            }
            Self::IdleGap { gap_ns, max_length } => {
                check(
                    *gap_ns > 0,
                    "framing.idle_gap",
                    "use a positive monotonic gap",
                )?;
                *max_length
            }
        };
        check(
            (1..=MAX_BYTES).contains(&limit),
            "framing",
            "frame limit must be 1..65536 bytes",
        )?;
        Ok(limit)
    }
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Classification {
    #[serde(default)]
    pub length: Option<u32>,
    #[serde(default)]
    pub prefix: Vec<u8>,
}
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Endian {
    Little,
    Big,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Field {
    pub id: String,
    pub offset: u32,
    pub length: u32,
    pub representation: Representation,
    pub endian: Endian,
    #[serde(default = "one")]
    pub scale: f64,
    #[serde(default)]
    pub bias: f64,
    #[serde(default)]
    pub unit: String,
}
fn one() -> f64 {
    1.0
}
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Representation {
    Unsigned,
    Signed,
    Float,
    Bytes,
    Ascii,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Interface {
    pub id: String,
    pub protocol_id: String,
    pub transport: Transport,
    #[serde(default)]
    pub filter: Filter,
    #[serde(default)]
    pub tx_allowed: bool,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Transport {
    Ethernet {},
    Tcp {},
    Udp {},
    Serial {
        baud: u32,
        data_bits: u8,
        stop_bits: u8,
        parity: Parity,
    },
    Usb {
        endpoint: u8,
        transfer: UsbTransfer,
    },
    Can {
        bitrate: u32,
        fd: bool,
    },
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Parity {
    None,
    Odd,
    Even,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsbTransfer {
    Hid,
    Bulk,
    Interrupt,
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Filter {
    #[serde(default)]
    pub direction: Option<super::Direction>,
    #[serde(default)]
    pub ports: Vec<u16>,
    #[serde(default)]
    pub can_ids: Vec<u32>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Chart {
    pub id: String,
    pub protocol_id: String,
    pub field_id: String,
    pub samples: u32,
    #[serde(default)]
    pub moving_average: Option<u32>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Template {
    pub id: String,
    pub interface_id: String,
    pub bytes: Vec<u8>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Schedule {
    pub id: String,
    pub template_id: String,
    pub interval_ms: u32,
    #[serde(default)]
    pub enabled: bool,
}

/// Local selectors are intentionally absent from ProjectConfig and Protocol.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MachineBindings {
    pub schema_version: u32,
    #[serde(default)]
    pub bindings: Vec<DeviceBinding>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceBinding {
    pub interface_id: String,
    pub selector: String,
}

impl Protocol {
    fn validate(&self) -> Result<(), ConfigError> {
        check(
            text(&self.name),
            "protocol.name",
            "use 1..128 bytes without control characters",
        )?;
        let limit = self.framing.limit(self.escape)?;
        check(
            self.classification.prefix.len() <= limit as usize
                && self.classification.length.is_none_or(|n| {
                    n > 0 && n <= limit && n as usize >= self.classification.prefix.len()
                }),
            "classification",
            "prefix and optional positive length must fit frame limit",
        )?;
        ids(self.fields.iter().map(|f| f.id.as_str()), "fields")?;
        for f in &self.fields {
            check(
                f.length > 0
                    && f.offset
                        .checked_add(f.length)
                        .is_some_and(|end| end <= limit),
                "field.range",
                "positive field range must fit frame limit without overflow",
            )?;
            check(
                match f.representation {
                    Representation::Unsigned | Representation::Signed => {
                        [1, 2, 4, 8].contains(&f.length)
                    }
                    Representation::Float => [4, 8].contains(&f.length),
                    _ => true,
                },
                "field.representation",
                "numeric widths must be 1/2/4/8; floats require 4/8 bytes",
            )?;
            check(
                f.scale.is_finite()
                    && f.bias.is_finite()
                    && f.unit.len() <= 32
                    && !f.unit.chars().any(char::is_control),
                "field.transform",
                "use finite scale/bias and a unit of at most 32 bytes",
            )?;
        }
        Ok(())
    }
}

impl ProjectConfig {
    pub fn validate(&self) -> Result<(), ConfigError> {
        check(
            self.schema_version == SCHEMA_VERSION,
            "schema_version",
            "unsupported version; explicit migration required",
        )?;
        ids(self.protocols.iter().map(|p| p.id.as_str()), "protocols")?;
        ids(self.interfaces.iter().map(|i| i.id.as_str()), "interfaces")?;
        ids(self.charts.iter().map(|c| c.id.as_str()), "charts")?;
        ids(self.templates.iter().map(|t| t.id.as_str()), "templates")?;
        ids(self.schedules.iter().map(|s| s.id.as_str()), "schedules")?;
        for p in &self.protocols {
            p.validate()?;
        }
        for i in &self.interfaces {
            let protocol = self.protocols.iter().find(|p| p.id == i.protocol_id);
            check(
                protocol.is_some(),
                "interface.protocol_id",
                "reference an existing protocol",
            )?;
            check(
                i.filter.ports.len() <= MAX_ITEMS
                    && i.filter.can_ids.len() <= MAX_ITEMS
                    && i.filter.can_ids.iter().all(|n| *n <= 0x1fff_ffff),
                "filter",
                "limit lists to 256 and CAN ids to 29 bits",
            )?;
            check(
                i.filter.ports.is_empty()
                    || matches!(i.transport, Transport::Tcp {} | Transport::Udp {}),
                "filter.ports",
                "port filters require TCP or UDP",
            )?;
            check(
                i.filter.can_ids.is_empty() || matches!(i.transport, Transport::Can { .. }),
                "filter.can_ids",
                "CAN filters require a CAN interface",
            )?;
            match i.transport {
                Transport::Serial {
                    baud,
                    data_bits,
                    stop_bits,
                    ..
                } => check(
                    baud > 0
                        && baud <= 4_000_000
                        && (5..=8).contains(&data_bits)
                        && (1..=2).contains(&stop_bits),
                    "transport.serial",
                    "invalid baud, data bits or stop bits",
                )?,
                Transport::Usb { endpoint, .. } => check(
                    endpoint & 0x70 == 0 && endpoint & 0x0f != 0,
                    "transport.usb",
                    "use a non-control endpoint with valid address bits",
                )?,
                Transport::Can { bitrate, .. } => check(
                    (1..=8_000_000).contains(&bitrate),
                    "transport.can",
                    "bitrate must be 1..8000000",
                )?,
                _ => (),
            }
            if matches!(i.transport, Transport::Tcp {} | Transport::Serial { .. }) {
                check(
                    !matches!(protocol.map(|p| &p.framing), Some(Framing::Datagram {})),
                    "interface.framing",
                    "stream interfaces require explicit framing",
                )?;
            }
        }
        for c in &self.charts {
            let field = self
                .protocols
                .iter()
                .find(|p| p.id == c.protocol_id)
                .and_then(|p| p.fields.iter().find(|f| f.id == c.field_id));
            check(
                field.is_some_and(|f| {
                    matches!(
                        f.representation,
                        Representation::Unsigned | Representation::Signed | Representation::Float
                    )
                }),
                "chart.field_id",
                "reference an existing numeric field",
            )?;
            check(
                (1..=MAX_BYTES).contains(&c.samples)
                    && c.moving_average.is_none_or(|n| n > 0 && n <= c.samples),
                "chart.window",
                "use 1..65536 samples and positive average no larger than window",
            )?;
        }
        for t in &self.templates {
            check(
                self.interfaces
                    .iter()
                    .any(|i| i.id == t.interface_id && i.tx_allowed),
                "template.interface_id",
                "reference an existing TX-permitted interface",
            )?;
            check(
                !t.bytes.is_empty() && t.bytes.len() <= MAX_BYTES as usize,
                "template.bytes",
                "use 1..65536 bytes",
            )?;
        }
        for s in &self.schedules {
            check(
                !s.enabled,
                "schedule.enabled",
                "persist disabled; enable only through explicit runtime action",
            )?;
            check(
                self.templates.iter().any(|t| t.id == s.template_id),
                "schedule.template_id",
                "reference an existing template",
            )?;
            check(
                (1..=86_400_000).contains(&s.interval_ms),
                "schedule.interval_ms",
                "interval must be 1..86400000 milliseconds",
            )?;
        }
        Ok(())
    }
}

impl MachineBindings {
    pub fn validate(&self, project: &ProjectConfig) -> Result<(), ConfigError> {
        project.validate()?;
        check(
            self.schema_version == SCHEMA_VERSION,
            "schema_version",
            "unsupported version; explicit migration required",
        )?;
        ids(
            self.bindings.iter().map(|b| b.interface_id.as_str()),
            "bindings",
        )?;
        for b in &self.bindings {
            check(
                project.interfaces.iter().any(|i| i.id == b.interface_id),
                "binding.interface_id",
                "reference an existing interface",
            )?;
            check(
                text(&b.selector),
                "binding.selector",
                "use 1..128 bytes without control characters",
            )?;
        }
        Ok(())
    }
}
