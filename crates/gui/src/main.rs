#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1200., 760.])
            .with_min_inner_size([900., 600.]),
        persist_window: false,
        ..Default::default()
    };
    eframe::run_native(
        "Exchange Monitor",
        options,
        Box::new(|_| {
            Ok(Box::new(exmon_gui::Shell::new(
                exmon_app::shell::default_layout_path(),
            )?))
        }),
    )
}
