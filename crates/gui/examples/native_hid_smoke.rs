//! Native renderer acceptance with a hardcoded synthetic HID source, never hardware.
use exmon_app::{hid_worker::*, shell::Layout};
use exmon_gui::{egui, hid::HidPanel};
use std::{
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    },
    time::{Duration, Instant},
};
struct Fixture(Option<usize>);
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
        assert_eq!(id, 1);
        self.0 = Some(id);
        Ok(())
    }
    fn read(&mut self, bytes: &mut [u8], _: i32) -> Result<usize, Error> {
        assert_eq!(self.0, Some(1));
        bytes[..4].copy_from_slice(&[3, 165, 0, 42]);
        Ok(4)
    }
    fn write(&mut self, bytes: &[u8]) -> Result<usize, Error> {
        assert_eq!(self.0, Some(1));
        assert_eq!(bytes, [3, 165, 0, 42]);
        Ok(bytes.len())
    }
    fn close(&mut self) {
        self.0 = None;
    }
}
struct Smoke {
    panel: HidPanel,
    layout: Layout,
    shot: bool,
    result: Arc<AtomicU8>,
    deadline: Instant,
}
impl eframe::App for Smoke {
    fn logic(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaintWhileHidden);
        ctx.request_repaint_after(Duration::from_millis(16));
    }
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        self.panel.show(ui, &mut self.layout);
        if !self.shot && self.panel.received() >= 16 {
            self.shot = true;
            let result = self.result.clone();
            let ctx = ui.ctx().clone();
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::Screenshot(
                    egui::ScreenshotCallback::new(move |image| {
                        std::thread::spawn(move || {
                            let bytes: Vec<_> =
                                image.pixels.iter().flat_map(|p| p.to_array()).collect();
                            let saved = image::save_buffer(
                                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                                    .join("../../target/exm014-native.png"),
                                &bytes,
                                image.width() as u32,
                                image.height() as u32,
                                image::ColorType::Rgba8,
                            )
                            .is_ok();
                            result.store(if saved { 1 } else { 2 }, Ordering::Release);
                            ctx.request_repaint();
                        });
                    }),
                ));
        }
        if self.result.load(Ordering::Acquire) != 0 || Instant::now() >= self.deadline {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}
fn until(mut ready: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while !ready() {
        assert!(Instant::now() < deadline, "synthetic worker setup timeout");
        std::thread::sleep(Duration::from_millis(2));
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // All setup is explicit synthetic test traffic, before the GUI event loop.
    let worker =
        HidWorker::spawn(|| Fixture(None)).map_err(|e| std::io::Error::other(e.to_string()))?;
    worker.discover().unwrap();
    until(|| !worker.pending() && worker.snapshot().is_some_and(|s| s.collections.len() == 2));
    worker
        .open_selected(
            1,
            worker.snapshot().unwrap().enumeration,
            ReportMode::Numbered,
            64,
        )
        .unwrap();
    until(|| worker.snapshot().is_some_and(|s| s.open));
    worker
        .send_selected(1, worker.snapshot().unwrap().epoch, "03 A5 00 2A".into())
        .unwrap();
    until(|| worker.snapshot().is_some_and(|s| s.last_sent == Some(4)));
    let panel = HidPanel::with_worker(worker, "Synthetic HID native renderer test (no hardware)");
    let result = Arc::new(AtomicU8::new(0));
    let app_result = result.clone();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_visible(false)
            .with_inner_size([1200., 760.]),
        persist_window: false,
        ..Default::default()
    };
    eframe::run_native(
        "HID synthetic smoke",
        options,
        Box::new(move |cc| {
            cc.egui_ctx
                .send_viewport_cmd(egui::ViewportCommand::RequestPaintWhileHidden);
            Ok(Box::new(Smoke {
                panel,
                layout: Layout::default(),
                shot: false,
                result: app_result,
                deadline: Instant::now() + Duration::from_secs(8),
            }))
        }),
    )?;
    if result.load(Ordering::Acquire) != 1 {
        return Err(std::io::Error::other("native HID view did not render").into());
    }
    println!(
        "PASS: native HID view rendered selected synthetic RX/TX and closed; target/exm014-native.png"
    );
    Ok(())
}
