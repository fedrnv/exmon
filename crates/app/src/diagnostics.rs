//! Background logging consumer. Never invoke logging from capture/metric paths.
use exmon_domain::metrics::{Counter, HealthSnapshot};
use std::fmt;

/// Borrowed sensitive context; formatting never invokes the wrapped formatter.
pub struct Redacted<'a, T: ?Sized>(&'a T);
pub fn redacted<T: ?Sized>(value: &T) -> Redacted<'_, T> {
    Redacted(value)
}
impl<T: ?Sized> fmt::Display for Redacted<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let _ = self.0;
        f.write_str("[REDACTED]")
    }
}
impl<T: ?Sized> fmt::Debug for Redacted<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

/// Emit a structured summary from a background worker at a bounded cadence.
/// The sink may block. No arbitrary error text, payload or identifier is emitted.
pub fn log_health(snapshot: &HealthSnapshot, payload: &[u8], device_id: &str) {
    tracing::info!(target: "exmon::health",
        health = ?snapshot.health(), connected = snapshot.connected,
        queue_depth = snapshot.queue_depth, saturated = snapshot.saturated(),
        rx_packets = snapshot.count(Counter::RxPackets), rx_bytes = snapshot.count(Counter::RxBytes),
        tx_packets = snapshot.count(Counter::TxPackets), tx_bytes = snapshot.count(Counter::TxBytes),
        backend_loss = snapshot.count(Counter::BackendLoss),
        backend_loss_unknown = snapshot.count(Counter::BackendLossUnknown),
        application_drops = snapshot.count(Counter::ApplicationDrops),
        display_decimation = snapshot.count(Counter::DisplayDecimation),
        decode_failures = snapshot.count(Counter::DecodeFailures),
        transmission_errors = snapshot.count(Counter::TransmissionErrors),
        recording_loss = snapshot.count(Counter::RecordingLoss),
        validation_rejections = snapshot.count(Counter::ValidationRejections),
        payload = %redacted(payload), device_id = %redacted(device_id),
        "capture health snapshot");
}
