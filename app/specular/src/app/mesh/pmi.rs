//! A STEP model's PMI in the 3D view: dimensions, feature control frames (ISO 1101 / ASME
//! Y14.5 symbols drawn as strokes), datum feature symbols and annotations that are only
//! drawn, each with a leader to its faces; a list of them in the review panel (`Alt+A`) and
//! the file's saved views.
//!
//! - `Shift+A` shows or hides the annotations (kept across files).
//! - In the list, the chosen annotation's faces light up; Enter goes to it. The saved views
//!   come first: Enter turns to one and shows only its annotations; `A` shows them all again.
//! - `D` draws the annotations as the file draws them (its polylines) instead of as labels.
//!
//! The data comes from the decode host ([`specular_decode::pmi`]).

use std::sync::Arc;

use specular_decode::MeshModel;
use specular_decode::pmi::{Characteristic, Label, Pmi, PmiItem, PmiKind};
use specular_gfx::{Align, DrawList, Rect, Text, TextStyle};
use specular_i18n::t;
use specular_input::{Action, Key, KeyCombo};
use specular_render_3d::{Camera, Pose, Projection};
use specular_theme::Palette;
use specular_theme::metrics::FONT_SMALL;

use super::MeshView;
use super::bim::{PanelKind, PanelRow, PanelView, centre};
use crate::app::App;

/// The colour of the chosen annotation's faces (linear RGBA).
const LIT: [f32; 4] = [0.95, 0.42, 0.05, 1.0];
/// Most polyline segments drawn a frame.
const MAX_SEGMENTS: usize = 200_000;

/// The current model's PMI and how it is shown.
#[derive(Default)]
pub(in crate::app) struct PmiState {
    pub data: Option<Arc<Pmi>>,
    /// Kept across files: annotations hidden, and drawn as the file draws them.
    pub hidden: bool,
    pub drawing: bool,
    /// The saved view turned to: only its annotations are shown.
    pub view: Option<usize>,
    /// The annotation whose faces are lit, and whether the mesh is restyled for it.
    lit: Option<usize>,
    styled: bool,
}

impl PmiState {
    /// Kept across files.
    pub(super) fn carry(&self) -> PmiState {
        PmiState {
            hidden: self.hidden,
            drawing: self.drawing,
            ..PmiState::default()
        }
    }

    fn views(&self) -> usize {
        self.data.as_ref().map_or(0, |d| d.views.len())
    }

    /// The annotations shown: the saved view's, else all.
    fn shown(&self, i: usize) -> bool {
        let Some(d) = &self.data else { return false };
        match self.view.and_then(|v| d.views.get(v)) {
            Some(v) if !v.items.is_empty() => v.items.contains(&i),
            _ => true,
        }
    }
}

/// One label as drawn: where (logical px), where its leader ends, what it says, and whether
/// it is the chosen one.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) struct Placed {
    pub at: (f32, f32),
    pub target: Option<(f32, f32)>,
    pub label: Label,
    pub chosen: bool,
}

/// A drawn segment: its ends (logical px) and whether it is the chosen annotation's.
pub(in crate::app) type Segment = ((f32, f32), (f32, f32), bool);

/// What the annotations draw over the view (logical px).
#[derive(Clone, Debug, Default, PartialEq)]
pub(in crate::app) struct PmiOverlay {
    pub labels: Vec<Placed>,
    /// Drawn annotations' segments, and whether each is the chosen one's.
    pub segments: Vec<Segment>,
    /// The view (labels are kept inside it).
    pub area: Rect,
    /// A panel over the view labels keep clear of.
    pub avoid: Option<Rect>,
}

/// The kind of an annotation in words.
fn kind_words(it: &PmiItem) -> String {
    if let Some(c) = it.characteristic {
        return t!("pmi-characteristic", what = c.id());
    }
    if let Some(d) = it.dim {
        return t!("pmi-dimension", what = d.id());
    }
    t!(
        "pmi-kind",
        what = match it.kind {
            PmiKind::Dimension => "dimension",
            PmiKind::GeometricTolerance => "tolerance",
            PmiKind::Datum => "datum",
            PmiKind::DatumTarget => "target",
            PmiKind::Graphic => "graphic",
        }
    )
}

/// A row's text: the label, or for a drawn-only annotation its name.
fn row_text(it: &PmiItem) -> String {
    if it.text.trim().is_empty() {
        t!("pmi-kind", what = "graphic")
    } else {
        it.text.clone()
    }
}

