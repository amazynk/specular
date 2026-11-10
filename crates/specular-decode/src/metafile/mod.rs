//! Windows metafiles: WMF, EMF, and EMF+ (the GDI+ records EMF files carry in comments), drawn
//! with tiny-skia.
//!
//! The records are parsed once, at open, into a display list ([`Op`]: fills, strokes, bitmaps)
//! in the file's device space. A session renders it the way [`crate::svg`] renders SVG: the
//! scale-1 raster is the picture's size at 96 dpi × a power of two (long side at least
//! [`MIN_LONG_SIDE`], capped by [`MAX_SIDE`] and [`MAX_PIXELS`]), and scales and regions are
//! rendered directly, never resampled. The background is transparent.
//!
//! Drawn: lines, polylines, polygons, Béziers, rectangles, rounded rectangles, ellipses, arcs,
//! chords and pies, GDI paths; solid, dashed and hatched pens and brushes (a hatch as a tint of
//! its colour, a pattern brush as the average of its bitmap); map modes, window/viewport and
//! world transforms; bitmaps (StretchDIBits, BitBlt, StretchBlt, AlphaBlend, TransparentBlt,
//! the WMF DIB records) and pattern blits. EMF+: solid, hatch and linear-gradient brushes, pens,
//! paths, rectangles, polygons, lines, ellipses, pies, arcs, Béziers, raw and PNG/JPEG bitmaps,
//! and the world/page transforms and their save stack.
//!
//! Not drawn, and said so in the info panel: text (the decoder has no fonts) and clipping.
//! Raster operations other than copy draw as copy; cardinal splines, regions and metafiles
//! nested in EMF+ are skipped.
//!
//! Limits: [`MAX_OPS`] display-list entries and [`MAX_IMAGE_PIXELS`] bitmap pixels per file;
//! every count and offset is checked against the record holding it, so hostile files fail or
//! draw less, never panic, and parsing is linear in the file size.

mod emf;
mod plus;
mod wmf;

use crate::svg::{MAX_PIXELS, MAX_SIDE, MIN_LONG_SIDE};
use crate::{DecodeError, DecodeRequest, Decoder, Format, ImageInfo, Result, Session};
use resvg::tiny_skia as sk;
use specular_core::{CancelToken, Orientation, PixelLayout, Raster, RasterPlane, Source};
use std::f32::consts::{FRAC_PI_2, TAU};
use std::sync::Arc;

/// Most display-list entries per file.
pub const MAX_OPS: usize = 1 << 21;
/// Most bitmap pixels per file (all bitmaps together).
pub const MAX_IMAGE_PIXELS: u64 = 1 << 26;
/// Deepest SaveDC / EMF+ save stack.
const MAX_SAVE: usize = 1024;
/// Most bytes a `.wmz`/`.emz` may unpack to.
const MAX_COMPRESSED_OUT: usize = 256 << 20;

#[derive(Clone, Debug)]
pub(crate) enum Paint {
    Solid(sk::Color),
    Linear {
        p0: sk::Point,
        p1: sk::Point,
        c0: sk::Color,
        c1: sk::Color,
    },
}

#[derive(Clone, Debug)]
pub(crate) struct Pen {
    pub color: sk::Color,
    /// In op space.
    pub width: f32,
    pub cap: sk::LineCap,
    pub join: sk::LineJoin,
    pub miter: f32,
    /// Dash and gap lengths in op space.
    pub dash: Option<Vec<f32>>,
}

pub(crate) enum Op {
    Fill {
        path: sk::Path,
        paint: Paint,
        even_odd: bool,
    },
    Stroke {
        path: sk::Path,
        pen: Pen,
    },
    Image {
        pixmap: Arc<sk::Pixmap>,
        /// Image pixels → op space.
        xf: sk::Transform,
        opacity: f32,
    },
}

/// What was in the file but not drawn.
#[derive(Default, Debug)]
pub(crate) struct Notes {
    pub text: bool,
    pub clip: bool,
    pub skipped_images: u32,
    /// EMF+ records were played; `true` for a dual file (GDI fallback records skipped).
    pub plus: Option<bool>,
}

/// The display list being built.
#[derive(Default)]
pub(crate) struct List {
    pub ops: Vec<Op>,
    pub notes: Notes,
    image_pixels: u64,
}

