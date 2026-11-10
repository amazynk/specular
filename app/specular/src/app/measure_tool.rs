//! The measurement tool: lengths, angles and regions over any image,
//! with their statistics, in one list per image that copies as CSV and saves as CSV, GeoJSON
//! or ImageJ ROI. The geometry, pixel coverage and statistics are `specular-measure`'s; this
//! module draws, takes input and runs the pixel work off the UI thread.
//!
//! - Points are stored pixel coordinates (pixel (i, j) covers i..i + 1 × j..j + 1), so the
//!   measurements follow zoom, pan, rotation and flips.
//! - Statistics come from the full-resolution overview when there is one, else from a region
//!   decode of the shape's bounds, else from the reduced overview, and are then marked
//!   approximate (and redone once the full-resolution overview arrives).
//! - Calibration: DICOM Pixel Spacing or Imager Pixel Spacing when the file states one, else
//!   pixels, until the user calibrates from a known length. Every value shows its source.
//! - Keyboard: the arrows move a crosshair (Shift: ten pixels), Space places a point, Enter
//!   closes a polygon, Tab walks the buttons and the list, letters pick the tools. A
//!   geometry point: `[` and `]` go to it, Space picks it up, the arrows move it, Space or
//!   Enter puts it down, Esc puts it back.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crossbeam_channel::{Receiver, Sender};
use specular_core::{CancelToken, PixelLayout, Raster};
use specular_decode::drawing::{DrawingScale, ScaleRegion};
use specular_decode::science::{NdPlane, TemperatureUnit};
use specular_decode::{DecodeRequest, Rect as Region, Session};
use specular_gfx::winit::event_loop::ActiveEventLoop;
use specular_gfx::{Align, DrawList, Rect, Text, TextStyle};
use specular_i18n::t;
use specular_input::{Action, Group, Key, KeyCombo, NamedKey};
use specular_inspect::{SourceValue, source_pixel};
use specular_log::{UI, debug, warn};
use specular_measure::calib::{Calibration, Source, Unit, format_sig};
use specular_measure::geom::{P, Shape};
use specular_measure::list::{List, Measurement};
use specular_measure::provenance::{self, Provenance};
use specular_measure::raster::{Samples, View};
use specular_measure::stats::{self, ChannelStats};
use specular_measure::{csv, geojson, imagej};
use specular_theme::metrics::{FONT_LABEL, FONT_SMALL, RADIUS_LG, RADIUS_SM};
use specular_theme::{Color, Palette};
use specular_widgets::accesskit;

use specular_measure::clinical::Template;

use super::App;
use crate::loader::{Copied, OverviewKind};

mod geometry;
mod probe;
#[cfg(test)]
mod shots;
mod sr;

use specular_measure::inspect::Probe;

const A11Y_BASE: u64 = 0x5C00_0000_0000_0000;
/// Histogram bins of exported statistics.
const BINS: usize = 256;
/// A press and release closer than this (logical px) is a click, not a drag.
const CLICK_SLOP: f32 = 4.0;
/// A click this close (logical px) to a polygon's first corner closes it.
const CLOSE_RADIUS: f32 = 8.0;
/// Units offered when calibrating from a known length.
const UNITS: [Unit; 6] = [Unit::Mm, Unit::Cm, Unit::M, Unit::Um, Unit::Nm, Unit::Inch];

/// A drawing tool.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    Line,
    /// Three points, the vertex second.
    Angle,
    Rect,
    Ellipse,
    Polygon,
    /// A line of known length that sets the image's calibration.
    Calibrate,
    /// One pixel's value (a thermal image's spot temperature); offered where pixels are
    /// values.
    Spot,
    /// A vet, orthopaedic or cephalometric template's points ([`geometry`]); offered with
    /// the medical tools on.
    Geometry,
    /// An inspection probe ([`probe`]): line profile, SNR, IQI wires, crack width, density.
    Inspect,
}

impl Kind {
    pub const ALL: [Kind; 9] = [
        Kind::Line,
        Kind::Angle,
        Kind::Rect,
        Kind::Ellipse,
        Kind::Polygon,
        Kind::Calibrate,
        Kind::Spot,
        Kind::Geometry,
        Kind::Inspect,
    ];

    pub fn label(self) -> String {
        match self {
            Kind::Line => t!("measure-tool-line"),
            Kind::Angle => t!("measure-tool-angle"),
            Kind::Rect => t!("measure-tool-rect"),
            Kind::Ellipse => t!("measure-tool-ellipse"),
            Kind::Polygon => t!("measure-tool-polygon"),
            Kind::Calibrate => t!("measure-tool-calibrate"),
            Kind::Spot => t!("measure-tool-spot"),
            Kind::Geometry => t!("measure-tool-geometry"),
            Kind::Inspect => t!("ndt-tool"),
        }
    }

    pub fn hint(self) -> String {
        match self {
            Kind::Line => t!("measure-tool-hint-line"),
            Kind::Angle => t!("measure-tool-hint-angle"),
            Kind::Rect => t!("measure-tool-hint-rect"),
            Kind::Ellipse => t!("measure-tool-hint-ellipse"),
            Kind::Polygon => t!("measure-tool-hint-polygon"),
            Kind::Calibrate => t!("measure-tool-hint-calibrate"),
            Kind::Spot => t!("measure-tool-hint-spot"),
            Kind::Geometry => t!("measure-tool-hint-geometry"),
            Kind::Inspect => probe::hint(Probe::Profile),
        }
    }

    /// Points that complete the shape (`None`: any number, closed with Enter; a geometry
    /// template's count is its own, see [`Tool::need`]).
    fn points(self) -> Option<usize> {
        match self {
            Kind::Angle => Some(3),
            Kind::Polygon | Kind::Geometry => None,
            Kind::Spot => Some(1),
            _ => Some(2),
        }
    }

    /// Whether a press-drag-release draws it (the tool asks [`Tool::drags`], which knows the
    /// probe).
    #[cfg(test)]
    fn drags(self) -> bool {
        self.points() == Some(2)
    }

    /// The key that picks the tool.
    fn letter(self) -> char {
        match self {
            Kind::Line => 'l',
            Kind::Angle => 'a',
            Kind::Rect => 'r',
            Kind::Ellipse => 'e',
            Kind::Polygon => 'p',
            Kind::Calibrate => 'c',
            Kind::Spot => 's',
            Kind::Geometry => 'g',
            Kind::Inspect => 'i',
        }
    }

    fn node(self) -> u64 {
        1 + Kind::ALL.iter().position(|k| *k == self).unwrap_or(0) as u64
    }
}

/// The shape a tool makes from its points (`None` when degenerate).
pub(super) fn shape_of(kind: Kind, pts: &[P]) -> Option<Shape> {
    let s = match (kind, pts) {
        (Kind::Line | Kind::Calibrate, &[a, b]) => Shape::Line(a, b),
        (Kind::Angle, &[a, vertex, b]) => Shape::Angle { a, vertex, b },
        (Kind::Rect, &[a, b]) => Shape::rect(a, b),
        (Kind::Ellipse, &[a, b]) => Shape::ellipse_in(a, b),
        (Kind::Polygon, v) if v.len() >= 3 => Shape::Polygon(v.to_vec()),
        (Kind::Spot, &[a]) => Shape::Point(a),
        _ => return None,
    };
    let degenerate = match &s {
        Shape::Point(_) => false,
        Shape::Line(a, b) => (*b - *a).hypot() == 0.0,
        Shape::Angle { a, vertex, b } => *a == *vertex || *b == *vertex,
        s => s.area_px().is_none_or(|v| v <= 0.0),
    };
    (s.is_valid() && !degenerate).then_some(s)
}

/// What a click or a screen reader hit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Hit {
    Tool(Kind),
    Undo,
    Delete,
    Copy,
    Export,
    Import,
    /// Calibration: the next unit.
    Unit,
    /// Calibration: apply the typed length.
    Apply,
    /// Geometry: the next template.
    Template,
    /// The inspection probe picked (steps to the next).
    Probe,
    Done,
    Item(usize),
}

/// One measurement as drawn and listed.
#[derive(Clone, Debug, Default)]
pub(super) struct ItemView {
    /// "3. Ellipse".
    pub title: String,
    /// The headline value: a length, an angle or an area.
    pub value: String,
    /// Statistics lines (or that they are being read).
    pub detail: Vec<String>,
    /// Where the calibration comes from.
    pub badge: String,
    /// The value needs a caveat: an estimated calibration, or statistics from a reduced copy.
    pub caveat: bool,
    /// The outline on screen (logical px).
    pub outline: Vec<(f32, f32)>,
    pub closed: bool,
    /// A geometry template's placed points and construction lines (logical px).
    pub handles: Vec<(f32, f32)>,
    pub guides: Vec<Vec<(f32, f32)>>,
    /// A line profile's values, drawn as a small plot.
    pub plot: Vec<f64>,
}

/// Everything the tool draws (logical px).
#[derive(Clone, Debug)]
pub(super) struct MeasureView {
    /// The bar and the list are shown (otherwise only the shapes).
    pub open: bool,
    pub kind: Kind,
    pub items: Vec<ItemView>,
    pub selected: Option<usize>,
    /// The shape being drawn, to the pointer or crosshair.
    pub drawing: Vec<(f32, f32)>,
    pub drawing_closed: bool,
    /// Its placed points.
    pub marks: Vec<(f32, f32)>,
    /// The keyboard crosshair.
    pub cursor: Option<(f32, f32)>,
    /// Under the buttons: the hint, the calibration prompt or the last result.
    pub status: String,
    /// The calibration prompt is open; its unit.
    pub prompt: Option<String>,
    pub can_undo: bool,
    /// Index into [`MeasureView::focusables`].
    pub focus: Option<usize>,
    /// The spot tool is offered (the pixels are values: a thermal image's temperatures).
    pub spot: bool,
    /// The geometry templates are offered (the medical tools are on), and the one picked.
    pub geometry: bool,
    pub template: Template,
    /// The inspection probe picked.
    pub probe: Probe,
}

impl MeasureView {
    fn offers(&self, k: Kind) -> bool {
        match k {
            Kind::Spot => self.spot,
            Kind::Geometry => self.geometry,
            _ => true,
        }
    }

    pub fn focusables(&self) -> Vec<Hit> {
        let mut v: Vec<Hit> = Kind::ALL
            .iter()
            .filter(|k| self.offers(**k))
            .map(|k| Hit::Tool(*k))
            .collect();
        if self.kind == Kind::Geometry {
            v.push(Hit::Template);
        }
        if self.kind == Kind::Inspect {
            v.push(Hit::Probe);
        }
        v.extend([Hit::Undo, Hit::Delete, Hit::Copy, Hit::Export, Hit::Import]);
        if self.prompt.is_some() {
            v.extend([Hit::Unit, Hit::Apply]);
        }
        v.push(Hit::Done);
        v.extend((0..self.items.len()).map(Hit::Item));
        v
    }

    /// Whether a button can be used now.
    pub fn enabled(&self, h: Hit) -> bool {
        match h {
            Hit::Undo => self.can_undo,
            Hit::Delete => self.selected.is_some(),
            Hit::Copy | Hit::Export => !self.items.is_empty(),
            _ => true,
        }
    }

    fn label(&self, h: Hit) -> String {
        match h {
            Hit::Tool(k) => k.label(),
            Hit::Undo => t!("measure-tool-undo"),
            Hit::Delete => t!("measure-tool-delete"),
            Hit::Copy => t!("measure-tool-copy"),
            Hit::Export => t!("measure-tool-export"),
            Hit::Import => t!("measure-tool-import"),
            Hit::Unit => t!(
                "measure-tool-unit",
                unit = self.prompt.clone().unwrap_or_default()
            ),
            Hit::Apply => t!("measure-tool-apply"),
            Hit::Template => t!(
                "measure-tool-template",
                name = geometry::name(self.template)
            ),
            Hit::Probe => t!("ndt-probe", name = probe::label(self.probe)),
            Hit::Done => t!("measure-tool-done"),
            Hit::Item(i) => self.items.get(i).map(item_text).unwrap_or_default(),
        }
    }
}

/// A list row as one sentence (screen readers).
fn item_text(v: &ItemView) -> String {
    let mut s = format!("{}: {}", v.title, v.value);
    for d in &v.detail {
        s.push_str(", ");
        s.push_str(d);
    }
    s.push_str(", ");
    s.push_str(&v.badge);
    s
}

/// Where the controls landed.
#[derive(Clone, Debug, Default)]
pub(super) struct Drawn {
    pub bar: Rect,
    pub panel: Rect,
    pub hits: Vec<(Rect, Hit)>,
}

/// `(line, x)` of items of `widths` flowed into lines of at most `max`, `gap` apart.
fn flow(widths: &[f32], max: f32, gap: f32) -> Vec<(usize, f32)> {
    let mut out = Vec::with_capacity(widths.len());
    let (mut line, mut x) = (0, 0.0);
    for &w in widths {
        if x > 0.0 && x + w > max {
            line += 1;
            x = 0.0;
        }
        out.push((line, x));
        x += w + gap;
    }
    out
}

/// A polyline on screen: a dark under-stroke so it shows on any image.
fn path(dl: &mut DrawList, pts: &[(f32, f32)], closed: bool, color: Color, width: f32) {
    let n = pts.len();
    let segs = if closed && n > 2 {
        n
    } else {
        n.saturating_sub(1)
    };
    for pass in 0..2 {
        let (c, w) = if pass == 0 {
            (Color::rgba(0.0, 0.0, 0.0, 0.7), width + 2.0)
        } else {
            (color, width)
        };
        for i in 0..segs {
            dl.line(pts[i], pts[(i + 1) % n], w, c);
        }
    }
}

