//! Krita layer trees composited: what `mergedimage.png` holds, rebuilt from the layers, for
//! documents without a usable merged image and for group layer pages; and transparency masks
//! applied to paint layer pages.
//!
//! The canvas is worked in 64-row bands (Krita's tile height), each in parallel: per band,
//! every node's pixels (a paint layer's tiles touching the band, decompressed there, over its
//! default pixel; a group's children composited into a band of its own) are made
//! premultiplied `f32` RGBA, their transparency masks multiplied in, and blended into the
//! band below with the node's opacity and composite op. A band never holds more than one
//! buffer per open group level, so a large canvas costs its output and a few bands.
//!
//! Blending follows Krita's ops through the formulas shared with the other layered formats
//! (`specular-blend`): `normal` is source-over; the others mix `B(s, d)` (straight colour,
//! whole colours for the non-separable ones) as `(1 - αs)·αd·d + (1 - αd)·αs·s +
//! αs·αd·B(s, d)` over the union alpha `αs + αd - αs·αd`, values in the stored coding
//! normalised to 0‥1 (floats as they are, add and dodge unclamped). `erase` removes the
//! layer's coverage, `behind` draws under what is there. "Inherit alpha" keeps the alpha
//! below and mixes `B` in by the layer's alpha.
//! A pass-through group composites its children into a copy of what is below it, faded back
//! by its opacity. The whole tree is drawn over the projection's background colour, as
//! Krita's merged image is. Composite ops not known are drawn as `normal`.
//!
//! Besides paint layers and groups: a filter layer is its filter applied to what is below it
//! (`kra_fx`), through its selection; a fill layer is its generator's colour through its
//! selection; a clone layer is its source node's projection at the clone's offset; vector
//! layers (their SVG) and file layers (the image they name) are drawn before compositing by
//! the caller's [`KraHost`]. Filter masks filter the layer's pixels through their selection,
//! bottom-most mask first. What is not drawn (other filters and generators, selection,
//! transform and colorize masks) is left out and listed by [`crate::Document::kra_notes`].
//! Animated layers are drawn at their first keyframe.

use std::borrow::Cow;
use std::sync::atomic::{AtomicBool, Ordering};

use rayon::prelude::*;

use crate::kra::{
    self, KraFilterSpec, KraImage, KraKind, KraLayer, KraNode, KraOutput, KraPixel, Tile,
    f16_to_f32, f32_to_f16, lzf_decompress, srgb16, unit16,
};
use crate::kra_fx::{Fx, Generator, Hsx, HsxOp, Params, params};
use specular_blend::Mode;

use crate::{Document, Error, MAX_CANVAS_PIXELS};

/// Rows of a band.
const BAND: usize = 64;
/// Layer and mask data held at once while compositing (inflated members), in bytes.
const MAX_HELD: u64 = 3 << 30;

/// A composite op of Krita's that the compositor applies: a blend mode of
/// [`specular_blend`] (composited over the union of the alphas), or one of the two ops that
/// change only alpha coverage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KraBlend {
    Blend(Mode),
    /// `erase`: removes the layer's coverage from what is below.
    Erase,
    /// `behind`: drawn under what is below.
    Behind,
    /// Krita's binary (logic) and modulo ops, per channel.
    Extra(Extra),
    /// `copy`: what is below moved towards the layer by its opacity (colour and alpha).
    Copy,
    /// `destination-in`: what is below kept where the layer is.
    DestinationIn,
    /// `destination-atop`: what is below drawn over the layer, inside the layer.
    DestinationAtop,
    /// Krita's hue, saturation, colour and lightness ops in one of its colour models.
    Hsx(Hsx, HsxOp),
}

/// Krita's binary ops (on the channels as 16-bit integers, which keeps 8-bit values exact:
/// an 8-bit `v` is `v·257`) and modulo ops, as Krita's manual describes them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Extra {
    And,
    Or,
    Xor,
    Nand,
    Nor,
    Xnor,
    /// `¬d ∨ s`.
    Implies,
    /// `d ∧ ¬s`.
    NotImplies,
    /// `d ∨ ¬s`.
    Converse,
    /// `¬d ∧ s`.
    NotConverse,
    /// The backdrop modulo the layer.
    Modulo,
    /// The backdrop divided by the layer, modulo 1.
    DivisiveModulo,
}

impl Extra {
    fn from_id(id: &str) -> Option<Extra> {
        Some(match id {
            "and" => Extra::And,
            "or" => Extra::Or,
            "xor" => Extra::Xor,
            "nand" => Extra::Nand,
            "nor" => Extra::Nor,
            "xnor" => Extra::Xnor,
            "implies" => Extra::Implies,
            "not_implies" => Extra::NotImplies,
            "converse" => Extra::Converse,
            "not_converse" => Extra::NotConverse,
            "modulo" => Extra::Modulo,
            "divisive_modulo" => Extra::DivisiveModulo,
            _ => return None,
        })
    }

    #[inline]
    fn mix(self, s: f32, d: f32) -> f32 {
        let int = |v: f32| (v.clamp(0.0, 1.0) * 65535.0).round() as u16;
        let back = |v: u16| f32::from(v) * (1.0 / 65535.0);
        let (a, b) = (int(s), int(d));
        const EPS: f32 = 1.0 / 65535.0;
        match self {
            Extra::And => back(a & b),
            Extra::Or => back(a | b),
            Extra::Xor => back(a ^ b),
            Extra::Nand => back(!(a & b)),
            Extra::Nor => back(!(a | b)),
            Extra::Xnor => back(!(a ^ b)),
            Extra::Implies => back(!b | a),
            Extra::NotImplies => back(b & !a),
            Extra::Converse => back(b | !a),
            Extra::NotConverse => back(!b & a),
            Extra::Modulo => d.max(0.0) % (s.max(0.0) + EPS),
            Extra::DivisiveModulo => (d.max(0.0) / (s.max(0.0) + EPS)).fract(),
        }
    }
}

impl KraBlend {
    pub const NORMAL: KraBlend = KraBlend::Blend(Mode::Normal);

    /// From Krita's composite op id; `None` for ops not applied.
    pub fn from_id(id: &str) -> Option<KraBlend> {
        Some(match id {
            "erase" => KraBlend::Erase,
            "behind" => KraBlend::Behind,
            "copy" => KraBlend::Copy,
            "destination-in" => KraBlend::DestinationIn,
            "destination-atop" => KraBlend::DestinationAtop,
            _ => match (Extra::from_id(id), Hsx::from_id(id)) {
                (Some(e), _) => KraBlend::Extra(e),
                (_, Some((m, op))) => KraBlend::Hsx(m, op),
                _ => KraBlend::Blend(Mode::from_krita(id)?),
            },
        })
    }
}

