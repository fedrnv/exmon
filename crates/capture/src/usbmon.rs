//! Bounded usbmon 1u text events; never an active USB transport.
use crate::adapter::Error;
use crate::adapter::{Adapter, Availability, Cancellation, Descriptor};
use exmon_domain::observation::{MAX_PAYLOAD, TransportKind};
use exmon_domain::{
    Direction,
    observation::{Metadata, Observation, UsbMonMetadata},
};
use std::io::{self, Read};
use std::time::Instant;
pub const MAX_LINE: usize = 262144;

/// Trusted passive source: finite, nonblocking polling and at most 64 buses.
pub trait Source: Send {
    fn buses(&mut self) -> Result<Vec<u16>, Error>;
    fn activate(&mut self, bus: u16) -> Result<(), Error>;
    fn next(&mut self) -> Result<Option<Record>, Error>;
    fn close(&mut self);
}
pub struct Usbmon<S: Source> {
    source: S,
    buses: Vec<u16>,
    selected: Option<u16>,
    snapshot: usize,
    clock: Option<Instant>,
    report_unknown_loss: bool,
}
impl<S: Source> Usbmon<S> {
    pub fn new(source: S) -> Self {
        Self {
            source,
            buses: vec![],
            selected: None,
            snapshot: 0,
            clock: None,
            report_unknown_loss: false,
        }
    }
    fn descriptor(bus: u16) -> Descriptor {
        Descriptor {
            id: u64::from(bus),
            kind: TransportKind::Usb,
            rx: Availability::Available,
            tx: Availability::Unsupported,
        }
    }
}
impl<S: Source> Adapter for Usbmon<S> {
    fn enumerate(&mut self, limit: usize) -> Result<Vec<Descriptor>, Error> {
        if self.selected.is_some() {
            return Err(Error::InvalidState);
        }
        if limit == 0 || limit > 64 {
            return Err(Error::InvalidConfig);
        }
        let mut buses = self.source.buses()?;
        if buses.len() > 64 {
            return Err(Error::Backend);
        }
        buses.sort_unstable();
        buses.dedup();
        buses.truncate(limit);
        self.buses = buses;
        Ok(self.buses.iter().copied().map(Self::descriptor).collect())
    }
    fn open(&mut self, id: u64) -> Result<Descriptor, Error> {
        if self.selected.is_some() {
            return Err(Error::InvalidState);
        }
        let bus = u16::try_from(id).map_err(|_| Error::InvalidConfig)?;
        if !self.buses.contains(&bus) {
            return Err(Error::InvalidConfig);
        }
        self.selected = Some(bus);
        self.snapshot = 0;
        Ok(Self::descriptor(bus))
    }
    fn configure(&mut self, snapshot: usize) -> Result<(), Error> {
        if self.selected.is_none() || self.clock.is_some() {
            return Err(Error::InvalidState);
        }
        if snapshot == 0 || snapshot > MAX_PAYLOAD {
            return Err(Error::InvalidConfig);
        }
        self.snapshot = snapshot;
        Ok(())
    }
    fn start(&mut self) -> Result<(), Error> {
        if self.clock.is_some() || self.snapshot == 0 {
            return Err(Error::InvalidState);
        }
        self.source
            .activate(self.selected.ok_or(Error::InvalidState)?)?;
        self.clock = Some(Instant::now());
        self.report_unknown_loss = true;
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
        let mut result = Vec::new();
        let mut remaining = max_bytes;
        while result.len() < max_items && remaining >= self.snapshot {
            if cancellation.is_cancelled() {
                return Err(Error::Cancelled);
            }
            let Some(mut record) = self.source.next()? else {
                break;
            };
            if record.bytes.len() > MAX_PAYLOAD {
                return Err(Error::Backend);
            }
            record.bytes.truncate(self.snapshot);
            remaining -= record.bytes.len();
            let ingest = u64::try_from(clock.elapsed().as_nanos()).map_err(|_| Error::Exhausted)?;
            result.push(record.into_observation(ingest)?);
        }
        Ok(result)
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
        std::mem::take(&mut self.report_unknown_loss).then_some(None)
    }
}
fn number<T: std::str::FromStr>(word: Option<&str>) -> Result<T, Error> {
    word.ok_or(Error::Backend)?
        .parse()
        .map_err(|_| Error::Backend)
}
fn hex(word: &str, maximum: usize) -> Result<u64, Error> {
    if word.len() > maximum || word.is_empty() {
        return Err(Error::Backend);
    }
    u64::from_str_radix(word, 16).map_err(|_| Error::Backend)
}