/// A row's text as the panel draws it: an annotation in Latin script and digits ("100 ±0.2",
/// "⌀12 H7") kept in its own left-to-right order in a right-to-left UI.
fn shown_row_text(it: &PmiItem) -> String {
    let s = row_text(it);
    if s.chars()
        .any(|c| ('\u{0590}'..='\u{08ff}').contains(&c) || ('\u{fb1d}'..='\u{fefc}').contains(&c))
    {
        return s;
    }
    specular_i18n::ltr_value(&s, specular_i18n::I18n::global().direction())
}

fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn scale3(a: [f64; 3], s: f64) -> [f64; 3] {
    [a[0] * s, a[1] * s, a[2] * s]
}

fn f64s(p: [f32; 3]) -> [f64; 3] {
    p.map(f64::from)
}

impl MeshView {
    /// The PMI panel.
    pub(super) fn pmi_panel(&self) -> PanelView {
        let st = &self.pmi;
        let mut rows = Vec::new();
        let (mut title, mut notes) = (t!("pmi-title", count = 0i64), Vec::new());
        if let Some(d) = &st.data {
            title = t!("pmi-title", count = d.items.len() as i64);
            for (i, v) in d.views.iter().enumerate() {
                let shows = if v.items.is_empty() {
                    t!("pmi-view-all")
                } else {
                    t!("pmi-view-shows", count = v.items.len() as i64)
                };
                let detail = specular_i18n::dot_join(&[t!("pmi-view"), shows]);
                rows.push(PanelRow {
                    index: i,
                    text: v.name.clone(),
                    detail: Some(detail),
                });
            }
            for (i, it) in d.items.iter().enumerate() {
                let mut detail = kind_words(it);
                if !it.triangles.is_empty() {
                    detail = specular_i18n::dot_join(&[detail, t!("pmi-has-faces")]);
                }
                rows.push(PanelRow {
                    index: d.views.len() + i,
                    text: shown_row_text(it),
                    detail: Some(detail),
                });
            }
            if let Some(v) = st.view.and_then(|v| d.views.get(v)) {
                notes.push(t!("pmi-view-shown", name = v.name.clone()));
            }
            if st.hidden {
                notes.push(t!("pmi-note", what = "hidden"));
            }
        }
        PanelView {
            title,
            chosen: (!rows.is_empty()).then_some(self.bim.row),
            empty: rows.is_empty().then(|| t!("pmi-note", what = "none")),
            rows,
            notes,
            hints: t!("pmi-note", what = "hints"),
            prompt: None,
        }
    }

    pub(super) fn pmi_rows_len(&self) -> usize {
        self.pmi
            .data
            .as_ref()
            .map_or(0, |d| d.views.len() + d.items.len())
    }

    /// The annotation of the chosen row, while the PMI panel is open.
    fn pmi_chosen(&self) -> Option<usize> {
        if self.bim.panel != Some(PanelKind::Pmi) {
            return None;
        }
        let d = self.pmi.data.as_ref()?;
        let i = self.bim.row.checked_sub(d.views.len())?;
        (i < d.items.len()).then_some(i)
    }

    /// Lights the chosen annotation's faces (the mesh restyled), or none.
    fn pmi_light(&mut self) {
        let want = self.pmi_chosen().filter(|&i| {
            self.pmi
                .data
                .as_ref()
                .is_some_and(|d| !d.items[i].triangles.is_empty())
        });
        if want == self.pmi.lit {
            return;
        }
        self.pmi.lit = want;
        match (want, self.model.as_ref(), self.pmi.data.as_ref()) {
            (Some(i), Some(model), Some(d)) => {
                let mesh =
                    specular_decode::pmi::highlighted(&model.mesh, &d.items[i].triangles, LIT);
                self.bim.restyled = Some(MeshModel {
                    mesh: Arc::new(mesh),
                    textures: model.textures.clone(),
                    ifc: model.ifc.clone(),
                });
                self.pmi.styled = true;
            }
            _ if self.pmi.styled => {
                self.bim.restyled = None;
                self.pmi.styled = false;
            }
            _ => return,
        }
        self.gpu = None;
        self.bim.sections_changed();
        self.changed();
    }

