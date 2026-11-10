//! What MPR draws over the GPU planes, in logical pixels: the 2 × 2 layout (axial, coronal,
//! sagittal, and the 3D view in the fourth slot), the crosshair lines in each view's
//! partners' colours with their rotation handles, orientation letters at each view's edges,
//! a readout per view, measurements, streamlines near each plane, and over the 3D view the controls (slab mode,
//! thickness, reset, close; the 3D preset, rendering, clipping and STL export) and the
//! panel's lines, with their accessibility nodes.

use specular_gfx::{Align, DrawList, Rect, Text, TextStyle};
use specular_i18n::t;
use specular_render_volume::Slab;
use specular_theme::metrics::{FONT_BODY, GAP, PAD, RADIUS_SM, TOOLBAR_MARGIN};
use specular_theme::{Color, Palette};
use specular_widgets::accesskit;

use super::super::medical::{self, labels, measure};
use super::load::Length;
use super::views::{KINDS, R, Views};

/// The views' and the fourth slot's rectangles (logical pixels, on whole physical pixels).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Layout {
    pub views: [R; 3],
    pub slot: R,
}

/// Splits the viewport `vp` (logical) into a 2 × 2 grid with a one-pixel gap: axial and
/// coronal on top, sagittal and the 3D slot below; the columns swap in right-to-left
/// layouts (the images themselves never mirror).
pub(crate) fn layout(vp: R, scale: f64, rtl: bool) -> Layout {
    let s = scale.max(0.1);
    let x0 = (vp[0] * s).round();
    let y0 = (vp[1] * s).round();
    let w = (vp[2] * s).round().max(3.0);
    let h = (vp[3] * s).round().max(3.0);
    let gap = s.round().max(1.0);
    let cw = ((w - gap) / 2.0).floor();
    let ch = ((h - gap) / 2.0).floor();
    let cols = [(x0, cw), (x0 + cw + gap, w - cw - gap)];
    let rows = [(y0, ch), (y0 + ch + gap, h - ch - gap)];
    let cell = |c: usize, r: usize| {
        let c = if rtl { 1 - c } else { c };
        let (x, cw) = cols[c];
        let (y, rh) = rows[r];
        [x / s, y / s, cw / s, rh / s]
    };
    Layout {
        views: [cell(0, 0), cell(1, 0), cell(0, 1)],
        slot: cell(1, 1),
    }
}

/// The layout with view `v` maximised: it fills the viewport but for a column at the end
/// (the start in right-to-left layouts) that keeps the controls and the 3D view; the
/// other views are empty rectangles.
pub(crate) fn layout_maximised(vp: R, scale: f64, rtl: bool, v: usize) -> Layout {
    let s = scale.max(0.1);
    let x0 = (vp[0] * s).round();
    let y0 = (vp[1] * s).round();
    let w = (vp[2] * s).round().max(3.0);
    let h = (vp[3] * s).round().max(3.0);
    let gap = s.round().max(1.0);
    // A quarter of the width, at least the 3D view's least height across.
    let side = (w / 4.0).max(MIN_SLOT * s).min((w - gap) / 2.0).floor();
    let main = w - side - gap;
    let (mx, sx) = if rtl {
        (x0 + side + gap, x0)
    } else {
        (x0, x0 + main + gap)
    };
    let big = [mx / s, y0 / s, main / s, h / s];
    let empty = [mx / s, y0 / s, 0.0, 0.0];
    let mut views = [empty; 3];
    views[v.min(2)] = big;
    Layout {
        views,
        slot: [sx / s, y0 / s, side / s, h / s],
    }
}

/// The controls' column beside a maximised view is at least this wide (logical px).
const MIN_SLOT: f64 = 240.0;

/// The rectangle has no area (a view hidden while another is maximised).
pub(crate) fn empty(r: R) -> bool {
    r[2] <= 0.0 || r[3] <= 0.0
}

pub(crate) fn rect(r: R) -> Rect {
    Rect::new(r[0] as f32, r[1] as f32, r[2] as f32, r[3] as f32)
}

pub(crate) fn contains(r: R, p: (f64, f64)) -> bool {
    p.0 >= r[0] && p.1 >= r[1] && p.0 < r[0] + r[2] && p.1 < r[1] + r[3]
}

