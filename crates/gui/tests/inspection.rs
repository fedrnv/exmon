use egui_kittest::{Harness, kittest::Queryable};
use exmon_gui::{Shell, egui};
use std::time::{Duration, Instant};
#[track_caller]
fn wait(h: &mut Harness<'_, Shell>, mut f: impl FnMut(&Shell) -> bool) {
    let end = Instant::now() + Duration::from_secs(4);
    while !f(h.state()) {
        assert!(
            Instant::now() < end,
            "GUI timeout at {}: pending={} notice={:?}",
            std::panic::Location::caller(),
            h.state().inspection.pending,
            h.state().inspection.notice
        );
        h.step();
        std::thread::sleep(Duration::from_millis(2));
    }
    h.run_steps(3);
}
fn click(h: &mut Harness<'_, Shell>, label: &str) {
    h.get_by_label(label).scroll_to_me();
    h.run_steps(20);
    h.get_by_label(label).click();
    h.run_steps(3);
}
fn edit(h: &mut Harness<'_, Shell>, label: &str, text: &str) {
    h.get_by_label(label).scroll_to_me();
    h.run_steps(20);
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
fn sample(h: &mut Harness<'_, Shell>) {
    click(h, "Packets");
    click(h, "Load sample packet");
    wait(h, |s| {
        s.inspection.snapshot.synthetic && s.inspection.selected.is_some() && !s.inspection.pending
    });
}
fn select_udp(h: &mut Harness<'_, Shell>) {
    click(h, "Ethernet II");
    click(h, "IPv4");
    click(h, "UDP");
    assert_eq!(h.state().inspection.range, Some(34..42));
}
#[test]
fn actual_tree_selection_search_copy_filter_and_navigation() {
    let mut h = Harness::builder()
        .with_size(egui::vec2(1360., 900.))
        .build_ui_state(|ui, s: &mut Shell| s.show(ui), Shell::new(None).unwrap());
    h.run_steps(3);
    sample(&mut h);
    select_udp(&mut h);
    edit(&mut h, "Find bytes", "EXMON");
    click(&mut h, "Find");
    wait(&mut h, |s| {
        !s.inspection.pending && !s.inspection.matches.is_empty()
    });
    assert_eq!(h.state().inspection.range, Some(42..47));
    click(&mut h, "Copy HEX");
    wait(&mut h, |s| {
        s.inspection.notice.as_deref() == Some("Selected bytes copied")
    });
    assert!(!h.state().status().requested);
    edit(&mut h, "Display filter", "tcp");
    click(&mut h, "Apply display filter");
    wait(&mut h, |s| s.inspection.snapshot.filter == "tcp");
    assert!(h.state().inspection.snapshot.records.is_empty());
    assert!(h.state().inspection.selected.is_some());
    edit(&mut h, "Display filter", "bad=syntax");
    click(&mut h, "Apply display filter");
    wait(&mut h, |s| !s.inspection.pending);
    assert_eq!(h.state().inspection.snapshot.filter, "tcp");
    edit(&mut h, "Display filter", "udp ipv4 port=12001");
    click(&mut h, "Apply display filter");
    wait(&mut h, |s| s.inspection.snapshot.records.len() == 1);
    click(&mut h, "Charts");
    assert!(!h.state().packet_view);
    click(&mut h, "Packets");
    assert!(h.state().packet_view);
    click(&mut h, "Traffic");
    assert!(!h.state().packet_view);
}
#[test]
fn actual_file_import_export_no_overwrite_and_same_source_reload() {
    let root = std::env::temp_dir().join(format!("exmon030-gui-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let input = root.join("synthetic.pcap");
    let output = root.join("selection.bin");
    let mut bytes = vec![
        0xd4, 0xc3, 0xb2, 0xa1, 2, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 1, 0, 0, 0,
    ];
    for n in [0u32, 0, 50, 50] {
        bytes.extend(n.to_le_bytes());
    }
    bytes.extend(exmon_app::inspection::sample().bytes());
    std::fs::write(&input, bytes).unwrap();
    let mut h = Harness::builder()
        .with_size(egui::vec2(1360., 900.))
        .build_ui_state(|ui, s: &mut Shell| s.show(ui), Shell::new(None).unwrap());
    h.run_steps(3);
    sample(&mut h);
    click(&mut h, "Import / export files");
    edit(&mut h, "Capture path", input.to_str().unwrap());
    click(&mut h, "Import PCAP / PCAPNG");
    wait(&mut h, |s| {
        s.inspection.snapshot.file_mode && !s.inspection.pending
    });
    select_udp(&mut h);
    let first = h.state().inspection.selected.as_ref().unwrap().id;
    edit(&mut h, "New binary export path", output.to_str().unwrap());
    click(&mut h, "Export selected bytes");
    wait(&mut h, |s| {
        s.inspection.notice.as_deref() == Some("Selected raw bytes exported")
    });
    assert_eq!(
        std::fs::read(&output).unwrap(),
        &exmon_app::inspection::sample().bytes()[34..42]
    );
    click(&mut h, "Export selected bytes");
    wait(&mut h, |s| {
        s.inspection.notice.as_deref() == Some("Destination exists; nothing overwritten")
    });
    assert_eq!(std::fs::metadata(&output).unwrap().len(), 8);
    click(&mut h, "Import PCAP / PCAPNG");
    wait(&mut h, |s| {
        s.inspection
            .selected
            .as_ref()
            .is_some_and(|r| r.id != first)
    });
    assert_eq!(
        h.state()
            .inspection
            .selected
            .as_ref()
            .unwrap()
            .frame
            .protocol,
        "UDP"
    );
    std::fs::write(&input, b"invalid").unwrap();
    click(&mut h, "Import PCAP / PCAPNG");
    wait(&mut h, |s| !s.inspection.pending);
    assert!(h.state().inspection.snapshot.file_mode);
    assert_eq!(h.state().inspection.snapshot.records.len(), 1);
    click(&mut h, "Live inspection");
    wait(&mut h, |s| !s.inspection.snapshot.file_mode);
    assert!(h.state().inspection.selected.is_none());
    assert!(!h.state().status().requested);
    drop(h);
    std::fs::remove_file(input).unwrap();
    std::fs::remove_file(output).unwrap();
    std::fs::remove_dir(root).unwrap();
}
#[test]
fn incoming_pause_does_not_pause_raw_packet_inspector() {
    let mut h = Harness::builder()
        .with_size(egui::vec2(1360., 900.))
        .build_ui_state(|ui, s: &mut Shell| s.show(ui), Shell::new(None).unwrap());
    h.run_steps(3);
    click(&mut h, "Start capture");
    click(&mut h, "Pause display");
    click(&mut h, "Clear view");
    click(&mut h, "Packets");
    wait(&mut h, |s| !s.inspection.snapshot.records.is_empty());
    assert!(h.state().incoming.view.rows.is_empty());
    let before = h.state().inspection.snapshot.records.last().unwrap().id;
    let end = Instant::now() + Duration::from_millis(600);
    let mut frames = 0;
    let mut max = Duration::ZERO;
    while Instant::now() < end {
        let start = Instant::now();
        h.step();
        max = max.max(start.elapsed());
        frames += 1;
        std::thread::sleep(Duration::from_millis(3));
    }
    assert!(h.state().inspection.snapshot.records.last().unwrap().id > before);
    assert!(frames > 20 && max < Duration::from_millis(200));
    println!("inspection live frames={frames},max={max:?}");
    click(&mut h, "Stop capture");
}
#[test]
fn inspection_visual_matrix_sizes_themes_and_dpi() {
    let root =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/exm030-visual");
    std::fs::create_dir_all(&root).unwrap();
    for (w, height) in [(900., 600.), (1360., 900.)] {
        for scale in [1., 1.5] {
            for light in [false, true] {
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
                sample(&mut h);
                select_udp(&mut h);
                h.get_by_label("Packet inspection").scroll_to_me();
                h.run_steps(20);
                h.render()
                    .unwrap()
                    .save(root.join(format!(
                        "inspection-{w}x{height}-{scale}-{}.png",
                        if light { "light" } else { "dark" }
                    )))
                    .unwrap();
            }
        }
    }
}
