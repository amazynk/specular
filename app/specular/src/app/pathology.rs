//! Pathology tools over colour images (tier 1: any slide or colour microscope image, in
//! every profile): stain separation drawn on the GPU per visible tile
//! ([`specular_render::StainView`], the arithmetic of [`specular_pathology::stain`]), and the
//! annotations a slide carries, imported or drawn, gathered for export
//! ([`super::annot_export`]).
//!
//! - "Separate stains" steps off → H&E → H-DAB → H&E-DAB → off; each stain shows or hides
//!   on its own (and through the channel keys on a slide in science mode), as light or as
//!   optical density. The legend in the view's corner lists them; a click hides one.
//! - "Estimate stains from region" runs Macenko's method on the last area drawn with the
//!   measuring tools, over the best overview in memory; "Normalise" maps the slide's stains
//!   onto the reference (the standard H&E one, or a region chosen as the reference). Display
//!   only: the pixels, the exports and the measurements are untouched.
//! - The set, the shown stains, the mode, normalisation and the reference stay across files;
//!   an estimate belongs to its file.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crossbeam_channel::{Receiver, Sender};
use specular_core::{PixelLayout, Raster};
use specular_gfx::{Align, DrawList, Rect, Text, TextStyle};
use specular_i18n::t;
use specular_input::Action;
use specular_measure::geom::{P, Shape};
use specular_pathology::annot::{Annotation, Class, Geometry, Poly};
use specular_pathology::stain::{
    self, Estimate, EstimateError, Mode, Reference, Stain, StainSet, Stains,
};
use specular_render::StainView;
use specular_theme::metrics::{FONT_LABEL, FONT_SMALL, RADIUS_MD, RADIUS_SM};
use specular_theme::{Color, Palette};
use specular_widgets::accesskit;

use super::App;

#[cfg(test)]
mod shots;

const A11Y_BASE: u64 = 0x5D00_0000_0000_0000;
/// Most pixels an estimate reads (a larger region is sampled evenly).
const MAX_SAMPLES: usize = 1 << 20;
/// Points of an exported ellipse's outline.
const ELLIPSE_SEGMENTS: usize = 72;

/// What finished in the background.
pub(super) enum Done {
    Imported(PathBuf, Result<(Vec<Annotation>, usize), String>),
}

/// The pathology tools' state.
pub(super) struct PathologyState {
    pub set: Option<StainSet>,
    pub visible: [bool; 3],
    pub mode: Mode,
    pub normalise: bool,
    /// The user's reference (`None`: the standard H&E reference).
    pub reference: Option<Reference>,
    /// The region estimate and the file it was made on.
    estimate: Option<(PathBuf, Estimate)>,
    /// Annotations imported, by image file.
    pub imported: HashMap<PathBuf, Vec<Annotation>>,
    /// The same as drawn (sidecar shapes), made once per import.
    drawn: HashMap<PathBuf, Arc<specular_decode::science::annot::Annotations>>,
    /// The legend as last drawn.
    pub legend: Option<LegendDrawn>,
    tx: Sender<Done>,
    rx: Receiver<Done>,
}

impl Default for PathologyState {
    fn default() -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        PathologyState {
            set: None,
            visible: [true, false, false],
            mode: Mode::Intensity,
            normalise: false,
            reference: None,
            estimate: None,
            imported: HashMap::new(),
            drawn: HashMap::new(),
            legend: None,
            tx,
            rx,
        }
    }
}

impl std::fmt::Debug for PathologyState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PathologyState")
            .field("set", &self.set)
            .field("visible", &self.visible)
            .finish_non_exhaustive()
    }
}

/// A stain's name.
pub(super) fn stain_name(s: Stain) -> String {
    match s {
        Stain::Hematoxylin => t!("stain-hematoxylin"),
        Stain::Eosin => t!("stain-eosin"),
        Stain::Dab => t!("stain-dab"),
        Stain::Residual => t!("stain-residual"),
    }
}

/// A set's name.
pub(super) fn set_name(s: StainSet) -> String {
    match s {
        StainSet::He => t!("stain-set-he"),
        StainSet::Hdab => t!("stain-set-hdab"),
        StainSet::HeDab => t!("stain-set-hedab"),
    }
}

/// The view the GPU draws for `d` with `visible` stains in `mode`.
pub(super) fn view_of(d: &stain::Display, visible: [bool; 3], mode: Mode) -> StainView {
    let f = |v: f64| v as f32;
    StainView {
        // The shader's rows are the unmixing matrix's columns.
        unmix: [0, 1, 2].map(|s| [0, 1, 2].map(|k| f(d.unmix[k][s]))),
        recon: d.recon.map(|r| r.map(f)),
        tint: stain::tints(d, visible).map(|r| r.map(f)),
        visible,
        od: mode == Mode::Od,
        od_max: stain::OD_MAX as f32,
    }
}

