use exmon_capture::{adapter::*, serial::*};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

struct Wire {
    incoming: VecDeque<u8>,
    outgoing: VecDeque<u8>,
    connected: bool,
    opened: bool,
    settings: Option<Settings>,
    partial: bool,
}
impl Default for Wire {
    fn default() -> Self {
        Self {
            incoming: VecDeque::new(),
            outgoing: VecDeque::new(),
            connected: true,
            opened: false,
            settings: None,
            partial: false,
        }
    }
}
struct VirtualPort(Arc<Mutex<Wire>>);
impl Source for VirtualPort {
    fn list(&mut self, _: usize) -> Result<Vec<String>, Error> {
        Ok(vec!["virtual-pair-A".into()])
    }
    fn open(&mut self, _: &str, settings: Settings) -> Result<(), Error> {
        if settings.electrical != ElectricalMode::Unchanged {
            return Err(Error::Unsupported);
        }
        let mut wire = self.0.lock().unwrap();
        if !wire.connected {
            return Err(Error::Disconnected);
        }
        wire.opened = true;
        wire.settings = Some(settings);
        Ok(())
    }
    fn read(&mut self, buffer: &mut [u8]) -> Result<usize, Error> {
        let mut wire = self.0.lock().unwrap();
        if !wire.connected {
            return Err(Error::Disconnected);
        }
        assert!(wire.opened);
        let count = buffer.len().min(wire.incoming.len());
        for byte in &mut buffer[..count] {
            *byte = wire.incoming.pop_front().unwrap();
        }
        Ok(count)
    }
    fn write(&mut self, bytes: &[u8]) -> Result<usize, Error> {
        let mut wire = self.0.lock().unwrap();
        if !wire.connected {
            return Err(Error::Disconnected);
        }
        assert!(wire.opened);
        let count = if wire.partial { 1 } else { bytes.len() };
        if wire.outgoing.len() + count > 64 {
            return Err(Error::Backend);
        }
        wire.outgoing.extend(&bytes[..count]);
        Ok(count)
    }
    fn close(&mut self) {
        self.0.lock().unwrap().opened = false;
    }
}
fn pair(settings: Settings) -> (Serial<VirtualPort>, Arc<Mutex<Wire>>) {
    let wire = Arc::new(Mutex::new(Wire::default()));
    (
        Serial::new(VirtualPort(wire.clone()), settings).unwrap(),
        wire,
    )
}
fn start(adapter: &mut Serial<VirtualPort>) {
    adapter.enumerate(64).unwrap();
    adapter.open(0).unwrap();
    adapter.configure(16).unwrap();
    adapter.start().unwrap();
}
fn inject(wire: &Arc<Mutex<Wire>>, bytes: &[u8]) {
    let mut wire = wire.lock().unwrap();
    assert!(wire.incoming.len() + bytes.len() <= 64);
    wire.incoming.extend(bytes);
}

#[test]
fn virtual_pair_verifies_fragmented_framing_input_and_explicit_tx_output() {
    let settings = Settings {
        baud: 115200,
        data_bits: CharSize::Bits7,
        parity: Parity::Even,
        stop_bits: StopBits::Two,
        flow: FlowControl::XonXoff,
        timeout_ms: 25,
        ..Settings::default()
    };
    let (mut adapter, peer) = pair(settings);
    adapter.enumerate(64).unwrap();
    adapter.open(0).unwrap();
    adapter.configure(16).unwrap();
    assert!(!peer.lock().unwrap().opened);
    assert_eq!(adapter.transmit(&[1]), Err(Error::InvalidState));
    adapter.start().unwrap();
    assert!(peer.lock().unwrap().outgoing.is_empty());
    let actual = peer.lock().unwrap().settings.unwrap();
    assert_eq!(actual.baud, 115200);
    assert_eq!(actual.data_bits, CharSize::Bits7);
    assert_eq!(actual.parity, Parity::Even);
    assert_eq!(actual.stop_bits, StopBits::Two);
    assert_eq!(actual.flow, FlowControl::XonXoff);
    assert_eq!(actual.timeout_ms, 25);
    let token = Cancellation::default();
    let mut input: Vec<u8> = Vec::new();
    for part in [&[165][..], &[0, 42][..]] {
        inject(&peer, part);
        let batch = adapter.poll(&token, 2, 16).unwrap();
        assert_eq!(batch.len(), 1);
        input.extend_from_slice(batch[0].bytes());
    }
    assert_eq!(input, [165, 0, 42]);
    assert!(adapter.poll(&token, 2, 16).unwrap().is_empty());
    assert_eq!(adapter.transmit(&[165, 0, 7]).unwrap(), 3);
    assert_eq!(
        peer.lock().unwrap().outgoing.drain(..).collect::<Vec<_>>(),
        [165, 0, 7]
    );
    adapter.stop().unwrap();
    assert_eq!(adapter.transmit(&[1]), Err(Error::InvalidState));
}

#[test]
fn disconnect_and_reconnect_preserve_unrelated_session() {
    let (mut first, peer) = pair(Settings::default());
    let (mut second, other) = pair(Settings::default());
    start(&mut first);
    start(&mut second);
    peer.lock().unwrap().connected = false;
    assert_eq!(
        first.poll(&Cancellation::default(), 1, 16).err(),
        Some(Error::Disconnected)
    );
    assert_eq!(first.transmit(&[1]), Err(Error::InvalidState));
    inject(&other, &[9, 8]);
    assert_eq!(
        second.poll(&Cancellation::default(), 1, 16).unwrap()[0].bytes(),
        [9, 8]
    );
    first.close().unwrap();
    peer.lock().unwrap().connected = true;
    start(&mut first);
    assert!(peer.lock().unwrap().outgoing.is_empty());
    inject(&peer, &[7]);
    assert_eq!(
        first.poll(&Cancellation::default(), 1, 16).unwrap()[0].bytes(),
        [7]
    );
    assert!(other.lock().unwrap().opened);
}

#[test]
fn cancellation_bounds_partial_writes_and_unsupported_direction_are_honest() {
    let (mut adapter, peer) = pair(Settings::default());
    start(&mut adapter);
    inject(&peer, &[1, 2, 3]);
    let token = Cancellation::default();
    token.cancel();
    assert_eq!(adapter.poll(&token, 1, 16).err(), Some(Error::Cancelled));
    assert_eq!(peer.lock().unwrap().incoming.len(), 3);
    peer.lock().unwrap().partial = true;
    assert_eq!(adapter.transmit(&[4, 5, 6]), Ok(1));
    assert_eq!(peer.lock().unwrap().outgoing.len(), 1);
    assert!(adapter.transmit(&[]).is_err());
    assert_eq!(adapter.direction_control(), Availability::Unsupported);
    let settings = Settings {
        electrical: ElectricalMode::Rs485 {
            full_duplex: false,
            invert_rts: false,
            before_ms: 0,
            after_ms: 0,
        },
        ..Settings::default()
    };
    let (mut unsupported, peer) = pair(settings);
    unsupported.enumerate(64).unwrap();
    unsupported.open(0).unwrap();
    unsupported.configure(8).unwrap();
    assert_eq!(unsupported.start(), Err(Error::Unsupported));
    assert!(!peer.lock().unwrap().opened);
    assert!(
        Settings {
            timeout_ms: 0,
            ..Settings::default()
        }
        .validate()
        .is_err()
    );
    assert!(
        Settings {
            flow: FlowControl::RtsCts,
            ..settings
        }
        .validate()
        .is_err()
    );
}
