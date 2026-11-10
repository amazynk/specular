//! Layer effects (layer styles) in the composite built from the layers: drop and inner
//! shadows, outer and inner glows, colour, gradient and pattern overlays, stroke, satin and
//! bevel & emboss, from the layer's `lmfx`, `lfx2` or `lfxs` descriptor (else its legacy
//! `lrFX` records, [`crate::legacy`]), at the sizes it stores ("Scale Effects" rewrites them;
//! the descriptor's own scale is not applied again).
//!
//! Every effect is a coverage plane derived from the layer's shape (its transparency with its
//! masks, before its opacities) in a region around it: grown or choked by a signed distance
//! (an exact Euclidean distance transform), softened by a blur close to a Gaussian (three box
//! passes), offset along the light angle. Shadows and outer glows are drawn under the layer,
//! straight onto the backdrop; the others are drawn on the layer's content, from bottom to
//! top: pattern, gradient and colour overlays, satin, inner glow, inner shadow, stroke, bevel.
//! Fill opacity fades the content but not its effects; opacity fades both. Effects on the
//! content keep its transparency (they are mixed as if its shape were opaque); interior
//! effects with a blend mode of their own, or on a layer with one, blend with what is under
//! the layer as well. The layer hides what is drawn under it only as far as its shape goes.
//! Patterns start at the layer's reference point when linked with it.
//!
//! Smooth bevel & emboss lights a height field made by blurring the shape (σ ≈ 0.45 × size;
//! an emboss uses half the size, a pillow emboss folds the field at the edge), scaled by size
//! and depth; satin compares two copies of the shape half the distance each way. Both were
//! fitted to Photoshop's merged images (tests/effects_oracle.rs). Chisel bevels use the
//! distance to the edge and are approximate; bevel textures are left out. Contours (gloss,
//! shape, glow, shadow and satin) map the effect's coverage through their curve. Noise and
//! jitter are left out of every effect, and a glow's gradient is drawn in its first colour.

use rayon::prelude::*;

use crate::compose::{Canvas, blend1, blend3};
use crate::desc::{self, Desc, Value};
use crate::{Layer, Psd};

/// Effects in the order they are drawn, bottom first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Kind {
    DropShadow,
    OuterGlow,
    PatternOverlay,
    GradientOverlay,
    ColorOverlay,
    Satin,
    InnerGlow,
    InnerShadow,
    Stroke,
    Bevel,
}

impl Kind {
    /// Photoshop's name, for notes.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Kind::DropShadow => "Drop Shadow",
            Kind::OuterGlow => "Outer Glow",
            Kind::PatternOverlay => "Pattern Overlay",
            Kind::GradientOverlay => "Gradient Overlay",
            Kind::ColorOverlay => "Color Overlay",
            Kind::Satin => "Satin",
            Kind::InnerGlow => "Inner Glow",
            Kind::InnerShadow => "Inner Shadow",
            Kind::Stroke => "Stroke",
            Kind::Bevel => "Bevel & Emboss",
        }
    }

    const ALL: [Kind; 10] = [
        Kind::DropShadow,
        Kind::OuterGlow,
        Kind::PatternOverlay,
        Kind::GradientOverlay,
        Kind::ColorOverlay,
        Kind::Satin,
        Kind::InnerGlow,
        Kind::InnerShadow,
        Kind::Stroke,
        Kind::Bevel,
    ];

    fn behind(self) -> bool {
        matches!(self, Kind::DropShadow | Kind::OuterGlow)
    }

    /// The interior effects "Blend Interior Effects as Group" (`infx`) fades with the
    /// layer's fill opacity.
    fn interior(self) -> bool {
        matches!(
            self,
            Kind::PatternOverlay
                | Kind::GradientOverlay
                | Kind::ColorOverlay
                | Kind::Satin
                | Kind::InnerGlow
        )
    }
}

/// The bits of the kinds named in `names` (Photoshop's names), for [`parse_skipping`].
pub(crate) fn kind_bits(names: &[&str]) -> u32 {
    Kind::ALL
        .iter()
        .filter(|k| names.contains(&k.name()))
        .fold(0, |m, k| m | 1 << *k as u32)
}

/// Which of the effects drawn on the layer's content [`Effects::draw_inside`] draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Inside {
    All,
    /// Overlays, satin and inner glow.
    Interior,
    /// The others (inner shadow, stroke, bevel).
    Rest,
}

/// Descriptor keys of single effects and of effect lists.
const KEYS: &[(&[u8], Kind)] = &[
    (b"DrSh", Kind::DropShadow),
    (b"dropShadowMulti", Kind::DropShadow),
    (b"OrGl", Kind::OuterGlow),
    (b"patternFill", Kind::PatternOverlay),
    (b"patternFillMulti", Kind::PatternOverlay),
    (b"GrFl", Kind::GradientOverlay),
    (b"gradientFillMulti", Kind::GradientOverlay),
    (b"SoFi", Kind::ColorOverlay),
    (b"solidFillMulti", Kind::ColorOverlay),
    (b"ChFX", Kind::Satin),
    (b"IrGl", Kind::InnerGlow),
    (b"IrSh", Kind::InnerShadow),
    (b"innerShadowMulti", Kind::InnerShadow),
    (b"FrFX", Kind::Stroke),
    (b"frameFXMulti", Kind::Stroke),
    (b"ebbl", Kind::Bevel),
];

struct Effect {
    kind: Kind,
    d: Desc,
}

/// A layer's enabled effects, bottom first.
pub(crate) struct Effects {
    scale: f64,
    list: Vec<Effect>,
}

fn enabled(d: &Desc) -> bool {
    d.boolean(b"enab").unwrap_or(true) && d.boolean(b"present").unwrap_or(true)
}