pub struct Record {
    pub source_timestamp_ns: u64,
    pub metadata: UsbMonMetadata,
    pub bytes: Vec<u8>,
}
impl Record {
    pub fn into_observation(self, ingest_ns: u64) -> Result<Observation, Error> {
        let direction = if self.metadata.endpoint & 0x80 != 0 {
            Direction::Rx
        } else {
            Direction::Tx
        };
        let original = if self.metadata.transfer == b'Z' {
            None
        } else {
            Some(self.metadata.reported_length as usize)
        };
        Observation::new(
            ingest_ns,
            direction,
            Metadata::UsbMon(self.metadata),
            original,
            &self.bytes,
        )
        .map(|o| o.with_source_timestamp(self.source_timestamp_ns))
        .map_err(|_| Error::Backend)
    }
}
pub fn parse(line: &str) -> Result<Record, Error> {
    if line.len() > MAX_LINE || !line.is_ascii() {
        return Err(Error::Backend);
    }
    let mut words = line.split_ascii_whitespace();
    let tag = hex(words.next().ok_or(Error::Backend)?, 16)?;
    let micros: u64 = number(words.next())?;
    let source_timestamp_ns = micros.checked_mul(1000).ok_or(Error::Backend)?;
    let event = words.next().ok_or(Error::Backend)?.as_bytes();
    if event.len() != 1 || !b"SCE".contains(&event[0]) {
        return Err(Error::Unsupported);
    }
    let address: Vec<_> = words.next().ok_or(Error::Backend)?.splitn(5, ':').collect();
    if address.len() != 4 {
        return Err(Error::Unsupported);
    }
    let kind = address[0].as_bytes();
    if kind.len() != 2 || !b"CZIB".contains(&kind[0]) || !b"io".contains(&kind[1]) {
        return Err(Error::Unsupported);
    }
    let bus: u16 = number(Some(address[1]))?;
    let device: u8 = number(Some(address[2]))?;
    let endpoint: u8 = number(Some(address[3]))?;
    if device > 127 || endpoint > 15 {
        return Err(Error::Backend);
    }
    let mut metadata = UsbMonMetadata {
        tag,
        event: event[0],
        transfer: kind[0],
        bus,
        device,
        endpoint: endpoint | if kind[1] == b'i' { 0x80 } else { 0 },
        status: None,
        interval: None,
        start_frame: None,
        errors: None,
        setup: None,
        setup_tag: None,
        iso_count: 0,
        iso: [None; 5],
        reported_length: 0,
        data_tag: 0,
    };
    let status = words.next().ok_or(Error::Backend)?;
    if status.len() == 1 && status.as_bytes()[0].is_ascii_alphabetic() {
        if kind[0] != b'C' || event[0] != b'S' {
            return Err(Error::Backend);
        }
        metadata.setup_tag = Some(status.as_bytes()[0]);
        let mut setup = [0; 8];
        let widths = [2, 2, 4, 4, 4];
        let mut offset = 0;
        for width in widths {
            let word = words.next().ok_or(Error::Backend)?;
            if status == "s" {
                let n = hex(word, width)?;
                setup[offset] = n as u8;
                if width == 4 {
                    setup[offset + 1] = (n >> 8) as u8;
                }
            }
            offset += width / 2;
        }
        if status == "s" {
            metadata.setup = Some(setup);
        }
    } else {
        let values: Vec<_> = status.splitn(5, ':').collect();
        if values.len() > 4 {
            return Err(Error::Backend);
        }
        metadata.status = Some(number(values.first().copied())?);
        if values.len() > 1 {
            metadata.interval = Some(number(values.get(1).copied())?);
        }
        if values.len() > 2 {
            metadata.start_frame = Some(number(values.get(2).copied())?);
        }
        if values.len() > 3 {
            metadata.errors = Some(number(values.get(3).copied())?);
        }
    }
    if kind[0] == b'Z' {
        metadata.iso_count = number(words.next())?;
        if metadata.iso_count > 65536 {
            return Err(Error::Backend);
        }
        for slot in metadata
            .iso
            .iter_mut()
            .take((metadata.iso_count as usize).min(5))
        {
            let fields: Vec<_> = words.next().ok_or(Error::Backend)?.splitn(5, ':').collect();
            if fields.len() != 3 {
                return Err(Error::Backend);
            }
            let descriptor = (
                number(Some(fields[0]))?,
                number(Some(fields[1]))?,
                number(Some(fields[2]))?,
            );
            let (status, offset, length): (i32, u32, u32) = descriptor;
            offset.checked_add(length).ok_or(Error::Backend)?;
            *slot = Some((status, offset, length));
        }
    }
    metadata.reported_length = number(words.next())?;
    let data = words.next().ok_or(Error::Backend)?.as_bytes();
    if data.len() != 1 || !data[0].is_ascii_graphic() {
        return Err(Error::Backend);
    }
    metadata.data_tag = data[0];
    let mut bytes = Vec::new();
    for word in words {
        if data[0] != b'=' || word.len() > 8 || !word.len().is_multiple_of(2) {
            return Err(Error::Backend);
        }
        for pair in word.as_bytes().as_chunks::<2>().0 {
            if bytes.len() == 65536 {
                return Err(Error::Backend);
            }
            let pair = std::str::from_utf8(pair).map_err(|_| Error::Backend)?;
            bytes.push(hex(pair, 2)? as u8);
        }
    }
    if kind[0] != b'Z' && bytes.len() > metadata.reported_length as usize {
        return Err(Error::Backend);
    }
    Ok(Record {
        source_timestamp_ns,
        metadata,
        bytes,
    })
}

