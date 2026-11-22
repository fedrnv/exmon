//! HID controls use the same selected-device worker for native and mock sources.
use eframe::egui::{self, RichText};
use exmon_app::{
    hid_worker::{HidWorker, MAX_REPORT, ReportMode, Snapshot},
    shell::Layout,
};
use exmon_domain::observation::Observation;
use std::collections::VecDeque;
pub struct HidPanel {
    worker: HidWorker,
    state: Snapshot,
    selected: Option<u64>,
    mode: Option<ReportMode>,
    maximum: usize,
    hex: String,
    history: VecDeque<Observation>,
    discovering: bool,
    notice: Option<&'static str>,
    source: &'static str,
}
impl HidPanel {
    pub fn native() -> Result<Self, exmon_app::hid_worker::Error> {
        Ok(Self::with_worker(
            HidWorker::native()?,
            if cfg!(feature = "native-hid") {
                "Native USB HID"
            } else {
                "Native USB HID unavailable in this build"
            },
        ))
    }
    pub fn with_worker(worker: HidWorker, source: &'static str) -> Self {
        Self {
            worker,
            state: Snapshot::default(),
            selected: None,
            mode: None,
            maximum: 64,
            hex: String::new(),
            history: VecDeque::new(),
            discovering: false,
            notice: None,
            source,
        }
    }
    pub fn busy(&self) -> bool {
        self.state.open || self.worker.pending() || self.worker.snapshot().is_some_and(|s| s.open)
    }
    pub fn open(&self) -> bool {
        self.worker.snapshot().is_some_and(|s| s.open) && !self.worker.revoked()
    }
    pub fn received(&self) -> u64 {
        self.state.received
    }
    pub fn show(&mut self, ui: &mut egui::Ui, layout: &mut Layout) {
        if let Some(state) = self.worker.snapshot() {
            if state.enumeration != self.state.enumeration {
                self.selected = None;
                self.mode = None;
                self.hex.clear();
                self.history.clear();
                self.discovering = false;
            }
            if state.epoch != self.state.epoch {
                self.hex.clear();
                self.history.clear();
            }
            if state.open {
                self.selected = state.selected;
                self.mode = state.report_mode;
                self.maximum = state.capacity;
            }
            self.state = state;
        }
        for report in self.worker.drain(4) {
            if !self.state.open
                || self.worker.revoked()
                || Some(report.id) != self.state.selected
                || report.epoch != self.state.epoch
            {
                continue;
            }
            if self.history.len() == 256 {
                self.history.pop_front();
            }
            self.history.push_back(report.observation);
        }
        let blocked = self.busy() || self.discovering;
        egui::Panel::top("hid-controls").resizable(false).show(ui,|ui|{
            ui.label(RichText::new(self.source).weak());
            ui.horizontal_wrapped(|ui|{
                if ui.add_enabled(!blocked,egui::Button::new("Discover HID")).clicked(){
                    match self.worker.discover(){Ok(())=>{self.discovering=true;self.notice=None;},Err(_)=>self.notice=Some("HID command queue is busy.")}
                }
                let before=self.selected;
                ui.add_enabled_ui(!blocked,|ui|{
                    let selected=self.state.collections.iter().find(|c|Some(c.id)==self.selected).map(|c|format!("{}: {:04X}:{:04X}",c.id,c.device.vendor_id,c.device.product_id)).unwrap_or_else(||"Select collection".into());
                    egui::ComboBox::from_label("HID collection").selected_text(selected).show_ui(ui,|ui|{
                        for collection in &self.state.collections{let d=collection.device;
                            ui.selectable_value(&mut self.selected,Some(collection.id),format!("Collection {} | {:04X}:{:04X} | usage {:04X}:{:04X}",collection.id,d.vendor_id,d.product_id,d.usage_page,d.usage));
                        }
                    });
                    egui::ComboBox::from_label("Report format").selected_text(match self.mode{None=>"Choose format",Some(ReportMode::Numbered)=>"Numbered",Some(ReportMode::Unnumbered)=>"Unnumbered"}).show_ui(ui,|ui|{
                        ui.selectable_value(&mut self.mode,Some(ReportMode::Numbered),"Numbered");ui.selectable_value(&mut self.mode,Some(ReportMode::Unnumbered),"Unnumbered");
                    });
                    ui.add(egui::DragValue::new(&mut self.maximum).range(1..=MAX_REPORT).prefix("Capacity bytes: "));
                });
                if before!=self.selected{self.mode=None;self.hex.clear();self.history.clear();}
                let valid=self.selected.is_some_and(|id|self.state.collections.iter().any(|c|c.id==id)) && self.mode.is_some();
                if ui.add_enabled(!blocked && valid,egui::Button::new("Open selected HID")).clicked()
                    && let (Some(id),Some(mode))=(self.selected,self.mode){
                        self.notice=if self.worker.open_selected(id,self.state.enumeration,mode,self.maximum).is_err(){Some("HID open request rejected; refresh and select again.")}else{None};
                }
                if ui.add_enabled(self.busy(),egui::Button::new("Close HID")).clicked(){self.worker.close();self.hex.clear();}
            });
            ui.label(RichText::new("Discovery and selection do not start a report session. Open may interact with the selected device; use its known report format/capacity.").small().weak());
        });
        egui::Panel::bottom("hid-status")
            .resizable(false)
            .show(ui, |ui| {
                ui.label(format!(
                    "HID RX: {} | Display skipped: {} | Failures: {} | Backend loss: unknown",
                    self.state.received, self.state.dropped, self.state.failures
                ));
                if self.state.open && self.worker.revoked() {
                    ui.label("Closing; a report already submitted may still complete.");
                }
                if let Some(count) = self.state.last_sent {
                    ui.label(format!(
                        "Last output completion: {count} bytes accepted (delivery not confirmed)"
                    ));
                }
                if let Some(error) = self.state.error {
                    ui.label(format!("HID: {error}"));
                }
                if let Some(notice) = self.notice {
                    ui.label(notice);
                }
            });
        let width = ui.available_width();
        let left = egui::Panel::left("hid-incoming")
            .default_size(width * layout.split)
            .size_range(width * 0.25..=width * 0.75)
            .show(ui, |ui| {
                ui.heading("Incoming HID reports");
                if let Some(collection) = self
                    .state
                    .collections
                    .iter()
                    .find(|c| Some(c.id) == self.state.selected)
                {
                    let d = collection.device;
                    ui.label(format!(
                        "Selected {} | VID {:04X} PID {:04X} | Usage {:04X}:{:04X}",
                        collection.id, d.vendor_id, d.product_id, d.usage_page, d.usage
                    ));
                }
                ui.separator();
                if self.history.is_empty() {
                    ui.label("No HID reports. Discover, select and explicitly open a collection.");
                }
                egui::ScrollArea::vertical()
                    .id_salt("hid-report-history")
                    .auto_shrink([false, false])
                    .stick_to_bottom(true)
                    .show_rows(ui, 22., self.history.len(), |ui, rows| {
                        for index in rows {
                            if let Some(record) = self.history.get(index) {
                                let hex = record
                                    .bytes()
                                    .iter()
                                    .take(12)
                                    .map(|b| format!("{b:02X}"))
                                    .collect::<Vec<_>>()
                                    .join(" ");
                                let report_id = match record.metadata() {
                                    exmon_domain::observation::Metadata::Hid {
                                        report_id, ..
                                    } => *report_id,
                                    _ => 0,
                                };
                                ui.monospace(format!(
                                    "RX id {report_id:02X}  {hex}{}",
                                    if record.bytes().len() > 12 {
                                        " ..."
                                    } else {
                                        ""
                                    }
                                ));
                            }
                        }
                    });
                ui.take_available_space();
            });
        if width > 0. {
            layout.split = (left.response.rect.width() / width).clamp(0.25, 0.75);
        }
        egui::CentralPanel::default().show(ui,|ui|{
            ui.heading("Outgoing HID report");ui.label("Include the report ID once; use 00 for an unnumbered output report.");
            let label=ui.label("Output report HEX");
            egui::ScrollArea::vertical().id_salt("hid-output-editor").max_height(140.).auto_shrink([false,false]).show(ui,|ui|{
                ui.add(egui::TextEdit::multiline(&mut self.hex).desired_rows(4).char_limit(MAX_REPORT*3)).labelled_by(label.id);
            });
            // A saturated text limit may be a truncated paste, never send that prefix.
            let saturated=self.hex.len()>=MAX_REPORT*3;
            if saturated{ui.label("Output input reached its limit; shorten and review it before sending.");}
            let ready=self.open() && self.selected==self.state.selected && !self.hex.trim().is_empty() && !saturated;
            if ui.add_enabled(ready,egui::Button::new("Send HID report")).clicked()
                && let Some(id)=self.selected{self.notice=if self.worker.send_selected(id,self.state.epoch,self.hex.clone()).is_err(){Some("HID report rejected; check the active selection and retry explicitly.")}else{None};
            }
            ui.label(RichText::new("One explicit report. No schedule, auto retry or transmission on discovery/reconnect.").small().weak());
        });
    }
}
