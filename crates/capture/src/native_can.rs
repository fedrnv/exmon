//! Linux SocketCAN only. Link creation, bitrate, permissions and drivers are external.
use crate::{
    adapter::{Availability, Error},
    can::*,
};
#[derive(Default)]
pub struct NativeCan {
    #[cfg(all(target_os = "linux", feature = "native-can"))]
    socket: Option<socketcan::CanSocket>,
    unknown_loss: bool,
}
pub fn capability() -> Availability {
    if cfg!(all(target_os = "linux", feature = "native-can")) {
        Availability::Available
    } else {
        Availability::Unsupported
    }
}
pub fn requirements() -> &'static str {
    if cfg!(target_os = "linux") {
        "enable native-can; administrator supplies an up SocketCAN/vcan link and access; application never configures bitrate/link/driver"
    } else {
        "SocketCAN requires Linux; no Windows/macOS vendor backend implemented; use virtual CAN or provision a supported Linux environment"
    }
}
impl Source for NativeCan {
    fn list(&mut self, limit: usize) -> Result<Vec<Channel>, Error> {
        if limit == 0 || limit > 64 {
            return Err(Error::InvalidConfig);
        }
        if capability() != Availability::Available {
            return Err(Error::Unsupported);
        }
        let mut channels = Vec::new();
        for entry in std::fs::read_dir("/sys/class/net")
            .map_err(map_error)?
            .take(1024)
        {
            let entry = entry.map_err(map_error)?;
            // Linux ARPHRD_CAN = 280. Names alone cannot prove CAN capability.
            if std::fs::read_to_string(entry.path().join("type"))
                .map_err(map_error)?
                .trim()
                != "280"
            {
                continue;
            }
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| Error::Backend)?;
            if name.len() > 15 || name.is_empty() {
                return Err(Error::Backend);
            }
            let id = std::fs::read_to_string(entry.path().join("ifindex"))
                .map_err(map_error)?
                .trim()
                .parse()
                .map_err(|_| Error::Backend)?;
            channels.push(Channel {
                id,
                name,
                tx: Availability::Available,
            });
            if channels.len() == limit {
                break;
            }
        }
        channels.sort_by_key(|c| c.id);
        Ok(channels)
    }
    fn activate(&mut self, channel: u32, options: &Options) -> Result<(), Error> {
        options.validate()?;
        if capability() != Availability::Available {
            return Err(Error::Unsupported);
        }
        #[cfg(all(target_os = "linux", feature = "native-can"))]
        {
            use socketcan::{CanFilter, CanSocket, Socket, SocketOptions};
            if self.socket.is_some() {
                return Err(Error::InvalidState);
            }
            if channel == 0 || !self.list(64)?.iter().any(|c| c.id == channel) {
                return Err(Error::Disconnected);
            }
            let socket = CanSocket::open_iface(channel).map_err(map_error)?;
            socket.set_nonblocking(true).map_err(map_error)?;
            socket.set_recv_own_msgs(false).map_err(map_error)?;
            socket.set_loopback(true).map_err(map_error)?;
            socket.set_recv_timestamp(true).map_err(map_error)?;
            socket
                .set_error_filter(if options.receive_errors { ID_MASK } else { 0 })
                .map_err(map_error)?;
            let filters: Vec<_> = options
                .filters
                .iter()
                .map(|f| CanFilter::new(f.id, f.mask))
                .collect();
            socket.set_filters(&filters).map_err(map_error)?;
            self.socket = Some(socket);
            self.unknown_loss = true;
            Ok(())
        }
        #[cfg(not(all(target_os = "linux", feature = "native-can")))]
        {
            let _ = channel;
            Err(Error::Unsupported)
        }
    }
    fn next(&mut self) -> Result<Option<Received>, Error> {
        #[cfg(all(target_os = "linux", feature = "native-can"))]
        {
            use socketcan::Socket;
            let socket = self.socket.as_ref().ok_or(Error::InvalidState)?;
            let (frame, timestamp) = match socket.read_frame_with_timestamp() {
                Ok(value) => value,
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                    ) =>
                {
                    return Ok(None);
                }
                Err(e) => return Err(map_error(e)),
            };
            let raw = frame.as_ref();
            if raw.can_dlc > 8 || raw.len8_dlc > 15 {
                return Err(Error::Backend);
            }
            let dlc = if raw.can_dlc == 8 && raw.len8_dlc > 8 {
                raw.len8_dlc
            } else {
                raw.can_dlc
            };
            let bytes = if raw.can_id & RTR != 0 {
                &[][..]
            } else {
                &raw.data[..usize::from(raw.can_dlc)]
            };
            let frame = Frame::new(raw.can_id, dlc, bytes).map_err(|_| Error::Backend)?;
            let ns = timestamp
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|_| Error::Backend)?
                .as_nanos();
            let timestamp = u64::try_from(ns).map_err(|_| Error::Exhausted)?;
            // recvmsg MSG_DONTROUTE/MSG_CONFIRM not exposed by this safe API;
            // never guess physical Rx versus another local sender's loopback.
            Ok(Some(Received {
                frame,
                direction: exmon_domain::Direction::Unknown,
                source_timestamp_ns: Some(timestamp),
            }))
        }
        #[cfg(not(all(target_os = "linux", feature = "native-can")))]
        Err(Error::Unsupported)
    }
    fn send(&mut self, frame: &Frame) -> Result<(), Error> {
        if frame.error() {
            return Err(Error::Unsupported);
        }
        #[cfg(all(target_os = "linux", feature = "native-can"))]
        {
            use socketcan::Socket;
            let socket = self.socket.as_ref().ok_or(Error::InvalidState)?;
            let mut raw = socketcan::frame::can_frame_default();
            raw.can_id = frame.id_word();
            raw.can_dlc = frame.dlc().min(8);
            raw.len8_dlc = if frame.dlc() > 8 { frame.dlc() } else { 0 };
            raw.data[..frame.data().len()].copy_from_slice(frame.data());
            socket
                .write_frame(&socketcan::CanFrame::from(raw))
                .map_err(map_error)
        }
        #[cfg(not(all(target_os = "linux", feature = "native-can")))]
        Err(Error::Unsupported)
    }
    fn close(&mut self) {
        #[cfg(all(target_os = "linux", feature = "native-can"))]
        {
            self.socket = None;
        }
        self.unknown_loss = false;
    }
    fn loss(&mut self) -> Option<Option<u64>> {
        std::mem::take(&mut self.unknown_loss).then_some(None)
    }
}
fn map_error(error: std::io::Error) -> Error {
    use std::io::ErrorKind;
    match error.kind() {
        ErrorKind::PermissionDenied => Error::PermissionDenied,
        ErrorKind::NotFound
        | ErrorKind::NotConnected
        | ErrorKind::NetworkDown
        | ErrorKind::NetworkUnreachable => Error::Disconnected,
        ErrorKind::Unsupported => Error::Unsupported,
        ErrorKind::WouldBlock | ErrorKind::Interrupted => Error::Busy,
        _ => match error.raw_os_error() {
            Some(19 | 6) => Error::Disconnected, // Linux ENODEV / ENXIO
            Some(97 | 93) => Error::Unsupported, // Linux EAFNOSUPPORT / EPROTONOSUPPORT
            Some(105) => Error::Busy,            // Linux ENOBUFS, no automatic retry
            _ => Error::Backend,
        },
    }
}