/// `B(s, d)` of a separable mode on straight, normalised values (source `s` over backdrop
/// `d`); `hdr` leaves the sums and quotients of float pixels unclamped.
#[inline]
fn mix(m: Mode, s: f32, d: f32, hdr: bool) -> f32 {
    match m {
        Mode::LinearDodge if hdr => s + d,
        Mode::ColorDodge if hdr => {
            if s >= 1.0 {
                1.0
            } else {
                d / (1.0 - s)
            }
        }
        Mode::SoftLightSqrt | Mode::SoftLight => specular_blend::blend1(m, d.max(0.0), s),
        _ => specular_blend::blend1(m, d, s),
    }
}

/// Blends premultiplied `src` (times `opacity`) into premultiplied `dst`.
fn blend(dst: &mut [f32], src: &[f32], opacity: f32, op: KraBlend, inherit: bool, hdr: bool) {
    match op {
        KraBlend::Erase => {
            for (d, s) in dst.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
                let k = 1.0 - (s[3] * opacity).clamp(0.0, 1.0);
                for c in d {
                    *c *= k;
                }
            }
        }
        KraBlend::Behind => {
            for (d, s) in dst.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
                let k = (1.0 - d[3]).max(0.0) * opacity;
                for c in 0..4 {
                    d[c] += s[c] * k;
                }
            }
        }
        KraBlend::Extra(e) => blend_with(dst, src, opacity, inherit, false, |s, d| {
            [0, 1, 2].map(|c| e.mix(s[c], d[c]))
        }),
        KraBlend::Hsx(m, op) => {
            blend_with(dst, src, opacity, inherit, false, |s, d| m.mix(op, s, d))
        }
        KraBlend::Copy => {
            for (d, s) in dst.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
                for c in 0..4 {
                    d[c] += (s[c] - d[c]) * opacity;
                }
            }
        }
        KraBlend::DestinationIn => {
            for (d, s) in dst.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
                let k = (s[3] * opacity).clamp(0.0, 1.0);
                for c in d {
                    *c *= k;
                }
            }
        }
        KraBlend::DestinationAtop => {
            for (d, s) in dst.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
                let sa = (s[3] * opacity).clamp(0.0, 1.0);
                let da = d[3];
                if sa <= 0.0 {
                    d.fill(0.0);
                    continue;
                }
                // Straight: the backdrop's colour over the layer's by the backdrop's alpha.
                let sc = [s[0] / s[3], s[1] / s[3], s[2] / s[3]];
                for c in 0..3 {
                    let dc = if da > 0.0 { d[c] / da } else { 0.0 };
                    d[c] = (sc[c] + (dc - sc[c]) * da) * sa;
                }
                d[3] = sa;
            }
        }
        KraBlend::Blend(Mode::Normal) => blend_with(dst, src, opacity, inherit, true, |s, _| s),
        KraBlend::Blend(m) if m.non_separable() => {
            blend_with(dst, src, opacity, inherit, false, |s, d| {
                specular_blend::blend3(m, d, s)
            })
        }
        KraBlend::Blend(m) => blend_with(dst, src, opacity, inherit, false, |s, d| {
            [0, 1, 2].map(|c| mix(m, s[c], d[c], hdr))
        }),
    }
}

#[inline(always)]
fn blend_with(
    dst: &mut [f32],
    src: &[f32],
    opacity: f32,
    inherit: bool,
    normal: bool,
    mix: impl Fn([f32; 3], [f32; 3]) -> [f32; 3],
) {
    for (d, s) in dst.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
        let sa = s[3] * opacity;
        if sa <= 0.0 {
            continue;
        }
        let da = d[3];
        if inherit {
            // Alpha kept: the straight colour below moves towards B by the source alpha.
            if da <= 0.0 {
                continue;
            }
            let (rs, rd) = (1.0 / s[3], 1.0 / da);
            let k = sa.min(1.0);
            let sc = [s[0] * rs, s[1] * rs, s[2] * rs];
            let dc = [d[0] * rd, d[1] * rd, d[2] * rd];
            let b = mix(sc, dc);
            for c in 0..3 {
                d[c] = (dc[c] + (b[c] - dc[c]) * k) * da;
            }
            continue;
        }
        if normal || da <= 0.0 {
            let k = 1.0 - sa;
            for c in 0..4 {
                d[c] = s[c] * opacity + d[c] * k;
            }
            continue;
        }
        let (rs, rd) = (1.0 / s[3], 1.0 / da);
        let (a, b) = ((1.0 - da) * sa, sa * da);
        let sc = [s[0] * rs, s[1] * rs, s[2] * rs];
        let dc = [d[0] * rd, d[1] * rd, d[2] * rd];
        let m = mix(sc, dc);
        for c in 0..3 {
            d[c] = (1.0 - sa) * d[c] + a * sc[c] + b * m[c];
        }
        d[3] = sa + da - b;
    }
}

/// Pixels of a paint layer or mask, rendered a band at a time.
struct Source<'a> {
    /// Sorted by canvas row of their top edge.
    tiles: Vec<Tile<'a>>,
    /// The layer origin on the canvas.
    x: i64,
    y: i64,
    /// `None`: an 8-bit selection (a mask).
    pixel: Option<KraPixel>,
    /// Premultiplied RGBA (a mask: its value in `[0]`) outside every tile.
    default: [f32; 4],
    /// Tallest tile.
    tall: i64,
}

/// A stored pixel as straight `f32` RGBA, grey replicated; non-finite values are 0.
#[inline]
fn to_f32(pixel: KraPixel, s: &[u8]) -> [f32; 4] {
    let u16le = |i: usize| f32::from(u16::from_le_bytes([s[i], s[i + 1]])) * (1.0 / 65535.0);
    let h = |i: usize| f16_to_f32(u16::from_le_bytes([s[i], s[i + 1]]));
    let f = |i: usize| f32::from_le_bytes([s[i], s[i + 1], s[i + 2], s[i + 3]]);
    let b = |i: usize| f32::from(s[i]) * (1.0 / 255.0);
    let v = match pixel {
        KraPixel::Rgba8 => [b(2), b(1), b(0), b(3)],
        KraPixel::Rgba16 => [u16le(4), u16le(2), u16le(0), u16le(6)],
        KraPixel::RgbaF16 => [h(0), h(2), h(4), h(6)],
        KraPixel::RgbaF32 => [f(0), f(4), f(8), f(12)],
        KraPixel::GrayA8 => [b(0), b(0), b(0), b(1)],
        KraPixel::GrayA16 => [u16le(0), u16le(0), u16le(0), u16le(2)],
        KraPixel::GrayAF16 => [h(0), h(0), h(0), h(2)],
        KraPixel::GrayAF32 => [f(0), f(0), f(0), f(4)],
    };
    v.map(|c| if c.is_finite() { c } else { 0.0 })
}

