use egui_kittest::{Harness, kittest::Queryable};
use exmon_app::sessions::{Event, State};
use exmon_gui::{Shell, egui};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "exmon031-gui-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        Self(root)
    }
    fn path(&self) -> PathBuf {
        self.0.join("session.sqlite")
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        assert_eq!(self.0.parent(), Some(std::env::temp_dir().as_path()));
        let end = Instant::now() + Duration::from_secs(3);
        loop {
            match std::fs::remove_dir_all(&self.0) {
                Ok(()) => break,
                Err(e) => {
                    assert!(Instant::now() < end, "owned session cleanup: {e}");
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
        }
    }
}
#[track_caller]
fn wait(h: &mut Harness<'_, Shell>, mut f: impl FnMut(&Shell) -> bool) {
    let end = Instant::now() + Duration::from_secs(5);
    while !f(h.state()) {
        assert!(
            Instant::now() < end,
            "session GUI timeout at {} {:?} {:?} pending={}",
            std::panic::Location::caller(),
            h.state().sessions.snapshot.state,
            h.state().sessions.snapshot.notice,
            h.state().sessions.pending
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
fn start(h: &mut Harness<'_, Shell>, path: &std::path::Path) {
    click(h, "Sessions");
    edit(h, "Session file path", path.to_str().unwrap());
    click(h, "Start recording");
    wait(h, |s| {
        s.sessions.snapshot.state == State::Recording && !s.sessions.pending
    });
}
#[test]
fn actual_record_capture_annotation_stop_recover_and_no_overwrite() {
    let root = Temp::new();
    let mut h = Harness::builder()
        .with_size(egui::vec2(1360., 900.))
        .build_ui_state(|ui, s: &mut Shell| s.show(ui), Shell::new(None).unwrap());
    h.run_steps(3);
    assert!(!root.path().exists());
    start(&mut h, &root.path());
    click(&mut h, "Start capture");
    wait(&mut h, |s| {
        s.sessions
            .snapshot
            .summary
            .as_ref()
            .is_some_and(|s| s.records > 10)
    });
    edit(&mut h, "Session annotation", "Synthetic user note");
    click(&mut h, "Add annotation");
    assert!(h.state().sessions.annotation.is_empty());
    let end = Instant::now() + Duration::from_millis(700);
    let mut frames = 0;
    let mut slowest = Duration::ZERO;
    while Instant::now() < end {
        let start = Instant::now();
        h.step();
        slowest = slowest.max(start.elapsed());
        frames += 1;
        std::thread::sleep(Duration::from_millis(3));
    }
    assert!(frames > 20 && slowest < Duration::from_millis(200));
    println!("session UI frames={frames},max={slowest:?}");
    click(&mut h, "Stop recording");
    wait(&mut h, |s| {
        matches!(s.sessions.snapshot.state, State::Stopped | State::Failed) && !s.sessions.pending
    });
    assert!(h.state().status().requested);
    let count = h
        .state()
        .sessions
        .snapshot
        .summary
        .as_ref()
        .unwrap()
        .records;
    let packets = h.state().status().packets;
    h.run_steps(20);
    assert!(h.state().status().packets > packets);
    assert_eq!(
        h.state()
            .sessions
            .snapshot
            .summary
            .as_ref()
            .unwrap()
            .records,
        count
    );
    click(&mut h, "Stop capture");
    let active = h.state().settings.controller.active().unwrap();
    click(&mut h, "Recover session");
    wait(&mut h, |s| {
        s.sessions.snapshot.state == State::Recovered && !s.sessions.pending
    });
    assert!(!h.state().status().requested);
    assert_eq!(h.state().recorder.client().token(), 0);
    assert!(std::sync::Arc::ptr_eq(
        &active,
        &h.state().settings.controller.active().unwrap()
    ));
    assert!(!h.state().outgoing.active());
    let raw = h
        .state()
        .sessions
        .snapshot
        .entries
        .iter()
        .find(|e| matches!(e.event.as_ref(), Event::Raw(_)))
        .unwrap()
        .clone();
    click(&mut h, &format!("#{} Raw observation", raw.id));
    assert_eq!(h.state().sessions.selected.as_ref().unwrap().id, raw.id);
    click(&mut h, "Export session data");
    click(&mut h, "Raw observations");
    click(&mut h, "CSV");
    let export = root.0.join("export.csv");
    edit(&mut h, "New export file path", export.to_str().unwrap());
    assert!(!export.exists());
    click(&mut h, "Save export");
    wait(&mut h, |s| {
        s.sessions.snapshot.export_notice == Some("Export complete") && !s.sessions.pending
    });
    let csv = std::fs::read_to_string(&export).unwrap();
    assert!(csv.starts_with("schema_version,kind,"));
    assert!(csv.contains("\"observation\""));
    assert!(!h.state().status().requested);
    assert_eq!(h.state().recorder.client().token(), 0);
    assert!(!h.state().outgoing.active());
    click(&mut h, "Save export");
    wait(&mut h, |s| {
        s.sessions.snapshot.export_notice == Some("Export destination exists; choose a new file")
            && !s.sessions.pending
    });
    assert_eq!(std::fs::read_to_string(&export).unwrap(), csv);
    click(&mut h, "Export session data");
    if h.state().sessions.snapshot.more {
        let first = h.state().sessions.snapshot.entries[0].id;
        click(&mut h, "Next records");
        wait(&mut h, |s| {
            s.sessions.snapshot.after > 0 && !s.sessions.pending
        });
        assert!(h.state().sessions.snapshot.entries[0].id > first);
        let after = h.state().sessions.snapshot.after;
        let revision = h.state().sessions.snapshot.revision;
        edit(
            &mut h,
            "Session file path",
            root.0.join("missing.sqlite").to_str().unwrap(),
        );
        click(&mut h, "Recover session");
        wait(&mut h, |s| {
            s.sessions.snapshot.revision > revision && !s.sessions.pending
        });
        assert_eq!(h.state().sessions.snapshot.after, after);
        click(&mut h, "Previous records");
        wait(&mut h, |s| {
            s.sessions.snapshot.after == 0 && !s.sessions.pending
        });
    }
    let r = exmon_app::sessions::Recovery::open(&root.path()).unwrap();
    let mut after = 0;
    let mut note = false;
    loop {
        let entries = r.page(after).unwrap();
        if entries.is_empty() {
            break;
        }
        for entry in &entries {
            if matches!(entry.event.as_ref(),Event::Annotation{text,..}if text=="Synthetic user note")
            {
                note = true;
            }
        }
        after = entries.last().unwrap().id;
    }
    assert!(note);
    drop(r);
    let bytes = std::fs::read(root.path()).unwrap();
    edit(&mut h, "Session file path", root.path().to_str().unwrap());
    click(&mut h, "Start recording");
    wait(&mut h, |s| s.sessions.snapshot.state == State::Failed);
    assert_eq!(std::fs::read(root.path()).unwrap(), bytes);
    drop(h);
}
#[test]
fn invalid_limits_recovery_and_recording_error_are_explicit() {
    let root = Temp::new();
    let mut h = Harness::builder()
        .with_size(egui::vec2(900., 600.))
        .build_ui_state(|ui, s: &mut Shell| s.show(ui), Shell::new(None).unwrap());
    h.run_steps(3);
    click(&mut h, "Sessions");
    edit(&mut h, "Session file path", root.path().to_str().unwrap());
    edit(&mut h, "Database MiB", "0");
    click(&mut h, "Start recording");
    assert!(h.state().sessions.notice.is_some());
    assert!(!root.path().exists());
    edit(&mut h, "Database MiB", "1");
    edit(&mut h, "Record limit", "10");
    click(&mut h, "Start recording");
    wait(&mut h, |s| s.sessions.snapshot.state == State::Recording);
    click(&mut h, "Start capture");
    wait(&mut h, |s| s.sessions.snapshot.state == State::Failed);
    assert!(h.state().status().requested);
    assert!(
        h.state()
            .sessions
            .snapshot
            .notice
            .unwrap()
            .contains("budget")
    );
    click(&mut h, "Stop capture");
    drop(h);
}
#[test]
fn sessions_visual_matrix_recovered_raw_and_export_controls() {
    let screenshots = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/exm032-visual");
    std::fs::create_dir_all(&screenshots).unwrap();
    for (w, height) in [(900., 600.), (1360., 900.)] {
        for scale in [1., 1.5] {
            for light in [false, true] {
                let root = Temp::new();
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
                start(&mut h, &root.path());
                click(&mut h, "Start capture");
                wait(&mut h, |s| {
                    s.sessions
                        .snapshot
                        .summary
                        .as_ref()
                        .is_some_and(|s| s.records > 10)
                });
                click(&mut h, "Stop capture");
                click(&mut h, "Stop recording");
                wait(&mut h, |s| {
                    s.sessions
                        .snapshot
                        .summary
                        .as_ref()
                        .is_some_and(|s| s.closed)
                        && !s.sessions.pending
                });
                click(&mut h, "Recover session");
                wait(&mut h, |s| {
                    s.sessions.snapshot.state == State::Recovered && !s.sessions.pending
                });
                let raw = h
                    .state()
                    .sessions
                    .snapshot
                    .entries
                    .iter()
                    .find(|e| matches!(e.event.as_ref(), Event::Raw(_)))
                    .unwrap()
                    .clone();
                click(&mut h, &format!("#{} Raw observation", raw.id));
                click(&mut h, "Export session data");
                if light {
                    click(&mut h, "Raw observations");
                }
                h.get_by_label("Save export").scroll_to_me();
                h.run_steps(20);
                h.render()
                    .unwrap()
                    .save(screenshots.join(format!(
                        "sessions-{w}x{height}-{scale}-{}.png",
                        if light { "light" } else { "dark" }
                    )))
                    .unwrap();
                drop(h);
            }
        }
    }
}