/// A control in the fourth slot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Control {
    /// A slab mode (`None`: a thin plane).
    Slab(Option<Slab>),
    Thinner,
    Thicker,
    Reset,
    /// The active view alone, or all views again (a toggle).
    Maximise,
    /// Saves the three planes as images.
    ExportPlanes,
    Close,
    /// The 3D view's next preset, rendering and clipping.
    Preset,
    Render,
    Clip,
    /// Smooths the exported surface (a toggle).
    Smooth,
    Export,
    /// Shows or hides segment (set, segment) of the segmentations found.
    Segment(usize, usize),
    /// In a slot too small for every button: shows the next ones, from this one.
    More(usize),
    /// Curved MPR on or off, the dental layout on or off, and the segments' STL export.
    Curve,
    Dental,
    ExportSegments,
    /// The segments' next triangle count (named by the one chosen).
    SegmentDetail(Option<usize>),
    /// Over the curved reformat: the next layout (named by the one shown), adding points,
    /// the last point removed, the curve cleared, saved or loaded, back to the planes.
    CurveKind(specular_render_volume::CprKind),
    CurvePlacing,
    CurveUndo,
    /// An EVAR mark at the cursor (set, or removed when there is one).
    CurveMark,
    CurveClear,
    CurveSave,
    CurveLoad,
    CurveDone,
    /// The centrelines of the volume: one chosen by its place in the list, a new one, the
    /// chosen one renamed (its name field while typing) or deleted (after asking).
    CurveName(usize),
    CurveNew,
    CurveRename,
    CurveRenameField,
    CurveDelete,
    /// Over the panoramic: the next slab (named by the one shown, mm), the next
    /// cross-section spacing (mm), tracing the nerve canal.
    PanoSlab(f64),
    Spacing(f64),
    Nerve,
}

/// The slot's curved MPR buttons: curved MPR shown, the dental layout (`None`: the profile
/// hides it) and whether there are segments to export.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct CurveButtons {
    pub on: bool,
    pub dental: Option<bool>,
    /// The segments' triangle count per segment (`Some(None)`: full detail).
    pub segments: Option<Option<usize>>,
}

/// The 3D view's state in words, for its buttons and its accessible name.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct VrLabels {
    pub preset: String,
    pub mode: String,
    pub clip: String,
    pub smooth: bool,
}

impl Control {
    fn label(self, vr: &VrLabels) -> String {
        match self {
            Control::Preset => vr.preset.clone(),
            Control::Render => vr.mode.clone(),
            Control::Clip => vr.clip.clone(),
            Control::Smooth => t!("vr-smooth"),
            Control::Export => t!("vr-export"),
            Control::Slab(None) => t!("mpr-thin"),
            Control::Slab(Some(s)) => slab_name(s),
            Control::Thinner => "\u{2212}".to_owned(),
            Control::Thicker => "+".to_owned(),
            Control::Reset => t!("mpr-reset"),
            Control::Maximise => t!("mpr-maximise"),
            Control::ExportPlanes => t!("mpr-export-planes"),
            Control::Close => t!("mpr-close"),
            Control::Segment(..) => String::new(),
            Control::More(_) => t!("mpr-more"),
            c => c.curve_label().unwrap_or_default(),
        }
    }

    /// The accessible name (the thickness buttons' glyphs say little).
    fn a11y_label(self, vr: &VrLabels) -> String {
        match self {
            Control::Thinner => t!("mpr-thinner"),
            Control::Thicker => t!("mpr-thicker"),
            c => c.label(vr),
        }
    }
}

pub(crate) fn slab_name(s: Slab) -> String {
    match s {
        Slab::Mip => t!("mpr-mip"),
        Slab::MinIp => t!("mpr-minip"),
        Slab::Average => t!("mpr-average"),
    }
}

/// The slab as words: "Thin", or "Slab: MIP, 10 mm" (in the volume's `length`).
pub(crate) fn slab_text(views: &Views, length: Length) -> String {
    let (mode, thickness) = (slab_name(views.slab), medical::number(views.thickness));
    if views.thickness <= 0.0 {
        return t!("mpr-thin");
    }
    match length {
        Length::Mm => t!("mpr-slab", mode = mode, thickness = thickness),
        Length::Angstrom => t!("mpr-slab-angstrom", mode = mode, thickness = thickness),
        Length::Voxel => t!("mpr-slab-voxels", mode = mode, thickness = thickness),
        l => t!(
            "mpr-slab-unit",
            mode = mode,
            thickness = thickness,
            unit = l.symbol().unwrap_or_default()
        ),
    }
}