/// The effects descriptor of layer `l` (`lmfx`, else `lfx2` or `lfxs`, else the legacy
/// `lrFX` read as one), when effects are on.
fn descriptor(data: &[u8], l: &Layer) -> Option<Desc> {
    let d = [b"lmfx", b"lfx2", b"lfxs"]
        .iter()
        .find_map(|k| {
            l.block(k)
                .and_then(|r| data.get(r))
                .and_then(desc::versioned2)
        })
        .or_else(|| {
            if [b"lmfx", b"lfx2", b"lfxs"]
                .iter()
                .any(|k| l.block(k).is_some())
            {
                return None;
            }
            l.block(b"lrFX")
                .and_then(|r| data.get(r))
                .and_then(crate::legacy::effects)
        })?;
    (d.boolean(b"masterFXSwitch") != Some(false)).then_some(d)
}

/// Layer `l`'s enabled effects, `None` without any.
pub(crate) fn parse(data: &[u8], l: &Layer) -> Option<Effects> {
    parse_skipping(data, l, 0)
}

/// [`parse`] without the kinds whose bits (`1 << kind`) are in `skip`.
pub(crate) fn parse_skipping(data: &[u8], l: &Layer, skip: u32) -> Option<Effects> {
    let d = descriptor(data, l)?;
    let mut list = Vec::new();
    for (key, kind) in KEYS {
        if skip & (1 << *kind as u32) != 0 {
            continue;
        }
        match d.get(key) {
            Some(Value::Obj(o)) if enabled(o) => list.push(Effect {
                kind: *kind,
                d: o.clone(),
            }),
            Some(Value::List(items)) => {
                // Lists name the top-most first.
                for v in items.iter().rev() {
                    if let Value::Obj(o) = v
                        && enabled(o)
                    {
                        list.push(Effect {
                            kind: *kind,
                            d: o.clone(),
                        });
                    }
                }
            }
            _ => {}
        }
    }
    if list.is_empty() {
        return None;
    }
    list.sort_by_key(|e| e.kind);
    Some(Effects {
        // The sizes are stored as drawn: "Scale Effects" rewrites them, and the descriptor's
        // `Scl ` (the document's resolution over 72 in files saved at other resolutions) is
        // not applied again (Photoshop's merged images agree).
        scale: 1.0,
        list,
    })
}

/// Legacy-only effects (`lrFX` without a descriptor) that cannot be read: not drawn.
pub(crate) fn legacy_only(data: &[u8], l: &Layer) -> bool {
    l.block(b"lrFX")
        .and_then(|r| data.get(r))
        .is_some_and(|b| crate::legacy::effects_and_dropped(b).is_none_or(|(_, lost)| lost))
        && ![b"lmfx", b"lfx2", b"lfxs"]
            .iter()
            .any(|k| l.block(k).is_some())
}

/// Whether one of `l`'s enabled effects paints a noise gradient (not drawn).
pub(crate) fn has_noise_gradient(data: &[u8], l: &Layer) -> bool {
    parse(data, l).is_some_and(|e| {
        e.list.iter().any(|e| {
            e.d.obj(b"Grad")
                .and_then(|g| g.enum_value(b"GrdF"))
                .is_some_and(|f| f == b"ClNs")
        })
    })
}

/// `l`'s enabled effects with their settings, one per line (oracle diagnostics).
pub(crate) fn describe(data: &[u8], l: &Layer) -> String {
    fn val(v: &Value) -> String {
        match v {
            Value::Num(x) => format!("{x}"),
            Value::Unit(u, x) => format!("{x}{}", String::from_utf8_lossy(u)),
            Value::Int(x) => format!("{x}"),
            Value::Bool(b) => format!("{b}"),
            Value::Text(t) => format!("{t:?}"),
            Value::Enum(_, e) => String::from_utf8_lossy(e).into_owned(),
            Value::Obj(d) => obj(d),
            Value::List(l) => format!("[{}]", l.iter().map(val).collect::<Vec<_>>().join(", ")),
            Value::Other => "?".into(),
        }
    }
    fn obj(d: &Desc) -> String {
        let items: Vec<String> = d
            .items
            .iter()
            .map(|(k, v)| format!("{}={}", String::from_utf8_lossy(k).trim_end(), val(v)))
            .collect();
        format!("{{{}}}", items.join(" "))
    }
    let top = descriptor(data, l).map_or_else(String::new, |d| {
        let keys: Vec<String> = d
            .items
            .iter()
            .map(|(k, _)| String::from_utf8_lossy(k).into_owned())
            .collect();
        format!("keys: {}\nfull: {}\n", keys.join(" "), obj(&d))
    });
    parse(data, l).map_or_else(String::new, |e| {
        top + &e
            .list
            .iter()
            .map(|e| format!("{}: {}\n", e.kind.name(), obj(&e.d)))
            .collect::<String>()
    })
}

/// Photoshop's names for what `l`'s enabled effects draw only approximately (for notes):
/// chisel bevels and bevel textures (no merged image to fit them to).
pub(crate) fn approximate(data: &[u8], l: &Layer) -> Vec<&'static str> {
    let Some(e) = parse(data, l) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for e in e.list.iter().filter(|e| e.kind == Kind::Bevel) {
        if e.d.enum_value(b"bvlT").is_some_and(|t| t != b"SfBL") {
            out.push("Bevel & Emboss (chisel)");
        }
        if e.d.boolean(b"useTexture") == Some(true) {
            out.push("Bevel & Emboss texture");
        }
    }
    out
}

/// A descriptor blend mode (`BlnM` enumeration) as a layer blend-mode key: the four-letter
/// codes, or the spelled-out names recent Photoshop versions write.
pub(crate) fn mode_key(v: Option<&[u8]>) -> [u8; 4] {
    *match v.unwrap_or(b"Nrml") {
        b"Nrml" | b"normal" => b"norm",
        b"Dslv" | b"dissolve" => b"diss",
        b"Drkn" | b"darken" => b"dark",
        b"Mltp" | b"multiply" => b"mul ",
        b"CBrn" | b"colorBurn" => b"idiv",
        b"linearBurn" => b"lbrn",
        b"darkerColor" => b"dkCl",
        b"Lghn" | b"lighten" => b"lite",
        b"Scrn" | b"screen" => b"scrn",
        b"CDdg" | b"colorDodge" => b"div ",
        b"linearDodge" => b"lddg",
        b"lighterColor" => b"lgCl",
        b"Ovrl" | b"overlay" => b"over",
        b"SftL" | b"softLight" => b"sLit",
        b"HrdL" | b"hardLight" => b"hLit",
        b"vividLight" => b"vLit",
        b"linearLight" => b"lLit",
        b"pinLight" => b"pLit",
        b"hardMix" => b"hMix",
        b"Dfrn" | b"difference" => b"diff",
        b"Xclu" | b"exclusion" => b"smud",
        b"blendSubtraction" | b"subtract" => b"fsub",
        b"blendDivide" | b"divide" => b"fdiv",
        b"H   " | b"hue" => b"hue ",
        b"Strt" | b"saturation" => b"sat ",
        b"Clr " | b"color" => b"colr",
        b"Lmns" | b"luminosity" => b"lum ",
        _ => b"norm",
    }
}

