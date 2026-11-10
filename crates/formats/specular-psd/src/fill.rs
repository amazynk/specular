//! What fill layers paint, from their settings (`SoCo` solid colour, `GdFl` gradient, `PtFl`
//! pattern), for the composite built from the layers. A shape layer is a fill layer with a
//! vector mask; Photoshop usually stores its rendered pixels too, and those are used when
//! present ([`crate::compose`]).
//!
//! Colours convert naively between colour models (the composite is a preview). Gradients
//! follow Photoshop's layout: the ramp spans the canvas, or the shape's bounds when aligned
//! with the layer, scaled and turned by the angle; stops interpolate linearly with their
//! midpoints. Noise gradients are not drawn. Patterns tile from the canvas origin, shifted
//! by the phase and scaled (nearest sample); their angle is ignored.

use rayon::prelude::*;

use crate::desc::{self, Desc, Value};
use crate::parse::Rd;
use crate::{ColorMode, FillKind, Layer, Psd};

/// The plain colour model a descriptor colour is in.
#[derive(Clone, Copy, Debug)]
enum Col {
    /// 0..1.
    Rgb([f64; 3]),
    /// Luminance 0..1.
    Gray(f64),
    /// Ink 0..1.
    Cmyk([f64; 4]),
    /// L 0..100, a and b signed.
    Lab([f64; 3]),
}

fn hsb_to_rgb(h: f64, s: f64, v: f64) -> [f64; 3] {
    let h = (h.rem_euclid(360.0)) / 60.0;
    let (s, v) = (s.clamp(0.0, 1.0), v.clamp(0.0, 1.0));
    let c = v * s;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let (r, g, b) = match h as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    [r + m, g + m, b + m]
}

fn col(d: &Desc) -> Option<Col> {
    let n = |k: &[u8]| d.num(k);
    Some(match d.class.as_slice() {
        b"RGBC" => match (n(b"Rd  "), n(b"Grn "), n(b"Bl  ")) {
            (Some(r), Some(g), Some(b)) => Col::Rgb([r / 255.0, g / 255.0, b / 255.0]),
            _ => Col::Rgb([n(b"redFloat")?, n(b"greenFloat")?, n(b"blueFloat")?]),
        },
        b"HSBC" => Col::Rgb(hsb_to_rgb(
            n(b"H   ")?,
            n(b"Strt")? / 100.0,
            n(b"Brgh")? / 100.0,
        )),
        b"Grsc" => Col::Gray(1.0 - n(b"Gry ")? / 100.0),
        b"CMYC" => Col::Cmyk([
            n(b"Cyn ")? / 100.0,
            n(b"Mgnt")? / 100.0,
            n(b"Ylw ")? / 100.0,
            n(b"Blck")? / 100.0,
        ]),
        b"LbCl" => Col::Lab([n(b"Lmnc")?, n(b"A   ")?, n(b"B   ")?]),
        _ => return None,
    })
}

/// Whether fill layer `l` paints a noise gradient (not drawn).
pub(crate) fn noise_fill(data: &[u8], l: &Layer) -> bool {
    let Some((FillKind::Gradient, r)) = l.fill.clone() else {
        return false;
    };
    data.get(r)
        .and_then(desc::versioned)
        .and_then(|d| {
            d.obj(b"Grad")
                .and_then(|g| g.enum_value(b"GrdF"))
                .map(|f| f == b"ClNs")
        })
        .unwrap_or(false)
}

/// A descriptor colour as sRGB 0..1.
pub(crate) fn desc_rgb(d: &Desc) -> Option<[f32; 3]> {
    Some(col(d)?.rgb().map(|v| v.clamp(0.0, 1.0) as f32))
}