/// Everything drawn over the planes.
pub(crate) struct Scene<'a> {
    pub views: &'a Views,
    pub layout: Layout,
    /// Each view's readout (its name and position, the value under the pointer).
    pub readouts: [Vec<String>; 3],
    /// Orientation letters on.
    pub labels: bool,
    /// Measurements per view, in logical pixels.
    pub measures: [Vec<measure::Drawn>; 3],
    /// The fourth slot's lines (window, loading, notes).
    pub panel: Vec<String>,
    /// The view under the pointer or last used (outlined; 3 is the 3D view).
    pub active: Option<usize>,
    pub vr: VrLabels,
    /// The segmentations' segments: (set, segment, name and volume, colour, shown).
    pub segments: Vec<(usize, usize, String, [u8; 3], bool)>,
    /// Each view's pieces of streamline within its slab ([`super::tracts`]).
    pub tracts: super::tracts::Pieces,
    /// The first button shown when they do not all fit ([`Control::More`] pages on).
    pub buttons_first: usize,
    /// A view is maximised (the Maximise button is on).
    pub maximised: bool,
    pub curve: CurveButtons,
}

/// What [`draw`] placed, for hit tests and accessibility.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Placed {
    pub controls: Vec<(Control, Rect, bool)>,
    pub readouts: Vec<(usize, Rect)>,
    pub edges: Vec<Vec<labels::Edge>>,
    /// The 3D view's rectangle: the slot below the controls (logical, on whole physical
    /// pixels).
    pub view3d: Option<R>,
}

/// The orientation letters at view `v`'s edges.
pub(crate) fn edges(views: &Views, v: usize, r: R) -> Vec<labels::Edge> {
    use labels::Side;
    let f = &views.frames[v];
    let neg = |d: [f64; 3]| d.map(|c| -c);
    let (x0, y0, x1, y1) = (r[0], r[1], r[0] + r[2], r[1] + r[3]);
    let (mx, my) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    [
        (Side::Left, neg(f.right), (x0, my)),
        (Side::Right, f.right, (x1, my)),
        (Side::Top, neg(f.down), (mx, y0)),
        (Side::Bottom, f.down, (mx, y1)),
    ]
    .into_iter()
    .filter_map(|(side, d, at)| {
        Some(labels::Edge {
            side,
            text: views.anatomy.label(d)?,
            at: (at.0 as f32, at.1 as f32),
        })
    })
    .collect()
}