/// A region of the canvas.
#[derive(Clone, Copy, Debug)]
struct Reg {
    x: usize,
    y: usize,
    w: usize,
    h: usize,
}

/// What the effects of one layer are drawn with.
pub(crate) struct Fx<'a> {
    pub psd: &'a Psd,
    pub data: &'a [u8],
    pub w: usize,
    pub h: usize,
    pub rgb_like: bool,
    /// Where linked patterns start (the layer's reference point).
    pub origin: (f64, f64),
    /// The alpha factor (fill opacity times opacity) the layer is drawn with over the
    /// effects beneath it.
    pub cover: f32,
}

/// The shape in a region: coverage and signed distance to its edge (negative inside).
struct Shape {
    r: Reg,
    a: Vec<f32>,
    /// Signed distances, computed when an effect needs them.
    sd: std::sync::OnceLock<Vec<f32>>,
    /// The bounds of the shape (left, top, width, height) in canvas pixels.
    bounds: (f64, f64, f64, f64),
}

impl Shape {
    /// Signed distance to the shape's edge, negative inside.
    fn sd(&self) -> &[f32] {
        self.sd.get_or_init(|| {
            // Inside: any coverage (a faint layer has the same outline as an opaque one); the
            // edge pixels' coverage places the edge within them.
            let inside: Vec<bool> = self.a.iter().map(|&v| v > 1.0 / 512.0).collect();
            let outside: Vec<bool> = inside.iter().map(|&b| !b).collect();
            let d_out = edt(&inside, self.r.w, self.r.h);
            let d_in = edt(&outside, self.r.w, self.r.h);
            self.a
                .iter()
                .zip(&inside)
                .zip(d_out.iter().zip(&d_in))
                .map(|((&cov, &ins), (&o, &i))| {
                    if !ins {
                        o - 0.5
                    } else if i <= 1.0 {
                        0.5 - cov.min(1.0)
                    } else {
                        -(i - 0.5)
                    }
                })
                .collect()
        })
    }
}

/// Squared Euclidean distance transform of one line (Felzenszwalb and Huttenlocher's lower
/// envelope of parabolas), in place.
fn edt1(f: &mut [f32], v: &mut [usize], z: &mut [f32], d: &mut [f32]) {
    let n = f.len();
    if n == 0 {
        return;
    }
    let mut k = 0usize;
    v[0] = 0;
    z[0] = f32::NEG_INFINITY;
    z[1] = f32::INFINITY;
    for q in 1..n {
        loop {
            let p = v[k];
            let s = ((f[q] + (q * q) as f32) - (f[p] + (p * p) as f32)) / (2.0 * (q - p) as f32);
            if s <= z[k] && k > 0 {
                k -= 1;
                continue;
            }
            if s <= z[k] {
                // k == 0: replace the only parabola.
                v[0] = q;
                z[0] = f32::NEG_INFINITY;
                z[1] = f32::INFINITY;
                break;
            }
            k += 1;
            v[k] = q;
            z[k] = s;
            z[k + 1] = f32::INFINITY;
            break;
        }
    }
    k = 0;
    for (q, dq) in d.iter_mut().enumerate().take(n) {
        while z[k + 1] < q as f32 {
            k += 1;
        }
        let p = v[k];
        let dx = q as f32 - p as f32;
        *dq = dx * dx + f[p];
    }
    f.copy_from_slice(&d[..n]);
}

/// Distance from each pixel to the nearest pixel where `inside` holds (0 there).
fn edt(inside: &[bool], w: usize, h: usize) -> Vec<f32> {
    const FAR: f32 = 1e12;
    let mut g: Vec<f32> = inside.iter().map(|&b| if b { 0.0 } else { FAR }).collect();
    let m = w.max(h);
    let (mut v, mut z, mut d, mut line) = (
        vec![0usize; m],
        vec![0f32; m + 1],
        vec![0f32; m],
        vec![0f32; m],
    );
    for x in 0..w {
        for y in 0..h {
            line[y] = g[y * w + x];
        }
        edt1(&mut line[..h], &mut v, &mut z, &mut d);
        for y in 0..h {
            g[y * w + x] = line[y];
        }
    }
    for row in g.chunks_mut(w) {
        edt1(row, &mut v, &mut z, &mut d);
    }
    g.iter_mut().for_each(|x| *x = x.sqrt());
    g
}

/// Three box passes along rows then columns (edges repeated): close to a Gaussian of
/// standard deviation `sigma`.
fn blur(a: &mut [f32], w: usize, h: usize, sigma: f64) {
    if sigma < 0.3 || w == 0 || h == 0 {
        return;
    }
    let r = ((sigma * sigma + 0.25).sqrt() - 0.5).round().max(1.0) as usize;
    let mut tmp = vec![0f32; w.max(h)];
    let mut line = vec![0f32; w.max(h)];
    let pass = |line: &mut [f32], tmp: &mut [f32]| {
        let n = line.len();
        let k = 1.0 / (2 * r + 1) as f32;
        for _ in 0..3 {
            let at = |i: isize| line[i.clamp(0, n as isize - 1) as usize];
            let mut s: f32 = (-(r as isize)..=r as isize).map(at).sum();
            for (i, t) in tmp.iter_mut().enumerate().take(n) {
                *t = s * k;
                s += at(i as isize + r as isize + 1) - at(i as isize - r as isize);
            }
            line.copy_from_slice(&tmp[..n]);
        }
    };
    for row in a.chunks_mut(w) {
        pass(row, &mut tmp[..w]);
    }
    for x in 0..w {
        for y in 0..h {
            line[y] = a[y * w + x];
        }
        pass(&mut line[..h], &mut tmp[..h]);
        for y in 0..h {
            a[y * w + x] = line[y];
        }
    }
}