/// Straight to premultiplied, alpha clamped to 0‥1.
#[inline]
fn premul(v: [f32; 4]) -> [f32; 4] {
    let a = v[3].clamp(0.0, 1.0);
    [v[0] * a, v[1] * a, v[2] * a, a]
}

impl<'a> Source<'a> {
    fn new(
        data: &'a [u8],
        default: Option<&[u8]>,
        x: i64,
        y: i64,
        pixel: Option<KraPixel>,
    ) -> Result<(Source<'a>, bool), Error> {
        let ps = pixel.map_or(1, KraPixel::size);
        let (mut tiles, truncated) = kra::tiles(data, ps)?;
        tiles.sort_by_key(|t| (t.y, t.x));
        let default = match (default.filter(|d| d.len() == ps), pixel) {
            (Some(d), Some(p)) => premul(to_f32(p, d)),
            (Some(d), None) => [f32::from(d[0]) / 255.0, 0.0, 0.0, 0.0],
            (None, _) => [0.0; 4],
        };
        let tall = tiles.iter().map(|t| i64::from(t.h)).max().unwrap_or(0);
        Ok((
            Source {
                tiles,
                x,
                y,
                pixel,
                default,
                tall,
            },
            truncated,
        ))
    }

    /// Renders canvas rows `y0..y0 + rows` of a `w`-wide canvas into `out` (`ch` floats per
    /// pixel: 4 premultiplied, or 1 for a mask). Returns whether anything but transparency
    /// (or, for a mask, full selection) may be there, and whether a tile was corrupt.
    fn render(
        &self,
        y0: i64,
        rows: usize,
        w: usize,
        out: &mut [f32],
        buf: &mut Vec<u8>,
        off: (i64, i64),
    ) -> (bool, bool) {
        let (sx, sy) = (self.x + off.0, self.y + off.1);
        let ch = if self.pixel.is_some() { 4 } else { 1 };
        let fill = &self.default[..ch];
        let mut any = fill.iter().any(|&v| v != 0.0);
        for o in out.chunks_exact_mut(ch) {
            o.copy_from_slice(fill);
        }
        let y1 = y0 + rows as i64;
        // Tiles whose canvas rows reach the band: top edge in `y0 - tall + 1 .. y1`.
        let lo = self.tiles.partition_point(|t| sy + t.y <= y0 - self.tall);
        let hi = self.tiles.partition_point(|t| sy + t.y < y1);
        let mut bad = false;
        for t in &self.tiles[lo..hi.max(lo)] {
            let (tx, ty) = (sx + t.x, sy + t.y);
            let (cx0, cx1) = (tx.max(0), (tx + i64::from(t.w)).min(w as i64));
            let (cy0, cy1) = (ty.max(y0), (ty + i64::from(t.h)).min(y1));
            if cx0 >= cx1 || cy0 >= cy1 {
                continue;
            }
            let ps = self.pixel.map_or(1, KraPixel::size);
            let n = t.w as usize * t.h as usize;
            let src: &[u8] = if t.lzf {
                if lzf_decompress(t.data, buf, n * ps).is_err() {
                    bad = true;
                    continue;
                }
                buf
            } else {
                match t.data.get(..n * ps) {
                    Some(d) => d,
                    None => {
                        bad = true;
                        continue;
                    }
                }
            };
            any = true;
            let span = (cx1 - cx0) as usize;
            for cy in cy0..cy1 {
                let i0 = (cy - ty) as usize * t.w as usize + (cx0 - tx) as usize;
                let row = &mut out[((cy - y0) as usize * w + cx0 as usize) * ch..][..span * ch];
                match self.pixel {
                    None => {
                        for (o, &v) in row.iter_mut().zip(&src[i0..i0 + span]) {
                            *o = f32::from(v) / 255.0;
                        }
                    }
                    Some(p) => row_pixels(p, t.lzf, src, n, i0, row),
                }
            }
        }
        (any, bad)
    }
}

/// Converts `row.len() / 4` pixels of tile pixels `src` (`n` pixels, byte-planar when `planar`)
/// from pixel `i0` on into premultiplied `f32`.
fn row_pixels(p: KraPixel, planar: bool, src: &[u8], n: usize, i0: usize, row: &mut [f32]) {
    fn go<const PS: usize>(
        p: KraPixel,
        planar: bool,
        src: &[u8],
        n: usize,
        i0: usize,
        row: &mut [f32],
    ) {
        let span = row.len() / 4;
        if planar {
            let planes: [&[u8]; PS] = std::array::from_fn(|c| &src[c * n + i0..][..span]);
            for (k, o) in row.chunks_exact_mut(4).enumerate() {
                let px: [u8; PS] = std::array::from_fn(|c| planes[c][k]);
                o.copy_from_slice(&premul(to_f32(p, &px)));
            }
        } else {
            for (s, o) in src[i0 * PS..][..span * PS]
                .chunks_exact(PS)
                .zip(row.chunks_exact_mut(4))
            {
                o.copy_from_slice(&premul(to_f32(p, s)));
            }
        }
    }
    match p.size() {
        2 => go::<2>(p, planar, src, n, i0, row),
        4 => go::<4>(p, planar, src, n, i0, row),
        8 => go::<8>(p, planar, src, n, i0, row),
        _ => go::<16>(p, planar, src, n, i0, row),
    }
}

/// A layer drawn before compositing (a vector layer's shapes, a file layer's image): RGBA8
/// at a canvas position, premultiplied or straight.
struct Raster {
    x: i64,
    y: i64,
    w: usize,
    h: usize,
    data: Vec<u8>,
    premultiplied: bool,
}