/// One stain in the legend.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct LegendRow {
    pub name: String,
    /// sRGB of the swatch.
    pub colour: [u8; 3],
    pub shown: bool,
}

/// What the legend draws.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct LegendView {
    pub title: String,
    pub rows: Vec<LegendRow>,
    /// Where the vectors come from.
    pub note: String,
}

/// Where the legend landed.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct LegendDrawn {
    pub panel: Rect,
    pub rows: Vec<Rect>,
}

/// The legend for a set, its vectors and what is shown.
pub(super) fn legend_view(
    set: StainSet,
    stains: &Stains,
    visible: [bool; 3],
    mode: Mode,
    normalised: bool,
) -> LegendView {
    let title = match mode {
        Mode::Intensity => t!("stain-legend-light", set = set_name(set)),
        Mode::Od => t!("stain-legend-od", set = set_name(set)),
    };
    let rows = set
        .stains()
        .iter()
        .enumerate()
        .map(|(k, s)| {
            let colour = if *s == Stain::Residual {
                [160, 160, 160]
            } else {
                stains.vectors[k].map(|x| (10f64.powf(-x) * 255.0).round().clamp(0.0, 255.0) as u8)
            };
            LegendRow {
                name: stain_name(*s),
                colour,
                shown: visible[k],
            }
        })
        .collect();
    let note = if normalised {
        t!("stain-legend-normalised")
    } else if stains.estimated {
        t!("stain-legend-estimated")
    } else {
        t!("stain-legend-published")
    };
    LegendView { title, rows, note }
}

/// Draws the legend in the bottom end corner of `area` (logical px).
pub(super) fn draw_legend(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    v: &LegendView,
    area: Rect,
    rtl: bool,
    ts: f32,
) -> LegendDrawn {
    let title_st = TextStyle::new(FONT_LABEL * ts).weight(600.0);
    let row_st = TextStyle::new(FONT_LABEL * ts);
    let note_st = TextStyle::new(FONT_SMALL * ts);
    let pad = 10.0;
    let title = text.layout(&v.title, title_st, None, Align::Start);
    let note = text.layout(&v.note, note_st, None, Align::Start);
    let rows: Vec<_> = v
        .rows
        .iter()
        .map(|r| text.layout(&r.name, row_st, None, Align::Start))
        .collect();
    let row_h = (rows.iter().map(|b| b.height).fold(0.0, f32::max) + 8.0).max(24.0);
    let sw = 14.0;
    let check = 16.0;
    let inner = rows
        .iter()
        .map(|b| check + 8.0 + sw + 8.0 + b.width)
        .fold(title.width.max(note.width), f32::max);
    let w = (inner + 2.0 * pad).min(area.w - 32.0).max(120.0);
    let h = pad + title.height + 6.0 + row_h * rows.len() as f32 + 4.0 + note.height + pad;
    let x = area.x + area.w - w - 16.0;
    let y = area.y + area.h - h - 56.0;
    let panel = Rect::new(x.max(area.x), y.max(area.y), w, h);
    let m = |r: Rect| {
        if rtl {
            r.mirror_x(area.x * 2.0 + area.w)
        } else {
            r
        }
    };
    let pm = m(panel);
    dl.shadow(pm, RADIUS_MD, pal.shadow, 18.0, 6.0);
    dl.rect_border(pm, RADIUS_MD, pal.surface, pal.glass_border, 1.0);
    // A text block's x at `x` (start edge, LTR coordinates).
    let sx = |x: f32, bw: f32| {
        if rtl {
            area.x * 2.0 + area.w - x - bw
        } else {
            x
        }
    };
    text.draw(
        dl,
        &title,
        sx(panel.x + pad, title.width),
        panel.y + pad,
        pal.text,
    );
    let mut out = LegendDrawn {
        panel: pm,
        rows: Vec::new(),
    };
    let top = panel.y + pad + title.height + 6.0;
    for (i, (rv, b)) in v.rows.iter().zip(&rows).enumerate() {
        let r = Rect::new(
            panel.x + pad - 4.0,
            top + i as f32 * row_h,
            w - 2.0 * pad + 8.0,
            row_h,
        );
        let cy = r.y + row_h * 0.5;
        // The check box: filled when shown.
        let cb = Rect::new(r.x + 4.0, cy - check * 0.5, check, check);
        let cbm = m(cb);
        if rv.shown {
            dl.rect(cbm, RADIUS_SM, pal.accent_fill);
            let ink = pal.accent_text;
            let (a, bb, c) = (
                (cbm.x + check * 0.25, cbm.y + check * 0.52),
                (cbm.x + check * 0.43, cbm.y + check * 0.7),
                (cbm.x + check * 0.76, cbm.y + check * 0.32),
            );
            dl.line(a, bb, 1.8, ink);
            dl.line(bb, c, 1.8, ink);
        } else {
            dl.rect_border(cbm, RADIUS_SM, Color::TRANSPARENT, pal.text_muted, 1.5);
        }
        let swatch = Rect::new(cb.right() + 8.0, cy - sw * 0.5, sw, sw);
        let c = rv.colour;
        dl.rect_border(
            m(swatch),
            3.0,
            Color::rgba(
                f32::from(c[0]) / 255.0,
                f32::from(c[1]) / 255.0,
                f32::from(c[2]) / 255.0,
                1.0,
            ),
            pal.separator,
            1.0,
        );
        let ink = if rv.shown { pal.text } else { pal.text_muted };
        text.draw(
            dl,
            b,
            sx(swatch.right() + 8.0, b.width),
            cy - b.height * 0.5,
            ink,
        );
        out.rows.push(m(r));
    }
    let ny = top + row_h * rows.len() as f32 + 4.0;
    text.draw(dl, &note, sx(panel.x + pad, note.width), ny, pal.text_muted);
    out
}

