//! Explicit device selection and bounded HID report semantics.
use crate::adapter::{Adapter, Availability, Cancellation, Descriptor, Error};
use exmon_domain::{
    Direction,
    observation::{Metadata, Observation, TransportKind},
};
use std::time::Instant;

pub const MAX_REPORT: usize = 4096;
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Device {
    pub vendor_id: u16,
    pub product_id: u16,
    pub usage_page: u16,
    pub usage: u16,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ReportMode {
    Numbered,
    Unnumbered,
}
pub trait Source: Send {
    fn enumerate(&mut self, maximum: usize) -> Result<Vec<Device>, Error>;
    fn open_selected(&mut self, index: usize) -> Result<(), Error>;
    fn read(&mut self, bytes: &mut [u8], timeout_ms: i32) -> Result<usize, Error>;
    fn write(&mut self, bytes: &[u8]) -> Result<usize, Error>;
    fn close(&mut self);
}
pub struct Hid<S: Source> {
    source: S,
    devices: Vec<Device>,
    selected: Option<usize>,
    mode: ReportMode,
    maximum: usize,
    clock: Option<Instant>,
}
impl<S: Source> Hid<S> {
    pub fn new(source: S, mode: ReportMode) -> Self {
        Self {
            source,
            devices: vec![],
            selected: None,
            mode,
            maximum: 0,
            clock: None,
        }
    }
    pub fn device(&self, id: u64) -> Option<Device> {
        self.devices.get(usize::try_from(id).ok()?).copied()
    }
    /// Report semantics may change only before a collection is selected/opened.
    pub fn set_report_mode(&mut self, mode: ReportMode) -> Result<(), Error> {
        if self.selected.is_some() || self.clock.is_some() {
            return Err(Error::InvalidState);
        }
        self.mode = mode;
        Ok(())
    }
    fn descriptor(id: u64) -> Descriptor {
        Descriptor {
            id,
            kind: TransportKind::Usb,
            rx: Availability::Available,
            tx: Availability::Available,
        }
    }
}
impl<S: Source> Drop for Hid<S> {
    fn drop(&mut self) {
        self.source.close();
    }
}
impl<S: Source> Adapter for Hid<S> {
    fn enumerate(&mut self, maximum: usize) -> Result<Vec<Descriptor>, Error> {
        if self.selected.is_some() {
            return Err(Error::InvalidState);
        }
        if maximum == 0 || maximum > 64 {
            return Err(Error::InvalidConfig);
        }
        let devices = self.source.enumerate(maximum)?;
        if devices.len() > maximum {
            return Err(Error::Backend);
        }
        self.devices = devices;
        Ok((0..self.devices.len())
            .map(|n| Self::descriptor(n as u64))
            .collect())
    }
    fn open(&mut self, id: u64) -> Result<Descriptor, Error> {
        if self.selected.is_some() {
            return Err(Error::InvalidState);
        }
        let index = usize::try_from(id).map_err(|_| Error::InvalidConfig)?;
        if index >= self.devices.len() {
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
        if maximum == 0 || maximum > MAX_REPORT {
            return Err(Error::InvalidConfig);
        }
        self.maximum = maximum;
        Ok(())
    }
    fn start(&mut self) -> Result<(), Error> {
        if self.clock.is_some() || self.maximum == 0 {
            return Err(Error::InvalidState);
        }
        self.source
            .open_selected(self.selected.ok_or(Error::InvalidState)?)?;
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
        if max_items == 0 || max_items > 64 || max_bytes < self.maximum || max_bytes > 65536 {
            return Err(Error::InvalidConfig);
        }
        let mut bytes = vec![0; self.maximum];
        let length = match self.source.read(&mut bytes, 10) {
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
        let info = self.devices[self.selected.ok_or(Error::InvalidState)?];
        let report_id = if self.mode == ReportMode::Numbered {
            bytes[0]
        } else {
            0
        };
        if self.mode == ReportMode::Numbered && report_id == 0 {
            return Err(Error::Backend);
        }
        let timestamp = u64::try_from(clock.elapsed().as_nanos()).map_err(|_| Error::Exhausted)?;
        let metadata = Metadata::Hid {
            vendor_id: info.vendor_id,
            product_id: info.product_id,
            usage_page: info.usage_page,
            usage: info.usage,
            report_id,
        };
        Ok(vec![
            Observation::new(timestamp, Direction::Rx, metadata, None, &bytes[..length])
                .map_err(|_| Error::Backend)?,
        ])
    }
    fn transmit(&mut self, bytes: &[u8]) -> Result<usize, Error> {
        if self.clock.is_none() {
            return Err(Error::InvalidState);
        }
        if bytes.is_empty()
            || bytes.len() > self.maximum
            || (self.mode == ReportMode::Unnumbered && bytes[0] != 0)
            || (self.mode == ReportMode::Numbered && bytes[0] == 0)
        {
            return Err(Error::InvalidConfig);
        }
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
