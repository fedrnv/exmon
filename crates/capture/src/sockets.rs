//! Worker-owned ordinary sockets. No I/O before explicit Start, no implicit payload TX.
use crate::adapter::{Adapter, Availability, Cancellation, Descriptor, Error};
use exmon_domain::{
    Direction,
    observation::{MAX_PAYLOAD, Metadata, Observation, TransportKind},
};
use socket2::{Domain, Protocol, Socket, Type};
use std::{
    collections::{BTreeMap, VecDeque},
    fmt,
    io::{self, Read, Write},
    net::{Shutdown, SocketAddr, TcpListener, TcpStream, UdpSocket},
    time::{Duration, Instant},
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    TcpClient {
        local: SocketAddr,
        remote: SocketAddr,
    },
    TcpServer {
        local: SocketAddr,
    },
    UdpBound {
        local: SocketAddr,
    },
    UdpConnected {
        local: SocketAddr,
        remote: SocketAddr,
    },
}
impl fmt::Debug for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SocketMode([REDACTED])")
    }
}
impl Mode {
    pub fn local(self) -> SocketAddr {
        match self {
            Self::TcpClient { local, .. }
            | Self::TcpServer { local }
            | Self::UdpBound { local }
            | Self::UdpConnected { local, .. } => local,
        }
    }
    pub fn remote(self) -> Option<SocketAddr> {
        match self {
            Self::TcpClient { remote, .. } | Self::UdpConnected { remote, .. } => Some(remote),
            _ => None,
        }
    }
    pub fn kind(self) -> TransportKind {
        match self {
            Self::TcpClient { .. } | Self::TcpServer { .. } => TransportKind::Tcp,
            _ => TransportKind::Udp,
        }
    }
}
#[derive(Debug, Clone, Copy)]
pub struct Settings {
    pub mode: Mode,
    pub max_peers: usize,
    pub event_capacity: usize,
    pub connect_timeout_ms: u32,
    pub socket_buffer_bytes: usize,
    pub max_send_bytes: usize,
}
impl Settings {
    pub fn new(mode: Mode) -> Self {
        Self {
            mode,
            max_peers: 16,
            event_capacity: 64,
            connect_timeout_ms: 250,
            socket_buffer_bytes: 65536,
            max_send_bytes: 65536,
        }
    }
    pub fn validate(self) -> Result<(), Error> {
        if !(1..=32).contains(&self.max_peers)
            || !(1..=256).contains(&self.event_capacity)
            || !(1..=1000).contains(&self.connect_timeout_ms)
            || !(1024..=1024 * 1024).contains(&self.socket_buffer_bytes)
            || !(1..=MAX_PAYLOAD).contains(&self.max_send_bytes)
            || self
                .mode
                .remote()
                .is_some_and(|r| !valid_remote(r, self.mode.local()))
        {
            return Err(Error::InvalidConfig);
        }
        Ok(())
    }
}
fn valid_remote(remote: SocketAddr, local: SocketAddr) -> bool {
    remote.is_ipv4() == local.is_ipv4() && remote.port() != 0 && !remote.ip().is_unspecified()
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Closed,
    Open,
    Configured,
    Starting,
    Listening,
    Connected,
    Bound,
    Stopped,
    Disconnected,
    Failed,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Failure {
    PermissionDenied,
    AddressInUse,
    AddressUnavailable,
    Refused,
    TimedOut,
    Reset,
    Capacity,
    DatagramTooLarge,
    Exhausted,
    Io,
}
fn failure(e: &io::Error) -> Failure {
    match e.kind() {
        io::ErrorKind::PermissionDenied => Failure::PermissionDenied,
        io::ErrorKind::AddrInUse => Failure::AddressInUse,
        io::ErrorKind::AddrNotAvailable => Failure::AddressUnavailable,
        io::ErrorKind::ConnectionRefused => Failure::Refused,
        io::ErrorKind::TimedOut => Failure::TimedOut,
        io::ErrorKind::ConnectionReset
        | io::ErrorKind::ConnectionAborted
        | io::ErrorKind::BrokenPipe
        | io::ErrorKind::NotConnected => Failure::Reset,
        _ => Failure::Io,
    }
}
fn common(f: Failure) -> Error {
    match f {
        Failure::PermissionDenied => Error::PermissionDenied,
        Failure::AddressInUse | Failure::Capacity => Error::Busy,
        Failure::Refused | Failure::Reset => Error::Disconnected,
        Failure::Exhausted => Error::Exhausted,
        _ => Error::Backend,
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    Phase(Phase),
    PeerConnected,
    PeerReadClosed,
    PeerWriteClosed,
    PeerClosed,
    PeerRejected,
    Failure,
    DatagramTruncated,
    Sent,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Event {
    pub epoch: u64,
    pub timestamp_ns: u64,
    pub kind: EventKind,
    pub connection: Option<u64>,
    pub local: Option<SocketAddr>,
    pub remote: Option<SocketAddr>,
    pub failure: Option<Failure>,
    pub length: Option<usize>,
}
impl fmt::Debug for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SocketEvent")
            .field("epoch", &self.epoch)
            .field("kind", &self.kind)
            .field("failure", &self.failure)
            .finish()
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Peer {
    pub connection: u64,
    pub local: SocketAddr,
    pub remote: SocketAddr,
    pub read_closed: bool,
    pub write_closed: bool,
}
impl fmt::Debug for Peer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SocketPeer([REDACTED])")
    }
}
struct Connection {
    stream: TcpStream,
    info: Peer,
}
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Counters {
    pub received_bytes: u64,
    pub accepted_tx_bytes: u64,
    pub received_datagrams: u64,
    pub sent_datagrams: u64,
    pub truncated_datagrams: u64,
    pub oversized_datagrams: u64,
    pub rejected_peers: u64,
    pub events_dropped: u64,
}
#[derive(Clone, Copy)]
pub struct Snapshot {
    pub phase: Phase,
    pub epoch: u64,
    pub local: Option<SocketAddr>,
    pub peers: usize,
    pub last_failure: Option<Failure>,
    pub counters: Counters,
}
impl fmt::Debug for Snapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SocketSnapshot")
            .field("phase", &self.phase)
            .field("peers", &self.peers)
            .field("last_failure", &self.last_failure)
            .finish()
    }
}

