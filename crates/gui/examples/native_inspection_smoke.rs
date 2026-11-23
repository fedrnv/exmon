//! Hidden native Windows inspection smoke. Synthetic bytes only; no device access.
use exmon_gui::{Shell, egui};
use std::{
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    },
    time::{Duration, Instant},
};
struct Smoke {
    shell: Shell,
    started: bool,
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
        if !self.started {
            self.shell.packet_view = true;
            self.shell.inspection.request(
                &self.shell.incoming.inspection,
                exmon_app::inspection::Action::Sample,
            );
            self.started = true;
        }
        // Poll first so the screenshot renders the selected range in this frame.
        self.shell
            .inspection
            .poll(&self.shell.incoming.inspection, ui.ctx());
        let ready = !self.shell.inspection.pending && self.shell.inspection.selected.is_some();
        if ready {
            self.shell.inspection.choose(34..42);
        }
        self.shell.show(ui);
        if !self.shot && ready {
            self.shot = true;
            let result = self.result.clone();
            let ctx = ui.ctx().clone();
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::Screenshot(
                    egui::ScreenshotCallback::new(move |image| {
                        std::thread::spawn(move || {
                            let bytes: Vec<_> =
                                image.pixels.iter().flat_map(|p| p.to_array()).collect();
                            let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                                .join("../../target/exm030-native.png");
                            let saved = image::save_buffer(
                                path,
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
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let result = Arc::new(AtomicU8::new(0));
    let app_result = result.clone();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_visible(false)
            .with_inner_size([1360., 900.]),
        persist_window: false,
        ..Default::default()
    };
    eframe::run_native(
        "Exchange Monitor packet smoke",
        options,
        Box::new(move |cc| {
            cc.egui_ctx
                .send_viewport_cmd(egui::ViewportCommand::RequestPaintWhileHidden);
            Ok(Box::new(Smoke {
                shell: Shell::new(None)?,
                started: false,
                shot: false,
                result: app_result,
                deadline: Instant::now() + Duration::from_secs(8),
            }))
        }),
    )?;
    if result.load(Ordering::Acquire) != 1 {
        return Err(std::io::Error::other("Native inspection screenshot did not complete").into());
    }
    println!(
        "PASS: native synthetic packet inspection rendered and closed; target/exm030-native.png"
    );
    Ok(())
}