/// Draws the measurements over the image, and when the tool is open its bar (top) and list
/// (on the end side) inside `area` (the viewport, logical px).
pub(super) fn draw(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    v: &MeasureView,
    area: Rect,
    rtl: bool,
    ts: f32,
) -> Drawn {
    let focus = v.focus.and_then(|i| v.focusables().get(i).copied());
    let mut drawn = Drawn::default();
    let tag_st = TextStyle::new(FONT_SMALL * ts).weight(650.0);
    dl.push_clip(area);
    for (i, it) in v.items.iter().enumerate() {
        let sel = v.selected == Some(i);
        let c = if sel { pal.accent } else { Color::WHITE };
        if let [(x, y)] = it.outline[..] {
            // A spot: a ring with a cross.
            dl.circle((x, y), 7.0, Color::rgba(0.0, 0.0, 0.0, 0.55));
            for (a, b) in [
                ((x - 10.0, y), (x - 3.0, y)),
                ((x + 3.0, y), (x + 10.0, y)),
                ((x, y - 10.0), (x, y - 3.0)),
                ((x, y + 3.0), (x, y + 10.0)),
            ] {
                path(dl, &[a, b], false, c, if sel { 2.0 } else { 1.5 });
            }
        } else {
            path(dl, &it.outline, it.closed, c, if sel { 2.0 } else { 1.5 });
        }
        // A geometry template: its construction lines, then its points.
        for g in &it.guides {
            path(
                dl,
                g,
                false,
                c.with_alpha(0.9),
                if sel { 1.75 } else { 1.25 },
            );
        }
        for h in &it.handles {
            dl.circle(*h, 4.0, Color::rgba(0.0, 0.0, 0.0, 0.7));
            dl.circle(*h, 2.5, c);
        }
        if let Some(&(x, y)) = it.outline.first().or(it.handles.first()) {
            let n = text.layout(&(i + 1).to_string(), tag_st, None, Align::Start);
            let r = Rect::new(x + 4.0, y - n.height - 6.0, n.width + 8.0, n.height + 2.0);
            dl.rect(r, RADIUS_SM, Color::rgba(0.0, 0.0, 0.0, 0.7));
            text.draw(dl, &n, r.x + 4.0, r.y + 1.0, c);
        }
    }
    if !v.drawing.is_empty() {
        path(dl, &v.drawing, v.drawing_closed, pal.accent, 1.5);
    }
    for m in &v.marks {
        dl.circle(*m, 4.0, Color::rgba(0.0, 0.0, 0.0, 0.7));
        dl.circle(*m, 2.5, pal.accent);
    }
    if let Some((x, y)) = v.cursor {
        for (a, b) in [
            ((x - 12.0, y), (x - 3.0, y)),
            ((x + 3.0, y), (x + 12.0, y)),
            ((x, y - 12.0), (x, y - 3.0)),
            ((x, y + 3.0), (x, y + 12.0)),
        ] {
            path(dl, &[a, b], false, pal.accent, 1.5);
        }
    }
    dl.pop_clip();
    if !v.open {
        return drawn;
    }

    // The bar: title, the tools, the actions; the status line under them.
    let ww = area.x * 2.0 + area.w;
    let m = |r: Rect| if rtl { r.mirror_x(ww) } else { r };
    let title_st = TextStyle::new(FONT_LABEL * ts).weight(650.0);
    let button_st = TextStyle::new(FONT_LABEL * ts).weight(550.0);
    let small_st = TextStyle::new(FONT_SMALL * ts);
    let pad = 12.0;
    let gap = 6.0;
    let bh = (28.0 * ts).max(28.0);
    let max_w = (area.w - 32.0).max(160.0);
    let title = text.layout(&t!("measure-tool-title"), title_st, None, Align::Start);
    let hits: Vec<Hit> = v
        .focusables()
        .into_iter()
        .filter(|h| !matches!(h, Hit::Item(_)))
        .collect();
    let blocks: Vec<_> = hits
        .iter()
        .map(|h| text.layout(&v.label(*h), button_st, None, Align::Start))
        .collect();
    let mut widths = vec![title.width + 10.0];
    widths.extend(blocks.iter().map(|b| (b.width + 20.0).max(44.0)));
    let inner = max_w - 2.0 * pad;
    let places = flow(&widths, inner, gap);
    let lines = places.last().map_or(1, |p| p.0 + 1);
    let used = places
        .iter()
        .zip(&widths)
        .map(|((_, x), w)| x + w)
        .fold(0.0f32, f32::max);
    let status = text.layout(&v.status, small_st, Some(inner), Align::Center);
    let w = (used.max(status.width) + 2.0 * pad).min(max_w);
    let h = pad + lines as f32 * (bh + gap) + status.height + pad;
    let bar = Rect::new(
        (area.x + (area.w - w) * 0.5).round(),
        area.y + 12.0,
        w.round(),
        h.round(),
    );
    dl.shadow(bar, RADIUS_LG, pal.shadow, 24.0, 8.0);
    dl.rect_border(bar, RADIUS_LG, pal.surface, pal.glass_border, 1.0);
    let at = |i: usize| {
        let (line, x) = places[i];
        Rect::new(
            bar.x + pad + x,
            bar.y + pad + line as f32 * (bh + gap),
            widths[i],
            bh,
        )
    };
    let tr = m(at(0));
    text.draw(
        dl,
        &title,
        if rtl { tr.right() - title.width } else { tr.x },
        tr.y + (bh - title.height) * 0.5,
        pal.text,
    );
    for (k, hit) in hits.iter().enumerate() {
        let r = m(at(k + 1));
        let on = v.enabled(*hit);
        let active = *hit == Hit::Tool(v.kind) || *hit == Hit::Done || *hit == Hit::Apply;
        let (fill, ink) = if active {
            (pal.accent_fill, pal.accent_text)
        } else {
            (pal.surface_raised, pal.text)
        };
        let ink = if on { ink } else { ink.with_alpha(0.45) };
        if active {
            dl.rect(r, RADIUS_SM + 2.0, fill);
        } else {
            dl.rect_border(r, RADIUS_SM + 2.0, fill, pal.separator, 1.0);
        }
        if focus == Some(*hit) {
            dl.rect_border(
                r.inset(-3.0),
                RADIUS_SM + 5.0,
                Color::TRANSPARENT,
                pal.focus_ring,
                2.0,
            );
        }
        let b = &blocks[k];
        text.draw(
            dl,
            b,
            r.x + (r.w - b.width) * 0.5,
            r.y + (bh - b.height) * 0.5,
            ink,
        );
        drawn.hits.push((r, *hit));
    }
    text.draw(
        dl,
        &status,
        bar.x + (bar.w - status.width) * 0.5,
        bar.bottom() - pad - status.height,
        pal.text_muted,
    );
    drawn.bar = bar;

    // The list, on the end side under the bar.
    let pw = (280.0 * ts).min(area.w * 0.45).max(160.0);
    let top = bar.bottom() + 12.0;
    let room = area.bottom() - 12.0 - top;
    if room < 60.0 {
        return drawn;
    }
    let row_st = TextStyle::new(FONT_LABEL * ts).weight(600.0);
    let head = text.layout(
        &t!("measure-tool-count", count = v.items.len() as i64),
        title_st,
        Some(pw - 2.0 * pad),
        Align::Start,
    );
    let tw = pw - 2.0 * pad - 8.0;
    let rows: Vec<_> = v
        .items
        .iter()
        .map(|it| {
            let title = text.layout(&it.title, row_st, Some(tw), Align::Start);
            let value = text.layout(&it.value, button_st, Some(tw), Align::Start);
            let detail: Vec<_> = it
                .detail
                .iter()
                .map(|d| text.layout(d, small_st, Some(tw), Align::Start))
                .collect();
            let badge = text.layout(&it.badge, small_st, Some(tw - 12.0), Align::Start);
            let plot_h = if it.plot.len() > 1 { PLOT_H + 4.0 } else { 0.0 };
            let h = 8.0
                + plot_h
                + title.height
                + 2.0
                + value.height
                + detail.iter().map(|d| d.height + 2.0).sum::<f32>()
                + 4.0
                + badge.height
                + 4.0
                + 8.0;
            (title, value, detail, badge, h)
        })
        .collect();
    let empty = v
        .items
        .is_empty()
        .then(|| text.layout(&t!("measure-tool-empty"), small_st, Some(tw), Align::Start));
    let content = head.height
        + 8.0
        + rows.iter().map(|r| r.4 + 4.0).sum::<f32>()
        + empty.as_ref().map_or(0.0, |e| e.height + 8.0);
    let ph = (pad + content + pad).min(room);
    let panel = m(Rect::new(area.right() - 12.0 - pw, top, pw, ph.round()));
    dl.shadow(panel, RADIUS_LG, pal.shadow, 24.0, 8.0);
    dl.rect_border(panel, RADIUS_LG, pal.surface, pal.glass_border, 1.0);
    let sx = |r: Rect, w: f32| if rtl { r.right() - pad - w } else { r.x + pad };
    text.draw(dl, &head, sx(panel, head.width), panel.y + pad, pal.text);
    let mut y = panel.y + pad + head.height + 8.0;
    if let Some(e) = &empty {
        text.draw(dl, e, sx(panel, e.width), y, pal.text_muted);
    }
    // Keep the selected (or focused) row in view.
    let want = match focus {
        Some(Hit::Item(i)) => Some(i),
        _ => v.selected,
    };
    let bottom = panel.bottom() - pad;
    let mut first = 0;
    if let Some(want) = want {
        while first < want && y + rows[first..=want].iter().map(|r| r.4 + 4.0).sum::<f32>() > bottom
        {
            first += 1;
        }
    }
    dl.push_clip(panel.inset(1.0));
    for (i, (title, value, detail, badge, h)) in rows.iter().enumerate().skip(first) {
        if y + h > bottom + 0.5 {
            break;
        }
        let r = Rect::new(panel.x + 6.0, y, panel.w - 12.0, *h);
        let sel = v.selected == Some(i);
        if sel {
            dl.rect(r, RADIUS_SM + 2.0, pal.accent.with_alpha(0.18));
        }
        if focus == Some(Hit::Item(i)) {
            dl.rect_border(r, RADIUS_SM + 2.0, Color::TRANSPARENT, pal.focus_ring, 2.0);
        }
        let ix = |w: f32| if rtl { r.right() - 8.0 - w } else { r.x + 8.0 };
        let mut ty = r.y + 8.0;
        text.draw(dl, title, ix(title.width), ty, pal.text);
        ty += title.height + 2.0;
        text.draw(dl, value, ix(value.width), ty, pal.text);
        ty += value.height + 2.0;
        for d in detail {
            text.draw(dl, d, ix(d.width), ty, pal.text_muted);
            ty += d.height + 2.0;
        }
        ty += 2.0;
        let it = &v.items[i];
        if it.plot.len() > 1 {
            let pr = Rect::new(r.x + 8.0, ty, r.w - 16.0, PLOT_H);
            plot(dl, pal, &it.plot, pr);
            ty += PLOT_H + 4.0;
        }
        let br = Rect::new(
            ix(badge.width + 12.0),
            ty,
            badge.width + 12.0,
            badge.height + 4.0,
        );
        let (bf, bi) = if it.caveat {
            (Color::rgba(0.95, 0.65, 0.15, 0.25), pal.text)
        } else {
            (pal.surface_raised, pal.text_muted)
        };
        dl.rect_border(br, RADIUS_SM, bf, pal.separator, 1.0);
        text.draw(dl, badge, br.x + 6.0, br.y + 2.0, bi);
        drawn.hits.push((r, Hit::Item(i)));
        y += h + 4.0;
    }
    dl.pop_clip();
    drawn.panel = panel;
    drawn
}

/// Height of a line profile's plot in the list (logical px).
const PLOT_H: f32 = 56.0;

/// A line profile as a small plot: the values from the line's start (left) to its end,
/// scaled to their range.
fn plot(dl: &mut DrawList, pal: &Palette, values: &[f64], r: Rect) {
    dl.rect_border(r, RADIUS_SM, pal.surface_raised, pal.separator, 1.0);
    let (lo, hi) = values
        .iter()
        .filter(|v| v.is_finite())
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| {
            (a.min(*v), b.max(*v))
        });
    if hi.partial_cmp(&lo) != Some(std::cmp::Ordering::Greater) {
        return;
    }
    let n = values.len() - 1;
    // At most two points a pixel of width.
    let every = (n / (2 * r.w.max(1.0) as usize).max(1)).max(1);
    let pts: Vec<(f32, f32)> = (0..=n)
        .step_by(every)
        .chain(std::iter::once(n))
        .filter(|i| values[*i].is_finite())
        .map(|i| {
            let x = r.x + 3.0 + (r.w - 6.0) * i as f32 / n as f32;
            let t = ((values[i] - lo) / (hi - lo)) as f32;
            (x, r.bottom() - 3.0 - (r.h - 6.0) * t)
        })
        .collect();
    path(dl, &pts, false, pal.accent, 1.5);
}

fn bounds(r: Rect, scale: f64) -> accesskit::Rect {
    accesskit::Rect {
        x0: f64::from(r.x) * scale,
        y0: f64::from(r.y) * scale,
        x1: f64::from(r.x + r.w) * scale,
        y1: f64::from(r.y + r.h) * scale,
    }
}

fn node(h: Hit) -> u64 {
    match h {
        Hit::Tool(k) => k.node(),
        Hit::Undo => 0x10,
        Hit::Delete => 0x11,
        Hit::Copy => 0x12,
        Hit::Export => 0x13,
        Hit::Import => 0x14,
        Hit::Unit => 0x15,
        Hit::Apply => 0x16,
        Hit::Done => 0x17,
        Hit::Template => 0x18,
        Hit::Probe => 0x19,
        Hit::Item(i) => 0x100 + i as u64,
    }
}

/// The control behind AccessKit node `id`.
pub(super) fn target(id: u64, v: &MeasureView) -> Option<Hit> {
    let k = id.checked_sub(A11Y_BASE)?;
    v.focusables().into_iter().find(|h| node(*h) == k)
}

const LIST_NODE: u64 = 0x20;

/// The tool's AccessKit nodes: a toolbar with its buttons, and a list box of the measurements.
pub(super) fn a11y(tree: &mut accesskit::TreeUpdate, v: &MeasureView, d: &Drawn, scale: f64) {
    use accesskit::{Node, NodeId, Role, Toggled};
    if !v.open {
        return;
    }
    let mut bar = Node::new(Role::Toolbar);
    bar.set_label(t!("measure-tool-title"));
    bar.set_description(v.status.clone());
    bar.set_bounds(bounds(d.bar, scale));
    let mut list = Node::new(Role::ListBox);
    list.set_label(t!("measure-tool-list"));
    list.set_bounds(bounds(d.panel, scale));
    list.set_size_of_set(v.items.len());
    let focusables = v.focusables();
    let focus_hit = v.focus.and_then(|i| focusables.get(i).copied());
    let mut focus_id = None;
    let (mut bar_ids, mut list_ids, mut nodes) = (Vec::new(), Vec::new(), Vec::new());
    for h in &focusables {
        let id = NodeId(A11Y_BASE + node(*h));
        let mut n = match h {
            Hit::Item(i) => {
                let mut n = Node::new(Role::ListBoxOption);
                n.set_selected(v.selected == Some(*i));
                n.set_position_in_set(*i + 1);
                n
            }
            Hit::Tool(k) => {
                let mut n = Node::new(Role::Button);
                n.set_toggled(if *k == v.kind {
                    Toggled::True
                } else {
                    Toggled::False
                });
                n
            }
            _ => Node::new(Role::Button),
        };
        n.set_label(v.label(*h));
        if let Some((r, _)) = d.hits.iter().find(|(_, x)| x == h) {
            n.set_bounds(bounds(*r, scale));
        }
        if v.enabled(*h) {
            n.add_action(accesskit::Action::Click);
        } else {
            n.set_disabled();
        }
        n.add_action(accesskit::Action::Focus);
        if focus_hit == Some(*h) {
            focus_id = Some(id);
        }
        if matches!(h, Hit::Item(_)) {
            list_ids.push(id);
        } else {
            bar_ids.push(id);
        }
        nodes.push((id, n));
    }
    bar.set_children(bar_ids);
    list.set_children(list_ids);
    if let Some((_, root)) = tree.nodes.iter_mut().find(|(id, _)| *id == NodeId(1)) {
        root.push_child(NodeId(A11Y_BASE));
        root.push_child(NodeId(A11Y_BASE + LIST_NODE));
    }
    tree.nodes.push((NodeId(A11Y_BASE), bar));
    tree.nodes.push((NodeId(A11Y_BASE + LIST_NODE), list));
    tree.nodes.extend(nodes);
    if let Some(id) = focus_id {
        tree.focus = id;
    }
}

