use egui_kittest::{Harness, kittest::Queryable};
use exmon_gui::{Shell, egui};
use std::time::{Duration, Instant};
fn shell() -> (Shell, std::path::PathBuf) {
    let path = std::env::temp_dir().join(format!(
        "exmon-ui-layout-{}-{}.json",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    (Shell::new(Some(path.clone())).unwrap(), path)
}
#[test]
fn production_ui_interacts_under_sustained_capture_and_renders_sizes_themes_dpi() {
    let (app, path) = shell();
    let mut h = Harness::builder()
        .with_size(egui::vec2(1200., 760.))
        .build_ui_state(|ui, s: &mut Shell| s.show(ui), app);
    h.run_steps(3);
    h.get_by_label("Start capture").click();
    h.run_steps(3);
    let deadline = Instant::now() + Duration::from_secs(1);
    let mut frames = 0;
    let mut slowest = Duration::ZERO;
    while Instant::now() < deadline {
        let begin = Instant::now();
        h.step();
        slowest = slowest.max(begin.elapsed());
        frames += 1;
        std::thread::sleep(Duration::from_millis(3));
    }
    assert!(h.state().status().packets >= 1024);
    assert!(h.state().retained() <= 1024);
    assert!(frames > 20);
    assert!(
        slowest < Duration::from_millis(200),
        "UI frame stalled: {slowest:?}"
    );
    h.get_by_label("Light theme").click();
    h.run_steps(3);
    h.get_by_label("Stop capture").click();
    h.run_steps(3);
    assert!(!h.state().status().requested);
    h.get_by_label("Clear view").click();
    h.run_steps(3);
    assert_eq!(h.state().retained(), 0);
    h.get_by_label("Transport").click();
    h.run_steps(3);
    h.get_by_label("CAN").click();
    h.run_steps(3);
    assert_eq!(
        h.state().transport(),
        exmon_domain::observation::TransportKind::Can
    );
    h.get_by_role_and_label(egui::accesskit::Role::Splitter, "Resize panel")
        .set_value_accesskit(0.65);
    h.run_steps(3);
    assert!((h.state().layout.split - 0.65).abs() < 0.02);
    println!(
        "sustained UI frames={frames}, slowest={slowest:?}, packets={}",
        h.state().status().packets
    );
    drop(h);
    let _ = std::fs::remove_file(path);
    let root =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/exm025-visual");
    std::fs::create_dir_all(&root).unwrap();
    for (width, height, scale, light) in [
        (900., 600., 1., false),
        (1360., 900., 1., false),
        (900., 600., 1.5, true),
        (1360., 900., 1.5, true),
    ] {
        let (mut app, path) = shell();
        app.layout.theme = if light {
            exmon_app::shell::Theme::Light
        } else {
            exmon_app::shell::Theme::Dark
        };
        let mut image = Harness::builder()
            .with_size(egui::vec2(width, height))
            .with_pixels_per_point(scale)
            .with_theme(if light {
                egui::Theme::Light
            } else {
                egui::Theme::Dark
            })
            .build_ui_state(|ui, s: &mut Shell| s.show(ui), app);
        image.run_steps(3);
        image.get_by_label("Start capture").click();
        image.run_steps(3);
        std::thread::sleep(Duration::from_millis(40));
        image.run_steps(3);
        image
            .render()
            .unwrap()
            .save(root.join(format!(
                "shell-{width}x{height}-{scale}-{}.png",
                if light { "light" } else { "dark" }
            )))
            .unwrap();
        image.get_by_label("Stop capture").click();
        image.run_steps(2);
        drop(image);
        let _ = std::fs::remove_file(path);
    }
}

#[test]
fn saved_split_and_theme_restore_into_the_actual_panel_without_starting_capture() {
    let (unused, path) = shell();
    drop(unused);
    let prefs = exmon_app::shell::PreferencesWorker::spawn(Some(path.clone())).unwrap();
    prefs
        .save(exmon_app::shell::Layout {
            width: 900.,
            height: 600.,
            split: 0.7,
            theme: exmon_app::shell::Theme::Light,
            ..Default::default()
        })
        .unwrap();
    drop(prefs);
    let app = Shell::new(Some(path.clone())).unwrap();
    let mut h = Harness::builder()
        .with_size(egui::vec2(900., 600.))
        .build_ui_state(|ui, s: &mut Shell| s.show(ui), app);
    h.run_steps(4);
    assert!(!h.state().status().requested);
    assert_eq!(h.state().layout.theme, exmon_app::shell::Theme::Light);
    assert!(
        (h.state().layout.split - 0.7).abs() < 0.02,
        "restored split {}",
        h.state().layout.split
    );
    drop(h);
    std::fs::remove_file(path).unwrap();
}