/// Draws the overlay; `ts` the text scale.
pub(crate) fn draw(
    dl: &mut DrawList,
    text: &mut Text,
    sc: &Scene,
    pal: &Palette,
    rtl: bool,
    ts: f32,
) -> Placed {
    let mut out = Placed::default();
    for (v, kind) in KINDS.into_iter().enumerate() {
        let r = sc.layout.views[v];
        if empty(r) {
            out.edges.push(Vec::new());
            continue;
        }
        let rr = rect(r);
        dl.push_clip(rr);
        // Streamlines under everything else, in their direction colours.
        for &(a, b, c) in sc.tracts[v].iter() {
            let [r, g, b2, _] = c.map(|x| f32::from(x) / 255.0);
            dl.line(a, b, 1.5, Color::rgba(r, g, b2, 0.9));
        }
        // Crosshair lines: the other planes, in their colours, with a gap at the centre so
        // the voxel under them shows.
        let gap = 10.0 * f64::from(ts);
        let far = r[2] + r[3];
        for l in sc.views.lines(v, r) {
            let c = KINDS[l.other].colour();
            for s in [1.0, -1.0] {
                let a = (l.at.0 + s * gap * l.dir.0, l.at.1 + s * gap * l.dir.1);
                let b = (l.at.0 + s * far * l.dir.0, l.at.1 + s * far * l.dir.1);
                let (a, b) = ((a.0 as f32, a.1 as f32), (b.0 as f32, b.1 as f32));
                dl.line(a, b, 3.0, Color::rgba(0.0, 0.0, 0.0, 0.45));
                dl.line(a, b, 1.25, c.with_alpha(0.9));
            }
        }
        // Rotation handles at the lines' ends.
        for (i, h) in sc.views.handles(v, r).into_iter().enumerate() {
            let c = KINDS[sc.views.lines(v, r)[i / 2].other].colour();
            let h = (h.0 as f32, h.1 as f32);
            dl.circle(h, 5.0, Color::rgba(0.0, 0.0, 0.0, 0.5));
            dl.circle(h, 3.5, c);
        }
        measure::draw(dl, text, &sc.measures[v], rtl, ts);
        if sc.labels {
            let e = edges(sc.views, v, r);
            labels::draw(dl, text, &e, ts);
            out.edges.push(e);
        } else {
            out.edges.push(Vec::new());
        }
        // The view's colour along its top edge, brighter when active.
        let a = if sc.active == Some(v) { 1.0 } else { 0.55 };
        dl.rect(
            Rect::new(rr.x, rr.y, rr.w, 2.0),
            0.0,
            kind.colour().with_alpha(a),
        );
        dl.pop_clip();
        if !sc.readouts[v].is_empty() {
            let b =
                medical::draw_readout(dl, text, &sc.readouts[v], (rr.x, rr.y, rr.w, rr.h), rtl, ts);
            out.readouts.push((v, b));
        }
    }
    let (controls, view3d) = draw_slot(dl, text, sc, pal, rtl, ts);
    out.controls = controls;
    out.view3d = Some(view3d);
    out
}

