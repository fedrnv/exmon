//! Bounded in-process CAN broadcast bus, independent of physical drivers.
use crate::{
    adapter::{Availability, Error},
    can::*,
};
use exmon_domain::Direction;
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};
struct Endpoint {
    options: Options,
    queue: VecDeque<Received>,
    lost: u64,
}
struct Bus {
    endpoints: Vec<Option<Endpoint>>,
    capacity: usize,
    time: u64,
}
#[derive(Clone)]
pub struct VirtualBus(Arc<Mutex<Bus>>);
impl VirtualBus {
    pub fn new(queue_capacity: usize) -> Result<Self, Error> {
        if queue_capacity == 0 || queue_capacity > 4096 {
            return Err(Error::InvalidConfig);
        }
        Ok(Self(Arc::new(Mutex::new(Bus {
            endpoints: vec![],
            capacity: queue_capacity,
            time: 0,
        }))))
    }
    pub fn endpoint(&self) -> VirtualCan {
        VirtualCan {
            bus: self.0.clone(),
            slot: None,
        }
    }
}
pub struct VirtualCan {
    bus: Arc<Mutex<Bus>>,
    slot: Option<usize>,
}
impl Source for VirtualCan {
    fn list(&mut self, limit: usize) -> Result<Vec<Channel>, Error> {
        if limit == 0 || limit > 64 {
            return Err(Error::InvalidConfig);
        }
        Ok(vec![Channel {
            id: 1,
            name: "virtual-can".into(),
            tx: Availability::Available,
        }])
    }
    fn activate(&mut self, channel: u32, options: &Options) -> Result<(), Error> {
        if self.slot.is_some() {
            return Err(Error::InvalidState);
        }
        if channel != 1 {
            return Err(Error::InvalidConfig);
        }
        options.validate()?;
        let mut bus = self.bus.lock().map_err(|_| Error::Backend)?;
        let slot = if let Some(i) = bus.endpoints.iter().position(Option::is_none) {
            i
        } else {
            if bus.endpoints.len() == 64 {
                return Err(Error::Exhausted);
            }
            bus.endpoints.push(None);
            bus.endpoints.len() - 1
        };
        bus.endpoints[slot] = Some(Endpoint {
            options: options.clone(),
            queue: VecDeque::new(),
            lost: 0,
        });
        self.slot = Some(slot);
        Ok(())
    }
    fn next(&mut self) -> Result<Option<Received>, Error> {
        let mut bus = self.bus.lock().map_err(|_| Error::Backend)?;
        Ok(bus
            .endpoints
            .get_mut(self.slot.ok_or(Error::InvalidState)?)
            .and_then(Option::as_mut)
            .ok_or(Error::Disconnected)?
            .queue
            .pop_front())
    }
    fn send(&mut self, frame: &Frame) -> Result<(), Error> {
        if frame.error() {
            return Err(Error::Unsupported);
        }
        let slot = self.slot.ok_or(Error::InvalidState)?;
        let mut bus = self.bus.lock().map_err(|_| Error::Backend)?;
        bus.time = bus.time.checked_add(1).ok_or(Error::Exhausted)?;
        let time = bus.time;
        let capacity = bus.capacity;
        for (i, endpoint) in bus.endpoints.iter_mut().enumerate() {
            let Some(endpoint) = endpoint else {
                continue;
            };
            if i == slot || !endpoint.options.accepts(frame) {
                continue;
            }
            if endpoint.queue.len() == capacity {
                endpoint.lost = endpoint.lost.saturating_add(1);
                continue;
            }
            endpoint.queue.push_back(Received {
                frame: frame.clone(),
                direction: Direction::Rx,
                source_timestamp_ns: Some(time),
            });
        }
        Ok(())
    }
    fn close(&mut self) {
        if let Some(slot) = self.slot.take()
            && let Ok(mut bus) = self.bus.lock()
        {
            bus.endpoints[slot] = None;
        }
    }
    fn loss(&mut self) -> Option<Option<u64>> {
        let mut bus = self.bus.lock().ok()?;
        let endpoint = bus.endpoints.get_mut(self.slot?)?.as_mut()?;
        let lost = std::mem::take(&mut endpoint.lost);
        (lost != 0).then_some(Some(lost))
    }
}
impl Drop for VirtualCan {
    fn drop(&mut self) {
        self.close();
    }
}