impl List {
    fn push(&mut self, op: Op) {
        if self.ops.len() < MAX_OPS {
            self.ops.push(op);
        }
    }
    pub fn fill(&mut self, path: sk::Path, paint: Paint, even_odd: bool) {
        self.push(Op::Fill {
            path,
            paint,
            even_odd,
        });
    }
    pub fn stroke(&mut self, path: sk::Path, mut pen: Pen) {
        if !(pen.width.is_finite() && pen.width >= 0.0) {
            return;
        }
        // A dash pattern far finer than the path would make millions of segments: solid.
        if let Some(d) = &pen.dash {
            let period: f32 = d.iter().sum();
            let pts = path.points();
            let len: f32 = pts
                .windows(2)
                .map(|w| (w[1].x - w[0].x).hypot(w[1].y - w[0].y))
                .sum();
            if !(period > 0.0 && period.is_finite() && len / period < 50_000.0) {
                pen.dash = None;
            }
        }
        self.push(Op::Stroke { path, pen });
    }
    /// A premultiplied pixmap from straight-alpha RGBA, within the file's pixel budget.
    pub fn pixmap(&mut self, w: u32, h: u32, rgba: &[u8]) -> Option<Arc<sk::Pixmap>> {
        let px = w as u64 * h as u64;
        if px == 0 || rgba.len() as u64 != px * 4 || self.image_pixels + px > MAX_IMAGE_PIXELS {
            self.notes.skipped_images += 1;
            return None;
        }
        let mut data = rgba.to_vec();
        for p in data.chunks_exact_mut(4) {
            let a = p[3] as u32;
            if a != 255 {
                for c in &mut p[..3] {
                    *c = ((*c as u32 * a + 127) / 255) as u8;
                }
            }
        }
        let pm = sk::Pixmap::from_vec(data, sk::IntSize::from_wh(w, h)?)?;
        self.image_pixels += px;
        Some(Arc::new(pm))
    }
    pub fn image(&mut self, pixmap: Arc<sk::Pixmap>, xf: sk::Transform, opacity: f32) {
        if xf.is_finite() && xf.invert().is_some() {
            self.push(Op::Image {
                pixmap,
                xf,
                opacity: opacity.clamp(0.0, 1.0),
            });
        }
    }

    /// The bounds of everything drawn, in op space.
    fn bounds(&self) -> Option<sk::Rect> {
        let mut r: Option<(f32, f32, f32, f32)> = None;
        let mut add = |x: f32, y: f32| {
            if x.is_finite() && y.is_finite() {
                r = Some(match r {
                    None => (x, y, x, y),
                    Some((l, t, rr, b)) => (l.min(x), t.min(y), rr.max(x), b.max(y)),
                });
            }
        };
        for op in &self.ops {
            match op {
                Op::Fill { path, .. } => {
                    let b = path.bounds();
                    add(b.left(), b.top());
                    add(b.right(), b.bottom());
                }
                Op::Stroke { path, pen } => {
                    let b = path.bounds();
                    let h = pen.width / 2.0;
                    add(b.left() - h, b.top() - h);
                    add(b.right() + h, b.bottom() + h);
                }
                Op::Image { pixmap, xf, .. } => {
                    let (w, h) = (pixmap.width() as f32, pixmap.height() as f32);
                    for (x, y) in [(0.0, 0.0), (w, 0.0), (0.0, h), (w, h)] {
                        let mut p = sk::Point::from_xy(x, y);
                        xf.map_point(&mut p);
                        add(p.x, p.y);
                    }
                }
            }
        }
        let (l, t, r, b) = r?;
        sk::Rect::from_ltrb(l, t, r, b)
    }
}

/// A parsed metafile, ready to render.
pub(crate) struct Picture {
    ops: Vec<Op>,
    /// Op space → picture pixels at 96 dpi.
    base: sk::Transform,
    width: f32,
    height: f32,
    notes: Notes,
    image_bytes: u64,
}

/// Close the list: `frame` is the picture's rectangle in op space and its size in 96-dpi pixels;
/// without one, the bounds of what was drawn, one op unit a pixel.
pub(crate) fn finish(list: List, frame: Option<(sk::Rect, f32, f32)>) -> Result<Picture> {
    let usable = |r: &sk::Rect, w: f32, h: f32| {
        r.width() > 0.0 && r.height() > 0.0 && w >= 0.5 && h >= 0.5 && w < 1e7 && h < 1e7
    };
    let (rect, w, h) = match frame.filter(|(r, w, h)| usable(r, *w, *h)) {
        Some(f) => f,
        None => {
            let b = list
                .bounds()
                .ok_or_else(|| DecodeError::Malformed("metafile draws nothing".into()))?;
            if !usable(&b, b.width(), b.height()) {
                return Err(DecodeError::Malformed("metafile without a size".into()));
            }
            (b, b.width(), b.height())
        }
    };
    let base = sk::Transform::from_row(
        w / rect.width(),
        0.0,
        0.0,
        h / rect.height(),
        -rect.left() * w / rect.width(),
        -rect.top() * h / rect.height(),
    );
    let image_bytes = list.image_pixels * 4;
    Ok(Picture {
        ops: list.ops,
        base,
        width: w,
        height: h,
        notes: list.notes,
        image_bytes,
    })
}

pub(crate) fn colorref(c: u32) -> sk::Color {
    sk::Color::from_rgba8(c as u8, (c >> 8) as u8, (c >> 16) as u8, 255)
}

pub(crate) fn argb(c: u32) -> sk::Color {
    sk::Color::from_rgba8((c >> 16) as u8, (c >> 8) as u8, c as u8, (c >> 24) as u8)
}

/// How much `t` scales lengths (the square root of its area scale).
pub(crate) fn scale_of(t: &sk::Transform) -> f32 {
    let s = (t.sx * t.sy - t.kx * t.ky).abs().sqrt();
    if s.is_finite() { s } else { 0.0 }
}

pub(crate) fn polygon(pts: &[(f32, f32)], close: bool) -> Option<sk::Path> {
    let mut pb = sk::PathBuilder::new();
    let (first, rest) = pts.split_first()?;
    pb.move_to(first.0, first.1);
    for p in rest {
        pb.line_to(p.0, p.1);
    }
    if close {
        pb.close();
    }
    pb.finish()
}

