//! Curved MPR (tier 1, any volume) and the dental layout over it (Medical, tier 2).
//!
//! Curved MPR: points clicked along a structure in one plane view (scrolling it between
//! clicks gives a curve in 3-D) make a centripetal Catmull–Rom spline
//! ([`specular_render_volume::curve`]). The layout puts the curved reformat across the top,
//! straightened or stretched, with the slab of the MPR views; below it the plane the points
//! are placed in, the cross-section perpendicular to the curve at the cursor, and the
//! controls. The wheel over the reformat or the cross-section moves the cursor along the
//! curve; Ctrl+wheel zooms; Shift+wheel over the reformat turns it about the curve.
//! Points can be dragged; the curve saves to and loads from a JSON sidecar and stays in
//! the MPR session.
//!
//! Dental layout: a panoramic reformat along the dental arch (suggested from an axial MIP of
//! the densest voxels, then editable like any curve), numbered cross-sections at a fixed
//! spacing with millimetre rulers, and a nerve canal traced by clicking, drawn as a tube in
//! every view. Display only: nothing is planned or measured for treatment.

use std::sync::Arc;

use specular_gfx::{Align, DrawList, Rect, Text, TextBlock, TextStyle};
use specular_i18n::t;
use specular_render::Window as WlWindow;
use specular_render_volume::{Cpr, CprKind, CprView, Plane, Slab, Spline, cross_section};
use specular_theme::metrics::{FONT_BODY, PAD, RADIUS_SM};
use specular_theme::{Color, Palette};
use specular_widgets::accesskit;

use super::super::medical::{self, number};
use super::draw::{Control, Layout, contains, empty, rect};
use super::load::Length;
use super::views::{R, V3, Views, dot, sub};

/// The undo key of a cleared curve's toast.
pub(crate) const UNDO_KEY: u64 = 0x0C5F_0000_0000_0001;
/// The cross-section's field of view (mm across its shorter side) at zoom 1.
const XSEC_FOV: f64 = 40.0;
/// The dental cross-sections: how many, the spacings offered (mm) and the field of view.
pub(crate) const CELLS: usize = 5;
pub(crate) const SPACINGS: [f64; 4] = [2.0, 1.0, 3.0, 5.0];
const DENTAL_FOV: f64 = 22.0;
/// The panoramic's least height (mm): the crowns, the roots and the canal below them.
const PANO_HEIGHT: f64 = 60.0;
/// The panoramic slab thicknesses offered (mm, average).
pub(crate) const PANO_SLABS: [f64; 6] = [15.0, 20.0, 30.0, 10.0, 5.0, 0.0];
/// The nerve canal's drawn radius (mm): a display aid, not a measurement.
pub(crate) const NERVE_RADIUS: f64 = 1.5;
/// Mm the cursor moves per wheel notch, and the points hit radius (logical px).
const CURSOR_STEP: f64 = 1.0;
pub(crate) const POINT_HIT: f64 = 9.0;
/// Curve colour, nerve colour (sRGB) and the cross-section marker's.
const CURVE: Color = Color::rgba(1.0, 0.78, 0.18, 0.95);
const NERVE: Color = Color::rgba(1.0, 0.35, 0.85, 0.95);
/// EVAR marks along the reformat.
const MARK: Color = Color::rgba(0.3, 0.9, 1.0, 0.95);

/// The rectangles of the curved layout (logical px): the reformat, the cross-section area
/// and, in the dental layout, its cells.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Rects {
    pub cpr: R,
    pub xsec: R,
    pub cells: Vec<R>,
}

/// The dental layout's state.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Dental {
    /// The nerve canal's points (patient mm) and its curve.
    pub nerve: Vec<V3>,
    nerve_spline: Option<Spline>,
    /// Clicks on the panoramic or a cross-section place canal points.
    pub tracing: bool,
    /// Index into [`SPACINGS`] and [`PANO_SLABS`].
    pub spacing: usize,
    pub slab: usize,
    /// The CBCT maker recognised from the series (Manufacturer), if one is.
    pub vendor: Option<&'static str>,
}

impl Dental {
    pub(crate) fn new(vendor: Option<&'static str>) -> Dental {
        Dental {
            nerve: Vec::new(),
            nerve_spline: None,
            tracing: false,
            spacing: 0,
            slab: 0,
            vendor,
        }
    }

    pub(crate) fn set_nerve(&mut self, pts: Vec<V3>) {
        self.nerve_spline = Spline::new(&pts);
        self.nerve = pts;
    }

    pub(crate) fn nerve_spline(&self) -> Option<&Spline> {
        self.nerve_spline.as_ref()
    }

    pub(crate) fn spacing_mm(&self) -> f64 {
        SPACINGS[self.spacing % SPACINGS.len()]
    }

    pub(crate) fn slab_mm(&self) -> f64 {
        PANO_SLABS[self.slab % PANO_SLABS.len()]
    }
}

/// Curved MPR's state in an MPR session.
#[derive(Clone, Debug)]
pub(crate) struct Curved {
    points: Vec<V3>,
    spline: Option<Spline>,
    /// The curved layout is shown (the curve stays when it is not).
    pub shown: bool,
    /// Clicks in the plane view add points.
    pub placing: bool,
    /// The view the points are placed in, and its normal when curved MPR began (the
    /// vector of interest: the reformat's up).
    pub view: usize,
    pub up: V3,
    pub kind: CprKind,
    /// The reformat's turn about the curve (radians).
    pub angle: f64,
    /// The cursor: arc length of the cross-section.
    pub cursor: f64,
    /// The reformat's zoom (1: the whole curve fits), its pan along (mm) and down (mm).
    pub zoom: f64,
    pub pan: [f64; 2],
    pub xsec_zoom: f64,
    pub rects: Option<Rects>,
    /// The reformat as laid out for the last frame (target px).
    pub cpr: Option<Cpr>,
    pub dental: Option<Dental>,
    /// Points of the curve last cleared (its toast's Undo brings them back).
    pub cleared: Option<Vec<V3>>,
    /// Controls drawn over the reformat (logical px).
    pub placed: Vec<(Control, Rect, bool)>,
    /// The centreline's name (empty: numbered in the list).
    pub name: String,
    /// The names of every centreline of the volume in list order, and this one's place
    /// (set by the session before drawing).
    pub list: Vec<String>,
    pub index: usize,
    /// Its name being typed: text, caret (byte) and an input method's composition.
    pub rename: Option<(String, usize, String)>,
    /// The selected bytes of the name being typed (empty: none).
    pub rename_selection: std::ops::Range<usize>,
    /// The name field's text as last drawn and the x it starts at (logical px; a press
    /// there puts the caret).
    pub field_text: FieldText,
    /// The centreline chip with the keyboard focus (list order).
    pub focus: Option<usize>,
    /// EVAR marks along the centreline (arc lengths, mm, ascending), placed at the cursor.
    pub marks: Vec<f64>,
}

impl Curved {
    pub(crate) fn new(view: usize, up: V3) -> Curved {
        Curved {
            points: Vec::new(),
            spline: None,
            shown: true,
            placing: true,
            view,
            up,
            kind: CprKind::Straightened,
            angle: 0.0,
            cursor: 0.0,
            zoom: 1.0,
            pan: [0.0; 2],
            xsec_zoom: 1.0,
            rects: None,
            cpr: None,
            dental: None,
            cleared: None,
            placed: Vec::new(),
            name: String::new(),
            list: Vec::new(),
            index: 0,
            rename: None,
            rename_selection: 0..0,
            field_text: None,
            focus: None,
            marks: Vec::new(),
        }
    }

    /// What a control over the reformat says: a centreline's name, the name being typed,
    /// else [`Control::curve_label`].
    pub(crate) fn control_label(&self, ctl: Control) -> String {
        match ctl {
            Control::CurveName(i) => self.list.get(i).cloned().unwrap_or_default(),
            Control::CurveRenameField => self
                .rename
                .as_ref()
                .map(|r| r.0.clone())
                .unwrap_or_default(),
            c => c.curve_label().unwrap_or_default(),
        }
    }

    pub(crate) fn points(&self) -> &[V3] {
        &self.points
    }

    pub(crate) fn spline(&self) -> Option<&Spline> {
        self.spline.as_ref()
    }

    /// Replaces the points; the cursor stays on the curve. EVAR marks follow the edit: each
    /// keeps its place between the same two control points; one whose stretch is gone (the
    /// last point undone) goes too.
    pub(crate) fn set_points(&mut self, pts: Vec<V3>) {
        let anchors = self.spline.as_ref().map(|s| mark_anchors(s, &self.marks));
        self.points = pts;
        self.points
            .truncate(specular_render_volume::curve::MAX_POINTS);
        self.spline = Spline::new(&self.points);
        if let Some(a) = anchors
            && let Some(s) = self.spline.as_ref()
        {
            self.marks = follow_marks(s, &a);
        }
        let l = self.spline.as_ref().map_or(0.0, Spline::length);
        if self.cursor <= 0.0 || self.cursor > l {
            self.cursor = l / 2.0;
        }
    }

