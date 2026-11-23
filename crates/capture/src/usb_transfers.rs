//! Completion-based USB API: submission is never reported as delivery.
use crate::adapter::Error;
use std::time::{Duration, Instant};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Bulk,
    Interrupt,
    Control,
    Isochronous,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    In,
    Out,
}
impl Direction {
    pub fn index(self) -> usize {
        match self {
            Self::In => 0,
            Self::Out => 1,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Success,
    Timeout,
    Stall,
    Disconnected,
    Cancelled,
    Fault,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Device {
    pub vendor_id: u16,
    pub product_id: u16,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Endpoint {
    pub address: u8,
    pub kind: Kind,
    pub packet_size: usize,
}
#[derive(Debug, Clone)]
pub struct Interface {
    pub number: u8,
    pub alternate: u8,
    pub endpoints: Vec<Endpoint>,
}
#[derive(Debug, Clone)]
pub struct Configuration {
    pub value: u8,
    pub interfaces: Vec<Interface>,
}
#[derive(Debug, Clone)]
pub struct Descriptors {
    pub active: u8,
    pub configurations: Vec<Configuration>,
}
impl Descriptors {
    pub fn validate(&self) -> Result<(), Error> {
        if self.configurations.len() > 16 {
            return Err(Error::Backend);
        }
        let mut configurations = std::collections::BTreeSet::new();
        for c in &self.configurations {
            if !configurations.insert(c.value) {
                return Err(Error::Backend);
            }
            if c.value == 0 || c.interfaces.len() > 32 {
                return Err(Error::Backend);
            }
            let mut seen = std::collections::BTreeSet::new();
            for i in &c.interfaces {
                if !seen.insert((i.number, i.alternate)) || i.endpoints.len() > 30 {
                    return Err(Error::Backend);
                }
                let mut endpoints = std::collections::BTreeSet::new();
                for e in &i.endpoints {
                    if e.address & 0x70 != 0
                        || e.address & 0x0f == 0
                        || e.packet_size == 0
                        || e.packet_size > 1024
                        || !endpoints.insert(e.address)
                    {
                        return Err(Error::Backend);
                    }
                }
            }
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Copy)]
pub struct Selection {
    pub configuration: u8,
    pub interface: u8,
    pub alternate: u8,
    pub input: Option<u8>,
    pub output: Option<u8>,
}
pub struct Completion {
    pub direction: Direction,
    pub endpoint: u8,
    pub bytes: Vec<u8>,
    pub actual_length: Option<usize>,
    pub status: Status,
}
pub trait Source: Send {
    fn enumerate(&mut self, maximum: usize) -> Result<Vec<Device>, Error>;
    fn inspect(&mut self, index: usize) -> Result<Descriptors, Error>;
    fn claim(&mut self, selection: Selection) -> Result<(), Error>;
    fn submit(
        &mut self,
        direction: Direction,
        endpoint: Endpoint,
        bytes: &[u8],
        requested: usize,
    ) -> Result<(), Error>;
    fn poll(&mut self, direction: Direction) -> Result<Option<Completion>, Error>;
    fn release(&mut self);
}
pub struct Usb<S: Source> {
    source: S,
    inspected_device: Option<usize>,
    count: usize,
    descriptors: Option<Descriptors>,
    selected: Option<Selection>,
    endpoints: [Option<Endpoint>; 2],
    pending: [Option<Instant>; 2],
    requested: [usize; 2],
    aborted: [Option<Completion>; 2],
    timeout: Duration,
}
impl<S: Source> Usb<S> {
    pub fn new(source: S, timeout: Duration) -> Result<Self, Error> {
        if timeout.is_zero() || timeout > Duration::from_secs(1) {
            return Err(Error::InvalidConfig);
        }
        Ok(Self {
            source,
            inspected_device: None,
            count: 0,
            descriptors: None,
            selected: None,
            endpoints: [None, None],
            pending: [None, None],
            requested: [0, 0],
            aborted: [None, None],
            timeout,
        })
    }
    pub fn enumerate(&mut self) -> Result<Vec<Device>, Error> {
        if self.selected.is_some() {
            return Err(Error::InvalidState);
        }
        self.count = 0;
        self.inspected_device = None;
        self.descriptors = None;
        let devices = self.source.enumerate(64)?;
        if devices.len() > 64 {
            return Err(Error::Backend);
        }
        self.count = devices.len();
        self.descriptors = None;
        Ok(devices)
    }
    /// Explicit device open to inspect descriptors, without interface claims/transfers.
    pub fn inspect(&mut self, index: usize) -> Result<&Descriptors, Error> {
        if self.selected.is_some() {
            return Err(Error::InvalidState);
        }
        if index >= self.count {
            return Err(Error::InvalidConfig);
        }
        self.descriptors = None;
        self.inspected_device = None;
        let descriptors = match self.source.inspect(index) {
            Ok(value) => value,
            Err(error) => {
                self.source.release();
                return Err(error);
            }
        };
        if let Err(error) = descriptors.validate() {
            self.source.release();
            return Err(error);
        }
        self.descriptors = Some(descriptors);
        self.inspected_device = Some(index);
        Ok(self.descriptors.as_ref().unwrap())
    }
    pub fn claim(&mut self, selection: Selection) -> Result<(), Error> {
        if self.selected.is_some() || self.aborted.iter().any(Option::is_some) {
            return Err(Error::InvalidState);
        }
        let descriptors = self.descriptors.as_ref().ok_or(Error::InvalidState)?;
        if selection.configuration != descriptors.active {
            return Err(Error::Unsupported);
        }
        let interface = descriptors
            .configurations
            .iter()
            .find(|c| c.value == selection.configuration)
            .and_then(|c| {
                c.interfaces
                    .iter()
                    .find(|i| i.number == selection.interface && i.alternate == selection.alternate)
            })
            .ok_or(Error::InvalidConfig)?;
        if selection.input.is_none() && selection.output.is_none() {
            return Err(Error::InvalidConfig);
        }
        let mut endpoints = [None, None];
        for (dir, address) in [
            (Direction::In, selection.input),
            (Direction::Out, selection.output),
        ] {
            if let Some(address) = address {
                if (address & 0x80 != 0) != (dir == Direction::In) {
                    return Err(Error::InvalidConfig);
                }
                let endpoint = *interface
                    .endpoints
                    .iter()
                    .find(|e| e.address == address)
                    .ok_or(Error::InvalidConfig)?;
                if !matches!(endpoint.kind, Kind::Bulk | Kind::Interrupt) {
                    return Err(Error::Unsupported);
                }
                endpoints[dir.index()] = Some(endpoint);
            }
        }
        self.source.claim(selection)?;
        self.selected = Some(selection);
        self.endpoints = endpoints;
        self.aborted = [None, None];
        Ok(())
    }
    fn submit(&mut self, dir: Direction, bytes: &[u8], requested: usize) -> Result<(), Error> {
        if self.selected.is_none()
            || self.pending[dir.index()].is_some()
            || self.aborted[dir.index()].is_some()
        {
            return Err(Error::InvalidState);
        }
        let endpoint = self.endpoints[dir.index()].ok_or(Error::Unsupported)?;
        if requested > 65536
            || (dir == Direction::In
                && (requested == 0 || !requested.is_multiple_of(endpoint.packet_size)))
        {
            return Err(Error::InvalidConfig);
        }
        self.source.submit(dir, endpoint, bytes, requested)?;
        self.requested[dir.index()] = requested;
        self.pending[dir.index()] = Some(Instant::now());
        Ok(())
    }
    pub fn submit_in(&mut self, length: usize) -> Result<(), Error> {
        self.submit(Direction::In, &[], length)
    }
    pub fn submit_out(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.submit(Direction::Out, bytes, bytes.len())
    }
    pub fn poll(&mut self, dir: Direction) -> Result<Option<Completion>, Error> {
        let n = dir.index();
        if let Some(done) = self.aborted[n].take() {
            return Ok(Some(done));
        }
        let started = self.pending[n].ok_or(Error::InvalidState)?;
        let completion = match self.source.poll(dir) {
            Ok(value) => value,
            Err(error) => {
                self.stop();
                return Err(error);
            }
        };
        if let Some(done) = completion {
            if done.direction != dir
                || Some(done.endpoint) != self.endpoints[n].map(|e| e.address)
                || done
                    .actual_length
                    .is_none_or(|len| len > self.requested[n] || len != done.bytes.len())
            {
                self.stop();
                return Err(Error::Backend);
            }
            self.pending[n] = None;
            if done.status != Status::Success {
                self.stop();
            }
            return Ok(Some(done));
        }
        if started.elapsed() >= self.timeout {
            self.stop();
            if let Some(done) = self.aborted[n].as_mut() {
                done.status = Status::Timeout;
            }
            return Ok(self.aborted[n].take());
        }
        Ok(None)
    }
    pub fn stop(&mut self) {
        self.source.release();
        for n in 0..2 {
            if self.pending[n].take().is_some() {
                self.aborted[n] = Some(Completion {
                    direction: if n == 0 {
                        Direction::In
                    } else {
                        Direction::Out
                    },
                    endpoint: self.endpoints[n].unwrap().address,
                    bytes: vec![],
                    actual_length: None,
                    status: Status::Cancelled,
                });
            }
        }
        self.selected = None;
        self.inspected_device = None;
        self.endpoints = [None, None];
        self.descriptors = None;
    }
}
impl<S: Source> crate::transmit::Port for Usb<S> {
    fn info(&self) -> Result<crate::transmit::Info, Error> {
        let selection = self.selected.ok_or(Error::InvalidState)?;
        let out = self.endpoints[Direction::Out.index()].ok_or(Error::Unsupported)?;
        Ok(crate::transmit::Info {
            identity: crate::transmit::Identity::Usb {
                device: self.inspected_device.ok_or(Error::InvalidState)?,
                configuration: selection.configuration,
                interface: selection.interface,
                alternate: selection.alternate,
                endpoint: out.address,
                interrupt: out.kind == Kind::Interrupt,
                packet_size: out.packet_size,
            },
            writable: crate::adapter::Availability::Available,
            max_bytes: 65536,
        })
    }
    fn metadata(
        &self,
        packet: &crate::transmit::Packet<'_>,
    ) -> Result<exmon_domain::observation::Metadata, Error> {
        crate::transmit::bytes(packet)?;
        let crate::transmit::Identity::Usb { endpoint, .. } = self.info()?.identity else {
            return Err(Error::Backend);
        };
        Ok(exmon_domain::observation::Metadata::Usb {
            endpoint,
            completed: false,
        })
    }
    fn submit(
        &mut self,
        packet: &crate::transmit::Packet<'_>,
        cancel: &crate::adapter::Cancellation,
    ) -> Result<crate::transmit::Progress, Error> {
        crate::transmit::allowed(&self.info()?, cancel)?;
        self.metadata(packet)?;
        self.submit_out(crate::transmit::bytes(packet)?)?;
        Ok(crate::transmit::Progress::Pending)
    }
    fn completion(&mut self) -> Result<crate::transmit::Progress, Error> {
        use crate::transmit::{Completion as C, Progress};
        Ok(match self.poll(Direction::Out)? {
            None => Progress::Pending,
            Some(c) => Progress::Complete {
                accepted: c.actual_length,
                status: match c.status {
                    Status::Success => C::Accepted,
                    Status::Timeout => C::Timeout,
                    Status::Stall => C::Stall,
                    Status::Disconnected => C::Disconnected,
                    Status::Cancelled => C::Cancelled,
                    Status::Fault => C::Fault,
                },
            },
        })
    }
    fn close(&mut self) -> Result<(), Error> {
        self.stop();
        Ok(())
    }
}
impl<S: Source> Drop for Usb<S> {
    fn drop(&mut self) {
        self.source.release();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Pending;
    impl Source for Pending {
        fn enumerate(&mut self, _: usize) -> Result<Vec<Device>, Error> {
            Ok(vec![])
        }
        fn inspect(&mut self, _: usize) -> Result<Descriptors, Error> {
            Err(Error::Unsupported)
        }
        fn claim(&mut self, _: Selection) -> Result<(), Error> {
            Ok(())
        }
        fn submit(&mut self, _: Direction, _: Endpoint, _: &[u8], _: usize) -> Result<(), Error> {
            Ok(())
        }
        fn poll(&mut self, _: Direction) -> Result<Option<Completion>, Error> {
            Ok(None)
        }
        fn release(&mut self) {}
    }
    #[test]
    fn expired_deadline_is_timeout_not_zero_byte_success() {
        let mut usb = Usb::new(Pending, Duration::from_millis(10)).unwrap();
        usb.pending[0] = Some(Instant::now() - Duration::from_secs(1));
        usb.endpoints[0] = Some(Endpoint {
            address: 0x81,
            kind: Kind::Bulk,
            packet_size: 8,
        });
        let done = usb.poll(Direction::In).unwrap().unwrap();
        assert_eq!(done.status, Status::Timeout);
        assert_eq!(done.actual_length, None);
        assert!(usb.pending.iter().all(Option::is_none));
        assert!(usb.submit_in(8).is_err());
    }
}
