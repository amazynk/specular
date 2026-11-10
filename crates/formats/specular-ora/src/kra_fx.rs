//! Krita filters (of filter layers and filter masks), fill-layer generators and the
//! hue/saturation/lightness family of composite ops.
//!
//! Filters work per pixel on straight colour in the image's coding (channel values
//! normalised to 0‥1 for integer pixels, as they are for float ones), as Krita's colour
//! transformations do. Their settings come from the node's `.filterconfig` member, an XML
//! list of `<param name="…">value</param>`. Only filters whose result does not depend on
//! neighbouring pixels are drawn; the rest are named in [`crate::Document::kra_notes`].
//!
//! The formulas follow Krita's manual and its documented colour models (HSV, HSL, HSI and
//! HSY with Rec. 601 luma); each was checked against Krita's own merged image of real
//! documents (`tests/kra_layered.rs`).

use quick_xml::Reader;
use quick_xml::events::Event;

/// The settings of a filter or generator: `(name, value)` pairs in file order.
pub(crate) type Params = Vec<(String, String)>;

/// Parses a `.filterconfig` member; at most 256 parameters of at most 1 MiB each.
pub(crate) fn params(xml: &[u8]) -> Params {
    let mut r = Reader::from_reader(xml);
    let mut buf = Vec::new();
    let mut out = Params::new();
    let mut name: Option<String> = None;
    let mut text = String::new();
    loop {
        match r.read_event_into(&mut buf) {
            Ok(Event::Start(e)) if e.local_name().as_ref() == b"param" => {
                name = e
                    .attributes()
                    .with_checks(false)
                    .flatten()
                    .find(|a| a.key.local_name().as_ref() == b"name")
                    .map(|a| String::from_utf8_lossy(&a.value).into_owned());
                text.clear();
            }
            Ok(Event::Text(t)) if name.is_some() => {
                if text.len() < 1 << 20 {
                    text.push_str(&String::from_utf8_lossy(t.as_ref()));
                }
            }
            Ok(Event::CData(t)) if name.is_some() => {
                if text.len() < 1 << 20 {
                    text.push_str(&String::from_utf8_lossy(t.as_ref()));
                }
            }
            Ok(Event::GeneralRef(t)) if name.is_some() => {
                let r: &[u8] = &t;
                let s = match r {
                    b"lt" => "<",
                    b"gt" => ">",
                    b"amp" => "&",
                    b"quot" => "\"",
                    b"apos" => "'",
                    _ => "",
                };
                text.push_str(s);
            }
            Ok(Event::End(e)) if e.local_name().as_ref() == b"param" => {
                if let Some(n) = name.take()
                    && out.len() < 256
                {
                    out.push((n, text.trim().to_owned()));
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    out
}

fn param<'a>(p: &'a Params, k: &str) -> Option<&'a str> {
    p.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str())
}

fn num(p: &Params, k: &str, default: f64) -> f64 {
    param(p, k)
        .and_then(|v| v.parse::<f64>().ok())
        .filter(|v| v.is_finite())
        .unwrap_or(default)
}

/// A filter drawn per pixel.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Fx {
    Invert,
    /// Krita's Gaussian blur: horizontal and vertical radius in pixels. Not per pixel: drawn
    /// with [`Fx::blur`] over rows extended by [`Fx::halo`].
    GaussianBlur {
        rx: f32,
        ry: f32,
    },
    /// Krita's desaturate types: lightness, Rec. 709 and 601 luminosity, average, min, max.
    Desaturate(u8),
    /// HSV adjustment: the colour model (0 HSV, 1 HSL, 2 HSI, 3 HSY, 4 YCbCr), the hue
    /// shift in degrees, saturation and value in −1‥1, and colorize.
    Hsv {
        model: u8,
        h: f32,
        s: f32,
        v: f32,
        colorize: bool,
        legacy: bool,
    },
    /// Per-channel curves as 4096-entry tables: the colour channels in display order (R, G,
    /// B, or grey) after the composite curve, then alpha.
    Curves {
        all: Option<Box<[f32]>>,
        colour: [Option<Box<[f32]>>; 3],
        alpha: Option<Box<[f32]>>,
    },
}