/// A binary colour record (space, four components) as sRGB 0..1: RGB, HSB, CMYK, Lab, grey.
pub(crate) fn space_rgb(space: u16, c: [u16; 4]) -> Option<[f32; 3]> {
    let u = |v: u16| f64::from(v) / 65535.0;
    let col = match space {
        0 => Col::Rgb([u(c[0]), u(c[1]), u(c[2])]),
        1 => Col::Rgb(hsb_to_rgb(u(c[0]) * 360.0, u(c[1]), u(c[2]))),
        2 => Col::Cmyk([1.0 - u(c[0]), 1.0 - u(c[1]), 1.0 - u(c[2]), 1.0 - u(c[3])]),
        7 => Col::Lab([
            f64::from(c[0]) / 100.0,
            f64::from(c[1] as i16) / 100.0,
            f64::from(c[2] as i16) / 100.0,
        ]),
        8 => Col::Gray(1.0 - f64::from(c[0]) / 10000.0),
        _ => return None,
    };
    Some(col.rgb().map(|v| v.clamp(0.0, 1.0) as f32))
}

fn lab_to_rgb(l: f64, a: f64, b: f64) -> [f64; 3] {
    let p = crate::lab_to_srgb8(l as f32, a as f32, b as f32);
    p.map(|v| f64::from(v) / 255.0)
}

fn srgb_to_linear(v: f64) -> f64 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