/// `a` moved by (dx, dy) pixels, `fill` where it uncovers.
fn shift(a: &[f32], w: usize, h: usize, dx: i64, dy: i64, fill: f32) -> Vec<f32> {
    let mut out = vec![fill; a.len()];
    for y in 0..h as i64 {
        let sy = y - dy;
        if sy < 0 || sy >= h as i64 {
            continue;
        }
        for x in 0..w as i64 {
            let sx = x - dx;
            if sx >= 0 && sx < w as i64 {
                out[(y * w as i64 + x) as usize] = a[(sy * w as i64 + sx) as usize];
            }
        }
    }
    out
}

/// The shape grown by `r` pixels (choked for negative `r`), as coverage; under half a pixel,
/// the shape's own coverage `a`.
fn grow(s: &Shape, r: f64) -> Vec<f32> {
    if r.abs() < 0.5 {
        return s.a.clone();
    }
    let r = r as f32;
    s.sd()
        .iter()
        .map(|&s| (0.5 + r - s).clamp(0.0, 1.0))
        .collect()
}

impl Effects {
    /// How far past the shape the effects reach, in pixels.
    fn reach(&self) -> f64 {
        let k = self.scale;
        self.list
            .iter()
            .map(|e| {
                let d = &e.d;
                let size = d.num_or(b"blur", 0.0) * k;
                let dist = d.num_or(b"Dstn", 0.0) * k;
                let stroke = d.num_or(b"Sz  ", 0.0) * k;
                size * 1.5 + dist + stroke + 2.0
            })
            .fold(2.0, f64::max)
    }

