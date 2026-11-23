use eframe::egui::{self, RichText};
use exmon_app::inspection::{Action, Frame, PAGE, Record, Reply, Snapshot, Worker};
use std::{ops::Range, sync::Arc};
#[derive(Default)]
pub struct Inspector {
    pub snapshot: Arc<Snapshot>,
    pub selected: Option<Record>,
    pub range: Option<Range<usize>>,
    pub page: usize,
    pub filter: String,
    pub search: String,
    pub hex: bool,
    pub matches: Vec<Range<usize>>,
    pub notice: Option<String>,
    pub pending: bool,
    pending_frame: Option<Arc<Frame>>,
    capture_path: String,
    export_path: String,
    match_index: usize,
    more_matches: bool,
}
impl Inspector {
    pub fn request(&mut self, worker: &Worker, action: Action) {
        if self.pending {
            self.notice = Some("Wait for the current inspection operation".into());
            return;
        }
        self.pending_frame = match &action {
            Action::Search { frame, .. } => Some(frame.clone()),
            _ => None,
        };
        match worker.request(action) {
            Ok(()) => {
                self.pending = true;
                self.notice = None;
            }
            Err(e) => self.notice = Some(e.into()),
        }
    }
    pub fn poll(&mut self, worker: &Worker, ctx: &egui::Context) {
        if let Some(snapshot) = worker.snapshot() {
            let changed_context = self.snapshot.generation != snapshot.generation
                && !snapshot.file_mode
                && !snapshot.synthetic;
            let mode_changed = (self.snapshot.file_mode, self.snapshot.synthetic)
                != (snapshot.file_mode, snapshot.synthetic);
            if changed_context || mode_changed || self.snapshot.context != snapshot.context {
                self.clear_selection();
            }
            self.snapshot = snapshot;
            if self.selected.is_none()
                && let Some(record) = self.snapshot.records.last()
            {
                self.select(record.clone());
            }
        }
        if let Some(reply) = worker.reply() {
            self.pending = false;
            match reply {
                Reply::Notice(n) => self.notice = Some(n.into()),
                Reply::Copy(text) => {
                    ctx.copy_text(text);
                    self.notice = Some("Selected bytes copied".into());
                }
                Reply::Search(ranges, more) => {
                    if self.selected.as_ref().is_some_and(|s| {
                        self.pending_frame
                            .as_ref()
                            .is_some_and(|f| Arc::ptr_eq(f, &s.frame))
                    }) {
                        self.matches = ranges;
                        self.more_matches = more;
                        self.match_index = 0;
                        if let Some(r) = self.matches.first().cloned() {
                            self.choose(r);
                        }
                        self.notice = Some(format!(
                            "{} matches{}",
                            self.matches.len(),
                            if more { " (more omitted)" } else { "" }
                        ));
                    }
                }
            }
            self.pending_frame = None;
        }
    }
    fn clear_selection(&mut self) {
        self.selected = None;
        self.range = None;
        self.matches.clear();
        self.page = 0;
        self.more_matches = false;
    }
    pub fn select(&mut self, record: Record) {
        self.clear_selection();
        self.range = Some(0..record.frame.bytes().len());
        self.selected = Some(record);
    }
    pub fn choose(&mut self, range: Range<usize>) {
        self.page = range.start / PAGE;
        self.range = Some(range);
    }
    pub fn show(&mut self, ui: &mut egui::Ui, worker: &Worker) {
        egui::ScrollArea::vertical().id_salt("packet-inspector").show(ui,|ui|{
            ui.horizontal_wrapped(|ui|{
                ui.heading("Packet inspection");
                ui.strong(if self.snapshot.synthetic {"SYNTHETIC FIXTURE"}else if self.snapshot.file_mode {"IMPORTED CAPTURE"}else {"LIVE RAW OBSERVATIONS"});
                if ui.add_enabled(!self.pending,egui::Button::new("Load sample packet")).clicked(){self.request(worker,Action::Sample);}
                if ui.add_enabled(!self.pending,egui::Button::new("Live inspection")).clicked(){self.request(worker,Action::Live);}
            });
            ui.label(RichText::new("Captured data may be sensitive. Copy and export expose exact raw bytes.").small().weak());
            ui.horizontal_wrapped(|ui|{
                input(ui,"Display filter",&mut self.filter,1024,260.);
                if ui.add_enabled(!self.pending,egui::Button::new("Apply display filter")).clicked(){self.request(worker,Action::Filter(self.filter.clone()));}
                ui.label(RichText::new("AND terms: udp ipv4 port=12001 host=127.0.0.1 len>=42 contains=4558 diagnostic").small().weak());
            });
            egui::CollapsingHeader::new("Import / export files").id_salt("packet-files").show(ui,|ui|{
                ui.horizontal_wrapped(|ui|{
                    input(ui,"Capture path",&mut self.capture_path,4096,280.);
                    if ui.add_enabled(!self.pending,egui::Button::new("Import PCAP / PCAPNG")).clicked(){self.request(worker,Action::Import(self.capture_path.clone()));}
                });
                ui.horizontal_wrapped(|ui|{
                    input(ui,"New binary export path",&mut self.export_path,4096,280.);
                    if ui.add_enabled(!self.pending && self.range.as_ref().is_some_and(|r|!r.is_empty()),egui::Button::new("Export selected bytes")).clicked()
                        && let (Some(s),Some(r))=(&self.selected,&self.range){self.request(worker,Action::Export{frame:s.frame.clone(),range:r.clone(),path:self.export_path.clone()});}
                });
                ui.label("Local files only. Import: 32 MiB / 10,000 records. Export: raw binary selection, new file only.");
            });
            ui.horizontal_wrapped(|ui|{
                input(ui,"Find bytes",&mut self.search,768,180.);ui.checkbox(&mut self.hex,"HEX pattern");
                if ui.add_enabled(!self.pending && self.selected.is_some(),egui::Button::new("Find")).clicked() && let Some(s)=&self.selected {
                    self.request(worker,Action::Search{frame:s.frame.clone(),text:self.search.clone(),hex:self.hex});
                }
                if ui.add_enabled(!self.matches.is_empty(),egui::Button::new("Find next")).clicked(){self.match_index=(self.match_index+1)%self.matches.len();self.choose(self.matches[self.match_index].clone());}
                for (label,ascii) in [("Copy HEX",false),("Copy ASCII",true)] {
                    if ui.add_enabled(!self.pending && self.range.as_ref().is_some_and(|r|!r.is_empty()),egui::Button::new(label)).clicked()
                        && let (Some(s),Some(r))=(&self.selected,&self.range){self.request(worker,Action::Copy{frame:s.frame.clone(),range:r.clone(),ascii});}
                }
            });
            if self.pending {ui.label("Preparing inspection on worker...");}
            if let Some(n)=&self.notice {ui.label(n);}
            ui.label(RichText::new(format!("{} visible / {} filtered | History evictions: {} | Inspection input drops: {} | Ignored live: {}",self.snapshot.records.len(),self.snapshot.hidden,self.snapshot.evicted,self.snapshot.input_dropped,self.snapshot.ignored_live)).small().weak());
            egui::ScrollArea::vertical().id_salt("packet-history").max_height((self.snapshot.records.len() as f32*22.).clamp(22.,78.)).auto_shrink([false,true]).show_rows(ui,22.,self.snapshot.records.len(),|ui,range|{
                for i in range {let r=self.snapshot.records[i].clone();
                    let label=format!("#{}   {}   {} B{}",r.id,r.frame.protocol,r.frame.bytes().len(),if r.frame.diagnostic {" / diagnostic"}else{""});
                    if ui.selectable_label(self.selected.as_ref().is_some_and(|s|s.id==r.id),label).clicked(){self.select(r);}
                }
            });
            let Some(record)=self.selected.clone() else {ui.label("No packet selected. Start capture, import a capture file, or load the synthetic sample.");return;};
            ui.separator();
            ui.horizontal_wrapped(|ui|{
                ui.strong(format!("Pinned packet #{} / {}",record.id,record.frame.protocol));
                ui.label(match record.frame.observation.source_timestamp_ns(){Some(n)=>format!("Source timestamp: {n} ns"),None=>"Source timestamp: unknown (no UTC assumed)".into()});
                if !self.snapshot.records.iter().any(|r|r.id==record.id){ui.label("Pinned selection is outside visible history");}
            });
            ui.columns(2,|columns|{
                columns[0].heading("Protocol tree");
                egui::ScrollArea::both().id_salt("packet-tree").max_height(330.).auto_shrink([false,false]).show(&mut columns[0],|ui|{
                    self.tree(ui,&record.frame,0);
                });
                let ui=&mut columns[1];ui.heading("Captured bytes / HEX + ASCII");
                ui.horizontal_wrapped(|ui|{
                    let pages=record.frame.bytes().len().div_ceil(PAGE).max(1);
                    if ui.add_enabled(self.page>0,egui::Button::new("Previous bytes")).clicked(){self.page-=1;}
                    ui.label(format!("Page {} / {pages}",self.page+1));
                    if ui.add_enabled(self.page+1<pages,egui::Button::new("Next bytes")).clicked(){self.page+=1;}
                });
                if let Some(range)=&self.range {ui.label(format!("Selected bytes {}..{} ({} B)",range.start,range.end,range.len()));}
                egui::ScrollArea::horizontal().id_salt("packet-hex").show(ui,|ui|{
                    let start=self.page*PAGE;let end=(start+PAGE).min(record.frame.bytes().len());
                    for row in (start..end).step_by(16){ui.horizontal(|ui|{
                        ui.spacing_mut().item_spacing.x=2.;ui.monospace(format!("{row:04X} "));
                        let mut ascii=egui::text::LayoutJob::default();
                        for at in row..(row+16).min(end){let b=record.frame.bytes()[at];
                            let selected=self.range.as_ref().is_some_and(|r|r.contains(&at));
                            let character=if b.is_ascii_graphic()||b==b' ' {b as char}else{'.'};
                            ascii.append(&character.to_string(),0.,egui::TextFormat{font_id:egui::TextStyle::Monospace.resolve(ui.style()),color:ui.visuals().text_color(),background:if selected {ui.visuals().selection.bg_fill}else{egui::Color32::TRANSPARENT},..Default::default()});
                            if ui.selectable_label(selected,RichText::new(format!("{b:02X}")).monospace()).on_hover_text(format!("Byte {at}")).clicked(){
                                if ui.input(|i|i.modifiers.shift) && let Some(r)=&self.range {self.range=Some(r.start.min(at)..r.end.max(at+1));}else{self.range=Some(at..at+1);}
                            }
                        }ui.label(ascii);
                    });}
                });
                ui.label(RichText::new("Click byte to select; Shift-click extends. Layer selection jumps to its first page.").small().weak());
            });
        });
    }
    fn tree(&mut self, ui: &mut egui::Ui, frame: &Frame, index: usize) {
        let node = &frame.nodes[index];
        let children: Vec<_> = frame
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.parent == Some(index))
            .map(|(i, _)| i)
            .collect();
        if children.is_empty() {
            if ui
                .selectable_label(
                    self.range == node.range && node.range.is_some(),
                    &node.label,
                )
                .clicked()
                && let Some(r) = &node.range
            {
                self.choose(r.clone());
            }
        } else {
            let response = egui::CollapsingHeader::new(&node.label)
                .id_salt(("packet-node", index))
                .default_open(true)
                .show(ui, |ui| {
                    for child in children {
                        self.tree(ui, frame, child);
                    }
                });
            if response.header_response.clicked()
                && let Some(r) = &node.range
            {
                self.choose(r.clone());
            }
        }
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