/// Cubic Béziers: a start point, then three points per curve.
pub(crate) fn beziers(pts: &[(f32, f32)]) -> Option<sk::Path> {
    let mut pb = sk::PathBuilder::new();
    let (first, rest) = pts.split_first()?;
    pb.move_to(first.0, first.1);
    for c in rest.chunks_exact(3) {
        pb.cubic_to(c[0].0, c[0].1, c[1].0, c[1].1, c[2].0, c[2].1);
    }
    pb.finish()
}

fn ltrb(l: f32, t: f32, r: f32, b: f32) -> Option<sk::Rect> {
    sk::Rect::from_ltrb(l.min(r), t.min(b), l.max(r), t.max(b))
}

pub(crate) fn rect(l: f32, t: f32, r: f32, b: f32) -> Option<sk::Path> {
    Some(sk::PathBuilder::from_rect(ltrb(l, t, r, b)?))
}

pub(crate) fn ellipse(l: f32, t: f32, r: f32, b: f32) -> Option<sk::Path> {
    sk::PathBuilder::from_oval(ltrb(l, t, r, b)?)
}

/// A rectangle with elliptical corners `cw` × `ch` (full widths, as GDI gives them).
pub(crate) fn round_rect(l: f32, t: f32, r: f32, b: f32, cw: f32, ch: f32) -> Option<sk::Path> {
    let x = ltrb(l, t, r, b)?;
    let rx = (cw.abs() / 2.0).min(x.width() / 2.0);
    let ry = (ch.abs() / 2.0).min(x.height() / 2.0);
    if rx <= 0.0 || ry <= 0.0 {
        return rect(l, t, r, b);
    }
    let mut pb = sk::PathBuilder::new();
    let (l, t, r, b) = (x.left(), x.top(), x.right(), x.bottom());
    pb.move_to(l + rx, t);
    pb.line_to(r - rx, t);
    arc_to(&mut pb, r - rx, t + ry, rx, ry, -FRAC_PI_2, FRAC_PI_2);
    pb.line_to(r, b - ry);
    arc_to(&mut pb, r - rx, b - ry, rx, ry, 0.0, FRAC_PI_2);
    pb.line_to(l + rx, b);
    arc_to(&mut pb, l + rx, b - ry, rx, ry, FRAC_PI_2, FRAC_PI_2);
    pb.line_to(l, t + ry);
    arc_to(&mut pb, l + rx, t + ry, rx, ry, -2.0 * FRAC_PI_2, FRAC_PI_2);
    pb.close();
    pb.finish()
}

