//! Adjustment layers in the composite built from the layers: their settings parsed from the
//! file (the binary records and descriptors of the Photoshop File Formats Specification) and
//! applied to the colours below them, in RGB and greyscale documents.
//!
//! Levels, curves, brightness/contrast, exposure, invert, posterize and threshold are
//! per-channel tables; hue/saturation, vibrance, colour balance, black & white, photo
//! filter, channel mixer, gradient map and selective colour work on whole pixels. Photoshop
//! does not publish its formulas: these follow the controls' documented meaning, so they
//! come close to Photoshop's merged image rather than match it exactly (see the crate's
//! oracle notes). Colour lookup layers name an outside 3D LUT and are not drawn.

use crate::desc::{self, Desc};
use crate::parse::Rd;
use crate::{ColorMode, Layer, Psd};

/// Entries of a channel table (values between them are interpolated).
const LUT: usize = 1024;

/// A channel table over 0..1.
#[derive(Clone, Debug)]
pub(crate) struct Lut(Vec<f32>);

impl Lut {
    fn from_fn(f: impl Fn(f64) -> f64) -> Lut {
        Lut((0..=LUT)
            .map(|i| f(i as f64 / LUT as f64).clamp(0.0, 1.0) as f32)
            .collect())
    }

    fn identity() -> Lut {
        Lut::from_fn(|t| t)
    }

    /// `self` after `first`.
    fn after(&self, first: &Lut) -> Lut {
        Lut(first.0.iter().map(|&v| self.get(v)).collect())
    }

    #[inline]
    fn get(&self, v: f32) -> f32 {
        let x = v.clamp(0.0, 1.0) * LUT as f32;
        let i = (x as usize).min(LUT - 1);
        let f = x - i as f32;
        self.0[i] + (self.0[i + 1] - self.0[i]) * f
    }
}

/// What an adjustment layer does to a colour.
#[derive(Clone, Debug)]
pub(crate) enum Adjust {
    /// One table per channel (red, green, blue; grey uses the first).
    Tables([Lut; 3]),
    /// White where the luma reaches the level (0..1), else black.
    Threshold(f32),
    HueSat(HueSat),
    Vibrance {
        vibrance: f32,
        saturation: f32,
    },
    Balance {
        /// Shadows, midtones, highlights: cyan–red, magenta–green, yellow–blue, −1..1.
        tones: [[f32; 3]; 3],
        preserve: bool,
    },
    BlackWhite {
        /// Red, yellow, green, cyan, blue, magenta weights.
        w: [f32; 6],
        tint: Option<[f32; 3]>,
    },
    PhotoFilter {
        color: [f32; 3],
        density: f32,
        preserve: bool,
    },
    Mixer {
        mono: bool,
        /// Per output channel: red, green, blue weights and a constant.
        rows: [[f32; 4]; 3],
    },
    GradientMap {
        ramp: Vec<(f32, [f32; 3])>,
        /// Placed by linear-light luminance.
        linear: bool,
    },
    Selective {
        absolute: bool,
        /// Reds, yellows, greens, cyans, blues, magentas, whites, neutrals, blacks: cyan,
        /// magenta, yellow, black, −1..1.
        ranges: [[f32; 4]; 9],
    },
}

#[derive(Clone, Debug)]
pub(crate) struct HueSat {
    colorize: Option<[f32; 3]>,
    master: [f32; 3],
    /// (begin ramp, begin, end, end ramp) in degrees, then hue, saturation, lightness.
    ranges: Vec<([f32; 4], [f32; 3])>,
}

