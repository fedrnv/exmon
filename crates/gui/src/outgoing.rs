use eframe::egui::{self, RichText};
use exmon_app::{
    outgoing::{Create, Editor, Error, Operation, ScheduleInput, Snapshot, Template},
    scheduler::{Drift, Missed},
};
use exmon_domain::config::ProjectConfig;
use std::{collections::BTreeMap, sync::Arc};
struct Inputs {
    hex: String,
    header: String,
    field: usize,
    value: String,
    engineering: bool,
    label: bool,
    revision: u64,
    dirty: bool,
    dirty_part: u8,
    fields_mode: bool,
    schedule: ScheduleInput,
}
impl Inputs {
    fn new(t: &Template) -> Self {
        Self {
            hex: t.hex.to_string(),
            header: t.header_hex.to_string(),
            field: 0,
            value: t.fields.first().map(|f| f.raw.clone()).unwrap_or_default(),
            engineering: false,
            label: false,
            revision: t.revision,
            dirty: false,
            dirty_part: 0,
            fields_mode: false,
            schedule: ScheduleInput::default(),
        }
    }
    fn sync(&mut self, t: &Template) {
        self.hex = t.hex.to_string();
        self.header = t.header_hex.to_string();
        self.revision = t.revision;
        self.value = t
            .fields
            .get(self.field)
            .map(|f| {
                if self.engineering {
                    f.engineering.clone()
                } else {
                    f.raw.clone()
                }
            })
            .unwrap_or_default();
        self.dirty = false;
        self.dirty_part = 0;
    }
}
pub struct Outgoing {
    pub editor: Editor,
    pub snapshot: Snapshot,
    pub selected: Option<u64>,
    inputs: BTreeMap<u64, Inputs>,
    permission: bool,
    interface: u64,
    protocol: usize,
    pub notice: Option<Error>,
}
impl Outgoing {
    pub fn new() -> Result<Self, Error> {
        Ok(Self {
            editor: Editor::spawn()?,
            snapshot: Snapshot::default(),
            selected: None,
            inputs: BTreeMap::new(),
            permission: false,
            interface: 1,
            protocol: 0,
            notice: None,
        })
    }
    pub fn poll(&mut self) {
        if let Some(s) = self.editor.snapshot() {
            self.snapshot = s;
        }
        for t in &self.snapshot.templates {
            let input = self.inputs.entry(t.id).or_insert_with(|| Inputs::new(t));
            if t.revision != input.revision && !t.dirty {
                input.sync(t);
            }
        }
    }
    pub fn copy_received(&mut self, row: &Arc<exmon_app::incoming::Row>) {
        match self.editor.clone_received(row) {
            Ok(id) => {
                self.selected = Some(id);
                self.notice = None;
            }
            Err(error) => self.notice = Some(error),
        }
    }
    pub fn disconnect(&mut self) {
        self.editor.disconnect();
        self.permission = false;
    }
    /// Explicit synthetic connection action, also used by native integration smoke.
    pub fn connect_mock(&mut self, interface: u64) -> Result<(), Error> {
        self.interface = interface;
        self.permission = true;
        self.editor.bind_mock(interface, true)
    }
    pub fn active(&self) -> bool {
        self.snapshot.preparing
            || self.snapshot.tx.as_ref().is_some_and(|s| {
                s.schedules.iter().any(|s| {
                    matches!(
                        s.state,
                        exmon_app::scheduler::State::Enabled
                            | exmon_app::scheduler::State::InFlight
                    )
                })
            })
    }
    pub fn show(&mut self, ui: &mut egui::Ui, config: Option<Arc<ProjectConfig>>) {
        self.poll();
        egui::ScrollArea::vertical()
            .id_salt("outgoing-pane")
            .auto_shrink([false, false])
            .show(ui, |ui| self.contents(ui, config));
    }
    fn contents(&mut self, ui: &mut egui::Ui, config: Option<Arc<ProjectConfig>>) {
        ui.heading("Outgoing");
        ui.horizontal_wrapped(|ui| {
            if ui.button("New raw message").clicked() {
                match self.editor.create(Create::raw()) {
                    Ok(id) => {
                        self.selected = Some(id);
                        self.notice = None;
                    }
                    Err(e) => self.notice = Some(e),
                }
            }
            if let Some(config) = &config
                && !config.protocols.is_empty()
            {
                self.protocol = self.protocol.min(config.protocols.len() - 1);
                egui::ComboBox::from_label("Protocol definition")
                    .selected_text(format!(
                        "Protocol: {}",
                        config.protocols[self.protocol].name
                    ))
                    .show_ui(ui, |ui| {
                        for (index, p) in config.protocols.iter().enumerate() {
                            ui.selectable_value(&mut self.protocol, index, &p.name);
                        }
                    });
                if ui.button("New from protocol").clicked() {
                    let p = &config.protocols[self.protocol];
                    let mut value = Create::raw();
                    value.name = p.name.clone();
                    value.protocol = Some(Arc::new(p.clone()));
                    match self.editor.create(value) {
                        Ok(id) => self.selected = Some(id),
                        Err(e) => self.notice = Some(e),
                    }
                }
            }
        });
        ui.horizontal_wrapped(|ui| {
            let old = self.interface;
            egui::ComboBox::from_label("TX interface")
                .selected_text(format!("Mock TX {:02}", self.interface))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.interface, 1, "Mock TX 01");
                    ui.selectable_value(&mut self.interface, 2, "Mock TX 02");
                });
            if old != self.interface {
                self.disconnect();
            }
            if ui
                .checkbox(&mut self.permission, "Allow mock transmission")
                .changed()
                && !self.permission
            {
                self.editor.disconnect();
            }
            if ui
                .add_enabled(self.permission, egui::Button::new("Connect mock TX"))
                .clicked()
            {
                self.notice = self.editor.bind_mock(self.interface, self.permission).err();
            }
            if ui.button("Disconnect TX").clicked() {
                self.disconnect();
            }
        });
        if self.snapshot.connected {
            ui.label(format!(
                "Bound interface: Mock TX {:02}",
                self.snapshot
                    .binding
                    .map_or(self.interface, |b| b.interface)
            ));
        } else {
            ui.label(RichText::new("Transmission disabled").weak());
        }
        if self.snapshot.preparing {
            ui.label("Preparing templates; transmission is blocked.");
        }
        if self.snapshot.failed {
            ui.colored_label(
                egui::Color32::from_rgb(220, 130, 70),
                "Outgoing worker unavailable; transmission stopped.",
            );
        }
        if let Some(error) = self.notice.or(self.snapshot.error) {
            ui.colored_label(egui::Color32::from_rgb(220, 130, 70), error_text(error));
        }
        ui.separator();
        egui::ScrollArea::horizontal()
            .id_salt("outgoing-tabs")
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    for t in &self.snapshot.templates {
                        if ui
                            .selectable_label(
                                self.selected == Some(t.id),
                                format!("{} #{}", t.name, t.id),
                            )
                            .clicked()
                        {
                            self.selected = Some(t.id);
                        }
                    }
                });
            });
        let selected = self
            .selected
            .or_else(|| self.snapshot.templates.first().map(|t| t.id));
        self.selected = selected;
        let Some(t) = self
            .snapshot
            .templates
            .iter()
            .find(|t| Some(t.id) == selected)
            .cloned()
        else {
            ui.label("No outgoing messages");
            ui.label("Create a template or copy a format from incoming traffic.");
            return;
        };
        let input = self.inputs.entry(t.id).or_insert_with(|| Inputs::new(&t));
        let status = if input.dirty || t.dirty {
            "Edits not validated"
        } else if t.valid {
            "Validated"
        } else {
            "Validation failed"
        };
        ui.label(format!(
            "{status} | {} bytes | TX count: {}",
            t.bytes.len(),
            t.tx_count
        ));
        if let Some(e) = t.error {
            ui.colored_label(egui::Color32::from_rgb(220, 130, 70), error_text(e));
        }
        if let Some(source) = t.source {
            ui.label(format!(
                "Copied from session {} / interface {} / instance #{}",
                source.session, source.interface, source.observation
            ));
            ui.label(format!(
                "Received quality: {} | source revision {}",
                t.source_quality
                    .map(|q| format!("{q:?}"))
                    .unwrap_or_else(|| "Unknown".into()),
                t.source_revision
                    .map(|r| r.to_string())
                    .unwrap_or_else(|| "Unknown".into())
            ));
        }
        if let Some(last) = &t.last {
            ui.label(result_text(last, t.last_requested));
        }
        if self.snapshot.history_incomplete {
            ui.label("TX history incomplete; displayed count is an observed minimum.");
        }
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    !input.dirty && self.editor.can_send(t.id),
                    egui::Button::new("Send once"),
                )
                .clicked()
            {
                self.notice = self.editor.send_once(t.id).err();
            }
            if ui.button("Validate current data").clicked() {
                self.notice = self.editor.edit(t.id, Operation::Validate).err();
            }
            if ui
                .add_enabled(
                    !input.dirty && t.protocol.is_some(),
                    egui::Button::new("Repair checksums"),
                )
                .clicked()
            {
                self.notice = self.editor.edit(t.id, Operation::Repair).err();
            }
        });
        ui.scope(|ui| {
            ui.horizontal(|ui| {
                ui.add_enabled_ui(!input.dirty, |ui| {
                    ui.selectable_value(&mut input.fields_mode, false, "HEX editor");
                    ui.selectable_value(&mut input.fields_mode, true, "Field editor");
                });
                if input.dirty {
                    ui.label("Apply the edit or validate current data to discard text changes.");
                }
            });
            if !input.fields_mode {
                let label = ui.label("Message HEX");
                egui::ScrollArea::vertical()
                    .id_salt("message-hex-text")
                    .max_height(110.)
                    .show(ui, |ui| {
                        if ui
                            .add_enabled(
                                input.dirty_part != 3,
                                egui::TextEdit::multiline(&mut input.hex)
                                    .font(egui::TextStyle::Monospace)
                                    .desired_rows(3)
                                    .desired_width(f32::INFINITY)
                                    .char_limit(3 * 65536),
                            )
                            .labelled_by(label.id)
                            .changed()
                        {
                            input.dirty = true;
                            input.dirty_part = 1;
                            self.notice = self.editor.invalidate(t.id).err();
                        }
                    });
                if ui
                    .add_enabled(input.dirty_part != 3, egui::Button::new("Apply HEX"))
                    .clicked()
                {
                    self.notice = self
                        .editor
                        .edit(t.id, Operation::Hex(input.hex.clone()))
                        .err();
                }
            } else if t.fields.is_empty() {
                ui.label("No structured fields; use HEX or copy a configured message.");
            } else {
                input.field = input.field.min(t.fields.len() - 1);
                let before = (input.field, input.engineering, input.label);
                ui.add_enabled_ui(!input.dirty, |ui| {
                    egui::ComboBox::from_label("Field")
                        .selected_text(&t.fields[input.field].name)
                        .show_ui(ui, |ui| {
                            for (i, f) in t.fields.iter().enumerate() {
                                ui.selectable_value(&mut input.field, i, &f.name);
                            }
                        });
                    ui.horizontal_wrapped(|ui| {
                        ui.checkbox(&mut input.engineering, "Engineering value");
                        ui.checkbox(&mut input.label, "Enum label");
                    });
                });
                if before != (input.field, input.engineering, input.label) {
                    input.value = if input.engineering {
                        t.fields[input.field].engineering.clone()
                    } else {
                        t.fields[input.field].raw.clone()
                    };
                }
                let f = &t.fields[input.field];
                ui.label(format!("{} = {}", f.name, f.display));
                let label = ui.label("Field value");
                if ui
                    .add_enabled(
                        input.dirty_part != 3,
                        egui::TextEdit::singleline(&mut input.value)
                            .desired_width(240.)
                            .char_limit(3 * 65536),
                    )
                    .labelled_by(label.id)
                    .changed()
                {
                    input.dirty = true;
                    input.dirty_part = 2;
                    self.notice = self.editor.invalidate(t.id).err();
                }
                if ui
                    .add_enabled(
                        f.editable && input.dirty_part != 3,
                        egui::Button::new("Apply field"),
                    )
                    .clicked()
                {
                    self.notice = self
                        .editor
                        .edit(
                            t.id,
                            Operation::Field {
                                index: input.field,
                                text: input.value.clone(),
                                engineering: input.engineering,
                                label: input.label,
                            },
                        )
                        .err();
                }
            }
            if t.protocol.is_some() {
                egui::CollapsingHeader::new("Preserved framing header").show(ui, |ui| {
                    let label = ui.label("Header HEX");
                    if ui
                        .add_enabled(
                            input.dirty_part == 0 || input.dirty_part == 3,
                            egui::TextEdit::multiline(&mut input.header)
                                .desired_rows(2)
                                .char_limit(3 * 65536),
                        )
                        .labelled_by(label.id)
                        .changed()
                    {
                        input.dirty = true;
                        input.dirty_part = 3;
                        self.notice = self.editor.invalidate(t.id).err();
                    }
                    if ui
                        .add_enabled(
                            input.dirty_part == 0 || input.dirty_part == 3,
                            egui::Button::new("Apply header"),
                        )
                        .clicked()
                    {
                        self.notice = self
                            .editor
                            .edit(t.id, Operation::Header(input.header.clone()))
                            .err();
                    }
                });
            }
            egui::CollapsingHeader::new("Periodic transmission")
                .default_open(true)
                .show(ui, |ui| {
                    let mut changed = false;
                    ui.horizontal_wrapped(|ui| {
                        let l = ui.label("Interval ms");
                        changed |= ui
                            .add(
                                egui::TextEdit::singleline(&mut input.schedule.interval)
                                    .desired_width(75.)
                                    .char_limit(20),
                            )
                            .labelled_by(l.id)
                            .changed();
                        let l = ui.label("Count (empty = unlimited)");
                        changed |= ui
                            .add(
                                egui::TextEdit::singleline(&mut input.schedule.count)
                                    .desired_width(75.)
                                    .char_limit(20),
                            )
                            .labelled_by(l.id)
                            .changed();
                    });
                    ui.horizontal_wrapped(|ui| {
                        let l = ui.label("Max lateness ms (optional)");
                        changed |= ui
                            .add(
                                egui::TextEdit::singleline(&mut input.schedule.lateness)
                                    .desired_width(75.)
                                    .char_limit(20),
                            )
                            .labelled_by(l.id)
                            .changed();
                        let old = (input.schedule.drift, input.schedule.missed);
                        egui::ComboBox::from_label("Drift")
                            .selected_text(format!("{:?}", input.schedule.drift))
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut input.schedule.drift,
                                    Drift::FixedRate,
                                    "Fixed rate",
                                );
                                ui.selectable_value(
                                    &mut input.schedule.drift,
                                    Drift::FixedDelay,
                                    "Fixed delay",
                                );
                            });
                        egui::ComboBox::from_label("Missed deadline")
                            .selected_text(format!("{:?}", input.schedule.missed))
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut input.schedule.missed,
                                    Missed::Skip,
                                    "Skip",
                                );
                                ui.selectable_value(
                                    &mut input.schedule.missed,
                                    Missed::Stop,
                                    "Stop",
                                );
                            });
                        changed |= old != (input.schedule.drift, input.schedule.missed);
                    });
                    if changed {
                        self.editor.stop_schedule(t.id);
                    }
                    ui.horizontal_wrapped(|ui| {
                        if ui
                            .add_enabled(
                                !input.dirty && self.editor.can_send(t.id),
                                egui::Button::new("Start schedule"),
                            )
                            .clicked()
                        {
                            self.notice = self
                                .editor
                                .start_schedule(t.id, input.schedule.clone())
                                .err();
                        }
                        if ui.button("Stop schedule").clicked() {
                            self.editor.stop_schedule(t.id);
                        }
                    });
                    if let Some(s) = self
                        .snapshot
                        .tx
                        .as_ref()
                        .and_then(|s| s.schedules.iter().find(|s| s.id == t.id))
                    {
                        ui.label(format!(
                            "Schedule: {:?} | attempts {} / missed {}",
                            s.state, s.stats.attempts, s.stats.missed
                        ));
                    } else {
                        ui.label("Schedule: Disabled");
                    }
                });
        });
    }
}
fn error_text(error: Error) -> &'static str {
    match error {
        Error::Busy => "Editor is busy; apply again.",
        Error::Invalid => "Invalid HEX, number or setting. Transmission is blocked.",
        Error::Codec => "Edit rejected: check field type, bounds or checksum ownership.",
        Error::Validation => "Field, checksum or framing validation failed.",
        Error::Dirty => "Apply and validate the pending edit before sending.",
        Error::NotReady => "Connect the chosen TX interface and validate the template.",
        Error::Capacity => "Template or editor memory limit reached.",
        Error::Exhausted => "Identity limit reached; transmission stopped.",
        Error::Unknown => "Template is no longer available.",
        Error::Unavailable => "Background worker unavailable.",
        Error::Transport(_) => "TX preparation failed; check permission, target and template.",
    }
}
fn result_text(outcome: &exmon_app::transmission::Outcome, requested: Option<usize>) -> String {
    use exmon_app::transmission::Outcome;
    match outcome {
        Outcome::Pending => "Last TX: pending".into(),
        Outcome::Complete { accepted, status } => format!(
            "Last TX: {status:?}, accepted {} / {} bytes",
            accepted
                .map(|n| n.to_string())
                .unwrap_or_else(|| "Unknown".into()),
            requested
                .map(|n| n.to_string())
                .unwrap_or_else(|| "Unknown".into())
        ),
        Outcome::Failed { error, accepted } => format!(
            "Last TX: {error:?}, accepted {}",
            accepted
                .map(|n| n.to_string())
                .unwrap_or_else(|| "Unknown".into())
        ),
        Outcome::Rejected(_) => "Last TX: rejected before submission".into(),
    }
}
