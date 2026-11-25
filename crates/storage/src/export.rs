//! Streaming analysis exports of a single recovered session snapshot.
use crate::sessions::{Event, Recovery, Source};
use exmon_domain::{
    Direction,
    fields::Literal,
    observation::{Metadata, TransportKind},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs::{self, File, OpenOptions},
    io::{self, BufWriter, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

const SCHEMA_VERSION: u32 = 1;
const MAX_OUTPUT_BYTES: u64 = 1024 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Format {
    Csv,
    #[default]
    Json,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Category {
    Observations,
    #[default]
    Fields,
    ChartSeries,
}

#[derive(Default)]
pub struct Selection {
    pub category: Category,
    pub from_ns: Option<u64>,
    pub through_ns: Option<u64>,
    /// Lowercase transport for observations, exact protocol ID otherwise.
    pub type_id: String,
    pub field: String,
}

impl Selection {
    pub fn valid(&self) -> bool {
        self.from_ns
            .zip(self.through_ns)
            .is_none_or(|(a, b)| a <= b)
            && [self.type_id.as_str(), self.field.as_str()]
                .iter()
                .all(|s| s.len() <= 128 && !s.chars().any(char::is_control))
            && (self.category != Category::Observations || self.field.is_empty())
    }

    fn matches(&self, timestamp: u64, type_id: &str) -> bool {
        self.from_ns.is_none_or(|t| timestamp >= t)
            && self.through_ns.is_none_or(|t| timestamp <= t)
            && (self.type_id.is_empty() || self.type_id == type_id)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    Invalid,
    Session(crate::sessions::Error),
    Cancelled,
    Exists,
    Limit,
    Io,
}

#[derive(Serialize)]
struct Session {
    session_closed: bool,
    session_dropped: u64,
    session_records: u64,
    last_loss: Option<Source>,
}

#[derive(Default, Serialize)]
struct Row<'a> {
    kind: &'static str,
    record_id: u64,
    timestamp_ns: u64,
    source_timestamp_ns: Option<u64>,
    #[serde(flatten)]
    source: Option<&'a Source>,
    type_id: &'a str,
    message_name: Option<&'a str>,
    definition_revision: Option<u64>,
    direction: Option<Direction>,
    metadata: Option<&'a Metadata>,
    original_length: Option<usize>,
    bytes_hex: Option<String>,
    field: Option<&'a str>,
    unit: Option<&'a str>,
    value: Option<f64>,
    decoded: Option<&'a Literal>,
    chart_revision: Option<u64>,
    chart_id: Option<u64>,
    chart_name: Option<&'a str>,
    series_index: Option<usize>,
    series_label: Option<&'a str>,
    series_unit: Option<&'a str>,
}

#[derive(Deserialize)]
struct Chart {
    id: u64,
    name: String,
    series: Vec<Series>,
}

#[derive(Deserialize)]
struct Series {
    protocol: String,
    field: String,
    label: String,
    unit: String,
}

const COLUMNS: &[&str] = &[
    "schema_version",
    "kind",
    "session_closed",
    "session_dropped",
    "session_records",
    "last_loss",
    "record_id",
    "timestamp_ns",
    "source_timestamp_ns",
    "generation",
    "interface",
    "epoch",
    "sequence",
    "type_id",
    "message_name",
    "definition_revision",
    "direction",
    "metadata",
    "original_length",
    "bytes_hex",
    "field",
    "unit",
    "value",
    "decoded",
    "chart_revision",
    "chart_id",
    "chart_name",
    "series_index",
    "series_label",
    "series_unit",
];

struct Output {
    writer: BufWriter<File>,
    format: Format,
    bytes: u64,
    rows: u64,
}

impl Output {
    fn write(&mut self, bytes: impl AsRef<[u8]>) -> Result<(), Error> {
        let bytes = bytes.as_ref();
        self.bytes += bytes.len() as u64;
        if self.bytes > MAX_OUTPUT_BYTES {
            return Err(Error::Limit);
        }
        self.writer.write_all(bytes).map_err(|_| Error::Io)
    }

    fn json(&mut self, value: &impl Serialize) -> Result<(), Error> {
        self.write(serde_json::to_vec(value).map_err(|_| Error::Invalid)?)
    }

    fn csv(&mut self, object: &Value) -> Result<(), Error> {
        let mut line = String::new();
        for (i, key) in COLUMNS.iter().enumerate() {
            if i > 0 {
                line.push(',');
            }
            let value = &object[*key];
            match value {
                Value::Null => {}
                Value::String(text) => csv_text(&mut line, text),
                Value::Object(_) | Value::Array(_) => csv_text(&mut line, &value.to_string()),
                _ => line.push_str(&value.to_string()),
            }
        }
        line.push_str("\r\n");
        self.write(line)
    }

    fn row(&mut self, row: &Row<'_>) -> Result<(), Error> {
        match self.format {
            Format::Json => {
                if self.rows > 0 {
                    self.write(",\n")?;
                }
                self.json(row)?;
            }
            Format::Csv => {
                let mut value = serde_json::to_value(row).map_err(|_| Error::Invalid)?;
                value["schema_version"] = SCHEMA_VERSION.into();
                self.csv(&value)?;
            }
        }
        self.rows += 1;
        Ok(())
    }
}

fn csv_text(line: &mut String, text: &str) {
    // Quote all text and neutralize spreadsheet expressions, including hidden
    // prefixes using whitespace, BOM, or ASCII control characters.
    let significant =
        text.trim_start_matches(|c: char| c.is_whitespace() || c.is_control() || c == '\u{feff}');
    line.push('"');
    if text.starts_with(['\t', '\r', '\n'])
        || significant.starts_with(['=', '+', '-', '@', '＝', '＋', '－', '＠'])
    {
        line.push('\'');
    }
    for c in text.chars() {
        if c == '"' {
            line.push('"');
        }
        line.push(c);
    }
    line.push('"');
}

fn transport(metadata: &Metadata) -> &'static str {
    match metadata.kind() {
        TransportKind::Ethernet => "ethernet",
        TransportKind::Tcp => "tcp",
        TransportKind::Udp => "udp",
        TransportKind::Serial => "serial",
        TransportKind::Usb => "usb",
        TransportKind::Can => "can",
    }
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8] = b"0123456789ABCDEF";
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        text.push(char::from(DIGITS[(byte >> 4) as usize]));
        text.push(char::from(DIGITS[(byte & 15) as usize]));
    }
    text
}