    /// The shape's region and distances; `None` when it is empty.
    fn shape(&self, fx: &Fx, alpha: &[f32]) -> Option<Shape> {
        let (w, h) = (fx.w, fx.h);
        let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0, 0);
        for (y, row) in alpha.chunks(w).enumerate().take(h) {
            let Some(l) = row.iter().position(|&a| a > 0.0) else {
                continue;
            };
            let r = row.iter().rposition(|&a| a > 0.0).unwrap_or(l);
            x0 = x0.min(l);
            x1 = x1.max(r + 1);
            y0 = y0.min(y);
            y1 = y1.max(y + 1);
        }
        if x0 >= x1 {
            return None;
        }
        let bounds = (x0 as f64, y0 as f64, (x1 - x0) as f64, (y1 - y0) as f64);
        let m = self.reach().ceil() as usize;
        let r = Reg {
            x: x0.saturating_sub(m),
            y: y0.saturating_sub(m),
            w: (x1 + m).min(w) - x0.saturating_sub(m),
            h: (y1 + m).min(h) - y0.saturating_sub(m),
        };
        let mut a = vec![0f32; r.w * r.h];
        for y in 0..r.h {
            let src = &alpha[(y + r.y) * w + r.x..(y + r.y) * w + r.x + r.w];
            a[y * r.w..(y + 1) * r.w].copy_from_slice(src);
        }
        Some(Shape {
            r,
            a,
            sd: std::sync::OnceLock::new(),
            bounds,
        })
    }

    /// Draws the shadows and outer glows under the layer onto `dst`; `alpha` is the layer's
    /// shape over the canvas, `k` its opacity.
    pub(crate) fn draw_behind(&self, fx: &Fx, alpha: &[f32], dst: &mut Canvas, k: f32) {
        if !self.list.iter().any(|e| e.kind.behind()) {
            return;
        }
        let Some(s) = self.shape(fx, alpha) else {
            return;
        };
        for e in self.list.iter().filter(|e| e.kind.behind()) {
            self.draw_one(fx, e, &s, dst, k, false);
        }
    }

    /// Draws the effects on the layer's content `c` (its colour, transparency faded by its
    /// fill opacity, or not yet for interior effects blended as a group); `alpha` is the
    /// layer's shape before its opacities.
    pub(crate) fn draw_inside(&self, fx: &Fx, alpha: &[f32], c: &mut Canvas, which: Inside) {
        let pick = |k: Kind| {
            !k.behind()
                && match which {
                    Inside::All => true,
                    Inside::Interior => k.interior(),
                    Inside::Rest => !k.interior(),
                }
        };
        if !self.list.iter().any(|e| pick(e.kind)) {
            return;
        }
        let Some(s) = self.shape(fx, alpha) else {
            return;
        };
        for e in self.list.iter().filter(|e| pick(e.kind)) {
            self.draw_one(fx, e, &s, c, 1.0, false);
        }
    }

    /// Whether the layer has interior effects (overlays, satin, inner glow), and whether one
    /// of them has a blend mode other than normal.
    pub(crate) fn interior_blends(&self) -> (bool, bool) {
        let mut it = self.list.iter().filter(|e| e.kind.interior()).peekable();
        let any = it.peek().is_some();
        (
            any,
            it.any(|e| &mode_key(e.d.enum_value(b"Md  ")) != b"norm"),
        )
    }

    /// Draws the interior effects straight onto the backdrop `dst` the layer was drawn on,
    /// inside its shape `alpha`, at opacity `k`: each blends with the layer and what is
    /// under it (Photoshop, unless "Blend Interior Effects as Group").
    pub(crate) fn draw_interior_on_backdrop(
        &self,
        fx: &Fx,
        alpha: &[f32],
        dst: &mut Canvas,
        k: f32,
    ) {
        if !self.list.iter().any(|e| e.kind.interior()) {
            return;
        }
        let Some(s) = self.shape(fx, alpha) else {
            return;
        };
        for e in self.list.iter().filter(|e| e.kind.interior()) {
            self.draw_one(fx, e, &s, dst, k, true);
        }
    }

    fn colour(fx: &Fx, d: Option<&Desc>, cc: usize) -> Vec<f32> {
        let info = &fx.psd.info;
        d.and_then(|c| crate::fill::desc_channels(c, info.mode, cc, info.depth == 32))
            .unwrap_or_else(|| vec![0.0; cc])
    }

    /// Offset of light angle `deg` and distance `dist`, rounded to whole pixels.
    fn offset(&self, fx: &Fx, d: &Desc) -> (i64, i64) {
        let angle = if d.boolean(b"uglg").unwrap_or(true) {
            f64::from(fx.psd.resources.global_angle.unwrap_or(120))
        } else {
            d.num_or(b"lagl", 120.0)
        };
        Self::offset_at(angle, d.num_or(b"Dstn", 0.0) * self.scale)
    }

    /// Offset of light angle `angle` (degrees) and distance `dist`, in whole pixels.
    fn offset_at(angle: f64, dist: f64) -> (i64, i64) {
        let t = angle.to_radians();
        (
            (-t.cos() * dist).round() as i64,
            (t.sin() * dist).round() as i64,
        )
    }

    fn draw_one(&self, fx: &Fx, e: &Effect, s: &Shape, dst: &mut Canvas, k: f32, backdrop: bool) {
        let d = &e.d;
        // On the layer's content, or straight onto the backdrop (only inside the shape).
        let within = if backdrop {
            How::Over
        } else {
            How::Inside(&s.a)
        };
        let r = s.r;
        let (w, h) = (r.w, r.h);
        let cc = dst.cc;
        let mode = mode_key(d.enum_value(b"Md  "));
        let opacity = (d.num_or(b"Opct", 100.0) / 100.0) as f32 * k;
        let size = d.num_or(b"blur", 0.0) * self.scale;
        let spread = (d.num_or(b"Ckmt", 0.0) / 100.0).clamp(0.0, 1.0);
        let sigma = |sz: f64| sz / 2.0;
        let solid = |key: &[u8]| Self::colour(fx, d.obj(key), cc);
        match e.kind {
            Kind::DropShadow => {
                let mut m = grow(s, size * spread);
                blur(&mut m, w, h, sigma(size * (1.0 - spread)));
                let (dx, dy) = self.offset(fx, d);
                let mut m = shift(&m, w, h, dx, dy, 0.0);
                contour(d, b"TrnS", &mut m);
                if d.boolean(b"layerConceals").unwrap_or(true) {
                    m.iter_mut()
                        .zip(&s.a)
                        .for_each(|(v, a)| *v *= conceal(*a, fx.cover));
                }
                paint(
                    dst,
                    r,
                    &m,
                    &Src::Solid(solid(b"Clr ")),
                    &mode,
                    opacity,
                    fx,
                    How::Over,
                );
            }
            Kind::OuterGlow => {
                let mut m = grow(s, size * spread);
                if d.enum_value(b"GlwT") == Some(b"PrBL") {
                    let reach = (size * (1.0 - spread)).max(1e-3) as f32;
                    let base = (size * spread) as f32;
                    m = s
                        .sd()
                        .iter()
                        .map(|&v| (1.0 - (v - base).max(0.0) / reach).clamp(0.0, 1.0))
                        .collect();
                } else {
                    blur(&mut m, w, h, sigma(size * (1.0 - spread)));
                }
                // The contour's range: the glow is full up to it.
                let range = range(d);
                m.iter_mut()
                    .zip(&s.a)
                    .for_each(|(v, _)| *v = (*v / range).min(1.0));
                contour(d, b"TrnS", &mut m);
                m.iter_mut()
                    .zip(&s.a)
                    .for_each(|(v, a)| *v *= conceal(*a, fx.cover));
                let col = glow_colour(fx, d, cc);
                paint(dst, r, &m, &Src::Solid(col), &mode, opacity, fx, How::Over);
            }
            Kind::ColorOverlay => {
                paint(
                    dst,
                    r,
                    &s.a,
                    &Src::Solid(solid(b"Clr ")),
                    &mode,
                    opacity,
                    fx,
                    within,
                );
            }
            Kind::GradientOverlay => {
                if let Some(p) =
                    crate::fill::gradient_paint(fx.psd, d, cc, (r.x, r.y, w, h), s.bounds)
                {
                    paint(dst, r, &s.a, &Src::Planes(p), &mode, opacity, fx, within);
                }
            }
            Kind::PatternOverlay => {
                if let Some(p) =
                    crate::fill::pattern_paint(fx.psd, fx.data, d, cc, (r.x, r.y, w, h), fx.origin)
                {
                    paint(dst, r, &s.a, &Src::Planes(p), &mode, opacity, fx, within);
                }
            }
            Kind::InnerShadow => {
                // The outside, choked into the shape, softened and offset.
                let mut m: Vec<f32> = grow(s, -size * spread).iter().map(|v| 1.0 - v).collect();
                blur(&mut m, w, h, sigma(size * (1.0 - spread)));
                let (dx, dy) = self.offset(fx, d);
                let mut m = shift(&m, w, h, dx, dy, 1.0);
                contour(d, b"TrnS", &mut m);
                m.iter_mut().zip(&s.a).for_each(|(v, a)| *v *= a);
                paint(
                    dst,
                    r,
                    &m,
                    &Src::Solid(solid(b"Clr ")),
                    &mode,
                    opacity,
                    fx,
                    within,
                );
            }
            Kind::InnerGlow => {
                let mut m: Vec<f32> = grow(s, -size * spread).iter().map(|v| 1.0 - v).collect();
                blur(&mut m, w, h, sigma(size * (1.0 - spread)));
                let centre = d.enum_value(b"glwS") == Some(b"SrcC");
                let range = range(d);
                m.iter_mut().for_each(|v| {
                    let g = if centre { 1.0 - *v } else { *v };
                    *v = (g / range).min(1.0);
                });
                contour(d, b"TrnS", &mut m);
                m.iter_mut().zip(&s.a).for_each(|(v, a)| *v *= a);
                let col = glow_colour(fx, d, cc);
                paint(dst, r, &m, &Src::Solid(col), &mode, opacity, fx, within);
            }
            Kind::Satin => {
                // Two copies of the shape, half the distance each way along the angle,
                // softened; their difference (twice it, clamped: fitted to Photoshop's
                // merged images) through the contour, inverted unless asked not to.
                // Satin has no global light: its own angle.
                let (dx, dy) = Self::offset_at(
                    d.num_or(b"lagl", 19.0),
                    d.num_or(b"Dstn", 0.0) * self.scale / 2.0,
                );
                let mut a1 = shift(&s.a, w, h, dx, dy, 0.0);
                let mut a2 = shift(&s.a, w, h, -dx, -dy, 0.0);
                blur(&mut a1, w, h, sigma(size));
                blur(&mut a2, w, h, sigma(size));
                let mut m: Vec<f32> = a1
                    .iter()
                    .zip(&a2)
                    .map(|(p, q)| ((p - q).abs() * 2.0).min(1.0))
                    .collect();
                contour(d, b"MpgS", &mut m);
                let invert = d.boolean(b"Invr").unwrap_or(true);
                m.iter_mut().zip(&s.a).for_each(|(v, a)| {
                    *v = (if invert { 1.0 - *v } else { *v }) * a;
                });
                paint(
                    dst,
                    r,
                    &m,
                    &Src::Solid(solid(b"Clr ")),
                    &mode,
                    opacity,
                    fx,
                    within,
                );
            }
            Kind::Stroke => self.stroke(fx, d, s, dst, &mode, opacity),
            Kind::Bevel => self.bevel(fx, d, s, dst, k),
        }
    }

    fn stroke(&self, fx: &Fx, d: &Desc, s: &Shape, dst: &mut Canvas, mode: &[u8; 4], k: f32) {
        let r = s.r;
        let cc = dst.cc;
        let sz = (d.num_or(b"Sz  ", 3.0) * self.scale) as f32;
        let style = d.enum_value(b"Styl").unwrap_or(b"OutF");
        let (m, under): (Vec<f32>, bool) = match style {
            b"InsF" => (
                s.sd()
                    .iter()
                    .map(|&v| (0.5 + sz + v).clamp(0.0, 1.0) * (0.5 - v).clamp(0.0, 1.0))
                    .collect(),
                false,
            ),
            b"CtrF" => (
                s.sd()
                    .iter()
                    .map(|&v| (0.5 + sz * 0.5 - v.abs()).clamp(0.0, 1.0))
                    .collect(),
                false,
            ),
            _ => (
                s.sd()
                    .iter()
                    .map(|&v| (0.5 + sz - v).clamp(0.0, 1.0) * (0.5 + v).clamp(0.0, 1.0))
                    .collect(),
                true,
            ),
        };
        let src = match d.enum_value(b"PntT") {
            Some(b"GrFl") => {
                let g = d.obj(b"Grad").map(|_| d).and_then(|g| {
                    crate::fill::gradient_paint(fx.psd, g, cc, (r.x, r.y, r.w, r.h), s.bounds)
                });
                match g {
                    Some(p) => Src::Planes(p),
                    None => return,
                }
            }
            Some(b"Ptrn") => {
                match crate::fill::pattern_paint(
                    fx.psd,
                    fx.data,
                    d,
                    cc,
                    (r.x, r.y, r.w, r.h),
                    fx.origin,
                ) {
                    Some(p) => Src::Planes(p),
                    None => return,
                }
            }
            _ => Src::Solid(Self::colour(fx, d.obj(b"Clr "), cc)),
        };
        paint(
            dst,
            r,
            &m,
            &src,
            mode,
            k,
            fx,
            if under { How::Under } else { How::Over },
        );
    }

    fn bevel(&self, fx: &Fx, d: &Desc, s: &Shape, dst: &mut Canvas, k: f32) {
        let r = s.r;
        let (w, h) = (r.w, r.h);
        let cc = dst.cc;
        let size = (d.num_or(b"blur", 5.0) * self.scale).max(1.0) as f32;
        let depth = (d.num_or(b"srgR", 100.0) / 100.0) as f32;
        let style = d.enum_value(b"bvlS").unwrap_or(b"InrB");
        let smooth = d.enum_value(b"bvlT").is_none_or(|t| t == b"SfBL");
        // An emboss puts half its size on each side of the edge.
        let reach = if style == b"Embs" { size / 2.0 } else { size };
        let shape = d
            .boolean(b"useShape")
            .unwrap_or(false)
            .then(|| Contour::of(d, b"MpgS"))
            .flatten();
        // Height, 0..1. Smooth: the shape's coverage blurred (sigma 0.45 x size, fitted to
        // Photoshop's merged images: inner bevel and emboss at sizes 5 and 41), so thin
        // shapes become domes; chisel: the distance to the edge.
        let mut hgt: Vec<f32> = if smooth {
            let mut g = s.a.clone();
            blur(&mut g, w, h, f64::from(reach) * 0.45);
            g
        } else {
            s.sd()
                .iter()
                .map(|&v| match style {
                    b"OtrB" => (1.0 - v / size).clamp(0.0, 1.0),
                    b"Embs" => (0.5 - v / (2.0 * size)).clamp(0.0, 1.0),
                    b"PlEb" => 1.0 - (v.abs() / size).clamp(0.0, 1.0),
                    _ => (-v / size).clamp(0.0, 1.0),
                })
                .collect()
        };
        if let Some(c) = &shape {
            hgt.iter_mut().for_each(|v| *v = c.at(*v));
        }
        if smooth && style == b"PlEb" {
            // A pillow: the slope falls into the edge from both sides.
            hgt.iter_mut().for_each(|v| *v = (*v - 0.5).abs());
        }
        let soften = d.num_or(b"Sftn", 0.0) * self.scale;
        let angle = if d.boolean(b"uglg").unwrap_or(true) {
            f64::from(fx.psd.resources.global_angle.unwrap_or(120))
        } else {
            d.num_or(b"lagl", 120.0)
        }
        .to_radians();
        let alt = if d.boolean(b"uglg").unwrap_or(true) {
            f64::from(fx.psd.resources.global_altitude.unwrap_or(30))
        } else {
            d.num_or(b"Lald", 30.0)
        }
        .to_radians();
        let (lx, ly, lz) = (
            (alt.cos() * angle.cos()) as f32,
            (-alt.cos() * angle.sin()) as f32,
            alt.sin() as f32,
        );
        let up = if d.enum_value(b"bvlD") == Some(b"Out ") {
            -1.0
        } else {
            1.0
        };
        let kz = depth * if smooth { reach } else { size } * up;
        let gloss = Contour::of(d, b"TrnS");
        let flat = gloss.as_ref().map_or(lz, |c| c.at(lz));
        let at = |x: usize, y: usize| hgt[y.min(h - 1) * w + x.min(w - 1)];
        let mut hi = vec![0f32; w * h];
        let mut lo = vec![0f32; w * h];
        for y in 0..h {
            for x in 0..w {
                let gx = (at(x + 1, y) - at(x.saturating_sub(1), y)) * 0.5 * kz;
                let gy = (at(x, y + 1) - at(x, y.saturating_sub(1))) * 0.5 * kz;
                let (nx, ny, nz) = (-gx, -gy, 1.0f32);
                let len = (nx * nx + ny * ny + nz * nz).sqrt();
                let mut shade = (nx * lx + ny * ly + nz * lz) / len;
                if let Some(c) = &gloss {
                    shade = c.at(shade);
                }
                let i = y * w + x;
                if shade > flat {
                    hi[i] = (shade - flat) / (1.0 - flat).max(1e-3);
                } else {
                    lo[i] = (flat - shade) / flat.max(1e-3);
                }
            }
        }
        if soften > 0.0 {
            blur(&mut hi, w, h, soften / 2.0);
            blur(&mut lo, w, h, soften / 2.0);
        }
        // Where the bevel shows: inside, outside (outer bevel) or both (emboss).
        for (i, a) in s.a.iter().enumerate() {
            let m = match style {
                b"OtrB" => 1.0 - a,
                b"Embs" | b"PlEb" => 1.0,
                _ => *a,
            };
            hi[i] = hi[i].clamp(0.0, 1.0) * m;
            lo[i] = lo[i].clamp(0.0, 1.0) * m;
        }
        let hm = mode_key(d.enum_value(b"hglM").or(Some(b"Scrn")));
        let sm = mode_key(d.enum_value(b"sdwM").or(Some(b"Mltp")));
        let ho = (d.num_or(b"hglO", 75.0) / 100.0) as f32 * k;
        let so = (d.num_or(b"sdwO", 75.0) / 100.0) as f32 * k;
        let hc = Self::colour(fx, d.obj(b"hglC"), cc);
        let sc = Self::colour(fx, d.obj(b"sdwC"), cc);
        paint(dst, r, &lo, &Src::Solid(sc), &sm, so, fx, How::Over);
        paint(dst, r, &hi, &Src::Solid(hc), &hm, ho, fx, How::Over);
    }
}

