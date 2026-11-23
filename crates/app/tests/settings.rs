use exmon_app::{
    settings::*,
    shell::{Batch, Captured},
};
use exmon_domain::{
    Direction,
    observation::{Metadata, Observation},
};
use serde_json::{Value, json};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
fn wait(mut f: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(4);
    while !f() {
        assert!(Instant::now() < end, "settings timeout");
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn controller() -> Controller {
    let c = Controller::new().unwrap();
    wait(|| c.active().is_some() && c.snapshot().is_some_and(|s| !s.project.is_empty()));
    c
}
fn perform(c: &Controller, a: Action) -> u64 {
    let r = c.request(a).unwrap();
    wait(|| c.snapshot().is_some_and(|s| s.revision == r));
    r
}
fn project(kind: &str) -> String {
    json!({"schema_version":1,"protocols":[{"id":"p","name":"Synthetic voltage","framing":{"kind":"fixed","length":4},"classification":{"length":4,"prefix":[165]},"fields":[{"id":"word","offset":1,"length":2,"representation":"unsigned","endian":"little","scale":0.5,"bias":1.0,"unit":"V"}]}],"interfaces":[{"id":"port","protocol_id":"p","transport":{"kind":kind},"filter":{"direction":"Rx","ports":[42]},"tx_allowed":false}]}).to_string()
}
fn records(kind: &str, port: u16) -> Vec<Captured> {
    let chunks = if kind == "tcp" {
        vec![vec![165, 1], vec![2, 0]]
    } else {
        vec![vec![165, 1, 2, 0]]
    };
    chunks
        .into_iter()
        .enumerate()
        .map(|(i, b)| Captured {
            interface_id: 1,
            epoch: 7,
            sequence: i as u64 + 1,
            observation: Observation::new(
                i as u64 * 1000,
                Direction::Rx,
                if kind == "tcp" {
                    Metadata::Tcp {
                        local: "127.0.0.1:41".parse().unwrap(),
                        remote: format!("127.0.0.1:{port}").parse().unwrap(),
                        connection: 1,
                    }
                } else {
                    Metadata::Udp {
                        local: "127.0.0.1:41".parse().unwrap(),
                        remote: format!("127.0.0.1:{port}").parse().unwrap(),
                    }
                },
                Some(b.len()),
                &b,
            )
            .unwrap(),
        })
        .collect()
}
#[test]
fn field_errors_cancel_defaults_and_invalid_apply_preserve_active() {
    let c = controller();
    let original = c.active().unwrap();
    perform(
        &c,
        Action::Patch(
            Document::Project,
            "/protocols/0/fields/0/offset".into(),
            "65535".into(),
        ),
    );
    perform(&c, Action::Apply);
    assert!(
        c.snapshot()
            .unwrap()
            .issue
            .as_ref()
            .unwrap()
            .path
            .contains("/protocols/0/fields/0")
    );
    assert!(c.activate().unwrap().is_none());
    assert!(Arc::ptr_eq(&c.active().unwrap(), &original));
    perform(&c, Action::Cancel);
    assert!(c.snapshot().unwrap().issue.is_none());
    assert_eq!(*c.active().unwrap().project, *original.project);
    perform(
        &c,
        Action::Patch(Document::Safety, "/batch_items".into(), "0".into()),
    );
    perform(&c, Action::Apply);
    assert_eq!(
        c.snapshot().unwrap().issue.as_ref().unwrap().path,
        "/safety/batch_items"
    );
    assert!(Arc::ptr_eq(&c.active().unwrap(), &original));
    perform(&c, Action::Defaults);
    assert!(c.snapshot().unwrap().project.contains("\"protocols\": []"));
    assert_eq!(*c.active().unwrap().project, *original.project);
    perform(&c, Action::Cancel);
    assert_eq!(
        c.snapshot().unwrap().project.as_ref(),
        original_json(&original)
    );
}
fn original_json(c: &Config) -> String {
    String::from_utf8(exmon_storage::config::save_project(&c.project).unwrap()).unwrap()
}
#[test]
fn preview_and_committed_pipeline_match_for_datagrams_streams_and_filters() {
    for kind in ["udp", "tcp"] {
        let c = controller();
        perform(&c, Action::Text(Document::Project, project(kind)));
        perform(&c, Action::Select(Some("port".into())));
        perform(&c, Action::Preview(records(kind, 42)));
        assert!(
            c.snapshot().unwrap().issue.is_none(),
            "{:?}",
            c.snapshot().unwrap().issue
        );
        let preview = c.snapshot().unwrap().preview.clone();
        let row = preview.iter().find(|r| !r.raw()).unwrap();
        assert_eq!(row.bytes(), [165, 1, 2, 0]);
        assert_eq!(row.fields[0].numeric, Some(257.5));
        assert_eq!(row.fields[0].name, "word");
        let revision = perform(&c, Action::Apply);
        let mut activation = c.activate().unwrap().unwrap();
        assert_eq!(activation.config.revision, revision);
        activation.worker.begin(1).unwrap();
        activation
            .worker
            .submit(Batch {
                generation: 1,
                records: records(kind, 42),
            })
            .unwrap();
        let mut live = vec![];
        wait(|| {
            for b in activation.worker.drain() {
                live.extend(b.rows);
            }
            live.iter().any(|r| !r.raw())
        });
        let actual = live.iter().find(|r| !r.raw()).unwrap();
        assert_eq!(actual.bytes(), row.bytes());
        assert_eq!(actual.fields[0].value, row.fields[0].value);
        assert_eq!(actual.fields[0].invalid, row.fields[0].invalid);
        assert_eq!(actual.quality, row.quality);
        assert_eq!(actual.sources, row.sources);
        perform(&c, Action::Preview(records(kind, 43)));
        assert!(c.snapshot().unwrap().preview.is_empty());
    }
}
#[test]
fn malformed_unknown_oversized_and_stale_preparations_fail_closed() {
    let c = controller();
    let original = c.active().unwrap();
    for text in [
        "{".to_owned(),
        "{\"schema_version\":1,\"surprise\":1}".into(),
        "{\"schema_version\":999}".into(),
    ] {
        perform(&c, Action::Text(Document::Project, text));
        perform(&c, Action::Apply);
        assert!(c.snapshot().unwrap().issue.is_some());
        assert!(c.activate().unwrap().is_none());
        assert!(Arc::ptr_eq(&c.active().unwrap(), &original));
    }
    perform(&c, Action::Cancel);
    perform(&c, Action::Apply);
    c.invalidate().unwrap();
    assert!(c.activate().unwrap().is_none());
    perform(&c, Action::Apply);
    assert!(
        c.request(Action::Text(Document::Project, "x".repeat(1_048_577)))
            .is_err()
    );
    assert!(c.activate().unwrap().is_none());
    perform(&c, Action::Cancel);
    perform(&c, Action::Apply);
    assert!(c.activate().unwrap().is_some());
}
#[test]
fn exact_duplicate_interface_schedule_diagnostics_and_framing_modes() {
    let c = controller();
    let active = c.active().unwrap();
    for (v, expected) in [
        (
            json!({"schema_version":1,"protocols":[{"id":"p","name":"p","framing":{"kind":"fixed","length":1}},{"id":"p","name":"p","framing":{"kind":"fixed","length":1}}]}),
            "/project/protocols/1/id",
        ),
        (
            json!({"schema_version":1,"protocols":[{"id":"p","name":"p","framing":{"kind":"fixed","length":1}}],"interfaces":[{"id":"i","protocol_id":"p","transport":{"kind":"serial","baud":0,"data_bits":8,"stop_bits":1,"parity":"none"}}]}),
            "/project/interfaces/0/transport/baud",
        ),
    ] {
        let mut m = Model::from_config(&active).unwrap();
        m.replace(Document::Project, v.to_string()).unwrap();
        assert_eq!(m.validate(1).err().unwrap().path, expected);
    }
    let mut m = Model::from_config(&active).unwrap();
    for kind in [
        "datagram",
        "fixed",
        "delimiter",
        "length_prefix",
        "markers",
        "idle_gap",
    ] {
        m.patch(Document::Project, "/protocols/1/framing/kind", kind)
            .unwrap();
        let cfg = m
            .validate(1)
            .unwrap_or_else(|e| panic!("{kind}: {}: {}", e.path, e.message));
        let value: Value = serde_json::from_str(&original_json(&cfg)).unwrap();
        assert_eq!(value["protocols"][1]["framing"]["kind"], kind);
    }
}
#[test]
fn portable_exports_bindings_roundtrip_and_failed_import_is_atomic() {
    let c = controller();
    perform(&c, Action::Text(Document::Project, project("udp")));
    perform(&c,Action::Text(Document::Bindings,json!({"schema_version":1,"bindings":[{"interface_id":"port","selector":"SYNTHETIC_LOCAL_ONLY"}]}).to_string()));
    perform(&c, Action::Validate);
    assert!(c.snapshot().unwrap().issue.is_none());
    let root = std::env::temp_dir().join(format!(
        "exmon-settings-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    let profile = root.join("project.json");
    let bindings = root.join("bindings.json");
    perform(
        &c,
        Action::Export(Document::Project, profile.to_str().unwrap().into()),
    );
    perform(
        &c,
        Action::Export(Document::Bindings, bindings.to_str().unwrap().into()),
    );
    let profile_bytes = std::fs::read(&profile).unwrap();
    assert!(
        !String::from_utf8(profile_bytes.clone())
            .unwrap()
            .contains("SYNTHETIC_LOCAL_ONLY")
    );
    assert!(
        String::from_utf8(std::fs::read(&bindings).unwrap())
            .unwrap()
            .contains("SYNTHETIC_LOCAL_ONLY")
    );
    perform(
        &c,
        Action::Export(Document::Project, profile.to_str().unwrap().into()),
    );
    assert!(
        c.snapshot()
            .unwrap()
            .issue
            .as_ref()
            .unwrap()
            .message
            .contains("exists")
    );
    assert_eq!(std::fs::read(&profile).unwrap(), profile_bytes);
    let draft = c.snapshot().unwrap().project.clone();
    std::fs::write(root.join("bad.json"), b"{bad}").unwrap();
    perform(
        &c,
        Action::Import(
            Document::Project,
            root.join("bad.json").to_str().unwrap().into(),
        ),
    );
    assert_eq!(c.snapshot().unwrap().project, draft);
    perform(&c, Action::Text(Document::Project, "{".into()));
    perform(
        &c,
        Action::Import(Document::Project, profile.to_str().unwrap().into()),
    );
    assert!(c.snapshot().unwrap().issue.is_none());
    for name in ["project.json", "bindings.json", "bad.json"] {
        std::fs::remove_file(root.join(name)).unwrap();
    }
    std::fs::remove_dir(&root).unwrap();
}
#[test]
fn names_from_running_pipeline_preserve_dirty_binding_drafts_and_report_exact_binding_field() {
    let c = controller();
    let mut running = (*c.active().unwrap().project).clone();
    running.protocols[0].name = "Runtime rename".into();
    perform(&c, Action::RefreshNames(Arc::new(running.clone())));
    assert_eq!(
        c.active().unwrap().project.protocols[0].name,
        "Runtime rename"
    );
    perform(&c, Action::Text(Document::Project, project("udp")));
    perform(
        &c,
        Action::Text(
            Document::Bindings,
            json!({"schema_version":1,"bindings":[{"interface_id":"port","selector":""}]})
                .to_string(),
        ),
    );
    perform(&c, Action::Validate);
    assert_eq!(
        c.snapshot().unwrap().issue.as_ref().unwrap().path,
        "/bindings/bindings/0/selector"
    );
    let bad_bindings = c.snapshot().unwrap().bindings.clone();
    running.protocols[1].name = "Other runtime rename".into();
    perform(&c, Action::RefreshNames(Arc::new(running)));
    assert_eq!(c.snapshot().unwrap().bindings, bad_bindings);
}

#[test]
fn public_model_builder_rejects_unvalidated_inputs_without_panicking() {
    let c = controller();
    let mut active = (*c.active().unwrap()).clone();
    let mut project = (*active.project).clone();
    project.schema_version = 999;
    active.project = Arc::new(project);
    assert!(Model::from_config(&active).is_err());
}
#[test]
fn structural_actions_reject_malformed_json_shapes_without_losing_worker() {
    let c = controller();
    for (text, kind) in [
        ("[]", "protocol"),
        ("{\"protocols\":true}", "field"),
        ("{\"protocols\":[null]}", "field"),
    ] {
        perform(&c, Action::Text(Document::Project, text.into()));
        perform(&c, Action::Add(kind, 0));
        assert!(c.snapshot().unwrap().issue.is_some());
        assert!(!c.snapshot().unwrap().failed);
    }
    perform(&c, Action::Text(Document::Project, project("udp")));
    perform(&c, Action::Text(Document::Bindings, "false".into()));
    perform(&c, Action::Add("binding", 0));
    assert!(c.snapshot().unwrap().issue.is_some());
    assert!(!c.snapshot().unwrap().failed);
    perform(&c, Action::Cancel);
    assert!(c.snapshot().unwrap().issue.is_none());
    assert_eq!(c.active().unwrap().revision, 0);
}
#[test]
fn runtime_limit_import_rejects_future_versions_bad_types_unknown_flags_and_oversize_atomically() {
    let c = controller();
    let active = c.active().unwrap();
    let path = std::env::temp_dir().join(format!(
        "exmon-invalid-limits-{}-{}.json",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let valid = json!({"schema_version":1,"limits":{"queue_items":64,"queue_bytes":4096,"batch_items":32,"batch_bytes":2048,"snapshot_bytes":64}});
    let mut cases = Vec::new();
    let mut v = valid.clone();
    v["schema_version"] = json!(2);
    cases.push(v);
    let mut v = valid.clone();
    v["limits"]["snapshot_bytes"] = json!(65);
    cases.push(v);
    let mut v = valid.clone();
    v["limits"]["batch_items"] = json!(0);
    cases.push(v);
    let mut v = valid.clone();
    v["enabled"] = json!(true);
    cases.push(v);
    let mut v = valid;
    v["limits"] = json!([]);
    cases.push(v);
    for invalid in cases {
        std::fs::write(&path, invalid.to_string()).unwrap();
        perform(
            &c,
            Action::Import(Document::Safety, path.to_str().unwrap().into()),
        );
        assert!(c.snapshot().unwrap().issue.is_some());
        assert!(Arc::ptr_eq(&c.active().unwrap(), &active));
        perform(&c, Action::Page(Document::Safety, 0));
        assert!(
            c.snapshot()
                .unwrap()
                .leaves
                .iter()
                .any(|l| l.pointer == "/batch_items" && l.value == "32")
        );
    }
    std::fs::write(&path, vec![b' '; 1_048_577]).unwrap();
    perform(
        &c,
        Action::Import(Document::Safety, path.to_str().unwrap().into()),
    );
    assert_eq!(c.snapshot().unwrap().issue.as_ref().unwrap().path, "/file");
    assert!(Arc::ptr_eq(&c.active().unwrap(), &active));
    std::fs::remove_file(path).unwrap();
}