/// The fourth slot, over the 3D view: the controls in two rows from the top (MPR's, then
/// the 3D view's), the panel's lines at the bottom.
fn draw_slot(
    dl: &mut DrawList,
    text: &mut Text,
    sc: &Scene,
    pal: &Palette,
    rtl: bool,
    ts: f32,
) -> (Vec<(Control, Rect, bool)>, R) {
    let r = rect(sc.layout.slot);
    dl.push_clip(r);
    let margin = PAD * ts;
    let views = sc.views;
    let thin = views.thickness <= 0.0;
    let items = [
        (Control::Slab(None), thin),
        (
            Control::Slab(Some(Slab::Mip)),
            !thin && views.slab == Slab::Mip,
        ),
        (
            Control::Slab(Some(Slab::MinIp)),
            !thin && views.slab == Slab::MinIp,
        ),
        (
            Control::Slab(Some(Slab::Average)),
            !thin && views.slab == Slab::Average,
        ),
        (Control::Thinner, false),
        (Control::Thicker, false),
        (Control::Reset, false),
        (Control::Maximise, sc.maximised),
        (Control::ExportPlanes, false),
        (Control::Close, false),
        (Control::Preset, false),
        (Control::Render, false),
        (Control::Clip, false),
        (Control::Smooth, sc.vr.smooth),
        (Control::Export, false),
    ];
    let mut items = items.to_vec();
    // No preset label: the profile hides the CT and MR presets.
    items.retain(|(c, _)| *c != Control::Preset || !sc.vr.preset.is_empty());
    items.push((Control::Curve, sc.curve.on));
    if let Some(on) = sc.curve.dental {
        items.push((Control::Dental, on));
    }
    if let Some(detail) = sc.curve.segments {
        items.push((Control::ExportSegments, false));
        items.push((Control::SegmentDetail(detail), false));
    }
    // The buttons flow in rows from the top. In a small slot they get smaller and share
    // rows until the 3D view below keeps its height; when they still do not fit, they show
    // a page at a time and the last button, More, shows the next page.
    let avail = (r.w - 2.0 * margin).max(1.0);
    let keep_3d = (MIN_3D as f32).min(r.h / 3.0);
    // The last row's bottom, a gap and a pixel for rounding above the 3D view kept.
    let limit = |f: &Flow| r.y + r.h - keep_3d - f.gap - 1.0;
    let fits = |f: &Flow| r.y + margin + f.height() <= limit(f);
    let mut labels: Vec<String> = items.iter().map(|(c, _)| c.label(&sc.vr)).collect();
    let mut flow = Flow::default();
    for (k, apart) in COMPACT {
        flow = flow_buttons(text, &labels, &items, ts * k, avail, apart);
        if fits(&flow) {
            break;
        }
    }
    if !fits(&flow) {
        let (page, f) = button_page(
            text,
            &items,
            &sc.vr,
            sc.buttons_first,
            flow.ts,
            avail,
            &fits,
        );
        labels = page.iter().map(|(c, _)| c.label(&sc.vr)).collect();
        (items, flow) = (page, f);
    }
    let st = TextStyle::new(FONT_BODY * flow.ts);
    let pad = 8.0 * flow.ts;
    let (bh, gap) = (flow.bh, flow.gap);
    let limit = limit(&flow);
    let mut placed = Vec::new();
    let mut y = r.y + margin;
    for &(i, x, row, w) in &flow.cells {
        let by = r.y + margin + row as f32 * (bh + gap);
        if by + bh > limit && row > 0 {
            break;
        }
        y = by;
        let (c, on) = items[i];
        let b = text.layout(&labels[i], st, None, Align::Start);
        let bx = if rtl {
            r.x + r.w - margin - x - w
        } else {
            r.x + margin + x
        };
        let br = Rect::new(bx, by, w, bh);
        let (fill, ink) = if on {
            (pal.accent_fill, pal.accent_text)
        } else {
            (pal.surface_raised, pal.text)
        };
        dl.rect_border(br, RADIUS_SM, fill, pal.separator, 1.0);
        text.draw(dl, &b, bx + pad, by + (bh - b.height) / 2.0, ink);
        placed.push((c, br, on));
    }
    let st = TextStyle::new(FONT_BODY * ts).tabular();
    let mut ty = y + bh + PAD * ts;
    // The segment list: a toggle per segment with its colour, name and volume. A long list
    // (a dose's isodose levels) flows into as many columns as its widest entry allows.
    let sw = 12.0 * ts;
    let cols = if sc.segments.len() > LIST_COLUMNS_FROM {
        let widest = sc
            .segments
            .iter()
            .map(|(_, _, name, _, _)| text.layout(name, st, None, Align::Start).width)
            .fold(0.0, f32::max);
        ((avail / (widest + sw + 4.0 * pad)).floor() as usize).clamp(1, 4)
    } else {
        1
    };
    let col_w = avail / cols as f32;
    let mut row_h = 0.0f32;
    for (i, (set, k, name, colour, on)) in sc.segments.iter().enumerate() {
        let col = i % cols;
        if col == 0 && i > 0 {
            ty += row_h;
            row_h = 0.0;
        }
        let b = text.layout(
            name,
            st,
            Some((col_w - sw - 3.0 * pad).max(1.0)),
            Align::Start,
        );
        let h = b.height.max(sw) + pad * 0.75;
        row_h = row_h.max(h);
        let cx = if rtl {
            r.x + margin + (cols - 1 - col) as f32 * col_w
        } else {
            r.x + margin + col as f32 * col_w
        };
        let br = Rect::new(cx, ty, col_w, h);
        let sx = if rtl {
            br.x + br.w - pad - sw
        } else {
            br.x + pad
        };
        let [cr, cg, cb] = colour.map(|c| f32::from(c) / 255.0);
        let swatch = Rect::new(sx, ty + (h - sw) / 2.0, sw, sw);
        if *on {
            dl.rect_border(
                swatch,
                2.0,
                Color::rgba(cr, cg, cb, 1.0),
                pal.separator,
                1.0,
            );
        } else {
            dl.rect_border(swatch, 2.0, pal.surface, Color::rgba(cr, cg, cb, 1.0), 1.5);
        }
        let tx = if rtl {
            sx - pad - b.width
        } else {
            sx + sw + pad
        };
        let ink = if *on { pal.text } else { pal.text_faint };
        text.draw(dl, &b, tx, ty + (h - b.height) / 2.0, ink);
        placed.push((Control::Segment(*set, *k), br, *on));
    }
    ty += row_h;
    // In a small window the readout keeps its first lines (window and slab) and drops the
    // rest rather than cover the buttons and the segment list.
    let heights: Vec<f32> = sc
        .panel
        .iter()
        .map(|l| text.layout(l, st, None, Align::Start).height)
        .collect();
    let room = r.y + r.h - TOOLBAR_MARGIN - ty - gap;
    let fit = readout_lines_that_fit(&heights, 2.0 * ts, PAD, room);
    if fit > 0 {
        medical::draw_readout(dl, text, &sc.panel[..fit], (r.x, r.y, r.w, r.h), rtl, ts);
    }
    dl.pop_clip();
    // The 3D view below the controls and the segment list, its top on a whole physical pixel.
    let below = if sc.segments.is_empty() {
        y + bh + gap
    } else {
        ty + gap
    };
    let slot = sc.layout.slot;
    let s = f64::from(dl.scale.max(0.1));
    let top = ((f64::from(below) * s).ceil() / s).min(slot[1] + slot[3] - 1.0);
    let view3d = [slot[0], top, slot[2], slot[1] + slot[3] - top];
    (placed, view3d)
}