    pub(crate) fn push(&mut self, p: V3) {
        let mut pts = self.points.clone();
        pts.push(p);
        self.set_points(pts);
    }

    pub(crate) fn move_point(&mut self, i: usize, p: V3) {
        let mut pts = self.points.clone();
        if let Some(q) = pts.get_mut(i) {
            *q = p;
            self.set_points(pts);
        }
    }

    pub(crate) fn pop(&mut self) -> bool {
        let mut pts = self.points.clone();
        let gone = pts.pop().is_some();
        self.set_points(pts);
        gone
    }

    /// The reformat's view for target rectangle `px`.
    fn view_for(&self, s: &Spline, px: [u32; 4]) -> CprView {
        let kind = if self.dental.is_some() {
            CprKind::Straightened
        } else {
            self.kind
        };
        let mut v = CprView::fit(s, self.up, kind, px);
        if self.dental.is_some() {
            v.mm = v.mm.max(PANO_HEIGHT / f64::from(px[3].max(1)));
        }
        v.mm /= self.zoom.max(0.05);
        v.along += self.pan[0];
        v.offset = self.pan[1];
        v.angle = if self.dental.is_some() {
            0.0
        } else {
            self.angle
        };
        v
    }

    /// Lays the reformat out for this frame (`scale` physical px per logical px).
    pub(crate) fn update(&mut self, scale: f64) {
        self.cpr = match (&self.spline, &self.rects) {
            (Some(s), Some(r)) if self.shown => {
                let px = physical(r.cpr, scale);
                Some(Cpr::new(s, &self.view_for(s, px), px))
            }
            _ => None,
        };
    }

    /// The arc lengths of the dental cross-sections (the cursor's in the middle).
    pub(crate) fn cell_positions(&self) -> Vec<f64> {
        let Some(d) = &self.dental else {
            return vec![self.cursor];
        };
        let step = d.spacing_mm();
        (0..CELLS)
            .map(|i| self.cursor + (i as f64 - (CELLS / 2) as f64) * step)
            .collect()
    }

    /// The cross-section planes (target px), one per cell (dental) or one.
    pub(crate) fn xsec_planes(
        &self,
        scale: f64,
        window: WlWindow,
        views: &Views,
    ) -> Vec<(R, f64, Plane)> {
        let (Some(s), Some(r)) = (&self.spline, &self.rects) else {
            return Vec::new();
        };
        let cells: Vec<R> = if self.dental.is_some() {
            r.cells.clone()
        } else {
            vec![r.xsec]
        };
        let fov = if self.dental.is_some() {
            DENTAL_FOV
        } else {
            XSEC_FOV
        };
        cells
            .into_iter()
            .zip(self.cell_positions())
            .filter(|(c, _)| !empty(*c))
            .filter(|(_, at)| *at >= 0.0 && *at <= s.length())
            .map(|(c, at)| {
                let px = physical(c, scale);
                let mm = fov / f64::from(px[2].min(px[3]).max(1)) / self.xsec_zoom.max(0.05);
                let angle = if self.dental.is_some() {
                    0.0
                } else {
                    self.angle
                };
                let mut p = cross_section(s, self.up, angle, at, px, mm, window);
                if self.dental.is_none() && views.thickness > 0.0 {
                    p.thickness = views.thickness;
                    p.slab = views.slab;
                }
                (c, at, p)
            })
            .collect()
    }

    /// Every plane curved MPR draws this frame: the reformat's columns, the cross-sections.
    pub(crate) fn planes(
        &self,
        scale: f64,
        window: WlWindow,
        views: &Views,
        canvas: [f32; 4],
    ) -> Vec<Plane> {
        let mut out = Vec::new();
        if let Some(c) = &self.cpr {
            let (thickness, slab) = match &self.dental {
                Some(d) => (d.slab_mm(), Slab::Average),
                None => (views.thickness, views.slab),
            };
            out.extend(c.planes(thickness, slab, window, canvas));
        }
        out.extend(
            self.xsec_planes(scale, window, views)
                .into_iter()
                .map(|(_, _, p)| p),
        );
        out
    }

    /// The patient point under logical point `p` of the reformat.
    pub(crate) fn cpr_point(&self, p: (f64, f64), scale: f64) -> Option<V3> {
        self.cpr.as_ref()?.to_patient(p.0 * scale, p.1 * scale)
    }

    /// The arc length under logical x of the reformat.
    pub(crate) fn cpr_s(&self, x: f64, scale: f64) -> Option<f64> {
        self.cpr.as_ref()?.column(x * scale).map(|c| c.s)
    }

    /// Moves the cursor by `steps` (whole notches), on the curve.
    pub(crate) fn step(&mut self, steps: f64) {
        let l = self.spline.as_ref().map_or(0.0, Spline::length);
        let mm = match &self.dental {
            Some(d) => d.spacing_mm(),
            None => CURSOR_STEP,
        };
        self.cursor = (self.cursor + steps * mm).clamp(0.0, l);
    }

    /// The control point drawn under logical point `p` of view `v`'s rectangle `r`.
    pub(crate) fn point_at(
        &self,
        views: &Views,
        v: usize,
        r: R,
        p: (f64, f64),
        hit: f64,
    ) -> Option<usize> {
        self.points
            .iter()
            .enumerate()
            .map(|(i, q)| {
                let s = views.to_screen(v, r, *q);
                (i, (s.0 - p.0).hypot(s.1 - p.1))
            })
            .filter(|(_, d)| *d <= hit)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    }

    /// The cross-section cell under logical point `p`: its plane.
    pub(crate) fn xsec_at(
        &self,
        p: (f64, f64),
        scale: f64,
        window: WlWindow,
        views: &Views,
    ) -> Option<Plane> {
        self.xsec_planes(scale, window, views)
            .into_iter()
            .find(|(c, _, _)| contains(*c, p))
            .map(|(_, _, pl)| pl)
    }
}

/// Where each mark (arc length) lies: the control-point stretch it is on and the fraction of
/// that stretch's length before it.
fn mark_anchors(s: &Spline, marks: &[f64]) -> Vec<(usize, f64)> {
    let stretches = s.points.len().saturating_sub(1);
    marks
        .iter()
        .map(|&m| {
            let i = (1..stretches).take_while(|&i| s.knot(i) <= m).count();
            let (a, b) = (s.knot(i), s.knot(i + 1));
            let f = if b > a { (m - a) / (b - a) } else { 0.0 };
            (i, f.clamp(0.0, 1.0))
        })
        .collect()
}

/// The marks back from [`mark_anchors`] on the edited curve; those on a stretch it no longer
/// has are dropped (a mark on a point the curve still ends at stays).
fn follow_marks(s: &Spline, anchors: &[(usize, f64)]) -> Vec<f64> {
    let stretches = s.points.len().saturating_sub(1);
    let mut out: Vec<f64> = anchors
        .iter()
        // A mark on the last point kept stays on the new end.
        .filter(|&&(i, f)| i < stretches || (i == stretches && f <= 1e-9))
        .map(|&(i, f)| {
            let (a, b) = (s.knot(i), s.knot(i + 1));
            a + f * (b - a)
        })
        .collect();
    out.sort_by(f64::total_cmp);
    out.dedup_by(|a, b| (*a - *b).abs() <= MARK_SNAP);
    out
}

/// Most EVAR marks on one centreline.
pub(crate) const MAX_MARKS: usize = 32;
/// A mark this close to the cursor (mm along the curve) is the cursor's: the button
/// removes it.
const MARK_SNAP: f64 = 0.5;
/// A length belongs to a cross-section when both its ends are this close to the plane (mm).
const ON_SECTION: f64 = 0.5;

/// EVAR-style readouts along a centreline (DESIGN.md §7, curved MPR): marks the user sets at
/// the cursor, the length along the centreline between them, and the smallest and largest
/// diameter measured by hand (`m`) on each mark's cross-section. Nothing is segmented.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct EvarRow {
    /// Arc length of the mark (mm).
    pub at: f64,
    /// Along the centreline from the previous mark (mm).
    pub from_prev: Option<f64>,
    /// The smallest and largest length measured on its cross-section, and how many.
    pub diameters: Option<(f64, f64, usize)>,
}

/// The lengths of `measures` lying on the cross-section at arc length `s`: (min, max, count).
pub(crate) fn section_diameters(
    spline: &Spline,
    up: V3,
    angle: f64,
    s: f64,
    measures: &[[V3; 2]],
) -> Option<(f64, f64, usize)> {
    let f = spline.frame_at(up, angle, s);
    let off = |p: V3| dot(sub(p, f.point), f.tangent).abs();
    let lens: Vec<f64> = measures
        .iter()
        .filter(|m| off(m[0]) <= ON_SECTION && off(m[1]) <= ON_SECTION)
        .map(|m| {
            let d = sub(m[1], m[0]);
            dot(d, d).sqrt()
        })
        .filter(|d| *d > 0.0)
        .collect();
    let min = lens.iter().copied().fold(f64::INFINITY, f64::min);
    let max = lens.iter().copied().fold(0.0, f64::max);
    (!lens.is_empty()).then_some((min, max, lens.len()))
}