/// Lookup table size of a curve.
const LUT: usize = 4096;

impl Fx {
    /// The filter `name` with settings `p` (`version`: the node's `filterversion`), or why
    /// it is not drawn.
    pub(crate) fn new(name: &str, p: &Params, grey: bool) -> Result<Fx, &'static str> {
        Ok(match name {
            "invert" => Fx::Invert,
            "gaussian blur" => {
                let rx = num(p, "horizRadius", 5.0).clamp(0.0, 256.0) as f32;
                let ry = if param(p, "lockAspect") == Some("true") {
                    rx
                } else {
                    num(p, "vertRadius", 5.0).clamp(0.0, 256.0) as f32
                };
                Fx::GaussianBlur { rx, ry }
            }
            "desaturate" => Fx::Desaturate(num(p, "type", 0.0).clamp(0.0, 5.0) as u8),
            "hsvadjustment" => {
                if grey {
                    return Err("on a greyscale image");
                }
                Fx::Hsv {
                    model: num(p, "type", 1.0).clamp(0.0, 4.0) as u8,
                    h: num(p, "h", 0.0) as f32,
                    s: (num(p, "s", 0.0) / 100.0) as f32,
                    v: (num(p, "v", 0.0) / 100.0) as f32,
                    colorize: param(p, "colorize") == Some("true"),
                    // Krita 2 and 3 settings had no `type` and worked in HSV additively.
                    legacy: param(p, "type").is_none(),
                }
            }
            "perchannel" => curves(p, grey)?,
            _ => return Err("not drawn"),
        })
    }

    /// Rows (and columns) of context the filter reads around each pixel: 0 for a per-pixel
    /// filter.
    pub(crate) fn halo(&self) -> usize {
        match self {
            Fx::GaussianBlur { rx, ry } => gauss_half(rx.max(*ry)),
            _ => 0,
        }
    }

    /// Applies a filter with a [`Fx::halo`] to premultiplied pixels, `w` wide, edges
    /// repeated.
    pub(crate) fn blur(&self, px: &mut [f32], w: usize) {
        if let Fx::GaussianBlur { rx, ry } = self {
            let rows = px.len() / 4 / w.max(1);
            let mut tmp = vec![0f32; px.len()];
            convolve(px, &mut tmp, w, rows, &gauss_kernel(*rx), true);
            convolve(&tmp, px, w, rows, &gauss_kernel(*ry), false);
        }
    }

    /// Applies the filter to straight RGBA pixels.
    pub(crate) fn apply(&self, px: &mut [f32]) {
        match self {
            Fx::GaussianBlur { .. } => {}
            Fx::Invert => {
                for p in px.chunks_exact_mut(4) {
                    for c in &mut p[..3] {
                        *c = 1.0 - *c;
                    }
                }
            }
            Fx::Desaturate(t) => {
                for p in px.chunks_exact_mut(4) {
                    let (r, g, b) = (p[0], p[1], p[2]);
                    let y = match t {
                        0 => (max3(r, g, b) + min3(r, g, b)) * 0.5,
                        1 => 0.2126 * r + 0.7152 * g + 0.0722 * b,
                        2 => 0.299 * r + 0.587 * g + 0.114 * b,
                        3 => (r + g + b) / 3.0,
                        4 => min3(r, g, b),
                        _ => max3(r, g, b),
                    };
                    p[..3].fill(y);
                }
            }
            Fx::Hsv {
                model,
                h,
                s,
                v,
                colorize,
                legacy,
            } => {
                for p in px.chunks_exact_mut(4) {
                    let rgb =
                        hsv_adjust([p[0], p[1], p[2]], *model, *h, *s, *v, *colorize, *legacy);
                    p[..3].copy_from_slice(&rgb);
                }
            }
            Fx::Curves { all, colour, alpha } => {
                for p in px.chunks_exact_mut(4) {
                    for c in 0..3 {
                        let mut x = p[c];
                        if let Some(t) = &colour[c] {
                            x = lut(t, x);
                        }
                        if let Some(t) = all {
                            x = lut(t, x);
                        }
                        p[c] = x;
                    }
                    if let Some(t) = alpha {
                        p[3] = lut(t, p[3]);
                    }
                }
            }
        }
    }
}