fn bounds(r: Rect, scale: f64) -> accesskit::Rect {
    accesskit::Rect {
        x0: f64::from(r.x) * scale,
        y0: f64::from(r.y) * scale,
        x1: f64::from(r.x + r.w) * scale,
        y1: f64::from(r.y + r.h) * scale,
    }
}

/// The legend's AccessKit nodes: a group of check boxes, one per stain.
pub(super) fn legend_a11y(
    tree: &mut accesskit::TreeUpdate,
    v: &LegendView,
    d: &LegendDrawn,
    scale: f64,
) {
    use accesskit::{Node, NodeId, Role, Toggled};
    let mut group = Node::new(Role::Group);
    group.set_label(v.title.clone());
    group.set_description(v.note.clone());
    group.set_bounds(bounds(d.panel, scale));
    let mut ids = Vec::new();
    for (i, (rv, r)) in v.rows.iter().zip(&d.rows).enumerate() {
        let id = NodeId(A11Y_BASE + 1 + i as u64);
        let mut n = Node::new(Role::CheckBox);
        n.set_label(rv.name.clone());
        n.set_toggled(if rv.shown {
            Toggled::True
        } else {
            Toggled::False
        });
        n.set_bounds(bounds(*r, scale));
        n.add_action(accesskit::Action::Click);
        n.add_action(accesskit::Action::Focus);
        ids.push(id);
        tree.nodes.push((id, n));
    }
    group.set_children(ids);
    if let Some((_, root)) = tree.nodes.iter_mut().find(|(id, _)| *id == NodeId(1)) {
        root.push_child(NodeId(A11Y_BASE));
    }
    tree.nodes.push((NodeId(A11Y_BASE), group));
}

/// 8-bit sRGB of pixel (x, y) of `r` (its own coordinates), for the layouts slides come in.
fn rgb_at(r: &Raster, x: u32, y: u32) -> Option<[u8; 3]> {
    let (x, y) = (x as usize, y as usize);
    let at = |p: usize, x: usize, y: usize, bpp: usize, k: usize| -> Option<u8> {
        let pl = r.planes.get(p)?;
        pl.data.get(y * pl.stride + x * bpp + k).copied()
    };
    match r.layout {
        PixelLayout::Rgba => Some([at(0, x, y, 4, 0)?, at(0, x, y, 4, 1)?, at(0, x, y, 4, 2)?]),
        PixelLayout::Rgb => Some([at(0, x, y, 1, 0)?, at(1, x, y, 1, 0)?, at(2, x, y, 1, 0)?]),
        PixelLayout::YCbCr => {
            let pl = |i: usize| r.planes.get(i);
            let (cb, cr) = (pl(1)?, pl(2)?);
            let (hx, vy) = (usize::from(cb.h_div.max(1)), usize::from(cb.v_div.max(1)));
            let yy = f32::from(at(0, x, y, 1, 0)?);
            let u = f32::from(*cb.data.get((y / vy) * cb.stride + x / hx)?) - 128.0;
            let v = f32::from(*cr.data.get((y / vy) * cr.stride + x / hx)?) - 128.0;
            let c = |f: f32| f.round().clamp(0.0, 255.0) as u8;
            Some([
                c(yy + 1.402 * v),
                c(yy - 0.344_136 * u - 0.714_136 * v),
                c(yy + 1.772 * u),
            ])
        }
        PixelLayout::Rgba16(_) => {
            let pl = r.planes.first()?;
            let o = y * pl.stride + x * 8;
            let ch = |k: usize| -> Option<u8> {
                let b = pl.data.get(o + 2 * k..o + 2 * k + 2)?;
                Some((u16::from_ne_bytes([b[0], b[1]]) >> 8) as u8)
            };
            Some([ch(0)?, ch(1)?, ch(2)?])
        }
        _ => None,
    }
}

