//! Manual synthetic performance and soak benchmark. See docs/performance.md.
use egui_kittest::Harness;
use exmon_app::{
    charts,
    incoming::{RETAINED, RETAINED_BYTES},
    sessions::{Limits, State},
    shell::{Batch, Captured},
};
use exmon_domain::{
    Direction,
    observation::{Metadata, Observation},
};
use exmon_gui::{Shell, egui};
use std::{
    io,
    path::Path,
    thread,
    time::{Duration, Instant},
};

const WORKLOADS: [&str; 3] = ["small_packets", "mixed_fragmented", "chart_heavy"];
const RELEASE_PHASE: u64 = 40 * 60;
const SMOKE_PHASE: u64 = 5;
const FRAME_MS: usize = 200;
const UI_STALL_MS: u128 = 1000;
const GPU_MS: u128 = 1000;
const MIN_PACKETS_PER_SECOND: f64 = 1024.;
const MIN_FRAMES_PER_SECOND: f64 = 20.;
const MAX_APP_DROP_RATIO: f64 = 0.10;
const MAX_DISPLAY_RATIO: f64 = 0.99;
const RECORDING_BYTES: u64 = 16 * 1024 * 1024;
const RECORD_INTERVAL: Duration = Duration::from_secs(60);

#[derive(Default)]
struct Phase {
    frames: u64,
    frames_by_ten_ms: [u64; FRAME_MS / 10 + 1],
    max_ui_us: u128,
    max_retained: usize,
    max_retained_bytes: usize,
    max_gpu_ms: u128,
    gpu_frames: u64,
    packets: u64,
    bytes: u64,
    processed: u64,
    chart_samples: u64,
    backend_loss: u64,
    app_drops: u64,
    display_decimation: u64,
    processing_output_decimation: u64,
    recording_drops: u64,
    record_bursts: u64,
    max_stop_ms: u128,
    committed: u64,
    fragments: u64,
    reassembled: bool,
}

impl Phase {
    fn frame(&mut self, app: &Shell, elapsed: Duration) {
        self.frames += 1;
        let us = elapsed.as_micros();
        let bucket = us.div_ceil(10_000).min(FRAME_MS as u128 / 10) as usize;
        self.frames_by_ten_ms[bucket] += 1;
        self.max_ui_us = self.max_ui_us.max(us);
        self.max_retained = self.max_retained.max(app.retained());
        self.max_retained_bytes = self
            .max_retained_bytes
            .max(app.incoming.view.retained_bytes());
        let status = app.status();
        self.packets = status.packets;
        self.bytes = status.bytes;
        self.backend_loss = status.backend_loss;
        self.app_drops = status.application_drops + app.incoming.status.input_dropped;
        self.display_decimation = status.display_decimation;
        self.processing_output_decimation = app.incoming.status.output_decimation;
        self.processed = app.incoming.status.processed;
        self.chart_samples = app.charts.snapshot.processed;
    }

    fn p95_ms(&self) -> usize {
        let wanted = (self.frames * 95).div_ceil(100);
        let mut count = 0;
        self.frames_by_ten_ms
            .iter()
            .position(|n| {
                count += n;
                count >= wanted
            })
            .unwrap_or(FRAME_MS / 10)
            * 10
    }

    fn passing(&self, index: usize, elapsed: Duration, required: Duration) -> bool {
        let seconds = elapsed.as_secs_f64();
        elapsed >= required
            && self.packets as f64 / seconds >= MIN_PACKETS_PER_SECOND
            && self.frames as f64 / seconds >= MIN_FRAMES_PER_SECOND
            && self.processed > 0
            && self.max_ui_us < UI_STALL_MS * 1000
            && self.p95_ms() < 50
            && self.max_gpu_ms < GPU_MS
            && self.gpu_frames > 0
            && self.max_retained <= RETAINED
            && self.max_retained_bytes <= RETAINED_BYTES
            && self.backend_loss == 0
            && self.app_drops as f64 / (self.packets + self.fragments) as f64 <= MAX_APP_DROP_RATIO
            && self.display_decimation as f64 / self.packets as f64 <= MAX_DISPLAY_RATIO
            && self.committed > 2
            && (index != 1 || (self.fragments > 0 && self.reassembled))
            && (index != 2 || self.chart_samples > 0)
    }