/// Natural cubic spline through `pts` (x ascending), flat beyond the end points.
pub(crate) fn spline(pts: &[(f64, f64)]) -> impl Fn(f64) -> f64 + '_ {
    let n = pts.len();
    let mut m = vec![0f64; n];
    if n > 2 {
        // Tridiagonal system for the second derivatives (natural ends).
        let mut c = vec![0f64; n];
        let mut d = vec![0f64; n];
        for i in 1..n - 1 {
            let (h0, h1) = (pts[i].0 - pts[i - 1].0, pts[i + 1].0 - pts[i].0);
            let a = h0;
            let b = 2.0 * (h0 + h1);
            let r = 6.0 * ((pts[i + 1].1 - pts[i].1) / h1 - (pts[i].1 - pts[i - 1].1) / h0);
            let den = b - a * c[i - 1];
            c[i] = h1 / den;
            d[i] = (r - a * d[i - 1]) / den;
        }
        for i in (1..n - 1).rev() {
            m[i] = d[i] - c[i] * m[i + 1];
        }
    }
    move |x: f64| {
        if n == 0 {
            return x;
        }
        if x <= pts[0].0 || n == 1 {
            return pts[0].1;
        }
        if x >= pts[n - 1].0 {
            return pts[n - 1].1;
        }
        let k = pts.iter().position(|p| p.0 > x).unwrap_or(n - 1).max(1);
        let (x0, y0, x1, y1) = (pts[k - 1].0, pts[k - 1].1, pts[k].0, pts[k].1);
        let h = x1 - x0;
        let (a, b) = ((x1 - x) / h, (x - x0) / h);
        a * y0 + b * y1 + ((a * a * a - a) * m[k - 1] + (b * b * b - b) * m[k]) * h * h / 6.0
    }
}

/// A curve's table from its (input, output) points in 0..255.
fn curve_lut(mut pts: Vec<(f64, f64)>) -> Option<Lut> {
    pts.sort_by(|a, b| a.0.total_cmp(&b.0));
    pts.dedup_by(|a, b| a.0 == b.0);
    if pts.len() < 2 {
        return None;
    }
    let pts: Vec<(f64, f64)> = pts.iter().map(|p| (p.0 / 255.0, p.1 / 255.0)).collect();
    let s = spline(&pts);
    Some(Lut::from_fn(s))
}

/// One levels record: input black/white, output black/white (0..255), gamma.
fn levels_lut(r: [u16; 5]) -> Lut {
    let (ib, iw, ob, ow) = (
        f64::from(r[0]) / 255.0,
        f64::from(r[1]) / 255.0,
        f64::from(r[2]) / 255.0,
        f64::from(r[3]) / 255.0,
    );
    let g = (f64::from(r[4]) / 100.0).clamp(0.01, 9.99);
    let span = if (iw - ib).abs() > 1e-9 { iw - ib } else { 1.0 };
    Lut::from_fn(|t| {
        let v = ((t - ib) / span).clamp(0.0, 1.0).powf(1.0 / g);
        ob + (ow - ob) * v
    })
}

fn tables_from(composite: Option<Lut>, channels: [Option<Lut>; 3]) -> Adjust {
    let comp = composite.unwrap_or_else(Lut::identity);
    Adjust::Tables(channels.map(|c| match c {
        Some(c) => comp.after(&c),
        None => comp.clone(),
    }))
}

fn levels(b: &[u8]) -> Option<Adjust> {
    let mut r = Rd::new(b, 0);
    if r.u16()? != 2 {
        return None;
    }
    let mut recs = Vec::new();
    for _ in 0..4 {
        let rec = [r.u16()?, r.u16()?, r.u16()?, r.u16()?, r.u16()?];
        recs.push(levels_lut(rec));
    }
    let mut it = recs.into_iter();
    let comp = it.next();
    Some(tables_from(comp, [it.next(), it.next(), it.next()]))
}

fn curves(b: &[u8]) -> Option<Adjust> {
    let mut r = Rd::new(b, 0);
    r.u8()?;
    let version = r.u16()?;
    let mut luts: [Option<Lut>; 4] = [None, None, None, None];
    let read_pts = |r: &mut Rd| -> Option<Vec<(f64, f64)>> {
        let n = r.u16()? as usize;
        if n > 32 {
            return None;
        }
        let mut pts = Vec::with_capacity(n);
        for _ in 0..n {
            let out = f64::from(r.u16()?);
            let inp = f64::from(r.u16()?);
            pts.push((inp, out));
        }
        Some(pts)
    };
    match version {
        1 => {
            let bits = r.u32()?;
            for c in 0..32usize {
                if bits & (1 << c) != 0 {
                    let pts = read_pts(&mut r)?;
                    if let Some(l) = luts.get_mut(c) {
                        *l = curve_lut(pts);
                    }
                }
            }
        }
        4 => {
            let count = r.u32()?;
            for _ in 0..count.min(32) {
                let c = r.u16()? as usize;
                let pts = read_pts(&mut r)?;
                if c < 4 {
                    luts[c] = curve_lut(pts);
                }
            }
        }
        _ => return None,
    }
    // The extra `Crv ` section (version 4 records) replaces what the first part said.
    if version == 1 && r.bytes(4) == Some(b"Crv ") && r.u16() == Some(4) {
        let count = r.u32().unwrap_or(0);
        for _ in 0..count.min(32) {
            let Some(c) = r.u16() else { break };
            let Some(pts) = read_pts(&mut r) else { break };
            if (c as usize) < 4 {
                luts[c as usize] = curve_lut(pts);
            }
        }
    }
    let [comp, red, green, blue] = luts;
    Some(tables_from(comp, [red, green, blue]))
}

