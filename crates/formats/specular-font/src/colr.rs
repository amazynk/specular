//! `COLR` version 1 paint graphs, drawn with tiny-skia: solid fills, linear, radial and sweep
//! gradients, the affine paints (transform, translate, scale, rotate, skew, around a centre or
//! not), glyph clips, layer lists, reused colour glyphs and compositing (every mode of the
//! OpenType 1.9 `CompositeMode` list). Written from the OpenType specification
//! (<https://learn.microsoft.com/typography/opentype/spec/colr>).
//!
//! Not applied: font variations (the `Var` paints are drawn at their default values) and clip
//! boxes. Paint graphs are walked to a bounded depth and number of paints, so cycles and huge
//! graphs stop.

use tiny_skia::{
    BlendMode, Color, FillRule, GradientStop, LinearGradient, Mask, Paint, PathBuilder, Pixmap,
    PixmapPaint, Point, RadialGradient, Rect, Shader, SpreadMode, SweepGradient, Transform,
};

use crate::face::Face;
use crate::outline::{Outline, PathEl};

/// Deepest paint nesting followed.
const MAX_DEPTH: u32 = 48;
/// Most paints drawn for one glyph.
const MAX_PAINTS: u32 = 4096;
/// Largest offscreen layer (pixels) for a composite.
const MAX_LAYER_PIXELS: u64 = 1 << 24;

/// A face's `COLR` version 1 tables.
pub(crate) struct ColrV1<'a> {
    t: &'a [u8],
    base_list: usize,
    layer_list: usize,
}

fn u8_at(b: &[u8], at: usize) -> Option<u8> {
    b.get(at).copied()
}

fn u16_at(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes([
        *b.get(at)?,
        *b.get(at.checked_add(1)?)?,
    ]))
}

fn i16_at(b: &[u8], at: usize) -> Option<f32> {
    u16_at(b, at).map(|v| f32::from(v as i16))
}

fn u24_at(b: &[u8], at: usize) -> Option<usize> {
    let s = b.get(at..at.checked_add(3)?)?;
    Some(usize::from(s[0]) << 16 | usize::from(s[1]) << 8 | usize::from(s[2]))
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(
        b.get(at..at.checked_add(4)?)?.try_into().ok()?,
    ))
}

/// F2DOT14.
fn f2dot14(b: &[u8], at: usize) -> Option<f32> {
    u16_at(b, at).map(|v| f32::from(v as i16) / 16384.0)
}

/// Fixed 16.16.
fn fixed(b: &[u8], at: usize) -> Option<f32> {
    u32_at(b, at).map(|v| v as i32 as f32 / 65536.0)
}

/// An offset relative to `base`, or `None` for 0.
fn rel(base: usize, off: usize) -> Option<usize> {
    (off != 0).then(|| base.checked_add(off)).flatten()
}

impl<'a> ColrV1<'a> {
    pub(crate) fn of(face: &'a Face) -> Option<ColrV1<'a>> {
        let t = face.table(b"COLR")?;
        if u16_at(t, 0)? < 1 {
            return None;
        }
        let base_list = u32_at(t, 14)? as usize;
        if base_list == 0 {
            return None;
        }
        Some(ColrV1 {
            t,
            base_list,
            layer_list: u32_at(t, 18).unwrap_or(0) as usize,
        })
    }

    /// The root paint of base glyph `g`.
    pub(crate) fn root(&self, g: u16) -> Option<usize> {
        let n = u32_at(self.t, self.base_list)? as usize;
        // Records are sorted by glyph id: binary search.
        let (mut lo, mut hi) = (0usize, n);
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let at = self.base_list + 4 + mid * 6;
            let id = u16_at(self.t, at)?;
            match id.cmp(&g) {
                std::cmp::Ordering::Equal => {
                    return rel(self.base_list, u32_at(self.t, at + 2)? as usize);
                }
                std::cmp::Ordering::Less => lo = mid + 1,
                std::cmp::Ordering::Greater => hi = mid,
            }
        }
        None
    }

