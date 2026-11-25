use eframe::egui::{self, RichText};
use exmon_app::sessions::{
    Controller, Entry, Event, ExportCategory, ExportFormat, ExportSelection, Limits, Snapshot,
    Source, State,
};
use std::sync::Arc;
pub struct Sessions {
    pub path: String,
    pub budget: String,
    pub max_records: String,
    pub annotation: String,
    pub snapshot: Arc<Snapshot>,
    pub selected: Option<Entry>,
    pub notice: Option<&'static str>,
    pub pending: bool,
    page_stack: Vec<u64>,
    waiting_revision: u64,
    exporting: bool,
    export: ExportForm,
    export_error: Option<&'static str>,
}

#[derive(Default)]
struct ExportForm {
    path: String,
    format: ExportFormat,
    category: ExportCategory,
    from_ns: String,
    through_ns: String,
    type_id: String,
    field: String,
}

impl ExportForm {
    fn selection(&self) -> Result<ExportSelection, &'static str> {
        fn bound(text: &str) -> Result<Option<u64>, &'static str> {
            let text = text.trim();
            if text.is_empty() {
                Ok(None)
            } else {
                text.parse()
                    .map(Some)
                    .map_err(|_| "Use whole nanoseconds for time bounds")
            }
        }
        Ok(ExportSelection {
            category: self.category,
            from_ns: bound(&self.from_ns)?,
            through_ns: bound(&self.through_ns)?,
            type_id: self.type_id.trim().into(),
            field: if self.category == ExportCategory::Observations {
                String::new()
            } else {
                self.field.trim().into()
            },
        })
    }
}
impl Default for Sessions {
    fn default() -> Self {
        Self {
            path: String::new(),
            budget: "64".into(),
            max_records: "100000".into(),
            annotation: String::new(),
            snapshot: Arc::new(Snapshot::default()),
            selected: None,
            notice: None,
            pending: false,
            page_stack: vec![0],
            waiting_revision: 0,
            exporting: false,
            export: ExportForm::default(),
            export_error: None,
        }
    }
}
impl Sessions {
    pub fn poll(&mut self, controller: &Controller) {
        if let Some(s) = controller.snapshot() {
            if s.recovery != self.snapshot.recovery {
                self.page_stack = vec![0];
                self.export_error = None;
            }
            if self.pending
                && s.revision != self.waiting_revision
                && (!self.exporting || !controller.busy())
            {
                self.pending = false;
                self.exporting = false;
                self.notice = None;
            }
            if s.state == State::Recovered
                && s.revision != self.snapshot.revision
                && self
                    .selected
                    .as_ref()
                    .is_none_or(|old| !s.entries.iter().any(|e| Arc::ptr_eq(&e.event, &old.event)))
            {
                self.selected = s.entries.first().cloned();
            }
            self.snapshot = s;
        }
    }
    fn requested(&mut self, result: Result<(), &'static str>) {
        match result {
            Ok(()) => {
                self.pending = true;
                self.waiting_revision = self.snapshot.revision;
                self.notice = None;
            }
            Err(e) => self.notice = Some(e),
        }
    }
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        controller: &Controller,
        config: Option<Arc<exmon_domain::config::ProjectConfig>>,
        charts: &[exmon_app::charts::Spec],
        incoming: &exmon_app::incoming::View,
    ) {
        egui::ScrollArea::vertical().id_salt("sessions-view").show(ui,|ui|{
            ui.heading("Session recording and recovery");
            ui.label("Raw capture data may be sensitive. Recording writes only to your chosen local file.");
            ui.label(RichText::new("Recovery never starts capture, applies definitions, opens a device or enables transmission.").small().weak());
            ui.horizontal_wrapped(|ui|{input(ui,"Session file path",&mut self.path,4096,380.);});
            ui.horizontal_wrapped(|ui|{
                input(ui,"Database MiB",&mut self.budget,3,65.);input(ui,"Record limit",&mut self.max_records,7,95.);
                if ui.add_enabled(!controller.busy()&&!self.pending,egui::Button::new("Start recording")).clicked(){
                    let limits=self.budget.parse::<u64>().ok().and_then(|n|n.checked_mul(1024*1024)).zip(self.max_records.parse::<u64>().ok()).map(|(bytes,records)|Limits{bytes,records});
                    if let (Some(limits),Some(config))=(limits,config.clone()){self.requested(controller.start(&self.path,limits,config,charts.to_vec()));}else{self.notice=Some("Choose valid limits and wait for processing definitions");}
                }
                // Stop admission is independent of both the data queue and pending UI commands.
                if ui.add_enabled(controller.requested(),egui::Button::new("Stop recording")).clicked(){controller.stop();self.pending=true;self.waiting_revision=self.snapshot.revision;}
                if ui.add_enabled(!controller.busy()&&!self.pending,egui::Button::new("Recover session")).clicked(){self.requested(controller.recover(&self.path));}
            });
            ui.label(RichText::new("New files only; existing recordings are never overwritten. Budget stops recording, independently of capture.").small().weak());
            ui.horizontal_wrapped(|ui|{
                ui.strong(format!("{:?}",self.snapshot.state));
                if self.pending{ui.label("Session worker preparing...");}
                if let Some(s)=&self.snapshot.summary {ui.label(format!("{} committed records / {} bytes | {} recording drops",s.records,s.bytes,self.snapshot.dropped.max(s.dropped)));}
            });
            if let Some(n)=self.notice.or(self.snapshot.notice){ui.label(n);}
            if let Some(source)=self.snapshot.last_loss.or_else(||self.snapshot.summary.as_ref().and_then(|s|s.last_loss)){ui.label(RichText::new(format!("Last known recording-loss boundary: generation {} / interface {} / epoch {} / sequence {}",source.generation,source.interface,source.epoch,source.sequence)).small().weak());}
            ui.label(RichText::new("Raw recording precedes display decimation. Derived samples cover processed messages; capture/backend losses are shown separately below.").small().weak());
            ui.horizontal_wrapped(|ui|{
                input(ui,"Session annotation",&mut self.annotation,4096,350.);
                if ui.add_enabled(controller.client().token()!=0,egui::Button::new("Add annotation")).clicked(){
                    let source=incoming.selected.as_ref().map(|r|Source{generation:r.key.generation,interface:r.interface,epoch:r.epoch,sequence:r.sequence});
                    self.notice=Some(match controller.note(source,&self.annotation){Ok(())=>{self.annotation.clear();"Annotation queued; wait for committed count"},Err(e)=>e});
                }
            });
            if self.snapshot.state==State::Recovered {
                self.show_export(ui, controller);
                ui.separator();ui.heading("Committed session records");
                ui.horizontal_wrapped(|ui|{
                    if ui.add_enabled(self.page_stack.len()>1&&!self.pending,egui::Button::new("Previous records")).clicked(){self.page_stack.pop();self.requested(controller.page(*self.page_stack.last().unwrap_or(&0)));}
                    if ui.add_enabled(self.snapshot.more&&!self.pending&&self.page_stack.len()<16384,egui::Button::new("Next records")).clicked() && let Some(last)=self.snapshot.entries.last(){let id=last.id;self.page_stack.push(id);self.requested(controller.page(id));}
                    ui.label(format!("Page {} / at most 64 records and 1 MiB",self.page_stack.len()));
                });
                ui.columns(2,|columns|{
                    egui::ScrollArea::vertical().id_salt("session-record-list").max_height(300.).show(&mut columns[0],|ui|{
                        for i in 0..self.snapshot.entries.len(){let entry=self.snapshot.entries[i].clone();if ui.selectable_label(self.selected.as_ref().is_some_and(|s|s.id==entry.id),format!("#{} {}",entry.id,entry.event.label())).clicked(){self.selected=Some(entry);}}
                    });
                    let ui=&mut columns[1];if let Some(e)=&self.selected {ui.heading(format!("#{} {}",e.id,e.event.label()));
                        egui::ScrollArea::both().id_salt("session-record-details").max_height(330.).show(ui,|ui|details(ui,&e.event));
                    }
                });
            }
        });
    }

    fn show_export(&mut self, ui: &mut egui::Ui, controller: &Controller) {
        ui.collapsing("Export session data", |ui| {
            ui.add_enabled_ui(!self.pending && !controller.busy(), |ui| {
                ui.horizontal_wrapped(|ui| {
                    input(ui, "New export file path", &mut self.export.path, 4096, 340.);
                });
                ui.horizontal_wrapped(|ui| {
                    ui.radio_value(&mut self.export.format, ExportFormat::Json, "JSON");
                    ui.radio_value(&mut self.export.format, ExportFormat::Csv, "CSV");
                });
                ui.horizontal_wrapped(|ui| {
                    ui.radio_value(&mut self.export.category, ExportCategory::Fields, "Decoded fields");
                    ui.radio_value(&mut self.export.category, ExportCategory::ChartSeries, "Chart series");
                    ui.radio_value(&mut self.export.category, ExportCategory::Observations, "Raw observations");
                });
                ui.horizontal_wrapped(|ui| {
                    input(ui, "From (ns)", &mut self.export.from_ns, 20, 128.);
                    input(ui, "Through (ns)", &mut self.export.through_ns, 20, 128.);
                });
                ui.horizontal_wrapped(|ui| {
                    let raw = self.export.category == ExportCategory::Observations;
                    let label = if raw { "Transport (optional)" } else { "Protocol ID (optional)" };
                    input(ui, label, &mut self.export.type_id, 128, 140.);
                    if !raw {
                        input(ui, "Field ID (optional)", &mut self.export.field, 128, 120.);
                    }
                });
                ui.label("Exports may contain sensitive data. Raw observations include payloads and transport metadata.");
                if ui.button("Save export").clicked() {
                    let result = self.export.selection().and_then(|selection| {
                        controller.export(self.snapshot.recovery, &self.export.path, self.export.format, selection)
                    });
                    self.export_error = result.err();
                    if self.export_error.is_none() {
                        self.exporting = true;
                        self.requested(Ok(()));
                    }
                }
            });
            if self.exporting {
                ui.label("Exporting session…");
                if ui.button("Cancel export").clicked() {
                    controller.cancel_export();
                }
            } else if let Some(message) = self.export_error.or(self.snapshot.export_notice) {
                ui.label(message);
            }
        });
    }
}
fn details(ui: &mut egui::Ui, event: &Event) {
    match event {
        Event::Raw(raw) => {
            source(ui, raw.source);
            ui.label(format!(
                "{} captured bytes / {:?} original",
                raw.bytes.len(),
                raw.original
            ));
            ui.label(format!(
                "Monotonic {} ns / source {:?} ns / {:?}",
                raw.timestamp_ns, raw.source_timestamp_ns, raw.direction
            ));
            hex(ui, &raw.bytes);
        }
        Event::Definitions {
            generation,
            revision,
            config,
        } => {
            ui.label(format!(
                "Generation {generation} / definition revision {revision} / {} protocols / {} chart definitions",
                config.protocols.len(),
                config.charts.len()
            ));
            for p in config.protocols.iter().take(16) {
                ui.label(format!("{}: {} / {} fields", p.id, p.name, p.fields.len()));
            }
            ui.label(format!(
                "{} automatic group names. Stored configuration stays inert.",
                config.grouping.names.len()
            ));
        }
        Event::Message {
            source: s,
            name,
            protocol,
            bytes,
            samples,
            ..
        } => {
            source(ui, *s);
            ui.label(format!("{protocol}: {name}"));
            for sample in samples.iter().take(16) {
                ui.label(format!(
                    "{}: {:?} {}",
                    sample.field, sample.value, sample.unit
                ));
            }
            hex(ui, bytes);
        }
        Event::Annotation { source: s, text } => {
            if let Some(s) = s {
                source(ui, *s);
            }
            ui.label(text);
        }
        Event::Charts { definitions } => {
            ui.label(format!(
                "{} saved chart specifications; sample values are stored with message records.",
                definitions.as_array().map_or(0, Vec::len)
            ));
        }
    }
}
fn source(ui: &mut egui::Ui, s: Source) {
    ui.label(format!(
        "Generation {} / interface {} / epoch {} / sequence {}",
        s.generation, s.interface, s.epoch, s.sequence
    ));
}
fn hex(ui: &mut egui::Ui, bytes: &[u8]) {
    for (i, row) in bytes.chunks(16).take(16).enumerate() {
        ui.monospace(format!(
            "{:04X}  {}",
            i * 16,
            row.iter()
                .map(|b| format!("{b:02X}"))
                .collect::<Vec<_>>()
                .join(" ")
        ));
    }
    if bytes.len() > 256 {
        ui.label("Preview: first 256 bytes. Full bytes remain in the recording.");
    }
}
fn input(ui: &mut egui::Ui, label: &str, value: &mut String, limit: usize, width: f32) {
    let l = ui.label(label);
    ui.add(
        egui::TextEdit::singleline(value)
            .char_limit(limit)
            .desired_width(width),
    )
    .labelled_by(l.id);
}