    fn report(&self, index: usize, elapsed: Duration, required: Duration) {
        let seconds = elapsed.as_secs_f64();
        println!(
            "phase={} seconds={:.3} packets={} packets_per_sec={:.1} bytes={} processed={} processed_per_sec={:.1} frames={} frames_per_sec={:.1} ui_p95_ms={} ui_max_us={} gpu_frames={} gpu_max_ms={} retained={} retained_bytes={} backend_loss={} app_drops={} display_decimation={} processing_output_decimation={} chart_samples={} fragments={} reassembled={} committed={} recording_drops={} record_bursts={} record_stop_max_ms={} pass={}",
            WORKLOADS[index],
            seconds,
            self.packets,
            self.packets as f64 / seconds,
            self.bytes,
            self.processed,
            self.processed as f64 / seconds,
            self.frames,
            self.frames as f64 / seconds,
            self.p95_ms(),
            self.max_ui_us,
            self.gpu_frames,
            self.max_gpu_ms,
            self.max_retained,
            self.max_retained_bytes,
            self.backend_loss,
            self.app_drops,
            self.display_decimation,
            self.processing_output_decimation,
            self.chart_samples,
            self.fragments,
            self.reassembled,
            self.committed,
            self.recording_drops,
            self.record_bursts,
            self.max_stop_ms,
            self.passing(index, elapsed, required)
        );
    }
}

fn error(message: &str) -> io::Error {
    io::Error::other(message)
}

fn charts_for_soak() -> Vec<charts::Spec> {
    let selections: Vec<_> = (0..8)
        .map(|i| charts::Selection {
            protocol: "sample32".into(),
            field: if i % 2 == 0 { "sequence" } else { "interface" }.into(),
            label: format!("Series {}", i + 1),
            unit: String::new(),
        })
        .collect();
    // One selected chart rendered while four chart histories are calculated.
    (1..=4)
        .map(|id| charts::Spec {
            id,
            name: format!("Benchmark {id}"),
            samples: charts::SAMPLES,
            average: 256,
            series: selections.clone(),
        })
        .collect()
}

fn fragmented_batch(generation: u64, sequence: u64) -> Batch {
    let mut bytes = [0; 32];
    bytes[0] = 165;
    bytes[1] = 1;
    bytes[2..10].copy_from_slice(&sequence.to_le_bytes());
    let mut records: Vec<_> = bytes
        .chunks(16)
        .enumerate()
        .map(|(i, payload)| Captured {
            interface_id: 2,
            epoch: 1,
            sequence: sequence + i as u64,
            observation: Observation::new(
                sequence * 1000 + i as u64,
                Direction::Rx,
                Metadata::Serial {
                    configuration_revision: 1,
                },
                Some(payload.len()),
                payload,
            )
            .expect("valid synthetic split"),
        })
        .collect();
    records.push(Captured {
        interface_id: 3,
        epoch: 1,
        sequence,
        observation: Observation::new(
            sequence * 1000 + 2,
            Direction::Rx,
            Metadata::Can {
                channel: 1,
                id: 3,
                extended: false,
                fd: false,
                remote: false,
                error: false,
                dlc: 8,
            },
            Some(8),
            &[165, 2, 0, 0, 0, 0, 0, 1],
        )
        .expect("valid synthetic CAN"),
    });
    Batch {
        generation,
        records,
    }
}

fn recording_start(app: &Shell, path: &Path) -> io::Result<()> {
    let config = app
        .incoming
        .worker
        .configuration()
        .ok_or_else(|| error("missing configuration"))?;
    app.recorder
        .start(
            path.to_str()
                .ok_or_else(|| error("invalid destination encoding"))?,
            Limits {
                bytes: RECORDING_BYTES,
                records: 32_000,
            },
            config,
            app.charts.specs.clone(),
        )
        .map_err(error)
}

fn recording_finish(app: &Shell, phase: &mut Phase, path: &Path) -> io::Result<()> {
    let started = Instant::now();
    app.recorder.stop();
    let deadline = started + Duration::from_secs(5);
    while app.recorder.busy() {
        if Instant::now() >= deadline {
            return Err(error("recording finalization exceeded five seconds"));
        }
        thread::sleep(Duration::from_millis(2));
    }
    let view = app
        .recorder
        .snapshot()
        .ok_or_else(|| error("recording status unavailable"))?;
    let summary = view
        .summary
        .as_ref()
        .ok_or_else(|| error("no storage summary"))?;
    if view.state != State::Stopped
        || !summary.closed
        || std::fs::metadata(path)?.len() > RECORDING_BYTES
    {
        return Err(error("recording failed to finalize within its budget"));
    }
    phase.committed += summary.records;
    phase.recording_drops += summary.dropped;
    phase.record_bursts += 1;
    phase.max_stop_ms = phase.max_stop_ms.max(started.elapsed().as_millis());
    let deadline = Instant::now() + Duration::from_millis(500);
    loop {
        match std::fs::remove_file(path) {
            #[cfg(windows)]
            Err(e) if e.raw_os_error() == Some(32) && Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(5));
            }
            result => return result,
        }
    }
}

