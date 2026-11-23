use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use exmon_gui::{Shell, egui};
use std::time::{Duration, Instant};
fn input(h: &mut Harness<'_, Shell>, label: &str, text: &str) {
    h.get_by_role_and_label(egui::accesskit::Role::TextInput, label)
        .focus();
    h.run_steps(1);
    h.get_by_role_and_label(egui::accesskit::Role::TextInput, label)
        .type_text(text);
    h.run_steps(3);
}
fn tcp(h: &mut Harness<'_, Shell>) {
    h.get_by_label("Transport").click();
    h.run_steps(3);
    h.get_by_label("TCP").click();
    h.run_steps(3);
}
#[test]
fn invalid_filter_is_visible_before_start_and_direct_start_cannot_bypass_it() {
    for (label, text, error) in [
        (
            "Either port",
            "65536",
            "Ports must be comma-separated numbers from 0 to 65535.",
        ),
        (
            "Source IP",
            "not-an-ip",
            "Enter a numeric IPv4 or IPv6 address.",
        ),
        (
            "Either port",
            "80,,443",
            "Ports must be comma-separated numbers from 0 to 65535.",
        ),
    ] {
        let mut h = Harness::builder()
            .with_size(egui::vec2(900., 600.))
            .build_ui_state(|ui, s: &mut Shell| s.show(ui), Shell::new(None).unwrap());
        h.run_steps(3);
        input(&mut h, label, text);
        h.get_by_label(error);
        assert!(
            h.get_by_label("Start capture")
                .accesskit_node()
                .is_disabled()
        );
        assert!(h.state_mut().start_capture().is_err());
        assert!(!h.state().status().requested);
        assert_eq!(h.state().status().packets, 0);
    }
}
#[test]
fn displayed_running_plan_is_frozen_then_rebound_on_explicit_restart() {
    let mut h = Harness::builder()
        .with_size(egui::vec2(1200., 800.))
        .build_ui_state(|ui, s: &mut Shell| s.show(ui), Shell::new(None).unwrap());
    h.run_steps(3);
    tcp(&mut h);
    input(&mut h, "Either port", "12001,12000,12000");
    h.get_by_label("Ready filter: interface 1; direction any; either port 12000,12001");
    h.get_by_label("Start capture").click();
    h.run_steps(3);
    let deadline = Instant::now() + Duration::from_secs(2);
    while h.state().retained() == 0 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(3));
        h.run_steps(2);
    }
    assert!(h.state().retained() > 0);
    h.get_by_label("Running filter: interface 1; direction any; either port 12000,12001");
    assert!(
        h.get_by_role_and_label(egui::accesskit::Role::TextInput, "Either port")
            .accesskit_node()
            .is_disabled()
    );
    assert!(h.get_by_label("Transport").accesskit_node().is_disabled());
    h.get_by_label("Stop capture").click();
    h.run_steps(3);
    let deadline = Instant::now() + Duration::from_secs(1);
    while h.state().status().running && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(3));
        h.run_steps(2);
    }
    h.run_steps(2);
    h.get_by_label("Ready filter: interface 1; direction any; either port 12000,12001");
    h.get_by_label("Interface").click();
    h.run_steps(3);
    h.get_by_label("Mock 02").click();
    h.run_steps(3);
    h.get_by_label("Ready filter: interface 2; direction any; either port 12000,12001");
    h.get_by_label("Start capture").click();
    h.run_steps(3);
    let deadline = Instant::now() + Duration::from_secs(1);
    while !h.state().status().running && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(3));
        h.run_steps(2);
    }
    h.run_steps(2);
    h.get_by_label("Running filter: interface 2; direction any; either port 12000,12001");
    h.get_by_label("Stop capture").click();
    h.run_steps(3);
}
#[test]
fn actual_filter_controls_render_at_supported_sizes_themes_and_dpi() {
    let root =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/exm010-visual");
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
            .with_theme(if light {
                egui::Theme::Light
            } else {
                egui::Theme::Dark
            })
            .build_ui_state(|ui, s: &mut Shell| s.show(ui), app);
        h.run_steps(3);
        tcp(&mut h);
        input(&mut h, "Either port", "12000");
        h.get_by_label("Start capture").click();
        h.run_steps(3);
        std::thread::sleep(Duration::from_millis(30));
        h.run_steps(3);
        let image = h.render().unwrap();
        assert_eq!(
            image.dimensions(),
            ((width * scale) as u32, (height * scale) as u32)
        );
        image
            .save(root.join(format!(
                "filter-{width}x{height}-{scale}-{}.png",
                if light { "light" } else { "dark" }
            )))
            .unwrap();
        h.get_by_label("Stop capture").click();
        h.run_steps(3);
    }
}
