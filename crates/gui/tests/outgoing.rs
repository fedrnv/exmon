use egui_kittest::{Harness, kittest::Queryable};
use exmon_gui::{Shell, egui};
use std::time::{Duration, Instant};
fn advance(h: &mut Harness<'_, Shell>, mut ready: impl FnMut(&Shell) -> bool) {
    let end = Instant::now() + Duration::from_secs(4);
    while !ready(h.state()) {
        assert!(Instant::now() < end, "outgoing GUI timeout");
        h.step();
        std::thread::sleep(Duration::from_millis(2));
    }
    h.run_steps(2);
}
fn type_text(h: &mut Harness<'_, Shell>, label: &str, text: &str) {
    h.get_by_label(label).focus();
    h.run_steps(1);
    h.get_by_label(label).type_text(text);
    h.run_steps(2);
}
#[test]
fn production_clone_field_edit_validate_connect_send_schedule_and_dirty_stop() {
    let mut h = Harness::builder()
        .with_size(egui::vec2(1360., 900.))
        .build_ui_state(|ui, s: &mut Shell| s.show(ui), Shell::new(None).unwrap());
    h.run_steps(3);
    h.get_by_label("Start capture").click();
    advance(&mut h, |s| !s.incoming.status.groups.is_empty());
    h.get_by_label("Sample telemetry").click();
    h.run_steps(2);
    let received = h.state().incoming.view.selected.as_ref().unwrap().clone();
    h.get_by_label("Copy format to outgoing").click();
    advance(&mut h, |s| !s.outgoing.snapshot.templates.is_empty());
    let id = h.state().outgoing.snapshot.templates[0].id;
    assert_eq!(h.state().outgoing.editor.mock_status().unwrap().calls, 0);
    h.get_by_label("Field editor").click();
    h.run_steps(2);
    type_text(&mut h, "Field value", "1");
    assert!(!h.state().outgoing.editor.can_send(id));
    h.get_by_label("Apply field").click();
    advance(&mut h, |s| s.outgoing.snapshot.templates[0].revision > 1);
    assert_eq!(
        received.bytes(),
        h.state().incoming.draft.as_ref().unwrap().row.bytes()
    );
    assert_eq!(h.state().outgoing.editor.mock_status().unwrap().calls, 0);
    h.get_by_label("Allow mock transmission").click();
    h.run_steps(1);
    h.get_by_label("Connect mock TX").click();
    advance(&mut h, |s| s.outgoing.editor.can_send(id));
    h.get_by_label("Send once").click();
    advance(&mut h, |s| s.outgoing.snapshot.templates[0].tx_count == 1);
    assert_eq!(
        h.state()
            .outgoing
            .editor
            .mock_status()
            .unwrap()
            .last
            .as_ref(),
        h.state().outgoing.snapshot.templates[0].bytes.as_ref()
    );
    let before = h.state().outgoing.editor.mock_status().unwrap().calls;
    h.get_by_label("Start schedule").scroll_to_me();
    h.run_steps(3);
    h.get_by_label("Start schedule").click();
    advance(&mut h, |s| {
        s.outgoing.editor.mock_status().unwrap().calls > before
    });
    h.get_by_label("Stop schedule").click();
    h.run_steps(3);
    let after = h.state().outgoing.editor.mock_status().unwrap().calls;
    std::thread::sleep(Duration::from_millis(30));
    h.run_steps(2);
    assert!(h.state().outgoing.editor.mock_status().unwrap().calls <= after + 1);
    h.get_by_label("HEX editor").scroll_to_me();
    h.run_steps(3);
    h.get_by_label("HEX editor").click();
    h.run_steps(2);
    type_text(&mut h, "Message HEX", "GG");
    assert!(!h.state().outgoing.editor.can_send(id));
    h.get_by_label("Apply HEX").click();
    advance(&mut h, |s| s.outgoing.snapshot.error.is_some());
    assert!(!h.state().outgoing.editor.can_send(id));
    let count = h.state().outgoing.editor.mock_status().unwrap().calls;
    std::thread::sleep(Duration::from_millis(30));
    assert_eq!(
        h.state().outgoing.editor.mock_status().unwrap().calls,
        count
    );
    h.get_by_label("Stop capture").click();
    h.run_steps(2);
    assert!(!h.state().status().requested);
}
#[test]
fn manual_templates_start_unbound_and_render_supported_sizes_themes_dpi() {
    let root =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/exm027-visual");
    std::fs::create_dir_all(&root).unwrap();
    for (width, height, scale, light) in [
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
            .with_size(egui::vec2(width, height))
            .with_pixels_per_point(scale)
            .build_ui_state(|ui, s: &mut Shell| s.show(ui), app);
        h.run_steps(3);
        h.get_by_label("New raw message").click();
        advance(&mut h, |s| !s.outgoing.snapshot.templates.is_empty());
        assert!(!h.state().outgoing.snapshot.connected);
        assert_eq!(h.state().outgoing.editor.mock_status().unwrap().calls, 0);
        h.render()
            .unwrap()
            .save(root.join(format!(
                "outgoing-{width}x{height}-{scale}-{}.png",
                if light { "light" } else { "dark" }
            )))
            .unwrap();
        if width == 900. {
            h.get_by_label("Stop schedule").scroll_to_me();
            h.run_steps(3);
            h.render()
                .unwrap()
                .save(root.join(format!("outgoing-schedule-{width}x{height}-{scale}.png")))
                .unwrap();
            h.get_by_role_and_label(egui::accesskit::Role::Splitter, "Resize panel")
                .set_value_accesskit(0.75);
            h.run_steps(3);
            h.get_by_label("Stop schedule").scroll_to_me();
            h.run_steps(3);
            h.render()
                .unwrap()
                .save(root.join(format!("outgoing-narrow-{width}x{height}-{scale}.png")))
                .unwrap();
        }
    }
}
