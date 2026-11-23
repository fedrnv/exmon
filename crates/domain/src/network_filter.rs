//! Ephemeral network capture selection, independent of native filter syntax.
use crate::Direction;
use std::{fmt, net::IpAddr};

pub const MAX_FILTER_PORTS: usize = 32;
#[derive(Clone, Default, PartialEq, Eq)]
pub struct NetworkFilter {
    pub interface: Option<u64>,
    pub direction: Option<Direction>,
    /// IP protocol / IPv6 upper-layer next-header number, not an EtherType.
    pub protocol: Option<u8>,
    pub source: Option<IpAddr>,
    pub destination: Option<IpAddr>,
    pub host: Option<IpAddr>,
    /// OR across these ports and either endpoint; other predicates are ANDed.
    pub ports: Vec<u16>,
}
impl fmt::Debug for NetworkFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("NetworkFilter([REDACTED])")
    }
}
impl NetworkFilter {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.ports.len() > MAX_FILTER_PORTS {
            return Err("At most 32 ports are allowed.");
        }
        let addresses: Vec<_> = [self.source, self.destination, self.host]
            .into_iter()
            .flatten()
            .collect();
        if addresses
            .windows(2)
            .any(|pair| pair[0].is_ipv4() != pair[1].is_ipv4())
        {
            return Err("Filter addresses must use the same IP family.");
        }
        Ok(())
    }
    pub fn has_network_predicates(&self) -> bool {
        self.protocol.is_some()
            || self.source.is_some()
            || self.destination.is_some()
            || self.host.is_some()
            || !self.ports.is_empty()
    }
}