pub struct Sockets {
    settings: Settings,
    phase: Phase,
    selected: bool,
    snapshot: usize,
    epoch: u64,
    next_connection: u64,
    local: Option<SocketAddr>,
    listener: Option<TcpListener>,
    udp: Option<UdpSocket>,
    peers: BTreeMap<u64, Connection>,
    cursor: u64,
    events: VecDeque<Event>,
    counters: Counters,
    last_failure: Option<Failure>,
    clock: Option<Instant>,
    buffer: Box<[u8]>,
    unknown_loss: bool,
    known_loss: u64,
}
impl Sockets {
    pub fn new(settings: Settings) -> Result<Self, Error> {
        settings.validate()?;
        Ok(Self {
            settings,
            phase: Phase::Closed,
            selected: false,
            snapshot: 0,
            epoch: 0,
            next_connection: 0,
            local: None,
            listener: None,
            udp: None,
            peers: BTreeMap::new(),
            cursor: 0,
            events: VecDeque::new(),
            counters: Default::default(),
            last_failure: None,
            clock: None,
            buffer: vec![0; MAX_PAYLOAD].into_boxed_slice(),
            unknown_loss: false,
            known_loss: 0,
        })
    }
    pub fn settings(&self) -> Settings {
        self.settings
    }
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            phase: self.phase,
            epoch: self.epoch,
            local: self.local,
            peers: self.peers.len(),
            last_failure: self.last_failure,
            counters: self.counters,
        }
    }
    pub fn peers(&self) -> Vec<Peer> {
        self.peers.values().map(|c| c.info).collect()
    }
    pub fn drain_events(&mut self, maximum: usize) -> Vec<Event> {
        self.events
            .drain(
                ..maximum
                    .min(self.settings.event_capacity)
                    .min(self.events.len()),
            )
            .collect()
    }
    fn running(&self) -> bool {
        matches!(
            self.phase,
            Phase::Listening | Phase::Connected | Phase::Bound
        )
    }
    fn now(&self) -> u64 {
        self.clock.map_or(0, |c| {
            c.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64
        })
    }
    fn event(
        &mut self,
        kind: EventKind,
        peer: Option<Peer>,
        error: Option<Failure>,
        length: Option<usize>,
    ) {
        if self.events.len() == self.settings.event_capacity {
            self.events.pop_front();
            self.counters.events_dropped = self.counters.events_dropped.saturating_add(1);
        }
        self.events.push_back(Event {
            epoch: self.epoch,
            timestamp_ns: self.now(),
            kind,
            connection: peer.map(|p| p.connection),
            local: peer.map(|p| p.local).or(self.local),
            remote: peer.map(|p| p.remote).or(self.settings.mode.remote()),
            failure: error,
            length,
        });
    }
    fn phase(&mut self, phase: Phase) {
        self.phase = phase;
        self.event(EventKind::Phase(phase), None, None, None);
    }
    fn udp_event(
        &mut self,
        kind: EventKind,
        remote: SocketAddr,
        error: Option<Failure>,
        length: Option<usize>,
    ) {
        self.event(kind, None, error, length);
        // event_capacity is validated nonzero; the freshly appended event exists.
        if let Some(event) = self.events.back_mut() {
            event.remote = Some(remote);
        }
    }
    fn descriptor(&self) -> Descriptor {
        Descriptor {
            id: 0,
            kind: self.settings.mode.kind(),
            rx: Availability::Available,
            tx: Availability::Available,
        }
    }
    fn release(&mut self) {
        let peers = std::mem::take(&mut self.peers);
        for (_, c) in peers {
            let _ = c.stream.shutdown(Shutdown::Both);
            self.event(EventKind::PeerClosed, Some(c.info), None, None);
        }
        self.listener = None;
        self.udp = None;
        self.local = None;
    }
    fn fail(&mut self, error: Failure) -> Error {
        self.phase = Phase::Failed;
        self.last_failure = Some(error);
        self.release();
        self.event(EventKind::Failure, None, Some(error), None);
        common(error)
    }
    fn socket(&self, stream: bool) -> io::Result<Socket> {
        let socket = Socket::new(
            if self.settings.mode.local().is_ipv4() {
                Domain::IPV4
            } else {
                Domain::IPV6
            },
            if stream { Type::STREAM } else { Type::DGRAM },
            Some(if stream { Protocol::TCP } else { Protocol::UDP }),
        )?;
        if self.settings.mode.local().is_ipv6() {
            socket.set_only_v6(true)?;
        }
        socket.set_recv_buffer_size(self.settings.socket_buffer_bytes)?;
        socket.set_send_buffer_size(self.settings.socket_buffer_bytes)?;
        socket.bind(&self.settings.mode.local().into())?;
        Ok(socket)
    }
    fn add_peer(&mut self, stream: TcpStream) -> Result<u64, Error> {
        stream
            .set_nonblocking(true)
            .map_err(|e| common(failure(&e)))?;
        stream.set_nodelay(true).map_err(|e| common(failure(&e)))?;
        let id = self
            .next_connection
            .checked_add(1)
            .ok_or(Error::Exhausted)?;
        let info = Peer {
            connection: id,
            local: stream.local_addr().map_err(|e| common(failure(&e)))?,
            remote: stream.peer_addr().map_err(|e| common(failure(&e)))?,
            read_closed: false,
            write_closed: false,
        };
        self.next_connection = id;
        self.peers.insert(id, Connection { stream, info });
        self.event(EventKind::PeerConnected, Some(info), None, None);
        Ok(id)
    }
    /// Explicit connect/bind action. Kernel handshakes may occur; cancellation
    /// cannot recall already submitted protocol traffic. No application payload.
    pub fn start_with_cancel(&mut self, cancel: &Cancellation) -> Result<(), Error> {
        if !self.selected
            || self.snapshot == 0
            || !matches!(self.phase, Phase::Configured | Phase::Stopped)
        {
            return Err(Error::InvalidState);
        }
        if cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        self.epoch = self.epoch.checked_add(1).ok_or(Error::Exhausted)?;
        self.clock = Some(Instant::now());
        self.last_failure = None;
        self.phase(Phase::Starting);
        let result = (|| -> Result<(), Failure> {
            match self.settings.mode {
                Mode::TcpClient { remote, .. } => {
                    let socket = self.socket(true).map_err(|e| failure(&e))?;
                    socket
                        .connect_timeout(
                            &remote.into(),
                            Duration::from_millis(u64::from(self.settings.connect_timeout_ms)),
                        )
                        .map_err(|e| failure(&e))?;
                    // connect_timeout overrides blocking mode on success.
                    let stream: TcpStream = socket.into();
                    self.local = Some(stream.local_addr().map_err(|e| failure(&e))?);
                    self.add_peer(stream).map_err(|e| {
                        if e == Error::Exhausted {
                            Failure::Exhausted
                        } else {
                            Failure::Io
                        }
                    })?;
                    self.phase(Phase::Connected);
                }
                Mode::TcpServer { .. } => {
                    let socket = self.socket(true).map_err(|e| failure(&e))?;
                    socket
                        .listen(self.settings.max_peers as i32)
                        .map_err(|e| failure(&e))?;
                    socket.set_nonblocking(true).map_err(|e| failure(&e))?;
                    let listener: TcpListener = socket.into();
                    self.local = Some(listener.local_addr().map_err(|e| failure(&e))?);
                    self.listener = Some(listener);
                    self.phase(Phase::Listening);
                }
                Mode::UdpBound { .. } | Mode::UdpConnected { .. } => {
                    let socket = self.socket(false).map_err(|e| failure(&e))?;
                    let socket: UdpSocket = socket.into();
                    if let Some(remote) = self.settings.mode.remote() {
                        socket.connect(remote).map_err(|e| failure(&e))?;
                    }
                    socket.set_nonblocking(true).map_err(|e| failure(&e))?;
                    self.local = Some(socket.local_addr().map_err(|e| failure(&e))?);
                    self.udp = Some(socket);
                    self.unknown_loss = true;
                    self.phase(if self.settings.mode.remote().is_some() {
                        Phase::Connected
                    } else {
                        Phase::Bound
                    });
                }
            }
            Ok(())
        })();
        if let Err(error) = result {
            return Err(self.fail(error));
        }
        if cancel.is_cancelled() {
            self.stop()?;
            return Err(Error::Cancelled);
        }
        Ok(())
    }
    fn authority(&self, epoch: u64, cancel: &Cancellation) -> Result<(), Error> {
        if cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        if !self.running() || epoch != self.epoch {
            return Err(Error::InvalidState);
        }
        Ok(())
    }
    fn remove_peer(&mut self, id: u64, error: Option<Failure>) {
        if let Some(c) = self.peers.remove(&id) {
            let _ = c.stream.shutdown(Shutdown::Both);
            self.event(EventKind::PeerClosed, Some(c.info), error, None);
        }
        if let Some(error) = error {
            self.last_failure = Some(error);
            self.unknown_loss = true;
        }
        if matches!(self.settings.mode, Mode::TcpClient { .. }) && self.peers.is_empty() {
            self.phase(Phase::Disconnected);
        }
    }
    pub fn close_peer(&mut self, epoch: u64, id: u64, cancel: &Cancellation) -> Result<(), Error> {
        self.authority(epoch, cancel)?;
        if !self.peers.contains_key(&id) {
            return Err(Error::InvalidConfig);
        }
        self.remove_peer(id, None);
        Ok(())
    }
    pub fn shutdown_write(
        &mut self,
        epoch: u64,
        id: u64,
        cancel: &Cancellation,
    ) -> Result<(), Error> {
        self.authority(epoch, cancel)?;
        let c = self.peers.get_mut(&id).ok_or(Error::InvalidConfig)?;
        if !c.info.write_closed {
            if let Err(e) = c.stream.shutdown(Shutdown::Write) {
                let f = failure(&e);
                self.remove_peer(id, Some(f));
                return Err(common(f));
            }
            c.info.write_closed = true;
        }
        let info = c.info;
        self.event(EventKind::PeerWriteClosed, Some(info), None, None);
        if info.read_closed {
            self.remove_peer(id, None);
        }
        Ok(())
    }
    /// One nonblocking submission. Return actual accepted prefix; caller alone may
    /// retry its remaining suffix while its explicit user authority is still valid.
    pub fn send_tcp(
        &mut self,
        epoch: u64,
        id: u64,
        bytes: &[u8],
        cancel: &Cancellation,
    ) -> Result<usize, Error> {
        self.authority(epoch, cancel)?;
        if bytes.len() > self.settings.max_send_bytes {
            return Err(Error::InvalidConfig);
        }
        let c = self.peers.get_mut(&id).ok_or(Error::InvalidConfig)?;
        if c.info.write_closed {
            return Err(Error::InvalidState);
        }
        if bytes.is_empty() {
            return Ok(0);
        }
        let info = c.info;
        match c.stream.write(bytes) {
            Ok(n) if n > 0 => {
                self.counters.accepted_tx_bytes =
                    self.counters.accepted_tx_bytes.saturating_add(n as u64);
                self.event(EventKind::Sent, Some(info), None, Some(n));
                Ok(n)
            }
            Ok(_) => {
                self.remove_peer(id, Some(Failure::Reset));
                Err(Error::Disconnected)
            }
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                ) =>
            {
                Err(Error::Busy)
            }
            Err(e) => {
                let f = failure(&e);
                self.remove_peer(id, Some(f));
                Err(common(f))
            }
        }
    }
    pub fn send_udp(
        &mut self,
        epoch: u64,
        destination: Option<SocketAddr>,
        bytes: &[u8],
        cancel: &Cancellation,
    ) -> Result<usize, Error> {
        self.authority(epoch, cancel)?;
        let peer = match self.settings.mode {
            Mode::UdpBound { local } => destination
                .filter(|r| valid_remote(*r, local))
                .ok_or(Error::InvalidConfig)?,
            Mode::UdpConnected { remote, .. } => {
                if destination.is_some_and(|r| r != remote) {
                    return Err(Error::InvalidConfig);
                }
                remote
            }
            _ => return Err(Error::Unsupported),
        };
        if bytes.len() > self.settings.max_send_bytes
            || bytes.len() > if peer.is_ipv4() { 65507 } else { 65527 }
        {
            return Err(Error::InvalidConfig);
        }
        let socket = self.udp.as_ref().ok_or(Error::InvalidState)?;
        let result = if matches!(self.settings.mode, Mode::UdpConnected { .. }) {
            socket.send(bytes)
        } else {
            socket.send_to(bytes, peer)
        };
        match result {
            Ok(n) if n == bytes.len() => {
                self.counters.accepted_tx_bytes =
                    self.counters.accepted_tx_bytes.saturating_add(n as u64);
                self.counters.sent_datagrams = self.counters.sent_datagrams.saturating_add(1);
                self.udp_event(EventKind::Sent, peer, None, Some(n));
                Ok(n)
            }
            Ok(_) => Err(self.fail(Failure::Io)),
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                ) =>
            {
                Err(Error::Busy)
            }
            Err(e) => {
                let f = failure(&e);
                self.last_failure = Some(f);
                self.udp_event(EventKind::Failure, peer, Some(f), None);
                Err(common(f))
            }
        }
    }
    fn accept(&mut self, cancel: &Cancellation, budget: &mut usize) -> Result<(), Error> {
        for _ in 0..8 {
            if *budget == 0 || self.listener.is_none() {
                break;
            }
            if cancel.is_cancelled() {
                return Err(Error::Cancelled);
            }
            *budget -= 1;
            match self.listener.as_ref().unwrap().accept() {
                Ok((stream, _)) => {
                    if self.peers.len() == self.settings.max_peers {
                        let _ = stream.shutdown(Shutdown::Both);
                        self.counters.rejected_peers =
                            self.counters.rejected_peers.saturating_add(1);
                        self.event(EventKind::PeerRejected, None, Some(Failure::Capacity), None);
                        continue;
                    }
                    if self.add_peer(stream).is_err() {
                        return Err(self.fail(Failure::Io));
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(self.fail(failure(&e))),
            }
        }
        Ok(())
    }
    fn tcp_poll(
        &mut self,
        cancel: &Cancellation,
        max_items: usize,
        max_bytes: usize,
    ) -> Result<Vec<Observation>, Error> {
        let mut budget = 64;
        self.accept(cancel, &mut budget)?;
        let ids: Vec<_> = self.peers.keys().copied().collect();
        if ids.is_empty() {
            return Ok(vec![]);
        }
        let offset = ids.partition_point(|id| *id <= self.cursor) % ids.len();
        let mut out = Vec::new();
        let mut remaining = max_bytes;
        let mut misses = 0;
        for attempt in 0..budget {
            if out.len() == max_items || remaining < self.snapshot || misses == ids.len() {
                break;
            }
            if cancel.is_cancelled() {
                return Err(Error::Cancelled);
            }
            let id = ids[(offset + attempt) % ids.len()];
            self.cursor = id;
            let Some(c) = self.peers.get_mut(&id) else {
                misses += 1;
                continue;
            };
            if c.info.read_closed {
                misses += 1;
                continue;
            }
            let info = c.info;
            match c
                .stream
                .read(&mut self.buffer[..self.snapshot.min(remaining)])
            {
                Ok(0) => {
                    c.info.read_closed = true;
                    let info = c.info;
                    self.event(EventKind::PeerReadClosed, Some(info), None, None);
                    if info.write_closed {
                        self.remove_peer(id, None);
                    }
                    misses += 1;
                }
                Ok(n) => {
                    self.counters.received_bytes =
                        self.counters.received_bytes.saturating_add(n as u64);
                    remaining -= n;
                    out.push(
                        Observation::new(
                            self.now(),
                            Direction::Rx,
                            Metadata::Tcp {
                                local: info.local,
                                remote: info.remote,
                                connection: id,
                            },
                            None,
                            &self.buffer[..n],
                        )
                        .map_err(|_| Error::Backend)?,
                    );
                    misses = 0;
                }
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) =>
                {
                    misses += 1
                }
                Err(e) => {
                    self.remove_peer(id, Some(failure(&e)));
                    misses += 1;
                }
            }
        }
        if out.is_empty() && self.phase == Phase::Disconnected {
            return Err(Error::Disconnected);
        }
        Ok(out)
    }
    fn udp_poll(
        &mut self,
        cancel: &Cancellation,
        max_items: usize,
        max_bytes: usize,
    ) -> Result<Vec<Observation>, Error> {
        let mut out = Vec::new();
        let mut remaining = max_bytes;
        for _ in 0..max_items {
            if remaining < self.snapshot {
                break;
            }
            if cancel.is_cancelled() {
                return Err(Error::Cancelled);
            }
            let result = self
                .udp
                .as_ref()
                .ok_or(Error::InvalidState)?
                .recv_from(&mut self.buffer);
            match result {
                Ok((n, remote)) => {
                    let captured = n.min(self.snapshot);
                    if captured < n {
                        self.counters.truncated_datagrams =
                            self.counters.truncated_datagrams.saturating_add(1);
                        self.udp_event(EventKind::DatagramTruncated, remote, None, Some(n));
                    }
                    remaining -= captured;
                    self.counters.received_datagrams =
                        self.counters.received_datagrams.saturating_add(1);
                    self.counters.received_bytes =
                        self.counters.received_bytes.saturating_add(n as u64);
                    out.push(
                        Observation::new(
                            self.now(),
                            Direction::Rx,
                            Metadata::Udp {
                                local: self.local.ok_or(Error::InvalidState)?,
                                remote,
                            },
                            Some(n),
                            &self.buffer[..captured],
                        )
                        .map_err(|_| Error::Backend)?,
                    );
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) if e.raw_os_error() == Some(10040) => {
                    self.counters.oversized_datagrams =
                        self.counters.oversized_datagrams.saturating_add(1);
                    self.known_loss = self.known_loss.saturating_add(1);
                    self.last_failure = Some(Failure::DatagramTooLarge);
                    self.event(
                        EventKind::Failure,
                        None,
                        Some(Failure::DatagramTooLarge),
                        None,
                    );
                }
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::ConnectionReset | io::ErrorKind::ConnectionRefused
                    ) =>
                {
                    let f = failure(&e);
                    self.unknown_loss = true;
                    self.last_failure = Some(f);
                    self.event(EventKind::Failure, None, Some(f), None);
                }
                Err(e) => return Err(self.fail(failure(&e))),
            }
        }
        Ok(out)
    }
}
impl Adapter for Sockets {
    fn enumerate(&mut self, limit: usize) -> Result<Vec<Descriptor>, Error> {
        if self.selected {
            return Err(Error::InvalidState);
        }
        if limit == 0 || limit > 64 {
            return Err(Error::InvalidConfig);
        }
        Ok(vec![self.descriptor()])
    }
    fn open(&mut self, id: u64) -> Result<Descriptor, Error> {
        if self.selected {
            return Err(Error::InvalidState);
        }
        if id != 0 {
            return Err(Error::InvalidConfig);
        }
        self.selected = true;
        self.phase(Phase::Open);
        Ok(self.descriptor())
    }
    fn configure(&mut self, snapshot: usize) -> Result<(), Error> {
        if !self.selected || !matches!(self.phase, Phase::Open | Phase::Stopped | Phase::Configured)
        {
            return Err(Error::InvalidState);
        }
        if snapshot == 0 || snapshot > MAX_PAYLOAD {
            return Err(Error::InvalidConfig);
        }
        self.snapshot = snapshot;
        self.phase(Phase::Configured);
        Ok(())
    }
    fn start(&mut self) -> Result<(), Error> {
        self.start_with_cancel(&Cancellation::default())
    }
    fn poll(
        &mut self,
        cancel: &Cancellation,
        max_items: usize,
        max_bytes: usize,
    ) -> Result<Vec<Observation>, Error> {
        if !self.running() {
            return Err(if self.phase == Phase::Disconnected {
                Error::Disconnected
            } else {
                Error::InvalidState
            });
        }
        if max_items == 0 || max_items > 64 || max_bytes < self.snapshot || max_bytes > MAX_PAYLOAD
        {
            return Err(Error::InvalidConfig);
        }
        if cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        if self.settings.mode.kind() == TransportKind::Tcp {
            self.tcp_poll(cancel, max_items, max_bytes)
        } else {
            self.udp_poll(cancel, max_items, max_bytes)
        }
    }
    fn stop(&mut self) -> Result<(), Error> {
        let was_running = self.running() || self.phase == Phase::Starting;
        self.phase = if self.selected {
            Phase::Stopped
        } else {
            Phase::Closed
        };
        self.release();
        if was_running {
            self.event(EventKind::Phase(self.phase), None, None, None);
        }
        Ok(())
    }
    fn close(&mut self) -> Result<(), Error> {
        self.stop()?;
        self.selected = false;
        self.snapshot = 0;
        self.phase(Phase::Closed);
        self.clock = None;
        Ok(())
    }
    fn take_backend_loss(&mut self) -> Option<Option<u64>> {
        if self.unknown_loss {
            self.unknown_loss = false;
            Some(None)
        } else if self.known_loss > 0 {
            Some(Some(std::mem::take(&mut self.known_loss)))
        } else {
            None
        }
    }
}
impl Drop for Sockets {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