/// Brightness/contrast from `CgEd` (else the `brit` record, legacy).
fn brightness(psd_data: &[u8], l: &Layer) -> Option<Adjust> {
    let cg = l
        .block(b"CgEd")
        .and_then(|r| psd_data.get(r))
        .and_then(desc::versioned);
    let (b, c, legacy) = match &cg {
        Some(d) => (
            d.num_or(b"Brgh", 0.0),
            d.num_or(b"Cntr", 0.0),
            d.boolean(b"useLegacy").unwrap_or(false),
        ),
        None => {
            let body = psd_data.get(l.block(b"brit")?)?;
            let mut r = Rd::new(body, 0);
            (f64::from(r.i16()?), f64::from(r.i16()?), true)
        }
    };
    let lut = if legacy {
        // Brightness shifts every level; contrast scales about the middle.
        let k = if c >= 0.0 {
            1.0 / (1.0 - c.min(99.0) / 100.0)
        } else {
            1.0 + c / 100.0
        };
        Lut::from_fn(|t| (t + b / 255.0 - 0.5) * k + 0.5)
    } else {
        // Brightness lifts or lowers the midtones and keeps black and white; contrast is
        // an S curve about the middle grey.
        let bb = (b / 150.0).clamp(-1.0, 1.0);
        let lift: Vec<(f64, f64)> = vec![(0.0, 0.0), (0.5, 0.5 + 0.25 * bb), (1.0, 1.0)];
        let cc = (c / 100.0).clamp(-0.5, 1.0);
        let d = 0.12 * cc;
        let s_pts: Vec<(f64, f64)> =
            vec![(0.0, 0.0), (0.25, 0.25 - d), (0.75, 0.75 + d), (1.0, 1.0)];
        let lift_f = spline(&lift);
        let s_f = spline(&s_pts);
        Lut::from_fn(|t| s_f(lift_f(t).clamp(0.0, 1.0)))
    };
    Some(Adjust::Tables([lut.clone(), lut.clone(), lut]))
}

fn exposure(b: &[u8]) -> Option<Adjust> {
    let mut r = Rd::new(b, 0);
    r.u16()?;
    let f = |r: &mut Rd| -> Option<f64> { Some(f64::from(f32::from_bits(r.u32()?))) };
    let (ev, offset, gamma) = (f(&mut r)?, f(&mut r)?, f(&mut r)?);
    if !(ev.is_finite() && offset.is_finite() && gamma.is_finite()) {
        return None;
    }
    let g = gamma.clamp(0.01, 9.99);
    let gain = ev.clamp(-20.0, 20.0).exp2();
    // In linear light (the document's curve taken as gamma 2.2).
    let lut = Lut::from_fn(|t| {
        let lin = (t.powf(2.2) * gain + offset).clamp(0.0, 1.0);
        lin.powf(1.0 / g).powf(1.0 / 2.2)
    });
    Some(Adjust::Tables([lut.clone(), lut.clone(), lut]))
}

