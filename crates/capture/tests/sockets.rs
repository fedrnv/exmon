use exmon_capture::{
    adapter::{Adapter, Cancellation, Error, Limits as QueueLimits, Session},
    sockets::*,
};
use exmon_domain::{
    Direction,
    observation::{Metadata, Observation},
};
use std::{
    io::{self, Read},
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener, TcpStream, UdpSocket},
    time::{Duration, Instant},
};

fn local(v6: bool) -> SocketAddr {
    SocketAddr::new(
        if v6 {
            IpAddr::V6(Ipv6Addr::LOCALHOST)
        } else {
            IpAddr::V4(Ipv4Addr::LOCALHOST)
        },
        0,
    )
}
fn configured(mode: Mode, snapshot: usize) -> Sockets {
    let mut a = Sockets::new(Settings::new(mode)).unwrap();
    assert_eq!(a.enumerate(1).unwrap().len(), 1);
    a.open(0).unwrap();
    a.configure(snapshot).unwrap();
    a
}
fn active(mode: Mode, snapshot: usize) -> Sockets {
    let mut a = configured(mode, snapshot);
    a.start().unwrap();
    a
}
fn tcp_pair(v6: bool, snapshot: usize) -> (Sockets, Sockets) {
    let mut server = active(Mode::TcpServer { local: local(v6) }, snapshot);
    let remote = server.snapshot().local.unwrap();
    let client = active(
        Mode::TcpClient {
            local: local(v6),
            remote,
        },
        snapshot,
    );
    wait_peers(&mut server, 1);
    (server, client)
}
fn wait_peers(a: &mut Sockets, count: usize) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while a.peers().len() < count && Instant::now() < deadline {
        assert!(
            a.poll(&Cancellation::default(), 64, 65536)
                .unwrap()
                .is_empty()
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(a.peers().len(), count);
}
fn receive(a: &mut Sockets, expected: usize) -> Vec<Observation> {
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut out = vec![];
    let mut bytes = 0;
    while (bytes < expected || out.is_empty()) && Instant::now() < deadline {
        let batch = a.poll(&Cancellation::default(), 64, 65536).unwrap();
        assert!(batch.len() <= 64);
        assert!(batch.iter().map(|o| o.bytes().len()).sum::<usize>() <= 65536);
        bytes += batch.iter().map(|o| o.bytes().len()).sum::<usize>();
        out.extend(batch);
        if bytes < expected {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    assert_eq!(bytes, expected);
    assert!(!out.is_empty());
    out
}
fn flatten(records: &[Observation]) -> Vec<u8> {
    records.iter().flat_map(|o| o.bytes()).copied().collect()
}
fn tcp_send(a: &mut Sockets, id: u64, bytes: &[u8]) {
    let epoch = a.snapshot().epoch;
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut sent = 0;
    while sent < bytes.len() && Instant::now() < deadline {
        match a.send_tcp(epoch, id, &bytes[sent..], &Cancellation::default()) {
            Ok(n) => {
                assert!(n > 0);
                sent += n;
            }
            Err(Error::Busy) => std::thread::sleep(Duration::from_millis(1)),
            other => panic!("send result {other:?}"),
        }
    }
    assert_eq!(sent, bytes.len());
}
#[test]
fn real_ipv4_ipv6_tcp_client_server_bidirectional_bytes_and_metadata() {
    for v6 in [false, true] {
        let (mut server, mut client) = tcp_pair(v6, 37);
        let id = client.peers()[0].connection;
        assert_eq!(client.snapshot().phase, Phase::Connected);
        assert_eq!(server.snapshot().phase, Phase::Listening);
        assert!(
            client
                .poll(&Cancellation::default(), 8, 1024)
                .unwrap()
                .is_empty()
        );
        assert_eq!(client.snapshot().counters.accepted_tx_bytes, 0);
        let payload: Vec<_> = (0..1500).map(|i| (i % 256) as u8).collect();
        tcp_send(&mut client, id, &payload);
        let records = receive(&mut server, payload.len());
        assert_eq!(flatten(&records), payload);
        let server_peer = server.peers()[0];
        for o in &records {
            assert_eq!(o.direction(), Direction::Rx);
            assert_eq!(o.original_length(), None);
            assert_eq!(o.source_timestamp_ns(), None);
            assert!(o.bytes().len() <= 37);
            let Metadata::Tcp {
                local,
                remote,
                connection,
            } = o.metadata()
            else {
                panic!()
            };
            assert_eq!(
                (*local, *remote, *connection),
                (
                    server_peer.local,
                    server_peer.remote,
                    server_peer.connection
                )
            );
        }
        let reply = b"\0reply\xffwith\r\nfragments";
        tcp_send(&mut server, server_peer.connection, reply);
        assert_eq!(flatten(&receive(&mut client, reply.len())), reply);
        assert_eq!(
            client.send_tcp(client.snapshot().epoch, id, b"", &Cancellation::default()),
            Ok(0)
        );
        assert_eq!(
            client.transmit(b"no implicit target"),
            Err(Error::Unsupported)
        );
        server.close().unwrap();
        client.close().unwrap();
    }
}
#[test]
fn multiple_peers_receive_fairly_and_explicit_send_never_broadcasts() {
    for v6 in [false, true] {
        let mut server = active(Mode::TcpServer { local: local(v6) }, 16);
        let remote = server.snapshot().local.unwrap();
        let mut clients: Vec<_> = (0..3)
            .map(|_| {
                active(
                    Mode::TcpClient {
                        local: local(v6),
                        remote,
                    },
                    16,
                )
            })
            .collect();
        wait_peers(&mut server, 3);
        for (i, client) in clients.iter_mut().enumerate() {
            let id = client.peers()[0].connection;
            tcp_send(client, id, &[i as u8; 3]);
        }
        let records = receive(&mut server, 9);
        let mut ids = std::collections::BTreeSet::new();
        for o in &records {
            let Metadata::Tcp { connection, .. } = o.metadata() else {
                panic!()
            };
            ids.insert(*connection);
        }
        assert_eq!(ids.len(), 3);
        let target = server
            .peers()
            .into_iter()
            .find(|p| p.remote == clients[1].snapshot().local.unwrap())
            .unwrap();
        tcp_send(&mut server, target.connection, b"only one");
        assert_eq!(flatten(&receive(&mut clients[1], 8)), b"only one");
        for index in [0, 2] {
            assert!(
                clients[index]
                    .poll(&Cancellation::default(), 8, 1024)
                    .unwrap()
                    .is_empty()
            );
        }
        server.stop().unwrap();
        assert_eq!(server.snapshot().peers, 0);
        assert_eq!(server.snapshot().local, None);
    }
}
#[test]
fn tcp_half_close_preserves_reverse_direction_and_explicit_peer_close() {
    for v6 in [false, true] {
        let (mut server, mut client) = tcp_pair(v6, 64);
        let ci = client.peers()[0].connection;
        let si = server.peers()[0].connection;
        client
            .shutdown_write(client.snapshot().epoch, ci, &Cancellation::default())
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while !server.peers()[0].read_closed && Instant::now() < deadline {
            server.poll(&Cancellation::default(), 8, 1024).unwrap();
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(server.peers()[0].read_closed);
        assert_eq!(
            client.send_tcp(
                client.snapshot().epoch,
                ci,
                b"late",
                &Cancellation::default()
            ),
            Err(Error::InvalidState)
        );
        tcp_send(&mut server, si, b"after FIN");
        assert_eq!(flatten(&receive(&mut client, 9)), b"after FIN");
        server
            .shutdown_write(server.snapshot().epoch, si, &Cancellation::default())
            .unwrap();
        assert!(server.peers().is_empty());
        let deadline = Instant::now() + Duration::from_secs(2);
        while client.snapshot().phase != Phase::Disconnected && Instant::now() < deadline {
            let _ = client.poll(&Cancellation::default(), 8, 1024);
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(client.snapshot().phase, Phase::Disconnected);
        assert_eq!(client.snapshot().peers, 0);
        assert_eq!(
            client.poll(&Cancellation::default(), 8, 1024).err(),
            Some(Error::Disconnected)
        );
    }
}
#[test]
fn explicit_client_local_bind_and_refused_or_busy_errors_are_observable() {
    for v6 in [false, true] {
        let listener = TcpListener::bind(local(v6)).unwrap();
        let remote = listener.local_addr().unwrap();
        let reservation = UdpSocket::bind(local(v6)).unwrap();
        let chosen = reservation.local_addr().unwrap();
        drop(reservation);
        let mut client = active(
            Mode::TcpClient {
                local: chosen,
                remote,
            },
            64,
        );
        assert_eq!(client.snapshot().local, Some(chosen));
        let (mut peer, _) = listener.accept().unwrap();
        peer.set_nonblocking(true).unwrap();
        let mut byte = [0; 1];
        assert_eq!(
            peer.read(&mut byte).unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        client.stop().unwrap();
        drop(peer);
        drop(listener);
        let held = TcpListener::bind(local(v6)).unwrap();
        let address = held.local_addr().unwrap();
        let mut busy = configured(Mode::TcpServer { local: address }, 64);
        assert_eq!(busy.start(), Err(Error::Busy));
        assert_eq!(busy.snapshot().phase, Phase::Failed);
        assert_eq!(busy.snapshot().last_failure, Some(Failure::AddressInUse));
        assert_eq!(busy.snapshot().local, None);
        drop(held);
        busy.stop().unwrap();
        busy.configure(64).unwrap();
        busy.start().unwrap();
        busy.stop().unwrap();
        let probe = socket2::Socket::new(
            if v6 {
                socket2::Domain::IPV6
            } else {
                socket2::Domain::IPV4
            },
            socket2::Type::STREAM,
            Some(socket2::Protocol::TCP),
        )
        .unwrap();
        probe.bind(&local(v6).into()).unwrap();
        let remote = probe.local_addr().unwrap().as_socket().unwrap();
        let mut refused = configured(
            Mode::TcpClient {
                local: local(v6),
                remote,
            },
            64,
        );
        assert!(refused.start().is_err());
        assert_eq!(refused.snapshot().phase, Phase::Failed);
        assert_eq!(refused.snapshot().peers, 0);
        assert!(matches!(
            refused.snapshot().last_failure,
            Some(Failure::Refused | Failure::TimedOut)
        ));
        assert!(
            refused
                .drain_events(256)
                .iter()
                .any(|e| e.kind == EventKind::Failure)
        );
    }
}
#[test]
fn udp_bound_connected_bidirectional_fidelity_empty_and_snapshot_length() {
    for v6 in [false, true] {
        let mut bound = active(Mode::UdpBound { local: local(v6) }, 64);
        let target = bound.snapshot().local.unwrap();
        let mut connected = active(
            Mode::UdpConnected {
                local: local(v6),
                remote: target,
            },
            64,
        );
        let remote = connected.snapshot().local.unwrap();
        assert_eq!(connected.take_backend_loss(), Some(None));
        assert_eq!(connected.take_backend_loss(), None);
        let bytes = b"\0binary\xff datagram\r\n";
        assert_eq!(
            connected.send_udp(
                connected.snapshot().epoch,
                None,
                bytes,
                &Cancellation::default()
            ),
            Ok(bytes.len())
        );
        let records = receive(&mut bound, bytes.len());
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].bytes(), bytes);
        assert_eq!(records[0].original_length(), Some(bytes.len()));
        assert!(
            matches!(records[0].metadata(),Metadata::Udp {local,remote:r} if *local==target&&*r==remote)
        );
        bound
            .send_udp(
                bound.snapshot().epoch,
                Some(remote),
                b"answer",
                &Cancellation::default(),
            )
            .unwrap();
        assert_eq!(flatten(&receive(&mut connected, 6)), b"answer");
        connected
            .send_udp(
                connected.snapshot().epoch,
                None,
                b"",
                &Cancellation::default(),
            )
            .unwrap();
        let records = receive(&mut bound, 0);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].original_length(), Some(0));
        bound.stop().unwrap();
        bound.configure(3).unwrap();
        bound.start().unwrap();
        let target = bound.snapshot().local.unwrap();
        let source = UdpSocket::bind(local(v6)).unwrap();
        source.send_to(b"long message", target).unwrap();
        let records = receive(&mut bound, 3);
        assert_eq!(records[0].bytes(), b"lon");
        assert_eq!(records[0].original_length(), Some(12));
        assert_eq!(bound.snapshot().counters.truncated_datagrams, 1);
    }
}
#[test]
fn connected_udp_filters_foreign_peers_and_rejects_destination_override() {
    for v6 in [false, true] {
        let peer = UdpSocket::bind(local(v6)).unwrap();
        peer.set_nonblocking(true).unwrap();
        let remote = peer.local_addr().unwrap();
        let foreign = UdpSocket::bind(local(v6)).unwrap();
        let mut adapter = active(
            Mode::UdpConnected {
                local: local(v6),
                remote,
            },
            64,
        );
        let target = adapter.snapshot().local.unwrap();
        assert_eq!(
            adapter.send_udp(
                adapter.snapshot().epoch,
                Some(foreign.local_addr().unwrap()),
                b"wrong",
                &Cancellation::default()
            ),
            Err(Error::InvalidConfig)
        );
        foreign.send_to(b"foreign", target).unwrap();
        peer.send_to(b"selected", target).unwrap();
        assert_eq!(flatten(&receive(&mut adapter, 8)), b"selected");
        let mut buffer = [0; 16];
        assert_eq!(
            peer.recv_from(&mut buffer).unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
    }
}
#[test]
fn stop_reopen_stale_epoch_cancellation_and_drop_release_owned_ports() {
    for v6 in [false, true] {
        let mut a = configured(Mode::UdpBound { local: local(v6) }, 32);
        let cancel = Cancellation::default();
        cancel.cancel();
        assert_eq!(a.start_with_cancel(&cancel), Err(Error::Cancelled));
        assert_eq!(a.snapshot().local, None);
        let mut old_epoch = 0;
        for _ in 0..24 {
            a.start().unwrap();
            let current = a.snapshot().epoch;
            assert!(current > old_epoch);
            let address = a.snapshot().local.unwrap();
            assert_eq!(
                a.send_udp(old_epoch, Some(address), b"stale", &Cancellation::default()),
                Err(Error::InvalidState)
            );
            assert_eq!(
                a.send_udp(current, Some(address), b"cancel", &cancel),
                Err(Error::Cancelled)
            );
            assert!(a.poll(&Cancellation::default(), 1, 32).unwrap().is_empty());
            assert_eq!(a.poll(&cancel, 1, 32).err(), Some(Error::Cancelled));
            a.stop().unwrap();
            assert_eq!(a.snapshot().phase, Phase::Stopped);
            assert_eq!(a.snapshot().local, None);
            assert!(a.peers().is_empty());
            let port = UdpSocket::bind(address).unwrap();
            drop(port);
            old_epoch = current;
        }
        a.start().unwrap();
        let address = a.snapshot().local.unwrap();
        drop(a);
        drop(UdpSocket::bind(address).unwrap());
        let a = active(Mode::TcpServer { local: local(v6) }, 64);
        let address = a.snapshot().local.unwrap();
        drop(a);
        drop(TcpListener::bind(address).unwrap());
        let (mut server, mut client) = tcp_pair(v6, 64);
        let old = client.peers()[0];
        let epoch = client.snapshot().epoch;
        client.stop().unwrap();
        client.start().unwrap();
        wait_peers(&mut server, 2);
        assert_ne!(client.peers()[0].connection, old.connection);
        assert_eq!(
            client.send_tcp(epoch, old.connection, b"stale", &Cancellation::default()),
            Err(Error::InvalidState)
        );
    }
}
#[test]
fn peer_and_event_caps_are_explicit_and_polling_is_bounded() {
    let settings = Settings {
        max_peers: 1,
        event_capacity: 2,
        ..Settings::new(Mode::TcpServer {
            local: local(false),
        })
    };
    let mut server = Sockets::new(settings).unwrap();
    server.open(0).unwrap();
    server.configure(8).unwrap();
    server.start().unwrap();
    let address = server.snapshot().local.unwrap();
    let a = TcpStream::connect(address).unwrap();
    wait_peers(&mut server, 1);
    // Drain the one-slot OS listen backlog before testing the active-peer cap.
    let b = TcpStream::connect(address).unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    while server.snapshot().counters.rejected_peers == 0 {
        assert!(Instant::now() < deadline);
        assert!(
            server
                .poll(&Cancellation::default(), 64, 65536)
                .unwrap()
                .is_empty()
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(server.snapshot().peers, 1);
    assert!(server.snapshot().counters.rejected_peers >= 1);
    assert!(server.snapshot().counters.events_dropped > 0);
    assert!(server.drain_events(usize::MAX).len() <= 2);
    assert_eq!(
        server.poll(&Cancellation::default(), 0, 8).err(),
        Some(Error::InvalidConfig)
    );
    assert_eq!(
        server.poll(&Cancellation::default(), 65, 65536).err(),
        Some(Error::InvalidConfig)
    );
    assert_eq!(
        server.poll(&Cancellation::default(), 1, 7).err(),
        Some(Error::InvalidConfig)
    );
    drop(a);
    drop(b);
    server.close().unwrap();
    assert!(server.peers().is_empty());
}
#[test]
fn tcp_backpressure_reports_only_actual_accepted_prefix_and_can_be_stopped() {
    let listener = TcpListener::bind(local(false)).unwrap();
    let remote = listener.local_addr().unwrap();
    let settings = Settings {
        socket_buffer_bytes: 1024,
        ..Settings::new(Mode::TcpClient {
            local: local(false),
            remote,
        })
    };
    let mut a = Sockets::new(settings).unwrap();
    a.open(0).unwrap();
    a.configure(64).unwrap();
    a.start().unwrap();
    let (mut peer, _) = listener.accept().unwrap();
    peer.set_nonblocking(true).unwrap();
    let bytes = vec![0x5a; 65536];
    let id = a.peers()[0].connection;
    let epoch = a.snapshot().epoch;
    let mut accepted = 0;
    let mut backpressure = false;
    let deadline = Instant::now() + Duration::from_secs(2);
    for _ in 0..1024 {
        assert!(Instant::now() < deadline);
        match a.send_tcp(epoch, id, &bytes, &Cancellation::default()) {
            Ok(n) => {
                assert!(n > 0 && n <= bytes.len());
                accepted += n;
            }
            Err(Error::Busy) => {
                backpressure = true;
                break;
            }
            other => panic!("{other:?}"),
        }
    }
    assert!(backpressure);
    assert_eq!(a.snapshot().counters.accepted_tx_bytes, accepted as u64);
    let mut received = 0;
    let mut buffer = [0; 65536];
    let deadline = Instant::now() + Duration::from_secs(2);
    while received < accepted && Instant::now() < deadline {
        match peer.read(&mut buffer) {
            Ok(n) => {
                assert!(n > 0);
                assert!(buffer[..n].iter().all(|b| *b == 0x5a));
                received += n;
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(1))
            }
            other => panic!("{other:?}"),
        }
    }
    assert_eq!(received, accepted);
    let begin = Instant::now();
    a.stop().unwrap();
    assert!(begin.elapsed() < Duration::from_millis(250));
    assert_eq!(a.snapshot().peers, 0);
}
#[test]
fn common_session_udp_limits_metadata_and_cancellation_use_actual_adapter() {
    let probe = UdpSocket::bind(local(false)).unwrap();
    let address = probe.local_addr().unwrap();
    drop(probe);
    let adapter = Sockets::new(Settings::new(Mode::UdpBound { local: address })).unwrap();
    let mut session = Session::new(
        adapter,
        QueueLimits {
            queue_items: 2,
            queue_bytes: 16,
            batch_items: 2,
            batch_bytes: 16,
        },
    )
    .unwrap();
    session.enumerate().unwrap();
    session.open(0).unwrap();
    session.configure(8).unwrap();
    session.start().unwrap();
    let peer = UdpSocket::bind(local(false)).unwrap();
    peer.send_to(b"message", address).unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut records = vec![];
    while records.is_empty() && Instant::now() < deadline {
        session.pump().unwrap();
        records.extend(session.drain(2));
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].observation.bytes(), b"message");
    session.stop().unwrap();
    session.reconnect(0).unwrap();
    session.configure(8).unwrap();
    session.start().unwrap();
    session.close().unwrap();
    drop(UdpSocket::bind(address).unwrap());
}
#[test]
fn reset_removes_only_affected_peer_and_explicit_close_revokes_its_id() {
    let mut server = active(
        Mode::TcpServer {
            local: local(false),
        },
        64,
    );
    let address = server.snapshot().local.unwrap();
    let external = TcpStream::connect(address).unwrap();
    let mut healthy = active(
        Mode::TcpClient {
            local: local(false),
            remote: address,
        },
        64,
    );
    wait_peers(&mut server, 2);
    let reset_id = server
        .peers()
        .iter()
        .find(|p| p.remote == external.local_addr().unwrap())
        .unwrap()
        .connection;
    let reset: socket2::Socket = external.into();
    reset.set_linger(Some(Duration::ZERO)).unwrap();
    drop(reset);
    let deadline = Instant::now() + Duration::from_secs(2);
    while server.snapshot().peers != 1 {
        assert!(Instant::now() < deadline);
        assert!(
            server
                .poll(&Cancellation::default(), 64, 65536)
                .unwrap()
                .is_empty()
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(server.snapshot().phase, Phase::Listening);
    assert_eq!(server.snapshot().last_failure, Some(Failure::Reset));
    assert!(
        server
            .drain_events(256)
            .iter()
            .any(|e| e.connection == Some(reset_id) && e.failure == Some(Failure::Reset))
    );
    assert_eq!(server.take_backend_loss(), Some(None));
    let id = healthy.peers()[0].connection;
    tcp_send(&mut healthy, id, b"survives");
    assert_eq!(flatten(&receive(&mut server, 8)), b"survives");
    let epoch = server.snapshot().epoch;
    let peer = server.peers()[0].connection;
    server
        .close_peer(epoch, peer, &Cancellation::default())
        .unwrap();
    assert_eq!(server.snapshot().peers, 0);
    assert_eq!(
        server.send_tcp(epoch, peer, b"closed", &Cancellation::default()),
        Err(Error::InvalidConfig)
    );
    assert_eq!(
        server.close_peer(epoch, peer, &Cancellation::default()),
        Err(Error::InvalidConfig)
    );
}
#[test]
fn udp_maximum_payload_send_limits_and_event_endpoints_are_exact() {
    let mut a = active(
        Mode::UdpBound {
            local: local(false),
        },
        65536,
    );
    let mut b = active(
        Mode::UdpBound {
            local: local(false),
        },
        65536,
    );
    let source = a.snapshot().local.unwrap();
    let destination = b.snapshot().local.unwrap();
    let epoch = a.snapshot().epoch;
    let payload: Vec<_> = (0..65507).map(|i| (i % 251) as u8).collect();
    assert_eq!(
        a.send_udp(epoch, Some(destination), &payload, &Cancellation::default()),
        Ok(payload.len())
    );
    let records = receive(&mut b, payload.len());
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].bytes(), payload);
    assert_eq!(records[0].original_length(), Some(payload.len()));
    let sent = a
        .drain_events(256)
        .into_iter()
        .find(|e| e.kind == EventKind::Sent)
        .unwrap();
    assert_eq!(sent.local, Some(source));
    assert_eq!(sent.remote, Some(destination));
    assert_eq!(sent.length, Some(payload.len()));
    assert_eq!(
        a.send_udp(
            epoch,
            Some(destination),
            &vec![0; 65508],
            &Cancellation::default()
        ),
        Err(Error::InvalidConfig)
    );
    assert_eq!(
        a.send_udp(epoch, None, b"no target", &Cancellation::default()),
        Err(Error::InvalidConfig)
    );
    assert_eq!(a.snapshot().counters.sent_datagrams, 1);
    b.stop().unwrap();
    b.configure(1).unwrap();
    b.start().unwrap();
    let destination = b.snapshot().local.unwrap();
    a.send_udp(
        epoch,
        Some(destination),
        b"truncate",
        &Cancellation::default(),
    )
    .unwrap();
    assert_eq!(flatten(&receive(&mut b, 1)), b"t");
    let event = b
        .drain_events(256)
        .into_iter()
        .find(|e| e.kind == EventKind::DatagramTruncated)
        .unwrap();
    assert_eq!(event.remote, Some(source));
    assert_eq!(event.length, Some(8));
}
#[test]
fn settings_validation_and_debug_do_not_open_or_leak_endpoints() {
    let mode = Mode::TcpClient {
        local: local(false),
        remote: "127.0.0.1:12345".parse().unwrap(),
    };
    for settings in [
        Settings {
            max_peers: 33,
            ..Settings::new(mode)
        },
        Settings {
            event_capacity: 257,
            ..Settings::new(mode)
        },
        Settings {
            connect_timeout_ms: 0,
            ..Settings::new(mode)
        },
        Settings {
            socket_buffer_bytes: 0,
            ..Settings::new(mode)
        },
        Settings {
            max_send_bytes: 65537,
            ..Settings::new(mode)
        },
    ] {
        assert_eq!(Sockets::new(settings).err(), Some(Error::InvalidConfig));
    }
    for mode in [
        Mode::UdpConnected {
            local: local(false),
            remote: local(false),
        },
        Mode::TcpClient {
            local: local(false),
            remote: "[::1]:80".parse().unwrap(),
        },
        Mode::TcpClient {
            local: local(false),
            remote: "0.0.0.0:80".parse().unwrap(),
        },
    ] {
        assert_eq!(
            Sockets::new(Settings::new(mode)).err(),
            Some(Error::InvalidConfig)
        );
    }
    let mut a = configured(mode, 64);
    assert!(!format!("{:?}", a.settings()).contains("127.0.0.1"));
    assert_eq!(a.snapshot().phase, Phase::Configured);
    assert_eq!(a.snapshot().local, None);
    assert!(
        a.drain_events(256)
            .iter()
            .all(|e| !format!("{e:?}").contains("127.0.0.1"))
    );
}