impl Raster {
    /// Renders canvas rows `y0..y0 + rows` of a `w`-wide canvas into premultiplied `out`;
    /// whether anything was there.
    fn render(&self, y0: i64, rows: usize, w: usize, out: &mut [f32], off: (i64, i64)) -> bool {
        let (rx, ry) = (self.x + off.0, self.y + off.1);
        let (x0, x1) = (rx.max(0), (rx + self.w as i64).min(w as i64));
        let (cy0, cy1) = (ry.max(y0), (ry + self.h as i64).min(y0 + rows as i64));
        if x0 >= x1 || cy0 >= cy1 {
            return false;
        }
        for cy in cy0..cy1 {
            let src = &self.data[((cy - ry) as usize * self.w + (x0 - rx) as usize) * 4..]
                [..(x1 - x0) as usize * 4];
            let row =
                &mut out[((cy - y0) as usize * w + x0 as usize) * 4..][..(x1 - x0) as usize * 4];
            for (o, s) in row.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
                let a = f32::from(s[3]) / 255.0;
                let k = if self.premultiplied {
                    1.0 / 255.0
                } else {
                    a / 255.0
                };
                o[0] = f32::from(s[0]) * k;
                o[1] = f32::from(s[1]) * k;
                o[2] = f32::from(s[2]) * k;
                o[3] = a;
            }
        }
        true
    }
}

/// What draws a node's own pixels.
enum Content {
    None,
    /// A paint layer's tiles (an index into `sources`).
    Paint(usize),
    Filter(Fx),
    Fill(Generator),
    Raster(Raster),
    /// A clone's source node.
    Clone(usize),
}

/// One mask of a node, applied bottom-most first: a selection (an index into `sources`;
/// `None`: everything selected) scaling the alpha, or a filter drawn through it.
struct MaskOp {
    sel: Option<usize>,
    fx: Option<Fx>,
}

/// The sources of a subtree, by node.
struct Prepared<'a> {
    nodes: &'a [KraNode],
    content: Vec<Content>,
    /// Per node: its masks, bottom-most first.
    masks: Vec<Vec<MaskOp>>,
    sources: Vec<Source<'a>>,
    w: usize,
    h: usize,
    hdr: bool,
    /// Per node: its whole-canvas result when it reads neighbouring pixels (a blur filter
    /// layer, or a layer with a blur mask), made once by the first band that needs it.
    full: Vec<std::sync::OnceLock<Vec<f32>>>,
}

/// Nodes to prepare for the visible nodes under `ids`: those, and the sources of clone layers
/// among them with their subtrees (visible or not).
fn wanted(nodes: &[KraNode], ids: &[usize], out: &mut Vec<usize>, all: bool) {
    for &i in ids {
        let n = &nodes[i];
        if (!all && !n.layer.visible) || out.contains(&i) {
            continue;
        }
        out.push(i);
        match n.kind {
            KraKind::Group { .. } => wanted(nodes, &n.children, out, all),
            KraKind::Clone { source: Some(s) } => wanted(nodes, &[s], out, true),
            _ => {}
        }
    }
}

/// A member's bytes and its `.defaultpixel`'s.
type Member<'d> = (Cow<'d, [u8]>, Option<Cow<'d, [u8]>>);

/// Reads a member and its `.defaultpixel`.
fn member<'d>(
    doc: &Document,
    data: &'d [u8],
    name: &str,
    held: &mut u64,
) -> Result<Member<'d>, Error> {
    let e = doc
        .zip
        .find(name)
        .ok_or(Error::Corrupt("Krita layer: member missing"))?;
    *held += e.size;
    if *held > MAX_HELD {
        return Err(Error::TooLarge("Krita layer data".into()));
    }
    let bytes = doc.read(data, e)?;
    let default = doc
        .zip
        .find(&format!("{name}.defaultpixel"))
        .and_then(|e| doc.zip.read(data, e, 64).ok());
    Ok((bytes, default))
}