fn hue_sat(b: &[u8]) -> Option<Adjust> {
    let mut r = Rd::new(b, 0);
    if r.u16()? != 2 {
        return None;
    }
    let colorize = r.u8()? != 0;
    r.u8()?;
    let hsl = |r: &mut Rd| -> Option<[f32; 3]> {
        Some([
            f32::from(r.i16()?),
            f32::from(r.i16()?) / 100.0,
            f32::from(r.i16()?) / 100.0,
        ])
    };
    let col = hsl(&mut r)?;
    let master = hsl(&mut r)?;
    let mut ranges = Vec::new();
    for _ in 0..6 {
        let bounds = [
            f32::from(r.i16()?),
            f32::from(r.i16()?),
            f32::from(r.i16()?),
            f32::from(r.i16()?),
        ];
        let v = hsl(&mut r)?;
        if v != [0.0; 3] {
            ranges.push((bounds, v));
        }
    }
    Some(Adjust::HueSat(HueSat {
        colorize: colorize.then_some(col),
        master,
        ranges,
    }))
}

fn balance(b: &[u8]) -> Option<Adjust> {
    let mut r = Rd::new(b, 0);
    let mut tones = [[0f32; 3]; 3];
    for t in &mut tones {
        for v in t.iter_mut() {
            *v = f32::from(r.i16()?) / 100.0;
        }
    }
    let preserve = r.u8().unwrap_or(1) != 0;
    Some(Adjust::Balance { tones, preserve })
}

fn black_white(d: &Desc) -> Adjust {
    let w = [
        b"Rd  ".as_slice(),
        b"Yllw",
        b"Grn ",
        b"Cyn ",
        b"Bl  ",
        b"Mgnt",
    ]
    .map(|k| d.num(k).unwrap_or(50.0) as f32 / 100.0);
    let tint = d
        .boolean(b"useTint")
        .filter(|&t| t)
        .and_then(|_| d.obj(b"tintColor"))
        .and_then(crate::fill::desc_rgb);
    Adjust::BlackWhite { w, tint }
}

fn photo_filter(b: &[u8]) -> Option<Adjust> {
    let mut r = Rd::new(b, 0);
    let version = r.u16()?;
    let color = match version {
        3 => {
            let x = f64::from(r.i32()?) / 100.0;
            let y = f64::from(r.i32()?) / 100.0;
            let z = f64::from(r.i32()?) / 100.0;
            xyz_to_rgb(x, y, z)
        }
        2 => {
            let space = r.u16()?;
            let c = [r.u16()?, r.u16()?, r.u16()?, r.u16()?];
            crate::fill::space_rgb(space, c)?
        }
        _ => return None,
    };
    let density = (r.u32()? as f32 / 100.0).clamp(0.0, 1.0);
    let preserve = r.u8().unwrap_or(1) != 0;
    Some(Adjust::PhotoFilter {
        color,
        density,
        preserve,
    })
}

/// XYZ (D50-relative, 0..100) to sRGB.
fn xyz_to_rgb(x: f64, y: f64, z: f64) -> [f32; 3] {
    let f = |t: f64| {
        if t > 216.0 / 24389.0 {
            t.cbrt()
        } else {
            (24389.0 / 27.0 * t + 16.0) / 116.0
        }
    };
    let (fx, fy, fz) = (f(x / 96.42), f(y / 100.0), f(z / 82.49));
    let lab = crate::lab_to_srgb8(
        (116.0 * fy - 16.0) as f32,
        (500.0 * (fx - fy)) as f32,
        (200.0 * (fy - fz)) as f32,
    );
    lab.map(|v| f32::from(v) / 255.0)
}

fn mixer(b: &[u8]) -> Option<Adjust> {
    let mut r = Rd::new(b, 0);
    r.u16()?;
    let mono = r.u16()? != 0;
    let mut rows = [[0f32; 4]; 3];
    for row in &mut rows {
        let v = [r.i16()?, r.i16()?, r.i16()?, r.i16()?, r.i16()?];
        *row = [
            f32::from(v[0]) / 100.0,
            f32::from(v[1]) / 100.0,
            f32::from(v[2]) / 100.0,
            f32::from(v[4]) / 100.0,
        ];
    }
    Some(Adjust::Mixer { mono, rows })
}

