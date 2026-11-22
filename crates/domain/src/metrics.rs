//! Fixed-memory, lock-free metrics shared by worker layers and GUI consumers.
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Instant,
};

const COUNT: usize = 12;

/// Different pipeline loss stages must never be merged into one packet count.
#[derive(Debug, Clone, Copy)]
#[repr(usize)]
pub enum Counter {
    RxPackets,
    RxBytes,
    TxPackets,
    TxBytes,
    BackendLoss,
    BackendLossUnknown,
    ApplicationDrops,
    DisplayDecimation,
    DecodeFailures,
    TransmissionErrors,
    RecordingLoss,
    ValidationRejections,
}

/// One collector per interface/session epoch. Reconnect creates a new collector.
pub struct Metrics {
    source: Arc<()>,
    counters: [AtomicU64; COUNT],
    queue_depth: AtomicU64,
    connected: AtomicBool,
}

impl Default for Metrics {
    fn default() -> Self {
        Self {
            source: Arc::new(()),
            counters: std::array::from_fn(|_| AtomicU64::new(0)),
            queue_depth: AtomicU64::new(0),
            connected: AtomicBool::new(false),
        }
    }
}

impl Metrics {
    /// No allocation, mutex, callback, logging or I/O. CAS retries are lock-free,
    /// not a hard real-time/wait-free latency guarantee under contention.
    pub fn add(&self, counter: Counter, amount: u64) {
        let _ = self.counters[counter as usize].fetch_update(
            Ordering::Relaxed,
            Ordering::Relaxed,
            |old| Some(old.saturating_add(amount)),
        );
    }

    pub fn set_queue_depth(&self, depth: u64) {
        self.queue_depth.store(depth, Ordering::Relaxed);
    }

    pub fn set_connected(&self, connected: bool) {
        self.connected.store(connected, Ordering::Relaxed);
    }

    pub fn backend_loss(&self, count: Option<u64>) {
        match count {
            Some(count) => self.add(Counter::BackendLoss, count),
            None => self.add(Counter::BackendLossUnknown, 1),
        }
    }