/// The readouts of every mark.
pub(crate) fn evar(c: &Curved, measures: &[[V3; 2]]) -> Vec<EvarRow> {
    let Some(s) = c.spline() else {
        return Vec::new();
    };
    let mut prev: Option<f64> = None;
    c.marks
        .iter()
        .map(|&at| {
            let row = EvarRow {
                at,
                from_prev: prev.map(|p| at - p),
                diameters: section_diameters(s, c.up, c.angle, at, measures),
            };
            prev = Some(at);
            row
        })
        .collect()
}

impl Curved {
    /// Sets a mark at the cursor, or removes the one there. True when one was set.
    pub(crate) fn toggle_mark(&mut self) -> bool {
        let Some(len) = self.spline().map(Spline::length) else {
            return false;
        };
        let at = self.cursor.clamp(0.0, len);
        if let Some(i) = self.marks.iter().position(|m| (m - at).abs() <= MARK_SNAP) {
            self.marks.remove(i);
            return false;
        }
        if self.marks.len() >= MAX_MARKS {
            return false;
        }
        let i = self.marks.partition_point(|m| *m < at);
        self.marks.insert(i, at);
        true
    }

    /// The mark at the cursor (its index), if there is one.
    pub(crate) fn mark_at_cursor(&self) -> Option<usize> {
        self.marks
            .iter()
            .position(|m| (m - self.cursor).abs() <= MARK_SNAP)
    }
}

/// The EVAR lines under the cross-section: the section's diameters, then each span between
/// marks and the total.
pub(crate) fn evar_lines(c: &Curved, measures: &[[V3; 2]], l: Length) -> Vec<String> {
    let mut out = Vec::new();
    let Some(s) = c.spline() else { return out };
    if let Some(i) = c.mark_at_cursor() {
        out.push(t!("cpr-mark-here", n = (i + 1) as i64));
    }
    if let Some((min, max, n)) = section_diameters(s, c.up, c.angle, c.cursor, measures) {
        out.push(if n == 1 {
            t!("cpr-diameter", value = length(max, l))
        } else {
            t!("cpr-diameters", min = length(min, l), max = length(max, l))
        });
    }
    let rows = evar(c, measures);
    for (i, r) in rows.iter().enumerate().skip(1) {
        if let Some(d) = r.from_prev {
            out.push(t!(
                "cpr-mark-span",
                from = i as i64,
                to = (i + 1) as i64,
                length = length(d, l)
            ));
        }
    }
    if rows.len() > 2 {
        out.push(t!(
            "cpr-mark-total",
            length = length(rows[rows.len() - 1].at - rows[0].at, l)
        ));
    }
    out
}

/// Target pixels of logical rectangle `r`.
pub(crate) fn physical(r: R, scale: f64) -> [u32; 4] {
    [
        (r[0] * scale).round().max(0.0) as u32,
        (r[1] * scale).round().max(0.0) as u32,
        (r[2] * scale).round().max(0.0) as u32,
        (r[3] * scale).round().max(0.0) as u32,
    ]
}

/// The curved layout over viewport `vp` (logical): the reformat across the top; below it
/// the plane view `v`, the cross-section area and the controls' slot (mirrored in
/// right-to-left layouts; the images never mirror). The other two views are hidden.
pub(crate) fn layout(vp: R, scale: f64, rtl: bool, v: usize, dental: bool) -> (Layout, Rects) {
    let s = scale.max(0.1);
    let x0 = (vp[0] * s).round();
    let y0 = (vp[1] * s).round();
    let w = (vp[2] * s).round().max(6.0);
    let h = (vp[3] * s).round().max(6.0);
    let gap = s.round().max(1.0);
    let top = (h * if dental { 0.42 } else { 0.45 }).floor();
    let below = h - top - gap;
    let shares: [f64; 3] = if dental {
        [0.28, 0.47, 0.25]
    } else {
        [0.34, 0.36, 0.30]
    };
    let mut widths = [0.0; 3];
    let avail = w - 2.0 * gap;
    widths[0] = (avail * shares[0]).floor();
    widths[1] = (avail * shares[1]).floor();
    widths[2] = avail - widths[0] - widths[1];
    let mut xs = [0.0; 3];
    let mut x = x0;
    for i in 0..3 {
        let k = if rtl { 2 - i } else { i };
        xs[k] = x;
        x += widths[k] + gap;
    }
    let by = y0 + top + gap;
    let cell = |i: usize| [xs[i] / s, by / s, widths[i] / s, below / s];
    let empty_r = [x0 / s, y0 / s, 0.0, 0.0];
    let mut views = [empty_r; 3];
    views[v.min(2)] = cell(0);
    let xsec = cell(1);
    let cells = if dental {
        let n = CELLS as f64;
        let cw = ((widths[1] - gap * (n - 1.0)) / n).floor().max(1.0);
        (0..CELLS)
            .map(|i| {
                // Cell 1 on the left in every language: the cells follow the panoramic,
                // which never mirrors.
                let k = i as f64;
                [(xs[1] + k * (cw + gap)) / s, by / s, cw / s, below / s]
            })
            .collect()
    } else {
        Vec::new()
    };
    (
        Layout {
            views,
            slot: cell(2),
        },
        Rects {
            cpr: [x0 / s, y0 / s, w / s, top / s],
            xsec,
            cells,
        },
    )
}

/// The CBCT maker named by a series' Manufacturer (0008,0070), for the dental makers whose
/// exports are recognised: Planmeca, Carestream, Dentsply Sirona and Vatech.
pub(crate) fn cbct_vendor(manufacturer: &str) -> Option<&'static str> {
    let m = manufacturer.to_ascii_lowercase();
    [
        ("planmeca", "Planmeca"),
        ("carestream", "Carestream"),
        ("sirona", "Dentsply Sirona"),
        ("vatech", "Vatech"),
        ("e-woo", "Vatech"),
    ]
    .into_iter()
    .find(|(k, _)| m.contains(k))
    .map(|(_, v)| v)
}

// ----- Drawing -----------------------------------------------------------------------------

/// What the overlay needs besides the state.
pub(crate) struct Ctx<'a> {
    pub views: &'a Views,
    pub layout: Layout,
    pub scale: f64,
    pub window: WlWindow,
    pub length: Length,
    /// The pointer (logical), for the readouts.
    pub pointer: Option<(f64, f64)>,
    /// The cross-sections' measurements (patient mm), with their lengths' text.
    pub measures: &'a [[V3; 2]],
    /// A measurement's first point, before the second click.
    pub draft: Option<V3>,
}

fn to_f32(p: (f64, f64)) -> (f32, f32) {
    (p.0 as f32, p.1 as f32)
}

/// A polyline (logical px), outlined for contrast.
fn polyline(dl: &mut DrawList, pts: &[(f32, f32)], width: f32, c: Color) {
    for w in pts.windows(2) {
        dl.line(w[0], w[1], width + 2.0, Color::rgba(0.0, 0.0, 0.0, 0.45));
    }
    for w in pts.windows(2) {
        dl.line(w[0], w[1], width, c);
    }
}

/// Where patient point `p` shows in cross-section `plane` (logical px), and its distance
/// off the plane (mm).
/// Where a measurement's first point shows on a cross-section (logical px), if it lies on it.
pub(crate) fn draft_mark(plane: &Plane, p: V3, scale: f64) -> Option<(f32, f32)> {
    let (at, off) = on_plane(plane, p, scale);
    (off.abs() <= 0.5).then(|| to_f32(at))
}

fn on_plane(plane: &Plane, p: V3, scale: f64) -> ((f64, f64), f64) {
    let d = sub(p, plane.origin);
    let x = dot(d, plane.du) / dot(plane.du, plane.du);
    let y = dot(d, plane.dv) / dot(plane.dv, plane.dv);
    let off = dot(d, plane.normal());
    ((x / scale, y / scale), off)
}

/// The length's text, in the volume's unit.
fn length(d: f64, l: Length) -> String {
    super::length_text(d, l)
}