fn gradient_map(b: &[u8]) -> Option<Adjust> {
    let mut r = Rd::new(b, 0);
    let version = r.u16()?;
    let reverse = r.u8()? != 0;
    r.u8()?;
    // Version 3 names its interpolation: "Perc" (perceptual) places colours by linear-light
    // luminance.
    let linear = version >= 3 && r.bytes(4)? == b"Perc";
    let n = r.u32()? as usize;
    r.skip(n as u64 * 2)?;
    let count = r.u16()? as usize;
    if count > 1024 {
        return None;
    }
    let mut stops = Vec::with_capacity(count);
    let mut mids = Vec::with_capacity(count);
    for _ in 0..count {
        let at = r.u32()? as f32 / 4096.0;
        let mid = r.u32()? as f32 / 100.0;
        let space = r.u16()?;
        let c = [r.u16()?, r.u16()?, r.u16()?, r.u16()?];
        r.u16()?;
        let rgb = crate::fill::space_rgb(space, c).unwrap_or([0.0; 3]);
        stops.push((at.clamp(0.0, 1.0), rgb));
        mids.push(mid.clamp(0.01, 0.99));
    }
    if stops.is_empty() {
        return None;
    }
    // Sampled once: 256 levels with the midpoints folded in.
    let mut ramp = Vec::with_capacity(257);
    for i in 0..=256 {
        let mut t = i as f32 / 256.0;
        if reverse {
            t = 1.0 - t;
        }
        let j = stops.iter().position(|s| s.0 > t);
        let c = match j {
            None => stops[stops.len() - 1].1,
            Some(0) => stops[0].1,
            Some(j) => {
                let (a, b) = (stops[j - 1], stops[j]);
                let u = if b.0 > a.0 {
                    (t - a.0) / (b.0 - a.0)
                } else {
                    0.0
                };
                let m = mids[j];
                let w = if u < m {
                    0.5 * u / m
                } else {
                    0.5 + 0.5 * (u - m) / (1.0 - m)
                };
                [0, 1, 2].map(|k| a.1[k] + (b.1[k] - a.1[k]) * w)
            }
        };
        ramp.push((i as f32 / 256.0, c));
    }
    Some(Adjust::GradientMap { ramp, linear })
}

fn selective(b: &[u8]) -> Option<Adjust> {
    let mut r = Rd::new(b, 0);
    r.u16()?;
    let absolute = r.u16()? == 1;
    r.skip(8)?;
    let mut ranges = [[0f32; 4]; 9];
    for g in &mut ranges {
        for v in g.iter_mut() {
            *v = f32::from(r.i16()?) / 100.0;
        }
    }
    Some(Adjust::Selective { absolute, ranges })
}

/// The adjustment layer `l` does, `None` when it is not drawn (colour lookup, an unknown or
/// damaged record, a colour mode adjustments are not drawn in).
pub(crate) fn parse(psd: &Psd, data: &[u8], l: &Layer) -> Option<Adjust> {
    if !supported_mode(psd.info.mode) {
        return None;
    }
    let get = |k: &[u8; 4]| l.block(k).and_then(|r| data.get(r));
    let key = adjustment_key(l)?;
    match key {
        b"levl" => levels(get(b"levl")?),
        b"curv" => curves(get(b"curv")?),
        b"brit" => brightness(data, l),
        b"expA" => exposure(get(b"expA")?),
        b"hue2" => hue_sat(get(b"hue2")?),
        b"blnc" => balance(get(b"blnc")?),
        b"blwh" => Some(black_white(&desc::versioned(get(b"blwh")?)?)),
        b"phfl" => photo_filter(get(b"phfl")?),
        b"mixr" => mixer(get(b"mixr")?),
        b"nvrt" => {
            let l = Lut::from_fn(|t| 1.0 - t);
            Some(Adjust::Tables([l.clone(), l.clone(), l]))
        }
        b"post" => {
            let n = f64::from(Rd::new(get(b"post")?, 0).u16()?.clamp(2, 255));
            let l = Lut::from_fn(|t| (t * n).floor().min(n - 1.0) / (n - 1.0));
            Some(Adjust::Tables([l.clone(), l.clone(), l]))
        }
        b"thrs" => {
            let level = f32::from(Rd::new(get(b"thrs")?, 0).u16()?.clamp(1, 255));
            Some(Adjust::Threshold(level / 255.0))
        }
        b"grdm" => gradient_map(get(b"grdm")?),
        b"clrL" => color_lookup(get(b"clrL")?),
        b"selc" => selective(get(b"selc")?),
        b"vibA" => {
            let d = desc::versioned(get(b"vibA")?)?;
            Some(Adjust::Vibrance {
                vibrance: d.num_or(b"vibrance", 0.0) as f32 / 100.0,
                saturation: d.num_or(b"Strt", 0.0) as f32 / 100.0,
            })
        }
        _ => None,
    }
}