// ---------------------------------------------------------------------------------------------
// Values and pixels.

/// The text of a calibration source (`measure-source-<key>`).
pub(super) fn source_label(s: Source) -> String {
    specular_i18n::I18n::global().tr(&format!("measure-source-{}", s.key()))
}

/// The source badge: the drawing's scale and viewport when the value went through one.
pub(super) fn badge(s: Source, region: Option<&ScaleRegion>) -> String {
    match (s, region.and_then(|r| Some((r.ratio.clone()?, r.viewport)))) {
        (Source::DrawingScale, Some((scale, Some(v)))) => t!(
            "measure-source-drawing-scale-viewport",
            scale = scale,
            viewport = v as i64
        ),
        (Source::DrawingScale, Some((scale, None))) => {
            t!("measure-source-drawing-scale-ratio", scale = scale)
        }
        _ => source_label(s),
    }
}

/// Where a shape is for picking its drawing-scale region: the middle of its points' box.
fn anchor(pts: &[P]) -> (f64, f64) {
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for p in pts {
        x0 = x0.min(p.x);
        y0 = y0.min(p.y);
        x1 = x1.max(p.x);
        y1 = y1.max(p.y);
    }
    ((x0 + x1) / 2.0, (y0 + y1) / 2.0)
}

/// The headline value of a measurement, written as the drawing writes it when its region
/// says how (PDF number formats, CAD unit styles).
pub(super) fn value_text(m: &Measurement, region: Option<&ScaleRegion>) -> String {
    use specular_measure::calib::Kind as QKind;
    use specular_measure::scale::format_chain;
    // The drawing's units as the UI's language writes them (Hebrew "מ׳"); its own labels
    // (', ", "units") as they are.
    let local = |chain: &[specular_measure::scale::NumberFormat]| -> Vec<_> {
        chain
            .iter()
            .map(|f| specular_measure::scale::NumberFormat {
                unit: specular_i18n::unit_symbol(&f.unit),
                ..f.clone()
            })
            .collect()
    };
    let styled = |q: &specular_measure::Quantity| {
        let r = region.filter(|r| r.cal.unit == q.unit)?;
        match q.kind {
            QKind::Length => format_chain(q.value, &local(&r.distance)),
            QKind::Area => format_chain(q.value, &local(&r.area)),
            QKind::Angle => None,
        }
    };
    if let Shape::Point(_) = m.shape {
        return match (&m.stats, m.channels.first()) {
            (Some(st), Some(unit)) if st.len() == 1 => {
                format!("{} {unit}", format_sig(st[0].mean, 4))
            }
            _ => String::new(),
        };
    }
    if let Some(q) = m.length() {
        return styled(&q)
            .unwrap_or_else(|| length_with_unit(format_sig(q.value, 4), q.unit.symbol()));
    }
    if let Some(q) = m.angle() {
        return styled(&q).unwrap_or_else(|| q.to_string());
    }
    match m.area() {
        Some(q) => match styled(&q) {
            Some(s) => t!("measure-tool-area", value = s),
            // Millimetres as the UI's language writes them, as the medical tools do.
            None if q.unit == Unit::Mm => t!("measure-area-mm", value = format_sig(q.value, 4)),
            None => t!(
                "measure-tool-area",
                value = format!(
                    "{} {}",
                    format_sig(q.value, 4),
                    specular_i18n::unit_symbol(&q.unit_text())
                )
            ),
        },
        None => String::new(),
    }
}

/// `value` with its length unit, millimetres, pixels and ångströms written as the UI's
/// language writes them (Hebrew "מ״מ"), the same messages as the medical and MPR overlays.
pub(crate) fn length_with_unit(value: String, symbol: &str) -> String {
    match symbol {
        "mm" => t!("measure-length-mm", value = value),
        "px" => t!("measure-length-px", value = value),
        "\u{c5}" => t!("measure-length-angstrom", value = value),
        "" => value,
        unit => t!(
            "mpr-length-unit",
            value = value,
            unit = specular_i18n::unit_symbol(unit)
        ),
    }
}

/// Statistics lines of a region: the mean (per channel) and the range.
fn stats_lines(st: &[ChannelStats], channels: &[String]) -> Vec<String> {
    // Integer samples print whole; others to four significant digits.
    let f = |v: f64| {
        if v.fract() == 0.0 && v.abs() < 1e9 {
            specular_i18n::fixed(v, 0)
        } else {
            format_sig(v, 4)
        }
    };
    match st {
        [] => Vec::new(),
        [s] => {
            // Temperatures (a thermal image's values) carry their unit.
            let unit = channels.first().filter(|c| {
                TemperatureUnit::ALL
                    .iter()
                    .any(|u| u.symbol() == c.as_str())
            });
            let f = |v: f64| match unit {
                Some(u) => format!("{} {u}", f(v)),
                None => f(v),
            };
            vec![
                t!("measure-tool-mean", mean = f(s.mean), sd = f(s.sd)),
                t!(
                    "measure-tool-range",
                    min = f(s.min),
                    max = f(s.max),
                    count = s.count as i64
                ),
            ]
        }
        many => {
            let means: Vec<String> = many
                .iter()
                .zip(channels)
                .map(|(s, c)| format!("{} {}", c.chars().next().unwrap_or('?'), f(s.mean)))
                .collect();
            let means = specular_i18n::dot_join(&means);
            vec![
                t!("measure-tool-means", values = means),
                t!("measure-tool-pixels", count = many[0].count as i64),
            ]
        }
    }
}

/// The channel names of a layout, as exports name them.
fn channel_names(layout: PixelLayout) -> Vec<String> {
    let names: &[&str] = match layout {
        PixelLayout::Gray | PixelLayout::Gray16 => &["Gray"],
        PixelLayout::GrayAlpha16 => &["Gray", "Alpha"],
        PixelLayout::YCbCr | PixelLayout::Rgb | PixelLayout::Yuv(_) => &["Red", "Green", "Blue"],
        PixelLayout::Rgba | PixelLayout::Rgba16(_) | PixelLayout::Rgba16F(_) => {
            &["Red", "Green", "Blue", "Alpha"]
        }
    };
    names.iter().map(|s| (*s).to_owned()).collect()
}

/// An IEEE half float.
fn half(b: u16) -> f32 {
    let sign = if b & 0x8000 != 0 { -1.0 } else { 1.0 };
    let e = i32::from((b >> 10) & 0x1F);
    let f = f32::from(b & 0x3FF);
    sign * match e {
        0 => f * (2.0f32).powi(-24),
        31 if f == 0.0 => f32::INFINITY,
        31 => f32::NAN,
        e => (1.0 + f / 1024.0) * (2.0f32).powi(e - 15),
    }
}

/// The statistics of `shape` (stored pixels) over `r` (any scale or region): the pixels of
/// the shape's bounds are copied out as samples and measured. `None` when the shape misses
/// the raster.
pub(super) fn raster_stats(r: &Raster, shape: &Shape) -> Option<(Vec<ChannelStats>, Vec<String>)> {
    let scale = r.scale.max(1);
    let s = if scale > 1 {
        shape.scaled(1.0 / f64::from(scale))
    } else {
        shape.clone()
    };
    let (bx0, by0, bx1, by1) = s.bounds();
    let (ox, oy) = r.origin;
    let x0 = (bx0.floor().max(f64::from(ox)) as u32).min(ox + r.width);
    let y0 = (by0.floor().max(f64::from(oy)) as u32).min(oy + r.height);
    let x1 = (bx1.ceil().max(0.0) as u32).clamp(x0, ox + r.width);
    let y1 = (by1.ceil().max(0.0) as u32).clamp(y0, oy + r.height);
    let (w, h) = (x1 - x0, y1 - y0);
    if w == 0 || h == 0 {
        return None;
    }
    let names = channel_names(r.layout);
    let ch = names.len();
    let n = w as usize * h as usize * ch;
    let full = |x: u32, y: u32| (x * scale, y * scale);
    let st = match r.layout {
        PixelLayout::Rgba16F(_) => {
            let p = r.planes.first()?;
            let mut out = Vec::with_capacity(n);
            for y in y0..y1 {
                let row = (y - oy) as usize * p.stride;
                for x in x0..x1 {
                    let at = row + (x - ox) as usize * 8;
                    let px = p.data.get(at..at + 8)?;
                    for c in 0..4 {
                        out.push(half(u16::from_ne_bytes([px[2 * c], px[2 * c + 1]])));
                    }
                }
            }
            let v = View::new(w, h, 4, Samples::F32(&out)).ok()?.at((x0, y0));
            stats::roi(&s, &v, BINS)?
        }
        l if l.is_wide() => {
            let mut out = Vec::with_capacity(n);
            for y in y0..y1 {
                for x in x0..x1 {
                    let (fx, fy) = full(x, y);
                    match source_pixel(r, fx, fy)? {
                        SourceValue::Gray16(g) => out.push(g),
                        SourceValue::GrayAlpha16(v) => out.extend(v),
                        SourceValue::Rgba16(v) => out.extend(v),
                        _ => return None,
                    }
                }
            }
            let v = View::new(w, h, ch as u32, Samples::U16(&out))
                .ok()?
                .at((x0, y0));
            stats::roi(&s, &v, BINS)?
        }
        _ => {
            let mut out = Vec::with_capacity(n);
            for y in y0..y1 {
                for x in x0..x1 {
                    let (fx, fy) = full(x, y);
                    match source_pixel(r, fx, fy)? {
                        SourceValue::Gray(g) => out.push(g),
                        SourceValue::Rgb(v) | SourceValue::YCbCr { rgb: v, .. } => out.extend(v),
                        SourceValue::Rgba(v) => out.extend(v),
                        _ => return None,
                    }
                }
            }
            let v = View::new(w, h, ch as u32, Samples::U8(&out))
                .ok()?
                .at((x0, y0));
            stats::roi(&s, &v, BINS)?
        }
    };
    Some((st, names))
}

/// Where the statistics job reads pixels from.
struct Pixels {
    /// The full-resolution overview.
    full: Option<Arc<Raster>>,
    /// A session that decodes regions cheaply, and the image size.
    regions: Option<(Arc<dyn Session>, (u32, u32))>,
    /// A reduced overview (approximate statistics).
    reduced: Option<Arc<Raster>>,
    /// The values shown in science mode (a thermal image's temperatures) and their unit:
    /// statistics of areas and lines are of these, not of the displayed pixels.
    values: Option<(Arc<NdPlane>, String)>,
}

/// Statistics of a plane of values over an area, or along a line or polyline (sampled every
/// pixel, the nearest value), with the unit as the channel's name.
pub(super) fn value_stats(
    shape: &Shape,
    plane: &NdPlane,
    unit: &str,
) -> Option<(Vec<ChannelStats>, Vec<String>)> {
    let (w, h) = (plane.width, plane.height);
    let at = |x: u32, y: u32| plane.value(x, y).map(f64::from).filter(|v| v.is_finite());
    if let Shape::Point(p) = shape {
        if p.x < 0.0 || p.y < 0.0 {
            return None;
        }
        let v = at(
            (p.x as u32).min(w.saturating_sub(1)),
            (p.y as u32).min(h.saturating_sub(1)),
        )?;
        let st = stats::of_values(vec![v], BINS)?;
        return Some((vec![st], vec![unit.to_owned()]));
    }
    let st = if shape.is_path() {
        let pts = shape.points();
        let mut vals = Vec::new();
        for seg in pts.windows(2) {
            let (a, b) = (seg[0], seg[1]);
            let n = ((b.x - a.x).hypot(b.y - a.y).ceil() as usize).max(1);
            // Every pixel along the segment; its end is the next segment's start.
            let last = if vals.is_empty() { 0 } else { 1 };
            for i in last..=n {
                let t = i as f64 / n as f64;
                let (x, y) = (a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t);
                if x >= 0.0 && y >= 0.0 && x < f64::from(w) + 1e-9 && y < f64::from(h) + 1e-9 {
                    let (xi, yi) = ((x as u32).min(w - 1), (y as u32).min(h - 1));
                    vals.extend(at(xi, yi));
                }
            }
        }
        stats::of_values(vals, BINS)?
    } else {
        stats::roi_with(shape, w, h, BINS, at)?
    };
    (st.count > 0).then(|| (vec![st], vec![unit.to_owned()]))
}

/// The statistics of `shape`: exact when possible, else approximate; `None` when no pixels.
fn compute(shape: &Shape, px: &Pixels) -> Option<(Vec<ChannelStats>, Vec<String>, bool)> {
    if let Some((plane, unit)) = &px.values {
        return value_stats(shape, plane, unit).map(|(s, n)| (s, n, false));
    }
    if let Some(r) = &px.full {
        return raster_stats(r, shape).map(|(s, n)| (s, n, false));
    }
    if let Some((session, (w, h))) = &px.regions {
        let (bx0, by0, bx1, by1) = shape.bounds();
        let x0 = (bx0.floor().max(0.0) as u32).min(*w);
        let y0 = (by0.floor().max(0.0) as u32).min(*h);
        let x1 = (bx1.ceil().max(0.0) as u32).clamp(x0, *w);
        let y1 = (by1.ceil().max(0.0) as u32).clamp(y0, *h);
        let cancel = CancelToken::new();
        if x1 > x0
            && y1 > y0
            && (session.regions_ready() || session.prepare_regions(&cancel).is_ok())
        {
            let req = DecodeRequest {
                scale: 1,
                region: Some(Region {
                    x: x0,
                    y: y0,
                    w: x1 - x0,
                    h: y1 - y0,
                }),
                parallel: false,
                prepare_regions: false,
            };
            if let Ok(r) = session.decode(&req, &cancel)
                && let Some((s, n)) = raster_stats(&r, shape)
            {
                return Some((s, n, false));
            }
        }
    }
    let r = px.reduced.as_ref()?;
    raster_stats(r, shape).map(|(s, n)| (s, n, true))
}