/// Krita's Gaussian kernel for a blur radius: σ = 0.3·r + 0.3, 3·⌈σ⌉ taps each side.
fn gauss_half(r: f32) -> usize {
    if r <= 0.0 {
        return 0;
    }
    3 * (0.3 * r + 0.3).ceil() as usize
}

fn gauss_kernel(r: f32) -> Vec<f32> {
    let half = gauss_half(r);
    if half == 0 {
        return vec![1.0];
    }
    let s = 0.3 * r + 0.3;
    let k: Vec<f32> = (0..=2 * half)
        .map(|i| {
            let x = i as f32 - half as f32;
            (-x * x / (2.0 * s * s)).exp()
        })
        .collect();
    let sum: f32 = k.iter().sum();
    k.into_iter().map(|v| v / sum).collect()
}

/// One pass of a separable kernel over RGBA rows (`horizontal`) or columns, edges repeated.
fn convolve(src: &[f32], dst: &mut [f32], w: usize, rows: usize, k: &[f32], horizontal: bool) {
    use rayon::prelude::*;
    let half = (k.len() / 2) as isize;
    dst.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let mut acc = [0f32; 4];
            for (t, &kv) in k.iter().enumerate() {
                let d = t as isize - half;
                let (sx, sy) = if horizontal {
                    ((x as isize + d).clamp(0, w as isize - 1) as usize, y)
                } else {
                    (x, (y as isize + d).clamp(0, rows as isize - 1) as usize)
                };
                let p = &src[(sy * w + sx) * 4..][..4];
                for c in 0..4 {
                    acc[c] += p[c] * kv;
                }
            }
            row[x * 4..][..4].copy_from_slice(&acc);
        }
    });
}

#[inline]
fn max3(a: f32, b: f32, c: f32) -> f32 {
    a.max(b).max(c)
}

#[inline]
fn min3(a: f32, b: f32, c: f32) -> f32 {
    a.min(b).min(c)
}

#[inline]
fn lut(t: &[f32], x: f32) -> f32 {
    let f = x.clamp(0.0, 1.0) * (t.len() - 1) as f32;
    let i = (f as usize).min(t.len() - 2);
    let k = f - i as f32;
    t[i] + (t[i + 1] - t[i]) * k
}

/// The per-channel curves filter. Krita writes one curve per channel of the colour space in
/// display order (Krita 2 and 3), or, since Krita 4, a composite curve first and lightness,
/// hue and saturation curves after them (the lightness ones are not drawn).
fn curves(p: &Params, grey: bool) -> Result<Fx, &'static str> {
    let n = num(p, "nTransfers", 0.0) as usize;
    let get = |i: usize| -> Result<Option<Box<[f32]>>, &'static str> {
        let s = param(p, &format!("curve{i}")).unwrap_or("0,0;1,1;");
        let pts = parse_curve(s).ok_or("a curve that cannot be read")?;
        Ok(spline_lut(&pts))
    };
    let real = if grey { 2 } else { 4 };
    let (first, extra) = if n == real {
        (0, 0)
    } else if n > real {
        (1, n - real - 1)
    } else {
        return Err("a curve count that does not match the image");
    };
    let all = if first == 1 { get(0)? } else { None };
    let mut colour: [Option<Box<[f32]>>; 3] = [None, None, None];
    if grey {
        let y = get(first)?;
        colour = [y.clone(), y.clone(), y];
    } else {
        for (c, slot) in colour.iter_mut().enumerate() {
            *slot = get(first + c)?;
        }
    }
    let alpha = get(first + real - 1)?;
    for i in 0..extra {
        if get(first + real + i)?.is_some() {
            return Err("with hue, saturation or lightness curves");
        }
    }
    Ok(Fx::Curves { all, colour, alpha })
}

