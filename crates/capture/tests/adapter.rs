use exmon_capture::adapter::*;
use exmon_domain::{
    Direction,
    metrics::Counter,
    observation::{Metadata, Observation, TransportKind},
};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Control {
    calls: Vec<&'static str>,
    error: Option<Error>,
    oversized: bool,
    denied: bool,
    fail_close: bool,
}
struct Mock(Arc<Mutex<Control>>);
fn descriptor() -> Descriptor {
    Descriptor {
        id: 7,
        kind: TransportKind::Serial,
        rx: Availability::Available,
        tx: Availability::Unsupported,
    }
}
impl Adapter for Mock {
    fn enumerate(&mut self, _: usize) -> Result<Vec<Descriptor>, Error> {
        self.0.lock().unwrap().calls.push("enumerate");
        Ok(vec![descriptor()])
    }
    fn open(&mut self, id: u64) -> Result<Descriptor, Error> {
        let mut control = self.0.lock().unwrap();
        control.calls.push("open");
        if control.denied {
            return Err(Error::PermissionDenied);
        }
        if id != 7 {
            return Err(Error::Unsupported);
        }
        Ok(descriptor())
    }
    fn configure(&mut self, _: usize) -> Result<(), Error> {
        self.0.lock().unwrap().calls.push("configure");
        Ok(())
    }
    fn start(&mut self) -> Result<(), Error> {
        self.0.lock().unwrap().calls.push("start");
        Ok(())
    }
    fn poll(
        &mut self,
        cancel: &Cancellation,
        _: usize,
        _: usize,
    ) -> Result<Vec<Observation>, Error> {
        let mut control = self.0.lock().unwrap();
        control.calls.push("poll");
        if cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        if let Some(error) = control.error.take() {
            return Err(error);
        }
        (0..if control.oversized { 65 } else { 2 })
            .map(|n| {
                Observation::new(
                    n,
                    Direction::Rx,
                    Metadata::Serial {
                        configuration_revision: 1,
                    },
                    Some(3),
                    &[165, 0, 42],
                )
                .map_err(|_| Error::Backend)
            })
            .collect()
    }
    fn stop(&mut self) -> Result<(), Error> {
        self.0.lock().unwrap().calls.push("stop");
        Ok(())
    }
    fn close(&mut self) -> Result<(), Error> {
        let mut control = self.0.lock().unwrap();
        control.calls.push("close");
        if control.fail_close {
            return Err(Error::Backend);
        }
        Ok(())
    }
    fn take_backend_loss(&mut self) -> Option<Option<u64>> {
        Some(None)
    }
    fn transmit(&mut self, _: &[u8]) -> Result<usize, Error> {
        panic!("lifecycle must never transmit")
    }
}
fn limits() -> Limits {
    Limits {
        queue_items: 3,
        queue_bytes: 8,
        batch_items: 2,
        batch_bytes: 6,
    }
}
fn setup() -> (Session<Mock>, Arc<Mutex<Control>>) {
    let control = Arc::new(Mutex::new(Control::default()));
    (
        Session::new(Mock(control.clone()), limits()).unwrap(),
        control,
    )
}
fn start(session: &mut Session<Mock>) {
    session.open(7).unwrap();
    session.configure(3).unwrap();
    session.start().unwrap();
}

#[test]
fn deterministic_lifecycle_cancel_and_reconnect() {
    let (mut session, control) = setup();
    assert_eq!(session.state(), State::Closed);
    assert_eq!(session.enumerate().unwrap(), vec![descriptor()]);
    assert_eq!(session.start(), Err(Error::InvalidState));
    session.open(7).unwrap();
    assert_eq!(session.start(), Err(Error::InvalidState));
    assert_eq!(session.configure(0), Err(Error::InvalidConfig));
    session.configure(3).unwrap();
    session.start().unwrap();
    assert_eq!(session.configure(3), Err(Error::InvalidState));
    session.pump().unwrap();
    let first = session.drain(100);
    assert_eq!(first.len(), 2);
    assert_eq!(first[0].epoch, 1);
    assert_eq!(first[1].sequence, 1);
    assert_eq!(first[0].observation.bytes(), &[165, 0, 42]);
    assert_eq!(first[1].observation.timestamp_ns(), 1);
    let old_token = session.cancellation();
    old_token.cancel();
    assert_eq!(session.pump(), Err(Error::Cancelled));
    assert_eq!(session.state(), State::Stopped);
    session.reconnect(7).unwrap();
    assert_eq!(session.state(), State::Open);
    assert_eq!(session.start(), Err(Error::InvalidState));
    session.configure(3).unwrap();
    session.start().unwrap();
    old_token.cancel();
    assert!(!session.cancellation().is_cancelled());
    session.pump().unwrap();
    let new = session.drain(1);
    assert_eq!(new[0].epoch, 2);
    assert_eq!(new[0].sequence, 0);
    session.stop().unwrap();
    session.close().unwrap();
    assert_eq!(session.state(), State::Closed);
    let calls = control.lock().unwrap().calls.clone();
    assert_eq!(
        &calls[..5],
        &["enumerate", "open", "configure", "start", "poll"]
    );
    assert_eq!(calls.iter().filter(|c| **c == "start").count(), 2);
}