/// Writes `bytes` to `path` through a temporary file in the same folder.
fn write_bytes(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp = path.with_file_name(format!(".{name}.part"));
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

/// The format of an export, from the chosen file's extension.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Format {
    Csv,
    GeoJson,
    /// One `.roi` file (a single measurement).
    Roi,
    /// An ImageJ ROI set (`.zip`).
    RoiSet,
    /// A DICOM SR measurement report (`.dcm`, TID 1500): needs the measured DICOM file.
    Sr,
}

impl Format {
    pub fn of(path: &Path) -> Format {
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        match ext.as_str() {
            "geojson" | "json" => Format::GeoJson,
            "roi" => Format::Roi,
            "zip" => Format::RoiSet,
            "dcm" => Format::Sr,
            _ => Format::Csv,
        }
    }
}

/// The bytes of an export.
pub(super) fn export_bytes(
    format: Format,
    items: &[Measurement],
    prov: &Provenance,
) -> Result<Vec<u8>, String> {
    match format {
        Format::Csv => Ok(csv::write(items, prov).into_bytes()),
        Format::GeoJson => Ok(geojson::write(items, prov).into_bytes()),
        Format::Roi if items.len() == 1 => {
            imagej::encode(&items[0]).map_err(|e| crate::error_text::reason(&e))
        }
        Format::Roi | Format::RoiSet => {
            imagej::write_zip(items).map_err(|e| crate::error_text::reason(&e))
        }
        Format::Sr => Err(t!("measure-tool-sr-not-dicom")),
    }
}

/// The measurements as a DICOM SR report on the DICOM file `source` (its bytes) measured,
/// dated `unix` seconds (UTC).
pub(super) fn sr_bytes(items: &[Measurement], source: &[u8], unix: u64) -> Result<Vec<u8>, String> {
    let src = specular_dicom::kos::source(source).map_err(|_| t!("measure-tool-sr-not-dicom"))?;
    let [y, mo, d, h, mi, s] = specular_convert::civil_from_unix(unix as i64);
    let date = format!("{y:04}{mo:02}{d:02}");
    let time = format!("{h:02}{mi:02}{s:02}");
    let device = format!("Specular {}", env!("CARGO_PKG_VERSION"));
    specular_dicom::sr_write::measurement_report(
        &src,
        sr::frame(items),
        &sr::groups(items),
        (&date, &time),
        &device,
    )
    .map_err(|e| e.to_string())
}

/// Reads measurements from a GeoJSON, `.roi` or ROI-set file: the measurements and how many
/// shapes were skipped.
pub(super) fn import_bytes(path: &Path, data: &[u8]) -> Result<(Vec<Measurement>, usize), String> {
    match Format::of(path) {
        Format::Roi => imagej::decode(data)
            .map(|m| (vec![m], 0))
            .map_err(|e| crate::error_text::reason(&e)),
        Format::RoiSet => imagej::read_zip(data).map_err(|e| crate::error_text::reason(&e)),
        _ => {
            let text = std::str::from_utf8(data).map_err(|e| crate::error_text::reason(&e))?;
            geojson::read(text)
                .map(|r| (r.items, r.skipped + r.holes))
                .map_err(|e| crate::error_text::reason(&e))
        }
    }
}

// ---------------------------------------------------------------------------------------------
// State.

/// The image a list belongs to: its file and page.
#[derive(Clone, Debug, PartialEq, Eq)]
struct ImageKey {
    path: PathBuf,
    page: u32,
}

/// What a prompt asks for.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Ask {
    /// The known length of the line `a`–`b`.
    Length,
    /// The density at density point `id`.
    Density(u32),
    /// The basic spatial resolution (µm), for SNRn.
    Srb,
}

/// The calibration prompt: the drawn line and the typed length (or density, or SRb).
struct Prompt {
    a: P,
    b: P,
    text: String,
    unit: usize,
    ask: Ask,
}

/// The open tool.
pub(super) struct Tool {
    kind: Kind,
    /// Points placed (stored pixels).
    pts: Vec<P>,
    /// Where the pointer is (stored pixels).
    hover: Option<P>,
    /// The keyboard crosshair (stored pixels), once an arrow moved it.
    cursor: Option<P>,
    /// The left button went down here (logical px) and is still down.
    press: Option<(f32, f32)>,
    selected: Option<u32>,
    focus: Option<usize>,
    prompt: Option<Prompt>,
    status: Option<String>,
    drawn: Drawn,
    /// The geometry template being placed.
    template: Template,
    /// The inspection probe being placed.
    probe: Probe,
    /// A geometry measurement's point being dragged: its id, the point, and the
    /// measurement as it was (one undo step per drag).
    drag: Option<(u32, usize, Box<Measurement>)>,
}

impl Tool {
    fn new(kind: Kind) -> Tool {
        Tool {
            kind,
            pts: Vec::new(),
            hover: None,
            cursor: None,
            press: None,
            selected: None,
            focus: None,
            prompt: None,
            status: None,
            drawn: Drawn::default(),
            template: Template::Vhs,
            probe: Probe::Profile,
            drag: None,
        }
    }

    /// Points that complete the shape being placed (`None`: closed with Enter).
    fn need(&self) -> Option<usize> {
        match self.kind {
            Kind::Geometry => Some(self.template.landmarks().len()),
            Kind::Inspect => self.probe.points(),
            k => k.points(),
        }
    }

    /// Whether a press-drag-release draws the shape.
    fn drags(&self) -> bool {
        self.need() == Some(2)
    }

    /// Whether Enter finishes the shape (a polygon, or a crack line of two or more points).
    fn open_ended(&self) -> bool {
        (self.kind == Kind::Polygon && self.pts.len() >= 3)
            || (self.kind == Kind::Inspect && self.probe == Probe::Crack && self.pts.len() >= 2)
    }

    /// The shape the points make.
    fn shape(&self, pts: &[P]) -> Option<Shape> {
        match self.kind {
            Kind::Inspect => probe::shape(self.probe, pts),
            k => shape_of(k, pts),
        }
    }
}

/// Results of background work.
enum Done {
    Stats {
        key: ImageKey,
        id: u32,
        /// Statistics, channel names, approximate.
        result: Option<(Vec<ChannelStats>, Vec<String>, bool)>,
    },
    Saved(Result<PathBuf, String>),
    Imported(ImageKey, Result<(Vec<Measurement>, usize), String>),
    /// An inspection probe's reading.
    Probe {
        key: ImageKey,
        id: u32,
        found: Option<probe::Found>,
    },
    /// The image's sidecar was read.
    Side(ImageKey, Option<probe::Sidecar>),
    /// Writing the sidecar failed.
    SideFailed(String),
}

/// The measurement list of the current image and the tool.
pub(super) struct MeasureState {
    pub tool: Option<Tool>,
    list: List,
    key: Option<ImageKey>,
    /// The image's calibration (from the file, or the user's).
    cal: Calibration,
    /// A DICOM image's ultrasound regions: each measurement takes the spacing of the region
    /// holding all its points, until the user calibrates.
    regions: Option<specular_dicom::Calibration>,
    /// The page's drawing scale by region (PDF viewports, CAD units and layout viewports);
    /// a measurement goes through the region under it until the user calibrates.
    scale: Option<Arc<DrawingScale>>,
    /// The region of `scale` each measurement went through.
    region_of: HashMap<u32, usize>,
    /// Statistics being computed.
    pending: HashSet<u32>,
    /// Regions whose pixels could not be read.
    missing: HashSet<u32>,
    /// The geometry template last picked (kept while the app runs).
    template: Template,
    /// The inspection probe last picked.
    probe: Probe,
    /// What the user set on this file (scale, densities, SRb), kept in its sidecar.
    side: probe::Sidecar,
    tx: Sender<Done>,
    rx: Receiver<Done>,
}

impl Default for MeasureState {
    fn default() -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        MeasureState {
            tool: None,
            list: List::new(),
            key: None,
            cal: Calibration::uncalibrated(),
            regions: None,
            scale: None,
            region_of: HashMap::new(),
            pending: HashSet::new(),
            missing: HashSet::new(),
            template: Template::Vhs,
            probe: Probe::Profile,
            side: probe::Sidecar::default(),
            tx,
            rx,
        }
    }
}

impl std::fmt::Debug for MeasureState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MeasureState")
            .field("open", &self.tool.is_some())
            .field("items", &self.list.len())
            .finish_non_exhaustive()
    }
}

/// The keyboard crosshair and reading the value under it work with the tool open: the
/// crosshair places its points without a mouse.
fn keyboard_inspection(a: Action) -> bool {
    matches!(a, Action::ToggleCrosshair | Action::AnnounceCursor)
}

impl MeasureState {
    /// The tool is asking for a calibration length (it takes the keys).
    pub(super) fn prompting(&self) -> bool {
        self.tool.as_ref().is_some_and(|t| t.prompt.is_some())
    }

    /// Stores where the controls were drawn.
    pub(super) fn set_drawn(&mut self, d: Drawn) {
        if let Some(t) = self.tool.as_mut() {
            t.drawn = d;
        }
    }

    pub(super) fn a11y(&self, tree: &mut accesskit::TreeUpdate, v: &MeasureView, scale: f64) {
        if let Some(t) = &self.tool {
            a11y(tree, v, &t.drawn, scale);
        }
    }
}

impl App {
    fn measure_image_key(&self) -> Option<ImageKey> {
        let path = self.current.and_then(|i| self.entries.get(i))?.clone();
        let page = self
            .current_id()
            .and_then(|id| self.images.get(&id))
            .map_or(0, |i| i.page);
        Some(ImageKey { path, page })
    }

    /// The drawing-scale region measurement `id` went through.
    fn measure_region(&self, id: u32) -> Option<&ScaleRegion> {
        let i = *self.measure.region_of.get(&id)?;
        self.measure.scale.as_ref()?.regions.get(i)
    }

    /// The calibration the file states (DICOM spacings), else pixels.
    pub(super) fn measure_file_calibration(&self) -> Calibration {
        let session = self
            .current_id()
            .and_then(|id| self.images.get(&id))
            .and_then(|i| i.session.as_ref());
        // A map: its ground scale at the centre (the map view measures on the ellipsoid).
        if let Some(c) = session
            .and_then(|s| s.geo())
            .and_then(|g| map_calibration(&g, session?.info()))
        {
            return c;
        }
        if let Some(c) = session
            .and_then(|s| s.drawing())
            .and_then(drawing_calibration)
        {
            return c;
        }
        let summary = session.and_then(|s| s.medical().cloned());
        let Some(m) = summary else {
            return session
                .and_then(|s| science_calibration(s.science()?))
                .unwrap_or_else(Calibration::uncalibrated);
        };
        // The file's own reading: calibrated, detector or magnification-corrected spacing.
        if let Some(c) = m.calibration.spacing.and_then(|s| s.calibration()) {
            return c;
        }
        // Ultrasound: each measurement in its region (`measure_cal_at`), pixels elsewhere.
        if !m.calibration.regions.is_empty() {
            return Calibration::uncalibrated();
        }
        let from =
            |p: Option<(f64, f64)>, src| p.and_then(|(r, c)| Calibration::dicom(r, c, src).ok());
        from(m.pixel_spacing, Source::DicomPixelSpacing)
            .or_else(|| from(m.imager_pixel_spacing, Source::DicomImagerPixelSpacing))
            .unwrap_or_else(Calibration::uncalibrated)
    }

    /// The calibration of a measurement through `pts` (stored pixels): the image's, or on
    /// an ultrasound image the region holding every point (pixels outside one), until the
    /// user calibrates.
    fn measure_cal_at(&self, pts: &[P]) -> Calibration {
        match &self.measure.regions {
            Some(r) if self.measure.cal.source != Source::User => {
                r.measure(pts).unwrap_or(self.measure.cal)
            }
            _ => self.measure.cal,
        }
    }