/// A Color Lookup adjustment: a version word, then a descriptor (`16`). One that names no
/// lookup (no `LUT3DFileData` or `profile`) changes nothing and is drawn as such; a 3D LUT
/// or device-link profile is not drawn (no sample file to check one against).
fn color_lookup(b: &[u8]) -> Option<Adjust> {
    let mut r = Rd::new(b, 0);
    r.u16()?;
    if r.u32()? != 16 {
        return None;
    }
    let d = desc::descriptor(&mut r)?;
    if d.get(b"LUT3DFileData").is_some() || d.get(b"profile").is_some() {
        return None;
    }
    let l = Lut::from_fn(|t| t);
    Some(Adjust::Tables([l.clone(), l.clone(), l]))
}

/// Blocks kept in [`Layer::blocks`] besides adjustment settings.
pub(crate) const OTHER_BLOCKS: &[[u8; 4]] = &[
    *b"CgEd", *b"lfx2", *b"lmfx", *b"lfxs", *b"lrFX", *b"vstk", *b"artb", *b"artd", *b"abdd",
    *b"knko", *b"clbl", *b"infx", *b"tsly", *b"fxrp",
];

/// The key of layer `l`'s adjustment record, if it has one.
pub(crate) fn adjustment_key(l: &Layer) -> Option<&[u8; 4]> {
    l.blocks
        .iter()
        .map(|(k, _)| k)
        .find(|&k| !OTHER_BLOCKS.contains(k))
}

/// Colour modes whose adjustment layers are drawn.
pub(crate) fn supported_mode(mode: ColorMode) -> bool {
    matches!(mode, ColorMode::Rgb | ColorMode::Grayscale)
}

/// Photoshop's name of an adjustment (for the notes on what is not drawn).
pub(crate) fn name(key: &[u8; 4]) -> &'static str {
    match key {
        b"levl" => "Levels",
        b"curv" => "Curves",
        b"brit" => "Brightness/Contrast",
        b"expA" => "Exposure",
        b"vibA" => "Vibrance",
        b"hue " | b"hue2" => "Hue/Saturation",
        b"blnc" => "Color Balance",
        b"blwh" => "Black & White",
        b"phfl" => "Photo Filter",
        b"mixr" => "Channel Mixer",
        b"clrL" => "Color Lookup",
        b"nvrt" => "Invert",
        b"post" => "Posterize",
        b"thrs" => "Threshold",
        b"grdm" => "Gradient Map",
        b"selc" => "Selective Color",
        _ => "Adjustment",
    }
}

fn luma(c: [f32; 3]) -> f32 {
    0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2]
}

/// RGB → (hue 0..360, saturation, lightness), HSL.
fn to_hsl(c: [f32; 3]) -> [f32; 3] {
    let mx = c[0].max(c[1]).max(c[2]);
    let mn = c[0].min(c[1]).min(c[2]);
    let l = (mx + mn) * 0.5;
    let d = mx - mn;
    if d <= 1e-7 {
        return [0.0, 0.0, l];
    }
    let s = if l <= 0.5 {
        d / (mx + mn)
    } else {
        d / (2.0 - mx - mn)
    };
    let h = if mx == c[0] {
        (c[1] - c[2]) / d + if c[1] < c[2] { 6.0 } else { 0.0 }
    } else if mx == c[1] {
        (c[2] - c[0]) / d + 2.0
    } else {
        (c[0] - c[1]) / d + 4.0
    };
    [h * 60.0, s, l]
}

fn from_hsl(h: [f32; 3]) -> [f32; 3] {
    let [hu, s, l] = h;
    if s <= 0.0 {
        return [l; 3];
    }
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    let f = |t: f32| {
        let t = t.rem_euclid(1.0);
        if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        }
    };
    let hh = hu / 360.0;
    [f(hh + 1.0 / 3.0), f(hh), f(hh - 1.0 / 3.0)]
}

/// Saturation scaled by `s` (−1..1): towards grey, or towards full colour.
fn saturate(sat: f32, s: f32) -> f32 {
    if s >= 0.0 {
        (sat / (1.0 - s).max(1e-3)).min(1.0)
    } else {
        sat * (1.0 + s)
    }
}