/// Draws curved MPR's overlay and controls; returns the controls placed.
pub(crate) fn draw(
    dl: &mut DrawList,
    text: &mut Text,
    c: &Curved,
    cx: &Ctx,
    pal: &Palette,
    rtl: bool,
    ts: f32,
) -> (Vec<(Control, Rect, bool)>, FieldText) {
    let Some(rects) = &c.rects else {
        return (Vec::new(), None);
    };
    let v = c.view.min(2);
    let vr = cx.layout.views[v];
    // The curve in the plane view: its samples projected, points filled when on the plane.
    if !empty(vr) {
        dl.push_clip(rect(vr));
        if let Some(s) = &c.spline {
            let pts: Vec<_> = s
                .samples()
                .iter()
                .map(|p| to_f32(cx.views.to_screen(v, vr, *p)))
                .collect();
            polyline(dl, &pts, 2.0, CURVE);
            let at = to_f32(cx.views.to_screen(v, vr, s.at(c.cursor)));
            dl.circle(at, 6.0 * ts, Color::rgba(0.0, 0.0, 0.0, 0.5));
            dl.circle(at, 4.0 * ts, Color::rgba(1.0, 1.0, 1.0, 0.95));
        }
        for p in &c.points {
            let q = to_f32(cx.views.to_screen(v, vr, *p));
            let on = cx.views.on_plane(v, *p);
            dl.circle(q, 5.5 * ts, Color::rgba(0.0, 0.0, 0.0, 0.55));
            dl.circle(
                q,
                4.0 * ts,
                if on {
                    CURVE
                } else {
                    Color::rgba(0.2, 0.2, 0.2, 0.9)
                },
            );
        }
        if let Some(n) = c.dental.as_ref().and_then(Dental::nerve_spline) {
            let pts: Vec<_> = n
                .samples()
                .iter()
                .map(|p| to_f32(cx.views.to_screen(v, vr, *p)))
                .collect();
            polyline(dl, &pts, 2.0, NERVE);
        }
        dl.pop_clip();
    }
    let (placed, _) = toolbar(dl, text, c, rects.cpr, pal, (rtl, false), ts);
    let below = placed
        .iter()
        .map(|(_, r, _)| r.y + r.h)
        .fold(rect(rects.cpr).y, f32::max);
    draw_cpr(dl, text, c, cx, rects, below, rtl, ts);
    draw_xsecs(dl, text, c, cx, rtl, ts);
    // The controls over the reformat's marks.
    toolbar(dl, text, c, rects.cpr, pal, (rtl, true), ts)
}

/// The reformat's overlay: the cursor, the dental cells' positions, the nerve canal, a scale
/// bar and the readout.
/// `below`: the controls' bottom edge (the cells' numbers go under it).
#[allow(clippy::too_many_arguments)]
fn draw_cpr(
    dl: &mut DrawList,
    text: &mut Text,
    c: &Curved,
    cx: &Ctx,
    rects: &Rects,
    below: f32,
    rtl: bool,
    ts: f32,
) {
    let r = rects.cpr;
    let rr = rect(r);
    dl.push_clip(rr);
    let scale = cx.scale;
    let mut lines = Vec::new();
    match (&c.spline, &c.cpr) {
        (Some(s), Some(cpr)) => {
            let x_of = |at: f64| cpr.x_of(at).map(|x| x / scale);
            let (top, bottom) = (rr.y, rr.y + rr.h);
            let marks = below + 4.0 * ts;
            if let Some(x) = x_of(c.cursor) {
                let x = x as f32;
                dl.line((x, top), (x, bottom), 3.0, Color::rgba(0.0, 0.0, 0.0, 0.45));
                dl.line((x, top), (x, bottom), 1.25, Color::rgba(1.0, 1.0, 1.0, 0.9));
            }
            // EVAR marks: numbered ticks across the reformat.
            if c.dental.is_none() {
                let st = TextStyle::new(FONT_BODY * 0.85 * ts).tabular();
                for (i, at) in c.marks.iter().enumerate() {
                    let Some(x) = x_of(*at) else { continue };
                    let x = x as f32;
                    let b = text.layout(&(i + 1).to_string(), st, None, Align::Start);
                    let y0 = marks + b.height + 2.0 * ts;
                    dl.line((x, y0), (x, bottom), 3.0, Color::rgba(0.0, 0.0, 0.0, 0.4));
                    dl.line((x, y0), (x, bottom), 1.25, MARK);
                    dl.rect(
                        Rect::new(
                            x - b.width / 2.0 - 3.0,
                            marks - 1.0,
                            b.width + 6.0,
                            b.height + 2.0,
                        ),
                        RADIUS_SM,
                        Color::rgba(0.0, 0.0, 0.0, 0.6),
                    );
                    text.draw(dl, &b, x - b.width / 2.0, marks, MARK);
                }
            }
            if c.dental.is_some() {
                let st = TextStyle::new(FONT_BODY * 0.85 * ts).tabular();
                for (i, at) in c.cell_positions().into_iter().enumerate() {
                    let Some(x) = x_of(at) else { continue };
                    let x = x as f32;
                    let b = text.layout(&(i + 1).to_string(), st, None, Align::Start);
                    dl.line(
                        (x, marks + b.height + 2.0 * ts),
                        (x, bottom),
                        1.0,
                        CURVE.with_alpha(0.45),
                    );
                    text.draw(dl, &b, x - b.width / 2.0, marks, CURVE);
                }
            }
            // The control points along the reformat (straightened: on the middle row).
            for p in &c.points {
                if let Some((x, y)) = cpr.to_image(s, *p) {
                    let q = ((x / scale) as f32, (y / scale) as f32);
                    dl.circle(q, 4.0 * ts, Color::rgba(0.0, 0.0, 0.0, 0.5));
                    dl.circle(q, 2.75 * ts, CURVE);
                }
            }
            if let Some(n) = c.dental.as_ref().and_then(Dental::nerve_spline) {
                let r_px = (NERVE_RADIUS / cpr.mm / scale) as f32;
                let mut mid = Vec::new();
                for p in n.samples() {
                    if let Some((x, y)) = cpr.to_image(s, *p) {
                        mid.push(((x / scale) as f32, (y / scale) as f32));
                    }
                }
                // Long gaps (a canal point far off the arch) break the drawing.
                for w in mid.windows(2) {
                    if (w[1].0 - w[0].0).abs() > 40.0 * ts {
                        continue;
                    }
                    for dy in [-r_px, r_px] {
                        let a = (w[0].0, w[0].1 + dy);
                        let b = (w[1].0, w[1].1 + dy);
                        dl.line(a, b, 3.0, Color::rgba(0.0, 0.0, 0.0, 0.4));
                        dl.line(a, b, 1.5, NERVE);
                    }
                }
            }
            scale_bar(dl, text, rr, cpr.mm * scale, cx.length, rtl, ts);
            let name = match &c.dental {
                Some(d) => match d.slab_mm() {
                    t if t > 0.0 => t!("cpr-panoramic-slab", thickness = number(t)),
                    _ => t!("cpr-panoramic"),
                },
                None => t!("cpr-kind", kind = c.kind.id()),
            };
            lines.push(specular_i18n::dot_join(&[
                name,
                length(s.length(), cx.length),
            ]));
            let pointer = cx.pointer.filter(|p| contains(r, *p));
            let at = pointer
                .and_then(|p| c.cpr_s(p.0, scale))
                .unwrap_or(c.cursor);
            lines.push(t!("cpr-at", position = length(at, cx.length)));
            if c.dental.is_some() {
                lines.push(t!("dental-not-for-diagnosis"));
            }
        }
        _ => {
            let view = super::view_name(c.view.min(2), true, cx.views.oblique(c.view.min(2)));
            lines.push(match c.points.len() {
                0 => t!("cpr-hint-start", view = view),
                _ => t!("cpr-hint-more"),
            });
            if c.dental.is_some() {
                lines.push(t!("dental-not-for-diagnosis"));
            }
        }
    }
    if let Some(d) = &c.dental
        && d.tracing
    {
        lines.push(t!("dental-nerve-hint"));
    }
    // The top edge in the curve's colour.
    dl.rect(Rect::new(rr.x, rr.y, rr.w, 2.0), 0.0, CURVE);
    dl.pop_clip();
    medical::draw_readout(dl, text, &lines, (rr.x, rr.y, rr.w, rr.h), rtl, ts);
}

/// A bar of a round length at the reformat's bottom start corner; `mm` per logical px.
fn scale_bar(dl: &mut DrawList, text: &mut Text, r: Rect, mm: f64, l: Length, rtl: bool, ts: f32) {
    let want = 80.0 * f64::from(ts) * mm;
    let nice = [1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0];
    let len_mm = nice
        .iter()
        .copied()
        .rev()
        .find(|n| *n <= want)
        .unwrap_or(1.0);
    let w = (len_mm / mm) as f32;
    let y = r.y + r.h - 12.0 * ts;
    let x = if rtl {
        r.x + r.w - 12.0 * ts - w
    } else {
        r.x + 12.0 * ts
    };
    dl.line((x, y), (x + w, y), 4.0, Color::rgba(0.0, 0.0, 0.0, 0.5));
    dl.line((x, y), (x + w, y), 2.0, Color::rgba(1.0, 1.0, 1.0, 0.9));
    let st = TextStyle::new(FONT_BODY * 0.85 * ts).tabular();
    let b = text.layout(&length(len_mm, l), st, None, Align::Start);
    text.draw(
        dl,
        &b,
        x + (w - b.width) / 2.0,
        y - 4.0 * ts - b.height,
        Color::rgba(1.0, 1.0, 1.0, 0.95),
    );
}

