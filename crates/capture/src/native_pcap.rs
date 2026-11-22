//! Safe upstream pcap wrapper; no driver installation or library path mutation.
use crate::{
    adapter::Error,
    ethernet::{Options, RawFrame, Source},
};

#[derive(Default)]
pub struct NativePcap {
    capture: Option<pcap::Capture<pcap::Active>>,
    dropped: u32,
}
fn map_error(error: pcap::Error) -> Error {
    match error {
        pcap::Error::PcapError(message)
            if message.to_ascii_lowercase().contains("permission denied")
                || message.to_ascii_lowercase().contains("not permitted")
                || message.to_ascii_lowercase().contains("access is denied") =>
        {
            Error::PermissionDenied
        }
        pcap::Error::IoError(std::io::ErrorKind::PermissionDenied) => Error::PermissionDenied,
        pcap::Error::InvalidInputString => Error::InvalidConfig,
        pcap::Error::InvalidLinktype => Error::Unsupported,
        _ => Error::Backend,
    }
}
impl Source for NativePcap {
    fn list(&mut self, maximum: usize) -> Result<Vec<String>, Error> {
        Ok(pcap::Device::list()
            .map_err(map_error)?
            .into_iter()
            .take(maximum)
            .map(|d| d.name)
            .collect())
    }
    fn activate(&mut self, name: &str, snapshot: usize, options: &Options) -> Result<u32, Error> {
        self.close();
        let mut handle = pcap::Capture::from_device(name)
            .map_err(map_error)?
            .snaplen(i32::try_from(snapshot).map_err(|_| Error::InvalidConfig)?)
            .promisc(options.promiscuous)
            .timeout(i32::try_from(options.timeout_ms).map_err(|_| Error::InvalidConfig)?)
            .open()
            .map_err(map_error)?;
        if !options.filter.is_empty() {
            handle.filter(&options.filter, true).map_err(map_error)?;
        }
        let link_type = u32::try_from(handle.get_datalink().0).map_err(|_| Error::Unsupported)?;
        self.capture = Some(handle.setnonblock().map_err(map_error)?);
        self.dropped = 0;
        Ok(link_type)
    }
    fn next(&mut self) -> Result<Option<RawFrame>, Error> {
        let capture = self.capture.as_mut().ok_or(Error::InvalidState)?;
        match capture.next_packet() {
            Ok(packet) => {
                let seconds = u64::try_from(packet.header.ts.tv_sec).map_err(|_| Error::Backend)?;
                let micros = u64::try_from(packet.header.ts.tv_usec).map_err(|_| Error::Backend)?;
                if micros >= 1_000_000 || packet.data.len() > exmon_domain::observation::MAX_PAYLOAD
                {
                    return Err(Error::Backend);
                }
                let timestamp_ns = seconds
                    .checked_mul(1_000_000_000)
                    .and_then(|s| s.checked_add(micros * 1000))
                    .ok_or(Error::Backend)?;
                Ok(Some(RawFrame {
                    timestamp_ns,
                    original_length: packet.header.len as usize,
                    bytes: packet.data.to_vec(),
                }))
            }
            Err(pcap::Error::TimeoutExpired | pcap::Error::NoMorePackets) => Ok(None),
            Err(e) => Err(map_error(e)),
        }
    }
    fn close(&mut self) {
        self.capture = None;
    }
    fn loss(&mut self) -> Option<Option<u64>> {
        match self.capture.as_mut()?.stats() {
            Ok(stats) => {
                let current = stats.dropped;
                let delta = current.checked_sub(self.dropped);
                self.dropped = current;
                Some(delta.map(u64::from))
            }
            Err(_) => Some(None),
        }
    }
}