    /// The labels and drawn annotations over the view (logical px; `scale` physical px per
    /// logical).
    pub(in crate::app) fn pmi_overlay(&self, scale: f64) -> Option<PmiOverlay> {
        let st = &self.pmi;
        let d = st.data.as_ref()?;
        if st.hidden {
            return None;
        }
        let r = self.rect?;
        let (w, h) = self.size();
        let p = self.camera.project(&self.bounds, w, h, 0.0);
        let to_l = |q: [f32; 3]| -> Option<(f32, f32)> {
            let (x, y, _) = p.to_px(f64s(q), w, h)?;
            Some((
                ((x + f64::from(r.x)) / scale) as f32,
                ((y + f64::from(r.y)) / scale) as f32,
            ))
        };
        let chosen = self.pmi_chosen();
        let mut o = PmiOverlay {
            area: Rect::new(
                (f64::from(r.x) / scale) as f32,
                (f64::from(r.y) / scale) as f32,
                (f64::from(r.w) / scale) as f32,
                (f64::from(r.h) / scale) as f32,
            ),
            ..PmiOverlay::default()
        };
        for (i, it) in d.items.iter().enumerate() {
            let is_chosen = chosen == Some(i);
            if !is_chosen && !st.shown(i) {
                continue;
            }
            let drawn = it.kind == PmiKind::Graphic
                || (st.drawing && !it.polylines.is_empty() && !is_chosen);
            if drawn {
                for line in &it.polylines {
                    for s in line.windows(2) {
                        if o.segments.len() >= MAX_SEGMENTS {
                            break;
                        }
                        if let (Some(a), Some(b)) = (to_l(s[0]), to_l(s[1])) {
                            o.segments.push((a, b, is_chosen));
                        }
                    }
                }
                if it.kind == PmiKind::Graphic {
                    continue;
                }
                if !is_chosen {
                    continue;
                }
            }
            let Some(at) = it.anchor.and_then(to_l) else {
                continue;
            };
            o.labels.push(Placed {
                at,
                target: it.target.and_then(to_l),
                label: it.label.clone(),
                chosen: is_chosen,
            });
        }
        // The chosen one on top.
        o.labels.sort_by_key(|l| l.chosen);
        Some(o)
    }
}

impl App {
    /// The PMI's own actions: the panel (`Alt+A`) and showing or hiding the annotations
    /// (`Shift+A`, over a model with PMI). True when taken.
    pub(super) fn pmi_action(&mut self, a: Action) -> bool {
        let has = self.mesh_active() && self.mesh.pmi.data.is_some();
        match a {
            Action::MeshPmi => {
                if self.mesh_active() {
                    self.bim_toggle_panel(PanelKind::Pmi);
                    self.mesh.changed();
                }
                true
            }
            Action::ToggleAnnotations if has => {
                let st = &mut self.mesh.pmi;
                st.hidden = !st.hidden;
                let msg = t!(
                    "pmi-note",
                    what = if st.hidden { "hidden" } else { "shown" }
                );
                let now = self.now();
                self.chrome.toast(msg, None, now);
                self.mesh.changed();
                self.redraw = true;
                true
            }
            _ => false,
        }
    }

    /// A model with PMI is shown (its annotations can be shown or hidden).
    pub(in crate::app) fn pmi_present(&self) -> bool {
        self.mesh_active() && self.mesh.pmi.data.is_some()
    }

    /// Follows the panel's chosen row: its faces lit.
    pub(super) fn pmi_sync(&mut self) {
        let before = self.mesh.pmi.lit;
        self.mesh.pmi_light();
        if before != self.mesh.pmi.lit {
            self.redraw = true;
        }
    }

    /// Enter in the PMI panel: turn to the saved view, or go to the annotation.
    pub(super) fn pmi_enter(&mut self) {
        let row = self.mesh.bim.row;
        let views = self.mesh.pmi.views();
        if row < views {
            self.pmi_go_view(row);
        } else {
            self.pmi_go_item(row - views);
        }
    }

    /// The PMI panel's own keys; true when taken.
    pub(super) fn pmi_key(&mut self, c: &KeyCombo) -> bool {
        let plain = !c.mods.ctrl && !c.mods.alt && !c.mods.meta && !c.mods.shift;
        let Key::Char(ch) = &c.key else {
            return false;
        };
        if !plain {
            return false;
        }
        let now = self.now();
        match ch.to_ascii_lowercase() {
            'd' => {
                let st = &mut self.mesh.pmi;
                st.drawing = !st.drawing;
                let msg = t!(
                    "pmi-note",
                    what = if st.drawing { "as-drawn" } else { "as-labels" }
                );
                self.chrome.toast(msg, None, now);
            }
            'a' => {
                self.mesh.pmi.view = None;
                self.chrome
                    .announce_status(t!("pmi-note", what = "all-shown"));
            }
            _ => return false,
        }
        self.mesh.changed();
        true
    }