/// Append an elliptical arc from parameter `a0` over `sweep` radians (positive is clockwise
/// on a y-down page) as cubic Béziers; the path must already be at its start point.
pub(crate) fn arc_to(
    pb: &mut sk::PathBuilder,
    cx: f32,
    cy: f32,
    rx: f32,
    ry: f32,
    a0: f32,
    sweep: f32,
) {
    if !(sweep.is_finite() && a0.is_finite()) {
        return;
    }
    let n = ((sweep.abs() / FRAC_PI_2).ceil() as usize).clamp(1, 8);
    let d = sweep / n as f32;
    let k = 4.0 / 3.0 * (d / 4.0).tan();
    let at = |t: f32| (cx + rx * t.cos(), cy + ry * t.sin());
    let tangent = |t: f32| (-rx * t.sin(), ry * t.cos());
    for i in 0..n {
        let (t0, t1) = (a0 + d * i as f32, a0 + d * (i + 1) as f32);
        let (p0, p3) = (at(t0), at(t1));
        let (d0, d1) = (tangent(t0), tangent(t1));
        pb.cubic_to(
            p0.0 + k * d0.0,
            p0.1 + k * d0.1,
            p3.0 - k * d1.0,
            p3.1 - k * d1.1,
            p3.0,
            p3.1,
        );
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ArcKind {
    Arc,
    Chord,
    Pie,
}

/// The GDI Arc/Chord/Pie of the ellipse in a box, from the radial through `s` to the radial
/// through `e`, counter-clockwise on the page unless `clockwise`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn gdi_arc(
    kind: ArcKind,
    (l, t, r, b): (f32, f32, f32, f32),
    s: (f32, f32),
    e: (f32, f32),
    clockwise: bool,
    lead_in: Option<(f32, f32)>,
) -> Option<sk::Path> {
    let (cx, cy) = ((l + r) / 2.0, (t + b) / 2.0);
    let (rx, ry) = (((r - l) / 2.0).abs(), ((b - t) / 2.0).abs());
    if rx <= 0.0 || ry <= 0.0 {
        return None;
    }
    let angle = |p: (f32, f32)| ((p.1 - cy) / ry).atan2((p.0 - cx) / rx);
    let (a0, a1) = (angle(s), angle(e));
    let mut sweep = if clockwise {
        (a1 - a0).rem_euclid(TAU)
    } else {
        -(a0 - a1).rem_euclid(TAU)
    };
    if sweep.abs() < 1e-6 {
        sweep = if clockwise { TAU } else { -TAU };
    }
    let start = (cx + rx * a0.cos(), cy + ry * a0.sin());
    let mut pb = sk::PathBuilder::new();
    match lead_in {
        Some(p) => {
            pb.move_to(p.0, p.1);
            pb.line_to(start.0, start.1);
        }
        None => pb.move_to(start.0, start.1),
    }
    arc_to(&mut pb, cx, cy, rx, ry, a0, sweep);
    match kind {
        ArcKind::Arc => {}
        ArcKind::Chord => pb.close(),
        ArcKind::Pie => {
            pb.line_to(cx, cy);
            pb.close();
        }
    }
    pb.finish()
}

/// The end point of [`gdi_arc`] (where ArcTo leaves the current position).
pub(crate) fn arc_end((l, t, r, b): (f32, f32, f32, f32), e: (f32, f32)) -> (f32, f32) {
    let (cx, cy) = ((l + r) / 2.0, (t + b) / 2.0);
    let (rx, ry) = (((r - l) / 2.0).abs(), ((b - t) / 2.0).abs());
    let a = ((e.1 - cy) / ry.max(1e-9)).atan2((e.0 - cx) / rx.max(1e-9));
    (cx + rx * a.cos(), cy + ry * a.sin())
}

// ---------------------------------------------------------------------------------------------
// The GDI device context both WMF and EMF play into.

#[derive(Clone, Debug)]
pub(crate) enum Brush {
    Null,
    Solid(sk::Color),
    /// Hatch lines, drawn as a tint of their colour.
    Hatch(sk::Color),
}

#[derive(Clone, Debug)]
pub(crate) struct GdiPen {
    /// PS_* style bits (pen style, end cap, join, geometric).
    pub style: u32,
    /// In logical units; 0 is one device pixel.
    pub width: f32,
    pub color: sk::Color,
}

impl GdiPen {
    pub fn black() -> GdiPen {
        GdiPen {
            style: 0,
            width: 0.0,
            color: sk::Color::BLACK,
        }
    }
    pub fn null() -> GdiPen {
        GdiPen {
            style: 5,
            width: 0.0,
            color: sk::Color::BLACK,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Dc {
    pub pen: GdiPen,
    pub brush: Brush,
    pub even_odd: bool,
    pub clockwise: bool,
    /// The current position, in logical units.
    pub cur: (f32, f32),
    pub map_mode: u32,
    pub win_org: (f32, f32),
    pub win_ext: (f32, f32),
    pub vp_org: (f32, f32),
    pub vp_ext: (f32, f32),
    pub world: sk::Transform,
    /// Logical units → op space, kept current by the player.
    pub xf: sk::Transform,
}

impl Default for Dc {
    fn default() -> Dc {
        Dc {
            pen: GdiPen::black(),
            brush: Brush::Solid(sk::Color::WHITE),
            even_odd: true,
            clockwise: false,
            cur: (0.0, 0.0),
            map_mode: 1,
            win_org: (0.0, 0.0),
            win_ext: (1.0, 1.0),
            vp_org: (0.0, 0.0),
            vp_ext: (1.0, 1.0),
            world: sk::Transform::identity(),
            xf: sk::Transform::identity(),
        }
    }
}

pub(crate) struct Gdi {
    pub dc: Dc,
    stack: Vec<Dc>,
    /// An open path bracket (BeginPath), in op space, and whether a figure is open in it.
    bracket: Option<(sk::PathBuilder, bool)>,
    /// The path closed by EndPath, waiting for FillPath/StrokePath.
    path: Option<sk::Path>,
    pub list: List,
    /// Drawing records draw (EMF+ files play their GDI records only after GetDC).
    pub draw: bool,
}

impl Gdi {
    pub fn new() -> Gdi {
        Gdi {
            dc: Dc::default(),
            stack: Vec::new(),
            bracket: None,
            path: None,
            list: List::default(),
            draw: true,
        }
    }

    pub fn save(&mut self) {
        if self.stack.len() < MAX_SAVE {
            self.stack.push(self.dc.clone());
        }
    }

    /// RestoreDC: `n` < 0 is relative to the top, `n` > 0 a level SaveDC returned.
    pub fn restore(&mut self, n: i32) {
        let len = self.stack.len() as i64;
        let at = if n < 0 { len + n as i64 } else { n as i64 - 1 };
        if (0..len).contains(&at) {
            self.dc = self.stack[at as usize].clone();
            self.stack.truncate(at as usize);
        }
    }

    fn t(&self, p: (f32, f32)) -> (f32, f32) {
        let mut q = sk::Point::from_xy(p.0, p.1);
        self.dc.xf.map_point(&mut q);
        (q.x, q.y)
    }

    fn pen(&self) -> Option<Pen> {
        let p = &self.dc.pen;
        if p.style & 0xf == 5 {
            return None;
        }
        // GDI never draws thinner than a device pixel.
        let width = (p.width * scale_of(&self.dc.xf)).max(1.0);
        let unit = width;
        let dash = match p.style & 0xf {
            1 => Some(vec![18.0, 6.0]),
            2 => Some(vec![3.0, 3.0]),
            3 => Some(vec![9.0, 6.0, 3.0, 6.0]),
            4 => Some(vec![9.0, 3.0, 3.0, 3.0, 3.0, 3.0]),
            _ => None,
        }
        .map(|d: Vec<f32>| d.into_iter().map(|v| v * unit).collect());
        let geometric = p.style & 0x10000 != 0 || p.width > 1.0;
        let (cap, join) = if geometric {
            (
                match p.style & 0xf00 {
                    0x100 => sk::LineCap::Square,
                    0x200 => sk::LineCap::Butt,
                    _ => sk::LineCap::Round,
                },
                match p.style & 0xf000 {
                    0x1000 => sk::LineJoin::Bevel,
                    0x2000 => sk::LineJoin::Miter,
                    _ => sk::LineJoin::Round,
                },
            )
        } else {
            (sk::LineCap::Butt, sk::LineJoin::Miter)
        };
        Some(Pen {
            color: p.color,
            width,
            cap,
            join,
            miter: 10.0,
            dash,
        })
    }

    fn paint(&self) -> Option<Paint> {
        match self.dc.brush {
            Brush::Null => None,
            Brush::Solid(c) => Some(Paint::Solid(c)),
            Brush::Hatch(mut c) => {
                c.apply_opacity(0.5);
                Some(Paint::Solid(c))
            }
        }
    }

    /// A shape in logical units: into the open path bracket, or filled with the brush (when
    /// `fill`) and outlined with the pen.
    pub fn shape(&mut self, p: Option<sk::Path>, fill: bool) {
        let Some(p) = p.and_then(|p| p.transform(self.dc.xf)) else {
            return;
        };
        if let Some((pb, open)) = &mut self.bracket {
            pb.push_path(&p);
            *open = false;
            return;
        }
        self.emit(p, fill, true);
    }

    fn emit(&mut self, p: sk::Path, fill: bool, stroke: bool) {
        if !self.draw {
            return;
        }
        if fill && let Some(paint) = self.paint() {
            self.list.fill(p.clone(), paint, self.dc.even_odd);
        }
        if stroke && let Some(pen) = self.pen() {
            self.list.stroke(p, pen);
        }
    }

    pub fn move_to(&mut self, x: f32, y: f32) {
        self.dc.cur = (x, y);
        let q = self.t((x, y));
        if let Some((pb, open)) = &mut self.bracket {
            pb.move_to(q.0, q.1);
            *open = true;
        }
    }

    /// LineTo / PolylineTo / PolyBezierTo from the current position.
    pub fn line_to(&mut self, pts: &[(f32, f32)], bezier: bool) {
        let Some(&last) = pts.last() else {
            return;
        };
        let start = self.dc.cur;
        let tp: Vec<(f32, f32)> = pts.iter().map(|&p| self.t(p)).collect();
        let ts = self.t(start);
        self.dc.cur = last;
        let add = |pb: &mut sk::PathBuilder| {
            if bezier {
                for c in tp.chunks_exact(3) {
                    pb.cubic_to(c[0].0, c[0].1, c[1].0, c[1].1, c[2].0, c[2].1);
                }
            } else {
                for p in &tp {
                    pb.line_to(p.0, p.1);
                }
            }
        };
        if let Some((pb, open)) = &mut self.bracket {
            if !*open {
                pb.move_to(ts.0, ts.1);
                *open = true;
            }
            add(pb);
            return;
        }
        let mut pb = sk::PathBuilder::new();
        pb.move_to(ts.0, ts.1);
        add(&mut pb);
        if let Some(p) = pb.finish() {
            self.emit(p, false, true);
        }
    }

    pub fn begin_path(&mut self) {
        self.bracket = Some((sk::PathBuilder::new(), false));
        self.path = None;
    }
    pub fn end_path(&mut self) {
        if let Some((pb, _)) = self.bracket.take() {
            self.path = pb.finish();
        }
    }
    pub fn close_figure(&mut self) {
        if let Some((pb, open)) = &mut self.bracket {
            pb.close();
            *open = false;
        }
    }
    pub fn abort_path(&mut self) {
        self.bracket = None;
        self.path = None;
    }
    /// FillPath / StrokePath / StrokeAndFillPath.
    pub fn draw_path(&mut self, fill: bool, stroke: bool) {
        self.end_path();
        if let Some(p) = self.path.take() {
            self.emit(p, fill, stroke);
        }
    }

    /// A bitmap stretched over the logical rectangle `dest` (x, y, w, h); `src` is the part of
    /// the bitmap used (x, y from the top, w, h).
    pub fn bitmap(
        &mut self,
        img: &specular_bitmap::Image,
        src: (f32, f32, f32, f32),
        dest: (f32, f32, f32, f32),
        opacity: f32,
        key: Option<sk::Color>,
    ) {
        if !self.draw {
            return;
        }
        let Some((w, h, rgba)) = crop_rgba(img, src, key) else {
            self.list.notes.skipped_images += 1;
            return;
        };
        let Some(pm) = self.list.pixmap(w, h, &rgba) else {
            return;
        };
        let xf = self.dc.xf.pre_concat(sk::Transform::from_row(
            dest.2 / w as f32,
            0.0,
            0.0,
            dest.3 / h as f32,
            dest.0,
            dest.1,
        ));
        self.list.image(pm, xf, opacity);
    }

    /// PatBlt and the bitmap-less blits: the brush, black or white over a logical rectangle.
    pub fn pat_blt(&mut self, (x, y, w, h): (f32, f32, f32, f32), rop: u32) {
        let paint = match rop {
            0x00f0_0021 => self.paint(),
            0x0000_0042 => Some(Paint::Solid(sk::Color::BLACK)),
            0x00ff_0062 => Some(Paint::Solid(sk::Color::WHITE)),
            _ => None,
        };
        if let (true, Some(paint), Some(p)) = (
            self.draw,
            paint,
            rect(x, y, x + w, y + h).and_then(|p| p.transform(self.dc.xf)),
        ) {
            self.list.fill(p, paint, false);
        }
    }
}

/// Decode a DIB (BITMAPINFO, colour table and pixels); `pixels` is where the pixels start when
/// they do not follow the colour table directly.
pub(crate) fn dib(data: &[u8], pixels: Option<usize>) -> Option<specular_bitmap::Image> {
    use specular_bitmap::Decode;
    let bmp = specular_bitmap::bmp::Bmp::parse_packed_dib(data, pixels).ok()?;
    let i = bmp.info();
    if i.width as u64 * i.height as u64 > MAX_IMAGE_PIXELS {
        return None;
    }
    bmp.decode(data, &specular_bitmap::Options::default(), &|| false)
        .ok()
}

/// Whether a DIB's rows run bottom-up (positive height), which puts StretchDIBits' source origin
/// at the bottom left.
pub(crate) fn dib_bottom_up(data: &[u8]) -> bool {
    match data
        .get(..4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    {
        Some(12) => data
            .get(6..8)
            .is_some_and(|b| i16::from_le_bytes([b[0], b[1]]) > 0),
        Some(_) => data
            .get(8..12)
            .is_some_and(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]) > 0),
        None => false,
    }
}

/// The RGBA pixels of `src` (x, y from the top, w, h; clamped to the image), with `key`
/// made transparent.
fn crop_rgba(
    img: &specular_bitmap::Image,
    (sx, sy, sw, sh): (f32, f32, f32, f32),
    key: Option<sk::Color>,
) -> Option<(u32, u32, Vec<u8>)> {
    let (iw, ih) = (img.width, img.height);
    let ch = match img.format {
        specular_bitmap::PixelFormat::Gray8 => 1,
        specular_bitmap::PixelFormat::Rgba8 => 4,
        _ => return None,
    };
    let (mut x0, mut y0, mut x1, mut y1) = (sx, sy, sx + sw, sy + sh);
    if x1 < x0 {
        std::mem::swap(&mut x0, &mut x1);
    }
    if y1 < y0 {
        std::mem::swap(&mut y0, &mut y1);
    }
    let clamp = |v: f32, m: u32| (v.round().max(0.0) as u64).min(m as u64) as u32;
    let (mut x0, mut y0, mut x1, mut y1) =
        (clamp(x0, iw), clamp(y0, ih), clamp(x1, iw), clamp(y1, ih));
    if x1 <= x0 || y1 <= y0 {
        // A degenerate source rectangle: the whole bitmap.
        (x0, y0, x1, y1) = (0, 0, iw, ih);
    }
    let (w, h) = (x1 - x0, y1 - y0);
    if w == 0 || h == 0 || img.data.len() < iw as usize * ih as usize * ch {
        return None;
    }
    let key = key.map(|c| c.to_color_u8());
    let mut out = Vec::with_capacity(w as usize * h as usize * 4);
    for y in y0..y1 {
        let row = &img.data[(y as usize * iw as usize + x0 as usize) * ch..][..w as usize * ch];
        for p in row.chunks_exact(ch) {
            let px = if ch == 1 {
                [p[0], p[0], p[0], 255]
            } else {
                [p[0], p[1], p[2], p[3]]
            };
            let keyed =
                key.is_some_and(|k| (k.red(), k.green(), k.blue()) == (px[0], px[1], px[2]));
            out.extend_from_slice(if keyed { &[0, 0, 0, 0] } else { &px });
        }
    }
    Some((w, h, out))
}

/// The average colour of a bitmap (pattern brushes are drawn flat).
pub(crate) fn average(img: &specular_bitmap::Image) -> Option<sk::Color> {
    let (w, h) = (img.width as usize, img.height as usize);
    let ch = match img.format {
        specular_bitmap::PixelFormat::Gray8 => 1,
        specular_bitmap::PixelFormat::Rgba8 => 4,
        _ => return None,
    };
    let n = w * h;
    if n == 0 || img.data.len() < n * ch {
        return None;
    }
    let mut s = [0u64; 4];
    for p in img.data[..n * ch].chunks_exact(ch) {
        for (i, v) in s.iter_mut().enumerate() {
            *v += if ch == 1 {
                if i == 3 { 255 } else { p[0] as u64 }
            } else {
                p[i] as u64
            };
        }
    }
    let a = |i: usize| (s[i] / n as u64) as u8;
    Some(sk::Color::from_rgba8(a(0), a(1), a(2), a(3)))
}

// ---------------------------------------------------------------------------------------------
// Little-endian readers that never panic.

pub(crate) fn u16le(d: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        d.get(at..at.checked_add(2)?)?.try_into().ok()?,
    ))
}
pub(crate) fn i16le(d: &[u8], at: usize) -> Option<i16> {
    u16le(d, at).map(|v| v as i16)
}
pub(crate) fn u32le(d: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        d.get(at..at.checked_add(4)?)?.try_into().ok()?,
    ))
}
pub(crate) fn i32le(d: &[u8], at: usize) -> Option<i32> {
    u32le(d, at).map(|v| v as i32)
}
pub(crate) fn f32le(d: &[u8], at: usize) -> Option<f32> {
    u32le(d, at).map(f32::from_bits).filter(|v| v.is_finite())
}

