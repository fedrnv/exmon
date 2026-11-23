//! Portable classification selectors and explicitly named automatic groups.
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, fmt, net::SocketAddr};
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum MetadataMatch {
    Ethernet {
        link_type: Option<u32>,
    },
    Tcp {
        local: Option<SocketAddr>,
        remote: Option<SocketAddr>,
        local_port: Option<u16>,
        remote_port: Option<u16>,
    },
    Udp {
        local: Option<SocketAddr>,
        remote: Option<SocketAddr>,
        local_port: Option<u16>,
        remote_port: Option<u16>,
    },
    Serial {
        configuration_revision: Option<u64>,
    },
    Usb {
        endpoint: Option<u8>,
    },
    Hid {
        vendor_id: Option<u16>,
        product_id: Option<u16>,
        usage_page: Option<u16>,
        usage: Option<u16>,
        report_id: Option<u8>,
    },
    #[serde(rename = "usbmon")]
    UsbMon {
        event: Option<u8>,
        transfer: Option<u8>,
        endpoint: Option<u8>,
        bus: Option<u16>,
        device: Option<u8>,
    },
    Can {
        channel: Option<u32>,
        id: Option<u32>,
        extended: Option<bool>,
        fd: Option<bool>,
        remote: Option<bool>,
        error: Option<bool>,
        dlc: Option<u8>,
    },
}
impl fmt::Debug for MetadataMatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("MetadataMatch([REDACTED])")
    }
}
impl MetadataMatch {
    pub fn validate(&self) -> bool {
        match self {
            Self::Usb {
                endpoint: Some(endpoint),
            } => endpoint & 0x70 == 0,
            Self::Can {
                id,
                extended,
                fd,
                remote,
                error,
                dlc,
                ..
            } => {
                id.is_none_or(|id| {
                    id <= if *extended == Some(false) && *error == Some(false) {
                        0x7ff
                    } else {
                        0x1fffffff
                    }
                }) && dlc.is_none_or(|n| n <= 15)
                    && !(*remote == Some(true) && (*fd == Some(true) || *error == Some(true)))
            }
            Self::Tcp {
                local,
                remote,
                local_port,
                remote_port,
            }
            | Self::Udp {
                local,
                remote,
                local_port,
                remote_port,
            } => {
                local.is_none_or(|a| local_port.is_none_or(|p| a.port() == p))
                    && remote.is_none_or(|a| remote_port.is_none_or(|p| a.port() == p))
                    && local
                        .zip(*remote)
                        .is_none_or(|(a, b)| a.is_ipv4() == b.is_ipv4())
            }
            _ => true,
        }
    }
}
pub const DEFAULT_HEADER_BYTES: u8 = 4;
fn header_default() -> u8 {
    DEFAULT_HEADER_BYTES
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupName {
    pub id: String,
    pub name: String,
}
impl fmt::Debug for GroupName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("GroupName([REDACTED])")
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Grouping {
    #[serde(default = "header_default")]
    pub header_bytes: u8,
    #[serde(default)]
    pub names: Vec<GroupName>,
}
impl Default for Grouping {
    fn default() -> Self {
        Self {
            header_bytes: DEFAULT_HEADER_BYTES,
            names: vec![],
        }
    }
}
impl Grouping {
    pub fn is_default(&self) -> bool {
        self == &Self::default()
    }
    pub fn validate(&self) -> bool {
        if self.header_bytes > 16 || self.names.len() > 256 {
            return false;
        }
        let mut seen = BTreeSet::new();
        self.names
            .iter()
            .all(|n| valid_auto_id(&n.id) && valid_name(&n.name) && seen.insert(&n.id))
    }
}
pub fn valid_name(name: &str) -> bool {
    !name.is_empty() && name.len() <= 128 && !name.chars().any(char::is_control)
}
/// Canonical v1 key: state,direction,kind,typed signature,length(be32),header length,header.
pub fn valid_auto_id(id: &str) -> bool {
    let Some(hex) = id.strip_prefix("auto-v1-") else {
        return false;
    };
    if hex.len() > 120 || !hex.len().is_multiple_of(2) {
        return false;
    }
    let mut b = [0u8; 60];
    let n = hex.len() / 2;
    for (index, pair) in hex.as_bytes().as_chunks::<2>().0.iter().enumerate() {
        let digit = |c| match c {
            b'0'..=b'9' => Some(c - b'0'),
            b'a'..=b'f' => Some(c - b'a' + 10),
            _ => None,
        };
        let (Some(a), Some(c)) = (digit(pair[0]), digit(pair[1])) else {
            return false;
        };
        b[index] = (a << 4) | c;
    }
    if n < 8 || b[0] > 2 || b[1] > 2 {
        return false;
    }
    let signature = match b[2] {
        0 => 4,
        1 | 2 => 4,
        3 => 0,
        4 => 1,
        5 => 9,
        6 => 3,
        7 => 10,
        _ => return false,
    };
    let at = 3 + signature;
    if n < at + 5 {
        return false;
    }
    let length = u32::from_be_bytes(b[at..at + 4].try_into().expect("checked key field"));
    let header = usize::from(b[at + 4]);
    if length > 65536 || header > 16 || header > length as usize || n != at + 5 + header {
        return false;
    }
    if b[2] == 4 && b[3] & 0x70 != 0 {
        return false;
    }
    if b[2] == 7 {
        let flags = b[11];
        let dlc = b[12];
        let id = u32::from_be_bytes(b[7..11].try_into().unwrap());
        if flags > 15
            || dlc > 15
            || flags & 4 != 0 && flags & 10 != 0
            || id > if flags & 9 != 0 { 0x1fffffff } else { 0x7ff }
        {
            return false;
        }
    }
    true
}
