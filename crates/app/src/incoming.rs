//! Worker-prepared incoming inspection. Definitions and copies grant no I/O authority.
use crate::shell::{Batch as CaptureBatch, Captured};
use exmon_domain::{
    Direction,
    config::{Escape, Framing, ProjectConfig, Protocol},
    fields::{Literal, Number},
    observation::{Metadata, Observation, TransportKind},
};
use exmon_protocol::{
    checksum::Registry,
    classification::{Classifier, GroupId, Outcome, Quality},
    codec::Codec,
    framing::{Event, Framer, SourceSpan},
};
use std::{
    collections::{BTreeMap, VecDeque},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread::{self, JoinHandle, ThreadId},
    time::{Duration, Instant},
};
pub const RETAINED: usize = 1024;
pub const RETAINED_BYTES: usize = 8 * 1024 * 1024;
const BATCH_BYTES: usize = 1024 * 1024;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Busy,
    Invalid,
    Unavailable,
    Exhausted,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Key {
    pub generation: u64,
    pub id: u64,
}
pub struct FieldView {
    pub name: String,
    pub value: String,
    pub invalid: bool,
}
pub struct Row {
    pub quality: Option<Quality>,
    pub key: Key,
    pub interface: u64,
    pub epoch: u64,
    pub sequence: u64,
    pub observation: Arc<Observation>,
    pub normalized: Option<Arc<[u8]>>,
    pub group: Option<String>,
    pub name: String,
    pub revision: u64,
    pub protocol: Option<Arc<Protocol>>,
    pub fields: Vec<FieldView>,
    pub status: &'static str,
    pub source: String,
    pub destination: String,
    pub sources: Vec<SourceSpan>,
}
impl Row {
    pub fn bytes(&self) -> &[u8] {
        self.normalized
            .as_deref()
            .unwrap_or_else(|| self.observation.bytes())
    }
    pub fn raw(&self) -> bool {
        self.normalized.is_none()
    }
    pub fn cost(&self) -> usize {
        1024 + self.observation.bytes().len()
            + self.bytes().len()
            + self.sources.len() * 64
            + self
                .fields
                .iter()
                .map(|f| f.name.len() + f.value.len() + 64)
                .sum::<usize>()
    }
    pub fn copy_format(self: &Arc<Self>) -> Option<Draft> {
        self.normalized.as_ref()?;
        Some(Draft { row: self.clone() })
    }
}
/// An immutable received-format draft, without target, ticket, permission or schedule.
pub struct Draft {
    pub row: Arc<Row>,
}
#[derive(Clone)]
pub struct Group {
    pub key: String,
    pub name: String,
    pub count: u64,
    pub bytes: u64,
    pub per_second: Option<f64>,
}
#[derive(Clone, Default)]
pub struct Status {
    pub generation: u64,
    pub ready: bool,
    pub failed: bool,
    pub rename_error: bool,
    pub processed: u64,
    pub input_dropped: u64,
    pub output_decimation: u64,
    pub processing_skipped: u64,
    pub group_overflow: u64,
    pub groups: Vec<Group>,
}
struct Shared {
    quit: AtomicBool,
    generation: AtomicU64,
    input_dropped: AtomicU64,
    status: Mutex<Status>,
    config: Mutex<Option<Arc<ProjectConfig>>>,
    owner: OnceLock<ThreadId>,
}
pub struct PreparedBatch {
    pub generation: u64,
    pub rows: Vec<Arc<Row>>,
}
struct Rename {
    key: String,
    name: String,
}
pub struct Worker {
    shared: Arc<Shared>,
    input: SyncSender<CaptureBatch>,
    names: SyncSender<Rename>,
    results: Receiver<PreparedBatch>,
    thread: Option<JoinHandle<()>>,
}
impl Worker {
    pub fn prototype() -> Result<Self, Error> {
        Self::factory(|| {
            let config=exmon_storage::config::load_project(br#"{"schema_version":1,"protocols":[{"id":"sample32","name":"Sample telemetry","framing":{"kind":"fixed","length":32},"classification":{"length":32,"prefix":[165]},"fields":[{"id":"sequence","offset":2,"length":8,"representation":"unsigned","endian":"little"},{"id":"interface","offset":1,"length":1,"representation":"unsigned","endian":"little"}]},{"id":"sample8","name":"CAN sample","framing":{"kind":"fixed","length":8},"classification":{"length":8,"prefix":[165]},"fields":[{"id":"counter","offset":2,"length":4,"representation":"unsigned","endian":"little"}]}]}"#).map_err(|_|Error::Invalid)?;
            Ok((config, Some((Framing::Fixed { length: 32 }, None))))
        })
    }
    pub fn configured(
        config: ProjectConfig,
        stream: Option<(Framing, Option<Escape>)>,
    ) -> Result<Self, Error> {
        Self::factory(move || Ok((config, stream)))
    }
    fn factory(
        factory: impl FnOnce() -> Result<(ProjectConfig, Option<(Framing, Option<Escape>)>), Error>
        + Send
        + 'static,
    ) -> Result<Self, Error> {
        let shared = Arc::new(Shared {
            quit: AtomicBool::new(false),
            generation: AtomicU64::new(0),
            input_dropped: AtomicU64::new(0),
            status: Mutex::new(Status::default()),
            config: Mutex::new(None),
            owner: OnceLock::new(),
        });
        let (input, rx) = mpsc::sync_channel::<CaptureBatch>(8);
        let (names, nr) = mpsc::sync_channel::<Rename>(4);
        let (tx, results) = mpsc::sync_channel(8);
        let state = shared.clone();
        let handle = thread::Builder::new()
            .name("exmon-incoming".into())
            .spawn(move || {
                let _ = state.owner.set(thread::current().id());
                let mut processor = match factory().and_then(|(c, s)| Processor::new(c, s)) {
                    Ok(p) => p,
                    Err(_) => {
                        if let Ok(mut s) = state.status.lock() {
                            s.failed = true;
                        }
                        return;
                    }
                };
                if let Ok(mut c) = state.config.lock() {
                    *c = Some(Arc::new(processor.config.clone()));
                }
                let mut generation = 0;
                let mut status = Status {
                    ready: true,
                    ..Default::default()
                };
                while !state.quit.load(Ordering::Acquire) {
                    let desired = state.generation.load(Ordering::Acquire);
                    if desired != generation {
                        generation = desired;
                        processor.reset();
                        status = Status {
                            ready: true,
                            generation,
                            ..Default::default()
                        };
                    }
                    if let Ok(rename) = nr.try_recv() {
                        status.rename_error = processor.rename(&rename.key, &rename.name).is_err();
                        if !status.rename_error {
                            if let Ok(mut c) = state.config.lock() {
                                *c = Some(Arc::new(processor.config.clone()));
                            }
                            for g in &mut status.groups {
                                if g.key == rename.key {
                                    g.name = rename.name.clone();
                                }
                            }
                        }
                    }
                    if let Ok(batch) = rx.try_recv() {
                        if batch.generation != generation || generation == 0 {
                            continue;
                        }
                        let mut rows = vec![];
                        let mut bytes = 0;
                        for record in batch.records {
                            status.processed = status.processed.saturating_add(1);
                            match processor.process(generation, record, &mut status) {
                                Ok(prepared) => {
                                    for row in prepared {
                                        let cost = row.cost();
                                        if rows.len() < 96
                                            && cost <= BATCH_BYTES
                                            && bytes + cost <= BATCH_BYTES
                                        {
                                            bytes += cost;
                                            rows.push(row);
                                        } else {
                                            status.processing_skipped =
                                                status.processing_skipped.saturating_add(1);
                                        }
                                    }
                                }
                                Err(_) => status.failed = true,
                            }
                        }
                        processor.rates(&mut status);
                        let count = rows.len() as u64;
                        if tx.try_send(PreparedBatch { generation, rows }).is_err() {
                            status.output_decimation =
                                status.output_decimation.saturating_add(count);
                        }
                    }
                    if let Ok(mut s) = state.status.lock() {
                        *s = status.clone();
                    }
                    thread::park_timeout(Duration::from_millis(1));
                }
            })
            .map_err(|_| Error::Unavailable)?;
        Ok(Self {
            shared,
            input,
            names,
            results,
            thread: Some(handle),
        })
    }
    pub fn begin(&self, generation: u64) -> Result<(), Error> {
        if generation == 0 {
            return Err(Error::Invalid);
        }
        self.shared
            .generation
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |old| {
                if generation >= old {
                    Some(generation)
                } else {
                    None
                }
            })
            .map_err(|_| Error::Invalid)?;
        if let Some(t) = &self.thread {
            t.thread().unpark();
        }
        Ok(())
    }
    pub fn submit(&self, batch: CaptureBatch) -> Result<(), Error> {
        if batch.records.len() > 32
            || batch
                .records
                .iter()
                .map(|r| r.observation.bytes().len())
                .sum::<usize>()
                > 65536
        {
            return Err(Error::Invalid);
        }
        let count = batch.records.len() as u64;
        self.input.try_send(batch).map_err(|e| {
            let _ = self.shared.input_dropped.fetch_update(
                Ordering::Relaxed,
                Ordering::Relaxed,
                |old| Some(old.saturating_add(count)),
            );
            match e {
                mpsc::TrySendError::Full(_) => Error::Busy,
                mpsc::TrySendError::Disconnected(_) => Error::Unavailable,
            }
        })?;
        if let Some(t) = &self.thread {
            t.thread().unpark();
        }
        Ok(())
    }
    pub fn rename(&self, key: &str, name: &str) -> Result<(), Error> {
        if key.len() > 140 || !exmon_domain::classification::valid_name(name) {
            return Err(Error::Invalid);
        }
        self.names
            .try_send(Rename {
                key: key.into(),
                name: name.into(),
            })
            .map_err(|_| Error::Busy)?;
        if let Some(t) = &self.thread {
            t.thread().unpark();
        }
        Ok(())
    }
    pub fn status(&self) -> Option<Status> {
        let mut s = self.shared.status.try_lock().ok()?.clone();
        s.input_dropped = self.shared.input_dropped.load(Ordering::Relaxed);
        Some(s)
    }
    pub fn configuration(&self) -> Option<Arc<ProjectConfig>> {
        self.shared.config.try_lock().ok()?.clone()
    }
    pub fn worker_thread(&self) -> Option<ThreadId> {
        self.shared.owner.get().copied()
    }
    pub fn drain(&mut self) -> Vec<PreparedBatch> {
        self.results.try_iter().take(4).collect()
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.shared.quit.store(true, Ordering::Release);
        if let Some(t) = self.thread.take() {
            t.thread().unpark();
            let _ = t.join();
        }
    }
}
struct Stream {
    quality: Quality,
    framer: Framer,
    metadata: Metadata,
    direction: Direction,
    interface: u64,
    epoch: u64,
    last: Option<u64>,
}
struct Processor {
    config: ProjectConfig,
    classifier: Classifier,
    codecs: BTreeMap<String, (Arc<Protocol>, Codec)>,
    stream_rule: Option<(Framing, Option<Escape>)>,
    stream: Option<Stream>,
    next: u64,
    revision: u64,
    start: Instant,
}
impl Processor {
    fn new(
        config: ProjectConfig,
        stream_rule: Option<(Framing, Option<Escape>)>,
    ) -> Result<Self, Error> {
        exmon_storage::config::save_project(&config).map_err(|_| Error::Invalid)?;
        if let Some((f, e)) = &stream_rule {
            Framer::new(f.clone(), *e).map_err(|_| Error::Invalid)?;
        }
        let classifier = Classifier::new(&config, 1).map_err(|_| Error::Invalid)?;
        let registry = Registry::default();
        let mut codecs = BTreeMap::new();
        for p in &config.protocols {
            codecs.insert(
                p.id.clone(),
                (
                    Arc::new(p.clone()),
                    Codec::new(p, &registry).map_err(|_| Error::Invalid)?,
                ),
            );
        }
        Ok(Self {
            config,
            classifier,
            codecs,
            stream_rule,
            stream: None,
            next: 0,
            revision: 1,
            start: Instant::now(),
        })
    }
    fn reset(&mut self) {
        self.stream = None;
        self.start = Instant::now();
    }
    fn rename(&mut self, key: &str, name: &str) -> Result<(), Error> {
        let mut config = self.config.clone();
        if let Some(id) = key.strip_prefix("rule:") {
            config.rename_type(id, name)
        } else if let Some(id) = key.strip_prefix("auto:") {
            config.rename_auto_group(id, name)
        } else {
            return Err(Error::Invalid);
        }
        .map_err(|_| Error::Invalid)?;
        let revision = self.revision.checked_add(1).ok_or(Error::Exhausted)?;
        let classifier = Classifier::new(&config, revision).map_err(|_| Error::Invalid)?;
        self.config = config;
        self.classifier = classifier;
        self.revision = revision;
        Ok(())
    }
    fn rates(&self, status: &mut Status) {
        let elapsed = self.start.elapsed().as_secs_f64();
        for g in &mut status.groups {
            g.per_second = if elapsed > 0. {
                Some(g.count as f64 / elapsed)
            } else {
                None
            };
        }
    }
    fn row(
        &mut self,
        generation: u64,
        record: &Captured,
        observation: &Arc<Observation>,
    ) -> Result<Row, Error> {
        self.next = self.next.checked_add(1).ok_or(Error::Exhausted)?;
        let (source, destination) = endpoints(observation.metadata(), observation.direction());
        Ok(Row {
            quality: None,
            key: Key {
                generation,
                id: self.next,
            },
            interface: record.interface_id,
            epoch: record.epoch,
            sequence: record.sequence,
            observation: observation.clone(),
            normalized: None,
            group: None,
            name: "Raw observation".into(),
            revision: self.revision,
            protocol: None,
            fields: vec![],
            status: "Raw capture",
            source,
            destination,
            sources: vec![],
        })
    }
    fn process(
        &mut self,
        generation: u64,
        record: Captured,
        status: &mut Status,
    ) -> Result<Vec<Arc<Row>>, Error> {
        let observation = Arc::new(record.observation.clone());
        let mut raw = self.row(generation, &record, &observation)?;
        let is_stream = matches!(
            observation.metadata().kind(),
            TransportKind::Tcp | TransportKind::Serial
        );
        let mut messages: Vec<(Vec<u8>, Quality, Vec<SourceSpan>)> = vec![];
        if is_stream {
            let Some((rule, escape)) = &self.stream_rule else {
                raw.status = "Stream framing is not configured";
                return Ok(vec![Arc::new(raw)]);
            };
            if self.stream.as_ref().is_none_or(|s| {
                s.metadata != *observation.metadata()
                    || s.direction != observation.direction()
                    || s.interface != record.interface_id
                    || s.epoch != record.epoch
            }) {
                self.stream = Some(Stream {
                    quality: Quality::Complete,
                    framer: Framer::new(rule.clone(), *escape).map_err(|_| Error::Invalid)?,
                    metadata: observation.metadata().clone(),
                    direction: observation.direction(),
                    interface: record.interface_id,
                    epoch: record.epoch,
                    last: None,
                });
            }
            let s = self.stream.as_mut().ok_or(Error::Invalid)?;
            if s.last
                .is_some_and(|last| last.checked_add(1) != Some(record.sequence))
            {
                let _ = s.framer.gap(observation.timestamp_ns());
                raw.status = "Stream gap: incomplete bytes discarded";
            }
            s.last = Some(record.sequence);
            if observation.original_length().is_none() {
                s.quality = Quality::Unknown;
            }
            match s.framer.observe(record.sequence, &observation) {
                Ok(events) => {
                    for event in events {
                        match event {
                            Event::Message(m) => {
                                if messages.len() < 64 {
                                    messages.push((m.bytes().to_vec(), s.quality, m.sources));
                                } else {
                                    status.processing_skipped =
                                        status.processing_skipped.saturating_add(1);
                                }
                            }
                            Event::Diagnostic { .. } => {
                                raw.status = "Framing diagnostic: inspect raw capture"
                            }
                        }
                    }
                    if messages.is_empty() && raw.status == "Raw capture" {
                        raw.status = "Stream fragment: awaiting boundary";
                    }
                    if s.framer.stats().pending_source_bytes == 0 {
                        s.quality = Quality::Complete;
                    }
                }
                Err(_) => {
                    raw.status = "Framing failed: raw capture retained";
                    self.stream = None;
                }
            }
        } else {
            let decision = self
                .classifier
                .classify_observation(&observation)
                .map_err(|_| Error::Invalid)?;
            messages.push((observation.bytes().to_vec(), decision.quality, vec![]));
        }
        let mut rows = vec![Arc::new(raw)];
        for (bytes, quality, sources) in messages {
            let decision = self
                .classifier
                .classify(
                    observation.metadata(),
                    observation.direction(),
                    &bytes,
                    quality,
                )
                .map_err(|_| Error::Invalid)?;
            let mut row = self.row(generation, &record, &observation)?;
            row.normalized = Some(Arc::from(bytes));
            row.quality = Some(quality);
            row.sources = sources;
            row.name = decision.name;
            row.status = match decision.outcome {
                Outcome::Matched => "Matched",
                Outcome::Ambiguous => "Ambiguous",
                Outcome::Unmatched => "Unmatched",
                Outcome::Incomplete => "Incomplete",
            };
            let key = match decision.group {
                GroupId::Rule(id) => {
                    if let Some((protocol, codec)) = self.codecs.get(&id) {
                        row.protocol = Some(protocol.clone());
                        match codec.decode(row.bytes()) {
                            Ok(draft) => {
                                let report = draft.inspect();
                                for field in report.fields {
                                    if let Some(def) = protocol.fields.get(field.index) {
                                        row.fields.push(FieldView {
                                            name: def.id.clone(),
                                            value: field_text(
                                                &field.raw,
                                                &field.number,
                                                field.label.as_deref(),
                                                &def.unit,
                                            ),
                                            invalid: !field.violations.is_empty(),
                                        });
                                    }
                                }
                                if !report.checksums.iter().all(|c| c.result.is_ok()) {
                                    row.status = "Checksum validation failed";
                                }
                            }
                            Err(_) => row.status = "Field decoding failed",
                        }
                    }
                    format!("rule:{id}")
                }
                GroupId::Auto(id) => format!("auto:{id}"),
            };
            let found = status.groups.iter().position(|g| g.key == key);
            let permitted = found.is_some()
                || key.starts_with("rule:")
                || status
                    .groups
                    .iter()
                    .filter(|g| g.key.starts_with("auto:"))
                    .count()
                    < 64;
            if permitted {
                let index = found.unwrap_or_else(|| {
                    status.groups.push(Group {
                        key: key.clone(),
                        name: row.name.clone(),
                        count: 0,
                        bytes: 0,
                        per_second: None,
                    });
                    status.groups.len() - 1
                });
                let g = &mut status.groups[index];
                g.count = g.count.saturating_add(1);
                g.bytes = g.bytes.saturating_add(row.bytes().len() as u64);
                row.group = Some(key);
            } else {
                status.group_overflow = status.group_overflow.saturating_add(1);
                row.status = "Automatic group capacity reached";
            }
            rows.push(Arc::new(row));
        }
        Ok(rows)
    }
}
fn field_text(
    raw: &Option<Literal>,
    number: &Result<Number, exmon_protocol::fields::Issue>,
    label: Option<&str>,
    unit: &str,
) -> String {
    let value = match number {
        Ok(Number::Integer(n)) => n.to_string(),
        Ok(Number::Real(n)) => n.to_string(),
        Err(_) => match raw {
            Some(Literal::Bytes { value }) => {
                let head: String = value.iter().take(24).map(|v| format!("{v:02X} ")).collect();
                if value.len() > 24 {
                    format!("{head}... ({} bytes; full data in HEX)", value.len())
                } else {
                    head
                }
            }
            Some(Literal::Text { value }) => {
                let head: String = value.chars().take(96).collect();
                if head.len() < value.len() {
                    format!("{head}... (full data in HEX)")
                } else {
                    head
                }
            }
            Some(Literal::Unsigned { value }) => value.to_string(),
            Some(Literal::Signed { value }) => value.to_string(),
            Some(Literal::Float32 { bits }) => format!("IEEE32 bits {bits:08X}"),
            Some(Literal::Float64 { bits }) => format!("IEEE64 bits {bits:016X}"),
            _ => "Unavailable".into(),
        },
    };
    format!(
        "{value} {unit}{}",
        label.map(|v| format!(" ({v})")).unwrap_or_default()
    )
}
fn endpoints(metadata: &Metadata, direction: Direction) -> (String, String) {
    let pair = match metadata {
        Metadata::Tcp { local, remote, .. } | Metadata::Udp { local, remote } => {
            (remote.to_string(), local.to_string())
        }
        Metadata::Ethernet { link_type } => (
            format!("Link type {link_type}"),
            "Address unavailable".into(),
        ),
        Metadata::Serial {
            configuration_revision,
        } => (
            format!("Serial revision {configuration_revision}"),
            "Host".into(),
        ),
        Metadata::Usb { endpoint, .. } => (format!("USB endpoint {endpoint:02X}"), "Host".into()),
        Metadata::Hid {
            usage_page, usage, ..
        } => (format!("HID {usage_page:04X}:{usage:04X}"), "Host".into()),
        Metadata::UsbMon(m) => (
            format!("USB trace endpoint {:02X}", m.endpoint),
            "Host controller".into(),
        ),
        Metadata::Can { channel, id, .. } => {
            (format!("CAN {channel} / {id:X}"), "Bus observer".into())
        }
    };
    if direction == Direction::Tx {
        (pair.1, pair.0)
    } else {
        pair
    }
}
#[derive(Default)]
pub struct View {
    pub generation: u64,
    pub rows: VecDeque<Arc<Row>>,
    pub selected: Option<Arc<Row>>,
    pub tab: Option<String>,
    pub paused: bool,
    pub paused_skipped: u64,
    pub evicted: u64,
    bytes: usize,
}
impl View {
    pub fn reset(&mut self, generation: u64) {
        *self = Self {
            generation,
            ..Default::default()
        };
    }
    pub fn clear(&mut self) {
        self.rows.clear();
        self.selected = None;
        self.bytes = 0;
        self.evicted = 0;
    }
    pub fn apply(&mut self, batch: PreparedBatch) {
        if batch.generation != self.generation {
            return;
        }
        if self.paused {
            self.paused_skipped = self.paused_skipped.saturating_add(batch.rows.len() as u64);
            return;
        }
        for row in batch.rows {
            let cost = row.cost();
            if cost > BATCH_BYTES {
                continue;
            }
            while self.rows.len() >= RETAINED || self.bytes + cost > RETAINED_BYTES {
                if let Some(old) = self.rows.pop_front() {
                    self.bytes -= old.cost();
                    self.evicted = self.evicted.saturating_add(1);
                } else {
                    break;
                }
            }
            self.bytes += cost;
            self.rows.push_back(row);
        }
    }
    pub fn visible(&self) -> Vec<Arc<Row>> {
        self.rows
            .iter()
            .filter(|r| match &self.tab {
                None => r.raw(),
                Some(key) => r.group.as_ref() == Some(key),
            })
            .cloned()
            .collect()
    }
    pub fn select_tab(&mut self, key: Option<String>) {
        self.tab = key;
        self.selected = self.visible().last().cloned();
    }
    pub fn select(&mut self, key: Key) {
        if let Some(row) = self.rows.iter().find(|r| r.key == key) {
            self.selected = Some(row.clone());
        }
    }
    pub fn selected_evicted(&self) -> bool {
        self.selected
            .as_ref()
            .is_some_and(|s| !self.rows.iter().any(|r| r.key == s.key))
    }
    pub fn retained_bytes(&self) -> usize {
        self.bytes
    }
}