// ---------------------------------------------------------------------------------------------
// Decoders and the session.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Kind {
    Wmf,
    Emf,
}

pub struct MetafileDecoder(pub(crate) Kind);

pub(crate) fn probe_wmf(h: &[u8]) -> bool {
    h.starts_with(&wmf::PLACEABLE_KEY)
        || h.len() >= 6
            && matches!(u16le(h, 0), Some(1 | 2))
            && u16le(h, 2) == Some(9)
            && matches!(u16le(h, 4), Some(0x100 | 0x300))
}

pub(crate) fn probe_emf(h: &[u8]) -> bool {
    u32le(h, 0) == Some(1) && h.get(40..44) == Some(b" EMF")
}

impl Decoder for MetafileDecoder {
    fn format(&self) -> Format {
        match self.0 {
            Kind::Wmf => Format::Wmf,
            Kind::Emf => Format::Emf,
        }
    }
    fn probe(&self, head: &[u8]) -> bool {
        match self.0 {
            Kind::Wmf => probe_wmf(head),
            Kind::Emf => probe_emf(head),
        }
    }
    /// `.wmz` and `.emz` are the metafiles gzip-compressed (as Office and LibreOffice save
    /// them): recognised by extension, as gzip says nothing of what it holds.
    fn extensions(&self) -> &'static [&'static str] {
        match self.0 {
            Kind::Wmf => &["wmf", "wmz"],
            Kind::Emf => &["emf", "emz"],
        }
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let cancel = CancelToken::new();
        let bytes = source.bytes();
        let gzip = bytes.starts_with(&[0x1F, 0x8B]);
        let inflated;
        let data = if gzip {
            inflated = specular_png::inflate_gzip(bytes, MAX_COMPRESSED_OUT).map_err(|_| {
                DecodeError::Malformed(format!(
                    "the compressed {} is damaged or unpacks to over {} MB",
                    if self.0 == Kind::Wmf { "WMF" } else { "EMF" },
                    MAX_COMPRESSED_OUT >> 20
                ))
            })?;
            let fits = match self.0 {
                Kind::Wmf => probe_wmf(&inflated),
                Kind::Emf => probe_emf(&inflated),
            };
            if !fits {
                let other = if probe_emf(&inflated) {
                    Some(".emz")
                } else if probe_wmf(&inflated) {
                    Some(".wmz")
                } else {
                    None
                };
                return Err(DecodeError::Unsupported(match other {
                    Some(e) => {
                        format!("this compressed metafile holds the other kind: name it {e}")
                    }
                    None => "the gzip file does not hold a Windows metafile".into(),
                }));
            }
            &inflated[..]
        } else {
            bytes
        };
        let (pic, kind) = match self.0 {
            Kind::Wmf => wmf::parse(data, &cancel)?,
            Kind::Emf => emf::parse(data, &cancel)?,
        };
        let (iw, ih) = (pic.width, pic.height);
        let mut k = 1.0f32;
        while iw.max(ih) * k < MIN_LONG_SIDE {
            k *= 2.0;
        }
        k = k
            .min(MAX_SIDE / iw.max(ih))
            .min((MAX_PIXELS / (iw * ih)).sqrt());
        let (w, h) = (
            ((iw * k).round() as u32).max(1),
            ((ih * k).round() as u32).max(1),
        );
        let mut coding = format!("{kind}, vector {iw:.0}×{ih:.0}, rendered ×{k:.3}");
        if gzip {
            coding.insert_str(0, "gzip-compressed ");
        }
        let n = &pic.notes;
        if n.text {
            coding.push_str(", text not shown");
        }
        if n.clip {
            coding.push_str(", clipping ignored");
        }
        if n.skipped_images > 0 {
            coding.push_str(&format!(", {} bitmaps skipped", n.skipped_images));
        }
        let info = ImageInfo {
            format: self.format(),
            width: w,
            height: h,
            orientation: Orientation::NORMAL,
            icc_profile: None,
            has_alpha: true,
            bit_depth: 8,
            progressive: false,
            coding,
            embedded_preview: None,
            fast_regions: true,
            scales: &[1, 2, 4, 8],
        };
        Ok(Arc::new(MetafileSession { source, info, pic }))
    }
}

