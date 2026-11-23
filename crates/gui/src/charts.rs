//! Bounded plot rendering and controls; sample history and averages belong to app worker.
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};
use exmon_app::charts::{self, Selection, Snapshot, Spec};
use exmon_domain::config::ProjectConfig;
use std::sync::Arc;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub x: [f64; 2],
    pub y: [f64; 2],
}
impl Bounds {
    pub fn valid(self) -> bool {
        [self.x, self.y].into_iter().all(|r| {
            r[0].is_finite() && r[1].is_finite() && r[1] > r[0] && (r[1] - r[0]).is_finite()
        })
    }
    fn change(self, dx: f64, dy: f64, z: f64) -> Option<Self> {
        let axis = |r: [f64; 2], d: f64| {
            let span = (r[1] - r[0]) * z;
            let mid = r[0] + (r[1] - r[0]) * 0.5 + d * (r[1] - r[0]);
            [mid - span * 0.5, mid + span * 0.5]
        };
        let b = Self {
            x: axis(self.x, dx),
            y: axis(self.y, dy),
        };
        b.valid().then_some(b)
    }
}
struct Controls {
    window: String,
    average: String,
    time: bool,
    auto: bool,
    bounds: Bounds,
    ranges: [String; 4],
}
impl Default for Controls {
    fn default() -> Self {
        Self {
            window: "1024".into(),
            average: "8".into(),
            time: true,
            auto: true,
            bounds: Bounds {
                x: [0., 1.],
                y: [0., 1.],
            },
            ranges: ["0".into(), "1".into(), "0".into(), "1".into()],
        }
    }
}
pub struct Charts {
    pub specs: Vec<Spec>,
    pub selected: usize,
    pub snapshot: Snapshot,
    pub plot_rect: Option<Rect>,
    controls: Vec<Controls>,
    next: u64,
    notice: Option<&'static str>,
    started: bool,
}
impl Default for Charts {
    fn default() -> Self {
        Self {
            specs: vec![Spec {
                id: 1,
                name: "Chart 1".into(),
                samples: 1024,
                average: 8,
                series: vec![],
            }],
            selected: 0,
            snapshot: Snapshot::default(),
            plot_rect: None,
            controls: vec![Controls::default()],
            next: 2,
            notice: None,
            started: false,
        }
    }
}
impl Charts {
    pub fn bounds(&self) -> Bounds {
        self.controls[self.selected].bounds
    }
    pub fn autoscale(&self) -> bool {
        self.controls[self.selected].auto
    }
    pub fn time_axis(&self) -> bool {
        self.controls[self.selected].time
    }