/// The cross-sections' overlay: the curve's point, the nerve canal where it crosses, the
/// rulers (dental), measurements, the numbers and positions.
fn draw_xsecs(dl: &mut DrawList, text: &mut Text, c: &Curved, cx: &Ctx, rtl: bool, ts: f32) {
    let scale = cx.scale;
    let planes = c.xsec_planes(scale, cx.window, cx.views);
    let nerve = c.dental.as_ref().and_then(Dental::nerve_spline);
    for (i, (r, at, plane)) in planes.iter().enumerate() {
        let rr = rect(*r);
        dl.push_clip(rr);
        let (centre, _) = on_plane(
            plane,
            plane.at(
                f64::from(plane.rect[0]) + f64::from(plane.rect[2]) / 2.0,
                f64::from(plane.rect[1]) + f64::from(plane.rect[3]) / 2.0,
            ),
            scale,
        );
        let q = to_f32(centre);
        let k = 6.0 * ts;
        for (a, b) in [
            ((q.0 - k, q.1), (q.0 + k, q.1)),
            ((q.0, q.1 - k), (q.0, q.1 + k)),
        ] {
            dl.line(a, b, 3.0, Color::rgba(0.0, 0.0, 0.0, 0.45));
            dl.line(a, b, 1.25, CURVE);
        }
        let mm_px = plane.du.iter().map(|x| x * x).sum::<f64>().sqrt() * scale;
        if let Some(n) = nerve {
            // Where the canal passes through the plane: a sign change of the distance.
            let samples = n.samples();
            for w in samples.windows(2) {
                let (pa, da) = on_plane(plane, w[0], scale);
                let (pb, db) = on_plane(plane, w[1], scale);
                if da == db || da.signum() == db.signum() && da != 0.0 {
                    continue;
                }
                let t = da / (da - db);
                let p = (pa.0 + (pb.0 - pa.0) * t, pa.1 + (pb.1 - pa.1) * t);
                let rad = (NERVE_RADIUS / mm_px) as f32;
                ring(dl, to_f32(p), rad.max(3.0), NERVE);
            }
        }
        // The first click of a measurement on this cross-section.
        if let Some(a) = cx.draft.and_then(|p| draft_mark(plane, p, scale)) {
            dl.circle(a, 4.0, Color::rgba(0.0, 0.0, 0.0, 0.5));
            dl.circle(a, 2.75, Color::rgba(0.3, 0.9, 1.0, 0.95));
        }
        // Lengths measured on this cross-section.
        for m in cx.measures {
            let (a, da) = on_plane(plane, m[0], scale);
            let (b, db) = on_plane(plane, m[1], scale);
            if da.abs() > 0.5 || db.abs() > 0.5 {
                continue;
            }
            let d = (0..3)
                .map(|k| (m[0][k] - m[1][k]).powi(2))
                .sum::<f64>()
                .sqrt();
            let (a, b) = (to_f32(a), to_f32(b));
            dl.line(a, b, 3.5, Color::rgba(0.0, 0.0, 0.0, 0.5));
            dl.line(a, b, 1.75, Color::rgba(0.3, 0.9, 1.0, 0.95));
            let st = TextStyle::new(FONT_BODY * ts).tabular();
            let bl = text.layout(&length(d, cx.length), st, None, Align::Start);
            let mx = (a.0 + b.0) / 2.0;
            let my = (a.1 + b.1) / 2.0 - bl.height - 4.0;
            dl.rect(
                Rect::new(mx - 2.0, my - 1.0, bl.width + 4.0, bl.height + 2.0),
                RADIUS_SM,
                Color::rgba(0.0, 0.0, 0.0, 0.6),
            );
            text.draw(dl, &bl, mx, my, Color::rgba(1.0, 1.0, 1.0, 1.0));
        }
        if c.dental.is_some() {
            ruler(dl, text, rr, mm_px, rtl, ts);
        }
        dl.rect(Rect::new(rr.x, rr.y, rr.w, 2.0), 0.0, CURVE.with_alpha(0.6));
        dl.pop_clip();
        if c.dental.is_some() {
            // Narrow cells: the number and the position at the top end corner, away from the
            // ruler (their accessible names say "Section").
            let st = TextStyle::new(FONT_BODY * 0.85 * ts).tabular();
            let pad = 4.0 * ts;
            let mut y = rr.y + pad + 2.0;
            for (k, line) in [(i + 1).to_string(), length(*at, cx.length)]
                .iter()
                .enumerate()
            {
                let b = text.layout(line, st, None, Align::Start);
                let x = if rtl {
                    rr.x + pad + 14.0 * ts
                } else {
                    rr.x + rr.w - pad - b.width
                };
                dl.rect(
                    Rect::new(x - 2.0, y - 1.0, b.width + 4.0, b.height + 2.0),
                    RADIUS_SM,
                    Color::rgba(0.0, 0.0, 0.0, 0.6),
                );
                text.draw(
                    dl,
                    &b,
                    x,
                    y,
                    if k == 0 {
                        CURVE
                    } else {
                        Color::rgba(1.0, 1.0, 1.0, 0.95)
                    },
                );
                y += b.height + 2.0;
            }
        } else {
            let mut lines = vec![
                t!("cpr-section"),
                t!("cpr-at", position = length(*at, cx.length)),
            ];
            lines.extend(evar_lines(c, cx.measures, cx.length));
            medical::draw_readout(dl, text, &lines, (rr.x, rr.y, rr.w, rr.h), rtl, ts);
        }
    }
}

fn ring(dl: &mut DrawList, c: (f32, f32), r: f32, col: Color) {
    let n = 24;
    let pt = |i: i32| {
        let a = std::f32::consts::TAU * i as f32 / n as f32;
        (c.0 + r * a.cos(), c.1 + r * a.sin())
    };
    for i in 0..n {
        dl.line(pt(i), pt(i + 1), 3.0, Color::rgba(0.0, 0.0, 0.0, 0.4));
    }
    for i in 0..n {
        dl.line(pt(i), pt(i + 1), 1.5, col);
    }
}

/// A vertical millimetre ruler along the cell's start edge: a tick per mm, longer per 5 mm,
/// labelled every 10 mm; `mm` per logical px.
fn ruler(dl: &mut DrawList, text: &mut Text, r: Rect, mm: f64, rtl: bool, ts: f32) {
    if mm <= 0.0 {
        return;
    }
    let px_per_mm = (1.0 / mm) as f32;
    if px_per_mm < 1.5 {
        return;
    }
    let edge = if rtl { r.x + r.w - 4.0 } else { r.x + 4.0 };
    let dir = if rtl { -1.0 } else { 1.0 };
    let cy = r.y + r.h / 2.0;
    let half = ((r.h / 2.0 - 24.0 * ts) / px_per_mm).floor().max(0.0) as i32;
    let st = TextStyle::new(FONT_BODY * 0.75 * ts).tabular();
    for m in -half..=half {
        let y = cy + m as f32 * px_per_mm;
        let len = if m % 10 == 0 {
            9.0
        } else if m % 5 == 0 {
            6.0
        } else {
            3.0
        } * ts;
        dl.line(
            (edge, y),
            (edge + dir * len, y),
            2.5,
            Color::rgba(0.0, 0.0, 0.0, 0.5),
        );
        dl.line(
            (edge, y),
            (edge + dir * len, y),
            1.0,
            Color::rgba(1.0, 1.0, 1.0, 0.85),
        );
        if m % 10 == 0 && m != 0 {
            let b = text.layout(&(m.abs()).to_string(), st, None, Align::Start);
            let x = if rtl {
                edge - 11.0 * ts - b.width
            } else {
                edge + 11.0 * ts
            };
            text.draw(
                dl,
                &b,
                x,
                y - b.height / 2.0,
                Color::rgba(1.0, 1.0, 1.0, 0.9),
            );
        }
    }
}

impl Control {
    /// Curved MPR's controls' names (the slot's other controls are named in `draw`).
    pub(crate) fn curve_label(self) -> Option<String> {
        Some(match self {
            Control::CurveKind(k) => t!("cpr-kind", kind = k.id()),
            Control::CurvePlacing => t!("cpr-add-points"),
            Control::CurveUndo => t!("cpr-undo-point"),
            Control::CurveMark => t!("cpr-mark"),
            Control::CurveClear => t!("cpr-clear"),
            Control::CurveSave => t!("cpr-save"),
            Control::CurveLoad => t!("cpr-load"),
            Control::CurveDone => t!("cpr-done"),
            Control::CurveNew => t!("cpr-new"),
            Control::CurveRename => t!("cpr-rename"),
            Control::CurveDelete => t!("cpr-delete"),
            Control::Nerve => t!("dental-nerve"),
            Control::Spacing(mm) => t!("dental-spacing", spacing = number(mm)),
            Control::PanoSlab(mm) if mm > 0.0 => t!("cpr-panoramic-slab", thickness = number(mm)),
            Control::PanoSlab(_) => t!("cpr-panoramic-thin"),
            Control::Curve => t!("cpr-button"),
            Control::Dental => t!("dental-button"),
            Control::ExportSegments => t!("seg-stl-export-button"),
            Control::SegmentDetail(n) => super::curve_app::detail_text(n),
            _ => return None,
        })
    }
}

