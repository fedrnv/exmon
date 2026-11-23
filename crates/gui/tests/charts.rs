use egui_kittest::{Harness, kittest::Queryable};
use exmon_gui::{Shell, egui};
use std::time::{Duration, Instant};
fn wait(h: &mut Harness<'_, Shell>, mut f: impl FnMut(&Shell) -> bool) {
    let end = Instant::now() + Duration::from_secs(4);
    while !f(h.state()) {
        assert!(Instant::now() < end, "chart GUI timeout");
        h.step();
        std::thread::sleep(Duration::from_millis(2));
    }
    h.run_steps(2);
}
fn type_text(h: &mut Harness<'_, Shell>, label: &str, text: &str) {
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
    h.run_steps(2);
}
fn start(h: &mut Harness<'_, Shell>) {
    h.run_steps(3);
    h.get_by_label("Start capture").click();
    wait(h, |s| {
        s.incoming.status.ready && s.incoming.worker.configuration().is_some()
    });
    h.get_by_label("Charts").click();
    h.run_steps(3);
    h.get_by_label("Select numeric series").click();
    h.run_steps(3);
    h.get_by_label("Sample telemetry / sequence []").click();
    h.run_steps(2);
    h.get_by_label("Sample telemetry / interface []").click();
    wait(h, |s| {
        s.charts
            .snapshot
            .charts
            .first()
            .is_some_and(|c| c.series.len() == 2 && c.series[0].retained > 32)
    });
}
#[test]
fn actual_series_tabs_windows_ranges_axes_and_live_responsiveness() {
    let mut h = Harness::builder()
        .with_size(egui::vec2(1360., 900.))
        .build_ui_state(|ui, s: &mut Shell| s.show(ui), Shell::new(None).unwrap());
    start(&mut h);
    h.get_by_label("Select numeric series").click();
    h.run_steps(2);
    type_text(&mut h, "Chart sample window", "64");
    type_text(&mut h, "Chart average length", "3");
    h.get_by_label("Apply chart window").click();
    wait(&mut h, |s| {
        s.charts.snapshot.charts[0].spec.samples == 64
            && s.charts.snapshot.charts[0].series[0].retained == 64
    });
    assert_eq!(h.state().charts.snapshot.charts[0].spec.average, 3);
    type_text(&mut h, "Chart average length", "0");
    h.get_by_label("Apply chart window").click();
    h.run_steps(2);
    assert_eq!(h.state().charts.snapshot.charts[0].spec.average, 3);
    h.get_by_label("Index axis").click();
    h.run_steps(2);
    assert!(!h.state().charts.time_axis());
    h.get_by_label("Zoom chart in").click();
    h.run_steps(2);
    assert!(!h.state().charts.autoscale());
    let b = h.state().charts.bounds();
    h.get_by_label("Zoom chart out").click();
    h.run_steps(2);
    assert!(h.state().charts.bounds().x[1] - h.state().charts.bounds().x[0] > b.x[1] - b.x[0]);
    let rect = h.state().charts.plot_rect.unwrap();
    let before_pan = h.state().charts.bounds();
    h.hover_at(rect.center());
    h.run_steps(1);
    h.drag_at(rect.center());
    h.run_steps(1);
    h.hover_at(rect.center() + egui::vec2(50., 15.));
    h.run_steps(2);
    h.drop_at(rect.center() + egui::vec2(50., 15.));
    h.run_steps(2);
    assert_ne!(h.state().charts.bounds(), before_pan);
    h.get_by_label("Manual chart ranges").click();
    h.run_steps(3);
    for (label, value) in [
        ("X minimum", "0"),
        ("X maximum", "1000000"),
        ("Y minimum", "-10"),
        ("Y maximum", "1000000"),
    ] {
        type_text(&mut h, label, value);
    }
    h.get_by_label("Apply chart ranges").click();
    h.run_steps(2);
    assert_eq!(h.state().charts.bounds().y, [-10., 1000000.]);
    let b = h.state().charts.bounds();
    type_text(&mut h, "Y maximum", "NaN");
    h.get_by_label("Apply chart ranges").click();
    h.run_steps(2);
    assert_eq!(h.state().charts.bounds(), b);
    h.get_by_label("Reset chart view").click();
    h.run_steps(2);
    assert!(h.state().charts.autoscale());
    h.get_by_label("New chart").click();
    h.run_steps(2);
    assert_eq!(h.state().charts.specs.len(), 2);
    h.get_by_label("Chart 1").click();
    h.run_steps(2);
    assert_eq!(h.state().charts.selected, 0);
    h.get_by_label("Chart 2").click();
    h.run_steps(2);
    h.get_by_label("Remove chart").click();
    h.run_steps(2);
    assert_eq!(h.state().charts.specs.len(), 1);
    let before = h.state().status().packets;
    let end = Instant::now() + Duration::from_secs(1);
    let mut frames = 0;
    let mut worst = Duration::ZERO;
    while Instant::now() < end {
        let t = Instant::now();
        h.step();
        worst = worst.max(t.elapsed());
        frames += 1;
        std::thread::sleep(Duration::from_millis(3));
    }
    assert!(frames > 20 && worst < Duration::from_millis(200));
    assert!(h.state().status().packets > before);
    assert!(
        h.state().charts.snapshot.charts[0]
            .series
            .iter()
            .all(|s| s.retained <= 64 && s.points.len() <= 512)
    );
    println!(
        "charts frames={frames},slowest={worst:?},packets={}",
        h.state().status().packets
    );
    h.get_by_label("Stop capture").click();
    h.run_steps(2);
}
#[test]
fn chart_visual_sizes_dpi_themes() {
    let root =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/exm028-visual");
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
        start(&mut h);
        h.get_by_label("Select numeric series").click();
        h.run_steps(3);
        h.render()
            .unwrap()
            .save(root.join(format!(
                "charts-{w}x{height}-{scale}-{}.png",
                if light { "light" } else { "dark" }
            )))
            .unwrap();
        h.get_by_label("Stop capture").click();
        h.run_steps(2);
    }
}