#[test]
fn slow_consumer_is_bounded_by_bytes_and_counts_with_visible_losses() {
    let (mut session, _) = setup();
    start(&mut session);
    for _ in 0..100 {
        session.pump().unwrap();
    }
    assert_eq!(session.queued_items(), 2);
    assert_eq!(session.queued_bytes(), 6);
    let health = session.metrics().snapshot();
    assert_eq!(health.count(Counter::RxPackets), 200);
    assert_eq!(health.count(Counter::ApplicationDrops), 198);
    assert_eq!(health.count(Counter::BackendLossUnknown), 100);
    assert_eq!(health.count(Counter::DisplayDecimation), 0);
    assert_eq!(health.queue_depth, 2);
    session.drain(1);
    session.pump().unwrap();
    let remaining = session.drain(2);
    assert_eq!(remaining[0].sequence, 1);
    assert_eq!(remaining[1].sequence, 200);
    assert_eq!(session.queued_bytes(), 0);
    let control = Arc::new(Mutex::new(Control::default()));
    let mut count_limited = Session::new(
        Mock(control),
        Limits {
            queue_items: 1,
            queue_bytes: 100,
            ..limits()
        },
    )
    .unwrap();
    start(&mut count_limited);
    count_limited.pump().unwrap();
    assert_eq!(count_limited.queued_items(), 1);
    assert_eq!(
        count_limited
            .metrics()
            .snapshot()
            .count(Counter::ApplicationDrops),
        1
    );
}

#[test]
fn errors_permission_and_backend_contract_failure_are_explicit() {
    let (mut session, control) = setup();
    control.lock().unwrap().denied = true;
    assert_eq!(session.open(7), Err(Error::PermissionDenied));
    assert_eq!(session.state(), State::Closed);
    control.lock().unwrap().denied = false;
    start(&mut session);
    control.lock().unwrap().error = Some(Error::Disconnected);
    assert_eq!(session.pump(), Err(Error::Disconnected));
    assert_eq!(session.last_error(), Some(Error::Disconnected));
    assert_eq!(session.state(), State::Disconnected);
    session.stop().unwrap();
    assert_eq!(session.state(), State::Disconnected);
    assert_eq!(session.start(), Err(Error::InvalidState));
    assert!(!session.metrics().snapshot().connected);
    session.reconnect(7).unwrap();
    session.configure(3).unwrap();
    session.start().unwrap();
    control.lock().unwrap().oversized = true;
    assert_eq!(session.pump(), Err(Error::Backend));
    assert_eq!(session.state(), State::Failed);
    assert_eq!(session.queued_bytes(), 0);
    assert!(session.cancellation().is_cancelled());
    assert!(
        Session::new(
            Mock(control),
            Limits {
                queue_items: 0,
                ..limits()
            }
        )
        .is_err()
    );
}

#[test]
fn snapshot_limit_and_failed_close_cannot_be_bypassed() {
    let (mut session, control) = setup();
    session.open(7).unwrap();
    session.configure(2).unwrap();
    session.start().unwrap();
    assert_eq!(session.pump(), Err(Error::Backend));
    control.lock().unwrap().fail_close = true;
    assert_eq!(session.close(), Err(Error::Backend));
    assert_eq!(session.state(), State::Failed);
    assert_eq!(session.open(7), Err(Error::InvalidState));
    control.lock().unwrap().fail_close = false;
    session.close().unwrap();
    assert_eq!(session.state(), State::Closed);
}