/// The 3D view's least height (logical px) the slot's buttons leave it, or a third of the
/// slot in a smaller one.
pub(crate) const MIN_3D: f64 = 96.0;

/// The slot's button sizes tried in turn (a factor of the text scale), and whether the 3D
/// view's buttons start a row of their own.
const COMPACT: [(f32, bool); 3] = [(1.0, true), (0.85, false), (0.7, false)];

/// The slot's buttons at one size: each one's item, x offset, row and width.
#[derive(Default)]
struct Flow {
    ts: f32,
    bh: f32,
    gap: f32,
    cells: Vec<(usize, f32, usize, f32)>,
}

impl Flow {
    /// The rows' height.
    fn height(&self) -> f32 {
        let rows = self.cells.last().map_or(0, |c| c.2 + 1) as f32;
        rows * self.bh + (rows - 1.0).max(0.0) * self.gap
    }
}

/// The page of `items` from `first` that fits (`fits`) with a More button after it, flowed
/// at text scale `ts`: the most buttons that do, at least one. More names the next page's
/// first button, wrapping round.
fn button_page(
    text: &mut Text,
    items: &[(Control, bool)],
    vr: &VrLabels,
    first: usize,
    ts: f32,
    avail: f32,
    fits: &dyn Fn(&Flow) -> bool,
) -> (Vec<(Control, bool)>, Flow) {
    let n = items.len().max(1);
    let start = first % n;
    let mut last = None;
    for k in (1..n).rev() {
        let page: Vec<(Control, bool)> = (0..k)
            .map(|j| items[(start + j) % n])
            .chain([(Control::More((start + k) % n), false)])
            .collect();
        let labels: Vec<String> = page.iter().map(|(c, _)| c.label(vr)).collect();
        let f = flow_buttons(text, &labels, &page, ts, avail, false);
        if fits(&f) {
            return (page, f);
        }
        last = Some((page, f));
    }
    last.unwrap_or_else(|| (items.to_vec(), Flow::default()))
}

/// Flows the buttons `labels` (of `items`) into rows `avail` wide at text scale `ts`; with
/// `apart` the 3D view's buttons start a row of their own.
fn flow_buttons(
    text: &mut Text,
    labels: &[String],
    items: &[(Control, bool)],
    ts: f32,
    avail: f32,
    apart: bool,
) -> Flow {
    let st = TextStyle::new(FONT_BODY * ts);
    let pad = 8.0 * ts;
    let gap = GAP * ts * 0.75;
    let mut cells = Vec::with_capacity(items.len());
    let (mut x, mut row) = (0.0f32, 0usize);
    // The 3D view's buttons start a row of their own: the preset, else (hidden by the
    // profile) the rendering.
    let first_3d = if items.iter().any(|(c, _)| *c == Control::Preset) {
        Control::Preset
    } else {
        Control::Render
    };
    for (i, ((c, _), label)) in items.iter().zip(labels).enumerate() {
        let w = text.layout(label, st, None, Align::Start).width + 2.0 * pad;
        if x > 0.0 && (x + w > avail || (apart && *c == first_3d)) {
            x = 0.0;
            row += 1;
        }
        cells.push((i, x, row, w));
        x += w + gap;
    }
    Flow {
        ts,
        bh: 26.0 * ts,
        gap,
        cells,
    }
}