/// The pixels of `shape` (full-resolution stored pixels) in `r`, an overview at 1/`r.scale`:
/// those whose centres are inside, sampled evenly past [`MAX_SAMPLES`].
pub(super) fn region_pixels(r: &Raster, shape: &Shape) -> Vec<[u8; 3]> {
    let s = f64::from(r.scale.max(1));
    let (x0, y0, x1, y1) = shape.bounds();
    let (ox, oy) = (f64::from(r.origin.0), f64::from(r.origin.1));
    let i0 = ((x0 / s - ox).floor().max(0.0)) as u32;
    let j0 = ((y0 / s - oy).floor().max(0.0)) as u32;
    let i1 = ((x1 / s - ox).ceil().max(0.0) as u32).min(r.width);
    let j1 = ((y1 / s - oy).ceil().max(0.0) as u32).min(r.height);
    if i1 <= i0 || j1 <= j0 {
        return Vec::new();
    }
    let n = u64::from(i1 - i0) * u64::from(j1 - j0);
    let step = ((n as f64 / MAX_SAMPLES as f64).sqrt().ceil() as u32).max(1);
    let mut out = Vec::new();
    for j in (j0..j1).step_by(step as usize) {
        for i in (i0..i1).step_by(step as usize) {
            let c = P {
                x: (f64::from(i) + ox + 0.5) * s,
                y: (f64::from(j) + oy + 0.5) * s,
            };
            if shape.contains(c)
                && let Some(px) = rgb_at(r, i, j)
            {
                out.push(px);
            }
        }
    }
    out
}

/// A measurement's shape as an annotation geometry (`None` for lengths and angles).
pub(super) fn geometry_of(s: &Shape) -> Option<Geometry> {
    let pt = |p: &P| (p.x, p.y);
    Some(match s {
        Shape::Point(p) => Geometry::Point(pt(p)),
        &Shape::Rect { x0, y0, x1, y1 } => Geometry::rect((x0, y0), (x1, y1)),
        Shape::Ellipse { .. } => Geometry::polygon(s.outline().iter().map(pt).collect()),
        Shape::Polygon(v) | Shape::Freehand(v) => Geometry::polygon(v.iter().map(pt).collect()),
        Shape::Polyline(v) => Geometry::Line(v.iter().map(pt).collect()),
        Shape::Line(a, b) => Geometry::Line(vec![pt(a), pt(b)]),
        Shape::Angle { .. } => return None,
    })
}

/// A slide sidecar's annotation as an exported one: its group is its class.
pub(super) fn from_sidecar(a: &specular_decode::science::annot::Annotation) -> Option<Annotation> {
    use specular_decode::science::annot::Shape as S;
    let pts = a.points.clone();
    let geometry = match a.shape {
        S::Polygon => Geometry::polygon(pts),
        S::Rectangle => {
            let (x0, y0, x1, y1) = a.bounds();
            Geometry::rect((x0, y0), (x1, y1))
        }
        S::Ellipse => Geometry::polygon(a.outline(ELLIPSE_SEGMENTS)),
        S::Point => Geometry::Point(*pts.first()?),
        S::Points => Geometry::Points(pts),
        S::Polyline | S::Ruler | S::Arrow => Geometry::Line(pts),
    };
    let class = (!a.group.trim().is_empty()).then(|| Class {
        name: a.group.trim().to_owned(),
        colour: a.colour,
    });
    let g = Annotation {
        name: a.name.clone(),
        class,
        geometry,
    };
    g.geometry.is_valid().then_some(g)
}

/// An imported annotation drawn like a sidecar's.
pub(super) fn to_sidecar(a: &Annotation) -> specular_decode::science::annot::Annotation {
    use specular_decode::science::annot::{Annotation as A, Shape as S};
    let (shape, points) = match &a.geometry {
        Geometry::Area(ps) => (
            S::Polygon,
            ps.first().map(|p| p.exterior.clone()).unwrap_or_default(),
        ),
        &Geometry::Rect { x0, y0, x1, y1 } => {
            (S::Rectangle, vec![(x0, y0), (x1, y0), (x1, y1), (x0, y1)])
        }
        Geometry::Point(p) => (S::Point, vec![*p]),
        Geometry::Points(v) => (S::Points, v.clone()),
        Geometry::Line(v) => (S::Polyline, v.clone()),
    };
    A {
        name: a.name.clone(),
        group: a.class_name().unwrap_or_default().to_owned(),
        shape,
        colour: a.class.as_ref().and_then(|c| c.colour),
        points,
    }
}

