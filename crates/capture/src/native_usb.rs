//! Async nusb queues. No automatic driver detach, device reset or configuration change.
use crate::{adapter::Error, usb_transfers::*};
use nusb::{
    MaybeFuture,
    transfer::{Buffer, Bulk, BulkOrInterrupt, EndpointDirection, In, Interrupt, Out},
};
use std::time::Duration;
trait Queue: Send {
    fn pending(&self) -> usize;
    fn submit(&mut self, buffer: Buffer);
    fn poll(&mut self) -> Option<nusb::transfer::Completion>;
}
impl<T: BulkOrInterrupt, D: EndpointDirection> Queue for nusb::Endpoint<T, D> {
    fn pending(&self) -> usize {
        self.pending()
    }
    fn submit(&mut self, buffer: Buffer) {
        self.submit(buffer);
    }
    fn poll(&mut self) -> Option<nusb::transfer::Completion> {
        if self.pending() == 0 {
            None
        } else {
            self.wait_next_complete(Duration::ZERO)
        }
    }
}
#[derive(Default)]
pub struct NativeUsb {
    devices: Vec<nusb::DeviceInfo>,
    device: Option<nusb::Device>,
    interface: Option<nusb::Interface>,
    queues: [Option<Box<dyn Queue>>; 2],
    addresses: [u8; 2],
    descriptors: Option<Descriptors>,
}
fn error(e: nusb::Error) -> Error {
    match e.kind() {
        nusb::ErrorKind::PermissionDenied => Error::PermissionDenied,
        nusb::ErrorKind::Busy => Error::Busy,
        nusb::ErrorKind::Disconnected => Error::Disconnected,
        nusb::ErrorKind::Unsupported => Error::Unsupported,
        _ => Error::Backend,
    }
}
impl Source for NativeUsb {
    fn enumerate(&mut self, maximum: usize) -> Result<Vec<Device>, Error> {
        if maximum == 0 || maximum > 64 {
            return Err(Error::InvalidConfig);
        }
        self.release();
        self.devices = nusb::list_devices()
            .wait()
            .map_err(error)?
            .take(maximum)
            .collect();
        Ok(self
            .devices
            .iter()
            .map(|d| Device {
                vendor_id: d.vendor_id(),
                product_id: d.product_id(),
            })
            .collect())
    }
    fn inspect(&mut self, index: usize) -> Result<Descriptors, Error> {
        self.release();
        let device = self
            .devices
            .get(index)
            .ok_or(Error::InvalidConfig)?
            .open()
            .wait()
            .map_err(error)?;
        let active = device
            .active_configuration()
            .map(|c| c.configuration_value())
            .unwrap_or(0);
        let mut configurations = Vec::new();
        for c in device.configurations() {
            if configurations.len() == 16 {
                return Err(Error::Backend);
            }
            let mut interfaces = Vec::new();
            for i in c.interface_alt_settings() {
                if interfaces.len() == 32 {
                    return Err(Error::Backend);
                }
                let mut endpoints = Vec::new();
                for e in i.endpoints() {
                    let kind = match e.transfer_type() {
                        nusb::descriptors::TransferType::Bulk => Kind::Bulk,
                        nusb::descriptors::TransferType::Interrupt => Kind::Interrupt,
                        nusb::descriptors::TransferType::Control => Kind::Control,
                        nusb::descriptors::TransferType::Isochronous => Kind::Isochronous,
                    };
                    if endpoints.len() == 30 {
                        return Err(Error::Backend);
                    }
                    endpoints.push(Endpoint {
                        address: e.address(),
                        kind,
                        packet_size: e.max_packet_size(),
                    });
                }
                interfaces.push(Interface {
                    number: i.interface_number(),
                    alternate: i.alternate_setting(),
                    endpoints,
                });
            }
            configurations.push(Configuration {
                value: c.configuration_value(),
                interfaces,
            });
        }
        let descriptors = Descriptors {
            active,
            configurations,
        };
        descriptors.validate()?;
        self.descriptors = Some(descriptors.clone());
        self.device = Some(device);
        Ok(descriptors)
    }
    fn claim(&mut self, selection: Selection) -> Result<(), Error> {
        if self.interface.is_some() {
            return Err(Error::InvalidState);
        }
        let device = self.device.as_ref().ok_or(Error::InvalidState)?;
        if device
            .active_configuration()
            .map(|c| c.configuration_value())
            .unwrap_or(0)
            != selection.configuration
        {
            return Err(Error::Unsupported);
        }
        let descriptor = self
            .descriptors
            .as_ref()
            .and_then(|d| {
                d.configurations
                    .iter()
                    .find(|c| c.value == selection.configuration)
            })
            .and_then(|c| {
                c.interfaces
                    .iter()
                    .find(|i| i.number == selection.interface && i.alternate == selection.alternate)
            })
            .ok_or(Error::InvalidConfig)?;
        let interface = device
            .claim_interface(selection.interface)
            .wait()
            .map_err(error)?;
        interface
            .set_alt_setting(selection.alternate)
            .wait()
            .map_err(error)?;
        let mut queues: [Option<Box<dyn Queue>>; 2] = [None, None];
        for (dir, address) in [
            (Direction::In, selection.input),
            (Direction::Out, selection.output),
        ] {
            if let Some(address) = address {
                let kind = descriptor
                    .endpoints
                    .iter()
                    .find(|e| e.address == address)
                    .ok_or(Error::InvalidConfig)?
                    .kind;
                queues[dir.index()] = Some(match (kind, dir) {
                    (Kind::Bulk, Direction::In) => {
                        Box::new(interface.endpoint::<Bulk, In>(address).map_err(error)?)
                    }
                    (Kind::Bulk, Direction::Out) => {
                        Box::new(interface.endpoint::<Bulk, Out>(address).map_err(error)?)
                    }
                    (Kind::Interrupt, Direction::In) => Box::new(
                        interface
                            .endpoint::<Interrupt, In>(address)
                            .map_err(error)?,
                    ),
                    (Kind::Interrupt, Direction::Out) => Box::new(
                        interface
                            .endpoint::<Interrupt, Out>(address)
                            .map_err(error)?,
                    ),
                    _ => return Err(Error::Unsupported),
                });
                self.addresses[dir.index()] = address;
            }
        }
        self.queues = queues;
        self.interface = Some(interface);
        Ok(())
    }
    fn submit(
        &mut self,
        direction: Direction,
        endpoint: Endpoint,
        bytes: &[u8],
        requested: usize,
    ) -> Result<(), Error> {
        let queue = self.queues[direction.index()]
            .as_mut()
            .ok_or(Error::InvalidState)?;
        if queue.pending() != 0 {
            return Err(Error::InvalidState);
        }
        if endpoint.address != self.addresses[direction.index()]
            || requested > 65536
            || bytes.len() > 65536
            || (direction == Direction::Out && requested != bytes.len())
        {
            return Err(Error::InvalidConfig);
        }
        let buffer = if direction == Direction::In {
            Buffer::new(requested)
        } else {
            Buffer::from(bytes)
        };
        queue.submit(buffer);
        Ok(())
    }
    fn poll(&mut self, direction: Direction) -> Result<Option<Completion>, Error> {
        let Some(done) = self.queues[direction.index()]
            .as_mut()
            .ok_or(Error::InvalidState)?
            .poll()
        else {
            return Ok(None);
        };
        let status = match done.status {
            Ok(()) => Status::Success,
            Err(nusb::transfer::TransferError::Cancelled) => Status::Cancelled,
            Err(nusb::transfer::TransferError::Stall) => Status::Stall,
            Err(nusb::transfer::TransferError::Disconnected) => Status::Disconnected,
            _ => Status::Fault,
        };
        if done.actual_len > done.buffer.len() {
            return Err(Error::Backend);
        }
        Ok(Some(Completion {
            direction,
            endpoint: self.addresses[direction.index()],
            bytes: done.buffer[..done.actual_len].to_vec(),
            actual_length: Some(done.actual_len),
            status,
        }))
    }
    fn release(&mut self) {
        self.queues = [None, None];
        self.interface = None;
        self.device = None;
        self.descriptors = None;
    }
}