    fn layer(&self, i: usize) -> Option<usize> {
        if self.layer_list == 0 {
            return None;
        }
        let n = u32_at(self.t, self.layer_list)? as usize;
        if i >= n {
            return None;
        }
        rel(
            self.layer_list,
            u32_at(self.t, self.layer_list + 4 + 4 * i)? as usize,
        )
    }
}

/// What paints draw with: the face (outlines), the palette, the text colour.
pub(crate) struct Ctx<'a> {
    pub face: &'a Face,
    pub colr: &'a ColrV1<'a>,
    pub palette: Option<&'a [[u8; 4]]>,
    pub foreground: [u8; 4],
}

struct Walk<'c, 'a> {
    ctx: &'c Ctx<'a>,
    paints: u32,
}

/// Draws base glyph `g`'s paint graph into `pm`: font units through `to_device` (which
/// includes the flip to y down). `false` if `g` has no paint graph.
pub(crate) fn draw(ctx: &Ctx<'_>, g: u16, pm: &mut Pixmap, to_device: Transform) -> bool {
    let Some(root) = ctx.colr.root(g) else {
        return false;
    };
    let mut w = Walk { ctx, paints: 0 };
    w.paint(root, pm, to_device, None, 0);
    true
}

/// A glyph outline as a tiny-skia path in font units.
fn glyph_path(ol: &Outline) -> Option<tiny_skia::Path> {
    let mut pb = PathBuilder::new();
    for el in &ol.path {
        match *el {
            PathEl::MoveTo(x, y) => pb.move_to(x, y),
            PathEl::LineTo(x, y) => pb.line_to(x, y),
            PathEl::QuadTo(a, b, x, y) => pb.quad_to(a, b, x, y),
            PathEl::CubicTo(a, b, c, d, x, y) => pb.cubic_to(a, b, c, d, x, y),
            PathEl::Close => pb.close(),
        }
    }
    pb.finish()
}

fn colour(ctx: &Ctx<'_>, index: u16, alpha: f32) -> Color {
    let c = if index == 0xffff {
        ctx.foreground
    } else {
        ctx.palette
            .and_then(|p| p.get(usize::from(index)))
            .copied()
            .unwrap_or([0, 0, 0, 255])
    };
    let a = (f32::from(c[3]) / 255.0 * alpha).clamp(0.0, 1.0);
    Color::from_rgba(
        f32::from(c[0]) / 255.0,
        f32::from(c[1]) / 255.0,
        f32::from(c[2]) / 255.0,
        a,
    )
    .unwrap_or(Color::BLACK)
}

/// A colour line: its stops (offset, colour) sorted, and the spread mode.
fn color_line(
    ctx: &Ctx<'_>,
    t: &[u8],
    at: usize,
    var: bool,
) -> Option<(Vec<(f32, Color)>, SpreadMode)> {
    let mode = match u8_at(t, at)? {
        1 => SpreadMode::Repeat,
        2 => SpreadMode::Reflect,
        _ => SpreadMode::Pad,
    };
    let n = usize::from(u16_at(t, at + 1)?);
    let size = if var { 10 } else { 6 };
    let mut stops = Vec::with_capacity(n.min(256));
    for i in 0..n.min(256) {
        let s = at + 3 + i * size;
        let off = f2dot14(t, s)?;
        let c = colour(ctx, u16_at(t, s + 2)?, f2dot14(t, s + 4)?);
        stops.push((off, c));
    }
    stops.sort_by(|a, b| a.0.total_cmp(&b.0));
    (!stops.is_empty()).then_some((stops, mode))
}

/// Stops brought to 0..1: the first and last offsets (the colour line's own range, which may
/// lie outside 0..1) and the stops rescaled into it.
fn normalised(stops: &[(f32, Color)]) -> (f32, f32, Vec<GradientStop>) {
    let (a, b) = (stops[0].0, stops[stops.len() - 1].0);
    let span = b - a;
    let s = stops
        .iter()
        .map(|&(o, c)| {
            let p = if span > 1e-6 { (o - a) / span } else { 0.0 };
            GradientStop::new(p.clamp(0.0, 1.0), c)
        })
        .collect();
    (a, b, s)
}

fn lerp(p: Point, q: Point, t: f32) -> Point {
    Point::from_xy(p.x + (q.x - p.x) * t, p.y + (q.y - p.y) * t)
}