/// Other parts of a multi-part area, drawn as their own outlines.
pub(super) fn extra_parts(a: &Annotation) -> Vec<specular_decode::science::annot::Annotation> {
    let Geometry::Area(ps) = &a.geometry else {
        return Vec::new();
    };
    ps.iter()
        .skip(1)
        .map(|p: &Poly| specular_decode::science::annot::Annotation {
            points: p.exterior.clone(),
            ..to_sidecar(a)
        })
        .collect()
}

impl App {
    /// The current image's file.
    fn pathology_path(&self) -> Option<PathBuf> {
        self.current.and_then(|i| self.entries.get(i)).cloned()
    }

    /// The current image can be stain-separated: a colour image drawn by the image shader.
    pub(super) fn stain_capable(&self) -> bool {
        if self.medical_state().is_some()
            || self.mpr_active()
            || self.cad_active()
            || self.mesh_state()
        {
            return false;
        }
        self.current_id()
            .and_then(|id| self.images.get(&id))
            .and_then(|i| i.overview.as_ref())
            .is_some_and(|(_, r)| !r.layout.is_gray() && !matches!(r.layout, PixelLayout::Yuv(_)))
    }

    /// The stains in use: the region's estimate when it was made on this file with a
    /// two-stain set, else the published vectors.
    fn stain_stains(&self, set: StainSet) -> Stains {
        let path = self.pathology_path();
        match &self.pathology.estimate {
            Some((p, e)) if Some(p) == path.as_ref() => {
                Stains::with_estimate(set, e.vectors).unwrap_or_else(|| Stains::published(set))
            }
            _ => Stains::published(set),
        }
    }

    /// The normalisation in effect: the slide's own estimate and the reference.
    fn stain_norm(&self) -> Option<(Estimate, Reference)> {
        if !self.pathology.normalise {
            return None;
        }
        let path = self.pathology_path()?;
        let (p, e) = self.pathology.estimate.as_ref()?;
        (p == &path).then(|| {
            (
                *e,
                self.pathology
                    .reference
                    .unwrap_or_else(Reference::standard_he),
            )
        })
    }

    /// What the GPU draws this frame (`None`: off or not a colour image).
    pub(super) fn stain_view(&self) -> Option<StainView> {
        let set = self.pathology.set?;
        if !self.stain_capable() {
            return None;
        }
        let stains = self.stain_stains(set);
        let norm = self.stain_norm();
        let d = stain::display(&stains, norm.as_ref().map(|(e, r)| (e, r)))?;
        Some(view_of(&d, self.pathology.visible, self.pathology.mode))
    }

    /// The legend while stains are separated.
    pub(super) fn stain_legend(&self) -> Option<LegendView> {
        let set = self.pathology.set?;
        if !self.stain_capable() {
            return None;
        }
        let stains = self.stain_stains(set);
        Some(legend_view(
            set,
            &stains,
            self.pathology.visible,
            self.pathology.mode,
            self.stain_norm().is_some(),
        ))
    }

    /// The last area drawn with the measuring tools on this image.
    pub(super) fn stain_region(&self) -> Option<Shape> {
        self.measure_items()
            .iter()
            .rev()
            .find(|m| m.shape.is_area())
            .map(|m| m.shape.clone())
    }

    fn stain_toast(&mut self, msg: String) {
        let now = self.now();
        self.chrome.toast(msg, None, now);
        self.scene = None;
        self.redraw = true;
    }

    /// The pixels of the last drawn region, from the best overview in memory.
    fn stain_region_pixels(&self) -> Option<Vec<[u8; 3]>> {
        let shape = self.stain_region()?;
        let img = self.current_id().and_then(|id| self.images.get(&id))?;
        let (_, r) = img.overview.as_ref()?;
        let px = region_pixels(r, &shape);
        (!px.is_empty()).then_some(px)
    }

    fn stain_estimate(&mut self) -> Option<Estimate> {
        let set = self.pathology.set?;
        if set == StainSet::HeDab {
            self.stain_toast(t!(
                "stain-estimate-failed",
                reason = t!("stain-estimate-three")
            ));
            return None;
        }
        let Some(px) = self.stain_region_pixels() else {
            self.stain_toast(t!(
                "stain-estimate-failed",
                reason = t!("stain-estimate-no-pixels")
            ));
            return None;
        };
        match stain::macenko(&px) {
            Ok(e) => Some(e),
            Err(err) => {
                let reason = match err {
                    EstimateError::TooFewPixels(_) => t!("stain-estimate-background"),
                    EstimateError::Degenerate => t!("stain-estimate-one-colour"),
                };
                self.stain_toast(t!("stain-estimate-failed", reason = reason));
                None
            }
        }
    }