struct MetafileSession {
    source: Arc<Source>,
    info: ImageInfo,
    pic: Picture,
}

impl MetafileSession {
    fn render(&self, pm: &mut sk::Pixmap, view: sk::Transform, cancel: &CancelToken) -> Result<()> {
        let t = view.pre_concat(self.pic.base);
        for (i, op) in self.pic.ops.iter().enumerate() {
            if i.is_multiple_of(256) && cancel.is_cancelled() {
                return Err(DecodeError::Cancelled);
            }
            match op {
                Op::Fill {
                    path,
                    paint,
                    even_odd,
                } => {
                    let Some(p) = sk_paint(paint) else { continue };
                    let rule = if *even_odd {
                        sk::FillRule::EvenOdd
                    } else {
                        sk::FillRule::Winding
                    };
                    if sane(path, t, 0.0) {
                        pm.fill_path(path, &p, rule, t, None);
                    }
                }
                Op::Stroke { path, pen } => {
                    let Some(p) = sk_paint(&Paint::Solid(pen.color)) else {
                        continue;
                    };
                    let stroke = sk::Stroke {
                        width: pen.width,
                        miter_limit: pen.miter.max(1.0),
                        line_cap: pen.cap,
                        line_join: pen.join,
                        dash: pen.dash.clone().and_then(|d| sk::StrokeDash::new(d, 0.0)),
                    };
                    if sane(path, t, pen.width) {
                        pm.stroke_path(path, &p, &stroke, t, None);
                    }
                }
                Op::Image {
                    pixmap,
                    xf,
                    opacity,
                } => {
                    let paint = sk::PixmapPaint {
                        opacity: *opacity,
                        blend_mode: sk::BlendMode::SourceOver,
                        quality: sk::FilterQuality::Bilinear,
                    };
                    pm.draw_pixmap(
                        0,
                        0,
                        sk::Pixmap::as_ref(pixmap),
                        &paint,
                        t.pre_concat(*xf),
                        None,
                    );
                }
            }
        }
        Ok(())
    }
}

