//! Structured settings editor; parsing, compilation, preview and files belong to app worker.
use eframe::egui::{self, Color32};
use exmon_app::{
    incoming::View,
    settings::{Action, Controller, Document, Snapshot},
    shell::Captured,
};
use std::{collections::BTreeMap, sync::Arc};
pub struct Settings {
    pub controller: Controller,
    pub snapshot: Arc<Snapshot>,
    pub notice: Option<String>,
    buffers: BTreeMap<String, (String, u64)>,
    last: u64,
    page: usize,
    document: Document,
    pub advanced: bool,
    pub preview_only: bool,
    project: String,
    bindings: String,
    advanced_dirty: bool,
    path: String,
    file_document: Document,
    index: usize,
    preview_row: usize,
    preview_field: usize,
}
impl Settings {
    pub fn new() -> Result<Self, std::io::Error> {
        Ok(Self {
            controller: Controller::new()?,
            snapshot: Arc::new(Snapshot::default()),
            notice: None,
            buffers: BTreeMap::new(),
            last: u64::MAX,
            page: 0,
            document: Document::Project,
            advanced: false,
            preview_only: false,
            project: String::new(),
            bindings: String::new(),
            advanced_dirty: false,
            path: String::new(),
            file_document: Document::Project,
            index: 0,
            preview_row: 0,
            preview_field: 0,
        })
    }
    pub fn send(&mut self, action: Action) {
        self.notice = self
            .controller
            .request(action)
            .err()
            .map(|e| format!("{}: {}", e.path, e.message));
    }
    pub fn poll(&mut self) {
        if let Some(s) = self.controller.snapshot() {
            if s.revision != self.last {
                self.last = s.revision;
                self.document = s.document;
                self.page = s.page;
                if !self.advanced_dirty {
                    self.project = s.project.to_string();
                    self.bindings = s.bindings.to_string();
                }
                self.buffers.retain(|path, (_, revision)| {
                    *revision > s.revision || s.invalid_fields.contains(path)
                });
            }
            if s.preview_revision != self.snapshot.preview_revision {
                self.preview_row = s.preview.iter().position(|r| !r.raw()).unwrap_or(0);
                self.preview_field = 0;
            }
            self.snapshot = s;
        }
    }
    pub fn show(&mut self, ui: &mut egui::Ui, view: &View) {
        self.poll();
        self.toolbar(ui, view);
        egui::ScrollArea::vertical()
            .id_salt("settings-scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| self.contents(ui));
    }
    fn toolbar(&mut self, ui: &mut egui::Ui, view: &View) {
        ui.heading("Validated settings");
        ui.label("Portable profiles and local device bindings are separate. Apply stops capture and outgoing transmission; reconnect explicitly afterward.");
        let active = self.controller.active().map_or(0, |c| c.revision);
        ui.label(format!(
            "Active revision {active} | Draft request {}",
            self.snapshot.revision
        ));
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(!self.advanced_dirty, egui::Button::new("Validate settings"))
                .clicked()
            {
                self.send(Action::Validate);
            }
            if ui
                .add_enabled(!self.advanced_dirty, egui::Button::new("Apply settings"))
                .clicked()
            {
                self.send(Action::Apply);
            }
            if ui.button("Cancel settings changes").clicked() {
                self.advanced_dirty = false;
                self.send(Action::Cancel);
            }
            if ui.button("Restore safe defaults").clicked() {
                self.advanced_dirty = false;
                self.send(Action::Defaults);
            }
            if ui
                .add_enabled(
                    !self.advanced_dirty,
                    egui::Button::new("Preview selected captures"),
                )
                .clicked()
            {
                self.preview_only = true;
                self.send(Action::Preview(selected_records(view)));
            }
        });
        if let Some(e) = &self.snapshot.issue {
            ui.colored_label(
                Color32::from_rgb(235, 135, 80),
                format!("{}: {}", e.path, e.message),
            );
        }
        if let Some(message) = &self.snapshot.notice {
            ui.label(*message);
        }
        if let Some(message) = &self.notice {
            ui.colored_label(Color32::from_rgb(235, 135, 80), message);
        }
        if self.snapshot.failed {
            ui.colored_label(
                Color32::RED,
                "Settings worker unavailable; active configuration retained",
            );
        }
    }
    fn contents(&mut self, ui: &mut egui::Ui) {
        let mut selected = self.snapshot.selected.clone();
        egui::ComboBox::from_label("Processing interface profile")
            .selected_text(selected.as_deref().unwrap_or("No selected profile"))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut selected, None, "No selected profile");
                for id in &self.snapshot.interfaces {
                    ui.selectable_value(&mut selected, Some(id.clone()), id);
                }
            });
        if selected != self.snapshot.selected {
            self.send(Action::Select(selected));
        }
        ui.checkbox(&mut self.preview_only, "Show preview results");
        if !self.preview_only {
            ui.checkbox(&mut self.advanced, "Advanced JSON editor");
            if self.advanced {
                ui.label("Complete schema-one documents. Stage JSON before validation/apply. Imported selectors never connect devices.");
                let label = ui.label("Portable project JSON");
                let changed = ui
                    .add(
                        egui::TextEdit::multiline(&mut self.project)
                            .code_editor()
                            .desired_rows(8)
                            .char_limit(1_048_576)
                            .desired_width(f32::INFINITY),
                    )
                    .labelled_by(label.id)
                    .changed();
                let label = ui.label("Machine bindings JSON");
                let changed = changed
                    | ui.add(
                        egui::TextEdit::multiline(&mut self.bindings)
                            .code_editor()
                            .desired_rows(4)
                            .char_limit(1_048_576)
                            .desired_width(f32::INFINITY),
                    )
                    .labelled_by(label.id)
                    .changed();
                if changed {
                    self.advanced_dirty = true;
                    if let Err(e) = self.controller.invalidate() {
                        self.notice = Some(e.message);
                    }
                }
                if ui.button("Stage JSON documents").clicked() {
                    self.send(Action::Text(Document::Project, self.project.clone()));
                    self.send(Action::Text(Document::Bindings, self.bindings.clone()));
                    self.advanced_dirty = false;
                }
            } else {
                ui.add_enabled_ui(!self.advanced_dirty, |ui| self.forms(ui));
                if self.advanced_dirty {
                    ui.label(
                        "Unstaged JSON changes: return to Advanced JSON and stage them, or Cancel.",
                    );
                }
            }
            ui.separator();
            ui.collapsing("Import and export configuration",|ui|{
            ui.label("Project exports may include template bytes. Choose a trusted destination. Local selectors are exported only with Machine bindings. Existing files are never overwritten.");
            ui.horizontal_wrapped(|ui|{ui.selectable_value(&mut self.file_document,Document::Project,"Portable project file");ui.selectable_value(&mut self.file_document,Document::Bindings,"Machine bindings file");ui.selectable_value(&mut self.file_document,Document::Safety,"Runtime safety file");});
            let label=ui.label("Configuration file path");ui.add(egui::TextEdit::singleline(&mut self.path).char_limit(4096).desired_width(ui.available_width())).labelled_by(label.id);
            ui.horizontal_wrapped(|ui|{if ui.add_enabled(!self.advanced_dirty,egui::Button::new("Import into draft")).clicked(){self.send(Action::Import(self.file_document,self.path.clone()));}
if ui.add_enabled(!self.advanced_dirty,egui::Button::new("Export draft to new file")).clicked(){self.send(Action::Export(self.file_document,self.path.clone()));}});
        });
        }
        if let Some(revision) = self.snapshot.preview_revision {
            ui.separator();
            ui.heading("Draft preview");
            ui.label(format!("Request {revision}; {} prepared rows, {} skipped. Uses retained raw chunks; missing earlier stream context stays incomplete.",self.snapshot.preview.len(),self.snapshot.preview_skipped));
            let count = self.snapshot.preview.len();
            if count > 0 {
                self.preview_row = self.preview_row.min(count - 1);
                ui.horizontal_wrapped(|ui| {
                    ui.label("Preview row");
                    ui.add(egui::DragValue::new(&mut self.preview_row).range(0..=count - 1));
                });
                let row = &self.snapshot.preview[self.preview_row];
                if self.preview_field >= row.fields.len() {
                    self.preview_field = 0;
                }
                ui.label(format!(
                    "{} | {} | {} bytes | {}",
                    row.name,
                    if row.raw() { "raw" } else { "normalized" },
                    row.bytes().len(),
                    row.status
                ));
                ui.label(format!(
                    "Source interface {} / epoch {} / sequence {}",
                    row.interface, row.epoch, row.sequence
                ));
                ui.label(format!(
                    "HEX {}",
                    row.bytes()
                        .iter()
                        .take(64)
                        .map(|b| format!("{b:02X} "))
                        .collect::<String>()
                ));
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add_enabled(
                            self.preview_field > 0,
                            egui::Button::new("Previous preview fields"),
                        )
                        .clicked()
                    {
                        self.preview_field = self.preview_field.saturating_sub(12);
                    }
                    if ui
                        .add_enabled(
                            self.preview_field + 12 < row.fields.len(),
                            egui::Button::new("Next preview fields"),
                        )
                        .clicked()
                    {
                        self.preview_field += 12;
                    }
                });
                for f in row.fields.iter().skip(self.preview_field).take(12) {
                    ui.label(format!(
                        "{} = {}{}",
                        f.name,
                        f.value,
                        if f.invalid { " (invalid)" } else { "" }
                    ));
                }
            }
        }
    }
    fn forms(&mut self, ui: &mut egui::Ui) {
        let before = self.document;
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(
                &mut self.document,
                Document::Project,
                "Protocols and interfaces",
            );
            ui.selectable_value(&mut self.document, Document::Bindings, "Local bindings");
            ui.selectable_value(
                &mut self.document,
                Document::Safety,
                "Capture safety limits",
            );
        });
        if before != self.document {
            self.page = 0;
            self.send(Action::Page(self.document, 0));
        }
        ui.horizontal_wrapped(|ui| {
            ui.label("Protocol/interface index for additions");
            ui.add(egui::DragValue::new(&mut self.index).range(0..=255));
            for (label, kind) in [
                ("Add protocol", "protocol"),
                ("Add interface", "interface"),
                ("Add field", "field"),
                ("Add local binding", "binding"),
            ] {
                if ui.button(label).clicked() {
                    self.send(Action::Add(kind, self.index));
                }
            }
        });
        ui.label("Each labelled path is an editable field. Changes remain drafts. JSON values null / [] / {} can be edited directly; the advanced editor supports full structural changes.");
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(self.page > 0, egui::Button::new("Previous settings fields"))
                .clicked()
            {
                self.page -= 1;
                self.send(Action::Page(self.document, self.page));
            }
            ui.label(format!(
                "Page {} / {} (64 fields per page)",
                self.page + 1,
                self.snapshot.total.div_ceil(64).max(1)
            ));
            if ui
                .add_enabled(
                    (self.page + 1) * 64 < self.snapshot.total,
                    egui::Button::new("Next settings fields"),
                )
                .clicked()
            {
                self.page += 1;
                self.send(Action::Page(self.document, self.page));
            }
        });
        let leaves = self.snapshot.leaves.clone();
        for leaf in leaves {
            ui.push_id((&leaf.pointer, leaf.document as u8), |ui| {
                let path = format!("/{}{}", leaf.document.label(), leaf.pointer);
                let mut text = self
                    .buffers
                    .get(&path)
                    .map(|(text, _)| text.clone())
                    .unwrap_or_else(|| leaf.value.clone());
                let mut changed = false;
                ui.horizontal_wrapped(|ui| {
                    let label = ui.label(&path);
                    if leaf.truncated {
                        ui.label("Large value: use advanced JSON editor");
                    } else if !leaf.options.is_empty() {
                        egui::ComboBox::from_id_salt("choice")
                            .selected_text(&text)
                            .show_ui(ui, |ui| {
                                for option in &leaf.options {
                                    changed |= ui
                                        .selectable_value(&mut text, (*option).into(), *option)
                                        .changed();
                                }
                            })
                            .response
                            .labelled_by(label.id);
                    } else if !leaf.string && (text == "true" || text == "false") {
                        let mut value = text == "true";
                        if ui
                            .checkbox(&mut value, "Enabled")
                            .labelled_by(label.id)
                            .changed()
                        {
                            text = value.to_string();
                            changed = true;
                        }
                    } else {
                        changed = ui
                            .add(
                                egui::TextEdit::singleline(&mut text)
                                    .char_limit(4096)
                                    .desired_width(220.),
                            )
                            .labelled_by(label.id)
                            .changed();
                    }
                });
                if changed {
                    match self.controller.request(Action::Patch(
                        leaf.document,
                        leaf.pointer.clone(),
                        text.clone(),
                    )) {
                        Ok(revision) => {
                            self.buffers.insert(path, (text, revision));
                        }
                        Err(e) => self.notice = Some(format!("{}: {}", e.path, e.message)),
                    }
                }
            });
        }
    }
}
/// Bounded actual raw observations, ordered and preserving original source coordinates.
pub fn selected_records(view: &View) -> Vec<Captured> {
    let Some(selected) = &view.selected else {
        return vec![];
    };
    let mut rows = Vec::new();
    let mut bytes = 0;
    for row in view.rows.iter().rev() {
        if row.raw()
            && row.key.generation == selected.key.generation
            && row.interface == selected.interface
            && row.epoch == selected.epoch
            && row.sequence <= selected.sequence
        {
            let n = row.observation.bytes().len();
            if rows.len() == 32 || bytes + n > 65536 {
                break;
            }
            bytes += n;
            rows.push(Captured {
                interface_id: row.interface,
                epoch: row.epoch,
                sequence: row.sequence,
                observation: (*row.observation).clone(),
            });
        }
    }
    rows.reverse();
    if rows.is_empty() {
        rows.push(Captured {
            interface_id: selected.interface,
            epoch: selected.epoch,
            sequence: selected.sequence,
            observation: (*selected.observation).clone(),
        });
    }
    rows
}
