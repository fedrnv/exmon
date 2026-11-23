//! Transactional settings. Drafts, files and previews never authorize physical I/O.
use crate::{
    incoming::{self, Processor, Row, Status},
    shell::Captured,
};
use exmon_domain::config::{MachineBindings, ProjectConfig};
use exmon_storage::{config, settings_files};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, SyncSender},
    },
    thread::{self, JoinHandle},
    time::Duration,
};
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Safety {
    pub queue_items: usize,
    pub queue_bytes: usize,
    pub batch_items: usize,
    pub batch_bytes: usize,
    pub snapshot_bytes: usize,
}
impl Default for Safety {
    fn default() -> Self {
        Self {
            queue_items: 64,
            queue_bytes: 4096,
            batch_items: 32,
            batch_bytes: 2048,
            snapshot_bytes: 64,
        }
    }
}
impl Safety {
    pub fn limits(self) -> exmon_capture::adapter::Limits {
        exmon_capture::adapter::Limits {
            queue_items: self.queue_items,
            queue_bytes: self.queue_bytes,
            batch_items: self.batch_items,
            batch_bytes: self.batch_bytes,
        }
    }
    pub fn validate(self) -> Result<(), Issue> {
        for (name, n, max) in [
            ("queue_items", self.queue_items, 4096),
            ("queue_bytes", self.queue_bytes, 16 * 1024 * 1024),
            ("batch_items", self.batch_items, 32),
            ("batch_bytes", self.batch_bytes, 65536),
            ("snapshot_bytes", self.snapshot_bytes, 64),
        ] {
            if n == 0 || n > max {
                return Err(Issue::new(
                    format!("/safety/{name}"),
                    format!("Use 1..{max}"),
                ));
            }
        }
        if self.batch_bytes < self.snapshot_bytes {
            return Err(Issue::new(
                "/safety/batch_bytes",
                "Batch byte budget must cover the selected synthetic snapshot",
            ));
        }
        Ok(())
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SafetyFile {
    schema_version: u32,
    limits: Safety,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Issue {
    pub path: String,
    pub message: String,
}
impl Issue {
    fn new(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
        }
    }
}
#[derive(Clone)]
pub struct Config {
    pub project: Arc<ProjectConfig>,
    pub bindings: Arc<MachineBindings>,
    pub safety: Safety,
    pub selected: Option<String>,
    pub revision: u64,
}
impl Config {
    pub fn interface(&self) -> Option<&exmon_domain::config::Interface> {
        let id = self.selected.as_ref()?;
        self.project.interfaces.iter().find(|i| &i.id == id)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Document {
    #[default]
    Project,
    Bindings,
    Safety,
}
impl Document {
    pub fn label(self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::Bindings => "bindings",
            Self::Safety => "safety",
        }
    }
}
#[derive(Clone)]
pub struct Leaf {
    pub document: Document,
    pub pointer: String,
    pub value: String,
    pub string: bool,
    pub options: Vec<&'static str>,
    pub truncated: bool,
}
#[derive(Clone, Default)]
pub struct Snapshot {
    pub revision: u64,
    pub active_revision: u64,
    pub project: Arc<str>,
    pub bindings: Arc<str>,
    pub selected: Option<String>,
    pub interfaces: Vec<String>,
    pub document: Document,
    pub page: usize,
    pub total: usize,
    pub leaves: Vec<Leaf>,
    pub issue: Option<Issue>,
    pub invalid_fields: Vec<String>,
    pub preview: Arc<[Arc<Row>]>,
    pub preview_skipped: u64,
    pub preview_revision: Option<u64>,
    pub notice: Option<&'static str>,
    pub failed: bool,
}
#[derive(Clone)]
pub struct Model {
    project: String,
    bindings: String,
    safety: Safety,
    selected: Option<String>,
    page: usize,
    document: Document,
    patch_errors: BTreeMap<String, Issue>,
    error_limit: bool,
    issue: Option<Issue>,
    preview: Arc<[Arc<Row>]>,
    preview_skipped: u64,
    preview_revision: Option<u64>,
    notice: Option<&'static str>,
    pending_revision: Option<u64>,
}
impl Model {
    pub fn from_config(active: &Config) -> Result<Self, Issue> {
        active.safety.validate()?;
        if active
            .selected
            .as_ref()
            .is_some_and(|id| !active.project.interfaces.iter().any(|i| &i.id == id))
        {
            return Err(Issue::new(
                "/selected_interface",
                "Select an existing portable interface",
            ));
        }
        Ok(Self {
            project: String::from_utf8(
                config::save_project(&active.project)
                    .map_err(|_| Issue::new("/project", "Invalid or oversized active profile"))?,
            )
            .expect("JSON UTF8"),
            bindings: String::from_utf8(
                config::save_bindings(&active.bindings, &active.project)
                    .map_err(|_| Issue::new("/bindings", "Invalid or oversized active bindings"))?,
            )
            .expect("JSON UTF8"),
            safety: active.safety,
            selected: active.selected.clone(),
            page: 0,
            document: Document::Project,
            patch_errors: BTreeMap::new(),
            error_limit: false,
            issue: None,
            preview: Arc::from([]),
            preview_skipped: 0,
            preview_revision: None,
            notice: None,
            pending_revision: None,
        })
    }
    fn document(&self, d: Document) -> Result<Value, Issue> {
        let t = match d {
            Document::Project => &self.project,
            Document::Bindings => &self.bindings,
            Document::Safety => {
                return Ok(serde_json::to_value(self.safety).expect("finite safety"));
            }
        };
        serde_json::from_str(t).map_err(|e| {
            Issue::new(
                format!("/{}:{}:{}", d.label(), e.line(), e.column()),
                "Invalid JSON; check this line/column and field type",
            )
        })
    }
    pub fn replace(&mut self, d: Document, text: String) -> Result<(), Issue> {
        if text.len() > config::MAX_DOCUMENT_BYTES {
            return Err(Issue::new(d.label(), "Document exceeds 1MiB"));
        }
        match d {
            Document::Project => self.project = text,
            Document::Bindings => self.bindings = text,
            Document::Safety => return Err(Issue::new("/safety", "Use individual safety fields")),
        }
        self.patch_errors
            .retain(|p, _| !p.starts_with(&format!("/{}/", d.label())));
        self.clear_preview();
        Ok(())
    }
    fn clear_preview(&mut self) {
        self.preview = Arc::from([]);
        self.preview_revision = None;
        self.preview_skipped = 0;
        self.notice = None;
    }
    pub fn patch(&mut self, d: Document, pointer: &str, text: &str) -> Result<(), Issue> {
        if pointer.len() > 512 || text.len() > 4096 {
            return Err(Issue::new(
                "/editor",
                "Field edit too large; use advanced document editor",
            ));
        }
        let path = format!("/{}{pointer}", d.label());
        let result = (|| {
            let mut value = self.document(d)?;
            let target = value
                .pointer_mut(pointer)
                .ok_or_else(|| Issue::new(&path, "Unknown field"))?;
            let replacement = if target.is_string() {
                Value::String(text.into())
            } else {
                serde_json::from_str(text).map_err(|_| {
                    Issue::new(&path, "Enter a JSON number, boolean, null, array or object")
                })?
            };
            *target = replacement;
            // Variant changes replace the complete tagged object, preserving valid schema shape.
            if pointer.ends_with("/framing/kind") {
                let parent = pointer.strip_suffix("/kind").unwrap_or(pointer);
                let kind = value.pointer(pointer).and_then(Value::as_str).unwrap_or("");
                let v = match kind {
                    "datagram" => json!({"kind":"datagram"}),
                    "fixed" => json!({"kind":"fixed","length":32}),
                    "delimiter" => json!({"kind":"delimiter","bytes":[10],"max_length":65536}),
                    "length_prefix" => {
                        json!({"kind":"length_prefix","offset":0,"width":1,"endian":"little","max_length":256})
                    }
                    "markers" => json!({"kind":"markers","start":[2],"end":[3],"max_length":65536}),
                    "idle_gap" => json!({"kind":"idle_gap","gap_ns":1000000,"max_length":65536}),
                    _ => return Err(Issue::new(&path, "Unknown framing mode")),
                };
                *value
                    .pointer_mut(parent)
                    .ok_or_else(|| Issue::new(&path, "Unknown framing"))? = v;
            }
            if pointer.ends_with("/transport/kind") {
                let parent = pointer.strip_suffix("/kind").unwrap_or(pointer);
                let kind = value.pointer(pointer).and_then(Value::as_str).unwrap_or("");
                let v = match kind {
                    "ethernet" | "tcp" | "udp" => json!({"kind":kind}),
                    "serial" => {
                        json!({"kind":"serial","baud":9600,"data_bits":8,"stop_bits":1,"parity":"none"})
                    }
                    "usb" => json!({"kind":"usb","endpoint":129,"transfer":"hid"}),
                    "can" => json!({"kind":"can","bitrate":500000,"fd":false}),
                    _ => return Err(Issue::new(&path, "Unknown transport")),
                };
                *value
                    .pointer_mut(parent)
                    .ok_or_else(|| Issue::new(&path, "Unknown transport"))? = v;
            }
            let bytes = serde_json::to_vec_pretty(&value)
                .map_err(|_| Issue::new(&path, "Cannot serialize draft"))?;
            if bytes.len() > config::MAX_DOCUMENT_BYTES {
                return Err(Issue::new(&path, "Document exceeds1MiB"));
            }
            match d {
                Document::Project => self.project = String::from_utf8(bytes).expect("JSON"),
                Document::Bindings => self.bindings = String::from_utf8(bytes).expect("JSON"),
                Document::Safety => {
                    let safety: Safety = serde_json::from_value(value)
                        .map_err(|_| Issue::new(&path, "Enter a nonnegative whole number"))?;
                    safety.validate()?;
                    self.safety = safety;
                }
            }
            Ok(())
        })();
        self.clear_preview();
        match &result {
            Ok(()) => {
                self.patch_errors.remove(&path);
            }
            Err(e) => {
                if self.patch_errors.len() < 64 || self.patch_errors.contains_key(&path) {
                    self.patch_errors.insert(path, e.clone());
                } else {
                    self.error_limit = true;
                }
            }
        }
        result
    }
    pub fn validate(&self, revision: u64) -> Result<Config, Issue> {
        if self.error_limit {
            return Err(Issue::new(
                "/editor",
                "Too many rejected field edits; Cancel or restore defaults",
            ));
        }
        if let Some(e) = self.patch_errors.values().next() {
            return Err(e.clone());
        }
        let project: ProjectConfig = serde_json::from_str(&self.project).map_err(|e| {
            Issue::new(
                format!("/project:{}:{}", e.line(), e.column()),
                "Invalid JSON field/type, unknown or duplicate key",
            )
        })?;
        if self.project.len() > config::MAX_DOCUMENT_BYTES {
            return Err(Issue::new("/project", "Document exceeds1MiB"));
        }
        if let Err(e) = project.validate() {
            return Err(locate(&project, e));
        }
        config::save_project(&project)
            .map_err(|_| Issue::new("/project", "Serialized profile exceeds1MiB"))?;
        let bindings =
            config::load_bindings(self.bindings.as_bytes(), &project).map_err(|e| match e {
                config::LoadError::Validation(e) => binding_issue(&self.bindings, &project, e),
                config::LoadError::Json { line, column } => Issue::new(
                    format!("/bindings:{line}:{column}"),
                    "Invalid binding JSON field/type",
                ),
                _ => Issue::new("/bindings", "Invalid or oversized bindings/schema"),
            })?;
        self.safety.validate()?;
        if self
            .selected
            .as_ref()
            .is_some_and(|id| !project.interfaces.iter().any(|i| &i.id == id))
        {
            return Err(Issue::new(
                "/selected_interface",
                "Select an existing portable interface",
            ));
        }
        Ok(Config {
            project: Arc::new(project),
            bindings: Arc::new(bindings),
            safety: self.safety,
            selected: self.selected.clone(),
            revision,
        })
    }
    pub fn preview(&mut self, revision: u64, records: Vec<Captured>) -> Result<(), Issue> {
        if records.is_empty()
            || records.len() > 32
            || records
                .iter()
                .map(|r| r.observation.bytes().len())
                .sum::<usize>()
                > 65536
        {
            return Err(Issue::new(
                "/preview",
                "Select1..32 raw captures with at most65536 payload bytes",
            ));
        }
        let cfg = self.validate(revision)?;
        let mut processor = Processor::selected((*cfg.project).clone(), cfg.selected.as_deref())
            .map_err(|_| Issue::new("/project", "Protocol compilation failed"))?;
        let mut rows = Vec::new();
        let mut cost = 0usize;
        let mut skipped = 0;
        let mut status = Status::default();
        for record in records {
            for row in processor
                .process(1, record, &mut status)
                .map_err(|_| Issue::new("/preview", "Pipeline rejected source context"))?
            {
                let n = row.cost();
                if rows.len() < 96 && n <= 1_048_576 && cost + n <= 1_048_576 {
                    cost += n;
                    rows.push(row);
                } else {
                    skipped += 1;
                }
            }
        }
        self.preview = rows.into();
        self.preview_skipped = skipped;
        self.preview_revision = Some(revision);
        Ok(())
    }
    pub fn add(&mut self, kind: &str, index: usize) -> Result<(), Issue> {
        let mut v = self.document(Document::Project)?;
        if !v.is_object() {
            return Err(Issue::new(
                "/project",
                "Expected a project object before adding rows",
            ));
        }
        let next = match kind {
            "protocol" => {
                let a = v
                    .pointer_mut("/protocols")
                    .and_then(Value::as_array_mut)
                    .ok_or_else(|| Issue::new("/project/protocols", "Use an array"))?;
                let n = a.len() + 1;
                a.push(json!({"id":format!("protocol{n}"),"name":format!("Protocol {n}"),"framing":{"kind":"fixed","length":32},"classification":{"length":32,"prefix":[],"priority":0,"offset":0,"mask":[],"transport":null,"direction":null},"fields":[]}));
                None
            }
            "interface" => {
                let protocol = v["protocols"][index]["id"]
                    .as_str()
                    .ok_or_else(|| Issue::new("/project/protocols", "Add/select a protocol first"))?
                    .to_owned();
                let a = v
                    .pointer_mut("/interfaces")
                    .and_then(Value::as_array_mut)
                    .ok_or_else(|| Issue::new("/project/interfaces", "Use an array"))?;
                let n = a.len() + 1;
                a.push(json!({"id":format!("interface{n}"),"protocol_id":protocol,"transport":{"kind":"ethernet"},"filter":{"ports":[],"can_ids":[],"direction":null},"tx_allowed":false}));
                None
            }
            "field" => {
                let a = v
                    .pointer_mut(&format!("/protocols/{index}/fields"))
                    .and_then(Value::as_array_mut)
                    .ok_or_else(|| Issue::new("/project/protocols/fields", "Select a protocol"))?;
                let n = a.len() + 1;
                a.push(json!({"id":format!("field{n}"),"offset":0,"length":1,"representation":"unsigned","endian":"little","scale":1.0,"bias":0.0,"unit":"","bits":null,"constant":null,"enumeration":[],"constraints":{"min":null,"max":null,"finite":false,"enum_closed":false},"derived":null}));
                None
            }
            "binding" => {
                let id = v["interfaces"][index]["id"]
                    .as_str()
                    .ok_or_else(|| {
                        Issue::new("/project/interfaces", "Add/select an interface first")
                    })?
                    .to_owned();
                let mut b = self.document(Document::Bindings)?;
                b.pointer_mut("/bindings")
                    .and_then(Value::as_array_mut)
                    .ok_or_else(|| Issue::new("/bindings", "Use an array"))?
                    .push(json!({"interface_id":id,"selector":"EDIT_LOCAL_SELECTOR"}));
                Some(b)
            }
            _ => return Err(Issue::new("/editor", "Unknown add action")),
        };
        if let Some(b) = next {
            self.replace(
                Document::Bindings,
                serde_json::to_string_pretty(&b).expect("JSON"),
            )
        } else {
            self.replace(
                Document::Project,
                serde_json::to_string_pretty(&v).expect("JSON"),
            )
        }
    }
    fn snapshot(&self, revision: u64, active: u64) -> Snapshot {
        let mut leaves = Vec::new();
        let mut total = 0;
        let value = self.document(self.document).ok();
        if let Some(value) = value {
            flatten(
                self.document,
                &value,
                "",
                self.page,
                &mut total,
                &mut leaves,
            );
        }
        let interfaces = serde_json::from_str::<Value>(&self.project)
            .ok()
            .and_then(|v| {
                v["interfaces"].as_array().map(|a| {
                    a.iter()
                        .filter_map(|i| i["id"].as_str().map(str::to_owned))
                        .take(256)
                        .collect()
                })
            })
            .unwrap_or_default();
        Snapshot {
            revision,
            active_revision: active,
            project: Arc::from(self.project.as_str()),
            bindings: Arc::from(self.bindings.as_str()),
            selected: self.selected.clone(),
            interfaces,
            document: self.document,
            page: self.page,
            total,
            leaves,
            invalid_fields: self.patch_errors.keys().cloned().collect(),
            issue: if self.error_limit {
                Some(Issue::new(
                    "/editor",
                    "Too many rejected field edits; Cancel or restore defaults",
                ))
            } else {
                self.patch_errors
                    .values()
                    .next()
                    .cloned()
                    .or_else(|| self.issue.clone())
            },
            preview: self.preview.clone(),
            preview_skipped: self.preview_skipped,
            preview_revision: self.preview_revision,
            notice: self.notice,
            failed: false,
        }
    }
}
fn flatten(
    document: Document,
    v: &Value,
    path: &str,
    page: usize,
    total: &mut usize,
    out: &mut Vec<Leaf>,
) {
    if path.len() > 512 {
        return;
    }
    match v {
        Value::Object(m) if !m.is_empty() => {
            for (k, v) in m {
                if k.len() <= 128 {
                    flatten(
                        document,
                        v,
                        &format!("{path}/{}", k.replace('~', "~0").replace('/', "~1")),
                        page,
                        total,
                        out,
                    );
                }
            }
        }
        Value::Array(a) if !a.is_empty() => {
            for (i, v) in a.iter().enumerate() {
                flatten(document, v, &format!("{path}/{i}"), page, total, out);
            }
        }
        _ => {
            let i = *total;
            *total = total.saturating_add(1);
            if i / 64 == page {
                let value = if let Some(s) = v.as_str() {
                    s.to_owned()
                } else {
                    v.to_string()
                };
                let options = if path.ends_with("/framing/kind") {
                    vec![
                        "datagram",
                        "fixed",
                        "delimiter",
                        "length_prefix",
                        "markers",
                        "idle_gap",
                    ]
                } else if path.ends_with("/transport/kind") {
                    vec!["ethernet", "tcp", "udp", "serial", "usb", "can"]
                } else if path.ends_with("/representation") {
                    vec![
                        "unsigned", "signed", "float", "bytes", "ascii", "utf8", "derived",
                    ]
                } else if path.ends_with("/endian") {
                    vec!["little", "big"]
                } else if path.ends_with("/direction") {
                    vec!["null", "\"Rx\"", "\"Tx\"", "\"Unknown\""]
                } else if path.ends_with("/parity") {
                    vec!["none", "odd", "even"]
                } else {
                    vec![]
                };
                out.push(Leaf {
                    document,
                    pointer: path.into(),
                    truncated: value.len() > 4096,
                    value: value.chars().take(4096).collect(),
                    string: v.is_string(),
                    options,
                });
            }
        }
    }
}
fn locate(project: &ProjectConfig, e: exmon_domain::config::ConfigError) -> Issue {
    let value = serde_json::to_value(project).expect("finite validated field types");
    for key in [
        "protocols",
        "interfaces",
        "charts",
        "templates",
        "schedules",
    ] {
        if let Some(a) = value[key].as_array() {
            let mut seen = std::collections::BTreeSet::new();
            for (i, v) in a.iter().enumerate() {
                if let Some(id) = v["id"].as_str()
                    && (!seen.insert(id)
                        || id.is_empty()
                        || id.len() > 128
                        || !id
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b)))
                {
                    return Issue::new(
                        format!("/project/{key}/{i}/id"),
                        "Use a unique ASCII identifier,1..128 bytes",
                    );
                }
            }
        }
    }

    for (i, p) in project.protocols.iter().enumerate() {
        let prefix = format!("/project/protocols/{i}");
        let limit = match p.framing.limit(p.escape) {
            Ok(l) => l,
            Err(err) => {
                return Issue::new(
                    format!("{prefix}/{}", framing_path(&p.framing, p.escape)),
                    err.reason,
                );
            }
        };
        if let Err(err) = exmon_domain::fields::validate(&p.fields, limit) {
            let field = err.field.map(|n| format!("/{n}")).unwrap_or_default();
            return Issue::new(
                format!("{prefix}/fields{field}"),
                format!(
                    "Field definition {:?}; check geometry, type, transform and references",
                    err.code
                ),
            );
        }
        if let Err(err) = exmon_domain::checksum::validate(&p.checksums, &p.fields, limit) {
            return Issue::new(
                format!("{prefix}/checksums/{}", err.checksum.unwrap_or(0)),
                format!("Checksum definition {:?}", err.code),
            );
        }
        let isolated = ProjectConfig {
            protocols: vec![p.clone()],
            ..Default::default()
        };
        if let Err(err) = isolated.validate() {
            return Issue::new(
                format!(
                    "{prefix}/{}",
                    err.section.replace("protocol.", "").replace('.', "/")
                ),
                err.reason,
            );
        }
    }
    for (i, interface) in project.interfaces.iter().enumerate() {
        let mut isolated = project.clone();
        isolated.interfaces = vec![interface.clone()];
        isolated.templates.clear();
        isolated.schedules.clear();
        isolated.charts.clear();
        if let Err(err) = isolated.validate() {
            if let Some(path) = transport_path(&interface.transport) {
                return Issue::new(
                    format!("/project/interfaces/{i}/transport/{path}"),
                    err.reason,
                );
            }
            return Issue::new(
                format!(
                    "/project/interfaces/{i}/{}",
                    err.section
                        .trim_start_matches("interface.")
                        .replace('.', "/")
                ),
                err.reason,
            );
        }
    }

    for (i, item) in project.charts.iter().enumerate() {
        let mut p = project.clone();
        p.charts = vec![item.clone()];
        p.templates.clear();
        p.schedules.clear();
        if let Err(e) = p.validate() {
            return Issue::new(
                format!(
                    "/project/charts/{i}/{}",
                    e.section.trim_start_matches("chart.")
                ),
                e.reason,
            );
        }
    }
    for (i, item) in project.templates.iter().enumerate() {
        let mut p = project.clone();
        p.templates = vec![item.clone()];
        p.schedules.clear();
        if let Err(e) = p.validate() {
            return Issue::new(
                format!(
                    "/project/templates/{i}/{}",
                    e.section.trim_start_matches("template.")
                ),
                e.reason,
            );
        }
    }
    for (i, item) in project.schedules.iter().enumerate() {
        let mut p = project.clone();
        p.schedules = vec![item.clone()];
        if let Err(e) = p.validate() {
            return Issue::new(
                format!(
                    "/project/schedules/{i}/{}",
                    e.section.trim_start_matches("schedule.")
                ),
                e.reason,
            );
        }
    }
    Issue::new(
        format!("/project/{}", e.section.replace('.', "/")),
        e.reason,
    )
}

fn binding_issue(
    text: &str,
    project: &ProjectConfig,
    e: exmon_domain::config::ConfigError,
) -> Issue {
    if let Ok(bindings) = serde_json::from_str::<MachineBindings>(text) {
        let mut seen = std::collections::BTreeSet::new();
        for (i, b) in bindings.bindings.iter().enumerate() {
            if !project.interfaces.iter().any(|p| p.id == b.interface_id)
                || !seen.insert(&b.interface_id)
            {
                return Issue::new(format!("/bindings/bindings/{i}/interface_id"), e.reason);
            }
            if b.selector.is_empty()
                || b.selector.len() > 128
                || b.selector.chars().any(char::is_control)
            {
                return Issue::new(format!("/bindings/bindings/{i}/selector"), e.reason);
            }
        }
    }
    Issue::new(
        format!("/bindings/{}", e.section.replace('.', "/")),
        e.reason,
    )
}
fn transport_path(t: &exmon_domain::config::Transport) -> Option<&'static str> {
    use exmon_domain::config::Transport;
    match t {
        Transport::Serial {
            baud,
            data_bits,
            stop_bits,
            ..
        } => {
            if *baud == 0 || *baud > 4_000_000 {
                Some("baud")
            } else if !(5..=8).contains(data_bits) {
                Some("data_bits")
            } else if !(1..=2).contains(stop_bits) {
                Some("stop_bits")
            } else {
                None
            }
        }
        Transport::Usb { endpoint, .. } if endpoint & 0x70 != 0 || endpoint & 0x0f == 0 => {
            Some("endpoint")
        }
        Transport::Can { bitrate, .. } if !(1..=8_000_000).contains(bitrate) => Some("bitrate"),
        _ => None,
    }
}
fn framing_path(
    f: &exmon_domain::config::Framing,
    escape: Option<exmon_domain::config::Escape>,
) -> &'static str {
    use exmon_domain::config::Framing;
    match f {
        Framing::Fixed { .. } => "framing/length",
        Framing::LengthPrefix {
            width,
            offset,
            max_length,
            ..
        } => {
            if ![1, 2, 4].contains(width) {
                "framing/width"
            } else if offset
                .checked_add(u32::from(*width))
                .is_none_or(|end| end > *max_length)
            {
                "framing/offset"
            } else {
                "framing/max_length"
            }
        }
        Framing::Delimiter { bytes, .. }
            if bytes.is_empty()
                || bytes.len() > 16
                || escape.is_some_and(|e| bytes.contains(&e.byte)) =>
        {
            "framing/bytes"
        }
        Framing::Markers { start, .. }
            if start.is_empty()
                || start.len() > 16
                || escape.is_some_and(|e| start.contains(&e.byte)) =>
        {
            "framing/start"
        }
        Framing::Markers { end, .. }
            if end.is_empty()
                || end.len() > 16
                || escape.is_some_and(|e| end.contains(&e.byte)) =>
        {
            "framing/end"
        }
        Framing::IdleGap { gap_ns: 0, .. } => "framing/gap_ns",
        _ => "framing/max_length",
    }
}
pub enum Action {
    /// Synchronize names already committed by the running incoming worker; structural edits are rejected.
    RefreshNames(Arc<ProjectConfig>),
    Text(Document, String),
    Patch(Document, String, String),
    Select(Option<String>),
    Page(Document, usize),
    Add(&'static str, usize),
    Validate,
    Preview(Vec<Captured>),
    Apply,
    Cancel,
    Defaults,
    Import(Document, String),
    Export(Document, String),
}
struct Prepared {
    revision: u64,
    config: Arc<Config>,
    processor: Processor,
}
struct Shared {
    quit: AtomicBool,
    revision: AtomicU64,
    blocked: AtomicBool,
    gate: Mutex<()>,
    snapshot: Mutex<Arc<Snapshot>>,
    active: Mutex<Option<Arc<Config>>>,
    ready: Mutex<Option<Prepared>>,
    retired: Mutex<Option<incoming::Worker>>,
}
struct Command {
    revision: u64,
    action: Action,
}
pub struct Controller {
    shared: Arc<Shared>,
    commands: SyncSender<Command>,
    thread: Option<JoinHandle<()>>,
}
pub struct Activation {
    pub config: Arc<Config>,
    pub worker: incoming::Worker,
}
impl Controller {
    pub fn new() -> Result<Self, std::io::Error> {
        let shared = Arc::new(Shared {
            quit: AtomicBool::new(false),
            revision: AtomicU64::new(0),
            blocked: AtomicBool::new(false),
            gate: Mutex::new(()),
            snapshot: Mutex::new(Arc::new(Snapshot::default())),
            active: Mutex::new(None),
            ready: Mutex::new(None),
            retired: Mutex::new(None),
        });
        let (commands, rx) = mpsc::sync_channel::<Command>(8);
        let state = shared.clone();
        let thread = thread::Builder::new()
            .name("exmon-settings".into())
            .spawn(move || run(state, rx))?;
        Ok(Self {
            shared,
            commands,
            thread: Some(thread),
        })
    }
    pub fn request(&self, action: Action) -> Result<u64, Issue> {
        let _gate = self
            .shared
            .gate
            .lock()
            .map_err(|_| Issue::new("/worker", "Settings unavailable"))?;
        let revision = self
            .shared
            .revision
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| n.checked_add(1))
            .map_err(|_| {
                self.shared.blocked.store(true, Ordering::Release);
                Issue::new("/worker", "Revision exhausted; restart required")
            })?
            + 1;
        let admitted = match &action {
            Action::Text(_, s) => s.len() <= 1_048_576,
            Action::Patch(_, p, s) => p.len() <= 512 && s.len() <= 4096,
            Action::Import(_, p) | Action::Export(_, p) => {
                !p.is_empty() && p.len() <= 4096 && !p.chars().any(char::is_control)
            }
            Action::Preview(r) => {
                r.len() <= 32
                    && r.iter().map(|r| r.observation.bytes().len()).sum::<usize>() <= 65536
            }
            Action::Select(s) => s.as_ref().is_none_or(|s| s.len() <= 128),
            _ => true,
        };
        if !admitted {
            self.shared.blocked.store(true, Ordering::Release);
            return Err(Issue::new(
                "/editor",
                "Input limit exceeded; Cancel or restore defaults before Apply",
            ));
        }
        if self
            .commands
            .try_send(Command { revision, action })
            .is_err()
        {
            self.shared.blocked.store(true, Ordering::Release);
            return Err(Issue::new(
                "/worker",
                "Settings queue full/unavailable; Cancel or restore defaults before Apply",
            ));
        }
        Ok(revision)
    }
    pub fn invalidate(&self) -> Result<(), Issue> {
        let _gate = self
            .shared
            .gate
            .lock()
            .map_err(|_| Issue::new("/worker", "Settings unavailable"))?;
        self.shared
            .revision
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| n.checked_add(1))
            .map_err(|_| {
                self.shared.blocked.store(true, Ordering::Release);
                Issue::new("/worker", "Revision exhausted; restart required")
            })?;
        Ok(())
    }
    pub fn snapshot(&self) -> Option<Arc<Snapshot>> {
        self.shared.snapshot.try_lock().ok().map(|s| s.clone())
    }
    pub fn active(&self) -> Option<Arc<Config>> {
        self.shared.active.try_lock().ok()?.clone()
    }
    pub fn activate(&self) -> Result<Option<Activation>, Issue> {
        let _gate = self
            .shared
            .gate
            .lock()
            .map_err(|_| Issue::new("/worker", "Settings unavailable"))?;
        if self.shared.blocked.load(Ordering::Acquire) {
            return Ok(None);
        }
        if self
            .shared
            .retired
            .lock()
            .map_err(|_| Issue::new("/worker", "Retirement unavailable"))?
            .is_some()
        {
            return Ok(None);
        }
        let Some(ready) = self
            .shared
            .ready
            .lock()
            .map_err(|_| Issue::new("/worker", "Prepared settings unavailable"))?
            .take()
        else {
            return Ok(None);
        };
        if ready.revision != self.shared.revision.load(Ordering::Acquire) {
            return Ok(None);
        }
        let worker = incoming::Worker::prepared(ready.processor).map_err(|_| {
            Issue::new(
                "/worker",
                "Cannot create incoming worker; active configuration retained",
            )
        })?;
        *self
            .shared
            .active
            .lock()
            .map_err(|_| Issue::new("/worker", "Active state unavailable"))? =
            Some(ready.config.clone());
        if let Ok(mut view) = self.shared.snapshot.lock() {
            let mut s = (**view).clone();
            s.active_revision = ready.config.revision;
            s.notice = Some("Applied; capture and outgoing connection are stopped");
            *view = Arc::new(s);
        }
        Ok(Some(Activation {
            config: ready.config,
            worker,
        }))
    }
    pub fn retire(&self, worker: incoming::Worker) {
        if let Ok(mut retired) = self.shared.retired.lock() {
            debug_assert!(retired.is_none());
            *retired = Some(worker);
        }
    }
}
impl Drop for Controller {
    fn drop(&mut self) {
        self.shared.quit.store(true, Ordering::Release);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}
struct Exit(Arc<Shared>);
impl Drop for Exit {
    fn drop(&mut self) {
        if let Ok(mut view) = self.0.snapshot.lock() {
            let mut snapshot = (**view).clone();
            snapshot.failed = true;
            *view = Arc::new(snapshot);
        }
    }
}
fn run(shared: Arc<Shared>, rx: mpsc::Receiver<Command>) {
    let _exit = Exit(shared.clone());
    let Ok(project) = incoming::prototype_config() else {
        return;
    };
    let initial = Arc::new(Config {
        project: Arc::new(project),
        bindings: Arc::new(MachineBindings {
            schema_version: 1,
            bindings: vec![],
        }),
        safety: Safety::default(),
        selected: None,
        revision: 0,
    });
    if let Ok(mut active) = shared.active.lock() {
        *active = Some(initial.clone());
    }
    let Ok(mut model) = Model::from_config(&initial) else {
        return;
    };
    if let Ok(mut view) = shared.snapshot.lock() {
        *view = Arc::new(model.snapshot(0, 0));
    }
    while !shared.quit.load(Ordering::Acquire) {
        let retired = shared.retired.lock().ok().and_then(|mut r| r.take());
        drop(retired);
        let Ok(command) = rx.recv_timeout(Duration::from_millis(8)) else {
            continue;
        };
        let revision = command.revision;
        let action = command.action;
        if !matches!(&action, Action::Page(..) | Action::RefreshNames(..)) {
            model.issue = None;
            model.notice = None;
        }
        let result = (|| {
            if shared.blocked.load(Ordering::Acquire)
                && matches!(
                    &action,
                    Action::Validate | Action::Preview(..) | Action::Apply | Action::Export(..)
                )
            {
                return Err(Issue::new(
                    "/editor",
                    "Earlier input was rejected; Cancel or restore defaults before validation/apply/export",
                ));
            }
            match action {
                Action::RefreshNames(project) => {
                    project
                        .validate()
                        .map_err(|e| Issue::new("/running", e.reason))?;
                    let active = shared
                        .active
                        .lock()
                        .ok()
                        .and_then(|a| a.clone())
                        .ok_or_else(|| Issue::new("/worker", "No active configuration"))?;
                    let mut comparable = (*project).clone();
                    for p in &mut comparable.protocols {
                        if let Some(old) = active.project.protocols.iter().find(|o| o.id == p.id) {
                            p.name = old.name.clone();
                        }
                    }
                    comparable.grouping.names = active.project.grouping.names.clone();
                    if comparable == *active.project && *project != *active.project {
                        let clean = model.project
                            == String::from_utf8(
                                config::save_project(&active.project).map_err(|_| {
                                    Issue::new("/running", "Invalid active project")
                                })?,
                            )
                            .expect("JSON")
                            && model.patch_errors.is_empty()
                            && !model.error_limit
                            && model.safety == active.safety
                            && model.selected == active.selected
                            && model.bindings
                                == String::from_utf8(
                                    config::save_bindings(&active.bindings, &active.project)
                                        .map_err(|_| {
                                            Issue::new("/running", "Invalid active bindings")
                                        })?,
                                )
                                .expect("JSON");
                        let updated = Arc::new(Config {
                            project,
                            ..(*active).clone()
                        });
                        *shared
                            .active
                            .lock()
                            .map_err(|_| Issue::new("/worker", "Active unavailable"))? =
                            Some(updated.clone());
                        if clean {
                            model = Model::from_config(&updated)?;
                        }
                    }
                }
                Action::Text(d, t) => {
                    model.replace(d, t)?;
                }
                Action::Patch(d, p, t) => model.patch(d, &p, &t)?,
                Action::Select(id) => {
                    model.selected = id;
                    model.clear_preview();
                }
                Action::Page(d, p) => {
                    model.document = d;
                    model.page = p.min(32768);
                }
                Action::Add(k, i) => model.add(k, i)?,
                Action::Cancel => {
                    let active = shared
                        .active
                        .lock()
                        .ok()
                        .and_then(|a| a.clone())
                        .ok_or_else(|| Issue::new("/worker", "No active configuration"))?;
                    model = Model::from_config(&active)?;
                    shared.blocked.store(false, Ordering::Release);
                }
                Action::Defaults => {
                    let safe = Config {
                        project: Arc::new(ProjectConfig::default()),
                        bindings: Arc::new(MachineBindings {
                            schema_version: 1,
                            bindings: vec![],
                        }),
                        safety: Safety::default(),
                        selected: None,
                        revision: 0,
                    };
                    model = Model::from_config(&safe)?;
                    shared.blocked.store(false, Ordering::Release);
                    model.notice = Some("Safe defaults staged; Apply required");
                }
                Action::Validate => {
                    let cfg = model.validate(revision)?;
                    let _ = Processor::selected((*cfg.project).clone(), cfg.selected.as_deref())
                        .map_err(|_| Issue::new("/project", "Protocol compilation failed"))?;
                    model.notice = Some("Draft validated; active configuration unchanged");
                }
                Action::Preview(records) => {
                    model.preview(revision, records)?;
                    model.notice = Some(
                        "Preview uses draft and selected raw captures; active configuration unchanged",
                    );
                }
                Action::Apply => {
                    if shared.blocked.load(Ordering::Acquire) {
                        return Err(Issue::new(
                            "/editor",
                            "Earlier input was rejected; Cancel or restore defaults",
                        ));
                    }
                    let config = Arc::new(model.validate(revision)?);
                    let processor =
                        Processor::selected((*config.project).clone(), config.selected.as_deref())
                            .map_err(|_| Issue::new("/project", "Protocol compilation failed"))?;
                    let _gate = shared
                        .gate
                        .lock()
                        .map_err(|_| Issue::new("/worker", "Settings unavailable"))?;
                    if revision == shared.revision.load(Ordering::Acquire) {
                        *shared
                            .ready
                            .lock()
                            .map_err(|_| Issue::new("/worker", "Ready slot unavailable"))? =
                            Some(Prepared {
                                revision,
                                config,
                                processor,
                            });
                        model.notice = Some("Prepared; waiting for safe activation");
                        model.pending_revision = Some(revision);
                    }
                }
                Action::Import(d, path) => {
                    let bytes = settings_files::read(Path::new(&path)).map_err(file_error)?;
                    let text = String::from_utf8(bytes)
                        .map_err(|_| Issue::new("/file", "Expected UTF8 JSON"))?;
                    if d == Document::Safety {
                        let imported: SafetyFile = serde_json::from_str(&text).map_err(|e| {
                            Issue::new(
                                format!("/safety:{}:{}", e.line(), e.column()),
                                "Invalid runtime limit document",
                            )
                        })?;
                        if imported.schema_version != 1 {
                            return Err(Issue::new(
                                "/safety/schema_version",
                                "Expected version1 runtime limit file",
                            ));
                        }
                        imported.limits.validate()?;
                        model.safety = imported.limits;
                        model.patch_errors.retain(|p, _| !p.starts_with("/safety/"));
                        model.clear_preview();
                        model.notice = Some("Imported runtime limits staged; Apply required");
                        return Ok(());
                    }
                    let mut candidate = model.clone();
                    candidate.replace(d, text.clone())?;
                    candidate.validate(revision)?;
                    model.replace(d, text)?;
                    model.notice = Some("Imported draft only; Apply required");
                }
                Action::Export(d, path) => {
                    if d == Document::Safety {
                        if let Some(e) = model
                            .patch_errors
                            .values()
                            .find(|e| e.path.starts_with("/safety/"))
                        {
                            return Err(e.clone());
                        }
                        model.safety.validate()?;
                        let bytes = serde_json::to_vec_pretty(&SafetyFile {
                            schema_version: 1,
                            limits: model.safety,
                        })
                        .map_err(|_| Issue::new("/safety", "Cannot serialize runtime limits"))?;
                        settings_files::create(Path::new(&path), &bytes).map_err(file_error)?;
                        model.notice =
                            Some("Runtime limit export created; active configuration unchanged");
                        return Ok(());
                    }
                    let cfg = model.validate(revision)?;
                    let bytes = match d {
                        Document::Project => config::save_project(&cfg.project),
                        Document::Bindings => config::save_bindings(&cfg.bindings, &cfg.project),
                        Document::Safety => unreachable!("handled runtime limit export"),
                    }
                    .map_err(|_| Issue::new("/file", "Cannot serialize configuration"))?;
                    settings_files::create(Path::new(&path), &bytes).map_err(file_error)?;
                    model.notice =
                        Some("Export created; no overwrite or active configuration change");
                }
            }
            Ok(())
        })();
        if let Err(e) = result {
            model.issue = Some(e);
        }
        let active = shared
            .active
            .lock()
            .ok()
            .and_then(|a| a.as_ref().map(|c| c.revision))
            .unwrap_or(0);
        if let Some(pending) = model.pending_revision {
            if active == pending {
                model.notice = Some("Applied; capture and outgoing connection are stopped");
                model.pending_revision = None;
            } else if shared.revision.load(Ordering::Acquire) > pending {
                if model.notice == Some("Prepared; waiting for safe activation") {
                    model.notice =
                        Some("Prepared settings superseded; Apply again after validation");
                }
                model.pending_revision = None;
            }
        }
        if let Ok(mut view) = shared.snapshot.lock() {
            *view = Arc::new(model.snapshot(revision, active));
        }
    }
    let retired = shared.retired.lock().ok().and_then(|mut r| r.take());
    drop(retired);
}
fn file_error(e: settings_files::Error) -> Issue {
    Issue::new(
        "/file",
        match e {
            settings_files::Error::Read => "Cannot read selected file",
            settings_files::Error::TooLarge => "File exceeds1MiB",
            settings_files::Error::Exists => "Destination exists; choose a new path",
            settings_files::Error::WriteIncomplete => {
                "Export failed; destination may be incomplete"
            }
        },
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    fn wait(mut f: impl FnMut() -> bool) {
        let end = std::time::Instant::now() + Duration::from_secs(3);
        while !f() {
            assert!(std::time::Instant::now() < end);
            thread::sleep(Duration::from_millis(1));
        }
    }
    #[test]
    fn full_command_queue_and_rejected_edits_never_activate_stale_preparation() {
        let c = Controller::new().unwrap();
        wait(|| c.active().is_some());
        let original = c.active().unwrap();
        let guard = c.shared.snapshot.lock().unwrap();
        let start = std::time::Instant::now();
        let mut last = 0;
        let mut failures = 0;
        for _ in 0..64 {
            match c.request(Action::Validate) {
                Ok(r) => last = r,
                Err(_) => failures += 1,
            }
        }
        assert!(start.elapsed() < Duration::from_millis(100));
        assert!(failures > 0);
        drop(guard);
        wait(|| c.snapshot().is_some_and(|s| s.revision == last));
        let r = c.request(Action::Apply).unwrap();
        wait(|| c.snapshot().is_some_and(|s| s.revision == r));
        assert!(c.activate().unwrap().is_none());
        assert!(Arc::ptr_eq(&c.active().unwrap(), &original));
        let r = c.request(Action::Cancel).unwrap();
        wait(|| c.snapshot().is_some_and(|s| s.revision == r));
        let r = c.request(Action::Apply).unwrap();
        wait(|| c.snapshot().is_some_and(|s| s.revision == r));
        assert!(c.activate().unwrap().is_some());
    }
    #[test]
    fn queued_independent_field_changes_are_not_lost_by_global_revision_fence() {
        let c = Controller::new().unwrap();
        wait(|| c.snapshot().is_some_and(|s| !s.project.is_empty()));
        c.request(Action::Patch(
            Document::Project,
            "/protocols/0/name".into(),
            "Renamed first".into(),
        ))
        .unwrap();
        let r = c
            .request(Action::Patch(
                Document::Project,
                "/protocols/1/name".into(),
                "Renamed second".into(),
            ))
            .unwrap();
        wait(|| c.snapshot().is_some_and(|s| s.revision == r));
        let s = c.snapshot().unwrap();
        assert!(s.project.contains("Renamed first"));
        assert!(s.project.contains("Renamed second"));
    }
}
#[cfg(test)]
mod bounded_failure_tests {
    use super::*;
    #[test]
    fn error_history_is_bounded_and_counter_exhaustion_revokes_ready_apply() {
        let c = Controller::new().unwrap();
        let end = std::time::Instant::now() + Duration::from_secs(3);
        while c.active().is_none() {
            assert!(std::time::Instant::now() < end);
            thread::sleep(Duration::from_millis(1));
        }
        let active = c.active().unwrap();
        let mut model = Model::from_config(&active).unwrap();
        for n in 0..100 {
            assert!(
                model
                    .patch(Document::Project, &format!("/missing{n}"), "1")
                    .is_err()
            );
        }
        assert_eq!(model.patch_errors.len(), 64);
        assert!(model.error_limit);
        assert!(model.validate(1).is_err());
        let processor = Processor::selected((*active.project).clone(), None).unwrap();
        *c.shared.ready.lock().unwrap() = Some(Prepared {
            revision: u64::MAX,
            config: active,
            processor,
        });
        c.shared.revision.store(u64::MAX, Ordering::Release);
        assert!(c.request(Action::Apply).is_err());
        assert!(c.shared.blocked.load(Ordering::Acquire));
        assert!(c.activate().unwrap().is_none());
        c.shared.blocked.store(false, Ordering::Release);
        assert!(c.invalidate().is_err());
        assert!(c.activate().unwrap().is_none());
    }
}
