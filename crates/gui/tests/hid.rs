use egui_kittest::{
    Harness,
    kittest::{NodeT as _, Queryable},
};
use exmon_app::hid_worker::*;
use exmon_gui::{Shell, egui, hid::HidPanel};
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
#[derive(Default)]
struct Audit {
    opens: Vec<usize>,
    writes: Vec<(usize, Vec<u8>)>,
    selected: Option<usize>,
    removed: bool,
}
struct Fixture(Arc<Mutex<Audit>>);
impl Source for Fixture {
    fn enumerate(&mut self, _: usize) -> Result<Vec<Device>, Error> {
        Ok((1..=2)
            .map(|product_id| Device {
                vendor_id: 0x1209,
                product_id,
                usage_page: 0xff00,
                usage: 1,
            })
            .collect())
    }
    fn open_selected(&mut self, id: usize) -> Result<(), Error> {
        let mut a = self.0.lock().unwrap();
        assert!(id < 2);
        a.opens.push(id);
        a.selected = Some(id);
        Ok(())
    }
    fn read(&mut self, bytes: &mut [u8], _: i32) -> Result<usize, Error> {
        let a = self.0.lock().unwrap();
        if a.removed {
            return Err(Error::Disconnected);
        }
        assert!(a.selected.is_some());
        bytes[..4].copy_from_slice(&[3, 165, 0, 42]);
        Ok(4)
    }
    fn write(&mut self, bytes: &[u8]) -> Result<usize, Error> {
        let mut a = self.0.lock().unwrap();
        let id = a.selected.ok_or(Error::InvalidState)?;
        assert!(a.writes.len() < 16);
        a.writes.push((id, bytes.to_vec()));
        Ok(bytes.len())
    }
    fn close(&mut self) {
        self.0.lock().unwrap().selected = None;
    }
}
fn advance(h: &mut Harness<'_, Shell>, label: &str, mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while !condition() {
        if Instant::now() >= deadline {
            h.render()
                .unwrap()
                .save(
                    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                        .join("../../target/exm014-ui-failure.png"),
                )
                .unwrap();
            panic!("HID UI deadline: {label}");
        }
        h.step();
        std::thread::sleep(Duration::from_millis(2));
    }
    h.run_steps(2);
}
fn advance_view(h: &mut Harness<'_, Shell>, mut ready: impl FnMut(&Harness<'_, Shell>) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while !ready(h) {
        assert!(Instant::now() < deadline, "HID view completion deadline");
        h.step();
        std::thread::sleep(Duration::from_millis(2));
    }
    h.run_steps(2);
}
#[test]
fn actual_shell_opens_and_sends_only_the_explicitly_selected_hid_collection() {
    let audit = Arc::new(Mutex::new(Audit::default()));
    let source = audit.clone();
    let worker = HidWorker::spawn(move || Fixture(source)).unwrap();
    let app = Shell::with_hid(
        None,
        HidPanel::with_worker(worker, "Audited synthetic HID source"),
    )
    .unwrap();
    let mut h = Harness::builder()
        .with_size(egui::vec2(1200., 760.))
        .build_ui_state(|ui, s: &mut Shell| s.show(ui), app);
    h.run_steps(3);
    h.get_by_label("HID reports").click();
    h.run_steps(3);
    assert!(audit.lock().unwrap().opens.is_empty());
    h.get_by_label("Discover HID").click();
    h.run_steps(4);
    advance_view(&mut h, |h| {
        !h.get_by_label("HID collection")
            .accesskit_node()
            .is_disabled()
    });
    assert!(audit.lock().unwrap().opens.is_empty());
    assert!(audit.lock().unwrap().writes.is_empty());
    h.get_by_label("HID collection").click();
    h.run_steps(3);
    h.get_by_label("Collection 1 | 1209:0002 | usage FF00:0001")
        .click();
    h.run_steps(3);
    h.get_by_label("Report format").click();
    h.run_steps(3);
    h.get_by_label("Numbered").click();
    h.run_steps(3);
    assert!(audit.lock().unwrap().opens.is_empty());
    h.get_by_label("Open selected HID").click();
    h.run_steps(3);
    advance(&mut h, "selected open", || {
        audit.lock().unwrap().selected == Some(1)
    });
    assert_eq!(audit.lock().unwrap().opens, [1]);
    assert!(audit.lock().unwrap().writes.is_empty());
    h.get_by_label("Output report HEX").focus();
    h.run_steps(1);
    h.get_by_label("Output report HEX").type_text("03 A5 00 2A");
    h.run_steps(3);
    h.get_by_label("Send HID report").click();
    h.run_steps(3);
    advance(&mut h, "report send", || {
        !audit.lock().unwrap().writes.is_empty()
    });
    assert_eq!(audit.lock().unwrap().writes, [(1, vec![3, 165, 0, 42])]);
    let root =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/exm014-visual");
    std::fs::create_dir_all(&root).unwrap();
    h.render()
        .unwrap()
        .save(root.join("hid-1200x760-dark.png"))
        .unwrap();
    // A fresh viewport fixes both native geometry and scale for this artifact.
    let app = h.into_state();
    let mut h = Harness::builder()
        .with_size(egui::vec2(900., 600.))
        .with_pixels_per_point(1.5)
        .build_ui_state(|ui, s: &mut Shell| s.show(ui), app);
    h.run_steps(3);
    let rendered = h.render().unwrap();
    assert_eq!(rendered.dimensions(), (1350, 900));
    rendered
        .save(root.join("hid-900x600-1.5-dark.png"))
        .unwrap();
    h.get_by_label("Output report HEX").focus();
    h.run_steps(1);
    h.get_by_label("Output report HEX")
        .type_text(&"00 ".repeat(MAX_REPORT + 1));
    h.run_steps(3);
    assert!(
        h.get_by_label("Send HID report")
            .accesskit_node()
            .is_disabled()
    );
    h.get_by_label("Send HID report").click();
    h.run_steps(2);
    assert_eq!(audit.lock().unwrap().writes.len(), 1);
    h.get_by_label("Close HID").click();
    h.run_steps(3);
    advance(&mut h, "selected close", || {
        audit.lock().unwrap().selected.is_none()
    });
    assert_eq!(audit.lock().unwrap().opens, [1]);
    assert_eq!(audit.lock().unwrap().writes.len(), 1);
    h.get_by_label("Light theme").click();
    h.run_steps(3);
    h.render()
        .unwrap()
        .save(root.join("hid-900x600-1.5-light-closed.png"))
        .unwrap();
    h.get_by_label("Mock capture").click();
    h.run_steps(3);
    h.get_by_label("Start capture").click();
    h.run_steps(3);
    assert!(h.state().status().requested);
    assert_eq!(audit.lock().unwrap().opens, [1]);
    assert_eq!(audit.lock().unwrap().writes.len(), 1);
    h.get_by_label("Stop capture").click();
    h.run_steps(2);
}
#[cfg(not(feature = "native-hid"))]
#[test]
fn default_native_hid_ui_is_explicitly_unsupported_without_discovering_hardware() {
    let mut h = Harness::builder()
        .with_size(egui::vec2(900., 600.))
        .build_ui_state(|ui, s: &mut Shell| s.show(ui), Shell::new(None).unwrap());
    h.run_steps(3);
    h.get_by_label("HID reports").click();
    h.run_steps(3);
    h.get_by_label("Discover HID").click();
    h.run_steps(4);
    advance_view(&mut h, |h| {
        h.query_by_label("HID: adapter capability is unsupported")
            .is_some()
    });
    assert!(
        h.query_by_label("HID: adapter capability is unsupported")
            .is_some()
    );
}