/// Curve points `x,y;x,y;…` (Krita 5 may add flags after the pair), sorted by x.
pub(crate) fn parse_curve(s: &str) -> Option<Vec<(f64, f64)>> {
    let mut pts = Vec::new();
    for item in s.split(';').map(str::trim).filter(|i| !i.is_empty()) {
        let mut it = item.split(',');
        let x: f64 = it.next()?.trim().parse().ok()?;
        let y: f64 = it.next()?.trim().parse().ok()?;
        if !x.is_finite() || !y.is_finite() || pts.len() >= 256 {
            return None;
        }
        pts.push((x.clamp(0.0, 1.0), y.clamp(0.0, 1.0)));
    }
    pts.sort_by(|a, b| a.0.total_cmp(&b.0));
    pts.dedup_by(|a, b| (a.0 - b.0).abs() < 1e-9);
    (!pts.is_empty()).then_some(pts)
}

/// A natural cubic spline through `pts` as a lookup table, clamped to 0‥1 and flat beyond
/// the end points; `None` for the identity.
pub(crate) fn spline_lut(pts: &[(f64, f64)]) -> Option<Box<[f32]>> {
    if pts.len() == 2 && pts[0] == (0.0, 0.0) && pts[1] == (1.0, 1.0) {
        return None;
    }
    let n = pts.len();
    // Second derivatives of the natural spline (tridiagonal solve).
    let mut m = vec![0.0f64; n];
    if n > 2 {
        let mut c = vec![0.0f64; n];
        let mut d = vec![0.0f64; n];
        for i in 1..n - 1 {
            let (h0, h1) = (pts[i].0 - pts[i - 1].0, pts[i + 1].0 - pts[i].0);
            let a = h0;
            let b = 2.0 * (h0 + h1);
            let cc = h1;
            let r = 6.0 * ((pts[i + 1].1 - pts[i].1) / h1 - (pts[i].1 - pts[i - 1].1) / h0);
            let den = b - a * c[i - 1];
            c[i] = cc / den;
            d[i] = (r - a * d[i - 1]) / den;
        }
        for i in (1..n - 1).rev() {
            m[i] = d[i] - c[i] * m[i + 1];
        }
    }
    let value = |x: f64| -> f64 {
        if n == 1 || x <= pts[0].0 {
            return pts[0].1;
        }
        if x >= pts[n - 1].0 {
            return pts[n - 1].1;
        }
        let k = pts.partition_point(|p| p.0 <= x).clamp(1, n - 1);
        let (x0, y0) = pts[k - 1];
        let (x1, y1) = pts[k];
        let h = x1 - x0;
        let (a, b) = ((x1 - x) / h, (x - x0) / h);
        a * y0 + b * y1 + ((a * a * a - a) * m[k - 1] + (b * b * b - b) * m[k]) * h * h / 6.0
    };
    Some(
        (0..LUT)
            .map(|i| value(i as f64 / (LUT - 1) as f64).clamp(0.0, 1.0) as f32)
            .collect(),
    )
}

/// The HSV adjustment of one straight RGB colour.
fn hsv_adjust(
    rgb: [f32; 3],
    model: u8,
    dh: f32,
    ds: f32,
    dv: f32,
    colorize: bool,
    legacy: bool,
) -> [f32; 3] {
    let shift = |h: f32| (h + dh / 360.0).rem_euclid(1.0);
    let light = |l: f32| {
        if dv < 0.0 {
            l * (dv + 1.0)
        } else {
            l + dv * (1.0 - l)
        }
    };
    if colorize {
        let y = 0.299 * rgb[0] + 0.587 * rgb[1] + 0.114 * rgb[2];
        let h = (dh / 360.0).rem_euclid(1.0);
        return hsl_to_rgb(h, ds.clamp(0.0, 1.0), light(y).clamp(0.0, 1.0));
    }
    if legacy || model == 0 {
        let (h, s, v) = rgb_to_hsv(rgb);
        return hsv_to_rgb(shift(h), (s + ds).clamp(0.0, 1.0), (v + dv).clamp(0.0, 1.0));
    }
    match model {
        1 => {
            let (h, s, l) = rgb_to_hsl(rgb);
            hsl_to_rgb(
                shift(h),
                (s * (ds + 1.0)).clamp(0.0, 1.0),
                light(l).clamp(0.0, 1.0),
            )
        }
        _ => {
            // HSI, HSY and YCbCr: hue turned about the grey axis, chroma scaled, the
            // model's lightness moved like HSL's.
            let w = match model {
                2 => [1.0 / 3.0; 3],
                _ => [0.299, 0.587, 0.114],
            };
            let y = w[0] * rgb[0] + w[1] * rgb[1] + w[2] * rgb[2];
            let mut c = [rgb[0] - y, rgb[1] - y, rgb[2] - y];
            let (sn, cs) = (dh.to_radians()).sin_cos();
            // Rotation about (1, 1, 1) (Rodrigues).
            let k = 1.0 / 3.0f32.sqrt();
            let dot = (c[0] + c[1] + c[2]) * k;
            let cross = [k * (c[2] - c[1]), k * (c[0] - c[2]), k * (c[1] - c[0])];
            for i in 0..3 {
                c[i] = c[i] * cs + cross[i] * sn + k * dot * (1.0 - cs);
            }
            let y2 = light(y);
            let s = (ds + 1.0).max(0.0);
            let cy = w[0] * c[0] + w[1] * c[1] + w[2] * c[2];
            [0, 1, 2].map(|i| ((c[i] - cy) * s + y2).clamp(0.0, 1.0))
        }
    }
}