/// Whether a path lands within a range the rasteriser handles: hostile coordinates millions of
/// pixels away trip its edge walker, and no real drawing needs them.
fn sane(path: &sk::Path, t: sk::Transform, width: f32) -> bool {
    const LIMIT: f32 = 1.0e6;
    let w = width * t.sx.abs().max(t.sy.abs()).max(t.kx.abs()).max(t.ky.abs());
    path.clone().transform(t).is_some_and(|p| {
        let b = p.bounds();
        w.is_finite()
            && w < LIMIT
            && [b.left(), b.top(), b.right(), b.bottom()]
                .iter()
                .all(|v| v.abs() < LIMIT)
    })
}

fn sk_paint(p: &Paint) -> Option<sk::Paint<'static>> {
    let mut paint = sk::Paint {
        anti_alias: true,
        ..Default::default()
    };
    match *p {
        Paint::Solid(c) => paint.set_color(c),
        Paint::Linear { p0, p1, c0, c1 } => {
            paint.shader = sk::LinearGradient::new(
                p0,
                p1,
                vec![
                    sk::GradientStop::new(0.0, c0),
                    sk::GradientStop::new(1.0, c1),
                ],
                sk::SpreadMode::Pad,
                sk::Transform::identity(),
            )
            .unwrap_or(sk::Shader::SolidColor(c0));
        }
    }
    Some(paint)
}