fn lighten(c: [f32; 3], l: f32) -> [f32; 3] {
    if l >= 0.0 {
        c.map(|v| v + (1.0 - v) * l)
    } else {
        c.map(|v| v * (1.0 + l))
    }
}

/// How much of a range of hues (degrees, ramps either side) hue `h` falls in, 0..1.
fn hue_weight(b: [f32; 4], h: f32) -> f32 {
    let rel = |x: f32| (x - b[0]).rem_euclid(360.0);
    let (b1, b2, b3) = (rel(b[1]), rel(b[2]), rel(b[3]));
    let x = rel(h);
    if x <= b1 {
        if b1 > 0.0 { x / b1 } else { 1.0 }
    } else if x <= b2 {
        1.0
    } else if x <= b3 {
        if b3 > b2 { (b3 - x) / (b3 - b2) } else { 0.0 }
    } else {
        0.0
    }
}

/// Lightness kept: `c` moved to `l`'s luma.
fn keep_luma(c: [f32; 3], target: f32) -> [f32; 3] {
    let d = target - luma(c);
    c.map(|v| (v + d).clamp(0.0, 1.0))
}

impl Adjust {
    /// The adjusted colour (RGB, 0..1).
    pub(crate) fn apply(&self, c: [f32; 3]) -> [f32; 3] {
        match self {
            Adjust::Tables(t) => [t[0].get(c[0]), t[1].get(c[1]), t[2].get(c[2])],
            Adjust::Threshold(level) => {
                // On the 0..255 levels Photoshop compares.
                let l = (luma(c) * 255.0).round();
                if l >= level * 255.0 - 0.01 {
                    [1.0; 3]
                } else {
                    [0.0; 3]
                }
            }
            Adjust::HueSat(hs) => {
                if let Some([h, s, l]) = hs.colorize {
                    let base = to_hsl(c)[2];
                    let lum = if l >= 0.0 {
                        base + (1.0 - base) * l
                    } else {
                        base * (1.0 + l)
                    };
                    return from_hsl([h, s.clamp(0.0, 1.0), lum]);
                }
                let c = lighten(c, hs.master[2]);
                let mut hsl = to_hsl(c);
                let (mut dh, mut ds, mut dl) = (hs.master[0], 0.0, 0.0);
                for (b, v) in &hs.ranges {
                    let w = hue_weight(*b, hsl[0]);
                    dh += w * v[0];
                    ds += w * v[1];
                    dl += w * v[2];
                }
                hsl[0] = (hsl[0] + dh).rem_euclid(360.0);
                hsl[1] = saturate(saturate(hsl[1], hs.master[1]), ds.clamp(-1.0, 1.0));
                let out = from_hsl(hsl);
                if dl != 0.0 {
                    lighten(out, (dl * hsl[1]).clamp(-1.0, 1.0))
                } else {
                    out
                }
            }
            Adjust::Vibrance {
                vibrance,
                saturation,
            } => {
                let mut hsl = to_hsl(c);
                let s = hsl[1];
                // Vibrance acts most on muted colours.
                let v = vibrance * (1.0 - s);
                hsl[1] = saturate(saturate(s, v.clamp(-1.0, 1.0)), *saturation);
                from_hsl(hsl)
            }
            Adjust::Balance { tones, preserve } => {
                let l = to_hsl(c)[2];
                let (a, b) = (0.25f32, 0.333f32);
                let sh = ((l - b) / -a + 0.5).clamp(0.0, 1.0) * 0.7;
                let mid = ((l - b) / a + 0.5).clamp(0.0, 1.0)
                    * ((l + b - 1.0) / -a + 0.5).clamp(0.0, 1.0)
                    * 0.7;
                let hi = ((l + b - 1.0) / a + 0.5).clamp(0.0, 1.0) * 0.7;
                let mut out = c;
                for (k, o) in out.iter_mut().enumerate() {
                    *o = (*o + tones[0][k] * sh + tones[1][k] * mid + tones[2][k] * hi)
                        .clamp(0.0, 1.0);
                }
                if *preserve {
                    let mut h = to_hsl(out);
                    h[2] = l;
                    from_hsl(h)
                } else {
                    out
                }
            }
            Adjust::BlackWhite { w, tint } => {
                let [r, g, b] = c;
                // The smallest channel, plus the mixed and primary hues' shares.
                let grey = if r >= g && g >= b {
                    b + (g - b) * w[1] + (r - g) * w[0]
                } else if r >= b && b >= g {
                    g + (b - g) * w[5] + (r - b) * w[0]
                } else if g >= r && r >= b {
                    b + (r - b) * w[1] + (g - r) * w[2]
                } else if g >= b && b >= r {
                    r + (b - r) * w[3] + (g - b) * w[2]
                } else if b >= g && g >= r {
                    r + (g - r) * w[3] + (b - g) * w[4]
                } else {
                    g + (r - g) * w[5] + (b - r) * w[4]
                }
                .clamp(0.0, 1.0);
                match tint {
                    Some(t) => keep_luma(*t, grey),
                    None => [grey; 3],
                }
            }
            Adjust::PhotoFilter {
                color,
                density,
                preserve,
            } => {
                let f = [0, 1, 2].map(|k| c[k] + (c[k] * color[k] - c[k]) * density);
                if *preserve { keep_luma(f, luma(c)) } else { f }
            }
            Adjust::Mixer { mono, rows } => {
                let mix =
                    |r: &[f32; 4]| (r[0] * c[0] + r[1] * c[1] + r[2] * c[2] + r[3]).clamp(0.0, 1.0);
                if *mono {
                    [mix(&rows[0]); 3]
                } else {
                    [mix(&rows[0]), mix(&rows[1]), mix(&rows[2])]
                }
            }
            Adjust::GradientMap { ramp, linear } => {
                let l = if *linear {
                    let lin = |v: f32| v.max(0.0).powf(2.2);
                    (0.2126 * lin(c[0]) + 0.7152 * lin(c[1]) + 0.0722 * lin(c[2])).min(1.0)
                } else {
                    0.3 * c[0] + 0.59 * c[1] + 0.11 * c[2]
                };
                let x = l.clamp(0.0, 1.0) * 256.0;
                let i = (x as usize).min(255);
                let f = x - i as f32;
                let (a, b) = (ramp[i].1, ramp[i + 1].1);
                [0, 1, 2].map(|k| a[k] + (b[k] - a[k]) * f)
            }
            Adjust::Selective { absolute, ranges } => selective_apply(c, *absolute, ranges),
        }
    }
}