/// The name field's laid-out text and the x it starts at (logical px), as drawn.
pub(crate) type FieldText = Option<(Arc<TextBlock>, f32)>;

/// The reformat's controls in rows from its top start corner, and the name field's text
/// as drawn.
fn toolbar(
    dl: &mut DrawList,
    text: &mut Text,
    c: &Curved,
    r: R,
    pal: &Palette,
    (rtl, paint): (bool, bool),
    ts: f32,
) -> (Vec<(Control, Rect, bool)>, FieldText) {
    let mut items: Vec<(Control, bool)> = Vec::new();
    match &c.dental {
        None => {
            // The volume's centrelines (the chosen one marked, or its name field while it
            // is renamed), then new, rename and delete.
            for i in 0..c.list.len() {
                if i == c.index && c.rename.is_some() {
                    items.push((Control::CurveRenameField, true));
                } else {
                    items.push((Control::CurveName(i), i == c.index));
                }
            }
            items.push((Control::CurveNew, false));
            if !c.list.is_empty() {
                items.push((Control::CurveRename, false));
                items.push((Control::CurveDelete, false));
            }
            items.push((Control::CurveKind(c.kind), false));
            if c.spline().is_some() {
                items.push((Control::CurveMark, c.mark_at_cursor().is_some()));
            }
        }
        Some(d) => {
            items.push((Control::PanoSlab(d.slab_mm()), false));
            items.push((Control::Spacing(d.spacing_mm()), false));
            items.push((Control::Nerve, d.tracing));
        }
    }
    items.push((Control::CurvePlacing, c.placing));
    items.push((Control::CurveUndo, false));
    items.push((Control::CurveClear, false));
    items.push((Control::CurveSave, false));
    items.push((Control::CurveLoad, false));
    items.push((Control::CurveDone, false));
    let st = TextStyle::new(FONT_BODY * ts);
    let pad = 8.0 * ts;
    let margin = PAD * ts;
    let rr = rect(r);
    let bh = 24.0 * ts;
    let gap = 4.0 * ts;
    let avail = rr.w - 2.0 * margin;
    let mut out = Vec::new();
    let mut field_text = None;
    let (mut x, mut y) = (0.0f32, rr.y + margin);
    for (ctl, on) in items {
        let label = c.control_label(ctl);
        let field = ctl == Control::CurveRenameField;
        // The name field shows the composition at the caret and is wide enough to type in.
        let (shown, composing) = match (&c.rename, field) {
            (Some((t, caret, pre)), true) => specular_widgets::line_edit::compose(t, *caret, pre),
            _ => (label, 0..0),
        };
        let b = text.layout(&shown, st, None, Align::Start);
        let w = if field {
            (b.width + 2.0 * pad + 2.0).max(140.0 * ts)
        } else {
            b.width + 2.0 * pad
        };
        if x > 0.0 && x + w > avail {
            x = 0.0;
            y += bh + gap;
        }
        // Rows stay in the top third of the reformat.
        if y + bh > rr.y + rr.h / 3.0 + bh && x == 0.0 && !out.is_empty() {
            break;
        }
        let bx = if rtl {
            rr.x + rr.w - margin - x - w
        } else {
            rr.x + margin + x
        };
        let br = Rect::new(bx, y, w, bh);
        let (fill, ink) = if on && !field {
            (pal.accent_fill, pal.accent_text)
        } else {
            (pal.surface_raised, pal.text)
        };
        if paint && field {
            dl.rect_border(br, RADIUS_SM, fill, pal.focus_ring, 2.0);
            let room = br.inset_xy(pad, 0.0);
            let cx = b.caret_x(composing.end);
            let x0 = specular_widgets::line_edit::field_x0(&b, cx, room, rtl || b.rtl);
            let ty = y + (bh - b.height) / 2.0;
            dl.push_clip(room.inset_xy(-2.0, 0.0));
            if composing.is_empty() {
                for (a, e) in b.range_spans(c.rename_selection.clone()) {
                    dl.rect(Rect::new(x0 + a, ty, e - a, b.height), 0.0, pal.selection);
                }
            }
            text.draw(dl, &b, x0, ty, ink);
            field_text = Some((b.clone(), x0));
            if !composing.is_empty() {
                specular_widgets::line_edit::draw_preedit_underline(
                    dl,
                    &b,
                    x0,
                    composing.clone(),
                    ty + b.height,
                    pal.text_muted,
                );
            }
            dl.rect(
                Rect::new((x0 + cx - 0.75).round(), ty, 1.5, b.height),
                0.0,
                pal.text,
            );
            dl.pop_clip();
        } else if paint {
            dl.rect_border(br, RADIUS_SM, fill, pal.separator, 1.0);
            text.draw(dl, &b, bx + pad, y + (bh - b.height) / 2.0, ink);
            // The chip with the keyboard focus: a ring outside it.
            if matches!(ctl, Control::CurveName(i) if c.focus == Some(i)) {
                dl.stroke_rect(
                    br.inset(-2.0 * ts),
                    RADIUS_SM + 2.0 * ts,
                    pal.focus_ring,
                    2.0,
                );
            }
        }
        out.push((ctl, br, on));
        x += w + gap;
    }
    (out, field_text)
}

/// AccessKit ids of curved MPR (beside MPR's 0x5900…).
pub(crate) const A11Y_BASE: u64 = 0x5900_0000_0000_0400;

/// The control of the curved MPR node `id` among `placed` (the controls as drawn).
pub(crate) fn a11y_control(placed: &[(Control, Rect, bool)], id: u64) -> Option<Control> {
    let i = id.checked_sub(A11Y_BASE + 0x20)?;
    placed.get(usize::try_from(i).ok()?).map(|x| x.0)
}

/// Nodes for the reformat and the cross-sections (images named by their readouts) and a
/// group of the reformat's controls.
pub(crate) fn a11y(tree: &mut accesskit::TreeUpdate, c: &Curved, length: Length, scale: f64) {
    use accesskit::{Action, Node, NodeId, Role};
    let Some(rects) = &c.rects else { return };
    let bounds = |r: Rect| accesskit::Rect {
        x0: f64::from(r.x) * scale,
        y0: f64::from(r.y) * scale,
        x1: f64::from(r.x + r.w) * scale,
        y1: f64::from(r.y + r.h) * scale,
    };
    let mut children = Vec::new();
    let mut image = |id: u64, label: String, r: R, tree: &mut accesskit::TreeUpdate| {
        let mut n = Node::new(Role::Image);
        n.set_label(label);
        n.set_bounds(bounds(rect(r)));
        tree.nodes.push((NodeId(A11Y_BASE + id), n));
        children.push(NodeId(A11Y_BASE + id));
    };
    let len = c.spline.as_ref().map_or(0.0, Spline::length);
    let name = match &c.dental {
        Some(_) => t!("cpr-panoramic"),
        None => t!("cpr-kind", kind = c.kind.id()),
    };
    let label = match &c.spline {
        Some(_) => t!(
            "cpr-a11y",
            name = name,
            length = super::length_text(len, length),
            position = super::length_text(c.cursor, length)
        ),
        None => t!(
            "cpr-hint-start",
            view = super::view_name(c.view.min(2), true, false)
        ),
    };
    image(0, label, rects.cpr, tree);
    let cells: Vec<(R, f64)> = match &c.dental {
        Some(_) => rects
            .cells
            .iter()
            .copied()
            .zip(c.cell_positions())
            .collect(),
        None => vec![(rects.xsec, c.cursor)],
    };
    for (i, (r, at)) in cells.into_iter().enumerate() {
        if empty(r) || c.spline.is_none() {
            continue;
        }
        let what = if c.dental.is_some() {
            t!("dental-section", number = (i + 1) as i64)
        } else {
            t!("cpr-section")
        };
        let label = format!(
            "{what}, {}",
            t!("cpr-at", position = super::length_text(at, length))
        );
        image(1 + i as u64, label, r, tree);
    }
    let mut group = Node::new(Role::Group);
    group.set_label(t!("cpr-controls"));
    group.set_bounds(bounds(rect(rects.cpr)));
    for (i, (ctl, r, on)) in c.placed.iter().enumerate() {
        let field = *ctl == Control::CurveRenameField;
        let mut n = Node::new(if field { Role::TextInput } else { Role::Button });
        if field {
            n.set_label(t!("cpr-name"));
            n.set_value(c.control_label(*ctl));
        } else {
            n.set_label(c.control_label(*ctl));
            n.add_action(Action::Click);
        }
        n.set_bounds(bounds(*r));
        let id = NodeId(A11Y_BASE + 0x20 + i as u64);
        if let Control::CurveName(k) = *ctl {
            // A centreline: Enter chooses it, F2 renames it, Delete asks to delete it.
            n.set_selected(*on);
            n.add_action(Action::Focus);
            n.set_keyboard_shortcut("Enter F2 Delete");
            if c.focus == Some(k) {
                tree.focus = id;
            }
        }
        if field && c.rename.is_some() {
            tree.focus = id;
        }
        if matches!(ctl, Control::CurvePlacing | Control::Nerve) {
            n.set_toggled(if *on {
                accesskit::Toggled::True
            } else {
                accesskit::Toggled::False
            });
        }
        tree.nodes.push((id, n));
        group.push_child(id);
    }
    let gid = NodeId(A11Y_BASE + 0x10);
    tree.nodes.push((gid, group));
    children.push(gid);
    if let Some((_, root)) = tree.nodes.iter_mut().find(|(id, _)| *id == NodeId(1)) {
        for ch in children {
            root.push_child(ch);
        }
    }
}