/// Incremental nonblocking reader. Oversized records enter discard-to-newline
/// recovery, never truncation presented as a valid event.
pub struct Reader<R> {
    source: std::io::BufReader<R>,
    pending: Vec<u8>,
    discarding: bool,
    ended: bool,
}
impl<R: Read> Reader<R> {
    pub fn new(source: R) -> Self {
        Self {
            source: std::io::BufReader::with_capacity(4096, source),
            pending: vec![],
            discarding: false,
            ended: false,
        }
    }
    pub fn next_record(&mut self) -> Result<Option<Record>, Error> {
        for _ in 0..4096 {
            let mut byte = [0];
            match self.source.read(&mut byte) {
                Ok(0) => {
                    self.ended = true;
                    if !self.pending.is_empty() {
                        self.pending.clear();
                        return Err(Error::Backend);
                    }
                    return Ok(None);
                }
                Ok(_) => {
                    if byte[0] == b'\n' {
                        if self.discarding {
                            self.discarding = false;
                            continue;
                        }
                        let line = std::mem::take(&mut self.pending);
                        return parse(std::str::from_utf8(&line).map_err(|_| Error::Backend)?)
                            .map(Some);
                    }
                    if self.discarding {
                        continue;
                    }
                    if self.pending.len() == MAX_LINE {
                        self.pending.clear();
                        self.discarding = true;
                        return Err(Error::Backend);
                    }
                    self.pending.push(byte[0]);
                }
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) =>
                {
                    return Ok(None);
                }
                Err(e) => {
                    return Err(if e.kind() == io::ErrorKind::PermissionDenied {
                        Error::PermissionDenied
                    } else {
                        Error::Backend
                    });
                }
            }
        }
        Ok(None)
    }
    pub fn ended(&self) -> bool {
        self.ended
    }
}