impl Session for MetafileSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    /// Vector art stores no samples: transparency is not a property of the encoding.
    fn alpha(&self) -> Option<crate::Alpha> {
        None
    }
    fn source(&self) -> &Arc<Source> {
        &self.source
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        None
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        if cancel.is_cancelled() {
            return Err(DecodeError::Cancelled);
        }
        let scale = req.scale.clamp(1, 8);
        let (sw, sh) = (
            self.info.width.div_ceil(scale),
            self.info.height.div_ceil(scale),
        );
        let (x, y, w, h) = match req.region {
            Some(r) => {
                let x = r.x.min(sw);
                let y = r.y.min(sh);
                (x, y, r.w.min(sw - x), r.h.min(sh - y))
            }
            None => (0, 0, sw, sh),
        };
        if w == 0 || h == 0 {
            return Err(DecodeError::Unsupported("empty region".into()));
        }
        let mut pm = sk::Pixmap::new(w, h).ok_or(DecodeError::TooLarge)?;
        let (fx, fy) = (sw as f32 / self.pic.width, sh as f32 / self.pic.height);
        let view = sk::Transform::from_row(fx, 0.0, 0.0, fy, -(x as f32), -(y as f32));
        self.render(&mut pm, view, cancel)?;
        let mut data = pm.take();
        // Premultiplied → straight alpha.
        for p in data.chunks_exact_mut(4) {
            let a = p[3] as u32;
            if a != 0 && a != 255 {
                for c in &mut p[..3] {
                    *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
                }
            }
        }
        Ok(Raster {
            width: w,
            height: h,
            origin: (x, y),
            scale,
            layout: PixelLayout::Rgba,
            planes: vec![RasterPlane {
                width: w,
                height: h,
                stride: w as usize * 4,
                data,
                h_div: 1,
                v_div: 1,
            }],
            truncated: false,
        })
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        let s = req.scale.clamp(1, 8);
        let px = match req.region {
            Some(r) => r.w as u64 * r.h as u64,
            None => self.info.width.div_ceil(s) as u64 * self.info.height.div_ceil(s) as u64,
        };
        px * 4 * 2
    }
    fn memory_bytes(&self) -> usize {
        (self.pic.image_bytes + self.pic.ops.len() as u64 * 64) as usize
    }
}

#[cfg(test)]
mod tests;
