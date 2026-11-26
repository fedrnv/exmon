use egui_kittest::{Harness, kittest::Queryable};
use exmon_app::sessions::State;
use exmon_gui::{Shell, egui};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

#[track_caller]
fn wait(h: &mut Harness<'_, Shell>, mut ready: impl FnMut(&Shell) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !ready(h.state()) {
        assert!(
            Instant::now() < deadline,
            "prototype step timed out at {}",
            std::panic::Location::caller()
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

struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "exmon039-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            match std::fs::remove_dir_all(&self.0) {
                Ok(()) => break,
                Err(e) => {
                    assert!(Instant::now() < deadline, "fixture cleanup failed: {e}");
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
        }
    }
}

#[test]
fn complete_virtual_session_preserves_data_and_explicit_transmission() {
    let files = Files::new();
    let session = files.0.join("session.sqlite");
    let pcap = files.0.join("synthetic.pcap");
    let frame = exmon_app::inspection::sample();
    let mut container = vec![
        0xd4, 0xc3, 0xb2, 0xa1, 2, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xff, 0xff, 0, 0, 1, 0, 0, 0,
    ];
    for n in [1, 0, frame.bytes().len() as u32, frame.bytes().len() as u32] {
        container.extend(n.to_le_bytes());
    }
    container.extend(frame.bytes());
    std::fs::write(&pcap, container).unwrap();
    let mut h = Harness::builder()
        .with_size(egui::vec2(1360., 900.))
        .build_ui_state(
            |ui, shell: &mut Shell| shell.show(ui),
            Shell::new(None).unwrap(),
        );
    h.run_steps(3);
    assert_eq!(h.state().outgoing.editor.mock_status().unwrap().calls, 0);
    assert!(!h.state().status().requested);
    click(&mut h, "Sessions");
    edit(&mut h, "Session file path", session.to_str().unwrap());
    click(&mut h, "Start recording");
    wait(&mut h, |s| {
        s.sessions.snapshot.state == State::Recording && !s.sessions.pending
    });
    click(&mut h, "Traffic");
    click(&mut h, "Start capture");
    wait(&mut h, |s| !s.incoming.status.groups.is_empty());
    click(&mut h, "Sample telemetry");
    let received = h.state().incoming.view.selected.clone().unwrap();
    let original = received.bytes().to_vec();
    click(&mut h, "Copy format to outgoing");
    wait(&mut h, |s| !s.outgoing.snapshot.templates.is_empty());
    let id = h.state().outgoing.snapshot.templates[0].id;
    click(&mut h, "Field editor");
    edit(&mut h, "Field value", "1");
    click(&mut h, "Apply field");
    wait(&mut h, |s| s.outgoing.snapshot.templates[0].revision > 1);
    assert_eq!(received.bytes(), original);
    assert_eq!(h.state().outgoing.editor.mock_status().unwrap().calls, 0);
    click(&mut h, "Allow mock transmission");
    click(&mut h, "Connect mock TX");
    wait(&mut h, |s| s.outgoing.editor.can_send(id));
    click(&mut h, "Send once");
    wait(&mut h, |s| s.outgoing.snapshot.templates[0].tx_count == 1);
    let status = h.state().outgoing.editor.mock_status().unwrap();
    assert_eq!(
        status.last.as_ref(),
        h.state().outgoing.snapshot.templates[0].bytes.as_ref()
    );
    click(&mut h, "Start schedule");
    wait(&mut h, |s| {
        s.outgoing.editor.mock_status().unwrap().calls > 1
    });
    click(&mut h, "Stop schedule");
    click(&mut h, "Charts");
    click(&mut h, "Select numeric series");
    click(&mut h, "Sample telemetry / sequence []");
    wait(&mut h, |s| {
        s.charts.snapshot.charts[0]
            .series
            .first()
            .is_some_and(|s| s.retained > 8)
    });
    assert!(
        h.state().charts.snapshot.charts[0].series[0]
            .points
            .iter()
            .any(|p| p.average.is_some())
    );
    click(&mut h, "Packets");
    click(&mut h, "Import / export files");
    edit(&mut h, "Capture path", pcap.to_str().unwrap());
    click(&mut h, "Import PCAP / PCAPNG");
    wait(&mut h, |s| {
        s.inspection.snapshot.file_mode && !s.inspection.pending
    });
    assert_eq!(
        h.state()
            .inspection
            .selected
            .as_ref()
            .unwrap()
            .frame
            .bytes(),
        frame.bytes()
    );
    click(&mut h, "Stop capture");
    click(&mut h, "Sessions");
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
    assert!(!h.state().status().requested);
    assert!(!h.state().outgoing.active());
    assert_eq!(h.state().recorder.client().token(), 0);
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/exm039-visual");
    std::fs::create_dir_all(&dir).unwrap();
    h.render()
        .unwrap()
        .save(dir.join("complete-session.png"))
        .unwrap();
    drop(h);
}
