//! Typed worker-only bridge to an already explicitly selected active adapter.
use crate::{
    adapter::{Adapter, Availability, Cancellation, Error},
    can::Frame,
    sockets::{Mode, Phase, Sockets},
};
use exmon_domain::observation::Metadata;
use std::{fmt, net::SocketAddr};

#[derive(Clone, PartialEq, Eq)]
pub enum Identity {
    Serial {
        device: u64,
        revision: u64,
    },
    Hid {
        collection: u64,
        vendor: u16,
        product: u16,
        page: u16,
        usage: u16,
        numbered: bool,
        capacity: usize,
    },
    Can {
        channel: u32,
    },
    Tcp {
        epoch: u64,
        connection: u64,
        local: SocketAddr,
        remote: SocketAddr,
    },
    Udp {
        epoch: u64,
        local: SocketAddr,
        remote: SocketAddr,
    },
    Usb {
        device: usize,
        configuration: u8,
        interface: u8,
        alternate: u8,
        endpoint: u8,
        interrupt: bool,
        packet_size: usize,
    },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Serial,
    Hid,
    Can,
    Tcp,
    Udp,
    Usb,
}
impl Identity {
    pub fn kind(&self) -> Kind {
        match self {
            Self::Serial { .. } => Kind::Serial,
            Self::Hid { .. } => Kind::Hid,
            Self::Can { .. } => Kind::Can,
            Self::Tcp { .. } => Kind::Tcp,
            Self::Udp { .. } => Kind::Udp,
            Self::Usb { .. } => Kind::Usb,
        }
    }
}
impl fmt::Debug for Identity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}([REDACTED])", self.kind())
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Info {
    pub identity: Identity,
    pub writable: Availability,
    pub max_bytes: usize,
}
pub enum Packet<'a> {
    Bytes(&'a [u8]),
    Can(&'a Frame),
}
impl Packet<'_> {
    pub fn bytes(&self) -> &[u8] {
        match self {
            Self::Bytes(b) => b,
            Self::Can(f) => f.data(),
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Completion {
    Accepted,
    Timeout,
    Stall,
    Disconnected,
    Cancelled,
    Fault,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Progress {
    Pending,
    Complete {
        accepted: Option<usize>,
        status: Completion,
    },
}
impl Progress {
    pub fn accepted(n: usize) -> Self {
        Self::Complete {
            accepted: Some(n),
            status: Completion::Accepted,
        }
    }
}

/// Trusted bounded worker implementation. Factories/open belong to explicit user
/// bind actions. A Port never selects, reconnects, retries or schedules implicitly.
pub trait Port: Send {
    fn info(&self) -> Result<Info, Error>;
    /// Pure validation and exact TX metadata before any source call.
    fn metadata(&self, packet: &Packet<'_>) -> Result<Metadata, Error>;
    fn submit(&mut self, packet: &Packet<'_>, cancel: &Cancellation) -> Result<Progress, Error>;
    fn completion(&mut self) -> Result<Progress, Error> {
        Err(Error::InvalidState)
    }
    fn close(&mut self) -> Result<(), Error>;
}
pub(crate) fn bytes<'a>(packet: &Packet<'a>) -> Result<&'a [u8], Error> {
    match packet {
        Packet::Bytes(b) if b.len() <= 65536 => Ok(b),
        _ => Err(Error::InvalidConfig),
    }
}
pub(crate) fn allowed(info: &Info, cancel: &Cancellation) -> Result<(), Error> {
    if cancel.is_cancelled() {
        return Err(Error::Cancelled);
    }
    match info.writable {
        Availability::Available => Ok(()),
        Availability::Unavailable(e) => Err(e),
        Availability::Unsupported => Err(Error::Unsupported),
    }
}

#[derive(Clone, Copy)]
pub enum SocketTarget {
    Tcp(u64),
    Udp(SocketAddr),
}
pub struct SocketPort {
    adapter: Sockets,
    target: SocketTarget,
    identity: Identity,
}
impl SocketPort {
    pub fn new(adapter: Sockets, target: SocketTarget) -> Result<Self, Error> {
        let identity = Self::identity(&adapter, target)?;
        Ok(Self {
            adapter,
            target,
            identity,
        })
    }
    fn identity(adapter: &Sockets, target: SocketTarget) -> Result<Identity, Error> {
        let snapshot = adapter.snapshot();
        match target {
            SocketTarget::Tcp(connection) => {
                if !matches!(snapshot.phase, Phase::Connected | Phase::Listening) {
                    return Err(Error::InvalidState);
                }
                let peer = adapter
                    .peers()
                    .into_iter()
                    .find(|p| p.connection == connection && !p.write_closed)
                    .ok_or(Error::Disconnected)?;
                Ok(Identity::Tcp {
                    epoch: snapshot.epoch,
                    connection,
                    local: peer.local,
                    remote: peer.remote,
                })
            }
            SocketTarget::Udp(remote) => {
                if !matches!(snapshot.phase, Phase::Connected | Phase::Bound) {
                    return Err(Error::InvalidState);
                }
                if !matches!(
                    adapter.settings().mode,
                    Mode::UdpBound { .. } | Mode::UdpConnected { .. }
                ) {
                    return Err(Error::Unsupported);
                }
                let local = snapshot.local.ok_or(Error::InvalidState)?;
                if local.is_ipv4() != remote.is_ipv4()
                    || remote.port() == 0
                    || remote.ip().is_unspecified()
                    || adapter
                        .settings()
                        .mode
                        .remote()
                        .is_some_and(|r| r != remote)
                {
                    return Err(Error::InvalidConfig);
                }
                Ok(Identity::Udp {
                    epoch: snapshot.epoch,
                    local,
                    remote,
                })
            }
        }
    }
}
impl Port for SocketPort {
    fn info(&self) -> Result<Info, Error> {
        if Self::identity(&self.adapter, self.target)? != self.identity {
            return Err(Error::Disconnected);
        }
        let maximum = match &self.identity {
            Identity::Udp { remote, .. } => {
                if remote.is_ipv4() {
                    65507
                } else {
                    65527
                }
            }
            _ => 65536,
        };
        Ok(Info {
            identity: self.identity.clone(),
            writable: Availability::Available,
            max_bytes: self.adapter.settings().max_send_bytes.min(maximum),
        })
    }
    fn metadata(&self, packet: &Packet<'_>) -> Result<Metadata, Error> {
        let data = bytes(packet)?;
        let info = self.info()?;
        if data.len() > info.max_bytes {
            return Err(Error::InvalidConfig);
        }
        Ok(match info.identity {
            Identity::Tcp {
                connection,
                local,
                remote,
                ..
            } => Metadata::Tcp {
                local,
                remote,
                connection,
            },
            Identity::Udp { local, remote, .. } => Metadata::Udp { local, remote },
            _ => return Err(Error::Unsupported),
        })
    }
    fn submit(&mut self, packet: &Packet<'_>, cancel: &Cancellation) -> Result<Progress, Error> {
        allowed(&self.info()?, cancel)?;
        self.metadata(packet)?;
        let data = bytes(packet)?;
        let count = match self.identity {
            Identity::Tcp {
                epoch, connection, ..
            } => self.adapter.send_tcp(epoch, connection, data, cancel)?,
            Identity::Udp { epoch, remote, .. } => {
                self.adapter.send_udp(epoch, Some(remote), data, cancel)?
            }
            _ => return Err(Error::Unsupported),
        };
        Ok(Progress::accepted(count))
    }
    fn close(&mut self) -> Result<(), Error> {
        Adapter::close(&mut self.adapter)
    }
}
