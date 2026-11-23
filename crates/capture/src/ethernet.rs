//! Ethernet lifecycle, independent of the native packet library for offline tests.
use crate::adapter::{Adapter, Availability, Cancellation, Descriptor, Error};
use exmon_domain::{
    Direction,
    observation::{MAX_PAYLOAD, Metadata, Observation, TransportKind},
};
use std::time::Instant;

#[derive(Clone)]
pub struct Options {
    pub promiscuous: bool,
    pub timeout_ms: u32,
    pub filter: String,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            promiscuous: false,
            timeout_ms: 10,
            filter: String::new(),
        }
    }
}
impl Options {
    fn validate(&self) -> Result<(), Error> {
        if self.timeout_ms == 0
            || self.timeout_ms > 1000
            || self.filter.len() > 4096
            || self.filter.contains('\0')
        {
            Err(Error::InvalidConfig)
        } else {
            Ok(())
        }
    }
}
pub struct RawFrame {
    pub timestamp_ns: u64,
    pub original_length: usize,
    pub bytes: Vec<u8>,
}
/// Trusted backend. Native next() must be nonblocking and bound copies by snaplen.
pub trait Source: Send {
    /// Compile without discovering/opening a live interface. Unsupported permits
    /// an explicitly displayed post-capture-only fallback, never a silent error.
    fn validate_filter(&mut self, filter: &str) -> Result<(), Error> {
        if filter.is_empty() {
            Ok(())
        } else {
            Err(Error::Unsupported)
        }
    }
    fn list(&mut self, maximum: usize) -> Result<Vec<String>, Error>;
    fn activate(&mut self, name: &str, snapshot: usize, options: &Options) -> Result<u32, Error>;
    fn next(&mut self) -> Result<Option<RawFrame>, Error>;
    fn close(&mut self);
    fn loss(&mut self) -> Option<Option<u64>> {
        None
    }
}
pub struct Ethernet<S: Source> {
    source: S,
    options: Options,
    names: Vec<String>,
    selected: Option<usize>,
    snapshot: usize,
    link_type: Option<u32>,
    clock: Option<Instant>,
}
impl<S: Source> Ethernet<S> {
    /// Local UI only: device names may be sensitive and must not enter logs.
    pub fn interface_name(&self, id: u64) -> Option<&str> {
        self.names
            .get(usize::try_from(id).ok()?)
            .map(String::as_str)
    }
    pub fn new(source: S, options: Options) -> Result<Self, Error> {
        options.validate()?;
        Ok(Self {
            source,
            options,
            names: vec![],
            selected: None,
            snapshot: 0,
            link_type: None,
            clock: None,
        })
    }
    fn descriptor(id: u64) -> Descriptor {
        Descriptor {
            id,
            kind: TransportKind::Ethernet,
            rx: Availability::Available,
            tx: Availability::Unsupported,
        }
    }
}
impl<S: Source> Adapter for Ethernet<S> {
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
            .map(|id| Self::descriptor(id as u64))
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
        self.snapshot = 0;
        Ok(Self::descriptor(id))
    }
    fn configure(&mut self, snapshot: usize) -> Result<(), Error> {
        if self.selected.is_none() || self.link_type.is_some() {
            return Err(Error::InvalidState);
        }
        if snapshot == 0 || snapshot > MAX_PAYLOAD {
            return Err(Error::InvalidConfig);
        }
        self.snapshot = snapshot;
        Ok(())
    }
    fn start(&mut self) -> Result<(), Error> {
        if self.link_type.is_some() || self.snapshot == 0 {
            return Err(Error::InvalidState);
        }
        let index = self.selected.ok_or(Error::InvalidState)?;
        let link = self
            .source
            .activate(&self.names[index], self.snapshot, &self.options)?;
        // This adapter promises Ethernet, not a guessed interpretation of SLL/loopback.
        if link != 1 {
            self.source.close();
            return Err(Error::Unsupported);
        }
        self.link_type = Some(link);
        self.clock = Some(Instant::now());
        Ok(())
    }
    fn poll(
        &mut self,
        cancellation: &Cancellation,
        max_items: usize,
        max_bytes: usize,
    ) -> Result<Vec<Observation>, Error> {
        let link_type = self.link_type.ok_or(Error::InvalidState)?;
        if max_items == 0 || max_items > 64 || max_bytes < self.snapshot || max_bytes > MAX_PAYLOAD
        {
            return Err(Error::InvalidConfig);
        }
        let mut observations = Vec::new();
        let mut remaining = max_bytes;
        while observations.len() < max_items && remaining >= self.snapshot {
            if cancellation.is_cancelled() {
                return Err(Error::Cancelled);
            }
            let Some(frame) = self.source.next()? else {
                break;
            };
            if frame.bytes.len() > self.snapshot {
                return Err(Error::Backend);
            }
            remaining -= frame.bytes.len();
            let ingest_ns =
                u64::try_from(self.clock.ok_or(Error::InvalidState)?.elapsed().as_nanos())
                    .map_err(|_| Error::Exhausted)?;
            observations.push(
                Observation::new(
                    ingest_ns,
                    Direction::Unknown,
                    Metadata::Ethernet { link_type },
                    Some(frame.original_length),
                    &frame.bytes,
                )
                .map_err(|_| Error::Backend)?
                .with_source_timestamp(frame.timestamp_ns),
            );
        }
        Ok(observations)
    }
    fn stop(&mut self) -> Result<(), Error> {
        self.source.close();
        self.link_type = None;
        Ok(())
    }
    fn close(&mut self) -> Result<(), Error> {
        self.stop()?;
        self.selected = None;
        self.snapshot = 0;
        Ok(())
    }
    fn take_backend_loss(&mut self) -> Option<Option<u64>> {
        self.source.loss()
    }
}
