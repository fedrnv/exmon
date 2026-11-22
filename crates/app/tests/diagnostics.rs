use exmon_app::diagnostics::{log_health, redacted};
use exmon_domain::metrics::{Counter, Metrics};
use std::{
    io::{self, Write},
    sync::{
        Arc, Mutex,
        mpsc::{self, SyncSender},
    },
    thread,
    time::Duration,
};

#[derive(Clone)]
struct Sink {
    bytes: Arc<Mutex<Vec<u8>>>,
    attempted: Option<SyncSender<()>>,
}
impl Write for Sink {
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        if let Some(signal) = &self.attempted {
            let _ = signal.try_send(());
        }
        self.bytes.lock().unwrap().extend_from_slice(input);
        Ok(input.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn structured_output_redacts_payload_and_identifiers() {
    let bytes = Arc::new(Mutex::new(Vec::new()));
    let sink = Sink {
        bytes: bytes.clone(),
        attempted: None,
    };
    let subscriber = tracing_subscriber::fmt()
        .without_time()
        .with_ansi(false)
        .with_writer(move || sink.clone())
        .finish();
    let metrics = Metrics::default();
    metrics.set_connected(true);
    metrics.add(Counter::DecodeFailures, 2);
    metrics.set_queue_depth(3);
    tracing::subscriber::with_default(subscriber, || {
        log_health(
            &metrics.snapshot(),
            b"synthetic-secret-payload",
            "synthetic-secret-device",
        );
    });
    let text = String::from_utf8(bytes.lock().unwrap().clone()).unwrap();
    assert!(text.contains("exmon::health"));
    assert!(text.contains("decode_failures=2"));
    assert!(text.contains("queue_depth=3"));
    assert!(text.contains("health=Degraded"));
    assert!(text.contains("payload=[REDACTED]"));
    assert!(text.contains("device_id=[REDACTED]"));
    assert!(!text.contains("synthetic-secret"));
    struct Explosive;
    impl std::fmt::Debug for Explosive {
        fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            panic!("sensitive formatter invoked")
        }
    }
    assert_eq!(format!("{:?}", redacted(&Explosive)), "[REDACTED]");
    assert_eq!(
        format!("{}", redacted("synthetic-secret-device")),
        "[REDACTED]"
    );
}

#[test]
fn blocked_log_sink_cannot_block_metric_producers() {
    let bytes = Arc::new(Mutex::new(Vec::new()));
    let guard = bytes.lock().unwrap();
    let (attempt_tx, attempt_rx) = mpsc::sync_channel(1);
    let sink = Sink {
        bytes: bytes.clone(),
        attempted: Some(attempt_tx),
    };
    let subscriber = tracing_subscriber::fmt()
        .without_time()
        .with_ansi(false)
        .with_writer(move || sink.clone())
        .finish();
    let metrics = Arc::new(Metrics::default());
    let logged = metrics.clone();
    let logger = thread::spawn(move || {
        tracing::subscriber::with_default(subscriber, || {
            log_health(&logged.snapshot(), b"synthetic", "synthetic")
        })
    });
    let blocked = attempt_rx.recv_timeout(Duration::from_secs(5));
    let (done_tx, done_rx) = mpsc::sync_channel(1);
    let producer_metrics = metrics.clone();
    let producer = thread::spawn(move || {
        for _ in 0..10_000 {
            producer_metrics.add(Counter::RxPackets, 1);
        }
        done_tx
            .send(producer_metrics.snapshot().count(Counter::RxPackets))
            .unwrap();
    });
    let result = done_rx.recv_timeout(Duration::from_secs(5));
    // Release the sink before assertions so even a failed test cleans up threads.
    drop(guard);
    producer.join().unwrap();
    logger.join().unwrap();
    assert!(blocked.is_ok(), "logger must reach the blocked sink");
    assert_eq!(result.unwrap(), 10_000);
    assert!(!bytes.lock().unwrap().is_empty());
}