/// An effect's contour (`TrnS`, `MpgS`: a curve through points in 0..255) as a table over
/// 0..1; `None` for the linear one.
struct Contour(Vec<f32>);

impl Contour {
    const N: usize = 1024;

    fn of(d: &Desc, key: &[u8]) -> Option<Contour> {
        let mut pts: Vec<(f64, f64)> = d
            .obj(key)?
            .list(b"Crv ")
            .iter()
            .filter_map(|v| match v {
                Value::Obj(p) => Some((p.num(b"Hrzn")? / 255.0, p.num(b"Vrtc")? / 255.0)),
                _ => None,
            })
            .collect();
        pts.sort_by(|a, b| a.0.total_cmp(&b.0));
        pts.dedup_by(|a, b| a.0 == b.0);
        if pts.len() < 2 || pts.iter().all(|p| (p.0 - p.1).abs() < 1e-6) {
            return None;
        }
        let f = crate::adjust::spline(&pts);
        Some(Contour(
            (0..=Self::N)
                .map(|i| f(i as f64 / Self::N as f64).clamp(0.0, 1.0) as f32)
                .collect(),
        ))
    }

    /// The curve at `v` (clamped to 0..1), interpolated.
    fn at(&self, v: f32) -> f32 {
        let x = v.clamp(0.0, 1.0) * Self::N as f32;
        let i = (x as usize).min(Self::N - 1);
        let t = x - i as f32;
        self.0[i] * (1.0 - t) + self.0[i + 1] * t
    }
}