// ----- Sidecar ------------------------------------------------------------------------------

/// The sidecar's format marker and version.
const SIDECAR: &str = "specular-curves";

/// The curve (and the canal, in the dental layout) as JSON, in patient mm.
#[cfg(test)]
pub(crate) fn to_json(c: &Curved, frame_of_reference: Option<&str>) -> String {
    to_json_all(&[c], frame_of_reference)
}

/// Curves (centrelines, or the dental arch and its canal) as JSON, in patient mm, each with
/// its name when it has one.
pub(crate) fn to_json_all(curves: &[&Curved], frame_of_reference: Option<&str>) -> String {
    use serde_json::json;
    let pts = |p: &[V3]| p.iter().map(|q| json!(q)).collect::<Vec<_>>();
    let curves: Vec<_> = curves
        .iter()
        .map(|c| {
            let mut curve = json!({
                "points": pts(&c.points),
                "up": c.up,
                "layout": c.kind.id(),
                "angle": c.angle,
            });
            if !c.name.trim().is_empty() {
                curve["name"] = json!(c.name.trim());
            }
            if !c.marks.is_empty() {
                curve["marks"] = json!(c.marks);
            }
            if let Some(d) = &c.dental {
                curve["dental"] = json!({ "nerve": pts(&d.nerve), "spacing": d.spacing_mm() });
            }
            curve
        })
        .collect();
    let mut out = json!({
        "format": SIDECAR,
        "version": 1,
        "unit": "mm",
        "coordinates": "patient (LPS)",
        "curves": curves,
    });
    if let Some(f) = frame_of_reference.filter(|f| !f.is_empty()) {
        out["frame_of_reference"] = json!(f);
    }
    serde_json::to_string_pretty(&out).unwrap_or_default()
}

/// A curve read back: its name, points, up vector, layout, turn, and the dental canal and
/// spacing.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Loaded {
    pub name: String,
    pub points: Vec<V3>,
    pub up: Option<V3>,
    pub kind: CprKind,
    pub angle: f64,
    pub dental: Option<(Vec<V3>, f64)>,
    /// EVAR marks (arc lengths, mm).
    pub marks: Vec<f64>,
}

/// Reads a sidecar written by [`to_json_all`] (its first curve). Errors are plain reasons.
#[cfg(test)]
pub(crate) fn from_json(s: &str) -> Result<Loaded, String> {
    from_json_all(s)?
        .into_iter()
        .next()
        .ok_or_else(|| t!("cpr-load-empty"))
}

/// Most curves one sidecar gives.
pub(crate) const MAX_CURVES: usize = 64;

/// Every curve of a sidecar (curves of fewer than two points left out); an error when it
/// is not a sidecar or holds no curve.
pub(crate) fn from_json_all(s: &str) -> Result<Vec<Loaded>, String> {
    let v: serde_json::Value = serde_json::from_str(s).map_err(|_| t!("cpr-load-not-json"))?;
    if v.get("format").and_then(|f| f.as_str()) != Some(SIDECAR) {
        return Err(t!("cpr-load-not-curves"));
    }
    let curves = v
        .get("curves")
        .and_then(|c| c.as_array())
        .ok_or_else(|| t!("cpr-load-empty"))?;
    let out: Vec<Loaded> = curves
        .iter()
        .take(MAX_CURVES)
        .filter_map(|c| curve_of(c).ok())
        .collect();
    if out.is_empty() {
        return Err(t!("cpr-load-empty"));
    }
    Ok(out)
}

