//! Bounded native Windows smoke, hidden viewport, synthetic traffic only.
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
    phase: u8,
    template: Option<u64>,
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
            self.shell.start_capture().expect("mock worker start");
            self.started = true;
        }
        if self.phase == 0 && !self.shell.incoming.status.groups.is_empty() {
            self.shell
                .incoming
                .view
                .select_tab(Some("rule:sample32".into()));
            if let Some(row) = self.shell.incoming.view.selected.clone() {
                self.shell.outgoing.copy_received(&row);
                self.template = self.shell.outgoing.selected;
                self.phase = 1;
            }
        }
        if let Some(id) = self.template {
            if self.phase == 1
                && self
                    .shell
                    .outgoing
                    .snapshot
                    .templates
                    .iter()
                    .any(|t| t.id == id)
            {
                self.shell
                    .outgoing
                    .editor
                    .edit(
                        id,
                        exmon_app::outgoing::Operation::Field {
                            index: 0,
                            text: "42".into(),
                            engineering: false,
                            label: false,
                        },
                    )
                    .expect("worker field edit");
                self.phase = 2;
            }
            if self.phase == 2
                && self
                    .shell
                    .outgoing
                    .snapshot
                    .templates
                    .iter()
                    .any(|t| t.id == id && t.bytes.get(2) == Some(&42) && !t.dirty)
            {
                self.shell
                    .outgoing
                    .connect_mock(1)
                    .expect("explicit mock bind");
                self.phase = 3;
            }
            if self.phase == 3 && self.shell.outgoing.editor.can_send(id) {
                self.shell
                    .outgoing
                    .editor
                    .send_once(id)
                    .expect("explicit mock send");
                self.phase = 4;
            }
        }
        self.shell.show(ui);
        if !self.shot
            && self.shell.status().packets >= 1024
            && self.phase == 4
            && self
                .shell
                .outgoing
                .snapshot
                .templates
                .iter()
                .any(|t| Some(t.id) == self.template && t.tx_count == 1)
        {
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
                                .join("../../target/exm027-native.png");
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
            self.shell.stop_capture();
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
        "Exchange Monitor smoke",
        options,
        Box::new(move |cc| {
            cc.egui_ctx
                .send_viewport_cmd(egui::ViewportCommand::RequestPaintWhileHidden);
            Ok(Box::new(Smoke {
                shell: Shell::new(Some(
                    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                        .join("../../target/exm027-native-layout.json"),
                ))?,
                started: false,
                phase: 0,
                template: None,
                shot: false,
                result: app_result,
                deadline: Instant::now() + Duration::from_secs(8),
            }))
        }),
    )?;
    if result.load(Ordering::Acquire) != 1 {
        return Err(
            std::io::Error::other("native screenshot/worker smoke did not complete").into(),
        );
    }
    println!(
        "PASS: native outgoing clone/edit/validate/bind/send rendered and closed; target/exm027-native.png"
    );
    Ok(())
}