pub(crate) fn rgb_to_hsv([r, g, b]: [f32; 3]) -> (f32, f32, f32) {
    let (mx, mn) = (max3(r, g, b), min3(r, g, b));
    let c = mx - mn;
    let s = if mx > 0.0 { c / mx } else { 0.0 };
    (hue(r, g, b, mx, c), s, mx)
}

fn hue(r: f32, g: f32, b: f32, mx: f32, c: f32) -> f32 {
    if c <= 0.0 {
        return 0.0;
    }
    let h = if mx == r {
        ((g - b) / c).rem_euclid(6.0)
    } else if mx == g {
        (b - r) / c + 2.0
    } else {
        (r - g) / c + 4.0
    };
    h / 6.0
}

pub(crate) fn hsv_to_rgb(h: f32, s: f32, v: f32) -> [f32; 3] {
    let c = v * s;
    from_chroma(h, c, v - c)
}

fn from_chroma(h: f32, c: f32, m: f32) -> [f32; 3] {
    let hp = h.rem_euclid(1.0) * 6.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r, g, b) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    [r + m, g + m, b + m]
}

pub(crate) fn rgb_to_hsl([r, g, b]: [f32; 3]) -> (f32, f32, f32) {
    let (mx, mn) = (max3(r, g, b), min3(r, g, b));
    let c = mx - mn;
    let l = (mx + mn) * 0.5;
    let d = 1.0 - (2.0 * l - 1.0).abs();
    let s = if d > 1e-6 { c / d } else { 0.0 };
    (hue(r, g, b, mx, c), s, l)
}

pub(crate) fn hsl_to_rgb(h: f32, s: f32, l: f32) -> [f32; 3] {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    from_chroma(h, c, l - c * 0.5)
}

/// Krita's colour models of the hue/saturation/lightness composite ops.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hsx {
    /// Rec. 601 luma (Krita's plain `hue`, `saturation`, `color`, `luminize`).
    Y,
    I,
    L,
    V,
}

/// What a hue/saturation/lightness composite op takes from the layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HsxOp {
    Hue,
    Saturation,
    Color,
    Lightness,
    IncSaturation,
    DecSaturation,
    IncLightness,
    DecLightness,
}

impl Hsx {
    /// From a Krita composite op id.
    pub(crate) fn from_id(id: &str) -> Option<(Hsx, HsxOp)> {
        use HsxOp::*;
        let (stem, model) = match id.rsplit_once('_') {
            Some((s, "hsv")) => (s, Hsx::V),
            Some((s, "hsl")) => (s, Hsx::L),
            Some((s, "hsi")) => (s, Hsx::I),
            _ => (id, Hsx::Y),
        };
        Some(match (stem, model) {
            ("hue", m) => (m, Hue),
            ("saturation", m) => (m, Saturation),
            ("color", m) => (m, Color),
            ("inc_saturation", m) => (m, IncSaturation),
            ("dec_saturation", m) => (m, DecSaturation),
            ("luminize", Hsx::Y) => (Hsx::Y, Lightness),
            ("inc_luminosity", Hsx::Y) => (Hsx::Y, IncLightness),
            ("dec_luminosity", Hsx::Y) => (Hsx::Y, DecLightness),
            ("value", Hsx::Y) => (Hsx::V, Lightness),
            ("inc_value", Hsx::Y) => (Hsx::V, IncLightness),
            ("dec_value", Hsx::Y) => (Hsx::V, DecLightness),
            ("lightness", Hsx::Y) => (Hsx::L, Lightness),
            ("inc_lightness", Hsx::Y) => (Hsx::L, IncLightness),
            ("dec_lightness", Hsx::Y) => (Hsx::L, DecLightness),
            ("intensity", Hsx::Y) => (Hsx::I, Lightness),
            ("inc_intensity", Hsx::Y) => (Hsx::I, IncLightness),
            ("dec_intensity", Hsx::Y) => (Hsx::I, DecLightness),
            _ => return None,
        })
    }

