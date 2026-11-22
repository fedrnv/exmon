//! OS serial access through safe serial2 APIs. No enumeration activates a port.
use crate::{
    adapter::{Availability, Error},
    serial::{ElectricalMode, Settings, Source},
};
use std::{io, time::Duration};

#[derive(Default)]
pub struct NativeSerial {
    port: Option<serial2::SerialPort>,
    direction: Option<Availability>,
}
fn map_error(e: io::Error) -> Error {
    match e.kind() {
        io::ErrorKind::PermissionDenied => Error::PermissionDenied,
        io::ErrorKind::Unsupported => Error::Unsupported,
        io::ErrorKind::InvalidInput => Error::InvalidConfig,
        io::ErrorKind::NotFound
        | io::ErrorKind::NotConnected
        | io::ErrorKind::BrokenPipe
        | io::ErrorKind::UnexpectedEof => Error::Disconnected,
        _ => Error::Backend,
    }
}
impl Source for NativeSerial {
    fn list(&mut self, limit: usize) -> Result<Vec<String>, Error> {
        serial2::SerialPort::available_ports()
            .map_err(map_error)?
            .into_iter()
            .take(limit)
            .map(|p| {
                p.into_os_string()
                    .into_string()
                    .map_err(|_| Error::Unsupported)
            })
            .collect()
    }
    fn open(&mut self, name: &str, settings: Settings) -> Result<(), Error> {
        settings.validate()?;
        self.close();
        #[cfg(not(target_os = "linux"))]
        if settings.electrical != ElectricalMode::Unchanged {
            return Err(Error::Unsupported);
        }
        let mut port = serial2::SerialPort::open(name, |mut config: serial2::Settings| {
            config.set_raw();
            config.set_baud_rate(settings.baud)?;
            config.set_char_size(settings.data_bits);
            config.set_parity(settings.parity);
            config.set_stop_bits(settings.stop_bits);
            config.set_flow_control(settings.flow);
            Ok(config)
        })
        .map_err(map_error)?;
        port.set_read_timeout(Duration::from_millis(u64::from(settings.timeout_ms)))
            .map_err(map_error)?;
        port.set_write_timeout(Duration::from_millis(u64::from(settings.timeout_ms)))
            .map_err(map_error)?;
        #[cfg(target_os = "linux")]
        {
            use serial2::rs4xx::{Rs485Config, TransceiverMode};
            let requested = match settings.electrical {
                ElectricalMode::Unchanged => None,
                ElectricalMode::Rs422 => Some(TransceiverMode::Rs422),
                ElectricalMode::Rs485 {
                    full_duplex,
                    invert_rts,
                    before_ms,
                    after_ms,
                } => {
                    let mut c = Rs485Config::default();
                    c.set_full_duplex(full_duplex);
                    c.set_invert_rts(invert_rts);
                    c.set_delay_before_send(Duration::from_millis(u64::from(before_ms)));
                    c.set_delay_after_send(Duration::from_millis(u64::from(after_ms)));
                    Some(TransceiverMode::Rs485(c))
                }
            };
            if let Some(requested) = requested {
                port.set_rs4xx_mode(requested.clone())
                    .map_err(|_| Error::Unsupported)?;
                let effective = port.get_rs4xx_mode().map_err(|_| Error::Unsupported)?;
                let matches = match (requested, effective) {
                    (TransceiverMode::Rs422, TransceiverMode::Rs422) => true,
                    (TransceiverMode::Rs485(a), TransceiverMode::Rs485(b)) => {
                        a.get_full_duplex() == b.get_full_duplex()
                            && a.get_invert_rts() == b.get_invert_rts()
                            && a.get_delay_before_send() == b.get_delay_before_send()
                            && a.get_delay_after_send() == b.get_delay_after_send()
                    }
                    _ => false,
                };
                if !matches {
                    return Err(Error::Unsupported);
                }
            }
            self.direction = Some(if port.get_rs4xx_mode().is_ok() {
                Availability::Available
            } else {
                Availability::Unsupported
            });
        }
        self.port = Some(port);
        Ok(())
    }
    fn read(&mut self, buffer: &mut [u8]) -> Result<usize, Error> {
        match self.port.as_ref().ok_or(Error::InvalidState)?.read(buffer) {
            Ok(n) => Ok(n),
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
                ) =>
            {
                Ok(0)
            }
            Err(e) => Err(map_error(e)),
        }
    }
    fn write(&mut self, bytes: &[u8]) -> Result<usize, Error> {
        self.port
            .as_ref()
            .ok_or(Error::InvalidState)?
            .write(bytes)
            .map_err(map_error)
    }
    fn close(&mut self) {
        self.port = None;
        self.direction = None;
    }
    fn direction_control(&self) -> Availability {
        self.direction.unwrap_or(Availability::Unsupported)
    }
}