    /// The channel keys (1–3) show or hide stains while they are separated.
    pub(super) fn stain_channel_key(&mut self, a: Action) -> bool {
        let Action::ScienceChannel(n @ 1..=3) = a else {
            return false;
        };
        if self.pathology.set.is_none() || !self.stain_capable() {
            return false;
        }
        self.stain_toggle(usize::from(n) - 1);
        true
    }

    fn stain_toggle(&mut self, k: usize) {
        let Some(set) = self.pathology.set else {
            return;
        };
        self.pathology.visible[k] = !self.pathology.visible[k];
        let name = stain_name(set.stains()[k]);
        let msg = if self.pathology.visible[k] {
            t!("stain-shown", stain = name)
        } else {
            t!("stain-hidden", stain = name)
        };
        self.stain_toast(msg);
    }

    /// The stain actions; true when `a` was one.
    pub(super) fn stain_action(&mut self, a: Action) -> bool {
        match a {
            Action::StainSeparation => {
                let next = match self.pathology.set {
                    None => Some(StainSet::He),
                    Some(StainSet::He) => Some(StainSet::Hdab),
                    Some(StainSet::Hdab) => Some(StainSet::HeDab),
                    Some(StainSet::HeDab) => None,
                };
                self.pathology.set = next;
                let msg = match next {
                    Some(s) => t!("stain-on", set = set_name(s)),
                    None => t!("stain-off"),
                };
                self.stain_toast(msg);
            }
            Action::StainShow1 => self.stain_toggle(0),
            Action::StainShow2 => self.stain_toggle(1),
            Action::StainShow3 => self.stain_toggle(2),
            Action::StainMode => {
                self.pathology.mode = match self.pathology.mode {
                    Mode::Intensity => Mode::Od,
                    Mode::Od => Mode::Intensity,
                };
                let msg = match self.pathology.mode {
                    Mode::Intensity => t!("stain-mode-light"),
                    Mode::Od => t!("stain-mode-od"),
                };
                self.stain_toast(msg);
            }
            Action::StainEstimate => {
                if let (Some(e), Some(path)) = (self.stain_estimate(), self.pathology_path()) {
                    self.pathology.estimate = Some((path, e));
                    self.stain_toast(t!("stain-estimated", pixels = e.pixels as i64));
                }
            }
            Action::StainReference => {
                if let Some(e) = self.stain_estimate() {
                    self.pathology.reference = Some(Reference::of(&e));
                    self.stain_toast(t!("stain-reference-set"));
                }
            }
            Action::StainNormalise => {
                self.pathology.normalise = !self.pathology.normalise;
                // Normalising needs the slide's own stains: estimated from the region, else
                // from the whole overview.
                if self.pathology.normalise
                    && self.stain_norm().is_none()
                    && let (Some(set), Some(path)) = (self.pathology.set, self.pathology_path())
                    && set != StainSet::HeDab
                    && let Some(e) = self.stain_whole_estimate()
                {
                    self.pathology.estimate = Some((path, e));
                }
                let msg = if self.pathology.normalise {
                    t!("stain-normalise-on")
                } else {
                    t!("stain-normalise-off")
                };
                self.stain_toast(msg);
            }
            Action::StainReset => {
                self.pathology.estimate = None;
                self.pathology.reference = None;
                self.pathology.normalise = false;
                self.stain_toast(t!("stain-reset-done"));
            }
            _ => return false,
        }
        true
    }

    /// An estimate over the whole overview (normalising a slide no region was drawn on).
    fn stain_whole_estimate(&self) -> Option<Estimate> {
        let img = self.current_id().and_then(|id| self.images.get(&id))?;
        let (_, r) = img.overview.as_ref()?;
        let s = f64::from(r.scale.max(1));
        let whole = Shape::rect(
            P { x: 0.0, y: 0.0 },
            P {
                x: f64::from(r.width) * s,
                y: f64::from(r.height) * s,
            },
        );
        stain::macenko(&region_pixels(r, &whole)).ok()
    }

    /// A click on the legend shows or hides a stain; true when it was on the legend.
    pub(super) fn stain_click(&mut self, lp: (f32, f32), down: bool) -> bool {
        let Some(d) = self.pathology.legend.clone() else {
            return false;
        };
        if !d.panel.contains(lp) {
            return false;
        }
        if down && let Some(k) = d.rows.iter().position(|r| r.contains(lp)) {
            self.stain_toggle(k);
        }
        true
    }

