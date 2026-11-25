//! Versioned, bounded SQLite recordings. This module grants no capture or TX authority.
use exmon_domain::{
    Direction,
    config::ProjectConfig,
    fields::Literal,
    observation::{Metadata, Observation},
};
use rusqlite::{Connection, OpenFlags, limits::Limit, params};
use serde::{Deserialize, Serialize};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom},
    ops::Range,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
pub const MAX_RECORD: usize = 1024 * 1024;
pub const MAX_TRANSACTION: usize = 4 * 1024 * 1024;
pub const PAGE_ROWS: usize = 64;
pub const MAX_DISK: u64 = 256 * 1024 * 1024;
const APP_ID: i32 = 0x45584d31;
const CONTROL: &str = "CREATE TABLE control(id INTEGER PRIMARY KEY CHECK(id=1), version INTEGER NOT NULL, closed INTEGER NOT NULL, dropped INTEGER NOT NULL, records INTEGER NOT NULL, bytes INTEGER NOT NULL, budget INTEGER NOT NULL, max_records INTEGER NOT NULL, last_loss BLOB)";
const ENTRIES: &str = "CREATE TABLE entries(id INTEGER PRIMARY KEY, data BLOB NOT NULL)";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Path,
    Exists,
    Io,
    Invalid,
    Version,
    Limit,
    Busy,
    Database,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "session {:?}", self)
    }
}
impl std::error::Error for Error {}
fn db(e: rusqlite::Error) -> Error {
    match e.sqlite_error_code() {
        Some(rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked) => {
            Error::Busy
        }
        Some(rusqlite::ErrorCode::DiskFull) => Error::Limit,
        _ => Error::Database,
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub bytes: u64,
    pub records: u64,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            bytes: 64 * 1024 * 1024,
            records: 100_000,
        }
    }
}
impl Limits {
    pub fn validate(self) -> Result<Self, Error> {
        if !(1024 * 1024..=MAX_DISK).contains(&self.bytes)
            || !(1..=1_000_000).contains(&self.records)
        {
            return Err(Error::Limit);
        }
        Ok(self)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub generation: u64,
    pub interface: u64,
    pub epoch: u64,
    pub sequence: u64,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Raw {
    pub source: Source,
    pub timestamp_ns: u64,
    pub source_timestamp_ns: Option<u64>,
    pub direction: Direction,
    pub metadata: Metadata,
    pub original: Option<usize>,
    pub bytes: Vec<u8>,
}
impl Raw {
    pub fn new(source: Source, observation: &Observation) -> Self {
        Self {
            source,
            timestamp_ns: observation.timestamp_ns(),
            source_timestamp_ns: observation.source_timestamp_ns(),
            direction: observation.direction(),
            metadata: observation.metadata().clone(),
            original: observation.original_length(),
            bytes: observation.bytes().to_vec(),
        }
    }
    pub fn observation(&self) -> Result<Observation, Error> {
        let mut o = Observation::new(
            self.timestamp_ns,
            self.direction,
            self.metadata.clone(),
            self.original,
            &self.bytes,
        )
        .map_err(|_| Error::Invalid)?;
        if let Some(t) = self.source_timestamp_ns {
            o = o.with_source_timestamp(t);
        }
        Ok(o)
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sample {
    pub field: String,
    pub unit: String,
    pub value: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decoded: Option<Literal>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Span {
    pub observation: u64,
    pub offsets: Range<u32>,
    pub stream: Range<u64>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Event {
    Raw(Raw),
    Definitions {
        generation: u64,
        revision: u64,
        config: ProjectConfig,
    },
    Message {
        source: Source,
        timestamp_ns: u64,
        source_timestamp_ns: Option<u64>,
        revision: u64,
        protocol: String,
        name: String,
        group: String,
        bytes: Vec<u8>,
        samples: Vec<Sample>,
        spans: Vec<Span>,
    },
    Charts {
        definitions: serde_json::Value,
    },
    Annotation {
        source: Option<Source>,
        text: String,
    },
}
fn text(s: &str, max: usize) -> bool {
    s.len() <= max && !s.chars().any(|c| c.is_control() && c != '\n' && c != '\t')
}
impl Event {
    pub fn validate(&self) -> Result<(), Error> {
        let valid = match self {
            Self::Raw(r) => r.observation().is_ok(),
            Self::Definitions { config, .. } => config.validate().is_ok(),
            Self::Message {
                protocol,
                name,
                group,
                bytes,
                samples,
                spans,
                ..
            } => {
                text(protocol, 128)
                    && text(name, 128)
                    && text(group, 2048)
                    && bytes.len() <= 65_536
                    && samples.len() <= 256
                    && samples.iter().all(|s| {
                        text(&s.field, 128)
                            && text(&s.unit, 128)
                            && s.value.is_none_or(f64::is_finite)
                            && match &s.decoded {
                                Some(Literal::Bytes { value }) => value.len() <= 65_536,
                                Some(Literal::Text { value }) => value.len() <= 65_536,
                                _ => true,
                            }
                    })
                    && spans.len() <= 256
                    && spans.iter().all(|s| {
                        s.offsets.start <= s.offsets.end
                            && s.offsets.end <= 65_536
                            && s.stream.start <= s.stream.end
                    })
            }
            Self::Charts { definitions } => validate_charts(definitions),
            Self::Annotation { text: t, .. } => !t.is_empty() && text(t, 4096),
        };
        if valid { Ok(()) } else { Err(Error::Invalid) }
    }
    pub fn encode(&self) -> Result<Vec<u8>, Error> {
        self.validate()?;
        let bytes = serde_json::to_vec(self).map_err(|_| Error::Invalid)?;
        if bytes.len() > MAX_RECORD {
            return Err(Error::Limit);
        }
        Ok(bytes)
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() > MAX_RECORD {
            return Err(Error::Limit);
        }
        let event: Self = serde_json::from_slice(bytes).map_err(|_| Error::Invalid)?;
        event.validate()?;
        Ok(event)
    }
    pub fn label(&self) -> &'static str {
        match self {
            Self::Raw(_) => "Raw observation",
            Self::Definitions { .. } => "Definitions / names",
            Self::Message { .. } => "Message / chart samples",
            Self::Charts { .. } => "Chart configuration",
            Self::Annotation { .. } => "Annotation",
        }
    }
}
// Chart data is inert metadata, with a small closed structural contract.
fn validate_charts(v: &serde_json::Value) -> bool {
    let Some(charts) = v.as_array() else {
        return false;
    };
    charts.len() <= 4
        && charts.iter().all(|v| {
            let Some(o) = v.as_object() else {
                return false;
            };
            o.len() == 5
                && o.get("id").and_then(|v| v.as_u64()).is_some_and(|n| n > 0)
                && o.get("name")
                    .and_then(|v| v.as_str())
                    .is_some_and(|s| text(s, 64))
                && o.get("samples")
                    .and_then(|v| v.as_u64())
                    .is_some_and(|n| (1..=4096).contains(&n))
                && o.get("average")
                    .and_then(|v| v.as_u64())
                    .is_some_and(|n| n > 0 && Some(n) <= o.get("samples").and_then(|v| v.as_u64()))
                && o.get("series").and_then(|v| v.as_array()).is_some_and(|s| {
                    s.len() <= 8
                        && s.iter().all(|v| {
                            v.as_object().is_some_and(|o| {
                                o.len() == 4
                                    && ["protocol", "field", "label", "unit"].iter().all(|k| {
                                        o.get(*k)
                                            .and_then(|v| v.as_str())
                                            .is_some_and(|s| text(s, 128))
                                    })
                            })
                        })
                })
        })
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Summary {
    pub closed: bool,
    pub dropped: u64,
    pub last_loss: Option<Source>,
    pub records: u64,
    pub bytes: u64,
    pub limits: Limits,
}
#[derive(Clone)]
pub struct Entry {
    pub id: u64,
    pub event: Arc<Event>,
}
fn setup(connection: &Connection) -> Result<(), Error> {
    connection
        .busy_timeout(Duration::from_millis(100))
        .map_err(db)?;
    for (limit, n) in [
        (Limit::SQLITE_LIMIT_LENGTH, MAX_RECORD as i32 + 4096),
        (Limit::SQLITE_LIMIT_SQL_LENGTH, 16384),
        (Limit::SQLITE_LIMIT_COLUMN, 64),
        (Limit::SQLITE_LIMIT_SCHEMA, 8),
        (Limit::SQLITE_LIMIT_EXPR_DEPTH, 64),
        (Limit::SQLITE_LIMIT_COMPOUND_SELECT, 8),
        (Limit::SQLITE_LIMIT_ATTACHED, 0),
        (Limit::SQLITE_LIMIT_VARIABLE_NUMBER, 32),
        (Limit::SQLITE_LIMIT_TRIGGER_DEPTH, 0),
        (Limit::SQLITE_LIMIT_VDBE_OP, 100_000),
    ] {
        connection.set_limit(limit, n).map_err(db)?;
    }
    connection.execute_batch("PRAGMA trusted_schema=OFF; PRAGMA cache_size=-2048; PRAGMA mmap_size=0; PRAGMA temp_store=MEMORY").map_err(db)
}
pub struct Writer {
    connection: Connection,
    summary: Summary,
}
impl Writer {
    pub fn create(path: &Path, limits: Limits) -> Result<Self, Error> {
        let limits = limits.validate()?;
        crate::settings_files::local(path).map_err(|_| Error::Path)?;
        let file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(path)
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::AlreadyExists {
                    Error::Exists
                } else {
                    Error::Io
                }
            })?;
        drop(file);
        let connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(db)?;
        setup(&connection)?;
        connection
            .execute_batch(
                "PRAGMA page_size=4096; PRAGMA journal_mode=DELETE; PRAGMA synchronous=EXTRA; PRAGMA locking_mode=EXCLUSIVE",
            )
            .map_err(db)?;
        connection
            .pragma_update(None, "max_page_count", (limits.bytes / 4096) as i64)
            .map_err(db)?;
        let tx = connection.unchecked_transaction().map_err(db)?;
        tx.pragma_update(None, "application_id", APP_ID)
            .map_err(db)?;
        tx.pragma_update(None, "user_version", 1).map_err(db)?;
        tx.execute_batch(CONTROL).map_err(db)?;
        tx.execute_batch(ENTRIES).map_err(db)?;
        tx.execute(
            "INSERT INTO control VALUES(1,1,0,0,0,0,?1,?2,NULL)",
            params![limits.bytes as i64, limits.records as i64],
        )
        .map_err(db)?;
        tx.commit().map_err(db)?;
        Ok(Self {
            connection,
            summary: Summary {
                closed: false,
                dropped: 0,
                last_loss: None,
                records: 0,
                bytes: 0,
                limits,
            },
        })
    }
    pub fn summary(&self) -> Summary {
        self.summary.clone()
    }
    pub fn append(&mut self, events: &[Event]) -> Result<(), Error> {
        if self.summary.closed || events.len() > 96 {
            return Err(Error::Invalid);
        }
        let mut encoded = Vec::new();
        let mut bytes = 0usize;
        for event in events {
            let data = event.encode()?;
            bytes = bytes.checked_add(data.len()).ok_or(Error::Limit)?;
            if bytes > MAX_TRANSACTION {
                return Err(Error::Limit);
            }
            encoded.push(data);
        }
        let records = self
            .summary
            .records
            .checked_add(events.len() as u64)
            .ok_or(Error::Limit)?;
        let total = self
            .summary
            .bytes
            .checked_add(bytes as u64)
            .ok_or(Error::Limit)?;
        if records > self.summary.limits.records
            || total > self.summary.limits.bytes.saturating_sub(65536)
        {
            return Err(Error::Limit);
        }
        let tx = self.connection.transaction().map_err(db)?;
        for (i, data) in encoded.iter().enumerate() {
            tx.execute(
                "INSERT INTO entries(id,data) VALUES(?1,?2)",
                params![(self.summary.records + i as u64 + 1) as i64, data],
            )
            .map_err(db)?;
        }
        tx.execute(
            "UPDATE control SET records=?1, bytes=?2 WHERE id=1",
            params![records as i64, total as i64],
        )
        .map_err(db)?;
        tx.commit().map_err(db)?;
        self.summary.records = records;
        self.summary.bytes = total;
        Ok(())
    }
    pub fn finish(&mut self, dropped: u64) -> Result<(), Error> {
        self.finish_with_loss(dropped, None)
    }
    pub fn finish_with_loss(
        &mut self,
        dropped: u64,
        last_loss: Option<Source>,
    ) -> Result<(), Error> {
        let encoded = last_loss
            .map(|s| serde_json::to_vec(&s).map_err(|_| Error::Invalid))
            .transpose()?;
        self.connection
            .execute(
                "UPDATE control SET closed=1,dropped=?1,last_loss=?2 WHERE id=1",
                params![i64::try_from(dropped).map_err(|_| Error::Limit)?, encoded],
            )
            .map_err(db)?;
        self.summary.closed = true;
        self.summary.dropped = dropped;
        self.summary.last_loss = last_loss;
        Ok(())
    }
}
pub struct Recovery {
    connection: Connection,
    pub summary: Summary,
    work: Arc<AtomicU64>,
}
impl Recovery {
    pub fn open(path: &Path) -> Result<Self, Error> {
        crate::settings_files::local(path).map_err(|_| Error::Path)?;
        let mut file = File::open(path).map_err(|_| Error::Io)?;
        let metadata = file.metadata().map_err(|_| Error::Io)?;
        if !metadata.is_file() || metadata.len() > MAX_DISK {
            return Err(Error::Limit);
        }
        let mut header = [0; 100];
        file.read_exact(&mut header).map_err(|_| Error::Invalid)?;
        if header[16..20] != [0x10, 0, 1, 1]
            || &header[..16] != b"SQLite format 3\0"
            || i32::from_be_bytes(header[68..72].try_into().map_err(|_| Error::Invalid)?) != APP_ID
        {
            return Err(Error::Invalid);
        }
        if u32::from_be_bytes(header[60..64].try_into().map_err(|_| Error::Invalid)?) != 1 {
            return Err(Error::Version);
        }
        drop(file);
        let mut journal = path.as_os_str().to_os_string();
        journal.push("-journal");
        match File::open(std::path::PathBuf::from(journal)) {
            Ok(mut file) => {
                let meta = file.metadata().map_err(|_| Error::Io)?;
                if !meta.is_file() || meta.len() > 2 * MAX_DISK + MAX_RECORD as u64 {
                    return Err(Error::Limit);
                }
                if meta.len() >= 16 {
                    let mut tail = [0; 16];
                    file.seek(SeekFrom::End(-16)).map_err(|_| Error::Io)?;
                    file.read_exact(&mut tail).map_err(|_| Error::Io)?;
                    let name_bytes =
                        u32::from_be_bytes(tail[..4].try_into().map_err(|_| Error::Invalid)?)
                            as u64;
                    // Single-database recordings never create super-journal references.
                    // Reject the footer without reading or resolving its embedded filename.
                    if tail[8..] == [0xd9, 0xd5, 0x05, 0xf9, 0x20, 0xa1, 0x63, 0xd7]
                        && name_bytes > 0
                        && name_bytes <= meta.len() - 16
                    {
                        return Err(Error::Invalid);
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(Error::Io),
        }
        // READ_WRITE permits SQLite's atomic hot-journal recovery, never CREATE or application DDL.
        let mut connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(db)?;
        setup(&connection)?;
        let work = Arc::new(AtomicU64::new(0));
        let budget = work.clone();
        connection
            .progress_handler(
                1000,
                Some(move || budget.fetch_add(1000, Ordering::Relaxed) > 20_000_000),
            )
            .map_err(db)?;
        connection
            .execute_batch("PRAGMA query_only=ON; BEGIN")
            .map_err(db)?;
        let schema: Vec<(String, String)> = connection
            .prepare("SELECT name,sql FROM sqlite_schema ORDER BY name LIMIT 3")
            .map_err(db)?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(db)?
            .collect::<Result<_, _>>()
            .map_err(db)?;
        if schema
            != vec![
                ("control".into(), CONTROL.into()),
                ("entries".into(), ENTRIES.into()),
            ]
        {
            return Err(Error::Invalid);
        }
        let summary=connection.query_row("SELECT version,closed,dropped,records,bytes,budget,max_records FROM control WHERE id=1",[],|r|Ok((positive(r,0)?,positive(r,1)?,positive(r,2)?,positive(r,3)?,positive(r,4)?,positive(r,5)?,positive(r,6)?))).map_err(db)?;
        if summary.0 != 1 {
            return Err(Error::Version);
        }
        let limits = Limits {
            bytes: summary.5,
            records: summary.6,
        }
        .validate()?;
        if summary.1 > 1
            || summary.3 > limits.records
            || summary.4 > limits.bytes
            || metadata.len() > limits.bytes
        {
            return Err(Error::Invalid);
        }
        let loss: Option<Vec<u8>> = connection
            .query_row("SELECT last_loss FROM control WHERE id=1", [], |r| r.get(0))
            .map_err(db)?;
        let last_loss = loss
            .map(|data| {
                if data.len() > 512 {
                    return Err(Error::Invalid);
                }
                serde_json::from_slice::<Source>(&data).map_err(|_| Error::Invalid)
            })
            .transpose()?;
        let actual: (u64, u64, u64) = connection
            .query_row(
                "SELECT count(*),coalesce(max(id),0),coalesce(sum(length(data)),0) FROM entries",
                [],
                |r| Ok((positive(r, 0)?, positive(r, 1)?, positive(r, 2)?)),
            )
            .map_err(db)?;
        let min_id: u64 = connection
            .query_row("SELECT coalesce(min(id),1) FROM entries", [], |r| {
                positive(r, 0)
            })
            .map_err(db)?;
        if min_id != 1 || actual != (summary.3, summary.3, summary.4) {
            return Err(Error::Invalid);
        }
        Ok(Self {
            connection,
            summary: Summary {
                closed: summary.1 == 1,
                dropped: summary.2,
                last_loss,
                records: summary.3,
                bytes: summary.4,
                limits,
            },
            work,
        })
    }
    pub fn page(&self, after: u64) -> Result<Vec<Entry>, Error> {
        if after > i64::MAX as u64 {
            return Err(Error::Invalid);
        }
        self.work.store(0, Ordering::Relaxed);
        let mut stmt = self
            .connection
            .prepare("SELECT id,data FROM entries WHERE id>?1 ORDER BY id LIMIT 64")
            .map_err(db)?;
        let mut query = stmt.query([after as i64]).map_err(db)?;
        let mut result = Vec::new();
        let mut bytes = 0;
        while let Some(row) = query.next().map_err(db)? {
            let data: Vec<u8> = row.get(1).map_err(db)?;
            if bytes + data.len() > MAX_RECORD && !result.is_empty() {
                break;
            }
            if data.len() > MAX_RECORD {
                return Err(Error::Limit);
            }
            bytes += data.len();
            let event = Event::decode(&data)?;
            result.push(Entry {
                id: positive(row, 0).map_err(db)?,
                event: Arc::new(event),
            });
        }
        Ok(result)
    }
}

fn positive(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<u64> {
    let value: i64 = row.get(index)?;
    u64::try_from(value).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(index, value))
}