    /// The image's calibration (the user's when they calibrated it, else the file's), its
    /// measurements in stored pixels and its last rectangle (x0, y0, x1, y1; an inset's
    /// region), for figures and key images.
    pub(super) fn measure_for_figure(&self) -> super::figure::ForFigure {
        if self.measure.key.is_none() || self.measure.key != self.measure_image_key() {
            return (self.measure_file_calibration(), Vec::new(), None);
        }
        let mut rect = None;
        let mut items = Vec::new();
        for m in self.measure.list.items() {
            if m.template.is_some() {
                items.extend(geometry::figure(m));
                continue;
            }
            items.push({
                if let Shape::Rect { x0, y0, x1, y1 } = m.shape {
                    rect = Some((x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1)));
                }
                let outline: Vec<P> = match &m.shape {
                    Shape::Angle { a, vertex, b } => vec![*a, *vertex, *b],
                    s => s.outline(),
                };
                let value = value_text(m, self.measure_region(m.id));
                let title = t!(
                    "measure-tool-item",
                    n = i64::from(m.id),
                    shape = shape_name(&m.shape)
                );
                let mut lines = vec![if value.is_empty() {
                    title
                } else {
                    format!("{title}: {value}")
                }];
                if let Some(st) = &m.stats {
                    lines.extend(stats_lines(st, &m.channels));
                }
                super::figure::Measured {
                    outline: outline.iter().map(|p| (p.x, p.y)).collect(),
                    closed: m.shape.is_area(),
                    label: value,
                    lines,
                }
            });
        }
        (self.measure.cal, items, rect)
    }

    /// Where the calibration comes from, as the badges say it.
    pub(super) fn measure_source_label(s: Source) -> String {
        source_label(s)
    }

    /// Starts a new list when the image changed.
    fn measure_sync(&mut self) {
        let key = self.measure_image_key();
        if key == self.measure.key {
            return;
        }
        self.measure.key = key;
        self.measure.list = List::new();
        self.measure.pending.clear();
        self.measure.missing.clear();
        self.measure.cal = self.measure_file_calibration();
        self.measure.regions = self
            .current_id()
            .and_then(|id| self.images.get(&id))
            .and_then(|i| i.session.as_ref())
            .and_then(|s| s.medical())
            .map(|m| m.calibration.clone())
            .filter(|c| !c.regions.is_empty());
        self.measure.scale = self
            .current_id()
            .and_then(|id| self.images.get(&id))
            .and_then(|i| i.session.as_ref())
            .and_then(|s| s.drawing_scale())
            .filter(|d| !d.regions.is_empty());
        self.measure.region_of.clear();
        self.measure.side = probe::Sidecar::default();
        if let Some(t) = self.measure.tool.as_mut() {
            t.pts.clear();
            t.prompt = None;
            t.selected = None;
            t.focus = None;
            t.cursor = None;
        }
        self.measure_read_side();
    }

    /// Opens the tool over the image.
    pub(super) fn measure_open(&mut self) {
        if self.measure.tool.is_some() || self.shown.is_none() {
            return;
        }
        self.measure_sync();
        let mut t = Tool::new(Kind::Line);
        t.template = self.measure.template;
        self.measure.tool = Some(t);
        self.chrome.announce_status(Kind::Line.hint());
        self.redraw = true;
    }

    /// The geometry templates are offered: with the medical tools on (tier 2, DESIGN.md
    /// §11), on any image.
    fn measure_geometry_offered(&self) -> bool {
        self.profile().is_on(specular_input::Capability::Medical)
    }

    /// Picks the next (or previous) geometry template; the points placed so far go.
    fn measure_step_template(&mut self, forward: bool) {
        let Some(t) = self.measure.tool.as_mut() else {
            return;
        };
        t.template = if forward {
            t.template.next()
        } else {
            t.template.prev()
        };
        t.kind = Kind::Geometry;
        t.pts.clear();
        t.status = None;
        self.measure.template = t.template;
        let msg = geometry::prompt(t.template, 0);
        self.chrome.announce_status(msg);
        self.redraw = true;
    }

    /// The geometry measurement's point under `lp` (logical px): (id, point).
    fn measure_handle_at(&self, lp: (f32, f32)) -> Option<(u32, usize)> {
        let handles: Vec<(u32, Vec<(f32, f32)>)> = self
            .measure
            .list
            .items()
            .iter()
            .filter(|m| m.template.is_some())
            .map(|m| {
                let pts = m.shape.points();
                (m.id, pts.iter().map(|p| self.measure_screen(*p)).collect())
            })
            .collect();
        geometry::handle_at(&handles, lp)
    }

    /// A dragged point was let go: the move becomes one undo step and the new headline is
    /// read out.
    fn measure_drop(&mut self, id: u32, before: Measurement) {
        let Some(now) = self.measure.list.get(id).cloned() else {
            return;
        };
        if now == before {
            return;
        }
        if let Some(m) = self.measure.list.get_mut(id) {
            *m = before;
        }
        self.measure.list.replace(now);
        let (head, _) =
            geometry::lines(self.measure.list.get(id).and_then(|m| m.outcome()).as_ref());
        let msg = t!("geometry-moved", value = head);
        if let Some(t) = self.measure.tool.as_mut() {
            t.status = Some(msg.clone());
        }
        self.chrome.announce_status(msg);
        self.redraw = true;
    }

    fn measure_close(&mut self) {
        self.measure.tool = None;
        let n = self.measure.list.len();
        self.chrome
            .announce_status(t!("measure-tool-count", count = n as i64));
        self.redraw = true;
    }

    fn measure_set_kind(&mut self, k: Kind) {
        if let Some(t) = self.measure.tool.as_mut() {
            t.kind = k;
            t.pts.clear();
            t.prompt = None;
            t.status = None;
        }
        let msg = match self.measure.tool.as_ref() {
            Some(t) if k == Kind::Geometry => geometry::prompt(t.template, 0),
            _ => k.hint(),
        };
        self.chrome.announce_status(msg);
        self.redraw = true;
    }

    /// Stored pixels from a window point (physical px), clamped to the image.
    fn measure_stored(&self, p: (f32, f32)) -> P {
        let (x, y) = self.view.stored_from_screen().apply(p);
        let (w, h) = self.view.stored_size();
        P::new(
            f64::from(x.clamp(0.0, w as f32)),
            f64::from(y.clamp(0.0, h as f32)),
        )
    }

    /// A stored point on screen (logical px).
    fn measure_screen(&self, p: P) -> (f32, f32) {
        let a = self.view.stored_from_screen().invert();
        let s = self.dl.scale.max(0.1);
        let q = a.apply((p.x as f32, p.y as f32));
        (q.0 / s, q.1 / s)
    }

    /// What the tool draws: with the tool open, or while the image has measurements.
    pub(super) fn measure_view(&self) -> Option<MeasureView> {
        let open = self.measure.tool.is_some();
        if self.measure.key.is_none() || self.measure.key != self.measure_image_key() {
            return None;
        }
        if !open && self.measure.list.is_empty() {
            return None;
        }
        let list = &self.measure.list;
        let items = list
            .items()
            .iter()
            .map(|m| {
                if m.template.is_some() {
                    return geometry::item(m, |p| self.measure_screen(p));
                }
                if m.probe.is_some() {
                    let pending = self.measure.pending.contains(&m.id);
                    return probe::item(m, pending, badge(m.cal.source, None), |p| {
                        self.measure_screen(p)
                    });
                }
                let detail = match &m.stats {
                    Some(st) => {
                        let mut d = stats_lines(st, &m.channels);
                        if m.approximate {
                            d.push(t!("measure-tool-approximate"));
                        }
                        d
                    }
                    None if !m.shape.is_area() => Vec::new(),
                    None if self.measure.pending.contains(&m.id) => {
                        vec![t!("measure-tool-stats-pending")]
                    }
                    None => vec![t!("measure-tool-stats-none")],
                };
                let pts: Vec<P> = match &m.shape {
                    Shape::Angle { a, vertex, b } => vec![*a, *vertex, *b],
                    s => s.outline(),
                };
                let region = self.measure_region(m.id);
                ItemView {
                    title: t!(
                        "measure-tool-item",
                        n = i64::from(m.id),
                        shape = shape_name(&m.shape)
                    ),
                    value: value_text(m, region),
                    detail,
                    badge: badge(m.cal.source, region),
                    caveat: m.cal.source.is_approximate() || m.approximate,
                    outline: pts.iter().map(|p| self.measure_screen(*p)).collect(),
                    closed: m.shape.is_area(),
                    handles: Vec::new(),
                    guides: Vec::new(),
                    plot: Vec::new(),
                }
            })
            .collect();
        let t = self.measure.tool.as_ref();
        let selected = t.and_then(|t| t.selected).and_then(|id| list.index_of(id));
        let (mut drawing, mut drawing_closed, mut marks, mut cursor) =
            (Vec::new(), false, Vec::new(), None);
        let mut kind = Kind::Line;
        let mut status = String::new();
        let mut prompt = None;
        if let Some(t) = t {
            kind = t.kind;
            let tip = t.cursor.or(t.hover);
            marks = t.pts.iter().map(|p| self.measure_screen(*p)).collect();
            cursor = t.cursor.map(|p| self.measure_screen(p));
            if let Some(p) = t.prompt.as_ref().filter(|p| p.ask != Ask::Length) {
                status = probe::prompt_text(p);
            } else if let Some(p) = &t.prompt {
                drawing = vec![self.measure_screen(p.a), self.measure_screen(p.b)];
                let unit = specular_i18n::unit_symbol(UNITS[p.unit].symbol());
                status = t!(
                    "measure-tool-known-length",
                    value = if p.text.is_empty() {
                        "…".to_owned()
                    } else {
                        p.text.clone()
                    },
                    unit = unit.clone()
                );
                prompt = Some(unit);
            } else if t.kind == Kind::Geometry {
                // Points only: the template's lines show once it is complete.
            } else if !t.pts.is_empty() {
                let mut pts = t.pts.clone();
                if let Some(h) = tip
                    && t.kind.points().is_none_or(|n| pts.len() < n)
                {
                    pts.push(h);
                }
                match t.shape(&pts) {
                    Some(s @ (Shape::Rect { .. } | Shape::Ellipse { .. })) => {
                        drawing = s
                            .outline()
                            .iter()
                            .map(|p| self.measure_screen(*p))
                            .collect();
                        drawing_closed = true;
                    }
                    _ => drawing = pts.iter().map(|p| self.measure_screen(*p)).collect(),
                }
            }
            if prompt.is_none() {
                status = t.status.clone().unwrap_or_else(|| match t.kind {
                    Kind::Geometry => geometry::prompt(t.template, t.pts.len()),
                    Kind::Inspect => probe::hint(t.probe),
                    k => k.hint(),
                });
            }
        }
        let template = t.map_or(self.measure.template, |t| t.template);
        let probe = t.map_or(self.measure.probe, |t| t.probe);
        Some(MeasureView {
            open,
            kind,
            items,
            selected,
            drawing,
            drawing_closed,
            marks,
            cursor,
            status,
            prompt,
            can_undo: list.can_undo(),
            focus: t.and_then(|t| t.focus),
            spot: self.science_measure_values().is_some(),
            geometry: self.measure_geometry_offered(),
            template,
            probe,
        })
    }

    /// Where the statistics job reads pixels from.
    fn measure_pixels(&self) -> Pixels {
        let img = self.current_id().and_then(|id| self.images.get(&id));
        let mut px = Pixels {
            full: None,
            regions: None,
            reduced: None,
            values: self.science_measure_values(),
        };
        let Some(img) = img else { return px };
        match &img.overview {
            Some((OverviewKind::Level(1), r)) => px.full = Some(r.clone()),
            Some((OverviewKind::Level(_), r)) => px.reduced = Some(r.clone()),
            _ => {}
        }
        if px.full.is_none()
            && let (Some(s), Some(info)) = (&img.session, &img.info)
            && info.fast_regions
        {
            px.regions = Some((s.clone(), (info.width, info.height)));
        }
        px
    }

    /// Computes the statistics of region `id` on a background thread.
    fn measure_stats(&mut self, id: u32) {
        let (Some(key), Some(m)) = (self.measure.key.clone(), self.measure.list.get(id)) else {
            return;
        };
        if m.probe.is_some() {
            self.measure_probe_read(id);
            return;
        }
        let px = self.measure_pixels();
        let point = matches!(m.shape, Shape::Point(_));
        if !m.shape.is_area() && !((m.shape.is_path() || point) && px.values.is_some()) {
            return;
        }
        let shape = m.shape.clone();
        if px.full.is_none() && px.regions.is_none() && px.reduced.is_none() && px.values.is_none()
        {
            self.measure.missing.insert(id);
            return;
        }
        self.measure.pending.insert(id);
        let tx = self.measure.tx.clone();
        self.loader.background("measure-stats", move |l| {
            let t0 = std::time::Instant::now();
            let result = compute(&shape, &px);
            debug!(target: UI, "measure: statistics of #{id} in {:.1} ms", t0.elapsed().as_secs_f64() * 1e3);
            let _ = tx.send(Done::Stats { key, id, result });
            l.waker()();
        });
    }

    /// Applies finished background work; redoes approximate statistics once the
    /// full-resolution overview is there.
    pub(super) fn measure_tick(&mut self) {
        if self.measure.tool.is_some() {
            self.measure_sync();
        }
        while let Ok(d) = self.measure.rx.try_recv() {
            self.redraw = true;
            match d {
                Done::Stats { key, id, result } => {
                    if Some(&key) != self.measure.key.as_ref() {
                        continue;
                    }
                    self.measure.pending.remove(&id);
                    let Some(m) = self.measure.list.get_mut(id) else {
                        continue;
                    };
                    match result {
                        Some((st, names, approx)) => {
                            m.stats = Some(st);
                            m.channels = names;
                            m.approximate = approx;
                        }
                        None => {
                            m.stats = None;
                            self.measure.missing.insert(id);
                        }
                    }
                }
                Done::Probe { key, id, found } => {
                    if Some(&key) == self.measure.key.as_ref() {
                        self.measure_probe_done(id, found);
                    }
                }
                Done::Side(key, side) => {
                    if Some(&key) == self.measure.key.as_ref() {
                        self.measure_side_read(side);
                    }
                }
                Done::SideFailed(e) => {
                    let now = self.now();
                    self.chrome
                        .toast(t!("ndt-sidecar-failed", reason = e), None, now);
                }
                Done::Saved(r) => {
                    if let Ok(p) = &r {
                        let n = serde_json::json!(self.measure.list.len());
                        self.audit(
                            super::audit::Event::Export,
                            "measurements",
                            &[p],
                            &[("count", n)],
                        );
                    }
                    let msg = match r {
                        Ok(p) => t!(
                            "measure-tool-exported",
                            name = p
                                .file_name()
                                .map(|n| n.to_string_lossy().into_owned())
                                .unwrap_or_default()
                        ),
                        Err(e) => t!("measure-tool-export-failed", reason = e),
                    };
                    let now = self.now();
                    self.chrome.toast(msg, None, now);
                }
                Done::Imported(key, r) => {
                    let now = self.now();
                    if Some(&key) != self.measure.key.as_ref() {
                        continue;
                    }
                    match r {
                        Ok((items, skipped)) => {
                            let cal = self.measure.cal;
                            let items: Vec<_> = items
                                .into_iter()
                                .map(|mut m| {
                                    // ImageJ files carry no calibration: the image's applies.
                                    if !m.cal.is_calibrated() {
                                        m.cal = cal;
                                    }
                                    m.stats = None;
                                    m
                                })
                                .collect();
                            let n = items.len();
                            let ids = self.measure.list.extend(items);
                            for id in ids {
                                self.measure_stats(id);
                            }
                            let mut msg = t!("measure-tool-imported", count = n as i64);
                            if skipped > 0 {
                                msg = format!(
                                    "{msg} {}",
                                    t!("measure-tool-skipped", count = skipped as i64)
                                );
                            }
                            self.chrome.toast(msg, None, now);
                        }
                        Err(e) => {
                            self.chrome.toast(
                                t!("measure-tool-import-failed", reason = e),
                                None,
                                now,
                            );
                        }
                    }
                }
            }
        }
        // Approximate statistics, or regions that had no pixels: redo them from the full
        // overview when it has arrived.
        if self.measure.key.is_some()
            && self.measure.key == self.measure_image_key()
            && self.measure_pixels().full.is_some()
        {
            let redo: Vec<u32> = self
                .measure
                .list
                .items()
                .iter()
                .filter(|m| {
                    (m.shape.is_area() || m.probe.is_some())
                        && !self.measure.pending.contains(&m.id)
                        && (m.approximate
                            || (m.stats.is_none() && self.measure.missing.contains(&m.id)))
                })
                .map(|m| m.id)
                .collect();
            for id in redo {
                self.measure.missing.remove(&id);
                self.measure_stats(id);
            }
        }
    }

    /// The values measured changed (a thermal image's temperatures recomputed, in another
    /// unit): the statistics of every area and line again.
    pub(super) fn measure_restat(&mut self) {
        if self.measure.key.is_none() || self.measure.key != self.measure_image_key() {
            return;
        }
        let ids: Vec<u32> = self
            .measure
            .list
            .items()
            .iter()
            .filter(|m| {
                m.probe.is_some()
                    || m.shape.is_area()
                    || m.shape.is_path()
                    || matches!(m.shape, Shape::Point(_))
            })
            .map(|m| m.id)
            .collect();
        for id in ids {
            self.measure.missing.remove(&id);
            self.measure_stats(id);
        }
    }

    /// Adds the shape the tool's points make.
    fn measure_finish(&mut self) {
        let Some(t) = self.measure.tool.as_mut() else {
            return;
        };
        let pts = std::mem::take(&mut t.pts);
        let kind = t.kind;
        if kind == Kind::Geometry {
            let template = t.template;
            if pts.len() != template.landmarks().len() {
                return;
            }
            let cal = self.measure_cal_at(&pts);
            let m = Measurement::clinical(template, pts, cal);
            let o = m.outcome();
            let id = self.measure.list.push(m);
            let (head, _) = geometry::lines(o.as_ref());
            if let Some(t) = self.measure.tool.as_mut() {
                t.selected = Some(id);
                t.status = Some(head.clone());
            }
            self.chrome
                .announce_status(format!("{}: {head}", geometry::name(template)));
            return;
        }
        let Some(shape) = t.shape(&pts) else {
            return;
        };
        if kind == Kind::Inspect {
            let p = t.probe;
            self.measure_probe_add(p, shape);
            return;
        }
        if kind == Kind::Calibrate {
            t.prompt = Some(Prompt {
                a: pts[0],
                b: pts[1],
                text: String::new(),
                unit: 0,
                ask: Ask::Length,
            });
            t.focus = None;
            self.chrome.announce_status(t!(
                "measure-tool-known-length",
                value = "…".to_owned(),
                unit = specular_i18n::unit_symbol(UNITS[0].symbol())
            ));
            return;
        }
        // The drawing's scale under the shape, until the user calibrates.
        let region = self
            .measure
            .scale
            .as_ref()
            .filter(|_| self.measure.cal.source != Source::User)
            .and_then(|d| {
                let (x, y) = anchor(&pts);
                d.region_at(x, y).map(|i| (i, d.regions[i].cal))
            });
        let cal = region.map_or_else(|| self.measure_cal_at(&shape.points()), |r| r.1);
        let mut m = Measurement::new(shape, cal);
        m.label = String::new();
        let id = self.measure.list.push(m);
        if let Some((i, _)) = region {
            self.measure.region_of.insert(id, i);
        }
        if let Some(t) = self.measure.tool.as_mut() {
            t.selected = Some(id);
            t.status = None;
        }
        self.measure_stats(id);
        if let Some(m) = self.measure.list.get(id) {
            let region = self.measure_region(m.id);
            let text = format!("{} {}", shape_name(&m.shape), value_text(m, region));
            self.chrome.announce_status(text);
        }
    }

    /// Places a point of the shape being drawn.
    fn measure_place(&mut self, p: P) {
        let Some(t) = self.measure.tool.as_mut() else {
            return;
        };
        if t.prompt.is_some() {
            return;
        }
        t.pts.push(p);
        if t.need() == Some(t.pts.len()) {
            self.measure_finish();
        } else if t.kind == Kind::Geometry {
            t.status = None;
            let msg = geometry::prompt(t.template, t.pts.len());
            self.chrome.announce_status(msg);
        }
        self.redraw = true;
    }

    /// Applies the typed known length.
    fn measure_apply_calibration(&mut self) {
        let Some(t) = self.measure.tool.as_mut() else {
            return;
        };
        let Some(p) = t.prompt.as_ref() else { return };
        if p.ask != Ask::Length {
            self.measure_probe_answer();
            return;
        }
        let len: f64 = p.text.replace(',', ".").trim().parse().unwrap_or(f64::NAN);
        let base = if self.measure.cal.source == Source::Uncalibrated {
            Calibration::uncalibrated()
        } else {
            self.measure.cal
        };
        match base.rescaled(p.a, p.b, len, UNITS[p.unit]) {
            Ok(c) => {
                let c = c.readable();
                t.prompt = None;
                self.measure.cal = c;
                self.measure.region_of.clear();
                let ids: Vec<u32> = self.measure.list.items().iter().map(|m| m.id).collect();
                for id in ids {
                    if let Some(m) = self.measure.list.get_mut(id) {
                        m.cal = c;
                    }
                }
                self.measure.side.scale = Some(c);
                self.measure_write_side();
                let msg = t!(
                    "measure-tool-calibrated",
                    size = format_sig(c.pixel_size(), 4),
                    unit = specular_i18n::unit_symbol(c.unit.symbol())
                );
                if let Some(t) = self.measure.tool.as_mut() {
                    t.status = Some(msg.clone());
                }
                self.chrome.announce_status(msg);
            }
            Err(_) => {
                let msg = t!("measure-tool-bad-length");
                t.status = Some(msg.clone());
                self.chrome.announce_status(msg);
            }
        }
        self.redraw = true;
    }

    fn measure_hit(&mut self, h: Hit) {
        match h {
            Hit::Tool(k) => self.measure_set_kind(k),
            Hit::Undo => {
                if self.measure.list.undo() {
                    if let Some(t) = self.measure.tool.as_mut() {
                        t.selected = None;
                        t.focus = None;
                    }
                    self.chrome.announce_status(t!("measure-tool-undone"));
                }
            }
            Hit::Delete => {
                let sel = self.measure.tool.as_mut().and_then(|t| t.selected.take());
                if let Some(id) = sel
                    && self.measure.list.remove(id).is_some()
                {
                    if let Some(t) = self.measure.tool.as_mut() {
                        t.focus = None;
                    }
                    self.chrome.announce_status(t!("measure-tool-deleted"));
                }
            }
            Hit::Copy => self.measure_copy_csv(),
            Hit::Export => self.measure_export(),
            Hit::Import => self.measure_import(),
            Hit::Unit => {
                if let Some(p) = self.measure.tool.as_mut().and_then(|t| t.prompt.as_mut()) {
                    p.unit = (p.unit + 1) % UNITS.len();
                }
            }
            Hit::Apply => self.measure_apply_calibration(),
            Hit::Template => self.measure_step_template(true),
            Hit::Probe => self.measure_step_probe(true),
            Hit::Done => self.measure_close(),
            Hit::Item(i) => {
                let id = self.measure.list.items().get(i).map(|m| m.id);
                if let Some(t) = self.measure.tool.as_mut() {
                    t.selected = id;
                }
                if let Some(v) = self.measure_view()
                    && let Some(it) = v.items.get(i)
                {
                    self.chrome.announce_status(item_text(it));
                }
            }
        }
        self.redraw = true;
    }

    /// Moves the keyboard crosshair by (`dx`, `dy`) screen directions, `step` stored pixels.
    fn measure_nudge(&mut self, dx: f32, dy: f32, step: f32) {
        let a = self.view.stored_from_screen();
        let start = self
            .measure
            .tool
            .as_ref()
            .and_then(|t| t.cursor.or(t.hover))
            .unwrap_or_else(|| {
                let (w, h) = self.view.stored_size();
                P::new(f64::from(w) / 2.0, f64::from(h) / 2.0)
            });
        // The screen direction in stored pixels (exact under 90° rotations and flips).
        let s = a.invert().apply((start.x as f32, start.y as f32));
        let o = a.apply(s);
        let e = a.apply((s.0 + dx, s.1 + dy));
        let (mut ex, mut ey) = (e.0 - o.0, e.1 - o.1);
        let k = ex.abs().max(ey.abs());
        if k > 0.0 {
            ex = (ex / k * step).round();
            ey = (ey / k * step).round();
        }
        let (w, h) = self.view.stored_size();
        let p = P::new(
            (start.x + f64::from(ex)).clamp(0.0, f64::from(w)),
            (start.y + f64::from(ey)).clamp(0.0, f64::from(h)),
        );
        let held = self.measure.tool.as_mut().and_then(|t| {
            t.cursor = Some(p);
            t.focus = None;
            t.drag.as_ref().map(|(id, i, _)| (*id, *i))
        });
        // A geometry point picked up with Space goes with the crosshair.
        if let Some((id, i)) = held
            && let Some(m) = self.measure.list.get_mut(id)
        {
            geometry::set_point(m, i, p);
        }
        self.redraw = true;
    }

    /// `[` or `]`: the crosshair goes to the previous or next point of the selected geometry
    /// measurement (else the last one in the list), and its name is read out.
    fn measure_step_point(&mut self, forward: bool) {
        let Some(t) = self.measure.tool.as_ref() else {
            return;
        };
        if !t.pts.is_empty() || t.drag.is_some() {
            return;
        }
        let list = self.measure.list.items();
        let m = t
            .selected
            .and_then(|id| list.iter().find(|m| m.id == id && m.template.is_some()))
            .or_else(|| list.iter().rev().find(|m| m.template.is_some()));
        let Some(m) = m else { return };
        let (id, template) = (m.id, m.template);
        let pts = m.shape.points();
        let screen: Vec<(f32, f32)> = pts.iter().map(|p| self.measure_screen(*p)).collect();
        let cur = t
            .cursor
            .map(|c| self.measure_screen(c))
            .and_then(|c| geometry::handle_at(&[(id, screen)], c))
            .map(|(_, i)| i);
        let Some(k) = geometry::step_point(pts.len(), cur, forward) else {
            return;
        };
        if let Some(t) = self.measure.tool.as_mut() {
            t.selected = Some(id);
            t.cursor = Some(pts[k]);
            t.focus = None;
        }
        if let Some(template) = template {
            self.chrome.announce_status(geometry::prompt(template, k));
        }
        self.redraw = true;
    }

    /// Space on a geometry point under the crosshair picks it up; on a held one, puts it
    /// down (one undo step). False when there is no point there.
    fn measure_key_pick(&mut self) -> bool {
        let Some(t) = self.measure.tool.as_mut() else {
            return false;
        };
        if let Some((id, _, before)) = t.drag.take() {
            self.measure_drop(id, *before);
            return true;
        }
        if !t.pts.is_empty() {
            return false;
        }
        let Some(c) = t.cursor else { return false };
        let lp = self.measure_screen(c);
        let Some((id, i)) = self.measure_handle_at(lp) else {
            return false;
        };
        let Some(m) = self.measure.list.get(id).cloned() else {
            return false;
        };
        let at = m.shape.points().get(i).copied();
        let name = m
            .template
            .and_then(|tp| tp.landmarks().get(i).map(geometry::landmark))
            .unwrap_or_default();
        if let Some(t) = self.measure.tool.as_mut() {
            t.drag = Some((id, i, Box::new(m)));
            t.selected = Some(id);
            t.cursor = at.or(t.cursor);
            t.focus = None;
        }
        self.chrome
            .announce_status(t!("geometry-picked", point = name));
        self.redraw = true;
        true
    }

    /// Esc while a point is held: it goes back where it was.
    fn measure_put_back(&mut self) {
        let Some((id, i, before)) = self.measure.tool.as_mut().and_then(|t| t.drag.take()) else {
            return;
        };
        let at = before.shape.points().get(i).copied();
        if let Some(m) = self.measure.list.get_mut(id) {
            *m = *before;
        }
        if let Some(t) = self.measure.tool.as_mut() {
            t.cursor = at.or(t.cursor);
        }
        self.chrome.announce_status(t!("geometry-put-back"));
        self.redraw = true;
    }

    /// Keys while the tool is open. It takes every key; zoom and pan keys still work.
    pub(super) fn measure_key(&mut self, combo: Option<&KeyCombo>, el: &ActiveEventLoop) -> bool {
        self.measure_sync();
        let Some(v) = self.measure_view() else {
            return false;
        };
        let Some(combo) = combo else { return true };
        let focusables = v.focusables();
        let n = focusables.len();
        let focus = v.focus.and_then(|i| focusables.get(i).copied());
        let mods = combo.mods;
        let plain = !mods.ctrl && !mods.alt && !mods.meta;
        // The calibration prompt takes the length's characters.
        let prompting = self
            .measure
            .tool
            .as_ref()
            .is_some_and(|t| t.prompt.is_some());
        if prompting && plain {
            let mut handled = true;
            if let Some(p) = self.measure.tool.as_mut().and_then(|t| t.prompt.as_mut()) {
                match combo.key {
                    Key::Char(c) if c.is_ascii_digit() || c == '.' || c == ',' => {
                        if p.text.len() < 24 {
                            p.text.push(c);
                        }
                    }
                    Key::Named(NamedKey::Backspace) => {
                        p.text.pop();
                    }
                    Key::Char('u') => p.unit = (p.unit + 1) % UNITS.len(),
                    _ => handled = false,
                }
            }
            if handled {
                self.redraw = true;
                return true;
            }
            if combo.key == Key::Named(NamedKey::Enter) && focus.is_none() {
                self.measure_apply_calibration();
                return true;
            }
        }
        let move_focus = |me: &mut App, d: i32| {
            if let Some(t) = me.measure.tool.as_mut() {
                let i = t.focus.map_or(if d > 0 { 0 } else { n - 1 }, |i| {
                    (i as i32 + d).rem_euclid(n as i32) as usize
                });
                t.focus = Some(i);
            }
            if let Some(v) = me.measure_view()
                && let Some(h) = v.focus.and_then(|i| v.focusables().get(i).copied())
            {
                me.chrome.announce_status(v.label(h));
            }
        };
        let step = if mods.shift { 10.0 } else { 1.0 };
        match combo.key {
            Key::Named(NamedKey::Escape) => {
                let t = self.measure.tool.as_mut();
                match t {
                    Some(t) if t.drag.is_some() => self.measure_put_back(),
                    Some(t) if t.prompt.is_some() => t.prompt = None,
                    Some(t) if !t.pts.is_empty() => t.pts.clear(),
                    Some(t) if t.focus.is_some() => t.focus = None,
                    Some(t) if t.selected.is_some() => t.selected = None,
                    _ => self.measure_close(),
                }
            }
            Key::Named(NamedKey::Tab) if mods.shift => move_focus(self, -1),
            Key::Named(NamedKey::Tab) => move_focus(self, 1),
            Key::Char('z' | 'Z') if mods.ctrl => self.measure_hit(Hit::Undo),
            Key::Char('c' | 'C') if mods.ctrl => self.measure_hit(Hit::Copy),
            Key::Char('s' | 'S') if mods.ctrl => self.measure_hit(Hit::Export),
            Key::Char('o' | 'O') if mods.ctrl => self.measure_hit(Hit::Import),
            Key::Named(NamedKey::Delete | NamedKey::Backspace) => self.measure_hit(Hit::Delete),
            Key::Named(NamedKey::Enter | NamedKey::Space) if focus.is_some() => {
                if let Some(h) = focus
                    && v.enabled(h)
                {
                    self.measure_hit(h);
                }
            }
            Key::Named(NamedKey::ArrowUp | NamedKey::ArrowDown)
                if matches!(focus, Some(Hit::Item(_))) =>
            {
                let d = if combo.key == Key::Named(NamedKey::ArrowUp) {
                    -1
                } else {
                    1
                };
                if let Some(Hit::Item(i)) = focus {
                    let j = (i as i32 + d).clamp(0, v.items.len() as i32 - 1) as usize;
                    if let Some(k) = focusables.iter().position(|h| *h == Hit::Item(j))
                        && let Some(t) = self.measure.tool.as_mut()
                    {
                        t.focus = Some(k);
                    }
                    self.measure_hit(Hit::Item(j));
                }
            }
            Key::Named(NamedKey::ArrowLeft) => self.measure_nudge(-1.0, 0.0, step),
            Key::Named(NamedKey::ArrowRight) => self.measure_nudge(1.0, 0.0, step),
            Key::Named(NamedKey::ArrowUp) => self.measure_nudge(0.0, -1.0, step),
            Key::Named(NamedKey::ArrowDown) => self.measure_nudge(0.0, 1.0, step),
            Key::Named(NamedKey::Space) if self.measure_key_pick() => {}
            Key::Named(NamedKey::Enter)
                if self.measure.tool.as_ref().is_some_and(|t| t.drag.is_some()) =>
            {
                self.measure_key_pick();
            }
            // Geometry: `[` and `]` go to a placed point.
            Key::Char(c @ ('[' | ']')) if plain => self.measure_step_point(c == ']'),
            Key::Named(NamedKey::Space) => {
                let at = self
                    .measure
                    .tool
                    .as_ref()
                    .and_then(|t| t.cursor.or(t.hover));
                if let Some(p) = at {
                    self.measure_place(p);
                } else {
                    // No crosshair yet: show it in the middle of the image.
                    self.measure_nudge(0.0, 0.0, 0.0);
                }
            }
            Key::Named(NamedKey::Enter) => {
                let polygon = self.measure.tool.as_ref().is_some_and(Tool::open_ended);
                if polygon {
                    self.measure_finish();
                } else {
                    self.measure_close();
                }
            }
            // Geometry: `t` and `Shift+T` step through the templates.
            Key::Char(c @ ('t' | 'T')) if plain && v.kind == Kind::Geometry => {
                self.measure_step_template(c == 't' && !mods.shift);
            }
            // Inspect: `]` and `[` step through the probes.
            Key::Char(c @ (']' | '[')) if plain && v.kind == Kind::Inspect => {
                self.measure_step_probe(c == ']');
            }
            Key::Char(c) if plain => {
                if let Some(k) = Kind::ALL
                    .iter()
                    .filter(|k| v.offers(**k))
                    .find(|k| k.letter() == c.to_ascii_lowercase())
                {
                    self.measure_set_kind(*k);
                } else if let Some(a) = self.keymap.lookup(combo)
                    && (a.group() == Group::ZoomPan || keyboard_inspection(a))
                {
                    self.action(a, el);
                }
            }
            _ => {
                if let Some(a) = self.keymap.lookup(combo)
                    && (a.group() == Group::ZoomPan
                        || a == Action::ToggleFullscreen
                        || keyboard_inspection(a))
                {
                    self.action(a, el);
                }
            }
        }
        self.redraw = true;
        true
    }

    /// The left button while the tool is open: the bar and the list, else drawing. `p` is in
    /// physical px, `lp` in logical px.
    pub(super) fn measure_button(&mut self, p: (f32, f32), lp: (f32, f32), down: bool) -> bool {
        let Some(t) = self.measure.tool.as_ref() else {
            return false;
        };
        if down {
            if let Some(h) = t
                .drawn
                .hits
                .iter()
                .find(|(r, _)| r.contains(lp))
                .map(|x| x.1)
            {
                let enabled = self.measure_view().is_some_and(|v| v.enabled(h));
                if enabled {
                    self.measure_hit(h);
                }
                return true;
            }
            if t.drawn.bar.contains(lp) || t.drawn.panel.contains(lp) || t.prompt.is_some() {
                return true;
            }
            // A geometry measurement's point is picked up (not while placing points).
            if t.pts.is_empty()
                && let Some((id, i)) = self.measure_handle_at(lp)
            {
                if let Some(m) = self.measure.list.get(id).cloned()
                    && let Some(t) = self.measure.tool.as_mut()
                {
                    t.drag = Some((id, i, Box::new(m)));
                    t.selected = Some(id);
                    t.focus = None;
                }
                self.redraw = true;
                return true;
            }
            let q = self.measure_stored(p);
            // A click on a polygon's first corner closes it.
            let close = t.kind == Kind::Polygon && t.pts.len() >= 3 && {
                let f = self.measure_screen(t.pts[0]);
                (f.0 - lp.0).hypot(f.1 - lp.1) <= CLOSE_RADIUS
            };
            if close {
                self.measure_finish();
            } else {
                if let Some(t) = self.measure.tool.as_mut() {
                    t.press = Some(lp);
                    t.cursor = None;
                    t.focus = None;
                }
                self.measure_place(q);
            }
            self.redraw = true;
            return true;
        }
        if let Some((id, _, before)) = self.measure.tool.as_mut().and_then(|t| t.drag.take()) {
            self.measure_drop(id, *before);
            return true;
        }
        let Some(start) = self.measure.tool.as_mut().and_then(|t| t.press.take()) else {
            return true;
        };
        // A drag draws two-point shapes in one go.
        let dragged = (start.0 - lp.0).hypot(start.1 - lp.1) > CLICK_SLOP;
        let pending = self
            .measure
            .tool
            .as_ref()
            .is_some_and(|t| t.drags() && t.pts.len() == 1);
        if dragged && pending {
            let q = self.measure_stored(p);
            self.measure_place(q);
        }
        self.redraw = true;
        true
    }

    /// Pointer motion while the tool is open: the rubber band follows. True while the button
    /// is down (drawing).
    pub(super) fn measure_motion(&mut self, p: (f32, f32)) -> bool {
        if self.measure.tool.is_none() {
            return false;
        }
        let q = self.measure_stored(p);
        let Some(t) = self.measure.tool.as_mut() else {
            return false;
        };
        if let Some((id, i, _)) = t.drag.as_ref() {
            let (id, i) = (*id, *i);
            if let Some(m) = self.measure.list.get_mut(id) {
                geometry::set_point(m, i, q);
            }
            self.redraw = true;
            return true;
        }
        t.hover = Some(q);
        if !t.pts.is_empty() {
            t.cursor = None;
            self.redraw = true;
        }
        t.press.is_some()
    }

    pub(super) fn measure_a11y_request(&mut self, req: &accesskit::ActionRequest) -> bool {
        if self.measure.tool.is_none() || req.target_node.0 & !0xFFFF != A11Y_BASE {
            return false;
        }
        if let Some(v) = self.measure_view()
            && let Some(h) = target(req.target_node.0, &v)
        {
            match req.action {
                accesskit::Action::Click if v.enabled(h) => self.measure_hit(h),
                accesskit::Action::Focus => {
                    if let Some(i) = v.focusables().iter().position(|x| *x == h)
                        && let Some(t) = self.measure.tool.as_mut()
                    {
                        t.focus = Some(i);
                    }
                }
                _ => {}
            }
        }
        self.redraw = true;
        true
    }

    /// What exports record about the image (the file hash is added where they are written).
    pub(super) fn measure_provenance(&self) -> (Provenance, Option<PathBuf>) {
        let key = self.measure.key.as_ref();
        let img = self.current_id().and_then(|id| self.images.get(&id));
        let file = key
            .and_then(|k| k.path.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let info = img.and_then(|i| i.info.as_ref());
        let prov = Provenance {
            software: format!("Specular {}", env!("CARGO_PKG_VERSION")),
            file,
            decoder: info
                .map(|i| format!("{:?} (Specular {})", i.format, env!("CARGO_PKG_VERSION"))),
            blake3: None,
            size: Some(self.view.stored_size()),
            created: None,
        };
        // A page inside an archive has no file of its own to hash.
        let path = key.map(|k| k.path.clone()).filter(|p| p.is_file());
        (prov, path)
    }

    /// Copies the list as CSV.
    /// Scripting (`rois`): the current image's measurements and their provenance.
    pub(super) fn measure_script(&mut self) -> (Vec<Measurement>, Provenance) {
        self.measure_sync();
        let (prov, _) = self.measure_provenance();
        (self.measure.list.items().to_vec(), prov)
    }

    pub(super) fn measure_copy_csv(&mut self) {
        self.measure_sync();
        if self.measure.list.is_empty() {
            let now = self.now();
            self.chrome.toast(t!("measure-tool-nothing"), None, now);
            return;
        }
        let (prov, _) = self.measure_provenance();
        let text = csv::write(self.measure.list.items(), &prov);
        self.clipboard
            .copy_text(&self.loader, text, Copied::Measurements);
    }

    /// Saves the list as CSV, GeoJSON or ImageJ ROI (by the chosen file's extension).
    pub(super) fn measure_export(&mut self) {
        self.measure_sync();
        if self.measure.list.is_empty() {
            let now = self.now();
            self.chrome.toast(t!("measure-tool-nothing"), None, now);
            return;
        }
        let items = self.measure.list.items().to_vec();
        let (mut prov, path) = self.measure_provenance();
        let stem = self
            .measure
            .key
            .as_ref()
            .and_then(|k| k.path.file_stem())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "image".to_owned());
        let dir = path
            .as_ref()
            .and_then(|p| p.parent())
            .map(Path::to_path_buf);
        let tx = self.measure.tx.clone();
        self.loader.background("measure-export", move |l| {
            let mut dlg = rfd::FileDialog::new()
                .add_filter("CSV", &["csv"])
                .add_filter("GeoJSON", &["geojson", "json"])
                .add_filter("ImageJ ROI", &["zip", "roi"])
                .add_filter("DICOM SR", &["dcm"])
                .set_file_name(format!("{stem}-measurements.csv"));
            if let Some(d) = dir {
                dlg = dlg.set_directory(d);
            }
            let Some(out) = dlg.save_file() else { return };
            if let Some(p) = &path {
                match provenance::hash_file(p) {
                    Ok(h) => prov.blake3 = Some(h),
                    Err(e) => warn!(target: UI, "measure export: hashing {}: {e}", p.display()),
                }
            }
            let bytes = match (Format::of(&out), &path) {
                (Format::Sr, Some(p)) => std::fs::read(p)
                    .map_err(|e| crate::error_text::io_reason(&e))
                    .and_then(|src| {
                        let unix = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map_or(0, |d| d.as_secs());
                        sr_bytes(&items, &src, unix)
                    }),
                (f, _) => export_bytes(f, &items, &prov),
            };
            let r = bytes
                .and_then(|b| write_bytes(&out, &b).map_err(|e| crate::error_text::io_reason(&e)))
                .map(|()| out);
            let _ = tx.send(Done::Saved(r));
            l.waker()();
        });
    }

    /// Adds shapes made elsewhere (DS9 regions) with their labels to the current image's
    /// measurements; how many were added.
    pub(super) fn measure_add(&mut self, shapes: Vec<(Shape, String)>) -> usize {
        self.measure_sync();
        if self.measure.key.is_none() {
            return 0;
        }
        let cal = self.measure.cal;
        let items: Vec<Measurement> = shapes
            .into_iter()
            .map(|(s, label)| {
                let mut m = Measurement::new(s, cal);
                m.label = label;
                m
            })
            .collect();
        let n = items.len();
        for id in self.measure.list.extend(items) {
            self.measure_stats(id);
        }
        self.redraw = true;
        n
    }

    /// Reads measurements from a GeoJSON or ImageJ ROI file into the list.
    pub(super) fn measure_import(&mut self) {
        self.measure_sync();
        let Some(key) = self.measure.key.clone() else {
            return;
        };
        let dir = key.path.parent().map(Path::to_path_buf);
        let tx = self.measure.tx.clone();
        self.loader.background("measure-import", move |l| {
            let mut dlg = rfd::FileDialog::new()
                .add_filter("GeoJSON, ImageJ ROI", &["geojson", "json", "roi", "zip"]);
            if let Some(d) = dir {
                dlg = dlg.set_directory(d);
            }
            let Some(p) = dlg.pick_file() else { return };
            const MAX: u64 = 256 << 20;
            let r = std::fs::metadata(&p)
                .map_err(|e| crate::error_text::io_reason(&e))
                .and_then(|m| {
                    if m.len() > MAX {
                        Err(t!("measure-tool-too-large"))
                    } else {
                        std::fs::read(&p).map_err(|e| crate::error_text::io_reason(&e))
                    }
                })
                .and_then(|data| import_bytes(&p, &data));
            let _ = tx.send(Done::Imported(key, r));
            l.waker()();
        });
    }

    /// The value of `shape` (stored pixels) as a new measurement would read: through the
    /// drawing's scale under it until the user calibrates (markup lengths and areas).
    pub(super) fn measure_value(&mut self, shape: Shape) -> String {
        self.measure_sync();
        let pts = shape.outline();
        let region = self
            .measure
            .scale
            .as_ref()
            .filter(|_| self.measure.cal.source != Source::User)
            .and_then(|d| {
                let (x, y) = anchor(&pts);
                d.region_at(x, y).map(|i| d.regions[i].clone())
            });
        let cal = region
            .as_ref()
            .map_or_else(|| self.measure_cal_at(&shape.points()), |r| r.cal);
        value_text(&Measurement::new(shape, cal), region.as_ref())
    }

    /// The current image's lengths and areas (stored pixels), whether each is an area, and
    /// its value, for markup.
    /// The current image's measurements (none when the list is another image's).
    pub(super) fn measure_items(&self) -> &[Measurement] {
        if self.measure.key.is_none() || self.measure.key != self.measure_image_key() {
            return &[];
        }
        self.measure.list.items()
    }

    pub(super) fn measure_markup(&self) -> Vec<ForMarkup> {
        if self.measure.key.is_none() || self.measure.key != self.measure_image_key() {
            return Vec::new();
        }
        self.measure
            .list
            .items()
            .iter()
            .filter_map(|m| {
                let (pts, area) = match &m.shape {
                    Shape::Line(a, b) => (vec![*a, *b], false),
                    s if s.is_area() => (s.outline(), true),
                    _ => return None,
                };
                Some((
                    pts.iter().map(|p| (p.x, p.y)).collect(),
                    area,
                    // Stored with the markup: no bidi isolates in the file.
                    specular_i18n::strip_isolates(&value_text(m, self.measure_region(m.id))),
                ))
            })
            .collect()
    }

    /// The current image's measurements for a report: number, name, headline value (with a
    /// region's statistics), where the scale comes from, and the shape.
    pub(super) fn measure_report_rows(&self) -> Vec<(u32, String, String, String, Shape)> {
        if self.measure.key.is_none() || self.measure.key != self.measure_image_key() {
            return Vec::new();
        }
        self.measure
            .list
            .items()
            .iter()
            .map(|m| {
                let region = self.measure_region(m.id);
                let (name, value) = match (m.probe, m.template) {
                    (Some(p), _) => (probe::label(p), probe::lines(m, false).0),
                    (_, Some(t)) => (geometry::name(t), geometry::lines(m.outcome().as_ref()).0),
                    _ => {
                        let mut v = vec![value_text(m, region)];
                        if let Some(st) = &m.stats {
                            v.extend(stats_lines(st, &m.channels));
                        }
                        let v = specular_i18n::dot_join(&v);
                        (shape_name(&m.shape), v)
                    }
                };
                (
                    m.id,
                    name,
                    value,
                    badge(m.cal.source, region),
                    m.shape.clone(),
                )
            })
            .collect()
    }

    /// The current image's spots, lines and areas: number, name, shape and statistics (a
    /// thermal sequence's temperatures over time, the thermal report).
    pub(super) fn measure_regions(&self) -> Vec<(u32, String, Shape, Option<ChannelStats>)> {
        if self.measure.key.is_none() || self.measure.key != self.measure_image_key() {
            return Vec::new();
        }
        self.measure
            .list
            .items()
            .iter()
            .filter(|m| {
                m.probe.is_none()
                    && (m.shape.is_area()
                        || m.shape.is_path()
                        || matches!(m.shape, Shape::Point(_)))
            })
            .map(|m| {
                let st = m.stats.as_ref().and_then(|s| s.first().cloned());
                (m.id, shape_name(&m.shape), m.shape.clone(), st)
            })
            .collect()
    }

    /// The palette's and keymap's measurement actions.
    pub(super) fn measure_action(&mut self, a: Action) -> bool {
        match a {
            Action::Measure => {
                if self.measure.tool.is_some() {
                    self.measure_close();
                } else {
                    self.measure_open();
                }
            }
            Action::MeasureCopyCsv => self.measure_copy_csv(),
            Action::MeasureExport => self.measure_export(),
            // The tool on its geometry templates (offered with the medical tools on).
            Action::MeasureTemplates => {
                if !self.measure_geometry_offered() {
                    let now = self.now();
                    self.chrome
                        .toast(t!("measure-templates-need-medical"), None, now);
                    return true;
                }
                self.measure_open();
                if self.measure.tool.is_some() {
                    self.measure_set_kind(Kind::Geometry);
                }
            }
            _ => return false,
        }
        true
    }

    /// `--show measure` (screenshots): the tool with a line and a rectangle.
    pub(super) fn measure_show(&mut self, what: &str) -> bool {
        if let Some(key) = what.strip_prefix("geometry:") {
            // `--show geometry:<template>`: its points on a 7 × 11 grid over the image.
            let Some(t) = Template::from_key(key) else {
                return false;
            };
            self.measure_open();
            if let Some(tool) = self.measure.tool.as_mut() {
                tool.template = t;
            }
            self.measure_set_kind(Kind::Geometry);
            let (w, h) = self.view.stored_size();
            let (w, h) = (f64::from(w), f64::from(h));
            for i in 0..t.landmarks().len() {
                let k = i as f64;
                let x = (0.2 + 0.6 * ((k * 0.381_966) % 1.0)) * w;
                let y = (0.2 + 0.6 * ((k * 0.618_034 + 0.1) % 1.0)) * h;
                self.measure_place(P::new(x, y));
            }
            return true;
        }
        if what != "measure" {
            return false;
        }
        self.measure_open();
        let (w, h) = self.view.stored_size();
        let (w, h) = (f64::from(w), f64::from(h));
        let line = [P::new(w * 0.2, h * 0.3), P::new(w * 0.6, h * 0.35)];
        let rect = [P::new(w * 0.3, h * 0.5), P::new(w * 0.55, h * 0.8)];
        self.measure_set_kind(Kind::Line);
        for p in line {
            self.measure_place(p);
        }
        self.measure_set_kind(Kind::Rect);
        for p in rect {
            self.measure_place(p);
        }
        true
    }
}

