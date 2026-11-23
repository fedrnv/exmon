//! Hidden native synthetic recording/stop/recovery smoke, no physical interfaces.
use exmon_app::sessions::{Event, Limits, State};
use exmon_gui::{Shell, egui};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    },
    time::{Duration, Instant},
};
struct Smoke {
    shell: Shell,
    phase: u8,
    result: Arc<AtomicU8>,
    deadline: Instant,
    path: PathBuf,
}
impl eframe::App for Smoke {
    fn logic(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaintWhileHidden);
        ctx.request_repaint_after(Duration::from_millis(16));
    }
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        self.shell.session_view = true;
        if self.phase == 0
            && let Some(config) = self.shell.incoming.worker.configuration()
        {
            self.shell
                .recorder
                .start(
                    self.path.to_str().unwrap(),
                    Limits::default(),
                    config,
                    self.shell.charts.specs.clone(),
                )
                .expect("explicit synthetic recording");
            self.shell
                .start_capture()
                .expect("explicit synthetic capture");
            self.phase = 1;
        }
        if self.phase == 1
            && self.shell.sessions.snapshot.state == State::Recording
            && self
                .shell
                .sessions
                .snapshot
                .summary
                .as_ref()
                .is_some_and(|s| s.records > 10)
        {
            self.shell.stop_capture();
            self.shell
                .recorder
                .note(None, "Native synthetic session annotation")
                .expect("explicit note");
            self.shell.recorder.stop();
            self.phase = 2;
        }
        if self.phase == 2
            && !self.shell.recorder.busy()
            && self
                .shell
                .sessions
                .snapshot
                .summary
                .as_ref()
                .is_some_and(|s| s.closed)
        {
            self.shell
                .recorder
                .recover(self.path.to_str().unwrap())
                .expect("explicit recovery");
            self.phase = 3;
        }
        let ready = self.phase == 3
            && self.shell.sessions.snapshot.state == State::Recovered
            && self
                .shell
                .sessions
                .snapshot
                .entries
                .iter()
                .any(|e| matches!(e.event.as_ref(), Event::Raw(_)));
        if ready {
            self.shell.sessions.selected = self
                .shell
                .sessions
                .snapshot
                .entries
                .iter()
                .find(|e| matches!(e.event.as_ref(), Event::Raw(_)))
                .cloned();
        }
        self.shell.show(ui);
        if ready {
            self.phase = 4;
            let result = self.result.clone();
            let ctx = ui.ctx().clone();
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::Screenshot(
                    egui::ScreenshotCallback::new(move |image| {
                        std::thread::spawn(move || {
                            let bytes: Vec<_> =
                                image.pixels.iter().flat_map(|p| p.to_array()).collect();
                            let output = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                                .join("../../target/exm031-native.png");
                            let saved = image::save_buffer(
                                output,
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
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../target/exm031-native-{}-{}.sqlite",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_visible(false)
            .with_inner_size([1360., 900.]),
        persist_window: false,
        ..Default::default()
    };
    eframe::run_native(
        "Exchange Monitor session smoke",
        options,
        Box::new(move |cc| {
            cc.egui_ctx
                .send_viewport_cmd(egui::ViewportCommand::RequestPaintWhileHidden);
            Ok(Box::new(Smoke {
                shell: Shell::new(None)?,
                phase: 0,
                result: app_result,
                deadline: Instant::now() + Duration::from_secs(8),
                path,
            }))
        }),
    )?;
    if result.load(Ordering::Acquire) != 1 {
        return Err(std::io::Error::other(
            "Native session recording/recovery screenshot did not complete",
        )
        .into());
    }
    println!(
        "PASS: native synthetic recording/stop/recovery rendered and closed; target/exm031-native.png"
    );
    Ok(())
}
