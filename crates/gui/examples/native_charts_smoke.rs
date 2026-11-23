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
        self.shell.chart_view = true;
        if self.phase == 0 && self.shell.incoming.worker.configuration().is_some() {
            self.shell.charts.poll(&self.shell.incoming.charts);
            let mut spec = self.shell.charts.specs[0].clone();
            spec.series = vec![
                exmon_app::charts::Selection {
                    protocol: "sample32".into(),
                    field: "sequence".into(),
                    label: "Sequence".into(),
                    unit: "count".into(),
                },
                exmon_app::charts::Selection {
                    protocol: "sample32".into(),
                    field: "interface".into(),
                    label: "Interface".into(),
                    unit: "id".into(),
                },
            ];
            self.shell.charts.specs[0] = spec.clone();
            self.shell
                .incoming
                .charts
                .configure(vec![spec])
                .expect("explicit chart configuration");
            self.phase = 1;
        }
        self.shell.show(ui);
        if !self.shot
            && self
                .shell
                .charts
                .snapshot
                .charts
                .first()
                .is_some_and(|c| c.series.len() == 2 && c.series[0].retained > 512)
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
                                .join("../../target/exm028-native.png");
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
                        .join("../../target/exm028-native-layout.json"),
                ))?,
                started: false,
                phase: 0,
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
        "PASS: native chart capture/series/average rendered and closed; target/exm028-native.png"
    );
    Ok(())
}