/// The output of a composite in `pixel`.
fn store(out_fmt: KraOutput, gray_srgb: bool, px: &[f32], o: &mut [u8]) {
    let a = px[3].clamp(0.0, 1.0);
    let straight = |c: usize| if a > 0.0 { px[c] / a } else { 0.0 };
    match out_fmt {
        KraOutput::Rgba8 => {
            for (c, o) in o[..3].iter_mut().enumerate() {
                *o = (straight(c).clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
            }
            o[3] = (a * 255.0 + 0.5) as u8;
        }
        KraOutput::Rgba16 => {
            for c in 0..3 {
                o[c * 2..c * 2 + 2].copy_from_slice(&unit16(straight(c)).to_ne_bytes());
            }
            o[6..8].copy_from_slice(&unit16(a).to_ne_bytes());
        }
        KraOutput::Rgba16F => {
            for c in 0..3 {
                o[c * 2..c * 2 + 2].copy_from_slice(&f32_to_f16(straight(c)).to_ne_bytes());
            }
            o[6..8].copy_from_slice(&f32_to_f16(a).to_ne_bytes());
        }
        KraOutput::GrayAlpha16 => {
            let y = straight(0);
            let y = if gray_srgb { srgb16(y) } else { unit16(y) };
            o[..2].copy_from_slice(&y.to_ne_bytes());
            o[2..4].copy_from_slice(&unit16(a).to_ne_bytes());
        }
    }
}

/// A worker's buffers, reused from band to band (fresh multi-megabyte buffers per band and
/// node would cost a page fault per page).
#[derive(Default)]
struct Scratch {
    floats: Vec<Vec<f32>>,
    /// Decompressed tile bytes.
    tile: Vec<u8>,
}

impl Scratch {
    /// A zeroed buffer of `len` floats.
    fn take(&mut self, len: usize) -> Vec<f32> {
        let mut v = self.floats.pop().unwrap_or_default();
        v.clear();
        v.resize(len, 0.0);
        v
    }

    fn give(&mut self, v: Vec<f32>) {
        self.floats.push(v);
    }
}

impl Prepared<'_> {
    /// Composites `ids` (top-most first) into `dst`, canvas rows `y0..y0 + rows`, every
    /// layer's content moved by `off`.
    #[allow(clippy::too_many_arguments)]
    fn draw(
        &self,
        ids: &[usize],
        y0: i64,
        rows: usize,
        dst: &mut [f32],
        sc: &mut Scratch,
        bad: &AtomicBool,
        off: (i64, i64),
        depth: usize,
        base: [f32; 4],
    ) {
        for (k, &i) in ids.iter().enumerate().rev() {
            let n = &self.nodes[i];
            let l = &n.layer;
            if !l.visible || l.opacity <= 0.0 {
                continue;
            }
            if let KraKind::Group { pass_through: true } = n.kind {
                let mut src = sc.take(dst.len());
                src.copy_from_slice(dst);
                // (A filter with context inside sees the group's own layers only.)
                self.draw(
                    &n.children,
                    y0,
                    rows,
                    &mut src,
                    sc,
                    bad,
                    off,
                    depth + 1,
                    base,
                );
                let o = l.opacity.clamp(0.0, 1.0);
                for (d, t) in dst.iter_mut().zip(&src) {
                    *d += (t - *d) * o;
                }
                sc.give(src);
                continue;
            }
            let mut src = sc.take(dst.len());
            let op = KraBlend::from_id(&l.blend).unwrap_or(KraBlend::NORMAL);
            // Ops that change what is below where the layer is empty draw an empty layer too.
            let clears = matches!(
                op,
                KraBlend::Copy | KraBlend::DestinationIn | KraBlend::DestinationAtop
            );
            let drawn = match &self.content[i] {
                // A filter reading neighbours: what is below it drawn again over rows
                // extended by its reach, filtered, cut back to the band.
                // A filter reading neighbours: what is below it over the whole canvas, once,
                // filtered; each band takes its rows.
                Content::Filter(fx) if fx.halo() > 0 && y0 >= 0 => {
                    let full = self.full[i].get_or_init(|| {
                        let mut s2 = Scratch::default();
                        let mut all = vec![0f32; self.h * self.w * 4];
                        for p in all.chunks_exact_mut(4) {
                            p.copy_from_slice(&base);
                        }
                        self.draw(
                            &ids[k + 1..],
                            0,
                            self.h,
                            &mut all,
                            &mut s2,
                            bad,
                            off,
                            depth + 1,
                            base,
                        );
                        fx.blur(&mut all, self.w);
                        all
                    });
                    let skip = y0 as usize * self.w * 4;
                    let len = src.len();
                    src.copy_from_slice(&full[skip..skip + len]);
                    self.mask(i, y0, rows, dst, &mut src, sc, bad, off);
                    true
                }
                _ => self.pixels(i, y0, rows, dst, &mut src, sc, bad, off, depth),
            };
            if !drawn && clears {
                src.fill(0.0);
            }
            if drawn || (clears && self.drawable(i)) {
                blend(
                    dst,
                    &src,
                    l.opacity.clamp(0.0, 1.0),
                    op,
                    l.inherit_alpha,
                    self.hdr,
                );
            }
            sc.give(src);
        }
    }

    /// Whether node `i` is one this composite draws (an empty one draws nothing).
    fn drawable(&self, i: usize) -> bool {
        matches!(self.nodes[i].kind, KraKind::Group { .. })
            || !matches!(self.content[i], Content::None)
    }

    /// Node `i`'s projection (its content, then its masks; not its opacity or blend mode)
    /// into the zeroed `out`, `below` being what it is drawn onto. Whether it drew anything.
    #[allow(clippy::too_many_arguments)]
    fn pixels(
        &self,
        i: usize,
        y0: i64,
        rows: usize,
        below: &[f32],
        out: &mut [f32],
        sc: &mut Scratch,
        bad: &AtomicBool,
        off: (i64, i64),
        depth: usize,
    ) -> bool {
        if depth > crate::MAX_DEPTH {
            return false;
        }
        // Filter masks reading neighbours: the node drawn over extended rows, cut back.
        let halo = self.masks[i]
            .iter()
            .filter_map(|m| m.fx.as_ref().map(Fx::halo))
            .max()
            .unwrap_or(0);
        // (Over the whole canvas, once; what is below is only read by filter and fill
        // layers' own selections, taken as transparent there.)
        if halo > 0 && y0 >= 0 && rows < self.h {
            let full = self.full[i].get_or_init(|| {
                let mut s2 = Scratch::default();
                let len = self.h * self.w * 4;
                let clear = vec![0f32; len];
                let mut all = vec![0f32; len];
                if !self.pixels(i, 0, self.h, &clear, &mut all, &mut s2, bad, off, depth) {
                    all.clear();
                }
                all
            });
            if full.is_empty() {
                return false;
            }
            let skip = y0 as usize * self.w * 4;
            out.copy_from_slice(&full[skip..skip + out.len()]);
            return true;
        }
        let n = &self.nodes[i];
        let any = match (&n.kind, &self.content[i]) {
            (KraKind::Group { .. }, _) => {
                self.draw(
                    &n.children,
                    y0,
                    rows,
                    out,
                    sc,
                    bad,
                    off,
                    depth + 1,
                    [0.0; 4],
                );
                true
            }
            (_, Content::Paint(s)) => {
                let (any, b) = self.sources[*s].render(y0, rows, self.w, out, &mut sc.tile, off);
                if b {
                    bad.store(true, Ordering::Relaxed);
                }
                any
            }
            // Krita filters what is below and composites that over it.
            (_, Content::Filter(fx)) => {
                out.copy_from_slice(below);
                filter_premultiplied(fx, out);
                true
            }
            (_, Content::Fill(Generator::Color(c))) => {
                let p = premul(*c);
                for o in out.chunks_exact_mut(4) {
                    o.copy_from_slice(&p);
                }
                true
            }
            (_, Content::Raster(r)) => r.render(y0, rows, self.w, out, off),
            (_, Content::Clone(s)) => {
                let o = (off.0 + n.layer.x, off.1 + n.layer.y);
                self.pixels(*s, y0, rows, below, out, sc, bad, o, depth + 1)
            }
            (_, Content::None) => false,
        };
        if any {
            self.mask(i, y0, rows, below, out, sc, bad, off);
        }
        any
    }

    /// Applies node `i`'s masks, bottom-most first, to its premultiplied projection `px`.
    #[allow(clippy::too_many_arguments)]
    fn mask(
        &self,
        i: usize,
        y0: i64,
        rows: usize,
        below: &[f32],
        px: &mut [f32],
        sc: &mut Scratch,
        bad: &AtomicBool,
        off: (i64, i64),
    ) {
        if self.masks[i].is_empty() {
            return;
        }
        // A filter or fill layer's own selection, with Krita's `copy` op: the selection is
        // the op's mask, moving what is below towards the layer.
        let copy = matches!(self.nodes[i].kind, KraKind::Filter(_) | KraKind::Fill(_))
            && KraBlend::from_id(&self.nodes[i].layer.blend) == Some(KraBlend::Copy);
        let mut m = sc.take(px.len() / 4);
        for (k, op) in self.masks[i].iter().enumerate() {
            match op.sel {
                Some(s) => {
                    let (_, b) =
                        self.sources[s].render(y0, rows, self.w, &mut m, &mut sc.tile, off);
                    if b {
                        bad.store(true, Ordering::Relaxed);
                    }
                }
                None => m.fill(1.0),
            }
            match &op.fx {
                Some(fx) => {
                    let mut f = sc.take(px.len());
                    f.copy_from_slice(px);
                    if fx.halo() > 0 {
                        fx.blur(&mut f, self.w);
                    } else {
                        filter_premultiplied(fx, &mut f);
                    }
                    for ((p, q), &v) in px.chunks_exact_mut(4).zip(f.chunks_exact(4)).zip(&m) {
                        for c in 0..4 {
                            p[c] += (q[c] - p[c]) * v;
                        }
                    }
                    sc.give(f);
                }
                // The node's own selection is its first mask in file order, the last here.
                None if copy && k + 1 == self.masks[i].len() => {
                    for ((p, b), &v) in px.chunks_exact_mut(4).zip(below.chunks_exact(4)).zip(&m) {
                        if v < 1.0 {
                            for c in 0..4 {
                                p[c] = b[c] + (p[c] - b[c]) * v;
                            }
                        }
                    }
                }
                None => {
                    for (p, &v) in px.chunks_exact_mut(4).zip(&m) {
                        if v < 1.0 {
                            for c in p {
                                *c *= v;
                            }
                        }
                    }
                }
            }
        }
        sc.give(m);
    }
}