    fn light(self, [r, g, b]: [f32; 3]) -> f32 {
        match self {
            Hsx::Y => 0.299 * r + 0.587 * g + 0.114 * b,
            Hsx::I => (r + g + b) / 3.0,
            Hsx::L => (max3(r, g, b) + min3(r, g, b)) * 0.5,
            Hsx::V => max3(r, g, b),
        }
    }

    fn sat(self, c: [f32; 3]) -> f32 {
        let (mx, mn) = (max3(c[0], c[1], c[2]), min3(c[0], c[1], c[2]));
        let chroma = mx - mn;
        match self {
            Hsx::Y => chroma,
            Hsx::I => {
                if chroma > 1e-6 {
                    1.0 - mn / self.light(c)
                } else {
                    0.0
                }
            }
            Hsx::L => {
                let d = 1.0 - (2.0 * self.light(c) - 1.0).abs();
                if d > 1e-6 { chroma / d } else { 0.0 }
            }
            Hsx::V => {
                if mx > 1e-6 {
                    chroma / mx
                } else {
                    0.0
                }
            }
        }
    }

    /// The colour moved by `delta` in this model's lightness, brought back into gamut
    /// towards its lightness (W3C ClipColor with the model's lightness).
    fn add_light(self, c: [f32; 3], delta: f32) -> [f32; 3] {
        let c = c.map(|v| v + delta);
        let l = self.light(c);
        let (mn, mx) = (min3(c[0], c[1], c[2]), max3(c[0], c[1], c[2]));
        let mut c = c;
        if mn < 0.0 && l - mn > 1e-6 {
            c = c.map(|v| l + (v - l) * l / (l - mn));
        }
        if mx > 1.0 && mx - l > 1e-6 {
            c = c.map(|v| l + (v - l) * (1.0 - l) / (mx - l));
        }
        c
    }

    fn set_light(self, c: [f32; 3], l: f32) -> [f32; 3] {
        self.add_light(c, l - self.light(c))
    }

    /// W3C SetSat: the colour's chroma set to `s`, its hue kept.
    fn set_sat(c: [f32; 3], s: f32) -> [f32; 3] {
        let mut idx = [0usize, 1, 2];
        idx.sort_by(|&a, &b| c[a].total_cmp(&c[b]));
        let (lo, mid, hi) = (idx[0], idx[1], idx[2]);
        let chroma = c[hi] - c[lo];
        let mut out = [0.0; 3];
        if chroma > 1e-6 {
            out[mid] = (c[mid] - c[lo]) * s / chroma;
            out[hi] = s;
        }
        out
    }

    /// `B(s, d)` of op `op` on straight colours (source `s` over backdrop `d`).
    pub(crate) fn mix(self, op: HsxOp, s: [f32; 3], d: [f32; 3]) -> [f32; 3] {
        let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
        match op {
            HsxOp::Hue => {
                let c = Hsx::set_sat(s, self.sat(d));
                self.set_light(c, self.light(d))
            }
            HsxOp::Saturation => {
                let c = Hsx::set_sat(d, self.sat(s));
                self.set_light(c, self.light(d))
            }
            HsxOp::Color => self.set_light(s, self.light(d)),
            HsxOp::Lightness => self.set_light(d, self.light(s)),
            HsxOp::IncSaturation => {
                let c = Hsx::set_sat(d, lerp(self.sat(d), 1.0, self.sat(s)));
                self.set_light(c, self.light(d))
            }
            HsxOp::DecSaturation => {
                let c = Hsx::set_sat(d, lerp(0.0, self.sat(d), self.sat(s)));
                self.set_light(c, self.light(d))
            }
            HsxOp::IncLightness => self.add_light(d, self.light(s)),
            HsxOp::DecLightness => self.add_light(d, self.light(s) - 1.0),
        }
    }
}