    fn pmi_go_view(&mut self, i: usize) {
        let Some(v) = self
            .mesh
            .pmi
            .data
            .as_ref()
            .and_then(|d| d.views.get(i))
            .cloned()
        else {
            return;
        };
        self.mesh.pmi.view = Some(i);
        // The file's direction and up, the whole model in view.
        let m = &self.mesh;
        let (w, h) = m.size();
        let projection = if v.orthographic {
            Projection::Orthographic
        } else {
            Projection::Perspective
        };
        let start = Pose {
            eye: add(centre(&m.bounds), scale3(v.direction, -1.0)),
            direction: v.direction,
            up: v.up,
            projection,
            fov: specular_render_3d::FOV_Y,
            height: 1.0,
            distance: 1.0,
        };
        let mut cam = Camera::from_pose(&start, &m.bounds, w, h);
        cam.projection = projection;
        cam.fit(&m.bounds, w, h);
        let to = cam.pose(&m.bounds, w, h);
        self.bim_animate_to(to, None);
        self.chrome.announce_status(v.name);
    }

    fn pmi_go_item(&mut self, i: usize) {
        let Some(it) = self
            .mesh
            .pmi
            .data
            .as_ref()
            .and_then(|d| d.items.get(i))
            .cloned()
        else {
            return;
        };
        let b = it.bounds;
        if b.is_empty() {
            return;
        }
        let m = &self.mesh;
        let (w, h) = m.size();
        let cur = m.camera.pose(&m.bounds, w, h);
        let c = centre(&b);
        let r = f64::from(b.radius().max(m.bounds.radius() * 0.05)).max(1e-9);
        let dist = r / (cur.fov / 2.0).tan() * 1.6;
        let to = Pose {
            eye: add(c, scale3(cur.direction, -dist)),
            height: 3.2 * r,
            distance: dist,
            ..cur
        };
        self.bim_animate_to(to, None);
        self.chrome.announce_status(row_text(&it));
    }
}

/// Draws a characteristic's symbol (ISO 1101) as strokes in a square `r`.
fn symbol(dl: &mut DrawList, c: Characteristic, r: Rect, ink: specular_theme::Color, lw: f32) {
    use Characteristic::*;
    let (x0, y0, s) = (r.x, r.y, r.w.min(r.h));
    let p = |u: f32, v: f32| (x0 + u * s, y0 + v * s);
    let (cx, cy) = p(0.5, 0.5);
    let ring = |dl: &mut DrawList, rad: f32| {
        dl.arc((cx, cy), rad * s, 0.0, std::f32::consts::TAU, lw, ink);
    };
    match c {
        Straightness => dl.line(p(0.15, 0.5), p(0.85, 0.5), lw, ink),
        Flatness => {
            let q = [p(0.3, 0.32), p(0.9, 0.32), p(0.7, 0.68), p(0.1, 0.68)];
            for k in 0..4 {
                dl.line(q[k], q[(k + 1) % 4], lw, ink);
            }
        }
        Circularity => ring(dl, 0.3),
        Cylindricity => {
            ring(dl, 0.25);
            dl.line(p(0.32, 0.85), p(0.55, 0.15), lw, ink);
            dl.line(p(0.45, 0.85), p(0.68, 0.15), lw, ink);
        }
        LineProfile => dl.arc(
            (cx, cy + 0.12 * s),
            0.32 * s,
            std::f32::consts::PI,
            std::f32::consts::PI,
            lw,
            ink,
        ),
        SurfaceProfile => {
            dl.arc(
                (cx, cy + 0.12 * s),
                0.32 * s,
                std::f32::consts::PI,
                std::f32::consts::PI,
                lw,
                ink,
            );
            dl.line(p(0.18, 0.62), p(0.82, 0.62), lw, ink);
        }
        Angularity => {
            dl.line(p(0.15, 0.8), p(0.85, 0.8), lw, ink);
            dl.line(p(0.15, 0.8), p(0.75, 0.25), lw, ink);
        }
        Perpendicularity => {
            dl.line(p(0.15, 0.8), p(0.85, 0.8), lw, ink);
            dl.line(p(0.5, 0.8), p(0.5, 0.15), lw, ink);
        }
        Parallelism => {
            dl.line(p(0.25, 0.85), p(0.5, 0.15), lw, ink);
            dl.line(p(0.5, 0.85), p(0.75, 0.15), lw, ink);
        }
        Position => {
            ring(dl, 0.24);
            dl.line(p(0.5, 0.1), p(0.5, 0.9), lw, ink);
            dl.line(p(0.1, 0.5), p(0.9, 0.5), lw, ink);
        }
        Concentricity | Coaxiality => {
            ring(dl, 0.33);
            ring(dl, 0.16);
        }
        Symmetry => {
            dl.line(p(0.25, 0.3), p(0.75, 0.3), lw, ink);
            dl.line(p(0.1, 0.5), p(0.9, 0.5), lw, ink);
            dl.line(p(0.25, 0.7), p(0.75, 0.7), lw, ink);
        }
        CircularRunout => {
            dl.line(p(0.25, 0.85), p(0.75, 0.2), lw, ink);
            dl.line(p(0.75, 0.2), p(0.5, 0.25), lw, ink);
            dl.line(p(0.75, 0.2), p(0.7, 0.45), lw, ink);
        }
        TotalRunout => {
            dl.line(p(0.15, 0.85), p(0.85, 0.85), lw, ink);
            for x in [0.3, 0.7] {
                dl.line(p(x - 0.12, 0.85), p(x + 0.12, 0.2), lw, ink);
                dl.line(p(x + 0.12, 0.2), p(x - 0.08, 0.27), lw, ink);
                dl.line(p(x + 0.12, 0.2), p(x + 0.1, 0.42), lw, ink);
            }
        }
    }
}