fn blend(mode: u8) -> Option<BlendMode> {
    use BlendMode as B;
    Some(match mode {
        0 => B::Clear,
        1 => B::Source,
        2 => B::Destination,
        3 => B::SourceOver,
        4 => B::DestinationOver,
        5 => B::SourceIn,
        6 => B::DestinationIn,
        7 => B::SourceOut,
        8 => B::DestinationOut,
        9 => B::SourceAtop,
        10 => B::DestinationAtop,
        11 => B::Xor,
        12 => B::Plus,
        13 => B::Screen,
        14 => B::Overlay,
        15 => B::Darken,
        16 => B::Lighten,
        17 => B::ColorDodge,
        18 => B::ColorBurn,
        19 => B::HardLight,
        20 => B::SoftLight,
        21 => B::Difference,
        22 => B::Exclusion,
        23 => B::Multiply,
        24 => B::Hue,
        25 => B::Saturation,
        26 => B::Color,
        27 => B::Luminosity,
        _ => return None,
    })
}

impl Walk<'_, '_> {
    /// Fills the clip (the glyph outlines met on the way down, already intersected into
    /// `clip`) with `shader`.
    fn fill(&self, pm: &mut Pixmap, shader: Shader<'_>, clip: Option<&Mask>) {
        let paint = Paint {
            shader,
            anti_alias: true,
            ..Paint::default()
        };
        if let Some(r) = Rect::from_xywh(0.0, 0.0, pm.width() as f32, pm.height() as f32) {
            pm.fill_rect(r, &paint, Transform::identity(), clip);
        }
    }