fn run_phase(
    h: &mut Harness<'_, Shell>,
    index: usize,
    duration: Duration,
    path: &Path,
) -> io::Result<bool> {
    {
        let app = h.state_mut();
        app.chart_view = index == 2;
        app.start_capture()
            .map_err(|_| error("synthetic capture could not start"))?;
    }
    let setup = Instant::now() + Duration::from_secs(5);
    let generation = loop {
        h.step();
        let app = h.state();
        if let Some(generation) = app.status().active_generation
            && app.incoming.status.generation == generation
            && app.incoming.status.ready
        {
            break generation;
        }
        if Instant::now() >= setup {
            return Err(error("capture startup timeout"));
        }
        thread::sleep(Duration::from_millis(2));
    };
    let mut phase = Phase::default();
    let started = Instant::now();
    let mut next_recording = started;
    let mut record_until = None;
    let mut next_gpu = started;
    let mut next_fragment = started;
    while started.elapsed() < duration {
        let now = Instant::now();
        if now >= next_recording {
            recording_start(h.state(), path)?;
            record_until = Some(now + Duration::from_millis(1250));
            next_recording = now + RECORD_INTERVAL;
        }
        if record_until.is_some_and(|end| now >= end) {
            recording_finish(h.state(), &mut phase, path)?;
            record_until = None;
        }
        if index == 1 && now >= next_fragment {
            let batch = fragmented_batch(generation, phase.fragments);
            let submitted = h.state().incoming.worker.submit(batch);
            if !matches!(submitted, Ok(()) | Err(exmon_app::incoming::Error::Busy)) {
                return Err(error("fragment submission failed"));
            }
            phase.fragments += 2;
            next_fragment = now + Duration::from_millis(35);
        }
        let frame = Instant::now();
        h.step();
        phase.frame(h.state(), frame.elapsed());
        if index == 1 && !phase.reassembled {
            phase.reassembled = h.state().incoming.view.rows.iter().any(|row| {
                row.interface == 2
                    && row.sources.len() == 2
                    && row.fields.iter().any(|f| f.numeric.is_some())
            });
        }
        if now >= next_gpu {
            let frame = Instant::now();
            h.render()
                .map_err(|_| error("offscreen GPU render failed"))?;
            phase.max_gpu_ms = phase.max_gpu_ms.max(frame.elapsed().as_millis());
            phase.gpu_frames += 1;
            next_gpu = now + Duration::from_secs(30);
        }
        if h.state().status().failed || h.state().incoming.status.failed {
            return Err(error("capture or processing worker failed"));
        }
        thread::sleep(Duration::from_millis(3));
    }
    let elapsed = started.elapsed();
    if record_until.is_some() {
        recording_finish(h.state(), &mut phase, path)?;
    }
    h.state_mut().stop_capture();
    h.step();
    phase.report(index, elapsed, duration);
    Ok(phase.passing(index, elapsed, duration))
}

fn main() -> io::Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let [mode, path] = args.as_slice() else {
        return Err(error(
            "usage: performance_soak --smoke|--release-gate NEW_SESSION_PATH",
        ));
    };
    let duration = match mode.to_str() {
        Some("--smoke") => Duration::from_secs(SMOKE_PHASE),
        Some("--release-gate") => Duration::from_secs(RELEASE_PHASE),
        _ => return Err(error("invalid benchmark mode")),
    };
    let path = Path::new(path);
    if path.exists() {
        return Err(error("benchmark session destination exists"));
    }
    let mut app = Shell::new(None).map_err(|_| error("prototype failed to start"))?;
    app.charts.specs = charts_for_soak();
    let mut h = Harness::builder()
        .with_size(egui::vec2(1360., 900.))
        .build_ui_state(|ui, app: &mut Shell| app.show(ui), app);
    h.step();
    h.render()
        .map_err(|_| error("offscreen GPU warm-up failed"))?;
    let mut passed = true;
    for index in 0..WORKLOADS.len() {
        passed &= run_phase(&mut h, index, duration, path)?;
    }
    if !passed {
        return Err(error("performance thresholds exceeded"));
    }
    println!("workloads=3 mode={} verdict=pass", mode.to_string_lossy());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_verdict_rejects_regression_and_missing_coverage() {
        let required = Duration::from_secs(RELEASE_PHASE);
        let mut phase = Phase {
            frames: RELEASE_PHASE * 25,
            packets: RELEASE_PHASE * 1024,
            processed: 1,
            max_ui_us: 190_000,
            gpu_frames: 1,
            committed: 3,
            fragments: 2,
            reassembled: true,
            chart_samples: 1,
            ..Phase::default()
        };
        phase.frames_by_ten_ms[2] = phase.frames;
        assert!(phase.passing(1, required, required));
        assert!(!phase.passing(1, required - Duration::from_secs(1), required));
        phase.reassembled = false;
        assert!(!phase.passing(1, required, required));
        phase.chart_samples = 0;
        assert!(!phase.passing(2, required, required));
        phase.max_ui_us = UI_STALL_MS * 1000;
        assert!(!phase.passing(0, required, required));
        phase.max_ui_us = 190_000;
        phase.backend_loss = 1;
        assert!(!phase.passing(0, required, required));
    }
}