/// Draws the annotations: drawn segments, then each label's leader and box (labels moved
/// down where they would cover one drawn before).
pub(in crate::app) fn draw_overlay(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    o: &PmiOverlay,
    ts: f32,
) {
    let lw = 1.2 * ts;
    for &(a, b, chosen) in &o.segments {
        dl.line(a, b, lw, if chosen { pal.accent } else { pal.text });
    }
    let st = TextStyle::new(FONT_SMALL * ts);
    let pad = 4.0 * ts;
    let mut taken: Vec<Rect> = o.avoid.into_iter().collect();
    for l in &o.labels {
        let ink = if l.chosen { pal.accent } else { pal.text };
        let edge = if l.chosen { pal.accent } else { pal.text_muted };
        // The cells: a symbol cell (a square), then text cells.
        let (sym, cells): (Option<Characteristic>, Vec<String>) = match &l.label {
            Label::Text(s) => (None, vec![s.clone()]),
            Label::Frame {
                characteristic,
                cells,
            } => (*characteristic, cells.clone()),
            Label::Datum(s) => (None, vec![s.clone()]),
            Label::Drawing => continue,
        };
        let blocks: Vec<_> = cells
            .iter()
            .map(|c| text.layout(c, st, None, Align::Start))
            .collect();
        let row_h = blocks
            .iter()
            .map(|b| b.height)
            .fold(FONT_SMALL * ts, f32::max)
            + 2.0 * pad;
        let frame = matches!(l.label, Label::Frame { .. });
        let sym_w = if frame && sym.is_some() { row_h } else { 0.0 };
        let widths: Vec<f32> = blocks.iter().map(|b| b.width + 2.0 * pad).collect();
        let w = sym_w + widths.iter().sum::<f32>();
        // Inside the view, then below any label it would cover.
        let a = o.area;
        let x = (l.at.0 - w / 2.0).clamp(a.x, (a.right() - w).max(a.x));
        let y = (l.at.1 - row_h / 2.0).clamp(a.y, (a.bottom() - row_h).max(a.y));
        let mut r = Rect::new(x, y, w, row_h);
        for _ in 0..8 {
            match taken
                .iter()
                .find(|t| t.intersect(&r).w > 0.0 && t.intersect(&r).h > 0.0)
            {
                Some(t) => r = r.translate(0.0, t.bottom() - r.y + 2.0 * ts),
                None => break,
            }
        }
        taken.push(r);
        if let Some(t) = l.target {
            // From the box's nearest side.
            let from = (t.0.clamp(r.x, r.right()), t.1.clamp(r.y, r.bottom()));
            dl.line(from, t, lw, edge);
            if matches!(l.label, Label::Datum(_)) {
                // A datum feature's filled triangle.
                let s = 5.0 * ts;
                dl.line((t.0 - s, t.1), (t.0 + s, t.1), 2.0 * lw, edge);
                dl.circle(t, s * 0.6, edge);
            } else {
                dl.circle(t, 2.2 * ts, edge);
            }
        }
        dl.rect_border(r, 2.0 * ts, pal.surface, edge, lw);
        let mut x = r.x;
        if let (true, Some(c)) = (frame, sym) {
            let cell = Rect::new(x, r.y, sym_w, row_h).inset(pad);
            symbol(dl, c, cell, ink, lw);
            x += sym_w;
            dl.line((x, r.y), (x, r.bottom()), lw, edge);
        }
        for (k, b) in blocks.iter().enumerate() {
            text.draw(dl, b, x + pad, r.y + (row_h - b.height) / 2.0, ink);
            x += widths[k];
            if frame && k + 1 < blocks.len() {
                dl.line((x, r.y), (x, r.bottom()), lw, edge);
            }
        }
    }
}

#[cfg(test)]
mod shots;
