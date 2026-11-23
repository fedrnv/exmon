//! One immutable plan for exact post-capture matching and conservative BPF pushdown.
use crate::network::{Decoder, Ip, Transport};
use exmon_domain::{
    Direction,
    network_filter::NetworkFilter,
    observation::{Metadata, Observation},
};
use std::{fmt, net::IpAddr};

#[derive(Clone)]
pub struct FilterPlan {
    settings: NetworkFilter,
    bpf: String,
}
impl fmt::Debug for FilterPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("FilterPlan([REDACTED])")
    }
}
impl FilterPlan {
    pub fn new(mut settings: NetworkFilter) -> Result<Self, &'static str> {
        settings.validate()?;
        settings.ports.sort_unstable();
        settings.ports.dedup();
        let bpf = prefilter(&settings);
        Ok(Self { settings, bpf })
    }
    pub fn settings(&self) -> &NetworkFilter {
        &self.settings
    }
    /// An optimization only: MUST be followed by matches(). Empty means all frames.
    pub fn bpf(&self) -> &str {
        &self.bpf
    }
    /// Intended for local UI, never diagnostic logs (contains selected endpoints).
    pub fn summary(&self) -> String {
        let s = &self.settings;
        let mut parts = vec![
            match s.interface {
                Some(i) => format!("interface {i}"),
                None => "all interfaces".into(),
            },
            format!(
                "direction {}",
                match s.direction {
                    None => "any",
                    Some(Direction::Rx) => "RX",
                    Some(Direction::Tx) => "TX",
                    Some(Direction::Unknown) => "unknown",
                }
            ),
        ];
        if let Some(protocol) = s.protocol {
            parts.push(format!("IP protocol {protocol}"));
        }
        for (label, address) in [
            ("source", s.source),
            ("destination", s.destination),
            ("host", s.host),
        ] {
            if let Some(address) = address {
                parts.push(format!("{label} {address}"));
            }
        }
        if !s.ports.is_empty() {
            parts.push(format!(
                "either port {}",
                s.ports
                    .iter()
                    .map(u16::to_string)
                    .collect::<Vec<_>>()
                    .join(",")
            ));
        }
        parts.join("; ")
    }
    pub fn matches(&self, interface: u64, observation: &Observation) -> bool {
        let s = &self.settings;
        if s.interface.is_some_and(|i| i != interface)
            || s.direction.is_some_and(|d| d != observation.direction())
        {
            return false;
        }
        if !s.has_network_predicates() {
            return true;
        }
        let (source, destination, hosts, ports, protocol) = match observation.metadata() {
            Metadata::Ethernet { link_type: 1 } => {
                let packet = Decoder::default().decode_observation(observation);
                let (source, destination, protocol) = match packet.ip {
                    Some(Ip::V4(ip)) => (
                        IpAddr::V4(ip.source),
                        IpAddr::V4(ip.destination),
                        Some(ip.protocol),
                    ),
                    Some(Ip::V6(ip)) => (IpAddr::V6(ip.source), IpAddr::V6(ip.destination), {
                        let next = packet
                            .extensions
                            .last()
                            .map_or(ip.next_header, |e| e.next_header);
                        // A truncated/limited extension chain has not resolved
                        // an upper-layer protocol, though base addresses exist.
                        (!matches!(next, 0 | 43 | 44 | 51 | 60)).then_some(next)
                    }),
                    _ => return false,
                };
                let ports = packet.transport.map(|t| match t {
                    Transport::Tcp(t) => (t.source_port, t.destination_port),
                    Transport::Udp(t) => (t.source_port, t.destination_port),
                });
                (
                    Some(source),
                    Some(destination),
                    [source, destination],
                    ports,
                    protocol,
                )
            }
            Metadata::Tcp { local, remote, .. } | Metadata::Udp { local, remote } => {
                let (source, destination) = match observation.direction() {
                    Direction::Rx => (Some(remote.ip()), Some(local.ip())),
                    Direction::Tx => (Some(local.ip()), Some(remote.ip())),
                    Direction::Unknown => (None, None),
                };
                (
                    source,
                    destination,
                    [local.ip(), remote.ip()],
                    Some((local.port(), remote.port())),
                    Some(if matches!(observation.metadata(), Metadata::Tcp { .. }) {
                        6
                    } else {
                        17
                    }),
                )
            }
            _ => return false,
        };
        s.protocol.is_none_or(|p| Some(p) == protocol)
            && s.source.is_none_or(|a| Some(a) == source)
            && s.destination.is_none_or(|a| Some(a) == destination)
            && s.host.is_none_or(|a| hosts.contains(&a))
            && (s.ports.is_empty()
                || ports.is_some_and(|(a, b)| {
                    s.ports.binary_search(&a).is_ok() || s.ports.binary_search(&b).is_ok()
                }))
    }
}
fn prefilter(s: &NetworkFilter) -> String {
    if !s.has_network_predicates() {
        return String::new();
    }
    // Classic BPF's port primitive does not walk IPv6 extensions, and `vlan`
    // mutates subsequent offsets in libpcap's compiler. Keep these branches broad.
    let mut branches = vec![
        "ether[12:2] = 0x8100".to_owned(),
        "ether[12:2] = 0x88a8".into(),
    ];
    if ![s.source, s.destination, s.host]
        .into_iter()
        .flatten()
        .any(|a| a.is_ipv4())
    {
        let mut predicates = vec!["ip6".to_owned()];
        for (label, address) in [
            ("src host", s.source),
            ("dst host", s.destination),
            ("host", s.host),
        ] {
            if let Some(address) = address {
                predicates.push(format!("{label} {address}"));
            }
        }
        branches.push(format!("({})", predicates.join(" and ")));
    }
    if ![s.source, s.destination, s.host]
        .into_iter()
        .flatten()
        .any(|a| a.is_ipv6())
    {
        let mut predicates = vec!["ip".to_owned()];
        if let Some(protocol) = s.protocol {
            predicates.push(format!("ip proto {protocol}"));
        }
        for (label, address) in [
            ("src host", s.source),
            ("dst host", s.destination),
            ("host", s.host),
        ] {
            if let Some(address) = address {
                predicates.push(format!("{label} {address}"));
            }
        }
        if !s.ports.is_empty() {
            predicates.push(format!(
                "((tcp or udp) and ({}))",
                s.ports
                    .iter()
                    .map(|p| format!("port {p}"))
                    .collect::<Vec<_>>()
                    .join(" or ")
            ));
        }
        branches.push(format!("({})", predicates.join(" and ")));
    }
    format!("({})", branches.join(" or "))
}