    /// Individually atomic samples, intentionally not a transactional snapshot.
    pub fn snapshot(&self) -> HealthSnapshot {
        HealthSnapshot {
            source: self.source.clone(),
            sampled_at: Instant::now(),
            counters: std::array::from_fn(|i| self.counters[i].load(Ordering::Relaxed)),
            queue_depth: self.queue_depth.load(Ordering::Relaxed),
            connected: self.connected.load(Ordering::Relaxed),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Health {
    Disconnected,
    Healthy,
    Degraded,
}

#[derive(Debug, Clone)]
pub struct HealthSnapshot {
    source: Arc<()>,
    sampled_at: Instant,
    counters: [u64; COUNT],
    pub queue_depth: u64,
    pub connected: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct Throughput {
    pub rx_packets_per_second: f64,
    pub rx_bytes_per_second: f64,
    pub tx_packets_per_second: f64,
    pub tx_bytes_per_second: f64,
}

impl HealthSnapshot {
    pub fn count(&self, counter: Counter) -> u64 {
        self.counters[counter as usize]
    }

    pub fn saturated(&self) -> bool {
        self.counters.contains(&u64::MAX)
    }

    pub fn health(&self) -> Health {
        if !self.connected {
            return Health::Disconnected;
        }
        if self.saturated()
            || [
                Counter::BackendLoss,
                Counter::BackendLossUnknown,
                Counter::ApplicationDrops,
                Counter::DecodeFailures,
                Counter::TransmissionErrors,
                Counter::RecordingLoss,
                Counter::ValidationRejections,
            ]
            .iter()
            .any(|c| self.count(*c) != 0)
        {
            Health::Degraded
        } else {
            Health::Healthy
        }
    }

    /// Reject different sources, reversed/equal sample times and saturated totals.
    /// Samples represent ingest/submission rates, never confirmed wire throughput.
    pub fn throughput_since(&self, previous: &Self) -> Option<Throughput> {
        if !Arc::ptr_eq(&self.source, &previous.source) || self.saturated() || previous.saturated()
        {
            return None;
        }
        let elapsed = self
            .sampled_at
            .checked_duration_since(previous.sampled_at)?
            .as_secs_f64();
        if elapsed <= 0.0 {
            return None;
        }
        let rate = |counter| {
            Some(self.count(counter).checked_sub(previous.count(counter))? as f64 / elapsed)
        };
        Some(Throughput {
            rx_packets_per_second: rate(Counter::RxPackets)?,
            rx_bytes_per_second: rate(Counter::RxBytes)?,
            tx_packets_per_second: rate(Counter::TxPackets)?,
            tx_bytes_per_second: rate(Counter::TxBytes)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{thread, time::Duration};

    #[test]
    fn stages_health_and_saturation_remain_distinct() {
        let metrics = Metrics::default();
        assert_eq!(metrics.snapshot().health(), Health::Disconnected);
        metrics.set_connected(true);
        assert_eq!(metrics.snapshot().health(), Health::Healthy);
        metrics.add(Counter::DisplayDecimation, 4);
        assert_eq!(metrics.snapshot().health(), Health::Healthy);
        metrics.backend_loss(Some(2));
        metrics.backend_loss(None);
        metrics.add(Counter::ApplicationDrops, 3);
        metrics.add(Counter::DecodeFailures, 5);
        metrics.add(Counter::TransmissionErrors, 6);
        metrics.add(Counter::RecordingLoss, 7);
        metrics.add(Counter::ValidationRejections, 8);
        metrics.set_queue_depth(42);
        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.health(), Health::Degraded);
        for (c, n) in [
            (Counter::BackendLoss, 2),
            (Counter::BackendLossUnknown, 1),
            (Counter::ApplicationDrops, 3),
            (Counter::DisplayDecimation, 4),
            (Counter::DecodeFailures, 5),
            (Counter::TransmissionErrors, 6),
            (Counter::RecordingLoss, 7),
            (Counter::ValidationRejections, 8),
        ] {
            assert_eq!(snapshot.count(c), n);
        }
        assert_eq!(snapshot.queue_depth, 42);
        metrics.add(Counter::RxBytes, u64::MAX);
        metrics.add(Counter::RxBytes, 1);
        assert_eq!(metrics.snapshot().count(Counter::RxBytes), u64::MAX);
        assert!(metrics.snapshot().saturated());
        metrics.set_connected(false);
        assert_eq!(metrics.snapshot().health(), Health::Disconnected);
    }

    #[test]
    fn concurrent_producers_do_not_lose_counts() {
        let metrics = Arc::new(Metrics::default());
        let threads: Vec<_> = (0..4)
            .map(|_| {
                let metrics = metrics.clone();
                thread::spawn(move || {
                    for _ in 0..10_000 {
                        metrics.add(Counter::RxPackets, 1);
                        metrics.add(Counter::RxBytes, 64);
                    }
                })
            })
            .collect();
        for thread in threads {
            thread.join().unwrap();
        }
        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.count(Counter::RxPackets), 40_000);
        assert_eq!(snapshot.count(Counter::RxBytes), 2_560_000);
    }

    #[test]
    fn rates_use_same_epoch_positive_time_and_unsaturated_totals() {
        let metrics = Metrics::default();
        let old = metrics.snapshot();
        metrics.add(Counter::RxPackets, 10);
        metrics.add(Counter::RxBytes, 100);
        metrics.add(Counter::TxPackets, 20);
        metrics.add(Counter::TxBytes, 200);
        let mut new = metrics.snapshot();
        new.sampled_at = old.sampled_at + Duration::from_secs(2);
        let rate = new.throughput_since(&old).unwrap();
        assert_eq!(rate.rx_packets_per_second, 5.0);
        assert_eq!(rate.rx_bytes_per_second, 50.0);
        assert_eq!(rate.tx_packets_per_second, 10.0);
        assert_eq!(rate.tx_bytes_per_second, 100.0);
        assert!(old.throughput_since(&new).is_none());
        assert!(old.throughput_since(&old).is_none());
        assert!(
            new.throughput_since(&Metrics::default().snapshot())
                .is_none()
        );
        metrics.add(Counter::RxPackets, u64::MAX);
        assert!(metrics.snapshot().throughput_since(&old).is_none());
    }
}