/// How many of the readout's first lines (each `heights` tall, `gap` apart, `pad` inside the
/// box) fit in `room` logical pixels.
pub(crate) fn readout_lines_that_fit(heights: &[f32], gap: f32, pad: f32, room: f32) -> usize {
    let mut used = 2.0 * pad;
    let mut fit = 0;
    for (i, h) in heights.iter().enumerate() {
        used += h + if i > 0 { gap } else { 0.0 };
        if used > room {
            break;
        }
        fit = i + 1;
    }
    fit
}

/// Segment lists longer than this flow into columns.
const LIST_COLUMNS_FROM: usize = 4;

/// AccessKit ids: 0x5900… (medical uses 0x5800…, cull 0x5000…, the gallery 0x4000…).
const A11Y_BASE: u64 = 0x5900_0000_0000_0000;

/// One node per view (its name, position and readout, and its orientation letters), one
/// per control, and a group around the controls.
pub(crate) fn a11y(
    tree: &mut accesskit::TreeUpdate,
    sc: &Scene,
    placed: &Placed,
    crosshair: String,
    vr: String,
    scale: f64,
) {
    use accesskit::{Action, Node, NodeId, Role};
    let bounds = |r: Rect| accesskit::Rect {
        x0: f64::from(r.x) * scale,
        y0: f64::from(r.y) * scale,
        x1: f64::from(r.x + r.w) * scale,
        y1: f64::from(r.y + r.h) * scale,
    };
    let mut children = Vec::new();
    for v in 0..3 {
        // A view hidden while another is maximised has no node.
        if empty(sc.layout.views[v]) {
            continue;
        }
        let mut n = Node::new(Role::Image);
        let mut label = sc.readouts[v].join(", ");
        if let Some(e) = placed.edges.get(v).filter(|e| !e.is_empty()) {
            label = format!("{label}. {}", labels::a11y_label(e));
        }
        if Some(v) == (0..3).find(|&k| !empty(sc.layout.views[k])) {
            label = format!("{label}. {crosshair}");
        }
        n.set_label(label);
        n.set_bounds(bounds(rect(sc.layout.views[v])));
        let id = NodeId(A11Y_BASE + v as u64);
        tree.nodes.push((id, n));
        children.push(id);
    }
    // The 3D view where it is drawn: below the controls, not the whole slot.
    let mut n = Node::new(Role::Image);
    n.set_label(vr);
    n.set_bounds(bounds(rect(placed.view3d.unwrap_or(sc.layout.slot))));
    let id = NodeId(A11Y_BASE + 3);
    tree.nodes.push((id, n));
    children.push(id);
    let mut group = Node::new(Role::Group);
    group.set_label(t!("mpr-controls"));
    group.set_bounds(bounds(rect(sc.layout.slot)));
    let gid = NodeId(A11Y_BASE + 0x10);
    for (i, (c, r, on)) in placed.controls.iter().enumerate() {
        let mut n = Node::new(Role::Button);
        n.set_label(match c {
            Control::Segment(set, k) => sc
                .segments
                .iter()
                .find(|s| s.0 == *set && s.1 == *k)
                .map_or_else(String::new, |s| s.2.clone()),
            c => c.a11y_label(&sc.vr),
        });
        n.set_bounds(bounds(*r));
        n.add_action(Action::Click);
        if matches!(
            c,
            Control::Slab(_)
                | Control::Smooth
                | Control::Segment(..)
                | Control::Maximise
                | Control::Curve
                | Control::Dental
        ) {
            n.set_toggled(if *on {
                accesskit::Toggled::True
            } else {
                accesskit::Toggled::False
            });
        }
        let id = NodeId(A11Y_BASE + 0x20 + i as u64);
        tree.nodes.push((id, n));
        group.push_child(id);
    }
    if !sc.panel.is_empty() {
        let mut n = Node::new(Role::Status);
        n.set_label(sc.panel.join(", "));
        n.set_bounds(bounds(rect(sc.layout.slot)));
        let id = NodeId(A11Y_BASE + 0x11);
        tree.nodes.push((id, n));
        group.push_child(id);
    }
    tree.nodes.push((gid, group));
    children.push(gid);
    if let Some((_, root)) = tree.nodes.iter_mut().find(|(id, _)| *id == NodeId(1)) {
        for c in children {
            root.push_child(c);
        }
    }
}
