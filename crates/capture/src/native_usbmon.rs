//! Read-only Linux debugfs usbmon source; other platforms explicitly unsupported.
use crate::{
    adapter::{Availability, Error},
    usbmon::{Reader, Record},
};
pub fn capability() -> Availability {
    if cfg!(all(target_os = "linux", feature = "native-usbmon")) {
        Availability::Available
    } else {
        Availability::Unsupported
    }
}
#[derive(Default)]
/// Setup remains an external administrator action, never performed by capture.
pub struct NativeUsbmon {
    reader: Option<Reader<std::fs::File>>,
}
pub fn requirements() -> &'static str {
    if cfg!(target_os = "linux") {
        "enable native-usbmon; administrator prepares usbmon/debugfs and grants selected bus read access"
    } else if cfg!(target_os = "windows") {
        "USBPcap driver/extcap backend evaluated but not implemented; passive capture unsupported"
    } else {
        "no implemented passive host-USB backend; active USB is not a substitute"
    }
}
impl NativeUsbmon {
    pub fn enumerate(&self) -> Result<Vec<u16>, Error> {
        if capability() != Availability::Available {
            return Err(Error::Unsupported);
        }
        let mut buses = Vec::new();
        for entry in std::fs::read_dir("/sys/kernel/debug/usb/usbmon")
            .map_err(map_error)?
            .take(512)
        {
            let entry = entry.map_err(map_error)?;
            let name = entry.file_name();
            if let Some(bus) = name
                .to_str()
                .and_then(|n| n.strip_suffix('u'))
                .and_then(|n| n.parse::<u16>().ok())
            {
                if buses.len() == 64 {
                    return Err(Error::Backend);
                }
                buses.push(bus);
            }
        }
        buses.sort_unstable();
        buses.dedup();
        Ok(buses)
    }
    pub fn open(&mut self, bus: u16) -> Result<(), Error> {
        if !self.enumerate()?.contains(&bus) {
            return Err(Error::InvalidConfig);
        }
        #[cfg(all(target_os = "linux", feature = "native-usbmon"))]
        {
            use rustix::fs::{Mode, OFlags, open};
            let path = format!("/sys/kernel/debug/usb/usbmon/{bus}u");
            let fd = open(
                path,
                OFlags::RDONLY | OFlags::NONBLOCK | OFlags::CLOEXEC | OFlags::NOFOLLOW,
                Mode::empty(),
            )
            .map_err(|e| map_error(e.into()))?;
            self.reader = Some(Reader::new(std::fs::File::from(fd)));
            Ok(())
        }
        #[cfg(not(all(target_os = "linux", feature = "native-usbmon")))]
        Err(Error::Unsupported)
    }
    pub fn poll(&mut self) -> Result<Option<Record>, Error> {
        self.reader
            .as_mut()
            .ok_or(Error::InvalidState)?
            .next_record()
    }
    pub fn stop(&mut self) {
        self.reader = None;
    }
}
fn map_error(error: std::io::Error) -> Error {
    match error.kind() {
        std::io::ErrorKind::PermissionDenied => Error::PermissionDenied,
        std::io::ErrorKind::NotFound => Error::Unsupported,
        _ => Error::Backend,
    }
}

impl crate::usbmon::Source for NativeUsbmon {
    fn buses(&mut self) -> Result<Vec<u16>, Error> {
        self.enumerate()
    }
    fn activate(&mut self, bus: u16) -> Result<(), Error> {
        self.open(bus)
    }
    fn next(&mut self) -> Result<Option<Record>, Error> {
        self.poll()
    }
    fn close(&mut self) {
        self.stop();
    }
}
