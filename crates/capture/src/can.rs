//! Classic CAN worker contract. Byte-only transmit cannot specify a CAN frame.
use crate::adapter::{Adapter, Availability, Cancellation, Descriptor, Error};
use exmon_domain::{
    Direction,
    observation::{MAX_PAYLOAD, Metadata, Observation, TransportKind},
};
use std::time::Instant;
pub const EFF: u32 = 0x8000_0000;
pub const RTR: u32 = 0x4000_0000;
pub const ERR: u32 = 0x2000_0000;
pub const ID_MASK: u32 = 0x1fff_ffff;

#[derive(Clone, PartialEq, Eq)]
pub struct Frame {
    id_word: u32,
    dlc: u8,
    data: Vec<u8>,
}
impl Frame {
    /// Raw SocketCAN flags, wire DLC (including classic len8 DLC 9..15), exact data.
    pub fn new(id_word: u32, dlc: u8, data: &[u8]) -> Result<Self, Error> {
        let error = id_word & ERR != 0;
        let remote = id_word & RTR != 0;
        if dlc > 15
            || (id_word & EFF == 0 && !error && id_word & ID_MASK > 0x7ff)
            || (remote && (error || !data.is_empty()))
            || (!remote && data.len() != usize::from(dlc.min(8)))
            || (error && (id_word & EFF != 0 || dlc != 8))
        {
            return Err(Error::InvalidConfig);
        }
        Ok(Self {
            id_word,
            dlc,
            data: data.into(),
        })
    }
    pub fn id_word(&self) -> u32 {
        self.id_word
    }
    pub fn dlc(&self) -> u8 {
        self.dlc
    }
    pub fn data(&self) -> &[u8] {
        &self.data
    }
    pub fn remote(&self) -> bool {
        self.id_word & RTR != 0
    }
    pub fn error(&self) -> bool {
        self.id_word & ERR != 0
    }
}
#[derive(Clone, Copy)]
pub struct Filter {
    pub id: u32,
    pub mask: u32,
}
impl Filter {
    pub fn matches(&self, frame: &Frame) -> bool {
        frame.id_word & self.mask == self.id & self.mask
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Classic,
    Fd,
}
#[derive(Clone)]
pub struct Options {
    pub filters: Vec<Filter>,
    pub receive_errors: bool,
    pub mode: Mode,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            filters: vec![Filter { id: 0, mask: 0 }],
            receive_errors: false,
            mode: Mode::Classic,
        }
    }
}
impl Options {
    pub fn validate(&self) -> Result<(), Error> {
        if self.mode != Mode::Classic {
            return Err(Error::Unsupported);
        }
        // ERR in filter ID is SocketCAN's inversion flag; unsupported explicitly.
        if self.filters.len() > 64 || self.filters.iter().any(|f| (f.id | f.mask) & ERR != 0) {
            return Err(Error::InvalidConfig);
        }
        Ok(())
    }
    pub fn accepts(&self, frame: &Frame) -> bool {
        if frame.error() {
            self.receive_errors
        } else {
            self.filters.iter().any(|f| f.matches(frame))
        }
    }
}
pub struct Channel {
    pub id: u32,
    pub name: String,
    pub tx: Availability,
}
pub struct Received {
    pub frame: Frame,
    pub direction: Direction,
    pub source_timestamp_ns: Option<u64>,
}
/// Trusted source: bounded discovery, finite nonblocking read/write, no auto setup.
pub trait Source: Send {
    fn list(&mut self, limit: usize) -> Result<Vec<Channel>, Error>;
    fn activate(&mut self, channel: u32, options: &Options) -> Result<(), Error>;
    fn next(&mut self) -> Result<Option<Received>, Error>;
    fn send(&mut self, frame: &Frame) -> Result<(), Error>;
    fn close(&mut self);
    fn loss(&mut self) -> Option<Option<u64>> {
        None
    }
}
pub struct Can<S: Source> {
    source: S,
    options: Options,
    channels: Vec<Channel>,
    selected: Option<usize>,
    snapshot: usize,
    clock: Option<Instant>,
}
impl<S: Source> crate::transmit::Port for Can<S> {
    fn info(&self) -> Result<crate::transmit::Info, Error> {
        if self.clock.is_none() {
            return Err(Error::InvalidState);
        }
        let channel = self
            .channels
            .get(self.selected.ok_or(Error::InvalidState)?)
            .ok_or(Error::Backend)?;
        Ok(crate::transmit::Info {
            identity: crate::transmit::Identity::Can {
                channel: channel.id,
            },
            writable: channel.tx,
            max_bytes: 8,
        })
    }
    fn metadata(&self, packet: &crate::transmit::Packet<'_>) -> Result<Metadata, Error> {
        let crate::transmit::Packet::Can(frame) = packet else {
            return Err(Error::InvalidConfig);
        };
        let crate::transmit::Identity::Can { channel } = self.info()?.identity else {
            return Err(Error::Backend);
        };
        if frame.error() {
            return Err(Error::Unsupported);
        }
        Ok(Metadata::Can {
            channel,
            id: frame.id_word() & ID_MASK,
            extended: frame.id_word() & EFF != 0,
            fd: false,
            remote: frame.remote(),
            error: false,
            dlc: frame.dlc(),
        })
    }
    fn submit(
        &mut self,
        packet: &crate::transmit::Packet<'_>,
        cancel: &Cancellation,
    ) -> Result<crate::transmit::Progress, Error> {
        crate::transmit::allowed(&self.info()?, cancel)?;
        self.metadata(packet)?;
        let crate::transmit::Packet::Can(frame) = packet else {
            return Err(Error::InvalidConfig);
        };
        self.send_frame(frame, cancel)?;
        Ok(crate::transmit::Progress::accepted(frame.data().len()))
    }
    fn close(&mut self) -> Result<(), Error> {
        Adapter::close(self)
    }
}
impl<S: Source> Can<S> {
    pub fn new(source: S, options: Options) -> Result<Self, Error> {
        options.validate()?;
        Ok(Self {
            source,
            options,
            channels: vec![],
            selected: None,
            snapshot: 0,
            clock: None,
        })
    }
    pub fn channel_name(&self, id: u64) -> Option<&str> {
        self.channels
            .iter()
            .find(|c| u64::from(c.id) == id)
            .map(|c| c.name.as_str())
    }
    fn descriptor(c: &Channel) -> Descriptor {
        Descriptor {
            id: u64::from(c.id),
            kind: TransportKind::Can,
            rx: Availability::Available,
            tx: c.tx,
        }
    }
    /// One explicit worker send. Caller must check current user/epoch authority.
    /// Success means driver acceptance, never electrical delivery or bus ACK.
    pub fn send_frame(&mut self, frame: &Frame, cancellation: &Cancellation) -> Result<(), Error> {
        if cancellation.is_cancelled() {
            return Err(Error::Cancelled);
        }
        if self.clock.is_none() {
            return Err(Error::InvalidState);
        }
        if frame.error() {
            return Err(Error::Unsupported);
        }
        let channel = &self.channels[self.selected.ok_or(Error::InvalidState)?];
        let result = match channel.tx {
            Availability::Available => self.source.send(frame),
            Availability::Unavailable(e) => Err(e),
            Availability::Unsupported => Err(Error::Unsupported),
        };
        if matches!(
            result,
            Err(Error::Disconnected | Error::PermissionDenied | Error::Backend)
        ) {
            self.stop()?;
        }
        result
    }
}
impl<S: Source> Adapter for Can<S> {
    fn enumerate(&mut self, limit: usize) -> Result<Vec<Descriptor>, Error> {
        if self.selected.is_some() {
            return Err(Error::InvalidState);
        }
        if limit == 0 || limit > 64 {
            return Err(Error::InvalidConfig);
        }
        let channels = self.source.list(limit)?;
        if channels.len() > limit
            || channels.iter().any(|c| {
                c.id == 0 || c.name.is_empty() || c.name.len() > 64 || c.name.contains('\0')
            })
            || channels
                .iter()
                .enumerate()
                .any(|(i, c)| channels[..i].iter().any(|other| other.id == c.id))
        {
            return Err(Error::Backend);
        }
        self.channels = channels;
        Ok(self.channels.iter().map(Self::descriptor).collect())
    }
    fn open(&mut self, id: u64) -> Result<Descriptor, Error> {
        if self.selected.is_some() {
            return Err(Error::InvalidState);
        }
        let index = self
            .channels
            .iter()
            .position(|c| u64::from(c.id) == id)
            .ok_or(Error::InvalidConfig)?;
        self.selected = Some(index);
        self.snapshot = 0;
        Ok(Self::descriptor(&self.channels[index]))
    }
    fn configure(&mut self, snapshot: usize) -> Result<(), Error> {
        if self.selected.is_none() || self.clock.is_some() {
            return Err(Error::InvalidState);
        }
        if snapshot == 0 || snapshot > 8 {
            return Err(Error::InvalidConfig);
        }
        self.snapshot = snapshot;
        Ok(())
    }
    fn start(&mut self) -> Result<(), Error> {
        if self.clock.is_some() || self.snapshot == 0 {
            return Err(Error::InvalidState);
        }
        self.source.activate(
            self.channels[self.selected.ok_or(Error::InvalidState)?].id,
            &self.options,
        )?;
        self.clock = Some(Instant::now());
        Ok(())
    }
    fn poll(
        &mut self,
        cancellation: &Cancellation,
        max_items: usize,
        max_bytes: usize,
    ) -> Result<Vec<Observation>, Error> {
        let clock = self.clock.ok_or(Error::InvalidState)?;
        if max_items == 0 || max_items > 64 || max_bytes < self.snapshot || max_bytes > MAX_PAYLOAD
        {
            return Err(Error::InvalidConfig);
        }
        let channel = self.channels[self.selected.ok_or(Error::InvalidState)?].id;
        let mut observations = Vec::new();
        let mut remaining = max_bytes;
        // Finite work even if a source continuously returns rejected/empty frames.
        for _ in 0..max_items {
            if remaining < self.snapshot {
                break;
            }
            if cancellation.is_cancelled() {
                return Err(Error::Cancelled);
            }
            let Some(received) = self.source.next()? else {
                break;
            };
            let f = received.frame;
            if !self.options.accepts(&f) {
                continue;
            }
            let length = f.data.len().min(self.snapshot);
            let ingest = u64::try_from(clock.elapsed().as_nanos()).map_err(|_| Error::Exhausted)?;
            let mut observation = Observation::new(
                ingest,
                received.direction,
                Metadata::Can {
                    channel,
                    id: f.id_word & ID_MASK,
                    extended: f.id_word & EFF != 0,
                    fd: false,
                    remote: f.remote(),
                    error: f.error(),
                    dlc: f.dlc,
                },
                Some(f.data.len()),
                &f.data[..length],
            )
            .map_err(|_| Error::Backend)?;
            if let Some(timestamp) = received.source_timestamp_ns {
                observation = observation.with_source_timestamp(timestamp);
            }
            remaining -= length;
            observations.push(observation);
        }
        Ok(observations)
    }
    fn stop(&mut self) -> Result<(), Error> {
        self.source.close();
        self.clock = None;
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
