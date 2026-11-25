use exmon_domain::{
    Direction,
    fields::Literal,
    observation::{Metadata, Observation},
};
use exmon_storage::{
    export::{self, Category, Error, Format, Selection},
    sessions::{Event, Limits, Raw, Recovery, Sample, Source, Writer},
};
use serde_json::{Value, json};
use std::{
    cell::Cell,
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

struct Temp(PathBuf);

impl Temp {
    fn new() -> Self {
        let time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("exmon032-{}-{time}", std::process::id()));
        fs::create_dir(&root).unwrap();
        Self(root)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    fn files(&self) -> Vec<String> {
        let mut names: Vec<_> = fs::read_dir(&self.0)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        names
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

const SOURCE: Source = Source {
    generation: 2,
    interface: 1 << 63,
    epoch: 3,
    sequence: 4,
};

fn raw(time: u64, metadata: Metadata) -> Event {
    let observation = Observation::new(time, Direction::Rx, metadata, Some(5), &[0, 0xab, 0xff])
        .unwrap()
        .with_source_timestamp(u64::MAX);
    Event::Raw(Raw::new(SOURCE, &observation))
}

fn message(time: u64, protocol: &str, name: &str, value: Option<f64>) -> Event {
    Event::Message {
        source: SOURCE,
        timestamp_ns: time,
        source_timestamp_ns: None,
        revision: 5,
        protocol: protocol.into(),
        name: name.into(),
        group: "group".into(),
        bytes: vec![0x01, 0x02],
        spans: vec![],
        samples: vec![
            Sample {
                field: "count".into(),
                unit: "V".into(),
                value,
                decoded: value.map(|_| Literal::Unsigned { value: u64::MAX }),
            },
            Sample {
                field: "state".into(),
                unit: String::new(),
                value: None,
                decoded: Some(Literal::Text {
                    value: String::from("=1+2, \"exact\"\n\t"),
                }),
            },
        ],
    }
}

fn charts(label: &str) -> Event {
    Event::Charts {
        definitions: json!([{
            "id": 7, "name": "Main plot", "samples": 8, "average": 2,
            "series": [{"protocol":"telemetry", "field":"count", "label": label, "unit": "mV"}]
        }]),
    }
}

fn fixture(path: &Path) -> Recovery {
    let mut session = Writer::create(path, Limits::default()).unwrap();
    session
        .append(&[
            raw(7, Metadata::Ethernet { link_type: 1 }),
            raw(
                8,
                Metadata::Serial {
                    configuration_revision: 2,
                },
            ),
            raw(9, Metadata::Ethernet { link_type: 1 }),
            charts("Before"),
            message(10, "telemetry", "  \u{feff}=1+2; \"reading\"", Some(-25.5)),
            message(12, "other", "other", Some(42.)),
            charts("@After"),
            message(20, "telemetry", "\treading", None),
            message(21, "telemetry", "later", Some(35.)),
        ])
        .unwrap();
    session.finish_with_loss(2, Some(SOURCE)).unwrap();
    drop(session);
    Recovery::open(path).unwrap()
}

fn json_at(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

#[test]
fn observation_json_is_exact_and_selection_is_inclusive() {
    let dir = Temp::new();
    let session = fixture(&dir.path("input.sqlite"));
    let dst = dir.path("out.json");
    assert_eq!(
        export::create(
            &session,
            &dst,
            Format::Json,
            &Selection {
                category: Category::Observations,
                from_ns: Some(7),
                through_ns: Some(8),
                type_id: "ethernet".into(),
                ..Selection::default()
            },
            || false
        ),
        Ok(1)
    );
    let data = json_at(&dst);
    assert_eq!(data["schema_version"], 1);
    assert_eq!(
        data["session"],
        json!({
            "session_closed": true, "session_dropped": 2, "session_records": 9,
            "last_loss": {"generation":2,"interface":1u64<<63,"epoch":3,"sequence":4}
        })
    );
    let row = &data["rows"][0];
    for (key, expected) in [
        ("kind", json!("observation")),
        ("record_id", json!(1)),
        ("timestamp_ns", json!(7)),
        ("source_timestamp_ns", json!(u64::MAX)),
        ("interface", json!(1u64 << 63)),
        ("type_id", json!("ethernet")),
        ("direction", json!("Rx")),
        ("metadata", json!({"Ethernet":{"link_type":1}})),
        ("original_length", json!(5)),
        ("bytes_hex", json!("00ABFF")),
    ] {
        assert_eq!(row[key], expected, "{key}");
    }
}

#[test]
fn fields_preserve_literals_and_csv_neutralizes_text_without_changing_numbers() {
    let dir = Temp::new();
    let session = fixture(&dir.path("input.sqlite"));
    let filter = Selection {
        from_ns: Some(10),
        through_ns: Some(20),
        type_id: "telemetry".into(),
        ..Selection::default()
    };
    let dst = dir.path("fields.json");
    assert_eq!(
        export::create(&session, &dst, Format::Json, &filter, || false),
        Ok(4)
    );
    let value = json_at(&dst);
    let rows = value["rows"].as_array().unwrap();
    assert_eq!(rows[0]["field"], "count");
    assert_eq!(rows[0]["unit"], "V");
    assert_eq!(rows[0]["value"], -25.5);
    assert_eq!(
        rows[0]["decoded"],
        json!({"kind":"unsigned","value":u64::MAX})
    );
    assert_eq!(rows[0]["definition_revision"], 5);
    assert!(rows[0]["bytes_hex"].is_null());
    assert_eq!(
        rows[1]["decoded"],
        json!({"kind":"text","value":"=1+2, \"exact\"\n\t"})
    );
    assert!(rows[2]["value"].is_null() && rows[2]["decoded"].is_null());
    let dst = dir.path("fields.csv");
    let filter = Selection {
        field: "count".into(),
        ..filter
    };
    assert_eq!(
        export::create(&session, &dst, Format::Csv, &filter, || false),
        Ok(2)
    );
    let csv = fs::read_to_string(dst).unwrap();
    let lines: Vec<_> = csv.split_terminator("\r\n").collect();
    assert_eq!(lines.len(), 4);
    assert!(
        lines[0].starts_with("schema_version,kind,session_closed,session_dropped,session_records,")
    );
    assert!(lines[1].starts_with("1,\"session\",true,2,9,"));
    assert_eq!(
        lines[2],
        "1,\"field\",,,,,5,10,,2,9223372036854775808,3,4,\"telemetry\",\"'  \u{feff}=1+2; \"\"reading\"\"\",5,,,,,\"count\",\"V\",-25.5,\"{\"\"kind\"\":\"\"unsigned\"\",\"\"value\"\":18446744073709551615}\",,,,,,"
    );
    assert!(lines[3].contains("\"'\treading\""));
}

#[test]
fn chart_points_use_current_selection_and_keep_invalid_samples_as_gaps() {
    let dir = Temp::new();
    let session = fixture(&dir.path("input.sqlite"));
    let selection = Selection {
        category: Category::ChartSeries,
        through_ns: Some(20),
        ..Selection::default()
    };
    let dst = dir.path("chart.json");
    assert_eq!(
        export::create(&session, &dst, Format::Json, &selection, || false),
        Ok(2)
    );
    let value = json_at(&dst);
    let rows = value["rows"].as_array().unwrap();
    assert_eq!(rows[0]["kind"], "chart_point");
    assert_eq!(rows[0]["chart_id"], 7);
    assert_eq!(rows[0]["chart_revision"], 4);
    assert_eq!(rows[0]["series_index"], 0);
    assert_eq!(rows[0]["series_label"], "Before");
    assert_eq!(rows[0]["unit"], "V");
    assert_eq!(rows[0]["series_unit"], "mV");
    assert_eq!(rows[0]["value"], -25.5);
    assert_eq!(rows[1]["chart_revision"], 7);
    assert_eq!(rows[1]["series_label"], "@After");
    assert!(rows[1]["value"].is_null() && rows[0]["decoded"].is_null());
    let empty = dir.path("empty.csv");
    let none = Selection {
        field: "missing".into(),
        ..selection
    };
    assert_eq!(
        export::create(&session, &empty, Format::Csv, &none, || false),
        Ok(0)
    );
    assert_eq!(fs::read_to_string(empty).unwrap().lines().count(), 2);
}

#[test]
fn large_paged_export_cancels_and_never_overwrites_a_concurrent_destination() {
    let dir = Temp::new();
    let input = dir.path("input.sqlite");
    let mut writer = Writer::create(&input, Limits::default()).unwrap();
    for batch in 0..25 {
        let records: Vec<_> = (batch * 80..(batch + 1) * 80)
            .map(|i| raw(i, Metadata::Ethernet { link_type: 1 }))
            .collect();
        writer.append(&records).unwrap();
    }
    drop(writer);
    let session = Recovery::open(&input).unwrap();
    let dst = dir.path("out.json");
    let selected = Selection {
        category: Category::Observations,
        ..Selection::default()
    };
    let polls = Cell::new(0);
    let cancelled = || {
        polls.set(polls.get() + 1);
        polls.get() == 150
    };
    assert_eq!(
        export::create(&session, &dst, Format::Json, &selected, cancelled),
        Err(Error::Cancelled)
    );
    assert_eq!(dir.files(), ["input.sqlite"]);
    assert_eq!(
        export::create(&session, &dst, Format::Csv, &selected, || true),
        Err(Error::Cancelled)
    );
    assert_eq!(dir.files(), ["input.sqlite"]);
    assert_eq!(
        export::create(&session, &dst, Format::Json, &selected, || {
            if !dst.exists() && dir.files().len() == 2 {
                fs::write(&dst, b"concurrent marker").unwrap();
            }
            false
        }),
        Err(Error::Exists)
    );
    assert_eq!(fs::read(&dst).unwrap(), b"concurrent marker");
    assert_eq!(dir.files(), ["input.sqlite", "out.json"]);
    assert_eq!(
        export::create(&session, &dst, Format::Json, &selected, || false),
        Err(Error::Exists)
    );
    let complete = dir.path("complete.json");
    assert_eq!(
        export::create(&session, &complete, Format::Json, &selected, || false),
        Ok(2000)
    );
    let data = json_at(&complete);
    assert_eq!(data["session"]["session_closed"], false);
    let rows = data["rows"].as_array().unwrap();
    for (i, row) in rows.iter().enumerate() {
        assert_eq!(row["record_id"], i + 1);
        assert_eq!(row["timestamp_ns"], i);
    }
}

#[test]
fn late_invalid_record_aborts_and_removes_the_staging_file() {
    let dir = Temp::new();
    let input = dir.path("input.sqlite");
    let mut writer = Writer::create(&input, Limits::default()).unwrap();
    let records: Vec<_> = (0..70)
        .map(|i| raw(i, Metadata::Ethernet { link_type: 1 }))
        .collect();
    writer.append(&records).unwrap();
    writer.finish(0).unwrap();
    drop(writer);
    let sql = rusqlite::Connection::open(&input).unwrap();
    sql.execute(
        "UPDATE entries SET data=zeroblob(length(data)) WHERE id=70",
        [],
    )
    .unwrap();
    drop(sql);
    let session = Recovery::open(&input).unwrap();
    let filter = Selection {
        category: Category::Observations,
        ..Selection::default()
    };
    assert!(matches!(
        export::create(
            &session,
            &dir.path("invalid.json"),
            Format::Json,
            &filter,
            || false
        ),
        Err(Error::Session(_))
    ));
    assert_eq!(dir.files(), ["input.sqlite"]);
}