    fn paint(
        &mut self,
        at: usize,
        pm: &mut Pixmap,
        tr: Transform,
        clip: Option<&Mask>,
        depth: u32,
    ) {
        self.paints += 1;
        if depth > MAX_DEPTH || self.paints > MAX_PAINTS {
            return;
        }
        let ctx = self.ctx;
        let t = ctx.colr.t;
        let Some(format) = u8_at(t, at) else {
            return;
        };
        let child = |off_at: usize| u24_at(t, off_at).and_then(|o| rel(at, o));
        let next = depth + 1;
        match format {
            // PaintColrLayers.
            1 => {
                let (Some(n), Some(first)) = (u8_at(t, at + 1), u32_at(t, at + 2)) else {
                    return;
                };
                for i in 0..usize::from(n) {
                    if let Some(p) = ctx.colr.layer(first as usize + i) {
                        self.paint(p, pm, tr, clip, next);
                    }
                }
            }
            // PaintSolid, PaintVarSolid.
            2 | 3 => {
                if let (Some(i), Some(a)) = (u16_at(t, at + 1), f2dot14(t, at + 3)) {
                    self.fill(pm, Shader::SolidColor(colour(ctx, i, a)), clip);
                }
            }
            // PaintLinearGradient, PaintVarLinearGradient.
            4 | 5 => {
                let Some(line) = child(at + 1) else { return };
                let Some((stops, mode)) = color_line(ctx, t, line, format == 5) else {
                    return;
                };
                let v: Option<Vec<f32>> = (0..6).map(|i| i16_at(t, at + 4 + 2 * i)).collect();
                let Some(v) = v else { return };
                let (p0, p1, p2) = (
                    Point::from_xy(v[0], v[1]),
                    Point::from_xy(v[2], v[3]),
                    Point::from_xy(v[4], v[5]),
                );
                // The gradient runs along p0p1 projected onto the normal of p0p2.
                let n = Point::from_xy(p2.y - p0.y, -(p2.x - p0.x));
                let d = Point::from_xy(p1.x - p0.x, p1.y - p0.y);
                let nn = n.x * n.x + n.y * n.y;
                let p1 = if nn > 1e-9 {
                    let k = (d.x * n.x + d.y * n.y) / nn;
                    Point::from_xy(p0.x + n.x * k, p0.y + n.y * k)
                } else {
                    p1
                };
                let (a, b, gs) = normalised(&stops);
                let (q0, q1) = (lerp(p0, p1, a), lerp(p0, p1, b));
                if let Some(s) = LinearGradient::new(q0, q1, gs, mode, tr) {
                    self.fill(pm, s, clip);
                } else {
                    self.fill(pm, Shader::SolidColor(stops[stops.len() - 1].1), clip);
                }
            }
            // PaintRadialGradient, PaintVarRadialGradient.
            6 | 7 => {
                let Some(line) = child(at + 1) else { return };
                let Some((stops, mode)) = color_line(ctx, t, line, format == 7) else {
                    return;
                };
                let (Some(x0), Some(y0), Some(r0), Some(x1), Some(y1), Some(r1)) = (
                    i16_at(t, at + 4),
                    i16_at(t, at + 6),
                    u16_at(t, at + 8).map(f32::from),
                    i16_at(t, at + 10),
                    i16_at(t, at + 12),
                    u16_at(t, at + 14).map(f32::from),
                ) else {
                    return;
                };
                let (a, b, gs) = normalised(&stops);
                let (c0, c1) = (Point::from_xy(x0, y0), Point::from_xy(x1, y1));
                let (q0, q1) = (lerp(c0, c1, a), lerp(c0, c1, b));
                let (s0, s1) = (r0 + (r1 - r0) * a, r0 + (r1 - r0) * b);
                match RadialGradient::new(q0, s0.max(0.0), q1, s1.max(0.0), gs, mode, tr) {
                    Some(s) => self.fill(pm, s, clip),
                    None => self.fill(pm, Shader::SolidColor(stops[stops.len() - 1].1), clip),
                }
            }
            // PaintSweepGradient, PaintVarSweepGradient: angles in 180° per 1.0 (biased),
            // counter-clockwise in font space (y up), which the y flip of `tr` keeps.
            8 | 9 => {
                let Some(line) = child(at + 1) else { return };
                let Some((stops, mode)) = color_line(ctx, t, line, format == 9) else {
                    return;
                };
                let (Some(cx), Some(cy), Some(sa), Some(ea)) = (
                    i16_at(t, at + 4),
                    i16_at(t, at + 6),
                    f2dot14(t, at + 8),
                    f2dot14(t, at + 10),
                ) else {
                    return;
                };
                let (a, b, gs) = normalised(&stops);
                // Stored with a bias of 1.0, so that +360° fits an F2DOT14.
                let (sa, ea) = ((sa + 1.0) * 180.0, (ea + 1.0) * 180.0);
                let (s0, s1) = (sa + (ea - sa) * a, sa + (ea - sa) * b);
                let shader = if s1 > s0 {
                    SweepGradient::new(Point::from_xy(cx, cy), s0, s1, gs, mode, tr)
                } else {
                    None
                };
                match shader {
                    Some(s) => self.fill(pm, s, clip),
                    None => self.fill(pm, Shader::SolidColor(stops[stops.len() - 1].1), clip),
                }
            }
            // PaintGlyph: clip to the glyph's outline, then its paint.
            10 => {
                let (Some(p), Some(g)) = (child(at + 1), u16_at(t, at + 4)) else {
                    return;
                };
                let Some(path) = ctx.face.outliner().outline(g).and_then(|o| glyph_path(&o)) else {
                    return;
                };
                let mask = match clip {
                    Some(m) => {
                        let mut m = m.clone();
                        m.intersect_path(&path, FillRule::Winding, true, tr);
                        Some(m)
                    }
                    None => Mask::new(pm.width(), pm.height()).map(|mut m| {
                        m.fill_path(&path, FillRule::Winding, true, tr);
                        m
                    }),
                };
                if let Some(m) = mask {
                    self.paint(p, pm, tr, Some(&m), next);
                }
            }
            // PaintColrGlyph: another base glyph's graph.
            11 => {
                if let Some(r) = u16_at(t, at + 1).and_then(|g| ctx.colr.root(g)) {
                    self.paint(r, pm, tr, clip, next);
                }
            }
            // PaintTransform, PaintVarTransform.
            12 | 13 => {
                let (Some(p), Some(m)) = (child(at + 1), child(at + 4)) else {
                    return;
                };
                let v: Option<Vec<f32>> = (0..6).map(|i| fixed(t, m + 4 * i)).collect();
                if let Some(v) = v {
                    let a = Transform::from_row(v[0], v[1], v[2], v[3], v[4], v[5]);
                    self.paint(p, pm, tr.pre_concat(a), clip, next);
                }
            }
            // PaintTranslate, PaintVarTranslate.
            14 | 15 => {
                if let (Some(p), Some(dx), Some(dy)) =
                    (child(at + 1), i16_at(t, at + 4), i16_at(t, at + 6))
                {
                    self.paint(p, pm, tr.pre_translate(dx, dy), clip, next);
                }
            }
            // PaintScale (and around a centre, uniform, uniform around a centre), Var forms.
            16..=23 => {
                let Some(p) = child(at + 1) else { return };
                let uniform = matches!(format, 20..=23);
                let centred = matches!(format, 18 | 19 | 22 | 23);
                let Some(sx) = f2dot14(t, at + 4) else { return };
                let (sy, c_at) = if uniform {
                    (sx, at + 6)
                } else {
                    let Some(sy) = f2dot14(t, at + 6) else { return };
                    (sy, at + 8)
                };
                let a = Transform::from_scale(sx, sy);
                self.paint(p, pm, around(tr, a, centred, t, c_at), clip, next);
            }
            // PaintRotate (and around a centre), Var forms.
            24..=27 => {
                let (Some(p), Some(angle)) = (child(at + 1), f2dot14(t, at + 4)) else {
                    return;
                };
                let a = Transform::from_rotate(angle * 180.0);
                let centred = matches!(format, 26 | 27);
                self.paint(p, pm, around(tr, a, centred, t, at + 6), clip, next);
            }
            // PaintSkew (and around a centre), Var forms: x skew counter-clockwise, as the
            // specification's matrix [1 tan(-x); tan(y) 1] gives.
            28..=31 => {
                let (Some(p), Some(xs), Some(ys)) =
                    (child(at + 1), f2dot14(t, at + 4), f2dot14(t, at + 6))
                else {
                    return;
                };
                let (kx, ky) = (
                    (-xs * std::f32::consts::PI).tan(),
                    (ys * std::f32::consts::PI).tan(),
                );
                let a = Transform::from_row(1.0, ky, kx, 1.0, 0.0, 0.0);
                let centred = matches!(format, 30 | 31);
                self.paint(p, pm, around(tr, a, centred, t, at + 8), clip, next);
            }
            // PaintComposite: source over backdrop in its own layer, then onto the canvas.
            32 => {
                let (Some(src), Some(mode), Some(dst)) =
                    (child(at + 1), u8_at(t, at + 4), child(at + 5))
                else {
                    return;
                };
                let (w, h) = (pm.width(), pm.height());
                if u64::from(w) * u64::from(h) > MAX_LAYER_PIXELS {
                    return;
                }
                let (Some(mut back), Some(mut fore)) = (Pixmap::new(w, h), Pixmap::new(w, h))
                else {
                    return;
                };
                self.paint(dst, &mut back, tr, clip, next);
                self.paint(src, &mut fore, tr, clip, next);
                let mode = blend(mode).unwrap_or(BlendMode::SourceOver);
                let pp = PixmapPaint {
                    blend_mode: mode,
                    ..PixmapPaint::default()
                };
                back.draw_pixmap(0, 0, fore.as_ref(), &pp, Transform::identity(), None);
                pm.draw_pixmap(
                    0,
                    0,
                    back.as_ref(),
                    &PixmapPaint::default(),
                    Transform::identity(),
                    None,
                );
            }
            _ => {}
        }
    }
}

/// `tr` followed by `a`, about the centre stored at `c_at` when `centred`.
fn around(tr: Transform, a: Transform, centred: bool, t: &[u8], c_at: usize) -> Transform {
    if !centred {
        return tr.pre_concat(a);
    }
    let (Some(cx), Some(cy)) = (i16_at(t, c_at), i16_at(t, c_at + 2)) else {
        return tr.pre_concat(a);
    };
    tr.pre_translate(cx, cy)
        .pre_concat(a)
        .pre_translate(-cx, -cy)
}