/// A fill layer's generator.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Generator {
    /// One colour, straight RGBA in the image's coding.
    Color([f32; 4]),
}

impl Generator {
    /// The generator `name` with settings `p`; `linear`: the image's profile is linear.
    pub(crate) fn new(name: &str, p: &Params, linear: bool) -> Result<Generator, &'static str> {
        match name {
            "color" => {
                let v = param(p, "color").ok_or("without a colour")?;
                color(v, linear)
                    .map(Generator::Color)
                    .ok_or("with a colour that cannot be read")
            }
            "pattern" => Err("pattern fill: the pattern is not stored in the file"),
            "gradient" => Err("gradient fill"),
            _ => Err("not drawn"),
        }
    }
}

/// A colour setting: `#rrggbb` (sRGB), or Krita's colour XML
/// (`<color><RGB r= g= b= space=/></color>`, `<Gray g=/>`), converted to the image's coding
/// (`linear`: a linear-light profile; sRGB-coded otherwise).
pub(crate) fn color(v: &str, linear: bool) -> Option<[f32; 4]> {
    let v = v.trim();
    // Some Krita versions escaped the colour XML once more inside the CDATA.
    let unescaped;
    let v = if v.starts_with("&lt;") {
        unescaped = v
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&apos;", "'")
            .replace("&amp;", "&");
        unescaped.as_str()
    } else {
        v
    };
    if let Some(hex) = v.strip_prefix('#') {
        let n = u32::from_str_radix(hex.get(..6)?, 16).ok()?;
        let c = [(n >> 16) & 255, (n >> 8) & 255, n & 255].map(|c| c as f32 / 255.0);
        let c = if linear { c.map(srgb_to_linear) } else { c };
        return Some([c[0], c[1], c[2], 1.0]);
    }
    let mut r = Reader::from_str(v);
    let mut buf = Vec::new();
    loop {
        match r.read_event_into(&mut buf) {
            Ok(Event::Start(e) | Event::Empty(e)) => {
                let tag = e.local_name().as_ref().to_vec();
                let a: Vec<(Vec<u8>, String)> = e
                    .attributes()
                    .with_checks(false)
                    .flatten()
                    .map(|a| {
                        (
                            a.key.local_name().as_ref().to_vec(),
                            String::from_utf8_lossy(&a.value).into_owned(),
                        )
                    })
                    .collect();
                // Written with the locale's decimal comma by some versions.
                let get = |k: &[u8]| {
                    a.iter()
                        .find(|(n, _)| n == k)
                        .and_then(|(_, v)| v.trim().replace(',', ".").parse::<f32>().ok())
                        .filter(|v| v.is_finite())
                };
                let space = a
                    .iter()
                    .find(|(n, _)| n == b"space")
                    .map(|(_, v)| v.to_ascii_lowercase())
                    .unwrap_or_default();
                let lin_in = space.contains("g10") || space.contains("linear");
                let conv = |c: f32| match (lin_in, linear) {
                    (true, false) => linear_to_srgb(c),
                    (false, true) => srgb_to_linear(c),
                    _ => c,
                };
                match tag.as_slice() {
                    b"RGB" => {
                        let c = [get(b"r")?, get(b"g")?, get(b"b")?].map(conv);
                        return Some([c[0], c[1], c[2], 1.0]);
                    }
                    b"Gray" => {
                        let g = conv(get(b"g")?);
                        return Some([g, g, g, 1.0]);
                    }
                    // Normalised L*a*b* (L 0‥1 for 0‥100, a and b centred on 0.5), D50.
                    b"Lab" => {
                        let (l, aa, bb) = (get(b"L")?, get(b"a")?, get(b"b")?);
                        let c = lab_to_srgb(l * 100.0, (aa - 0.5) * 255.0, (bb - 0.5) * 255.0);
                        let c = if linear { c.map(srgb_to_linear) } else { c };
                        return Some([c[0], c[1], c[2], 1.0]);
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) | Err(_) => return None,
            _ => {}
        }
        buf.clear();
    }
}

/// CIE L*a*b* (D50) to sRGB-coded RGB (Bradford-adapted to D65), clamped.
fn lab_to_srgb(l: f32, a: f32, b: f32) -> [f32; 3] {
    let fy = (l + 16.0) / 116.0;
    let (fx, fz) = (fy + a / 500.0, fy - b / 200.0);
    let f = |t: f32| {
        if t > 6.0 / 29.0 {
            t * t * t
        } else {
            3.0 * (6.0f32 / 29.0).powi(2) * (t - 4.0 / 29.0)
        }
    };
    let (x, y, z) = (0.9642 * f(fx), f(fy), 0.8249 * f(fz));
    // XYZ (D50) to linear sRGB, with the Bradford adaptation folded in.
    let r = 3.134_051_3 * x - 1.617_385_1 * y - 0.490_642_8 * z;
    let g = -0.978_788_3 * x + 1.916_142 * y + 0.033_449_5 * z;
    let bl = 0.071_942_7 * x - 0.228_951_4 * y + 1.405_276_3 * z;
    [r, g, bl].map(|c| linear_to_srgb(c.clamp(0.0, 1.0)))
}

pub(crate) fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

pub(crate) fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.max(0.0).powf(1.0 / 2.4) - 0.055
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn params_read_cdata_and_text() {
        let p = params(
            br#"<!DOCTYPE params><params version="1"><param name="h"><![CDATA[12]]></param>
            <param name="color" type="string">#ff0000</param></params>"#,
        );
        assert_eq!(param(&p, "h"), Some("12"));
        assert_eq!(
            color(param(&p, "color").unwrap(), false),
            Some([1.0, 0.0, 0.0, 1.0])
        );
    }

    #[test]
    fn spline_through_points() {
        assert!(spline_lut(&[(0.0, 0.0), (1.0, 1.0)]).is_none());
        let t = spline_lut(&parse_curve("0,0;0.5,0.75;1,1;").unwrap()).unwrap();
        assert!((lut(&t, 0.5) - 0.75).abs() < 1e-3);
        assert!(lut(&t, 0.25) > 0.25 && lut(&t, 0.25) < 0.75);
        let inv = spline_lut(&parse_curve("0,1;1,0").unwrap()).unwrap();
        assert!((lut(&inv, 0.3) - 0.7).abs() < 1e-3);
    }

    #[test]
    fn hsx_ops_keep_the_backdrop_lightness() {
        let (d, s) = ([0.2, 0.5, 0.8], [0.9, 0.1, 0.1]);
        for m in [Hsx::Y, Hsx::I, Hsx::L, Hsx::V] {
            let c = m.mix(HsxOp::Color, s, d);
            assert!((m.light(c) - m.light(d)).abs() < 1e-4, "{m:?}");
            let l = m.mix(HsxOp::Lightness, s, d);
            assert!((m.light(l) - m.light(s)).abs() < 1e-4, "{m:?}");
        }
        assert_eq!(Hsx::from_id("value"), Some((Hsx::V, HsxOp::Lightness)));
        assert_eq!(
            Hsx::from_id("saturation_hsi"),
            Some((Hsx::I, HsxOp::Saturation))
        );
        assert_eq!(Hsx::from_id("hue"), Some((Hsx::Y, HsxOp::Hue)));
    }

    #[test]
    fn hsv_round_trips() {
        for c in [[0.2f32, 0.5, 0.8], [1.0, 0.0, 0.0], [0.3, 0.3, 0.3]] {
            let (h, s, v) = rgb_to_hsv(c);
            let b = hsv_to_rgb(h, s, v);
            let (h, s, l) = rgb_to_hsl(c);
            let d = hsl_to_rgb(h, s, l);
            for i in 0..3 {
                assert!((b[i] - c[i]).abs() < 1e-5 && (d[i] - c[i]).abs() < 1e-5);
            }
        }
    }
}