struct Staged(PathBuf);

impl Staged {
    fn create(destination: &Path) -> Result<(Self, File), Error> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let parent = destination.parent().ok_or(Error::Invalid)?;
        let time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| Error::Io)?
            .as_nanos();
        let seq = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = parent.join(format!(
            ".exmon-export-{}-{time:x}-{seq:x}.partial",
            std::process::id()
        ));
        let mut options = OpenOptions::new();
        options.create_new(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options.open(&path).map_err(|_| Error::Io)?;
        Ok((Self(path), file))
    }
}

impl Drop for Staged {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn check(cancelled: &impl Fn() -> bool) -> Result<(), Error> {
    if cancelled() {
        Err(Error::Cancelled)
    } else {
        Ok(())
    }
}

/// Call on a worker thread. The final pathname becomes visible only after a
/// complete sync and a last cancellation check. Hard-link publication fails
/// atomically if the destination is taken, including concurrent creation.
pub fn create(
    session: &Recovery,
    destination: &Path,
    format: Format,
    selection: &Selection,
    cancelled: impl Fn() -> bool,
) -> Result<u64, Error> {
    crate::settings_files::local(destination).map_err(|_| Error::Invalid)?;
    if !selection.valid() || destination.file_name().is_none() {
        return Err(Error::Invalid);
    }
    check(&cancelled)?;
    match fs::symlink_metadata(destination) {
        Ok(_) => return Err(Error::Exists),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(_) => return Err(Error::Io),
    }
    let (staged, file) = Staged::create(destination)?;
    let mut output = Output {
        writer: BufWriter::new(file),
        format,
        bytes: 0,
        rows: 0,
    };
    let summary = Session {
        session_closed: session.summary.closed,
        session_dropped: session.summary.dropped,
        session_records: session.summary.records,
        last_loss: session.summary.last_loss,
    };
    match format {
        Format::Json => {
            output.write(format!(
                "{{\"schema_version\":{SCHEMA_VERSION},\"session\":"
            ))?;
            output.json(&summary)?;
            output.write(",\"rows\":[\n")?;
        }
        Format::Csv => {
            output.write(COLUMNS.join(","))?;
            output.write("\r\n")?;
            let mut metadata = serde_json::to_value(summary).map_err(|_| Error::Invalid)?;
            metadata["kind"] = "session".into();
            metadata["schema_version"] = SCHEMA_VERSION.into();
            output.csv(&metadata)?;
        }
    }
    stream(session, selection, &mut output, &cancelled)?;
    if format == Format::Json {
        output.write("\n]}\n")?;
    }
    check(&cancelled)?;
    output.writer.flush().map_err(|_| Error::Io)?;
    output.writer.get_ref().sync_all().map_err(|_| Error::Io)?;
    let count = output.rows;
    drop(output);
    check(&cancelled)?;
    fs::hard_link(&staged.0, destination).map_err(|e| {
        if e.kind() == io::ErrorKind::AlreadyExists {
            Error::Exists
        } else {
            Error::Io
        }
    })?;
    Ok(count)
}

fn stream(
    session: &Recovery,
    selection: &Selection,
    output: &mut Output,
    cancelled: &impl Fn() -> bool,
) -> Result<(), Error> {
    let (mut after, mut chart_revision) = (0, 0);
    let mut charts: Vec<Chart> = Vec::new();
    while after < session.summary.records {
        check(cancelled)?;
        let page = session.page(after).map_err(Error::Session)?;
        if page.is_empty() {
            return Err(Error::Invalid);
        }
        for entry in page {
            check(cancelled)?;
            after = entry.id;
            match entry.event.as_ref() {
                Event::Raw(raw) if selection.category == Category::Observations => {
                    let type_id = transport(&raw.metadata);
                    if selection.matches(raw.timestamp_ns, type_id) {
                        output.row(&Row {
                            kind: "observation",
                            record_id: entry.id,
                            timestamp_ns: raw.timestamp_ns,
                            source_timestamp_ns: raw.source_timestamp_ns,
                            source: Some(&raw.source),
                            type_id,
                            direction: Some(raw.direction),
                            metadata: Some(&raw.metadata),
                            original_length: raw.original,
                            bytes_hex: Some(hex(&raw.bytes)),
                            ..Row::default()
                        })?;
                    }
                }
                Event::Charts { definitions } if selection.category == Category::ChartSeries => {
                    charts = Vec::deserialize(definitions).map_err(|_| Error::Invalid)?;
                    chart_revision = entry.id;
                }
                Event::Message {
                    source,
                    timestamp_ns,
                    source_timestamp_ns,
                    revision,
                    protocol,
                    name,
                    samples,
                    ..
                } if selection.category != Category::Observations
                    && selection.matches(*timestamp_ns, protocol) =>
                {
                    for sample in samples
                        .iter()
                        .filter(|s| selection.field.is_empty() || s.field == selection.field)
                    {
                        check(cancelled)?;
                        let mut row = Row {
                            kind: "field",
                            record_id: entry.id,
                            timestamp_ns: *timestamp_ns,
                            source_timestamp_ns: *source_timestamp_ns,
                            source: Some(source),
                            type_id: protocol,
                            message_name: Some(name),
                            definition_revision: Some(*revision),
                            field: Some(&sample.field),
                            unit: Some(&sample.unit),
                            value: sample.value,
                            ..Row::default()
                        };
                        if selection.category == Category::Fields {
                            row.decoded = sample.decoded.as_ref();
                            output.row(&row)?;
                            continue;
                        }
                        row.kind = "chart_point";
                        row.chart_revision = Some(chart_revision);
                        for chart in &charts {
                            for (i, series) in chart.series.iter().enumerate() {
                                if series.protocol == *protocol && series.field == sample.field {
                                    check(cancelled)?;
                                    row.chart_id = Some(chart.id);
                                    row.chart_name = Some(&chart.name);
                                    row.series_index = Some(i);
                                    row.series_label = Some(&series.label);
                                    row.series_unit = Some(&series.unit);
                                    output.row(&row)?;
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
    Ok(())
}