/// Applies a per-pixel filter to premultiplied pixels.
fn filter_premultiplied(fx: &Fx, px: &mut [f32]) {
    for p in px.chunks_exact_mut(4) {
        let a = p[3];
        if a > 0.0 {
            let r = 1.0 / a;
            for c in &mut p[..3] {
                *c *= r;
            }
        }
    }
    fx.apply(px);
    for p in px.chunks_exact_mut(4) {
        let a = p[3].clamp(0.0, 1.0);
        p[3] = a;
        for c in &mut p[..3] {
            *c *= a;
        }
    }
}

/// Services a composite needs from outside this crate: drawing vector layers (SVG) and
/// reading file layers (images in other files).
pub trait KraHost: Sync {
    /// `svg` drawn on a `w`×`h` canvas, its viewport (Krita writes the canvas size in
    /// points) stretched over the canvas: premultiplied RGBA8, `w·h·4` bytes.
    fn svg(&self, svg: &[u8], w: u32, h: u32) -> Option<Vec<u8>>;
    /// A file layer's image (`source` as the document stores it), straight RGBA8, or why
    /// it cannot be read, in plain words.
    fn file(&self, source: &str) -> Result<(u32, u32, Vec<u8>), String>;
    /// Whether [`KraHost::file`] would find `source` (without decoding it), or why not.
    fn locate(&self, source: &str) -> Result<(), String>;
}

/// The settings of node filter `spec`.
pub(crate) fn spec_params(doc: &Document, data: &[u8], spec: &KraFilterSpec) -> Params {
    doc.zip
        .find(&spec.config)
        .and_then(|e| doc.zip.read(data, e, 4 << 20).ok())
        .map(|b| params(&b))
        .unwrap_or_default()
}