fn curve_of(curve: &serde_json::Value) -> Result<Loaded, String> {
    let v3 = |x: &serde_json::Value| -> Option<V3> {
        let a = x.as_array()?;
        if a.len() != 3 {
            return None;
        }
        let p = [a[0].as_f64()?, a[1].as_f64()?, a[2].as_f64()?];
        p.iter().all(|c| c.is_finite()).then_some(p)
    };
    let list = |x: Option<&serde_json::Value>| -> Vec<V3> {
        x.and_then(|x| x.as_array())
            .map(|a| {
                a.iter()
                    .take(specular_render_volume::curve::MAX_POINTS)
                    .filter_map(v3)
                    .collect()
            })
            .unwrap_or_default()
    };
    let points = list(curve.get("points"));
    if points.len() < 2 {
        return Err(t!("cpr-load-empty"));
    }
    let kind = match curve.get("layout").and_then(|k| k.as_str()) {
        Some("stretched") => CprKind::Stretched,
        _ => CprKind::Straightened,
    };
    let dental = curve.get("dental").map(|d| {
        let spacing = d
            .get("spacing")
            .and_then(|s| s.as_f64())
            .unwrap_or(SPACINGS[0]);
        (list(d.get("nerve")), spacing)
    });
    let mut marks: Vec<f64> = curve
        .get("marks")
        .and_then(|m| m.as_array())
        .map(|a| {
            a.iter()
                .filter_map(serde_json::Value::as_f64)
                .filter(|m| m.is_finite() && *m >= 0.0)
                .take(MAX_MARKS)
                .collect()
        })
        .unwrap_or_default();
    marks.sort_by(f64::total_cmp);
    Ok(Loaded {
        marks,
        name: curve
            .get("name")
            .and_then(|n| n.as_str())
            .map(|n| n.chars().filter(|c| !c.is_control()).take(80).collect())
            .unwrap_or_default(),
        points,
        up: curve.get("up").and_then(v3),
        kind,
        angle: curve
            .get("angle")
            .and_then(|a| a.as_f64())
            .filter(|a| a.is_finite())
            .unwrap_or(0.0),
        dental,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A cross-section measurement's first click shows a mark where it was made (on that
    /// cross-section only), before the second click finishes the length.
    #[test]
    fn a_measurement_first_point_is_marked() {
        let plane = Plane {
            rect: [0, 0, 200, 100],
            origin: [10.0, 20.0, 30.0],
            du: [0.5, 0.0, 0.0],
            dv: [0.0, 0.5, 0.0],
            thickness: 0.0,
            slab: Slab::Mip,
            window: WlWindow {
                center: 0.0,
                width: 1.0,
                shape: specular_render::VoiShape::Linear,
                invert: false,
            },
            canvas: [0.0; 4],
        };
        // Patient (15, 25, 30) is target pixel (10, 10): logical (5, 5) at scale 2.
        let at = plane.at(10.0, 10.0);
        assert_eq!(draft_mark(&plane, at, 2.0), Some((5.0, 5.0)));
        // A point on another cross-section (2 mm off this plane) is not marked here.
        assert_eq!(draft_mark(&plane, [15.0, 25.0, 32.0], 2.0), None);
    }

    #[test]
    fn the_sidecar_round_trips() {
        let mut c = Curved::new(0, [0.0, 0.0, 1.0]);
        c.set_points(vec![[1.0, 2.0, 3.0], [10.5, -4.0, 3.0], [20.0, 0.25, 7.0]]);
        c.kind = CprKind::Stretched;
        c.angle = 0.3;
        let mut d = Dental::new(None);
        d.set_nerve(vec![[5.0, 5.0, -18.0], [15.0, 5.0, -18.0]]);
        d.spacing = 2;
        c.dental = Some(d);
        let s = to_json(&c, Some("1.2.3"));
        let l = from_json(&s).unwrap();
        assert_eq!(l.points, c.points());
        assert_eq!(l.up, Some([0.0, 0.0, 1.0]));
        assert_eq!(l.kind, CprKind::Stretched);
        assert!((l.angle - 0.3).abs() < 1e-12);
        let (nerve, spacing) = l.dental.unwrap();
        assert_eq!(nerve.len(), 2);
        assert_eq!(spacing, 3.0);
        assert!(s.contains("\"frame_of_reference\": \"1.2.3\""));
    }

    /// Several named centrelines go into one sidecar and all come back, in order; a curve of
    /// one point among them is left out.
    #[test]
    fn every_centreline_round_trips() {
        let mut a = Curved::new(0, [0.0, 0.0, 1.0]);
        a.set_points(vec![[0.0; 3], [10.0, 0.0, 0.0]]);
        a.name = "LAD".into();
        let mut b = Curved::new(1, [0.0, 1.0, 0.0]);
        b.set_points(vec![[0.0; 3], [0.0, 0.0, 5.0], [3.0, 0.0, 9.0]]);
        let s = to_json_all(&[&a, &b], None);
        let l = from_json_all(&s).unwrap();
        assert_eq!(l.len(), 2);
        assert_eq!((l[0].name.as_str(), l[0].points.len()), ("LAD", 2));
        assert_eq!((l[1].name.as_str(), l[1].points.len()), ("", 3));
        let mixed = "{\"format\": \"specular-curves\", \"curves\": [{\"points\": [[0,0,0]]}, {\"name\": \"RCA\", \"points\": [[0,0,0],[1,1,1]]}]}";
        let l = from_json_all(mixed).unwrap();
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].name, "RCA");
    }

    #[test]
    fn bad_sidecars_are_refused_with_a_reason() {
        assert!(from_json("not json").is_err());
        assert!(from_json("{\"format\": \"other\"}").is_err());
        let one = "{\"format\": \"specular-curves\", \"curves\": [{\"points\": [[0,0,0]]}]}";
        assert!(from_json(one).is_err());
        let bad = "{\"format\": \"specular-curves\", \"curves\": [{\"points\": [[0,0,0],[1,\"x\",2],[3,3,3]]}]}";
        assert_eq!(from_json(bad).unwrap().points.len(), 2);
    }

    #[test]
    fn dental_makers_are_recognised_by_manufacturer() {
        assert_eq!(cbct_vendor("PLANMECA"), Some("Planmeca"));
        assert_eq!(cbct_vendor("Carestream Health"), Some("Carestream"));
        assert_eq!(
            cbct_vendor("Sirona Dental Systems GmbH"),
            Some("Dentsply Sirona")
        );
        assert_eq!(cbct_vendor("VATECH"), Some("Vatech"));
        assert_eq!(cbct_vendor("E-WOO Technology"), Some("Vatech"));
        assert_eq!(cbct_vendor("SIEMENS"), None);
    }

    #[test]
    fn the_layout_tiles_the_viewport() {
        for dental in [false, true] {
            for rtl in [false, true] {
                let (l, r) = layout([0.0, 0.0, 900.0, 600.0], 1.5, rtl, 1, dental);
                assert!(empty(l.views[0]) && empty(l.views[2]) && !empty(l.views[1]));
                let area = |x: R| x[2] * x[3];
                let total = area(r.cpr) + area(r.xsec) + area(l.views[1]) + area(l.slot);
                assert!(
                    total <= 900.0 * 600.0 && total > 0.97 * 900.0 * 600.0,
                    "{total}"
                );
                assert_eq!(r.cells.len(), if dental { CELLS } else { 0 });
                for c in &r.cells {
                    assert!(
                        c[0] >= r.xsec[0] - 1e-9 && c[0] + c[2] <= r.xsec[0] + r.xsec[2] + 1e-9
                    );
                }
                // The view sits at the start: the left in left-to-right layouts.
                assert_eq!(l.views[1][0] < l.slot[0], !rtl);
            }
        }
    }

    #[test]
    fn points_edit_the_spline_and_the_cursor_stays_on_it() {
        let mut c = Curved::new(0, [0.0, 0.0, 1.0]);
        c.push([0.0; 3]);
        assert!(c.spline().is_none());
        c.push([30.0, 0.0, 0.0]);
        assert!((c.spline().unwrap().length() - 30.0).abs() < 1e-9);
        assert!((c.cursor - 15.0).abs() < 1e-9);
        c.step(100.0);
        assert!((c.cursor - 30.0).abs() < 1e-9);
        c.move_point(1, [10.0, 0.0, 0.0]);
        assert!(c.cursor <= 10.0);
        assert!(c.pop());
        assert!(c.spline().is_none());
    }

    /// EVAR marks: set and removed at the cursor, the length along the centreline between
    /// them, and the diameters measured on each mark's cross-section; they ride in the
    /// sidecar.
    #[test]
    fn evar_marks_lengths_and_diameters() {
        // A straight centreline along +z, 100 mm.
        let mut c = Curved::new(0, [1.0, 0.0, 0.0]);
        c.set_points(vec![[0.0, 0.0, 0.0], [0.0, 0.0, 50.0], [0.0, 0.0, 100.0]]);
        let len = c.spline().unwrap().length();
        assert!((len - 100.0).abs() < 1e-6, "{len}");
        for at in [20.0, 70.0, 95.0] {
            c.cursor = at;
            assert!(c.toggle_mark());
        }
        // The cursor on a mark removes it.
        c.cursor = 95.2;
        assert!(!c.toggle_mark());
        assert_eq!(c.marks.len(), 2);
        c.cursor = 95.0;
        assert!(c.toggle_mark());
        // Two hand-drawn diameters on the section at 20 mm, one on 70 mm, one off-plane.
        let m = [
            [[-11.0, 0.0, 20.0], [11.0, 0.0, 20.0]],
            [[0.0, -12.5, 20.2], [0.0, 12.5, 19.8]],
            [[-9.0, 0.0, 70.0], [9.0, 0.0, 70.0]],
            [[-9.0, 0.0, 40.0], [9.0, 0.0, 42.0]],
        ];
        let rows = evar(&c, &m);
        assert_eq!(rows.len(), 3);
        assert!(rows[0].from_prev.is_none());
        assert!((rows[1].from_prev.unwrap() - 50.0).abs() < 1e-6);
        assert!((rows[2].from_prev.unwrap() - 25.0).abs() < 1e-6);
        let (min, max, n) = rows[0].diameters.unwrap();
        assert_eq!(n, 2);
        assert!((min - 22.0).abs() < 1e-9);
        assert!((max - (25.0f64.powi(2) + 0.4f64.powi(2)).sqrt()).abs() < 1e-9);
        assert_eq!(rows[1].diameters.map(|d| d.2), Some(1));
        assert!(rows[2].diameters.is_none());
        // The lines under the cross-section at mark 1.
        c.cursor = 20.0;
        let en = specular_i18n::langid!("en-US");
        let lines =
            specular_i18n::I18n::with_thread_language(en, || evar_lines(&c, &m, Length::Mm));
        let plain: Vec<String> = lines
            .iter()
            .map(|l| l.replace(['\u{2068}', '\u{2069}'], ""))
            .collect();
        assert_eq!(plain[0], "Mark 1");
        assert!(plain[1].starts_with("Diameter 22"), "{plain:?}");
        assert!(
            plain.iter().any(|l| l.starts_with("Mark 1 to 2: 50")),
            "{plain:?}"
        );
        assert!(
            plain.iter().any(|l| l.starts_with("All marks: 75")),
            "{plain:?}"
        );
        // The sidecar keeps them.
        let back = from_json(&to_json(&c, None)).unwrap();
        assert_eq!(back.marks, c.marks);
    }

    /// Editing the centreline carries the EVAR marks along: each keeps its place between the
    /// same two points; the stretch undone takes its marks with it; a cleared curve put back
    /// has them where they were.
    #[test]
    fn evar_marks_follow_the_centreline() {
        let mut c = Curved::new(0, [1.0, 0.0, 0.0]);
        c.set_points(vec![[0.0, 0.0, 0.0], [0.0, 0.0, 50.0], [0.0, 0.0, 100.0]]);
        for at in [25.0, 50.0, 90.0] {
            c.cursor = at;
            assert!(c.toggle_mark());
        }
        // The last point moved 40 mm further: the stretch 50–100 becomes 50–140.
        c.move_point(2, [0.0, 0.0, 140.0]);
        let s = c.spline().unwrap();
        assert!((s.knot(1) - 50.0).abs() < 1e-6);
        assert_eq!(c.marks.len(), 3);
        assert!((c.marks[0] - 25.0).abs() < 1e-6, "{:?}", c.marks);
        assert!((c.marks[1] - 50.0).abs() < 1e-6, "{:?}", c.marks);
        assert!((c.marks[2] - 122.0).abs() < 1e-6, "{:?}", c.marks);
        // The first point moved back 20 mm: the first stretch grows, its mark keeps its share.
        c.move_point(0, [0.0, 0.0, -50.0]);
        assert!((c.marks[0] - 50.0).abs() < 1e-6, "{:?}", c.marks);
        assert!((c.marks[1] - 100.0).abs() < 1e-6, "{:?}", c.marks);
        // A point added at the end leaves them; undoing it too.
        let before = c.marks.clone();
        c.push([0.0, 0.0, 200.0]);
        assert_eq!(c.marks.len(), 3);
        assert!(c.pop());
        for (a, b) in before.iter().zip(&c.marks) {
            assert!((a - b).abs() < 1e-6);
        }
        // Undoing the last point drops the mark on the stretch it ends.
        assert!(c.pop());
        assert_eq!(c.marks.len(), 2, "{:?}", c.marks);
        // Cleared and put back: the marks are where they were.
        let kept = c.marks.clone();
        let pts = c.points().to_vec();
        c.set_points(Vec::new());
        c.set_points(pts);
        assert_eq!(c.marks, kept);
        // Every mark stays on the curve.
        let len = c.spline().unwrap().length();
        assert!(c.marks.iter().all(|m| (0.0..=len).contains(m)));
    }
}