/// Applies the contour under `key` to coverage `m`, when there is one.
fn contour(d: &Desc, key: &[u8], m: &mut [f32]) {
    if let Some(c) = Contour::of(d, key) {
        m.iter_mut().for_each(|v| *v = c.at(*v));
    }
}

/// How much of an effect under the layer stays where the layer's shape covers `a`: the layer
/// hides it there (it does not show through a faded fill), and the layer is then drawn over
/// it at alpha `a × cover` (its fill and opacity), so that cover is taken back out.
fn conceal(a: f32, cover: f32) -> f32 {
    let a = a.clamp(0.0, 1.0);
    let d = 1.0 - a * cover.clamp(0.0, 1.0);
    if d <= 1e-4 {
        1.0
    } else {
        ((1.0 - a) / d).min(1.0)
    }
}

/// A glow's contour range (`Inpr`), 0..1: the glow is full where its softened shape reaches
/// it.
fn range(d: &Desc) -> f32 {
    (d.num_or(b"Inpr", 50.0) / 100.0).clamp(0.01, 1.0) as f32
}

/// A glow's colour: its own, or its gradient's first stop.
fn glow_colour(fx: &Fx, d: &Desc, cc: usize) -> Vec<f32> {
    if let Some(c) = d.obj(b"Clr ") {
        return Effects::colour(fx, Some(c), cc);
    }
    let first = d.obj(b"Grad").and_then(|g| match g.list(b"Clrs").first() {
        Some(Value::Obj(s)) => s.obj(b"Clr ").cloned(),
        _ => None,
    });
    Effects::colour(fx, first.as_ref(), cc)
}

enum Src {
    /// One colour, the document's channels.
    Solid(Vec<f32>),
    /// Region-sized planes: colour channels then alpha.
    Planes(Vec<f32>),
}