/// Composites node `group` (a group layer page: its children, its masks and its opacity;
/// its blend mode not applied) or, with `None`, the whole layer tree, over transparency.
pub(crate) fn composite(
    doc: &Document,
    data: &[u8],
    group: Option<usize>,
    host: Option<&dyn KraHost>,
    cancel: &(dyn Fn() -> bool + Sync),
) -> Result<KraImage, Error> {
    let (w, h) = doc
        .canvas
        .ok_or(Error::Corrupt("maindoc.xml: image size"))?;
    if u64::from(w) * u64::from(h) > MAX_CANVAS_PIXELS {
        return Err(Error::TooLarge(format!("{w}×{h} canvas")));
    }
    let pixel = doc
        .kra_pixel
        .ok_or_else(|| Error::Unsupported("Krita colour model of the image".into()))?;
    let nodes = &doc.kra_nodes[..];
    let ids: &[usize] = match group {
        Some(g) => &nodes[g].children,
        None => &doc.kra_root,
    };
    let linear = kra::linear_profile(&doc.kra_profile);
    let grey = pixel.is_gray();
    // Read every member first, then index their tiles.
    let mut list = Vec::new();
    wanted(nodes, ids, &mut list, false);
    if let Some(g) = group {
        list.push(g);
    }
    let mut held = 0u64;
    let mut content: Vec<Content> = (0..nodes.len()).map(|_| Content::None).collect();
    // (node, pixel of a paint layer or None for a selection, x, y, bytes, the mask's index)
    type Pending<'d> = (usize, Option<KraPixel>, i64, i64, Member<'d>, Option<usize>);
    let mut bytes: Vec<Pending<'_>> = Vec::new();
    let mut masks: Vec<Vec<MaskOp>> = (0..nodes.len()).map(|_| Vec::new()).collect();
    for &i in &list {
        let l = &nodes[i].layer;
        if Some(i) != group {
            match &nodes[i].kind {
                KraKind::Paint => {
                    let (b, d) = member(doc, data, &l.member, &mut held)?;
                    bytes.push((
                        i,
                        Some(l.pixel),
                        l.x,
                        l.y,
                        (kra::plain_tiles(b, l.pixel)?, d),
                        None,
                    ));
                }
                KraKind::Filter(spec) => {
                    if let Ok(fx) = Fx::new(&spec.name, &spec_params(doc, data, spec), grey) {
                        content[i] = Content::Filter(fx);
                    }
                }
                KraKind::Fill(spec) => {
                    if let Ok(g) = Generator::new(&spec.name, &spec_params(doc, data, spec), linear)
                    {
                        content[i] = Content::Fill(g);
                    }
                }
                KraKind::Vector { svg } => {
                    if let (Some(host), Some(e)) = (host, doc.zip.find(svg)) {
                        held += u64::from(w) * u64::from(h) * 4 + e.size;
                        if held > MAX_HELD {
                            return Err(Error::TooLarge("Krita layer data".into()));
                        }
                        let bytes = doc.read(data, e)?;
                        let fixed = crate::kra_svg::fixup(&bytes);
                        if let Some(px) = host.svg(fixed.as_deref().unwrap_or(&bytes), w, h)
                            && px.len() == w as usize * h as usize * 4
                        {
                            content[i] = Content::Raster(Raster {
                                x: l.x,
                                y: l.y,
                                w: w as usize,
                                h: h as usize,
                                data: px,
                                premultiplied: true,
                            });
                        }
                    }
                }
                KraKind::File { source } => {
                    if let Some(Ok((fw, fh, px))) = host.map(|h| h.file(source))
                        && px.len() == fw as usize * fh as usize * 4
                    {
                        held += px.len() as u64;
                        if held > MAX_HELD {
                            return Err(Error::TooLarge("Krita layer data".into()));
                        }
                        content[i] = Content::Raster(Raster {
                            x: l.x,
                            y: l.y,
                            w: fw as usize,
                            h: fh as usize,
                            data: px,
                            premultiplied: false,
                        });
                    }
                }
                KraKind::Clone { source: Some(s) } => content[i] = Content::Clone(*s),
                _ => {}
            }
        }
        // Masks bottom-most first.
        for m in l.masks.iter().rev() {
            let fx = match &m.filter {
                None => None,
                Some(spec) => match Fx::new(&spec.name, &spec_params(doc, data, spec), grey) {
                    Ok(fx) => Some(fx),
                    // A filter mask that is not drawn is left out.
                    Err(_) => continue,
                },
            };
            match member(doc, data, &m.member, &mut held) {
                Ok(b) => {
                    bytes.push((i, None, m.x, m.y, b, Some(masks[i].len())));
                    masks[i].push(MaskOp { sel: None, fx });
                }
                // Without its selection a filter mask or a filter layer's selection covers
                // everything; a transparency mask whose data is missing is left out.
                Err(_) if fx.is_some() || m.name.is_empty() => {
                    masks[i].push(MaskOp { sel: None, fx })
                }
                Err(_) => {}
            }
        }
    }
    if cancel() {
        return Err(Error::Cancelled);
    }
    let mut p = Prepared {
        nodes,
        content,
        masks,
        sources: Vec::with_capacity(bytes.len()),
        w: w as usize,
        h: h as usize,
        hdr: pixel.is_float(),
        full: (0..nodes.len())
            .map(|_| std::sync::OnceLock::new())
            .collect(),
    };
    let mut truncated = false;
    for (i, px, x, y, (b, d), slot) in &bytes {
        let (s, t) = Source::new(b, d.as_deref(), *x, *y, *px)?;
        truncated |= t;
        let at = p.sources.len();
        p.sources.push(s);
        match slot {
            None => p.content[*i] = Content::Paint(at),
            Some(k) => p.masks[*i][*k].sel = Some(at),
        }
    }
    let out_fmt = kra::output_for(pixel, linear);
    let gray_srgb = pixel.is_gray() && pixel.is_float() && linear;
    let ob = out_fmt.bytes();
    let stride = w as usize * ob;
    let size = h as usize * stride;
    let mut out: Vec<u8> = Vec::new();
    out.try_reserve_exact(size)
        .map_err(|_| Error::TooLarge(format!("{w}×{h} canvas")))?;
    out.resize(size, 0);
    // The whole tree is drawn over the projection's background colour.
    let under = match group {
        None if doc.kra_background.len() == pixel.size() => {
            premul(to_f32(pixel, &doc.kra_background))
        }
        _ => [0.0; 4],
    };
    let bad = AtomicBool::new(false);
    out.par_chunks_mut(stride * BAND)
        .enumerate()
        .try_for_each_init(Scratch::default, |sc, (k, rows_out)| -> Result<(), Error> {
            if cancel() {
                return Err(Error::Cancelled);
            }
            let rows = rows_out.len() / stride;
            let y0 = (k * BAND) as i64;
            let mut band = sc.take(rows * w as usize * 4);
            if under != [0.0; 4] {
                for px in band.chunks_exact_mut(4) {
                    px.copy_from_slice(&under);
                }
            }
            p.draw(ids, y0, rows, &mut band, sc, &bad, (0, 0), 0, under);
            if let Some(g) = group {
                let clear = sc.take(band.len());
                p.mask(g, y0, rows, &clear, &mut band, sc, &bad, (0, 0));
                sc.give(clear);
                let o = nodes[g].layer.opacity.clamp(0.0, 1.0);
                if o < 1.0 {
                    band.iter_mut().for_each(|v| *v *= o);
                }
            }
            for (px, o) in band.chunks_exact(4).zip(rows_out.chunks_exact_mut(ob)) {
                store(out_fmt, gray_srgb, px, o);
            }
            sc.give(band);
            Ok(())
        })?;
    Ok(KraImage {
        width: w,
        height: h,
        format: out_fmt,
        data: out,
        truncated: truncated || bad.load(Ordering::Relaxed),
    })
}

/// Multiplies the transparency masks of paint layer `l` into its page `img`.
pub(crate) fn mask_page(
    doc: &Document,
    data: &[u8],
    l: &KraLayer,
    img: &mut KraImage,
    cancel: &(dyn Fn() -> bool + Sync),
) -> Result<(), Error> {
    let mut held = 0;
    let mut bytes = Vec::new();
    for m in l.masks.iter().filter(|m| m.filter.is_none()) {
        if let Ok((b, d)) = member(doc, data, &m.member, &mut held) {
            bytes.push((m, b, d));
        }
    }
    let mut sources = Vec::new();
    for (m, b, d) in &bytes {
        let (s, t) = Source::new(b, d.as_deref(), m.x, m.y, None)?;
        img.truncated |= t;
        sources.push(s);
    }
    if sources.is_empty() {
        return Ok(());
    }
    let w = img.width as usize;
    let ob = img.format.bytes();
    let fmt = img.format;
    let stride = w * ob;
    let bad = AtomicBool::new(false);
    img.data
        .par_chunks_mut(stride * BAND)
        .enumerate()
        .try_for_each(|(k, rows_out)| -> Result<(), Error> {
            if cancel() {
                return Err(Error::Cancelled);
            }
            let rows = rows_out.len() / stride;
            let mut m = vec![0f32; rows * w];
            let mut buf = Vec::new();
            for s in &sources {
                let (_, b) = s.render((k * BAND) as i64, rows, w, &mut m, &mut buf, (0, 0));
                if b {
                    bad.store(true, Ordering::Relaxed);
                }
                for (o, &v) in rows_out.chunks_exact_mut(ob).zip(&m) {
                    if v < 1.0 {
                        scale_alpha(fmt, o, v);
                    }
                }
            }
            Ok(())
        })?;
    img.truncated |= bad.load(Ordering::Relaxed);
    Ok(())
}

