use eframe::egui::{self, Color32, RichText};
use exmon_app::shell::{
    CaptureStatus, CaptureWorker, Captured, Layout, PreferencesWorker, ShellError, Theme,
};
use exmon_domain::observation::TransportKind;
use std::{collections::VecDeque, path::PathBuf, time::Duration};
pub const TRANSPORTS: [(TransportKind, &str); 6] = [
    (TransportKind::Ethernet, "Ethernet"),
    (TransportKind::Tcp, "TCP"),
    (TransportKind::Udp, "UDP"),
    (TransportKind::Serial, "Serial"),
    (TransportKind::Usb, "USB"),
    (TransportKind::Can, "CAN"),
];
pub struct Shell {
    network_filter: crate::network_filters::FilterPanel,
    hid: crate::hid::HidPanel,
    hid_view: bool,
    capture: CaptureWorker,
    preferences: PreferencesWorker,
    pub layout: Layout,
    selected: usize,
    interface: u64,
    history: VecDeque<Captured>,
    notice: Option<&'static str>,
    loaded: bool,
    dirty: bool,
    evicted: u64,
    layout_generation: u64,
}
impl Shell {
    pub fn new(path: Option<PathBuf>) -> Result<Self, ShellError> {
        Self::with_hid(
            path,
            crate::hid::HidPanel::native().map_err(|_| ShellError::Unavailable)?,
        )
    }
    pub fn with_hid(path: Option<PathBuf>, hid: crate::hid::HidPanel) -> Result<Self, ShellError> {
        Ok(Self {
            network_filter: Default::default(),
            hid,
            hid_view: false,
            capture: CaptureWorker::spawn()?,
            preferences: PreferencesWorker::spawn(path)?,
            layout: Layout::default(),
            selected: 0,
            interface: 1,
            history: VecDeque::new(),
            notice: None,
            loaded: false,
            dirty: false,
            evicted: 0,
            layout_generation: 0,
        })
    }
    pub fn status(&self) -> CaptureStatus {
        self.capture.status()
    }
    pub fn retained(&self) -> usize {
        self.history.len()
    }
    pub fn transport(&self) -> TransportKind {
        TRANSPORTS[self.selected].0
    }
    pub fn start_capture(&mut self) -> Result<(), ShellError> {
        if self.hid_view || self.hid.busy() {
            return Err(ShellError::Busy);
        }
        let settings = if crate::network_filters::network(self.transport()) {
            self.network_filter
                .settings(self.interface)
                .map_err(|_| ShellError::Invalid)?
        } else {
            Default::default()
        };
        self.capture
            .start_filtered(self.transport(), self.interface, settings)?;
        self.history.clear();
        self.evicted = 0;
        self.notice = None;
        Ok(())
    }
    pub fn stop_capture(&mut self) {
        self.capture.stop();
    }
    pub fn show(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let mut restoring_size = false;
        if let Some(result) = self.preferences.load_result() {
            self.loaded = true;
            match result {
                Ok(Some(layout)) => {
                    self.layout = layout;
                    self.layout_generation += 1;
                    restoring_size = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                        layout.width,
                        layout.height,
                    )));
                }
                Ok(None) => {}
                Err(_) => self.notice = Some("Layout could not be loaded; using defaults."),
            }
        }
        ctx.set_theme(match self.layout.theme {
            Theme::Dark => egui::Theme::Dark,
            Theme::Light => egui::Theme::Light,
        });
        let status = self.capture.status();
        for batch in self.capture.drain(4) {
            if !status.requested || batch.generation != self.capture.generation() {
                continue;
            }
            for record in batch.records {
                if self.history.len() == 1024 {
                    self.history.pop_front();
                    self.evicted = self.evicted.saturating_add(1);
                }
                self.history.push_back(record);
            }
        }
        let before = self.layout;
        let size = ctx.input(|i| {
            i.viewport()
                .inner_rect
                .map_or(ui.max_rect().size(), |r| r.size())
        });
        if !restoring_size && size.x >= 900. && size.y >= 600. {
            self.layout.width = size.x.min(3840.);
            self.layout.height = size.y.min(2160.);
        }
        let selection_before = (self.selected, self.interface);
        let live = status.requested || self.hid.open();
        let filter_valid = !crate::network_filters::network(self.transport())
            || self.network_filter.settings(self.interface).is_ok();
        egui::Panel::top("app-header")
            .resizable(false)
            .show(ui, |ui| {
                ui.add_space(8.);
                ui.horizontal(|ui| {
                    ui.heading("Exchange Monitor");
                    ui.label(
                        RichText::new("VIRTUAL PROTOTYPE")
                            .small()
                            .color(Color32::from_rgb(97, 162, 187)),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add_enabled(
                                !status.requested && !status.running && !self.hid.busy(),
                                egui::Button::new(if self.hid_view {
                                    "Mock capture"
                                } else {
                                    "HID reports"
                                }),
                            )
                            .clicked()
                        {
                            self.hid_view = !self.hid_view;
                        }
                        let title = if self.layout.theme == Theme::Dark {
                            "Light theme"
                        } else {
                            "Dark theme"
                        };
                        if ui.button(title).clicked() {
                            self.layout.theme = if self.layout.theme == Theme::Dark {
                                Theme::Light
                            } else {
                                Theme::Dark
                            };
                        }
                        ui.label(
                            RichText::new(if live { "CAPTURING" } else { "IDLE" }).color(if live {
                                Color32::from_rgb(65, 185, 140)
                            } else {
                                Color32::GRAY
                            }),
                        );
                    });
                });
                ui.add_space(8.);
                ui.separator();
                if !self.hid_view {
                    ui.horizontal_wrapped(|ui| {
                        ui.add_enabled_ui(!status.requested && !status.running, |ui| {
                            egui::ComboBox::from_label("Transport")
                                .selected_text(TRANSPORTS[self.selected].1)
                                .width(100.)
                                .show_ui(ui, |ui| {
                                    for (i, (_, name)) in TRANSPORTS.iter().enumerate() {
                                        ui.selectable_value(&mut self.selected, i, *name);
                                    }
                                });
                            egui::ComboBox::from_label("Interface")
                                .selected_text(format!("Mock {:02}", self.interface))
                                .width(110.)
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut self.interface, 1, "Mock 01");
                                    ui.selectable_value(&mut self.interface, 2, "Mock 02");
                                });
                        });
                        if ui
                            .add_enabled(
                                !status.requested && !status.running && filter_valid,
                                egui::Button::new("Start capture"),
                            )
                            .clicked()
                            && self.start_capture().is_err()
                        {
                            self.notice = Some("Capture worker is busy or unavailable.");
                        }
                        if ui
                            .add_enabled(
                                status.requested || status.running,
                                egui::Button::new("Stop capture"),
                            )
                            .clicked()
                        {
                            self.stop_capture();
                        }
                        if ui.button("Clear view").clicked() {
                            self.history.clear();
                            self.evicted = 0;
                        }
                    });
                    let kind = self.transport();
                    self.network_filter.show(
                        ui,
                        self.interface,
                        kind,
                        status,
                        self.capture.effective_filter(),
                    );
                }
                ui.add_space(6.);
            });
        if selection_before != (self.selected, self.interface) {
            self.history.clear();
            self.evicted = 0;
        }
        if self.hid_view {
            self.hid.show(ui, &mut self.layout);
        } else {
            egui::Panel::bottom("app-status")
                .resizable(false)
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(format!(
                            "RX {} observations / {} bytes",
                            status.packets, status.bytes
                        ));
                        ui.separator();
                        ui.label(format!("Display decimation: {}", status.display_decimation));
                        ui.label(format!(
                            "Backend loss: {} | Application drops: {}",
                            status.backend_loss, status.application_drops
                        ));
                        ui.separator();
                        ui.label(format!("History evictions: {}", self.evicted));
                        ui.label(format!("Filtered out: {}", status.filtered_out));
                    });
                    if status.failed {
                        ui.colored_label(
                            Color32::from_rgb(230, 140, 90),
                            "Capture stopped after a worker failure.",
                        );
                    }
                    if let Some(notice) = self.notice {
                        ui.label(notice);
                    }
                    if self.preferences.failed() {
                        ui.label("Layout saving unavailable; current session remains usable.");
                    }
                });
            let width = ui.available_width();
            let left = egui::Panel::left(("incoming", self.layout_generation))
                .resizable(true)
                .default_size(width * self.layout.split)
                .size_range(width * 0.25..=width * 0.75)
                .show(ui, |ui| {
                    ui.heading("Incoming");
                    ui.label(
                        RichText::new(format!(
                            "{} / Mock {:02}",
                            TRANSPORTS[self.selected].1, self.interface
                        ))
                        .weak(),
                    );
                    ui.separator();
                    if self.history.is_empty() {
                        ui.add_space(40.);
                        ui.label("No incoming observations");
                        ui.label(
                            RichText::new("Start a mock interface to inspect traffic.").weak(),
                        );
                    } else {
                        ui.label(
                            RichText::new("SEQ        DIRECTION     BYTES (HEX)")
                                .monospace()
                                .weak(),
                        );
                        egui::ScrollArea::vertical()
                            .id_salt("raw-observations")
                            .auto_shrink([false, false])
                            .stick_to_bottom(true)
                            .show_rows(ui, 22., self.history.len(), |ui, rows| {
                                for index in rows {
                                    if let Some(record) = self.history.get(index) {
                                        let bytes = record.observation.bytes();
                                        let hex = bytes
                                            .iter()
                                            .take(12)
                                            .map(|b| format!("{b:02X}"))
                                            .collect::<Vec<_>>()
                                            .join(" ");
                                        ui.label(
                                            RichText::new(format!(
                                                "{:08}   RX   {}{}",
                                                record.sequence,
                                                hex,
                                                if bytes.len() > 12 { " ..." } else { "" }
                                            ))
                                            .monospace(),
                                        );
                                    }
                                }
                            });
                    }
                    ui.take_available_space();
                });
            if width > 0. {
                self.layout.split = (left.response.rect.width() / width).clamp(0.25, 0.75);
            }
            egui::CentralPanel::default().show(ui, |ui| {
                ui.heading("Outgoing");
                ui.label(RichText::new("Transmission disabled").weak());
                ui.separator();
                ui.add_space(40.);
                ui.label("No outgoing messages");
                ui.label("Message templates and send controls will appear here when configured.");
                ui.add_space(12.);
                ui.label(
                    RichText::new("Starting capture never sends data.")
                        .small()
                        .weak(),
                );
            });
        }
        self.dirty |= before != self.layout;
        if self.loaded && self.dirty && self.preferences.save(self.layout).is_ok() {
            self.dirty = false;
        }
        ctx.request_repaint_after(Duration::from_millis(
            if status.requested || status.running || self.hid_view {
                16
            } else {
                100
            },
        ));
    }
}
impl eframe::App for Shell {
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        self.show(ui);
    }
}
