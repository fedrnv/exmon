//! Explicitly synthetic observations for the unattended desktop prototype.
use crate::adapter::*;
use exmon_domain::{
    Direction,
    observation::{Metadata, Observation, TransportKind},
};
use std::{
    net::SocketAddr,
    thread::{self, ThreadId},
    time::Instant,
};
pub struct Synthetic {
    kind: TransportKind,
    selected: Option<u64>,
    snapshot: usize,
    clock: Option<Instant>,
    sequence: u64,
    owner: ThreadId,
}
impl Synthetic {
    pub fn new(kind: TransportKind) -> Self {
        Self {
            kind,
            selected: None,
            snapshot: 0,
            clock: None,
            sequence: 0,
            owner: thread::current().id(),
        }
    }
    pub fn owner(&self) -> ThreadId {
        self.owner
    }
    fn descriptor(&self, id: u64) -> Descriptor {
        Descriptor {
            id,
            kind: self.kind,
            rx: Availability::Available,
            tx: Availability::Unsupported,
        }
    }
}
impl Adapter for Synthetic {
    fn enumerate(&mut self, limit: usize) -> Result<Vec<Descriptor>, Error> {
        if limit == 0 || limit > 64 {
            return Err(Error::InvalidConfig);
        }
        Ok((1..=2).take(limit).map(|id| self.descriptor(id)).collect())
    }
    fn open(&mut self, id: u64) -> Result<Descriptor, Error> {
        if self.selected.is_some() {
            return Err(Error::InvalidState);
        }
        if !(1..=2).contains(&id) {
            return Err(Error::InvalidConfig);
        }
        self.selected = Some(id);
        Ok(self.descriptor(id))
    }
    fn configure(&mut self, snapshot: usize) -> Result<(), Error> {
        if self.selected.is_none() || self.clock.is_some() {
            return Err(Error::InvalidState);
        }
        if snapshot == 0 || snapshot > 64 {
            return Err(Error::InvalidConfig);
        }
        self.snapshot = snapshot;
        Ok(())
    }
    fn start(&mut self) -> Result<(), Error> {
        if self.selected.is_none() || self.snapshot == 0 || self.clock.is_some() {
            return Err(Error::InvalidState);
        }
        self.clock = Some(Instant::now());
        self.sequence = 0;
        Ok(())
    }
    fn poll(
        &mut self,
        cancel: &Cancellation,
        max_items: usize,
        max_bytes: usize,
    ) -> Result<Vec<Observation>, Error> {
        if thread::current().id() != self.owner {
            return Err(Error::Backend);
        }
        let clock = self.clock.ok_or(Error::InvalidState)?;
        if max_items == 0 || max_items > 64 || max_bytes < self.snapshot || max_bytes > 65536 {
            return Err(Error::InvalidConfig);
        }
        let mut result = vec![];
        let mut remaining = max_bytes;
        for _ in 0..max_items {
            if cancel.is_cancelled() {
                return Err(Error::Cancelled);
            }
            let len = if self.kind == TransportKind::Can {
                8
            } else {
                32
            };
            let count = len.min(self.snapshot);
            if remaining < count {
                break;
            }
            let mut bytes = [0; 32];
            bytes[0] = 0xa5;
            bytes[1] = self.selected.ok_or(Error::InvalidState)? as u8;
            bytes[2..10].copy_from_slice(&self.sequence.to_le_bytes());
            let local = SocketAddr::from(([127, 0, 0, 1], 12000));
            let remote = SocketAddr::from(([127, 0, 0, 1], 12001));
            let metadata = match self.kind {
                TransportKind::Ethernet => Metadata::Ethernet { link_type: 1 },
                TransportKind::Tcp => Metadata::Tcp {
                    local,
                    remote,
                    connection: 1,
                },
                TransportKind::Udp => Metadata::Udp { local, remote },
                TransportKind::Serial => Metadata::Serial {
                    configuration_revision: 1,
                },
                TransportKind::Usb => Metadata::Usb {
                    endpoint: 0x81,
                    completed: true,
                },
                TransportKind::Can => Metadata::Can {
                    channel: 1,
                    id: 0x123,
                    extended: false,
                    fd: false,
                    remote: false,
                    error: false,
                    dlc: 8,
                },
            };
            let time = u64::try_from(clock.elapsed().as_nanos()).map_err(|_| Error::Exhausted)?;
            result.push(
                Observation::new(time, Direction::Rx, metadata, Some(len), &bytes[..count])
                    .map_err(|_| Error::Backend)?,
            );
            remaining -= count;
            self.sequence = self.sequence.checked_add(1).ok_or(Error::Exhausted)?;
        }
        Ok(result)
    }
    fn stop(&mut self) -> Result<(), Error> {
        self.clock = None;
        Ok(())
    }
    fn close(&mut self) -> Result<(), Error> {
        self.stop()?;
        self.selected = None;
        self.snapshot = 0;
        Ok(())
    }
}