fn rgb_to_lab(c: [f64; 3]) -> [f64; 3] {
    let [r, g, b] = c.map(srgb_to_linear);
    // sRGB (D65) to XYZ, then to Lab against D50 as Photoshop's Lab uses (Bradford-adapted).
    let x = 0.4360747 * r + 0.3850649 * g + 0.1430804 * b;
    let y = 0.2225045 * r + 0.7168786 * g + 0.0606169 * b;
    let z = 0.0139322 * r + 0.0971045 * g + 0.7141733 * b;
    let f = |t: f64| {
        if t > 216.0 / 24389.0 {
            t.cbrt()
        } else {
            (24389.0 / 27.0 * t + 16.0) / 116.0
        }
    };
    let (fx, fy, fz) = (f(x / 0.9642), f(y), f(z / 0.8249));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

impl Col {
    fn rgb(self) -> [f64; 3] {
        match self {
            Col::Rgb(c) => c,
            Col::Gray(g) => [g; 3],
            Col::Cmyk([c, m, y, k]) => [
                (1.0 - c) * (1.0 - k),
                (1.0 - m) * (1.0 - k),
                (1.0 - y) * (1.0 - k),
            ],
            Col::Lab([l, a, b]) => lab_to_rgb(l, a, b),
        }
    }

    /// The document's channel values, 0..1 as stored (CMYK: 1 = no ink).
    fn channels(self, mode: ColorMode, cc: usize, linear: bool) -> Vec<f32> {
        let mut out: Vec<f64> = match mode {
            ColorMode::Cmyk => {
                let ink = match self {
                    Col::Cmyk(i) => i,
                    _ => {
                        let [r, g, b] = self.rgb();
                        let k = 1.0 - r.max(g).max(b);
                        let d = (1.0 - k).max(1e-9);
                        [(1.0 - r - k) / d, (1.0 - g - k) / d, (1.0 - b - k) / d, k]
                    }
                };
                ink.iter().map(|v| 1.0 - v).collect()
            }
            ColorMode::Lab => {
                let [l, a, b] = match self {
                    Col::Lab(c) => c,
                    _ => rgb_to_lab(self.rgb()),
                };
                vec![l / 100.0, (a + 128.0) / 255.0, (b + 128.0) / 255.0]
            }
            ColorMode::Rgb => self.rgb().to_vec(),
            _ => vec![match self {
                Col::Gray(g) => g,
                _ => {
                    let [r, g, b] = self.rgb();
                    0.299 * r + 0.587 * g + 0.114 * b
                }
            }],
        };
        if linear && matches!(mode, ColorMode::Rgb | ColorMode::Grayscale) {
            out.iter_mut().for_each(|v| *v = srgb_to_linear(*v));
        }
        let pad = if mode == ColorMode::Cmyk { 1.0 } else { 0.0 };
        out.resize(cc, pad);
        out.iter().map(|&v| v.clamp(0.0, 1.0) as f32).collect()
    }
}

/// One colour or opacity stop: location 0..1, midpoint 0..1, value.
struct Stop<T> {
    at: f64,
    mid: f64,
    v: T,
}

fn stops<T>(list: &[Value], f: impl Fn(&Desc) -> Option<T>) -> Vec<Stop<T>> {
    let mut out: Vec<Stop<T>> = Vec::new();
    for s in list {
        let Value::Obj(d) = s else { continue };
        let Some(v) = f(d) else { continue };
        let at = (d.num(b"Lctn").unwrap_or(0.0) / 4096.0).clamp(0.0, 1.0);
        let mid = (d.num(b"Mdpn").unwrap_or(50.0) / 100.0).clamp(0.01, 0.99);
        if out.last().is_some_and(|l| l.at == at) {
            out.pop();
        }
        out.push(Stop { at, mid, v });
    }
    out.sort_by(|a, b| a.at.total_cmp(&b.at));
    out
}

/// Where `t` falls between stops: (lower index, upper index, weight of the upper).
fn locate<T>(s: &[Stop<T>], t: f64) -> (usize, usize, f64) {
    if s.len() < 2 || t <= s[0].at {
        return (0, 0, 0.0);
    }
    let last = s.len() - 1;
    if t >= s[last].at {
        return (last, last, 0.0);
    }
    let j = s.iter().position(|x| x.at > t).unwrap_or(last);
    let (a, b) = (&s[j - 1], &s[j]);
    let u = if b.at > a.at {
        (t - a.at) / (b.at - a.at)
    } else {
        0.0
    };
    // The midpoint (of the lower stop's span) is where the mix is half and half.
    let m = b.mid;
    let w = if u < m {
        0.5 * u / m
    } else {
        0.5 + 0.5 * (u - m) / (1.0 - m)
    };
    (j - 1, j, w)
}

struct Gradient {
    style: Vec<u8>,
    angle: f64,
    scale: f64,
    reverse: bool,
    align: bool,
    offset: (f64, f64),
    colors: Vec<Stop<Vec<f32>>>,
    alphas: Vec<Stop<f64>>,
}

fn gradient(d: &Desc, mode: ColorMode, cc: usize, linear: bool) -> Option<Gradient> {
    let g = d.obj(b"Grad")?;
    if g.enum_value(b"GrdF").is_some_and(|f| f != b"CstS") {
        return None;
    }
    let colors = stops(g.list(b"Clrs"), |s| {
        let c = match s.enum_value(b"Type") {
            Some(b"FrgC") => Col::Gray(0.0),
            Some(b"BckC") => Col::Gray(1.0),
            _ => col(s.obj(b"Clr ")?)?,
        };
        Some(c.channels(mode, cc, linear))
    });
    if colors.is_empty() {
        return None;
    }
    let alphas = stops(g.list(b"Trns"), |s| {
        Some((s.num(b"Opct")? / 100.0).clamp(0.0, 1.0))
    });
    let ofs = d.obj(b"Ofst");
    Some(Gradient {
        style: d.enum_value(b"Type").unwrap_or(b"Lnr ").to_vec(),
        angle: d.num(b"Angl").unwrap_or(90.0),
        scale: d.num(b"Scl ").unwrap_or(100.0) / 100.0,
        reverse: d.boolean(b"Rvrs").unwrap_or(false),
        align: d.boolean(b"Algn").unwrap_or(true),
        offset: (
            ofs.and_then(|o| o.num(b"Hrzn")).unwrap_or(0.0) / 100.0,
            ofs.and_then(|o| o.num(b"Vrtc")).unwrap_or(0.0) / 100.0,
        ),
        colors,
        alphas,
    })
}

impl Gradient {
    /// Fills `out` (`cc` colour planes then alpha, `w`×`h`) over the box (left, top, width,
    /// height).
    fn draw(&self, out: &mut [f32], cc: usize, w: usize, h: usize, bx: (f64, f64, f64, f64)) {
        let n = w * h;
        let (bl, bt) = (bx.0, bx.1);
        let (bw, bh) = (bx.2.max(1.0), bx.3.max(1.0));
        let t = self.angle.rem_euclid(180.0);
        let ratio = if t <= 90.0 { t } else { 180.0 - t };
        let s = (self.scale * ((90.0 - ratio) / 90.0 * bw + ratio / 90.0 * bh)).max(1e-6);
        let step = |e: f64| {
            if e > 1.0 {
                2.0 * e / s / (e - 1.0)
            } else {
                0.0
            }
        };
        let (sx, sy) = (step(bw), step(bh));
        let th = self.angle.rem_euclid(360.0).to_radians();
        let (c, sn) = (th.cos(), th.sin());
        let ox = self.offset.0 * bw * 2.0 / s;
        let oy = self.offset.1 * bh * 2.0 / s;
        // Where each pixel falls on the ramp, rows in parallel.
        let mut zs = vec![0f32; n];
        zs.par_chunks_mut(w.max(1))
            .enumerate()
            .for_each(|(y, row)| {
                let yy = -bh / s + (y as f64 - bt) * sy - oy;
                for (x, zv) in row.iter_mut().enumerate() {
                    let xx = -bw / s + (x as f64 - bl) * sx - ox;
                    let z = match self.style.as_slice() {
                        b"Rdl " => (xx * xx + yy * yy).sqrt(),
                        b"Angl" => {
                            ((yy.atan2(xx).to_degrees() + self.angle).rem_euclid(360.0)) / 360.0
                        }
                        b"Rflc" => (c * xx - sn * yy).abs(),
                        b"Dmnd" => (c * xx - sn * yy).abs() + (sn * xx + c * yy).abs(),
                        _ => 0.5 * (c * xx - sn * yy + 1.0),
                    };
                    let z = z.clamp(0.0, 1.0);
                    *zv = if self.reverse { 1.0 - z } else { z } as f32;
                }
            });
        // The ramp sampled finely, then looked up per pixel.
        const STEPS: usize = 4096;
        let table: Vec<Vec<f32>> = (0..=cc)
            .map(|ch| {
                (0..=STEPS)
                    .map(|i| {
                        let z = i as f64 / STEPS as f64;
                        if ch < cc {
                            let (a, b, t) = locate(&self.colors, z);
                            let (va, vb) = (self.colors[a].v[ch], self.colors[b].v[ch]);
                            va + (vb - va) * t as f32
                        } else if self.alphas.is_empty() {
                            1.0
                        } else {
                            let (a, b, t) = locate(&self.alphas, z);
                            let (va, vb) = (self.alphas[a].v, self.alphas[b].v);
                            (va + (vb - va) * t) as f32
                        }
                    })
                    .collect()
            })
            .collect();
        for (plane, t) in out.chunks_mut(n).take(cc + 1).zip(&table) {
            plane.par_iter_mut().zip(&zs).for_each(|(o, &z)| {
                let x = z * STEPS as f32;
                let i = (x as usize).min(STEPS - 1);
                let f = x - i as f32;
                *o = t[i] + (t[i + 1] - t[i]) * f;
            });
        }
    }
}

/// A decoded pattern: `cc` planes (the document's model) and alpha, 0..1.
struct Pattern {
    w: usize,
    h: usize,
    planes: Vec<Vec<f32>>,
    alpha: Option<Vec<f32>>,
}

/// One virtual-memory-array channel: its samples as 0..1, `w`×`h`.
fn vma(r: &mut Rd) -> Option<Option<(usize, usize, Vec<f32>)>> {
    if r.u32()? == 0 {
        return Some(None);
    }
    let len = r.u32()? as usize;
    if len == 0 {
        return Some(None);
    }
    let end = r.pos.checked_add(len)?;
    let mut q = Rd::new(r.d.get(..end)?, r.pos);
    r.pos = end;
    if len < 23 {
        return None;
    }
    let _depth = q.u32()?;
    let (t, l, b, rt) = (q.i32()?, q.i32()?, q.i32()?, q.i32()?);
    let bits = q.u16()?;
    let comp = q.u8()?;
    let (w, h) = ((rt - l).max(0) as usize, (b - t).max(0) as usize);
    if w == 0 || h == 0 || w > 1 << 14 || h > 1 << 14 {
        return Some(None);
    }
    let bps = match bits {
        8 => 1,
        16 => 2,
        _ => return Some(None),
    };
    let raw_len = w.checked_mul(h)?.checked_mul(bps)?;
    // A channel claims no more than its stored bytes could unpack to.
    if raw_len > (end - q.pos).saturating_mul(128) + 1024 {
        return None;
    }
    let mut raw = vec![0u8; raw_len];
    match comp {
        0 => {
            let src = q.bytes(raw_len)?;
            raw.copy_from_slice(src);
        }
        1 => {
            let mut counts = Vec::with_capacity(h);
            for _ in 0..h {
                counts.push(q.u16()? as usize);
            }
            for (y, &c) in counts.iter().enumerate() {
                let src = q.bytes(c)?;
                crate::rle::unpack(src, &mut raw[y * w * bps..(y + 1) * w * bps]);
            }
        }
        _ => return Some(None),
    }
    let v = if bps == 1 {
        raw.iter().map(|&b| f32::from(b) / 255.0).collect()
    } else {
        raw.chunks_exact(2)
            .map(|c| f32::from(u16::from_be_bytes([c[0], c[1]])) / 65535.0)
            .collect()
    };
    Some(Some((w, h, v)))
}

/// The pattern record `rec` if its ID is `want`, decoded for a document in `mode`.
fn pattern(rec: &[u8], want: &str, mode: ColorMode, cc: usize) -> Option<Pattern> {
    let mut r = Rd::new(rec, 0);
    if r.u32()? != 1 {
        return None;
    }
    let pmode = r.u32()?;
    let _h = r.i16()?;
    let _w = r.i16()?;
    let n = r.u32()? as usize;
    r.skip(n as u64 * 2)?;
    let idl = r.u8()? as usize;
    let id = std::str::from_utf8(r.bytes(idl)?).ok()?;
    if id.trim_end_matches('\0') != want {
        return None;
    }
    let palette = if pmode == 2 {
        let p = r.bytes(768)?.to_vec();
        r.skip(4)?;
        Some(p)
    } else {
        None
    };
    let _version = r.u32()?;
    let _len = r.u32()?;
    r.skip(16)?;
    let count = r.u32()? as usize;
    if count > 64 {
        return None;
    }
    let mut chans = Vec::new();
    for _ in 0..count + 2 {
        chans.push(vma(&mut r)?);
    }
    let (w, h) = chans.iter().flatten().map(|c| (c.0, c.1)).next()?;
    let get = |i: usize| -> Option<Vec<f32>> {
        chans
            .get(i)?
            .as_ref()
            .filter(|c| c.0 == w && c.1 == h)
            .map(|c| c.2.clone())
    };
    // Slots: colour, a user mask, then transparency (the last one).
    let alpha = get(count + 1).or_else(|| get(count));
    // The pattern as plain colours, then in the document's model.
    let px = w * h;
    let mut cols = Vec::with_capacity(px);
    match pmode {
        1 | 8 => {
            let g = get(0)?;
            cols.extend(g.iter().map(|&v| Col::Gray(f64::from(v))));
        }
        2 => {
            let idx = get(0)?;
            let p = palette?;
            cols.extend(idx.iter().map(|&v| {
                let i = (v * 255.0).round() as usize * 3;
                Col::Rgb([p[i], p[i + 1], p[i + 2]].map(|c| f64::from(c) / 255.0))
            }));
        }
        // Multichannel: its first three inks shown as RGB, as the document's are.
        7 if get(2).is_none() => {
            let g = get(0)?;
            cols.extend(g.iter().map(|&v| Col::Gray(f64::from(v))));
        }
        3 | 7 => {
            let (r_, g, b) = (get(0)?, get(1)?, get(2)?);
            for i in 0..px {
                cols.push(Col::Rgb([r_[i], g[i], b[i]].map(f64::from)));
            }
        }
        4 => {
            let (c, m, y, k) = (get(0)?, get(1)?, get(2)?, get(3)?);
            for i in 0..px {
                cols.push(Col::Cmyk(
                    [c[i], m[i], y[i], k[i]].map(|v| 1.0 - f64::from(v)),
                ));
            }
        }
        9 => {
            let (l, a, b) = (get(0)?, get(1)?, get(2)?);
            for i in 0..px {
                cols.push(Col::Lab([
                    f64::from(l[i]) * 100.0,
                    f64::from(a[i]) * 255.0 - 128.0,
                    f64::from(b[i]) * 255.0 - 128.0,
                ]));
            }
        }
        _ => return None,
    }
    let mut planes = vec![vec![0f32; px]; cc];
    for (i, c) in cols.iter().enumerate() {
        for (k, v) in c.channels(mode, cc, false).into_iter().enumerate() {
            planes[k][i] = v;
        }
    }
    Some(Pattern {
        w,
        h,
        planes,
        alpha,
    })
}

/// The fill a fill layer paints over the whole canvas: `cc` colour planes then alpha
/// (`w`×`h`, straight), before its masks and opacities. `None` when it cannot be drawn
/// (unknown settings, a noise gradient, a missing pattern).
pub(crate) fn render(psd: &Psd, data: &[u8], layer: &Layer, cc: usize) -> Option<Vec<f32>> {
    let (kind, range) = layer.fill.clone()?;
    let d = desc::versioned(data.get(range)?)?;
    let info = &psd.info;
    let (w, h) = (info.width as usize, info.height as usize);
    let n = w * h;
    let linear = info.depth == 32;
    let mut out = vec![0f32; (cc + 1) * n];
    match kind {
        FillKind::Solid => {
            let c = col(d.obj(b"Clr ")?)?.channels(info.mode, cc, linear);
            for (ch, v) in c.iter().enumerate() {
                out[ch * n..(ch + 1) * n].fill(*v);
            }
            out[cc * n..].fill(1.0);
        }
        FillKind::Gradient => {
            let g = gradient(&d, info.mode, cc, linear)?;
            let mut bx = (0.0, 0.0, w as f64, h as f64);
            if g.align {
                let vm = layer
                    .vector_mask
                    .clone()
                    .and_then(|r| data.get(r))
                    .and_then(crate::vector::parse);
                if let Some((l, t, r, b)) = vm.and_then(|v| v.bounds(w, h))
                    && r - l >= 1.0
                    && b - t >= 1.0
                {
                    bx = (l.round(), t.round(), (r - l).round(), (b - t).round());
                }
            }
            g.draw(&mut out, cc, w, h, bx);
        }
        FillKind::Pattern => {
            let v = pattern_paint(
                psd,
                data,
                &d,
                cc,
                (0, 0, w, h),
                reference_point(data, layer),
            )?;
            out = v;
        }
    }
    Some(out)
}

/// Where layer `l`'s patterns are anchored when linked with it: its effects reference point
/// (`fxrp`), else the canvas origin.
pub(crate) fn reference_point(data: &[u8], l: &Layer) -> (f64, f64) {
    l.block(b"fxrp")
        .and_then(|r| data.get(r))
        .and_then(|b| {
            let x = f64::from_be_bytes(b.get(..8)?.try_into().ok()?);
            let y = f64::from_be_bytes(b.get(8..16)?.try_into().ok()?);
            (x.is_finite() && y.is_finite()).then_some((x, y))
        })
        .unwrap_or((0.0, 0.0))
}

/// A pattern fill's settings (`Ptrn`, `Scl `, `phase`, `Algn`) painted over the region (left,
/// top, width, height) of the canvas: `cc` planes then alpha, region-sized. Linked with the
/// layer (`Algn`), the tiles start at its reference point `origin`; scaled tiles are sampled
/// bilinearly.
pub(crate) fn pattern_paint(
    psd: &Psd,
    data: &[u8],
    d: &Desc,
    cc: usize,
    region: (usize, usize, usize, usize),
    origin: (f64, f64),
) -> Option<Vec<f32>> {
    let info = &psd.info;
    let p = d.obj(b"Ptrn")?;
    let want = p.text(b"Idnt")?.to_string();
    let pat = psd
        .patterns
        .iter()
        .filter_map(|r| data.get(r.clone()))
        .find_map(|rec| pattern(rec, &want, info.mode, cc))?;
    let scale = (d.num(b"Scl ").unwrap_or(100.0) / 100.0).max(0.01);
    let ph = d.obj(b"phase");
    let (ox, oy) = if d.boolean(b"Algn") == Some(true) {
        origin
    } else {
        (0.0, 0.0)
    };
    let px = ph.and_then(|p| p.num(b"Hrzn")).unwrap_or(0.0) + ox;
    let py = ph.and_then(|p| p.num(b"Vrtc")).unwrap_or(0.0) + oy;
    let (x0, y0, w, h) = region;
    let n = w * h;
    let (pw, ph) = (pat.w as i64, pat.h as i64);
    // Tile coordinate of canvas position `c` along one axis: cell and weight of the next one.
    let at = |c: f64, p: f64, len: i64| {
        let u = (c - p) / scale;
        let f = u.floor();
        let i = (f as i64).rem_euclid(len);
        let t = (u - f) as f32;
        (i as usize, ((i + 1) % len) as usize, t)
    };
    let cols: Vec<(usize, usize, f32)> = (0..w).map(|x| at((x + x0) as f64, px, pw)).collect();
    let mut out = vec![0f32; (cc + 1) * n];
    for y in 0..h {
        let (r0, r1, ty) = at((y + y0) as f64, py, ph);
        for (x, &(c0, c1, tx)) in cols.iter().enumerate() {
            let i = y * w + x;
            let s = |plane: &[f32]| {
                let (a, b) = (plane[r0 * pat.w + c0], plane[r0 * pat.w + c1]);
                let (c, e) = (plane[r1 * pat.w + c0], plane[r1 * pat.w + c1]);
                let top = a + (b - a) * tx;
                let bottom = c + (e - c) * tx;
                top + (bottom - top) * ty
            };
            for ch in 0..cc {
                out[ch * n + i] = s(&pat.planes[ch]);
            }
            out[cc * n + i] = pat.alpha.as_ref().map_or(1.0, |a| s(a));
        }
    }
    Some(out)
}

/// A gradient's settings (`Grad`, `Type`, `Angl`, `Scl `, `Rvrs`, `Algn`, `Ofst`) painted
/// over the region (left, top, width, height) of the canvas, the ramp laid over `bounds`
/// (left, top, width, height; the canvas when not aligned): `cc` planes then alpha.
pub(crate) fn gradient_paint(
    psd: &Psd,
    d: &Desc,
    cc: usize,
    region: (usize, usize, usize, usize),
    bounds: (f64, f64, f64, f64),
) -> Option<Vec<f32>> {
    let info = &psd.info;
    let g = gradient(d, info.mode, cc, info.depth == 32)?;
    let bx = if g.align {
        bounds
    } else {
        (0.0, 0.0, f64::from(info.width), f64::from(info.height))
    };
    let (x0, y0, w, h) = region;
    let mut out = vec![0f32; (cc + 1) * w * h];
    g.draw(
        &mut out,
        cc,
        w,
        h,
        (bx.0 - x0 as f64, bx.1 - y0 as f64, bx.2, bx.3),
    );
    Some(out)
}

/// A descriptor colour in the document's channels (0..1 as stored).
pub(crate) fn desc_channels(
    d: &Desc,
    mode: ColorMode,
    cc: usize,
    linear: bool,
) -> Option<Vec<f32>> {
    Some(col(d)?.channels(mode, cc, linear))
}

/// The pattern records of a `Patt` / `Pat2` / `Pat3` block body, as ranges of `data`.
pub(crate) fn pattern_records(
    data: &[u8],
    body: std::ops::Range<usize>,
    out: &mut Vec<std::ops::Range<usize>>,
) {
    let mut r = Rd::new(&data[..body.end], body.start);
    while r.pos + 4 <= body.end && out.len() < 4096 {
        let Some(len) = r.u32() else { break };
        let start = r.pos;
        let Some(end) = start.checked_add(len as usize).filter(|&e| e <= body.end) else {
            break;
        };
        out.push(start..end);
        r.pos = end + (4 - len as usize % 4) % 4;
    }
}

/// A shape layer's stroke (`vstk`): its geometry, opacity, blend mode, whether the fill shows,
/// and what it paints.
pub(crate) struct ShapeStroke {
    pub style: crate::vector::StrokeStyle,
    pub opacity: f32,
    pub mode: [u8; 4],
    pub fill_enabled: bool,
    content: Desc,
}

/// Layer `l`'s stroke settings, `None` without an enabled stroke.
pub(crate) fn shape_stroke(data: &[u8], l: &Layer) -> Option<ShapeStroke> {
    use crate::vector::{Align, Cap, Join, StrokeStyle};
    let d = desc::versioned(data.get(l.block(b"vstk")?)?)?;
    let fill_enabled = d.boolean(b"fillEnabled").unwrap_or(true);
    if !d.boolean(b"strokeEnabled").unwrap_or(true) {
        return None;
    }
    let res = d
        .num(b"strokeStyleResolution")
        .unwrap_or(72.0)
        .clamp(1.0, 100_000.0);
    let px = |key: &[u8]| -> Option<f64> {
        Some(match d.get(key)? {
            Value::Unit(u, v) if *u == *b"#Pnt" => v * res / 72.0,
            v => v.num()?,
        })
    };
    let width = px(b"strokeStyleLineWidth")
        .unwrap_or(1.0)
        .clamp(0.0, 10_000.0);
    let e = |key: &[u8]| d.enum_value(key).unwrap_or(b"");
    let style = StrokeStyle {
        width,
        align: match e(b"strokeStyleLineAlignment") {
            b"strokeStyleAlignInside" => Align::Inside,
            b"strokeStyleAlignOutside" => Align::Outside,
            _ => Align::Center,
        },
        cap: match e(b"strokeStyleLineCapType") {
            b"strokeStyleRoundCap" => Cap::Round,
            b"strokeStyleSquareCap" => Cap::Square,
            _ => Cap::Butt,
        },
        join: match e(b"strokeStyleLineJoinType") {
            b"strokeStyleRoundJoin" => Join::Round,
            b"strokeStyleBevelJoin" => Join::Bevel,
            _ => Join::Miter,
        },
        miter_limit: d.num(b"strokeStyleMiterLimit").unwrap_or(100.0),
        dashes: d
            .list(b"strokeStyleLineDashSet")
            .iter()
            .filter_map(Value::num)
            .map(|v| v.clamp(0.0, 1000.0))
            .take(64)
            .collect(),
        dash_offset: d.num(b"strokeStyleLineDashOffset").unwrap_or(0.0),
    };
    Some(ShapeStroke {
        style,
        opacity: (d.num(b"strokeStyleOpacity").unwrap_or(100.0) / 100.0).clamp(0.0, 1.0) as f32,
        mode: crate::effects::mode_key(d.enum_value(b"strokeStyleBlendMode")),
        fill_enabled,
        content: d.obj(b"strokeStyleContent").cloned().unwrap_or_default(),
    })
}

/// What a shape's stroke paints over the canvas (`cc` planes then alpha): a colour, a
/// gradient laid over `bounds` or a pattern.
pub(crate) fn stroke_paint(
    psd: &Psd,
    data: &[u8],
    s: &ShapeStroke,
    cc: usize,
    bounds: (f64, f64, f64, f64),
    origin: (f64, f64),
) -> Option<Vec<f32>> {
    let info = &psd.info;
    let (w, h) = (info.width as usize, info.height as usize);
    let d = &s.content;
    if d.get(b"Grad").is_some() {
        return gradient_paint(psd, d, cc, (0, 0, w, h), bounds);
    }
    if d.get(b"Ptrn").is_some() {
        return pattern_paint(psd, data, d, cc, (0, 0, w, h), origin);
    }
    let c = d
        .obj(b"Clr ")
        .and_then(col)
        .unwrap_or(Col::Gray(0.0))
        .channels(info.mode, cc, info.depth == 32);
    let n = w * h;
    let mut out = vec![0f32; (cc + 1) * n];
    for (ch, v) in c.iter().enumerate() {
        out[ch * n..(ch + 1) * n].fill(*v);
    }
    out[cc * n..].fill(1.0);
    Some(out)
}