/// Multiplies the alpha of straight pixel `o` by `v`.
#[inline]
fn scale_alpha(fmt: KraOutput, o: &mut [u8], v: f32) {
    match fmt {
        KraOutput::Rgba8 => o[3] = (f32::from(o[3]) * v + 0.5) as u8,
        KraOutput::Rgba16 => {
            let a = u16::from_ne_bytes([o[6], o[7]]);
            o[6..8].copy_from_slice(&((f32::from(a) * v + 0.5) as u16).to_ne_bytes());
        }
        KraOutput::Rgba16F => {
            let a = f16_to_f32(u16::from_ne_bytes([o[6], o[7]]));
            o[6..8].copy_from_slice(&f32_to_f16(a * v).to_ne_bytes());
        }
        KraOutput::GrayAlpha16 => {
            let a = u16::from_ne_bytes([o[2], o[3]]);
            o[2..4].copy_from_slice(&((f32::from(a) * v + 0.5) as u16).to_ne_bytes());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn separable_ops_at_known_points() {
        let close = |a: f32, b: f32| (a - b).abs() < 1e-6;
        let mx = |id: &str, s: f32, d: f32, hdr: bool| match KraBlend::from_id(id) {
            Some(KraBlend::Blend(m)) => mix(m, s, d, hdr),
            _ => panic!("{id}"),
        };
        assert!(close(mx("multiply", 0.5, 0.5, false), 0.25));
        assert!(close(mx("screen", 0.5, 0.5, false), 0.75));
        assert!(close(mx("overlay", 0.2, 0.8, false), 1.0 - 2.0 * 0.8 * 0.2));
        assert!(close(
            mx("hard_light", 0.8, 0.2, false),
            1.0 - 2.0 * 0.8 * 0.2
        ));
        assert!(close(mx("add", 0.7, 0.6, false), 1.0));
        assert!(close(mx("add", 0.7, 0.6, true), 1.3));
        assert!(close(mx("subtract", 0.7, 0.6, false), 0.0));
        assert!(close(mx("subtract", 0.25, 0.75, false), 0.5));
        assert!(close(mx("diff", 0.25, 0.75, false), 0.5));
        assert!(close(mx("dodge", 0.5, 0.25, false), 0.5));
        assert!(close(mx("dodge", 0.75, 0.5, true), 2.0));
        assert!(close(mx("burn", 0.5, 0.75, false), 0.5));
        assert!(close(mx("burn", 0.0, 0.5, false), 0.0));
        assert!(close(mx("soft_light", 0.5, 0.3, false), 0.3));
        assert!(close(mx("soft_light", 1.0, 0.25, false), 0.5));
        assert!(close(mx("soft_light_svg", 1.0, 0.25, false), 0.5));
        assert!(close(mx("darken", 0.3, 0.6, false), 0.3));
        assert!(close(mx("lighten", 0.3, 0.6, false), 0.6));
        assert!(close(mx("linear_burn", 0.75, 0.5, false), 0.25));
        assert!(close(mx("exclusion", 0.5, 0.25, false), 0.5));
        assert!(close(mx("allanon", 0.25, 0.75, false), 0.5));
        assert!(close(mx("grain_merge", 0.75, 0.5, false), 0.75));
        assert_eq!(KraBlend::from_id("erase"), Some(KraBlend::Erase));
        assert_eq!(KraBlend::from_id("xor"), Some(KraBlend::Extra(Extra::Xor)));
        // 8-bit values stay exact through the 16-bit integers.
        let v = |k: u16| f32::from(k) / 255.0;
        assert!((Extra::And.mix(v(0b1100_1010), v(0b1010_0110)) - v(0b1000_0010)).abs() < 1e-6);
        assert!((Extra::Xor.mix(v(0xF0), v(0xFF)) - v(0x0F)).abs() < 1e-6);
        assert!((Extra::Modulo.mix(0.3, 0.7) - 0.1).abs() < 1e-3);
        assert!((Extra::DivisiveModulo.mix(0.4, 0.5) - 0.25).abs() < 1e-3);
        assert_eq!(KraBlend::from_id("no_such_op"), None);
    }

    #[test]
    fn erase_behind_and_whole_colour_ops() {
        // Erase at half coverage halves what is below.
        let mut d = [0.5, 0.25, 0.0, 1.0];
        blend(
            &mut d,
            &[0.5, 0.5, 0.5, 0.5],
            1.0,
            KraBlend::Erase,
            false,
            false,
        );
        assert_eq!(d, [0.25, 0.125, 0.0, 0.5]);
        // Behind fills only the uncovered half.
        let mut d = [0.5, 0.0, 0.0, 0.5];
        blend(
            &mut d,
            &[0.0, 1.0, 0.0, 1.0],
            1.0,
            KraBlend::Behind,
            false,
            false,
        );
        assert_eq!(d, [0.5, 0.5, 0.0, 1.0]);
        // Luminosity of a white source over opaque red: red lifted to white's luminosity.
        let mut d = [1.0, 0.0, 0.0, 1.0];
        let op = KraBlend::from_id("luminize").unwrap();
        blend(&mut d, &[1.0, 1.0, 1.0, 1.0], 1.0, op, false, false);
        assert!(d.iter().all(|&v| (v - 1.0).abs() < 1e-5), "{d:?}");
    }

    #[test]
    fn blend_alpha_union_and_inherit() {
        // Half-transparent red multiplied over half-transparent grey.
        let mut d = [0.25, 0.25, 0.25, 0.5];
        blend(
            &mut d,
            &[0.5, 0.0, 0.0, 0.5],
            1.0,
            KraBlend::from_id("multiply").unwrap(),
            false,
            false,
        );
        // αs = αd = 0.5: union 0.75; red: 0.5·0.25 + 0.5·0.5·1 + 0.25·0.5 = 0.5.
        assert!(
            (d[3] - 0.75).abs() < 1e-6 && (d[0] - 0.5).abs() < 1e-6,
            "{d:?}"
        );
        assert!((d[1] - 0.125).abs() < 1e-6, "{d:?}");
        // Inherit alpha over nothing paints nothing; over something keeps its alpha.
        let mut e = [0.0; 4];
        blend(
            &mut e,
            &[1.0, 1.0, 1.0, 1.0],
            1.0,
            KraBlend::NORMAL,
            true,
            false,
        );
        assert_eq!(e, [0.0; 4]);
        let mut f = [0.0, 0.0, 0.0, 0.5];
        blend(
            &mut f,
            &[1.0, 1.0, 1.0, 1.0],
            0.5,
            KraBlend::NORMAL,
            true,
            false,
        );
        assert!(
            (f[3] - 0.5).abs() < 1e-6 && (f[0] - 0.25).abs() < 1e-6,
            "{f:?}"
        );
    }
}
