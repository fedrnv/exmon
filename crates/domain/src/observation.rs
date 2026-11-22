//! Validated immutable observations; transport handles never enter this layer.
use crate::{Direction, validate_lengths};
use std::{fmt, net::SocketAddr};

pub const MAX_PAYLOAD: usize = 65_536;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportKind {
    Ethernet,
    Tcp,
    Udp,
    Serial,
    Usb,
    Can,
}
#[derive(Clone, PartialEq, Eq)]
pub enum Metadata {
    Ethernet {
        link_type: u32,
    },
    Tcp {
        local: SocketAddr,
        remote: SocketAddr,
        connection: u64,
    },
    Udp {
        local: SocketAddr,
        remote: SocketAddr,
    },
    Serial {
        configuration_revision: u64,
    },
    Usb {
        endpoint: u8,
        completed: bool,
    },
    Can {
        id: u32,
        extended: bool,
        fd: bool,
    },
}
impl Metadata {
    pub fn kind(&self) -> TransportKind {
        match self {
            Self::Ethernet { .. } => TransportKind::Ethernet,
            Self::Tcp { .. } => TransportKind::Tcp,
            Self::Udp { .. } => TransportKind::Udp,
            Self::Serial { .. } => TransportKind::Serial,
            Self::Usb { .. } => TransportKind::Usb,
            Self::Can { .. } => TransportKind::Can,
        }
    }
}
impl fmt::Debug for Metadata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}([REDACTED])", self.kind())
    }
}

#[derive(Clone)]
pub struct Observation {
    timestamp_ns: u64,
    source_timestamp_ns: Option<u64>,
    direction: Direction,
    metadata: Metadata,
    original_length: Option<usize>,
    bytes: Box<[u8]>,
}
impl fmt::Debug for Observation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Observation")
            .field("timestamp_ns", &self.timestamp_ns)
            .field("direction", &self.direction)
            .field("metadata", &self.metadata)
            .field("bytes", &"[REDACTED]")
            .finish()
    }
}
impl Observation {
    /// Preserve capture clock separately; it must never drive monotonic schedules.
    pub fn with_source_timestamp(mut self, timestamp_ns: u64) -> Self {
        self.source_timestamp_ns = Some(timestamp_ns);
        self
    }
    pub fn source_timestamp_ns(&self) -> Option<u64> {
        self.source_timestamp_ns
    }
    pub fn timestamp_ns(&self) -> u64 {
        self.timestamp_ns
    }
    pub fn direction(&self) -> Direction {
        self.direction
    }
    /// Bound and validate before copying untrusted bytes. Timestamps are relative
    /// monotonic nanoseconds within the adapter epoch, not assumed UTC.
    pub fn new(
        timestamp_ns: u64,
        direction: Direction,
        metadata: Metadata,
        original_length: Option<usize>,
        bytes: &[u8],
    ) -> Result<Self, &'static str> {
        if bytes.len() > MAX_PAYLOAD {
            return Err("payload exceeds 65536 bytes");
        }
        validate_lengths(bytes.len(), original_length).map_err(|_| "invalid original length")?;
        match &metadata {
            Metadata::Usb { endpoint, .. } if endpoint & 0x70 != 0 => {
                return Err("invalid USB endpoint");
            }
            Metadata::Can { id, extended, fd }
                if *id > (if *extended { 0x1fff_ffff } else { 0x7ff })
                    || bytes.len() > (if *fd { 64 } else { 8 }) =>
            {
                return Err("invalid CAN identifier or data length");
            }
            _ => (),
        }
        Ok(Self {
            timestamp_ns,
            source_timestamp_ns: None,
            direction,
            metadata,
            original_length,
            bytes: bytes.into(),
        })
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn metadata(&self) -> &Metadata {
        &self.metadata
    }
    pub fn original_length(&self) -> Option<usize> {
        self.original_length
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounds_metadata_and_privacy() {
        let meta = Metadata::Can {
            id: 0x123,
            extended: false,
            fd: false,
        };
        assert!(Observation::new(1, Direction::Rx, meta.clone(), Some(1), &[1, 2]).is_err());
        assert!(Observation::new(1, Direction::Rx, meta.clone(), None, &[0; 9]).is_err());
        let obs = Observation::new(1, Direction::Rx, meta, Some(4), b"abc").unwrap();
        assert_eq!(obs.original_length(), Some(4));
        assert_eq!(obs.bytes(), b"abc");
        assert!(!format!("{obs:?}").contains("abc"));
        assert!(
            Observation::new(
                0,
                Direction::Unknown,
                Metadata::Serial {
                    configuration_revision: 1
                },
                None,
                &vec![0; MAX_PAYLOAD + 1]
            )
            .is_err()
        );
    }
}