    pub(super) fn stain_a11y_request(&mut self, req: &accesskit::ActionRequest) -> bool {
        let Some(k) = req.target_node.0.checked_sub(A11Y_BASE + 1) else {
            return false;
        };
        if k >= 3 || self.pathology.legend.is_none() {
            return false;
        }
        if req.action == accesskit::Action::Click {
            self.stain_toggle(k as usize);
        }
        true
    }

    /// Every annotation of the current image: the sidecar's, the imported ones, then the
    /// regions and points drawn with the measuring tools.
    pub(super) fn pathology_annotations(&self) -> Vec<Annotation> {
        let mut out = Vec::new();
        if let Some(s) = self.science_summary_current()
            && let Some(a) = &s.annotations
        {
            out.extend(a.set.items.iter().filter_map(from_sidecar));
        }
        if let Some(p) = self.pathology_path()
            && let Some(v) = self.pathology.imported.get(&p)
        {
            out.extend(v.iter().cloned());
        }
        for m in self.measure_items() {
            if let Some(geometry) = geometry_of(&m.shape) {
                out.push(Annotation {
                    name: m.label.clone(),
                    class: None,
                    geometry,
                });
            }
        }
        out
    }

    /// Annotations were imported onto the current image.
    pub(super) fn pathology_has_imported(&self) -> bool {
        self.pathology_path()
            .and_then(|p| self.pathology.imported.get(&p))
            .is_some_and(|v| !v.is_empty())
    }

    /// There is something to export (cheap: nothing is gathered).
    pub(super) fn pathology_has_annotations(&self) -> bool {
        self.science_summary_current()
            .and_then(|s| s.annotations.as_ref())
            .is_some_and(|a| !a.set.items.is_empty())
            || self
                .pathology_path()
                .and_then(|p| self.pathology.imported.get(&p))
                .is_some_and(|v| !v.is_empty())
            || self
                .measure_items()
                .iter()
                .any(|m| !matches!(m.shape, Shape::Angle { .. }))
    }

    /// The imported annotations drawn over the image, as sidecar shapes.
    pub(super) fn pathology_imported_set(
        &self,
    ) -> Option<Arc<specular_decode::science::annot::Annotations>> {
        if self.science.annotations_hidden {
            return None;
        }
        let p = self.pathology_path()?;
        self.pathology
            .drawn
            .get(&p)
            .filter(|a| !a.items.is_empty())
            .cloned()
    }

    /// Imports QuPath GeoJSON or COCO annotations onto the current image.
    pub(super) fn pathology_import(&mut self) {
        let Some(path) = self.pathology_path() else {
            return;
        };
        let dir = path.parent().map(Path::to_path_buf);
        let tx = self.pathology.tx.clone();
        self.loader.background("annot-import", move |l| {
            let mut dlg = rfd::FileDialog::new()
                .add_filter(t!("annot-file-geojson"), &["geojson", "json"])
                .add_filter(t!("annot-file-coco"), &["json"]);
            if let Some(d) = dir {
                dlg = dlg.set_directory(d);
            }
            let Some(p) = dlg.pick_file() else { return };
            let _ = tx.send(Done::Imported(path, import_file(&p)));
            l.waker()();
        });
    }

    /// Applies finished background work.
    pub(super) fn pathology_tick(&mut self) {
        while let Ok(d) = self.pathology.rx.try_recv() {
            match d {
                Done::Imported(path, Ok((items, skipped))) => {
                    let n = items.len();
                    let all = self.pathology.imported.entry(path.clone()).or_default();
                    all.extend(items);
                    let mut shapes = Vec::new();
                    for a in all.iter() {
                        shapes.push(to_sidecar(a));
                        shapes.extend(extra_parts(a));
                    }
                    self.pathology.drawn.insert(
                        path,
                        Arc::new(specular_decode::science::annot::Annotations {
                            format: specular_decode::science::annot::Format::Asap,
                            items: shapes,
                            skipped: 0,
                        }),
                    );
                    let mut msg = t!("annot-import-done", count = n as i64);
                    if skipped > 0 {
                        msg = format!(
                            "{msg}. {}",
                            t!("annot-import-skipped", count = skipped as i64)
                        );
                    }
                    self.stain_toast(msg);
                }
                Done::Imported(_, Err(e)) => {
                    self.stain_toast(t!("annot-import-failed", reason = e))
                }
            }
        }
    }
}