    pub fn poll(&mut self, worker: &charts::Worker) {
        if !self.started {
            self.publish(worker);
            self.started = true;
        }
        if let Some(s) = worker.snapshot() {
            self.snapshot = s;
        }
    }
    fn publish(&mut self, worker: &charts::Worker) {
        self.notice = worker.configure(self.specs.clone()).err();
    }
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        config: Option<Arc<ProjectConfig>>,
        worker: &charts::Worker,
    ) {
        self.poll(worker);
        let changed = egui::ScrollArea::vertical()
            .id_salt("chart-controls-scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| self.contents(ui, config))
            .inner;
        if changed {
            self.publish(worker);
        }
    }
    fn contents(&mut self, ui: &mut egui::Ui, config: Option<Arc<ProjectConfig>>) -> bool {
        let mut changed = false;
        ui.heading("Live charts");
        ui.horizontal_wrapped(|ui| {
            for (i, s) in self.specs.iter().enumerate() {
                if ui.selectable_label(i == self.selected, &s.name).clicked() {
                    self.selected = i;
                }
            }
            if ui
                .add_enabled(
                    self.specs.len() < charts::CHARTS && self.next < u64::MAX,
                    egui::Button::new("New chart"),
                )
                .clicked()
            {
                let id = self.next;
                self.next += 1;
                self.specs.push(Spec {
                    id,
                    name: format!("Chart {id}"),
                    samples: 1024,
                    average: 8,
                    series: vec![],
                });
                self.controls.push(Controls::default());
                self.selected = self.specs.len() - 1;
                changed = true;
            }
            if ui
                .add_enabled(self.specs.len() > 1, egui::Button::new("Remove chart"))
                .clicked()
            {
                self.specs.remove(self.selected);
                self.controls.remove(self.selected);
                self.selected = 0;
                changed = true;
            }
        });
        let spec = &mut self.specs[self.selected];
        let controls = &mut self.controls[self.selected];
        ui.collapsing("Select numeric series", |ui| {
            if let Some(config) = config {
                for protocol in &config.protocols {
                    for field in &protocol.fields {
                        if !exmon_domain::fields::is_numeric(field.representation) {
                            continue;
                        }
                        let exists = spec
                            .series
                            .iter()
                            .position(|s| s.protocol == protocol.id && s.field == field.id);
                        let mut selected = exists.is_some();
                        let label = format!("{} / {} [{}]", protocol.name, field.id, field.unit);
                        if ui
                            .add_enabled(
                                selected || spec.series.len() < charts::SERIES,
                                egui::Checkbox::new(&mut selected, label),
                            )
                            .changed()
                        {
                            if let Some(i) = exists {
                                spec.series.remove(i);
                            } else {
                                spec.series.push(Selection {
                                    protocol: protocol.id.clone(),
                                    field: field.id.clone(),
                                    label: format!("{} / {}", protocol.id, field.id)
                                        .chars()
                                        .take(64)
                                        .collect(),
                                    unit: field.unit.clone(),
                                });
                            }
                            changed = true;
                        }
                    }
                }
            } else {
                ui.label("Waiting for validated protocol definitions");
            }
        });
        ui.horizontal_wrapped(|ui|{
                let label=ui.label("Chart sample window");ui.add(egui::TextEdit::singleline(&mut controls.window).desired_width(65.).char_limit(5).hint_text("1024")).labelled_by(label.id);
                let label=ui.label("Chart average length");ui.add(egui::TextEdit::singleline(&mut controls.average).desired_width(65.).char_limit(5).hint_text("8")).labelled_by(label.id);
                if ui.button("Apply chart window").clicked(){let parse=|s:&str|{if s.is_empty()||!s.bytes().all(|b|b.is_ascii_digit()){None}else{s.parse::<usize>().ok()}};match(parse(&controls.window),parse(&controls.average)){(Some(w),Some(a)) if (1..=charts::SAMPLES).contains(&w)&&(1..=w).contains(&a)=>{spec.samples=w;spec.average=a;self.notice=None;changed=true;},_=>self.notice=Some("Window must be 1..4096 and average 1..window; previous settings retained")}}
            });
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut controls.time, true, "Time axis");
            ui.selectable_value(&mut controls.time, false, "Index axis");
            ui.checkbox(&mut controls.auto, "Autoscale");
            if ui.button("Reset chart view").clicked() {
                controls.auto = true;
            }
            if ui.button("Zoom chart in").clicked()
                && let Some(b) = controls.bounds.change(0., 0., 0.8)
            {
                controls.bounds = b;
                controls.auto = false;
            }
            if ui.button("Zoom chart out").clicked()
                && let Some(b) = controls.bounds.change(0., 0., 1.25)
            {
                controls.bounds = b;
                controls.auto = false;
            }
        });
        ui.collapsing("Manual chart ranges",|ui|{
                ui.horizontal_wrapped(|ui|{for(i,label)in["X minimum","X maximum","Y minimum","Y maximum"].iter().enumerate(){let label=ui.label(*label);ui.add(egui::TextEdit::singleline(&mut controls.ranges[i]).desired_width(75.).char_limit(32)).labelled_by(label.id);}if ui.button("Apply chart ranges").clicked(){let n:Option<Vec<f64>>=controls.ranges.iter().map(|s|s.parse().ok()).collect();if let Some(n)=n && (Bounds{x:[n[0],n[1]],y:[n[2],n[3]]}).valid(){controls.bounds=Bounds{x:[n[0],n[1]],y:[n[2],n[3]]};controls.auto=false;self.notice=None;}else{self.notice=Some("Ranges must be finite, increasing and have finite spans; previous view retained");}}});
            });
        if let Some(error) = self.notice {
            ui.colored_label(Color32::from_rgb(225, 135, 80), error);
        }
        if self.snapshot.configuration_error || self.snapshot.failed {
            ui.colored_label(
                Color32::RED,
                "Chart worker configuration unavailable; previous valid view retained",
            );
        }
        ui.label(format!(
            "Chart input drops: {} batches | Prepared field samples: {} | Session {}",
            self.snapshot.input_dropped, self.snapshot.processed, self.snapshot.generation
        ));
        let view = self.snapshot.charts.iter().find(|c| c.spec.id == spec.id);
        let palette = [
            Color32::from_rgb(80, 180, 245),
            Color32::from_rgb(245, 170, 70),
            Color32::from_rgb(125, 220, 130),
            Color32::from_rgb(210, 125, 230),
            Color32::from_rgb(240, 100, 120),
            Color32::from_rgb(100, 220, 210),
            Color32::from_rgb(210, 200, 110),
            Color32::from_rgb(160, 170, 250),
        ];
        if let Some(view) = view {
            ui.vertical(|ui|{for(i,s)in view.series.iter().enumerate(){ui.add(egui::Label::new(egui::RichText::new(format!("{} [{}] | solid raw, dashed MA{} | {} retained, {} evicted, {} decimated, {} gaps",s.selection.label,s.selection.unit,spec.average,s.retained,s.evicted,s.decimated,s.invalid)).color(palette[i])).wrap());}});
        }
        let size = Vec2::new(
            ui.available_width().max(1.),
            (ui.available_height() - 40.).clamp(180., 600.),
        );
        let (response, painter) = ui.allocate_painter(size, Sense::drag());
        let rect = response.rect.shrink2(Vec2::new(64., 26.));
        self.plot_rect = Some(rect);
        if rect.width() <= 1. || rect.height() <= 1. {
            return changed;
        }
        if controls.auto {
            let mut x: Option<[f64; 2]> = None;
            let mut y: Option<[f64; 2]> = None;
            if let Some(view) = view {
                for s in &view.series {
                    for p in s.points.iter() {
                        let px = if controls.time {
                            p.seconds
                        } else {
                            p.index as f64
                        };
                        for v in [p.value, p.average].into_iter().flatten() {
                            expand(&mut x, px);
                            expand(&mut y, v);
                        }
                    }
                }
            }
            controls.bounds = Bounds {
                x: pad(x),
                y: pad(y),
            };
        }
        if response.dragged() {
            let delta = ui.input(|i| i.pointer.delta());
            if let Some(b) = controls.bounds.change(
                -f64::from(delta.x / rect.width()),
                f64::from(delta.y / rect.height()),
                1.,
            ) {
                controls.bounds = b;
                controls.auto = false;
            }
        }
        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.
                && let Some(b) = controls.bounds.change(
                    0.,
                    0.,
                    (-f64::from(scroll) * 0.005).exp().clamp(0.2, 5.),
                )
            {
                controls.bounds = b;
                controls.auto = false;
            }
        }
        let b = controls.bounds;
        let text = ui.visuals().text_color();
        painter.rect_filled(rect, 0., ui.visuals().extreme_bg_color);
        for i in 0..=4 {
            let t = i as f32 / 4.;
            let x = egui::lerp(rect.x_range(), t);
            let y = egui::lerp(rect.y_range(), 1. - t);
            painter.line_segment(
                [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
                Stroke::new(0.5, ui.visuals().weak_text_color()),
            );
            painter.line_segment(
                [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
                Stroke::new(0.5, ui.visuals().weak_text_color()),
            );
            painter.text(
                Pos2::new(x, rect.bottom() + 3.),
                egui::Align2::CENTER_TOP,
                format!("{:.3}", b.x[0] + (b.x[1] - b.x[0]) * f64::from(t)),
                egui::FontId::monospace(10.),
                text,
            );
            painter.text(
                Pos2::new(rect.left() - 3., y),
                egui::Align2::RIGHT_CENTER,
                format!("{:.3}", b.y[0] + (b.y[1] - b.y[0]) * f64::from(t)),
                egui::FontId::monospace(10.),
                text,
            );
        }
        painter.text(
            Pos2::new(rect.center().x, rect.top() - 4.),
            egui::Align2::CENTER_BOTTOM,
            if controls.time {
                "Relative time (s)"
            } else {
                "Per-series sample index"
            },
            egui::FontId::proportional(12.),
            text,
        );
        if let Some(view) = view {
            let clip = painter.with_clip_rect(rect);
            for (i, s) in view.series.iter().enumerate() {
                for average in [false, true] {
                    let mut last: Option<([f64; 2], u64)> = None;
                    for p in s.points.iter() {
                        let v = if average { p.average } else { p.value };
                        let next = v
                            .and_then(|v| {
                                project(
                                    b,
                                    if controls.time {
                                        p.seconds
                                    } else {
                                        p.index as f64
                                    },
                                    v,
                                )
                            })
                            .map(|pos| (pos, p.segment));
                        if let (Some((a, segment)), Some((z, current))) = (last, next)
                            && segment == current
                            && let Some([a, z]) = clip_line(rect, a, z)
                        {
                            if average {
                                let delta = z - a;
                                let len = delta.length();
                                if len.is_finite() {
                                    let steps = (len / 8.).ceil().clamp(1., 16.) as usize;
                                    for k in (0..steps).step_by(2) {
                                        let start = k as f32 / steps as f32;
                                        let end = ((k + 1) as f32 / steps as f32).min(1.);
                                        clip.line_segment(
                                            [a + delta * start, a + delta * end],
                                            Stroke::new(1.5, palette[i]),
                                        );
                                    }
                                }
                            } else {
                                clip.line_segment([a, z], Stroke::new(1., palette[i]));
                            }
                        }
                        last = next;
                    }
                }
            }
        }
        if spec.series.is_empty() {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "Select numeric series to plot",
                egui::FontId::proportional(14.),
                text,
            );
        }

        changed
    }
}
fn expand(range: &mut Option<[f64; 2]>, v: f64) {
    if let Some(r) = range {
        r[0] = r[0].min(v);
        r[1] = r[1].max(v);
    } else {
        *range = Some([v, v]);
    }
}
fn pad(range: Option<[f64; 2]>) -> [f64; 2] {
    let Some(r) = range else { return [0., 1.] };
    if r[0] < r[1] && (r[1] - r[0]).is_finite() {
        return r;
    }
    let d = (r[0].abs() * 0.01).max(1.);
    let out = [r[0] - d, r[1] + d];
    if (Bounds { x: out, y: out }).valid() {
        out
    } else {
        [0., 1.]
    }
}
fn project(b: Bounds, x: f64, y: f64) -> Option<[f64; 2]> {
    let p = [
        (x - b.x[0]) / (b.x[1] - b.x[0]),
        (y - b.y[0]) / (b.y[1] - b.y[0]),
    ];
    p.iter().all(|v| v.is_finite()).then_some(p)
}
/// Clip in f64 before converting to bounded screen coordinates; never clamp each endpoint independently.
fn clip_line(rect: Rect, a: [f64; 2], z: [f64; 2]) -> Option<[Pos2; 2]> {
    let mut low = 0f64;
    let mut high = 1f64;
    let delta = [z[0] - a[0], z[1] - a[1]];
    for i in 0..2 {
        let d = delta[i];
        if !d.is_finite() {
            return None;
        }
        if d == 0. {
            if !(0.0..=1.0).contains(&a[i]) {
                return None;
            }
        } else {
            let t0 = -a[i] / d;
            let t1 = (1. - a[i]) / d;
            low = low.max(t0.min(t1));
            high = high.min(t0.max(t1));
        }
    }
    if low > high {
        return None;
    }
    let screen = |t: f64| {
        Pos2::new(
            rect.left() + (a[0] + delta[0] * t).clamp(0., 1.) as f32 * rect.width(),
            rect.bottom() - (a[1] + delta[1] * t).clamp(0., 1.) as f32 * rect.height(),
        )
    };
    Some([screen(low), screen(high)])
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clipping_preserves_visible_slope_and_rejects_nonintersections() {
        let r = Rect::from_min_max(Pos2::ZERO, Pos2::new(100., 100.));
        assert_eq!(
            clip_line(r, [-1., -1.], [2., 2.]),
            Some([Pos2::new(0., 100.), Pos2::new(100., 0.)])
        );
        assert!(clip_line(r, [-2., 2.], [-1., 3.]).is_none());
        assert!(
            clip_line(r, [-1e10, 0.5], [1e10, 0.5])
                .unwrap()
                .iter()
                .all(|p| r.contains(*p))
        );
    }
}