/// How an effect is laid onto the layer.
#[derive(Clone, Copy)]
enum How<'a> {
    /// Over what is there.
    Over,
    /// Beneath what is there.
    Under,
    /// Inside the layer's shape (region-sized coverage): mixed with the content as if the
    /// shape were opaque, then cut back to it, so soft edges keep their transparency.
    Inside(&'a [f32]),
}

/// Draws an effect onto `dst` over region `r`: coverage `m` (region-sized) times `k`, colour
/// `src`, blend `mode`, laid on as `how` says. Rows are blended in parallel bands, then
/// written back.
#[allow(clippy::too_many_arguments, clippy::needless_range_loop)]
fn paint(
    dst: &mut Canvas,
    r: Reg,
    m: &[f32],
    src: &Src,
    mode: &[u8; 4],
    k: f32,
    fx: &Fx,
    how: How,
) {
    let under = matches!(how, How::Under);
    const BAND: usize = 64;
    let (cc, n, w) = (dst.cc, dst.n, fx.w);
    let cq = cc.min(16);
    let rn = r.w * r.h;
    let three = fx.rgb_like && cc >= 3 && blend3(mode, [0.0; 3], [0.0; 3]).is_some();
    for y0 in (0..r.h).step_by(BAND) {
        let y1 = (y0 + BAND).min(r.h);
        let v = &dst.v;
        // Per row: the new colour and alpha of each pixel (NaN alpha: unchanged).
        let rows: Vec<Vec<f32>> = (y0..y1)
            .into_par_iter()
            .map(|y| {
                let mut out = vec![f32::NAN; r.w * (cc + 1)];
                let mut cs = [0f32; 16];
                let mut cr = [0f32; 16];
                let mut cb = [0f32; 16];
                for x in 0..r.w {
                    let li = y * r.w + x;
                    let mut a_s = m[li] * k;
                    match src {
                        Src::Solid(c) => {
                            for (q, s) in cs.iter_mut().enumerate().take(cq) {
                                *s = c.get(q).copied().unwrap_or(0.0);
                            }
                        }
                        Src::Planes(p) => {
                            for (q, s) in cs.iter_mut().enumerate().take(cq) {
                                *s = p[q * rn + li];
                            }
                            a_s *= p[cc * rn + li];
                        }
                    }
                    // Inside the shape: both alphas relative to its coverage.
                    let sa = match how {
                        How::Inside(sh) => sh[li].clamp(0.0, 1.0),
                        _ => 1.0,
                    };
                    if sa <= 0.0 {
                        continue;
                    }
                    let a_s = (a_s / sa).clamp(0.0, 1.0);
                    if a_s <= 0.0 {
                        continue;
                    }
                    let gi = (y + r.y) * w + x + r.x;
                    for (q, b) in cb.iter_mut().enumerate().take(cq) {
                        *b = v[q * n + gi];
                    }
                    let a_b = (v[cc * n + gi] / sa).clamp(0.0, 1.0);
                    let o = &mut out[x * (cc + 1)..(x + 1) * (cc + 1)];
                    if under {
                        // The effect beneath: what is there stays on top.
                        let a_o = a_b + a_s * (1.0 - a_b);
                        for q in 0..cq {
                            o[q] = (a_b * cb[q] + (1.0 - a_b) * a_s * cs[q]) / a_o;
                        }
                        o[cc] = a_o;
                        continue;
                    }
                    if three {
                        let b3 = blend3(mode, [cb[0], cb[1], cb[2]], [cs[0], cs[1], cs[2]])
                            .unwrap_or([cs[0], cs[1], cs[2]]);
                        cr[..3].copy_from_slice(&b3);
                        cr[3..cq].copy_from_slice(&cs[3..cq]);
                    } else {
                        for q in 0..cq {
                            cr[q] = blend1(mode, cb[q], cs[q]);
                        }
                    }
                    let a_o = a_s + a_b * (1.0 - a_s);
                    for q in 0..cq {
                        let mixed = (1.0 - a_b) * cs[q] + a_b * cr[q];
                        o[q] = (a_s * mixed + (1.0 - a_s) * a_b * cb[q]) / a_o;
                    }
                    o[cc] = a_o * sa;
                }
                out
            })
            .collect();
        for (dy, row) in rows.iter().enumerate() {
            let y = y0 + dy;
            for x in 0..r.w {
                let o = &row[x * (cc + 1)..(x + 1) * (cc + 1)];
                if o[cc].is_nan() {
                    continue;
                }
                let gi = (y + r.y) * w + x + r.x;
                for q in 0..cq {
                    dst.v[q * n + gi] = o[q];
                }
                dst.v[cc * n + gi] = o[cc];
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distance_transform_is_euclidean() {
        let (w, h) = (7, 5);
        let mut inside = vec![false; w * h];
        inside[2 * w + 3] = true;
        let d = edt(&inside, w, h);
        assert_eq!(d[2 * w + 3], 0.0);
        assert!((d[2 * w + 6] - 3.0).abs() < 1e-5);
        assert!((d[0] - (9.0f32 + 4.0).sqrt()).abs() < 1e-5);
    }

    #[test]
    fn blur_keeps_the_total() {
        let (w, h) = (40, 30);
        let mut a = vec![0f32; w * h];
        a[15 * w + 20] = 1.0;
        blur(&mut a, w, h, 3.0);
        let total: f32 = a.iter().sum();
        assert!((total - 1.0).abs() < 1e-3, "{total}");
        assert!(a[15 * w + 20] > a[15 * w + 23]);
        assert!(a[15 * w + 23] > 0.0);
    }

    #[test]
    fn shift_moves_and_fills() {
        let a = vec![1.0, 2.0, 3.0, 4.0];
        assert_eq!(shift(&a, 2, 2, 1, 0, 9.0), vec![9.0, 1.0, 9.0, 3.0]);
        assert_eq!(shift(&a, 2, 2, 0, -1, 0.0), vec![3.0, 4.0, 0.0, 0.0]);
    }

    #[test]
    fn modes_map_to_layer_keys() {
        assert_eq!(&mode_key(Some(b"Mltp")), b"mul ");
        assert_eq!(&mode_key(Some(b"linearDodge")), b"lddg");
        assert_eq!(&mode_key(None), b"norm");
    }
}