/// A length or an area for markup: its points (stored pixels), whether it is an area, and its
/// value.
pub(super) type ForMarkup = (Vec<(f64, f64)>, bool, String);

/// The name of a shape in the list.
fn shape_name(s: &Shape) -> String {
    match s {
        Shape::Point(_) => t!("measure-tool-point"),
        Shape::Line(..) => t!("measure-tool-line"),
        Shape::Polyline(_) => t!("measure-tool-polyline"),
        Shape::Angle { .. } => t!("measure-tool-angle"),
        Shape::Rect { .. } => t!("measure-tool-rect"),
        Shape::Ellipse { .. } => t!("measure-tool-ellipse"),
        Shape::Polygon(_) => t!("measure-tool-polygon"),
        Shape::Freehand(_) => t!("measure-tool-freehand"),
    }
}

/// A drawing's own units (DXF `$INSUNITS`, HP-GL millimetres) per stored pixel: the
/// session's pixels are the sheet scaled uniformly, so lengths measure true.
fn drawing_calibration(d: &specular_decode::drawing::DrawingPage) -> Option<Calibration> {
    let (unit, per) = match d.prepared.scene.units.code {
        1 => (Unit::Inch, 1.0),
        2 => (Unit::Inch, 12.0),
        4 => (Unit::Mm, 1.0),
        5 => (Unit::Cm, 1.0),
        6 => (Unit::M, 1.0),
        12 => (Unit::Nm, 1.0),
        13 => (Unit::Um, 1.0),
        _ => return None,
    };
    let c = d.to_stored.as_coeffs();
    let (sx, sy) = (c[0].hypot(c[1]), c[2].hypot(c[3]));
    let cal = Calibration::spacing(per / sx, per / sy, unit, Source::DrawingUnits).ok()?;
    // IC layouts span nanometres to millimetres: micrometres, or nanometres for a small cell.
    Some(if d.prepared.scene.layout.is_some() {
        cal.readable()
    } else {
        cal
    })
}

