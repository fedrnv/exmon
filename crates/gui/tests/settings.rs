use egui_kittest::{Harness, kittest::Queryable};
use exmon_app::settings::Document;
use exmon_gui::{Shell, egui};
use std::time::{Duration, Instant};
#[track_caller]
fn wait(h: &mut Harness<'_, Shell>, mut f: impl FnMut(&Shell) -> bool) {
    let end = Instant::now() + Duration::from_secs(4);
    while !f(h.state()) {
        assert!(
            Instant::now() < end,
            "settings GUI timeout at {}: {:?}; active={:?} capture={} templates={} calls={:?}",
            std::panic::Location::caller(),
            h.state().settings.snapshot.issue,
            h.state().settings.controller.active().map(|c| c.revision),
            h.state().status().requested,
            h.state().outgoing.snapshot.templates.len(),
            h.state().outgoing.editor.mock_status().map(|s| s.calls)
        );
        h.step();
        std::thread::sleep(Duration::from_millis(2));
    }
    h.run_steps(2);
}
fn edit(h: &mut Harness<'_, Shell>, label: &str, text: &str) {
    h.get_by_label(label).scroll_to_me();
    h.run_steps(3);
    h.get_by_label(label).focus();
    h.run_steps(1);
    h.key_press_modifiers(
        egui::Modifiers {
            ctrl: true,
            command: true,
            ..Default::default()
        },
        egui::Key::A,
    );
    h.run_steps(1);
    h.get_by_label(label).type_text(text);
    h.run_steps(3);
}
fn click(h: &mut Harness<'_, Shell>, label: &str) {
    h.get_by_label(label).scroll_to_me();
    h.run_steps(20);
    h.get_by_label(label).click();
    h.run_steps(2);
}
#[test]
fn actual_field_error_cancel_preview_apply_and_schedule_revocation() {
    let mut h = Harness::builder()
        .with_size(egui::vec2(1360., 900.))
        .build_ui_state(|ui, s: &mut Shell| s.show(ui), Shell::new(None).unwrap());
    h.run_steps(3);
    click(&mut h, "Start capture");
    wait(&mut h, |s| !s.incoming.status.groups.is_empty());
    click(&mut h, "Sample telemetry");
    let received = h.state().incoming.view.selected.clone().unwrap();
    h.state_mut().outgoing.copy_received(&received);
    let id = h.state().outgoing.selected.unwrap();
    wait(&mut h, |s| {
        s.outgoing.snapshot.templates.iter().any(|t| t.id == id)
    });
    h.state_mut().outgoing.connect_mock(1).unwrap();
    wait(&mut h, |s| s.outgoing.editor.can_send(id));
    h.state()
        .outgoing
        .editor
        .start_schedule(
            id,
            exmon_app::outgoing::ScheduleInput {
                interval: "10".into(),
                ..Default::default()
            },
        )
        .unwrap();
    wait(&mut h, |s| {
        s.outgoing.editor.mock_status().unwrap().calls > 0
    });
    click(&mut h, "Settings");
    wait(&mut h, |s| !s.settings.snapshot.leaves.is_empty());
    let original = h.state().settings.controller.active().unwrap();
    edit(&mut h, "/project/protocols/0/fields/0/offset", "65535");
    assert!(
        h.state()
            .settings
            .snapshot
            .project
            .contains("\"offset\": 65535"),
        "Field edit not staged; revision {}, issue {:?}, value {:?}",
        h.state().settings.snapshot.revision,
        h.state().settings.snapshot.issue,
        h.get_by_label("/project/protocols/0/fields/0/offset")
            .value()
    );
    click(&mut h, "Validate settings");
    wait(&mut h, |s| s.settings.snapshot.issue.is_some());
    assert!(
        h.state()
            .settings
            .snapshot
            .issue
            .as_ref()
            .unwrap()
            .path
            .contains("/protocols/0/fields/0")
    );
    assert_eq!(
        h.state().settings.controller.active().unwrap().project,
        original.project
    );
    assert!(h.state().status().requested);
    click(&mut h, "Cancel settings changes");
    wait(&mut h, |s| s.settings.snapshot.issue.is_none());
    let before = h.state().status().packets;
    let end = Instant::now() + Duration::from_secs(1);
    let mut frames = 0;
    let mut worst = Duration::ZERO;
    while Instant::now() < end {
        let start = Instant::now();
        h.step();
        worst = worst.max(start.elapsed());
        frames += 1;
        std::thread::sleep(Duration::from_millis(3));
    }
    assert!(
        frames > 20 && worst < Duration::from_millis(200),
        "settings frames={frames},slowest={worst:?}"
    );
    assert!(h.state().status().packets > before);
    println!(
        "settings frames={frames},slowest={worst:?},packets={}",
        h.state().status().packets
    );
    click(&mut h, "Preview selected captures");
    wait(&mut h, |s| {
        s.settings.snapshot.preview.iter().any(|r| !r.raw())
    });
    let preview = h
        .state()
        .settings
        .snapshot
        .preview
        .iter()
        .find(|r| !r.raw())
        .unwrap();
    assert_eq!(preview.bytes(), received.bytes());
    assert_eq!(preview.fields[0].value, received.fields[0].value);
    click(&mut h, "Apply settings");
    wait(&mut h, |s| {
        s.settings
            .controller
            .active()
            .is_some_and(|c| c.revision > 0)
            && !s.status().requested
    });
    assert!(!h.state().outgoing.editor.can_send(id));
    let calls = h.state().outgoing.editor.mock_status().unwrap().calls;
    std::thread::sleep(Duration::from_millis(30));
    h.run_steps(2);
    assert!(h.state().outgoing.editor.mock_status().unwrap().calls <= calls + 1);
    assert!(h.state().incoming.view.selected.is_none());
}
#[test]
fn structured_additions_safety_and_complete_json_stage_defaults_are_inactive() {
    let mut h = Harness::builder()
        .with_size(egui::vec2(1360., 900.))
        .build_ui_state(|ui, s: &mut Shell| s.show(ui), Shell::new(None).unwrap());
    h.run_steps(3);
    click(&mut h, "Settings");
    wait(&mut h, |s| !s.settings.snapshot.project.is_empty());
    click(&mut h, "Add interface");
    wait(&mut h, |s| s.settings.snapshot.interfaces.len() == 1);
    click(&mut h, "Capture safety limits");
    wait(&mut h, |s| s.settings.snapshot.document == Document::Safety);
    edit(&mut h, "/safety/batch_items", "0");
    click(&mut h, "Apply settings");
    wait(&mut h, |s| {
        s.settings
            .snapshot
            .issue
            .as_ref()
            .is_some_and(|e| e.path == "/safety/batch_items")
    });
    assert_eq!(h.state().settings.controller.active().unwrap().revision, 0);
    click(&mut h, "Cancel settings changes");
    wait(&mut h, |s| s.settings.snapshot.issue.is_none());
    click(&mut h, "Advanced JSON editor");
    edit(&mut h, "Portable project JSON", "{\"schema_version\":999}");
    click(&mut h, "Stage JSON documents");
    click(&mut h, "Validate settings");
    wait(&mut h, |s| s.settings.snapshot.issue.is_some());
    assert_eq!(h.state().settings.controller.active().unwrap().revision, 0);
    click(&mut h, "Restore safe defaults");
    wait(&mut h, |s| {
        s.settings.snapshot.project.contains("\"protocols\": []")
    });
    assert_eq!(
        h.state()
            .settings
            .controller
            .active()
            .unwrap()
            .project
            .protocols
            .len(),
        2
    );
    assert!(!h.state().status().requested);
    click(&mut h, "Traffic");
    assert!(!h.state().settings_view);
    click(&mut h, "Settings");
    click(&mut h, "Charts");
    assert!(!h.state().settings_view && h.state().chart_view);
}
#[test]
fn actual_settings_sizes_themes_dpi_and_preview_render() {
    let root =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/exm029-visual");
    std::fs::create_dir_all(&root).unwrap();
    for (w, height, scale, light) in [
        (900., 600., 1., false),
        (1360., 900., 1., false),
        (900., 600., 1.5, true),
        (1360., 900., 1.5, true),
    ] {
        let mut app = Shell::new(None).unwrap();
        app.layout.theme = if light {
            exmon_app::shell::Theme::Light
        } else {
            exmon_app::shell::Theme::Dark
        };
        let mut h = Harness::builder()
            .with_size(egui::vec2(w, height))
            .with_pixels_per_point(scale)
            .build_ui_state(|ui, s: &mut Shell| s.show(ui), app);
        h.run_steps(3);
        click(&mut h, "Start capture");
        wait(&mut h, |s| !s.incoming.status.groups.is_empty());
        click(&mut h, "Sample telemetry");
        click(&mut h, "Settings");
        wait(&mut h, |s| !s.settings.snapshot.leaves.is_empty());
        h.render()
            .unwrap()
            .save(root.join(format!(
                "settings-{w}x{height}-{scale}-{}.png",
                if light { "light" } else { "dark" }
            )))
            .unwrap();
        click(&mut h, "Preview selected captures");
        wait(&mut h, |s| {
            s.settings.snapshot.preview.iter().any(|r| !r.raw())
        });
        h.render()
            .unwrap()
            .save(root.join(format!(
                "preview-{w}x{height}-{scale}-{}.png",
                if light { "light" } else { "dark" }
            )))
            .unwrap();
        click(&mut h, "Stop capture");
    }
}
#[test]
fn runtime_limit_file_export_import_uses_explicit_path_and_never_overwrites() {
    let root = std::env::temp_dir().join(format!(
        "exmon-gui-settings-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    let path = root.join("limits.json");
    let mut h = Harness::builder()
        .with_size(egui::vec2(1360., 900.))
        .build_ui_state(|ui, s: &mut Shell| s.show(ui), Shell::new(None).unwrap());
    h.run_steps(3);
    click(&mut h, "Settings");
    wait(&mut h, |s| !s.settings.snapshot.project.is_empty());
    click(&mut h, "Capture safety limits");
    wait(&mut h, |s| s.settings.snapshot.document == Document::Safety);
    edit(&mut h, "/safety/batch_items", "8");
    click(&mut h, "Import and export configuration");
    click(&mut h, "Runtime safety file");
    edit(&mut h, "Configuration file path", path.to_str().unwrap());
    click(&mut h, "Export draft to new file");
    wait(&mut h, |s| {
        s.settings
            .snapshot
            .notice
            .is_some_and(|n| n.contains("export created"))
    });
    let original = std::fs::read(&path).unwrap();
    assert!(
        String::from_utf8(original.clone())
            .unwrap()
            .contains("\"batch_items\": 8")
    );
    click(&mut h, "Export draft to new file");
    wait(&mut h, |s| {
        s.settings
            .snapshot
            .issue
            .as_ref()
            .is_some_and(|e| e.message.contains("exists"))
    });
    assert_eq!(std::fs::read(&path).unwrap(), original);
    edit(&mut h, "/safety/batch_items", "16");
    click(&mut h, "Import into draft");
    wait(&mut h, |s| {
        s.settings
            .snapshot
            .leaves
            .iter()
            .any(|l| l.pointer == "/batch_items" && l.value == "8")
    });
    assert_eq!(
        h.state()
            .settings
            .controller
            .active()
            .unwrap()
            .safety
            .batch_items,
        32
    );
    click(&mut h, "Apply settings");
    wait(&mut h, |s| {
        s.settings
            .controller
            .active()
            .is_some_and(|c| c.safety.batch_items == 8)
    });
    drop(h);
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(root).unwrap();
}
