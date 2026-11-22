//! Serial byte-stream adapter; electrical safety remains device-specific.
use crate::adapter::{Adapter, Availability, Cancellation, Descriptor, Error};
use exmon_domain::{
    Direction,
    observation::{MAX_PAYLOAD, Metadata, Observation, TransportKind},
};
pub use serial2::{CharSize, FlowControl, Parity, StopBits};
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElectricalMode {
    Unchanged,
    Rs422,
    Rs485 {
        full_duplex: bool,
        invert_rts: bool,
        before_ms: u32,
        after_ms: u32,
    },
}
#[derive(Debug, Clone, Copy)]
pub struct Settings {
    pub baud: u32,
    pub data_bits: CharSize,
    pub parity: Parity,
    pub stop_bits: StopBits,
    pub flow: FlowControl,
    pub timeout_ms: u32,
    pub electrical: ElectricalMode,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            baud: 9600,
            data_bits: CharSize::Bits8,
            parity: Parity::None,
            stop_bits: StopBits::One,
            flow: FlowControl::None,
            timeout_ms: 10,
            electrical: ElectricalMode::Unchanged,
        }
    }
}
impl Settings {
    pub fn validate(&self) -> Result<(), Error> {
        if self.baud == 0 || self.baud > 4_000_000 || self.timeout_ms == 0 || self.timeout_ms > 1000
        {
            return Err(Error::InvalidConfig);
        }
        if let ElectricalMode::Rs485 {
            before_ms,
            after_ms,
            ..
        } = self.electrical
            && (before_ms > 1000 || after_ms > 1000 || self.flow != FlowControl::None)
        {
            return Err(Error::InvalidConfig);
        }
        Ok(())
    }
}
pub trait Source: Send {
    fn list(&mut self, limit: usize) -> Result<Vec<String>, Error>;
    fn open(&mut self, name: &str, settings: Settings) -> Result<(), Error>;
    fn read(&mut self, buffer: &mut [u8]) -> Result<usize, Error>;
    fn write(&mut self, bytes: &[u8]) -> Result<usize, Error>;
    fn close(&mut self);
    fn direction_control(&self) -> Availability {
        Availability::Unsupported
    }
}
pub struct Serial<S: Source> {
    source: S,
    settings: Settings,
    names: Vec<String>,
    selected: Option<usize>,
    maximum: usize,
    clock: Option<Instant>,
    revision: u64,
}
impl<S: Source> Drop for Serial<S> {
    fn drop(&mut self) {
        self.source.close();
    }
}
impl<S: Source> Serial<S> {
    pub fn new(source: S, settings: Settings) -> Result<Self, Error> {
        settings.validate()?;
        Ok(Self {
            source,
            settings,
            names: vec![],
            selected: None,
            maximum: 0,
            clock: None,
            revision: 0,
        })
    }
    pub fn direction_control(&self) -> Availability {
        self.source.direction_control()
    }
    pub fn port_name(&self, id: u64) -> Option<&str> {
        self.names
            .get(usize::try_from(id).ok()?)
            .map(String::as_str)
    }
    fn descriptor(id: u64) -> Descriptor {
        Descriptor {
            id,
            kind: TransportKind::Serial,
            rx: Availability::Available,
            tx: Availability::Available,
        }
    }
}
impl<S: Source> Adapter for Serial<S> {
    fn enumerate(&mut self, limit: usize) -> Result<Vec<Descriptor>, Error> {
        if self.selected.is_some() {
            return Err(Error::InvalidState);
        }
        if limit == 0 || limit > 64 {
            return Err(Error::InvalidConfig);
        }
        let names = self.source.list(limit)?;
        if names.len() > limit
            || names
                .iter()
                .any(|n| n.is_empty() || n.len() > 256 || n.contains('\0'))
        {
            return Err(Error::Backend);
        }
        self.names = names;
        Ok((0..self.names.len())
            .map(|n| Self::descriptor(n as u64))
            .collect())
    }
    fn open(&mut self, id: u64) -> Result<Descriptor, Error> {
        if self.selected.is_some() {
            return Err(Error::InvalidState);
        }
        let index = usize::try_from(id).map_err(|_| Error::InvalidConfig)?;
        if index >= self.names.len() {
            return Err(Error::InvalidConfig);
        }
        self.selected = Some(index);
        self.maximum = 0;
        Ok(Self::descriptor(id))
    }
    fn configure(&mut self, maximum: usize) -> Result<(), Error> {
        if self.selected.is_none() || self.clock.is_some() {
            return Err(Error::InvalidState);
        }
        if maximum == 0 || maximum > MAX_PAYLOAD {
            return Err(Error::InvalidConfig);
        }
        self.maximum = maximum;
        Ok(())
    }
    fn start(&mut self) -> Result<(), Error> {
        if self.clock.is_some() || self.maximum == 0 {
            return Err(Error::InvalidState);
        }
        let index = self.selected.ok_or(Error::InvalidState)?;
        let revision = self.revision.checked_add(1).ok_or(Error::Exhausted)?;
        self.source.open(&self.names[index], self.settings)?;
        self.revision = revision;
        self.clock = Some(Instant::now());
        Ok(())
    }
    fn poll(
        &mut self,
        cancel: &Cancellation,
        max_items: usize,
        max_bytes: usize,
    ) -> Result<Vec<Observation>, Error> {
        let clock = self.clock.ok_or(Error::InvalidState)?;
        if cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        if max_items == 0 || max_items > 64 || max_bytes == 0 || max_bytes > MAX_PAYLOAD {
            return Err(Error::InvalidConfig);
        }
        // One finite read per poll: no empty-read busy loop or unbounded aggregate.
        let mut bytes = vec![0; self.maximum.min(max_bytes)];
        let length = match self.source.read(&mut bytes) {
            Ok(n) => n,
            Err(e) => {
                self.stop()?;
                return Err(e);
            }
        };
        if length > bytes.len() {
            self.stop()?;
            return Err(Error::Backend);
        }
        if cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        if length == 0 {
            return Ok(vec![]);
        }
        let ns = u64::try_from(clock.elapsed().as_nanos()).map_err(|_| Error::Exhausted)?;
        Ok(vec![
            Observation::new(
                ns,
                Direction::Rx,
                Metadata::Serial {
                    configuration_revision: self.revision,
                },
                Some(length),
                &bytes[..length],
            )
            .map_err(|_| Error::Backend)?,
        ])
    }
    fn transmit(&mut self, bytes: &[u8]) -> Result<usize, Error> {
        if self.clock.is_none() {
            return Err(Error::InvalidState);
        }
        if bytes.is_empty() || bytes.len() > MAX_PAYLOAD {
            return Err(Error::InvalidConfig);
        }
        // Explicit single write; partial results are never silently retried.
        match self.source.write(bytes) {
            Ok(n) if n <= bytes.len() => Ok(n),
            Ok(_) => {
                self.stop()?;
                Err(Error::Backend)
            }
            Err(e) => {
                self.stop()?;
                Err(e)
            }
        }
    }
    fn stop(&mut self) -> Result<(), Error> {
        self.source.close();
        self.clock = None;
        Ok(())
    }
    fn close(&mut self) -> Result<(), Error> {
        self.stop()?;
        self.selected = None;
        self.maximum = 0;
        Ok(())
    }
}