fn selective_apply(c: [f32; 3], absolute: bool, ranges: &[[f32; 4]; 9]) -> [f32; 3] {
    let [r, g, b] = c;
    let mx = r.max(g).max(b);
    let mn = r.min(g).min(b);
    let mid = r + g + b - mx - mn;
    // How much of each colour range the pixel is.
    let mut w = [0f32; 9];
    if r == mx {
        w[0] = mx - mid;
    }
    if b == mn {
        w[1] = mid - mn;
    }
    if g == mx {
        w[2] = mx - mid;
    }
    if r == mn {
        w[3] = mid - mn;
    }
    if b == mx {
        w[4] = mx - mid;
    }
    if g == mn {
        w[5] = mid - mn;
    }
    // A primary range and the secondary it shares the pixel with: only the matching pair.
    if r == mx && b == mn {
        w[5] = 0.0;
    }
    w[6] = ((mn - 0.5) * 2.0).max(0.0);
    w[8] = ((0.5 - mx) * 2.0).max(0.0);
    w[7] = (1.0 - (mx - 0.5).abs() - (mn - 0.5).abs()).clamp(0.0, 1.0) * (1.0 - (mx - mn));
    let mut ink = [1.0 - r, 1.0 - g, 1.0 - b];
    for (k, i) in ink.iter_mut().enumerate() {
        let orig = *i;
        let mut d = 0.0;
        for (wi, a) in w.iter().zip(ranges) {
            if *wi <= 0.0 {
                continue;
            }
            // The colour's own ink, then black on every ink.
            let own = a[k];
            let black = a[3];
            let scale = if absolute { 1.0 } else { orig };
            d += wi * (own + black) * scale;
        }
        *i = (orig + d).clamp(0.0, 1.0);
    }
    ink.map(|v| 1.0 - v)
}
