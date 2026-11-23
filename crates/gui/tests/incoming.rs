use egui_kittest::{Harness, kittest::Queryable};
use exmon_gui::{Shell, egui};
use std::time::{Duration, Instant};
fn app() -> Shell {
    Shell::new(None).unwrap()
}
fn advance(h: &mut Harness<'_, Shell>, mut condition: impl FnMut(&Shell) -> bool) {
    let end = Instant::now() + Duration::from_secs(3);
    while !condition(h.state()) {
        assert!(Instant::now() < end, "incoming UI timeout");
        h.step();
        std::thread::sleep(Duration::from_millis(2));
    }
    h.run_steps(2);
}
#[test]
fn production_incoming_tabs_selection_pause_copy_rename_and_responsive_rendering() {
    let mut h = Harness::builder()
        .with_size(egui::vec2(1360., 900.))
        .build_ui_state(|ui, s: &mut Shell| s.show(ui), app());
    h.run_steps(3);
    h.get_by_label("Start capture").click();
    advance(&mut h, |s| {
        !s.incoming.status.groups.is_empty() && s.retained() > 32
    });
    h.get_by_label("Sample telemetry").click();
    h.run_steps(3);
    let selected = h.state().incoming.view.selected.as_ref().unwrap().clone();
    assert_eq!(selected.fields.len(), 2);
    h.get_by_label("Copy format to outgoing").click();
    h.run_steps(3);
    assert_eq!(
        h.state().incoming.draft.as_ref().unwrap().row.bytes(),
        selected.bytes()
    );
    let end = Instant::now() + Duration::from_secs(1);
    let mut frames = 0;
    let mut slowest = Duration::ZERO;
    while Instant::now() < end {
        let start = Instant::now();
        h.step();
        slowest = slowest.max(start.elapsed());
        frames += 1;
        std::thread::sleep(Duration::from_millis(3));
    }
    assert!(frames > 20);
    assert!(slowest < Duration::from_millis(200), "frame {slowest:?}");
    assert!(h.state().status().packets > 1024);
    assert!(h.state().retained() <= 1024);
    assert_eq!(
        h.state().incoming.view.selected.as_ref().unwrap().key,
        selected.key
    );
    assert!(h.state().incoming.view.selected_evicted());
    h.get_by_label("Pause display").click();
    h.run_steps(2);
    assert!(h.state().incoming.view.paused);
    let packets = h.state().status().packets;
    let count = h.state().retained();
    for _ in 0..8 {
        h.step();
        std::thread::sleep(Duration::from_millis(3));
    }
    assert!(h.state().status().packets > packets);
    assert_eq!(h.state().retained(), count);
    assert!(h.state().incoming.view.paused_skipped > 0);
    h.get_by_label("Pause display").click();
    h.run_steps(3);
    assert!(!h.state().incoming.view.paused);
    h.get_by_label("Type name").focus();
    h.run_steps(1);
    h.get_by_label("Type name").type_text(" renamed");
    h.run_steps(2);
    h.get_by_label("Rename type").click();
    advance(&mut h, |s| {
        s.incoming
            .status
            .groups
            .iter()
            .any(|g| g.name.contains("renamed"))
    });
    assert_eq!(
        h.state().incoming.view.selected.as_ref().unwrap().key,
        selected.key
    );
    h.get_by_label("Raw traffic").click();
    h.run_steps(3);
    assert!(h.state().incoming.view.tab.is_none());
    h.get_by_label("Pause display").click();
    h.run_steps(3);
    let raw = h.state().incoming.view.visible()[0].clone();
    let row_label = format!(
        "{:.3}   #{}   {:?}   Raw / {} B",
        raw.observation.timestamp_ns() as f64 / 1e9,
        raw.sequence,
        raw.observation.direction(),
        raw.bytes().len()
    );
    h.get_by_label(&row_label).click();
    h.run_steps(3);
    assert_eq!(
        h.state().incoming.view.selected.as_ref().unwrap().key,
        raw.key
    );
    h.get_by_label("Stop capture").click();
    h.run_steps(3);
    assert!(!h.state().status().requested);
    println!(
        "incoming UI: frames={frames}, slowest={slowest:?}, packets={}, evicted={}, paused_skipped={}",
        h.state().status().packets,
        h.state().incoming.view.evicted,
        h.state().incoming.view.paused_skipped
    );
}
#[test]
fn incoming_visual_matrix_has_real_decoded_instances_at_supported_sizes() {
    let root =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/exm026-visual");
    std::fs::create_dir_all(&root).unwrap();
    for (width, height, scale, light) in [
        (900., 600., 1., false),
        (1360., 900., 1., false),
        (900., 600., 1.5, true),
        (1360., 900., 1.5, true),
    ] {
        let mut shell = app();
        shell.layout.theme = if light {
            exmon_app::shell::Theme::Light
        } else {
            exmon_app::shell::Theme::Dark
        };
        let mut h = Harness::builder()
            .with_size(egui::vec2(width, height))
            .with_pixels_per_point(scale)
            .build_ui_state(|ui, s: &mut Shell| s.show(ui), shell);
        h.run_steps(3);
        h.get_by_label("Start capture").click();
        advance(&mut h, |s| {
            !s.incoming.status.groups.is_empty() && s.retained() > 32
        });
        h.get_by_label("Sample telemetry").click();
        h.run_steps(3);
        if width == 900. {
            h.get_by_label("Parsed fields").scroll_to_me();
            h.run_steps(3);
        }
        h.render()
            .unwrap()
            .save(root.join(format!(
                "incoming-{width}x{height}-{scale}-{}.png",
                if light { "light" } else { "dark" }
            )))
            .unwrap();
        h.get_by_label("Stop capture").click();
        h.run_steps(2);
    }
}
#[test]
fn invalid_definition_failure_remains_visible_after_capture_generation_changes() {
    let mut shell = app();
    shell.incoming.worker = exmon_app::incoming::Worker::configured(
        exmon_domain::config::ProjectConfig {
            schema_version: 999,
            ..Default::default()
        },
        None,
    )
    .unwrap();
    let mut h = Harness::builder()
        .with_size(egui::vec2(1200., 760.))
        .build_ui_state(|ui, s: &mut Shell| s.show(ui), shell);
    h.run_steps(3);
    h.get_by_label("Start capture").click();
    advance(&mut h, |s| {
        s.incoming.status.failed && s.status().packets > 128
    });
    h.get_by_label("Incoming definitions unavailable; capture remains independent.");
    h.get_by_label("Stop capture").click();
    h.run_steps(2);
    assert!(!h.state().status().requested);
}
