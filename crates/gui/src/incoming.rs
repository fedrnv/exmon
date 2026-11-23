use eframe::egui::{self, RichText};
use exmon_app::incoming::{self, Draft, Status, View, Worker};
pub struct Incoming {
    pub worker: Worker,
    pub view: View,
    pub status: Status,
    pub draft: Option<Draft>,
    rename: String,
    notice: Option<&'static str>,
}
impl Incoming {
    pub fn new() -> Result<Self, incoming::Error> {
        Ok(Self {
            worker: Worker::prototype()?,
            view: View::default(),
            status: Status::default(),
            draft: None,
            rename: String::new(),
            notice: None,
        })
    }
    pub fn begin(&mut self, generation: u64) {
        self.view.reset(generation);
        self.status = Status::default();
        let _ = self.worker.begin(generation);
    }
    pub fn poll(&mut self) {
        for batch in self.worker.drain() {
            self.view.apply(batch);
        }
        if let Some(s) = self.worker.status()
            && (s.generation == self.view.generation || (s.failed && !s.ready))
        {
            self.status = s;
        }
    }
    pub fn show(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("Incoming");
            ui.checkbox(&mut self.view.paused, "Pause display");
        });
        egui::ScrollArea::horizontal()
            .id_salt("incoming-tabs")
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if ui
                        .selectable_label(self.view.tab.is_none(), "Raw traffic")
                        .clicked()
                    {
                        self.view.select_tab(None);
                    }
                    for group in &self.status.groups {
                        if ui
                            .selectable_label(
                                self.view.tab.as_ref() == Some(&group.key),
                                &group.name,
                            )
                            .on_hover_text(format!(
                                "{} messages / {} bytes",
                                group.count, group.bytes
                            ))
                            .clicked()
                        {
                            self.view.select_tab(Some(group.key.clone()));
                            self.rename = group.name.clone();
                        }
                    }
                });
            });
        if let Some(key) = &self.view.tab {
            if let Some(g) = self.status.groups.iter().find(|g| &g.key == key) {
                ui.label(format!(
                    "{} messages / {} bytes / {} msg/s",
                    g.count,
                    g.bytes,
                    g.per_second
                        .map(|v| format!("{v:.1}"))
                        .unwrap_or_else(|| "Unavailable".into())
                ));
            }
            ui.horizontal(|ui| {
                let name_label = ui.label("Type name");
                ui.add(
                    egui::TextEdit::singleline(&mut self.rename)
                        .hint_text("Type name")
                        .char_limit(128)
                        .desired_width(150.),
                )
                .labelled_by(name_label.id);
                if ui.button("Rename type").clicked() {
                    self.notice = Some(if self.worker.rename(key, &self.rename).is_ok() {
                        "Name change requested."
                    } else {
                        "Name must be 1..128 bytes without control characters."
                    });
                }
            });
        } else {
            ui.label(format!("{} observations processed", self.status.processed));
        }
        if self.status.failed {
            ui.colored_label(
                egui::Color32::from_rgb(220, 130, 70),
                if self.status.ready {
                    "Incoming processing error; some records could not be inspected."
                } else {
                    "Incoming definitions unavailable; capture remains independent."
                },
            );
        }
        if self.status.rename_error {
            ui.label("Name change rejected; previous configuration retained.");
        }
        if let Some(n) = self.notice {
            ui.label(n);
        }
        ui.label(
            RichText::new(format!(
                "Input drops: {} | Processing skips: {} | View skips: {}",
                self.status.input_dropped,
                self.status.processing_skipped,
                self.status.output_decimation
            ))
            .small()
            .weak(),
        );
        ui.label(
            RichText::new(format!(
                "History evictions: {} | Paused skips: {} | Group overflow: {}",
                self.view.evicted, self.view.paused_skipped, self.status.group_overflow
            ))
            .small()
            .weak(),
        );
        ui.separator();
        let rows = self.view.visible();
        if rows.is_empty() {
            ui.label("No incoming observations");
            ui.label(RichText::new("Start a mock interface to inspect traffic.").weak());
        }
        ui.label(
            RichText::new("TIME (s)     SEQ     DIR     PROTOCOL / BYTES")
                .monospace()
                .weak(),
        );
        let table_height = (ui.available_height() * 0.35).clamp(65., 220.);
        egui::ScrollArea::vertical()
            .id_salt("incoming-instances")
            .max_height(table_height)
            .auto_shrink([false, false])
            .show_rows(ui, 22., rows.len(), |ui, range| {
                for index in range {
                    let row = &rows[index];
                    let selected = self
                        .view
                        .selected
                        .as_ref()
                        .is_some_and(|s| s.key == row.key);
                    let label = format!(
                        "{:.3}   #{}   {:?}   {} / {} B",
                        row.observation.timestamp_ns() as f64 / 1e9,
                        row.sequence,
                        row.observation.direction(),
                        if row.raw() { "Raw" } else { &row.name },
                        row.bytes().len()
                    );
                    if ui
                        .add(
                            egui::Button::selectable(
                                selected,
                                RichText::new(label.clone()).monospace(),
                            )
                            .truncate(),
                        )
                        .on_hover_text(label)
                        .clicked()
                    {
                        self.view.select(row.key);
                    }
                }
            });
        ui.separator();
        egui::ScrollArea::vertical()
            .id_salt("incoming-details")
            .show(ui, |ui| {
                if let Some(row) = self.view.selected.clone() {
                    ui.horizontal(|ui| {
                        ui.strong(format!("Instance #{} / {}", row.sequence, row.name));
                        if ui
                            .add_enabled(!row.raw(), egui::Button::new("Copy format to outgoing"))
                            .clicked()
                        {
                            self.draft = row.copy_format();
                        }
                    });
                    if self.view.selected_evicted() {
                        ui.label(
                            RichText::new("Selected instance pinned after history eviction").weak(),
                        );
                    }
                    ui.label(format!(
                        "{} | {} | revision {}",
                        row.status,
                        row.quality
                            .map(|q| format!("{q:?}"))
                            .unwrap_or_else(|| "Raw observation".into()),
                        row.revision
                    ));
                    egui::CollapsingHeader::new("Source and capture details")
                        .default_open(ui.available_width() >= 550.)
                        .show(ui, |ui| {
                            ui.label(format!("Interface {} / epoch {}", row.interface, row.epoch));
                            ui.label(format!("Source: {}", row.source));
                            ui.label(format!("Destination: {}", row.destination));
                            ui.label(format!(
                                "Captured {} B / original {} | normalized {}",
                                row.observation.bytes().len(),
                                row.observation
                                    .original_length()
                                    .map(|n| format!("{n} B"))
                                    .unwrap_or_else(|| "Unknown".into()),
                                row.normalized
                                    .as_ref()
                                    .map(|b| format!("{} B", b.len()))
                                    .unwrap_or_else(|| "Not a message".into())
                            ));
                        });
                    let bytes = row.bytes();
                    let columns = if ui.available_width() < 550. { 8 } else { 16 };
                    ui.label(
                        RichText::new("OFFSET   HEX / ASCII")
                            .small()
                            .monospace()
                            .weak(),
                    );
                    egui::ScrollArea::both()
                        .id_salt("incoming-hex")
                        .max_height(110.)
                        .auto_shrink([false, true])
                        .show_rows(ui, 18., bytes.len().div_ceil(columns), |ui, range| {
                            for index in range {
                                let block = &bytes
                                    [index * columns..((index + 1) * columns).min(bytes.len())];
                                let hex = block
                                    .iter()
                                    .map(|b| format!("{b:02X} "))
                                    .collect::<String>();
                                let ascii = block
                                    .iter()
                                    .map(|b| {
                                        if b.is_ascii_graphic() || *b == b' ' {
                                            char::from(*b)
                                        } else {
                                            '.'
                                        }
                                    })
                                    .collect::<String>();
                                ui.monospace(format!(
                                    "{:04X}   {hex:width$} {ascii}",
                                    index * columns,
                                    width = columns * 3
                                ));
                            }
                        });
                    ui.label(RichText::new("Parsed fields").strong());
                    if row.fields.is_empty() {
                        ui.label("No configured fields for this instance");
                    }
                    egui::ScrollArea::vertical()
                        .id_salt("incoming-fields")
                        .max_height(130.)
                        .show_rows(ui, 20., row.fields.len(), |ui, range| {
                            for index in range {
                                let f = &row.fields[index];
                                let text = format!(
                                    "{} = {}{}",
                                    f.name,
                                    f.value,
                                    if f.invalid { " [invalid]" } else { "" }
                                );
                                ui.add(egui::Label::new(&text).truncate())
                                    .on_hover_text(text);
                            }
                        });
                } else {
                    ui.label("Select an instance to inspect HEX, ASCII and parsed fields.");
                }
            });
    }
    pub fn outgoing(&self, ui: &mut egui::Ui) {
        ui.heading("Outgoing");
        ui.label(RichText::new("Transmission disabled").weak());
        ui.separator();
        if let Some(draft) = &self.draft {
            ui.heading("Received format copy");
            ui.label(format!(
                "{} / {} bytes",
                draft.row.name,
                draft.row.bytes().len()
            ));
            ui.label(format!(
                "From interface {} / instance #{}",
                draft.row.interface, draft.row.sequence
            ));
            ui.label("Immutable received bytes and field definitions copied.");
            ui.label("No target or transmission permission is assigned.");
            ui.monospace(
                draft
                    .row
                    .bytes()
                    .iter()
                    .take(64)
                    .map(|b| format!("{b:02X} "))
                    .collect::<String>(),
            );
            if let Some(protocol) = &draft.row.protocol {
                egui::ScrollArea::vertical()
                    .id_salt("copied-fields")
                    .max_height(220.)
                    .show_rows(ui, 20., protocol.fields.len(), |ui, range| {
                        for i in range {
                            let field = &protocol.fields[i];
                            ui.label(format!(
                                "{}: offset {} / {} bytes",
                                field.id, field.offset, field.length
                            ));
                        }
                    });
            }
        } else {
            ui.add_space(40.);
            ui.label("No outgoing messages");
            ui.label("Select a classified instance and copy its format here.");
        }
        ui.label(
            RichText::new("Starting capture never sends data.")
                .small()
                .weak(),
        );
    }
}
