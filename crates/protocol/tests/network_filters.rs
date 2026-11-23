#[path = "support/filter_wire.rs"]
mod fixture;
use exmon_domain::{
    Direction,
    network_filter::{MAX_FILTER_PORTS, NetworkFilter},
    observation::{Metadata, Observation},
};
use exmon_protocol::network_filter::FilterPlan;
use fixture::*;

#[test]
fn ipv4_ipv6_tcp_udp_ports_and_all_endpoint_predicates_match_exactly() {
    for v6 in [false, true] {
        for tcp in [false, true] {
            for tags in [0, 1, 2, 8] {
                for extensions in [false, true] {
                    let raw = wire(v6, tcp, 12345, 443, tags, extensions);
                    let observation = observation(&raw, Direction::Unknown);
                    let source = if v6 { "2001:db8::1" } else { "192.0.2.1" }
                        .parse()
                        .unwrap();
                    let destination = if v6 { "2001:db8::2" } else { "198.51.100.2" }
                        .parse()
                        .unwrap();
                    let settings = NetworkFilter {
                        interface: Some(7),
                        direction: Some(Direction::Unknown),
                        protocol: Some(if tcp { 6 } else { 17 }),
                        source: Some(source),
                        destination: Some(destination),
                        host: Some(source),
                        ports: vec![443, 80, 443],
                    };
                    let plan = FilterPlan::new(settings.clone()).unwrap();
                    assert!(plan.matches(7, &observation));
                    assert!(!plan.matches(8, &observation));
                    assert_eq!(plan.settings().ports, [80, 443]);
                    assert!(plan.bpf().len() < 4096);
                    for mismatch in [
                        NetworkFilter {
                            ports: vec![444],
                            ..settings.clone()
                        },
                        NetworkFilter {
                            source: Some(destination),
                            ..settings.clone()
                        },
                        NetworkFilter {
                            destination: Some(source),
                            ..settings.clone()
                        },
                        NetworkFilter {
                            direction: Some(Direction::Rx),
                            ..settings.clone()
                        },
                        NetworkFilter {
                            protocol: Some(if tcp { 17 } else { 6 }),
                            ..settings.clone()
                        },
                    ] {
                        assert!(!FilterPlan::new(mismatch).unwrap().matches(7, &observation));
                    }
                    let reverse_port = FilterPlan::new(NetworkFilter {
                        ports: vec![12345],
                        ..Default::default()
                    })
                    .unwrap();
                    assert!(reverse_port.matches(7, &observation));
                    assert!(!format!("{plan:?}").contains("192.0.2"));
                }
            }
        }
    }
}
#[test]
fn unknown_direction_does_not_invent_socket_source_or_destination() {
    for direction in [Direction::Rx, Direction::Tx, Direction::Unknown] {
        for tcp in [false, true] {
            let local = "192.0.2.1:12000".parse().unwrap();
            let remote = "198.51.100.2:443".parse().unwrap();
            let metadata = if tcp {
                Metadata::Tcp {
                    local,
                    remote,
                    connection: 1,
                }
            } else {
                Metadata::Udp { local, remote }
            };
            let observation =
                Observation::new(0, direction, metadata, None, b"application bytes").unwrap();
            let base = NetworkFilter {
                host: Some(local.ip()),
                ports: vec![443],
                protocol: Some(if tcp { 6 } else { 17 }),
                ..Default::default()
            };
            assert!(
                FilterPlan::new(base.clone())
                    .unwrap()
                    .matches(0, &observation)
            );
            let source = if direction == Direction::Rx {
                remote.ip()
            } else {
                local.ip()
            };
            let plan = FilterPlan::new(NetworkFilter {
                source: Some(source),
                ..base
            })
            .unwrap();
            assert_eq!(
                plan.matches(0, &observation),
                direction != Direction::Unknown
            );
        }
    }
}
#[test]
fn bounded_validation_unknown_fragments_and_all_prefixes_never_guess_ports() {
    assert!(
        FilterPlan::new(NetworkFilter {
            ports: vec![80; MAX_FILTER_PORTS + 1],
            ..Default::default()
        })
        .is_err()
    );
    assert!(
        FilterPlan::new(NetworkFilter {
            source: Some("192.0.2.1".parse().unwrap()),
            destination: Some("::1".parse().unwrap()),
            ..Default::default()
        })
        .is_err()
    );
    let port = FilterPlan::new(NetworkFilter {
        ports: vec![443],
        ..Default::default()
    })
    .unwrap();
    let all = FilterPlan::new(NetworkFilter::default()).unwrap();
    assert_eq!(all.bpf(), "");
    for v6 in [false, true] {
        for tcp in [false, true] {
            let raw = wire(v6, tcp, 12345, 443, 0, false);
            let header = 14 + if v6 { 40 } else { 20 } + if tcp { 20 } else { 8 };
            for length in 0..=raw.len() {
                let observation = observation(&raw[..length], Direction::Unknown);
                assert!(all.matches(0, &observation));
                assert_eq!(port.matches(0, &observation), length >= header);
            }
            let mut fragment = raw.clone();
            if !v6 {
                fragment[20..22].copy_from_slice(&0x2000u16.to_be_bytes());
                let end = (fragment.len() - 34) / 8 * 8 + 34;
                fragment.truncate(end);
                let total = (fragment.len() - 14) as u16;
                fragment[16..18].copy_from_slice(&total.to_be_bytes());
            } else {
                fragment[20] = 44;
                fragment.splice(54..54, [if tcp { 6 } else { 17 }, 0, 0, 8, 0, 0, 0, 1]);
                let len = (fragment.len() - 54) as u16;
                fragment[18..20].copy_from_slice(&len.to_be_bytes());
            }
            assert!(!port.matches(0, &observation(&fragment, Direction::Unknown)));
            for i in 0..raw.len() {
                for value in [0, 1, 127, 255] {
                    let mut bad = raw.clone();
                    bad[i] = value;
                    port.matches(0, &observation(&bad, Direction::Unknown));
                }
            }
        }
    }
    let unknown = observation(b"not a wire frame", Direction::Unknown);
    assert!(!port.matches(0, &unknown));
    assert!(all.matches(0, &unknown));
    let raw = wire(true, true, 12345, 443, 0, true);
    let truncated = observation(&raw[..55], Direction::Unknown);
    let pending_extension = FilterPlan::new(NetworkFilter {
        protocol: Some(0),
        ..Default::default()
    })
    .unwrap();
    assert!(!pending_extension.matches(0, &truncated));
    let known_address = FilterPlan::new(NetworkFilter {
        source: Some("2001:db8::1".parse().unwrap()),
        ..Default::default()
    })
    .unwrap();
    assert!(known_address.matches(0, &truncated));
}