/// Reads an annotation file: COCO when it has COCO's members, else GeoJSON.
pub(super) fn import_file(p: &Path) -> Result<(Vec<Annotation>, usize), String> {
    const MAX: u64 = 256 << 20;
    let m = std::fs::metadata(p).map_err(|e| crate::error_text::io_reason(&e))?;
    if m.len() > MAX {
        return Err(t!("measure-tool-too-large"));
    }
    let text = std::fs::read_to_string(p).map_err(|e| crate::error_text::io_reason(&e))?;
    import_text(&text)
}

/// COCO or GeoJSON text as annotations, and the shapes left out.
pub(super) fn import_text(text: &str) -> Result<(Vec<Annotation>, usize), String> {
    match specular_pathology::coco::read(text) {
        Ok(r) => Ok((r.items, r.skipped + r.other_images)),
        Err(specular_pathology::coco::Error::NotCoco) => specular_pathology::qupath::read(text)
            .map(|r| (r.items, r.skipped))
            .map_err(|e| e.to_string()),
        Err(e) => Err(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measurements_and_sidecars_become_annotations() {
        let r = Shape::rect(P { x: 1.0, y: 2.0 }, P { x: 5.0, y: 7.0 });
        assert_eq!(
            geometry_of(&r),
            Some(Geometry::rect((1.0, 2.0), (5.0, 7.0)))
        );
        assert!(
            geometry_of(&Shape::Angle {
                a: P { x: 0.0, y: 0.0 },
                vertex: P { x: 1.0, y: 0.0 },
                b: P { x: 1.0, y: 1.0 }
            })
            .is_none()
        );
        let e = Shape::Ellipse {
            cx: 10.0,
            cy: 10.0,
            rx: 4.0,
            ry: 2.0,
        };
        let g = geometry_of(&e).unwrap();
        assert!((g.area() - std::f64::consts::PI * 8.0).abs() < 0.1);
        use specular_decode::science::annot::{Annotation as A, Shape as S};
        let side = A {
            name: "n".into(),
            group: "Tumor".into(),
            shape: S::Rectangle,
            colour: Some([1, 2, 3]),
            points: vec![(0.0, 0.0), (4.0, 0.0), (4.0, 3.0), (0.0, 3.0)],
        };
        let a = from_sidecar(&side).unwrap();
        assert_eq!(a.class_name(), Some("Tumor"));
        assert_eq!(a.geometry, Geometry::rect((0.0, 0.0), (4.0, 3.0)));
        let back = to_sidecar(&a);
        assert_eq!(back.group, "Tumor");
        assert_eq!(back.shape, S::Rectangle);
    }

    #[test]
    fn imports_tell_coco_from_geojson() {
        let geo = r#"{"type":"FeatureCollection","features":[{"type":"Feature","geometry":{"type":"Point","coordinates":[1,2]},"properties":{"classification":{"name":"Cell"}}}]}"#;
        let (items, skipped) = import_text(geo).unwrap();
        assert_eq!((items.len(), skipped), (1, 0));
        let coco = r#"{"images":[{"id":1,"width":10,"height":10}],"categories":[{"id":1,"name":"c"}],"annotations":[{"id":1,"image_id":1,"category_id":1,"bbox":[1,1,2,2]}]}"#;
        let (items, _) = import_text(coco).unwrap();
        assert_eq!(items[0].geometry, Geometry::rect((1.0, 1.0), (3.0, 3.0)));
        assert!(import_text("[1,2").is_err());
    }

    #[test]
    fn region_pixels_follow_the_overview_scale() {
        // A 4×4 overview at 1/2 of an 8×8 image: red left half, blue right half.
        let mut data = Vec::new();
        for _y in 0..4 {
            for x in 0..4 {
                data.extend_from_slice(if x < 2 {
                    &[255, 0, 0, 255]
                } else {
                    &[0, 0, 255, 255]
                });
            }
        }
        let r = Raster {
            width: 4,
            height: 4,
            origin: (0, 0),
            scale: 2,
            layout: PixelLayout::Rgba,
            planes: vec![specular_core::RasterPlane {
                width: 4,
                height: 4,
                stride: 16,
                data,
                h_div: 1,
                v_div: 1,
            }],
            truncated: false,
        };
        let left = Shape::rect(P { x: 0.0, y: 0.0 }, P { x: 4.0, y: 8.0 });
        let px = region_pixels(&r, &left);
        assert_eq!(px.len(), 8);
        assert!(px.iter().all(|p| *p == [255, 0, 0]));
        let right = Shape::rect(P { x: 4.0, y: 0.0 }, P { x: 8.0, y: 4.0 });
        assert!(region_pixels(&r, &right).iter().all(|p| *p == [0, 0, 255]));
    }
}