/// A georeferenced image's ground size of a pixel at its centre, in metres.
fn map_calibration(
    g: &specular_decode::geo::GeoRef,
    info: &specular_decode::ImageInfo,
) -> Option<Calibration> {
    let (cx, cy) = (f64::from(info.width) / 2.0, f64::from(info.height) / 2.0);
    let (sx, sy) = g.ground_step(cx, cy)?;
    Calibration::spacing(sx.abs(), sy.abs(), Unit::M, Source::MapScale)
        .ok()
        .map(|c| c.readable())
}

/// A scientific TIFF's pixel size (its resolution in ImageJ's unit, or a vendor tag such as
/// an LSM voxel or a SEM pixel size).
fn science_calibration(s: &specular_decode::science::ScienceSummary) -> Option<Calibration> {
    use specular_decode::science::PixelSource;
    let source = match s.pixel_source? {
        PixelSource::Resolution => Source::TiffResolution,
        PixelSource::Vendor => Source::VendorTag,
    };
    let p = s.pixel.as_ref()?;
    let (ux, uy) = (Unit::from_key(&p.unit_x)?, Unit::from_key(&p.unit_y)?);
    let (x, y) = (p.x.step.abs(), p.y.step.abs());
    if ux == uy {
        return Calibration::spacing(x, y, ux, source).ok();
    }
    // Mixed units: both in metres, then shown readable.
    let (mx, my) = (ux.metres()?, uy.metres()?);
    Calibration::spacing(x * mx, y * my, Unit::M, source)
        .ok()
        .map(|c| c.readable())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn science_pixels_calibrate() {
        use specular_decode::science::{Linear, PixelCalibration, PixelSource, ScienceSummary};
        let lin = |step| Linear { offset: 0.0, step };
        let mut s = ScienceSummary {
            pixel: Some(PixelCalibration {
                x: lin(0.25),
                y: lin(0.5),
                unit_x: "µm".into(),
                unit_y: "µm".into(),
            }),
            ..Default::default()
        };
        // A pixel size of unknown origin (another reader's grid) is not the tool's.
        assert!(science_calibration(&s).is_none());
        s.pixel_source = Some(PixelSource::Resolution);
        let c = science_calibration(&s).unwrap();
        assert_eq!(c.axis_spacing(), (0.25, 0.5));
        assert_eq!(c.length(1.0).unit, Unit::Um);
        assert_eq!(c.length(1.0).source, Source::TiffResolution);
        s.pixel_source = Some(PixelSource::Vendor);
        s.pixel.as_mut().unwrap().unit_y = "nm".into();
        let c = science_calibration(&s).unwrap();
        assert_eq!(c.length(1.0).source, Source::VendorTag);
        s.pixel.as_mut().unwrap().unit_x = "parsec".into();
        assert!(science_calibration(&s).is_none());
    }

    #[test]
    fn tools_make_their_shapes() {
        let a = P::new(1.0, 2.0);
        let b = P::new(5.0, 8.0);
        let c = P::new(9.0, 2.0);
        assert_eq!(shape_of(Kind::Line, &[a, b]), Some(Shape::Line(a, b)));
        assert_eq!(shape_of(Kind::Line, &[a, a]), None);
        assert!(matches!(
            shape_of(Kind::Rect, &[b, a]),
            Some(Shape::Rect { x0, y0, x1, y1 }) if (x0, y0, x1, y1) == (1.0, 2.0, 5.0, 8.0)
        ));
        assert_eq!(shape_of(Kind::Rect, &[a, P::new(1.0, 9.0)]), None);
        assert!(matches!(
            shape_of(Kind::Ellipse, &[a, b]),
            Some(Shape::Ellipse { .. })
        ));
        assert!(matches!(
            shape_of(Kind::Angle, &[a, b, c]),
            Some(Shape::Angle { .. })
        ));
        assert_eq!(shape_of(Kind::Polygon, &[a, b]), None);
        assert!(matches!(
            shape_of(Kind::Polygon, &[a, b, c]),
            Some(Shape::Polygon(_))
        ));
        assert_eq!(Kind::Polygon.points(), None);
        assert!(Kind::Calibrate.drags() && !Kind::Angle.drags());
    }

    #[test]
    fn flow_wraps_lines() {
        let p = flow(&[50.0, 50.0, 50.0], 120.0, 10.0);
        assert_eq!(p, vec![(0, 0.0), (0, 60.0), (1, 0.0)]);
        // An item wider than a line still gets one.
        assert_eq!(flow(&[200.0, 10.0], 100.0, 0.0), vec![(0, 0.0), (1, 0.0)]);
    }

    fn plane(w: u32, h: u32, bpp: usize, data: Vec<u8>) -> specular_core::RasterPlane {
        specular_core::RasterPlane {
            width: w,
            height: h,
            stride: w as usize * bpp,
            data,
            h_div: 1,
            v_div: 1,
        }
    }

    fn raster(
        w: u32,
        h: u32,
        scale: u32,
        layout: PixelLayout,
        planes: Vec<specular_core::RasterPlane>,
    ) -> Raster {
        Raster {
            width: w,
            height: h,
            origin: (0, 0),
            scale,
            layout,
            planes,
            truncated: false,
        }
    }

    #[test]
    fn statistics_of_rasters() {
        // A gray ramp: value = x.
        let (w, h) = (16u32, 8u32);
        let data: Vec<u8> = (0..h).flat_map(|_| (0..w).map(|x| x as u8)).collect();
        let r = raster(w, h, 1, PixelLayout::Gray, vec![plane(w, h, 1, data)]);
        let shape = Shape::Rect {
            x0: 2.0,
            y0: 1.0,
            x1: 6.0,
            y1: 3.0,
        };
        let (st, names) = raster_stats(&r, &shape).unwrap();
        assert_eq!(names, ["Gray"]);
        assert_eq!(st[0].count, 8);
        assert_eq!((st[0].min, st[0].max, st[0].mean), (2.0, 5.0, 3.5));
        // Outside the raster: nothing.
        assert!(
            raster_stats(
                &r,
                &Shape::Rect {
                    x0: 20.0,
                    y0: 0.0,
                    x1: 30.0,
                    y1: 4.0
                }
            )
            .is_none()
        );

        // RGBA at half scale: the shape is scaled down with it.
        let (w2, h2) = (8u32, 4u32);
        let data: Vec<u8> = (0..w2 * h2)
            .flat_map(|i| [10, 20, (i % w2) as u8, 255])
            .collect();
        let r2 = raster(w2, h2, 2, PixelLayout::Rgba, vec![plane(w2, h2, 4, data)]);
        let (st, names) = raster_stats(
            &r2,
            &Shape::Rect {
                x0: 4.0,
                y0: 0.0,
                x1: 8.0,
                y1: 8.0,
            },
        )
        .unwrap();
        assert_eq!(names.len(), 4);
        assert_eq!(st[0].count, 8);
        assert_eq!(st[0].mean, 10.0);
        assert_eq!((st[2].min, st[2].max), (2.0, 3.0));

        // 16-bit grey.
        let data: Vec<u8> = (0..4u16).flat_map(|v| (v * 1000).to_ne_bytes()).collect();
        let r3 = raster(2, 2, 1, PixelLayout::Gray16, vec![plane(2, 2, 2, data)]);
        let (st, _) = raster_stats(
            &r3,
            &Shape::Rect {
                x0: 0.0,
                y0: 0.0,
                x1: 2.0,
                y1: 2.0,
            },
        )
        .unwrap();
        assert_eq!((st[0].min, st[0].max, st[0].sum), (0.0, 3000.0, 6000.0));
    }

    #[test]
    fn half_floats() {
        assert_eq!(half(0x3C00), 1.0);
        assert_eq!(half(0xC000), -2.0);
        assert_eq!(half(0x3800), 0.5);
        assert_eq!(half(0), 0.0);
        assert!(half(0x7E00).is_nan());
    }

    #[test]
    fn exports_by_extension_and_back() {
        let cal = Calibration::dicom(0.5, 0.5, Source::DicomPixelSpacing).unwrap();
        let mut list = List::new();
        list.push(Measurement::new(
            Shape::Line(P::new(0.0, 0.0), P::new(3.0, 4.0)),
            cal,
        ));
        list.push(Measurement::new(
            Shape::Rect {
                x0: 1.0,
                y0: 1.0,
                x1: 5.0,
                y1: 4.0,
            },
            cal,
        ));
        let prov = Provenance {
            software: "Specular test".into(),
            file: "a.dcm".into(),
            ..Provenance::default()
        };
        assert_eq!(Format::of(Path::new("x.CSV")), Format::Csv);
        assert_eq!(Format::of(Path::new("x.geojson")), Format::GeoJson);
        assert_eq!(Format::of(Path::new("x.zip")), Format::RoiSet);
        assert_eq!(Format::of(Path::new("x.roi")), Format::Roi);
        let csv = export_bytes(Format::Csv, list.items(), &prov).unwrap();
        assert!(
            String::from_utf8(csv)
                .unwrap()
                .contains("dicom-pixel-spacing")
        );
        for (f, name) in [(Format::GeoJson, "m.geojson"), (Format::RoiSet, "m.zip")] {
            let b = export_bytes(f, list.items(), &prov).unwrap();
            let (back, skipped) = import_bytes(Path::new(name), &b).unwrap();
            assert_eq!(skipped, 0);
            assert_eq!(back.len(), 2, "{name}");
            assert_eq!(back[1].shape, list.items()[1].shape, "{name}");
        }
        // One measurement as a single .roi; several as a set even when named .roi.
        let one = export_bytes(Format::Roi, &list.items()[..1], &prov).unwrap();
        assert_eq!(&one[..4], b"Iout");
        let set = export_bytes(Format::Roi, list.items(), &prov).unwrap();
        assert_eq!(&set[..2], b"PK");
        assert!(import_bytes(Path::new("x.geojson"), b"{").is_err());
    }
}
