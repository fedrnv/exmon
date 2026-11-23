//! Bounded live chart history. No native I/O, decoding or transmission authority.
use crate::incoming::{PreparedBatch, Row};
use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, SyncSender},
    },
    thread::{self, JoinHandle},
    time::Duration,
};
pub const CHARTS: usize = 4;
pub const SERIES: usize = 8;
pub const SAMPLES: usize = 4096;
pub const POINTS: usize = 512;
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct Selection {
    pub protocol: String,
    pub field: String,
    pub label: String,
    pub unit: String,
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct Spec {
    pub id: u64,
    pub name: String,
    pub samples: usize,
    pub average: usize,
    pub series: Vec<Selection>,
}
impl Spec {
    pub(crate) fn valid(&self) -> bool {
        self.id > 0
            && !self.name.is_empty()
            && self.name.len() <= 64
            && !self.name.chars().any(char::is_control)
            && (1..=SAMPLES).contains(&self.samples)
            && (1..=self.samples).contains(&self.average)
            && self.series.len() <= SERIES
            && self.series.iter().all(|s| {
                [&s.protocol, &s.field, &s.label]
                    .into_iter()
                    .all(|v| !v.is_empty() && v.len() <= 64 && !v.chars().any(char::is_control))
                    && s.unit.len() <= 32
                    && !s.unit.chars().any(char::is_control)
            })
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Point {
    pub index: u64,
    pub seconds: f64,
    pub value: Option<f64>,
    pub average: Option<f64>,
    pub segment: u64,
}
#[derive(Clone)]
pub struct SeriesView {
    pub selection: Selection,
    pub retained: usize,
    pub evicted: u64,
    pub decimated: usize,
    pub invalid: u64,
    pub points: Arc<[Point]>,
}
#[derive(Clone)]
pub struct ChartView {
    pub spec: Spec,
    pub series: Vec<SeriesView>,
}
#[derive(Clone, Default)]
pub struct Snapshot {
    pub revision: u64,
    pub generation: u64,
    pub processed: u64,
    pub input_dropped: u64,
    pub failed: bool,
    pub configuration_error: bool,
    pub charts: Vec<ChartView>,
}
struct History {
    selection: Selection,
    points: VecDeque<Point>,
    recent: VecDeque<f64>,
    sum: f64,
    index: u64,
    segment: u64,
    evicted: u64,
    invalid: u64,
    last_time: Option<u64>,
}
impl History {
    fn new(selection: Selection) -> Self {
        Self {
            selection,
            points: VecDeque::new(),
            recent: VecDeque::new(),
            sum: 0.,
            index: 0,
            segment: 0,
            evicted: 0,
            invalid: 0,
            last_time: None,
        }
    }
    fn gap(&mut self) {
        self.segment = self.segment.saturating_add(1);
        self.recent.clear();
        self.sum = 0.;
    }
    fn push(&mut self, time: u64, origin: u64, value: Option<f64>, spec: &Spec) {
        if self.index >= (1u64 << 53) {
            self.invalid = self.invalid.saturating_add(1);
            self.gap();
            return;
        }
        let ordered = self.last_time.is_none_or(|t| time >= t) && time >= origin;
        self.last_time = Some(time);
        let value = value.filter(|v| v.is_finite() && ordered);
        let average = if let Some(v) = value {
            let n = spec.average as f64;
            if self.recent.len() == spec.average {
                self.sum -= self.recent.pop_front().unwrap_or(0.) / n;
            }
            self.recent.push_back(v);
            self.sum += v / n;
            if self.index.is_multiple_of(4096) {
                self.sum = self.recent.iter().map(|x| x / n).sum();
            }
            let a = self.sum / (self.recent.len() as f64 / n);
            a.is_finite().then_some(a)
        } else {
            self.invalid = self.invalid.saturating_add(1);
            self.gap();
            None
        };
        self.points.push_back(Point {
            index: self.index,
            seconds: time.saturating_sub(origin) as f64 / 1e9,
            value,
            average,
            segment: self.segment,
        });
        self.index += 1;
        while self.points.len() > spec.samples {
            self.points.pop_front();
            self.evicted = self.evicted.saturating_add(1);
        }
    }
    fn view(&self) -> SeriesView {
        let all: Vec<_> = self.points.iter().copied().collect();
        let points = decimate(&all);
        SeriesView {
            selection: self.selection.clone(),
            retained: all.len(),
            evicted: self.evicted,
            decimated: all.len().saturating_sub(points.len()),
            invalid: self.invalid,
            points: points.into(),
        }
    }
}
/// Ordered bucket extrema for both raw and averaged lines; endpoints/gaps retained.
pub fn decimate(all: &[Point]) -> Vec<Point> {
    if all.len() <= POINTS {
        return all.to_vec();
    }
    let width = all.len().div_ceil(POINTS / 8);
    let mut out = Vec::with_capacity(POINTS);
    for chunk in all.chunks(width) {
        let mut ids = vec![0, chunk.len() - 1];
        for average in [false, true] {
            let get = |p: &Point| if average { p.average } else { p.value };
            let mut valid = chunk
                .iter()
                .enumerate()
                .filter_map(|(i, p)| get(p).map(|v| (i, v)));
            if let Some(first) = valid.next() {
                let (mut lo, mut hi) = (first, first);
                for x in valid {
                    if x.1 < lo.1 {
                        lo = x;
                    }
                    if x.1 > hi.1 {
                        hi = x;
                    }
                }
                ids.extend([lo.0, hi.0]);
            }
        }
        if let Some(i) = chunk.iter().position(|p| p.value.is_none()) {
            ids.push(i);
        }
        ids.sort_unstable();
        ids.dedup();
        out.extend(ids.into_iter().map(|i| chunk[i]));
    }
    out
}
struct Chart {
    spec: Spec,
    series: Vec<History>,
}
#[derive(Default)]
pub struct Engine {
    charts: Vec<Chart>,
    generation: u64,
    origin: Option<u64>,
    processed: u64,
}
impl Engine {
    pub fn configure(&mut self, specs: Vec<Spec>) -> bool {
        if specs.len() > CHARTS
            || specs.iter().any(|s| !s.valid())
            || specs
                .iter()
                .enumerate()
                .any(|(i, s)| specs[..i].iter().any(|x| x.id == s.id))
        {
            return false;
        }
        let mut old = std::mem::take(&mut self.charts);
        self.charts = specs
            .into_iter()
            .map(|spec| {
                if let Some(i) = old.iter().position(|c| c.spec.id == spec.id) {
                    let mut c = old.remove(i);
                    if c.spec.samples == spec.samples && c.spec.average == spec.average {
                        let mut previous = std::mem::take(&mut c.series);
                        c.series = spec
                            .series
                            .iter()
                            .map(|s| {
                                previous
                                    .iter()
                                    .position(|h| h.selection == *s)
                                    .map(|i| previous.remove(i))
                                    .unwrap_or_else(|| History::new(s.clone()))
                            })
                            .collect();
                        c.spec = spec;
                        return c;
                    }
                }
                Chart {
                    series: spec.series.iter().cloned().map(History::new).collect(),
                    spec,
                }
            })
            .collect();
        true
    }
    pub fn begin(&mut self, generation: u64) {
        if self.generation != generation {
            self.generation = generation;
            self.origin = None;
            self.processed = 0;
            for c in &mut self.charts {
                c.series = c.spec.series.iter().cloned().map(History::new).collect();
            }
        }
    }
    pub fn gap(&mut self) {
        for c in &mut self.charts {
            for h in &mut c.series {
                h.gap();
            }
        }
    }
    pub fn sample(&mut self, protocol: &str, field: &str, time: u64, value: Option<f64>) {
        let origin = *self.origin.get_or_insert(time);
        self.processed = self.processed.saturating_add(1);
        for c in &mut self.charts {
            for h in &mut c.series {
                if h.selection.protocol == protocol && h.selection.field == field {
                    h.push(time, origin, value, &c.spec);
                }
            }
        }
    }
    pub fn row(&mut self, row: &Row) {
        let Some(protocol) = &row.protocol else {
            return;
        };
        if row.raw() {
            return;
        }
        self.begin(row.key.generation);
        for f in &row.fields {
            self.sample(
                &protocol.id,
                &f.name,
                row.observation.timestamp_ns(),
                f.numeric,
            );
        }
    }
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            generation: self.generation,
            processed: self.processed,
            charts: self
                .charts
                .iter()
                .map(|c| ChartView {
                    spec: c.spec.clone(),
                    series: c.series.iter().map(History::view).collect(),
                })
                .collect(),
            ..Default::default()
        }
    }
}
struct Shared {
    quit: AtomicBool,
    generation: AtomicU64,
    dropped: AtomicU64,
    configuration: Mutex<Option<(u64, Vec<Spec>)>>,
    snapshot: Mutex<Snapshot>,
}
struct Input {
    batch: PreparedBatch,
    serial: u64,
}
struct Exit(Arc<Shared>);
impl Drop for Exit {
    fn drop(&mut self) {
        if let Ok(mut s) = self.0.snapshot.lock() {
            s.failed = true;
        }
    }
}
pub struct Worker {
    shared: Arc<Shared>,
    input: SyncSender<Input>,
    serial: AtomicU64,
    thread: Option<JoinHandle<()>>,
    revision: AtomicU64,
}
impl Worker {
    pub fn new() -> Result<Self, std::io::Error> {
        let shared = Arc::new(Shared {
            quit: AtomicBool::new(false),
            generation: AtomicU64::new(0),
            dropped: AtomicU64::new(0),
            configuration: Mutex::new(None),
            snapshot: Mutex::new(Snapshot::default()),
        });
        let (input, rx) = mpsc::sync_channel::<Input>(8);
        let state = shared.clone();
        let handle = thread::Builder::new()
            .name("exmon-charts".into())
            .spawn(move || {
                let _exit = Exit(state.clone());
                let mut last_serial: Option<u64> = None;
                let mut engine = Engine::default();
                let mut revision = 0;
                let mut error = false;
                let mut dropped = 0;
                while !state.quit.load(Ordering::Acquire) {
                    if let Ok(mut pending) = state.configuration.lock()
                        && let Some((r, specs)) = pending.take()
                    {
                        error = !engine.configure(specs);
                        revision = r;
                    }
                    engine.begin(state.generation.load(Ordering::Acquire));
                    let observed = state.dropped.load(Ordering::Acquire);
                    if observed != dropped {
                        engine.gap();
                        dropped = observed;
                    }
                    if let Ok(input) = rx.recv_timeout(Duration::from_millis(16))
                        && input.batch.generation == state.generation.load(Ordering::Acquire)
                    {
                        if last_serial.is_some_and(|n| n.checked_add(1) != Some(input.serial)) {
                            engine.gap();
                        }
                        last_serial = Some(input.serial);
                        let batch = input.batch;
                        for row in batch.rows {
                            if row.key.generation == batch.generation {
                                engine.row(&row);
                            }
                        }
                    }
                    let mut snapshot = engine.snapshot();
                    snapshot.revision = revision;
                    snapshot.configuration_error = error;
                    snapshot.input_dropped = dropped;
                    if let Ok(mut view) = state.snapshot.lock() {
                        *view = snapshot;
                    }
                }
            })?;
        Ok(Self {
            shared,
            input,
            thread: Some(handle),
            revision: AtomicU64::new(0),
            serial: AtomicU64::new(0),
        })
    }
    pub fn configure(&self, specs: Vec<Spec>) -> Result<u64, &'static str> {
        // Structural admission before queuing; all numeric/history computation is worker-owned.
        if specs.len() > CHARTS || specs.iter().any(|s| !s.valid()) {
            return Err("Invalid chart configuration");
        }
        let revision = self
            .revision
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |r| r.checked_add(1))
            .map_err(|_| "Chart revision exhausted")?
            + 1;
        *self
            .shared
            .configuration
            .lock()
            .map_err(|_| "Chart worker unavailable")? = Some((revision, specs));
        Ok(revision)
    }
    pub fn begin(&self, generation: u64) {
        self.shared.generation.store(generation, Ordering::Release);
    }
    pub fn submit(&self, batch: PreparedBatch) -> bool {
        let serial = self
            .serial
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| n.checked_add(1));
        if serial.is_err()
            || batch.rows.len() > 96
            || batch
                .rows
                .iter()
                .try_fold(0usize, |total, r| total.checked_add(r.cost()))
                .is_none_or(|n| n > 1_048_576)
        {
            let _ = self
                .shared
                .dropped
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| {
                    Some(n.saturating_add(1))
                });
            return false;
        }
        if self
            .input
            .try_send(Input {
                batch,
                serial: serial.unwrap_or(0),
            })
            .is_err()
        {
            let _ = self
                .shared
                .dropped
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| {
                    Some(n.saturating_add(1))
                });
            return false;
        }
        true
    }
    pub fn snapshot(&self) -> Option<Snapshot> {
        self.shared.snapshot.try_lock().ok().map(|s| s.clone())
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.shared.quit.store(true, Ordering::Release);
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn full_input_queue_never_blocks_submit_and_exposes_loss() {
        let w = Worker::new().unwrap();
        w.begin(1);
        let guard = w.shared.snapshot.lock().unwrap();
        let start = std::time::Instant::now();
        let mut rejected = 0;
        for _ in 0..64 {
            if !w.submit(PreparedBatch {
                generation: 1,
                rows: vec![],
            }) {
                rejected += 1;
            }
        }
        assert!(start.elapsed() < Duration::from_millis(100));
        assert!(rejected > 0);
        drop(guard);
        let end = std::time::Instant::now() + Duration::from_secs(2);
        loop {
            if w.snapshot().is_some_and(|s| s.input_dropped > 0) {
                break;
            }
            assert!(std::time::Instant::now() < end);
            thread::sleep(Duration::from_millis(2));
        }
    }
}
