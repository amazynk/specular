//! Naive colour conversion to 8-bit sRGB for previews: 16 → 8 bit by rounding, 32-bit float
//! (linear) through the sRGB curve, CMYK by multiplying inverted inks, Lab (D50) through XYZ.
//! No ICC: callers that want accuracy take [`Planes`] and the profile instead.

use std::sync::OnceLock;

use crate::decode::{for_rows, out_format};
use crate::*;

/// The linear-light value at which each 8-bit sRGB code starts (code `k + 1` from entry
/// `k`): the midpoints between codes decoded exactly, so encoding rounds to the nearest code
/// (a 14-bit table rounded the index first and was half a code off near black).
fn srgb_lut() -> &'static [f32] {
    static LUT: OnceLock<Vec<f32>> = OnceLock::new();
    LUT.get_or_init(|| {
        (0..255)
            .map(|k| {
                let s = (k as f64 + 0.5) / 255.0;
                let v = if s <= 0.040_45 {
                    s / 12.92
                } else {
                    ((s + 0.055) / 1.055).powf(2.4)
                };
                v as f32
            })
            .collect()
    })
}

/// Linear light to the nearest sRGB code through `lut` ([`srgb_lut`]); NaN gives 0.
#[inline]
fn encode(lut: &[f32], v: f32) -> u8 {
    lut.partition_point(|&t| t <= v) as u8
}

/// Linear-light value (0..1, clamped) to an 8-bit sRGB code value.
#[inline]
pub fn linear_to_srgb8(v: f32) -> u8 {
    encode(srgb_lut(), v)
}

/// CIE L*a*b* (D50, L 0..100) to 8-bit sRGB (Bradford-adapted to D65).
#[inline]
pub fn lab_to_srgb8(l: f32, a: f32, b: f32) -> [u8; 3] {
    lab_to_srgb8_with(srgb_lut(), l, a, b)
}

#[inline]
fn lab_to_srgb8_with(lut: &[f32], l: f32, a: f32, b: f32) -> [u8; 3] {
    const E: f32 = 6.0 / 29.0;
    let finv = |t: f32| {
        if t > E {
            t * t * t
        } else {
            3.0 * E * E * (t - 4.0 / 29.0)
        }
    };
    let fy = (l + 16.0) / 116.0;
    let x = 0.9642 * finv(fy + a / 500.0);
    let y = finv(fy);
    let z = 0.8251 * finv(fy - b / 200.0);
    let r = 3.133_856 * x - 1.616_867 * y - 0.490_615 * z;
    let g = -0.978_768 * x + 1.916_142 * y + 0.033_454 * z;
    let bl = 0.071_945 * x - 0.228_991 * y + 1.405_243 * z;
    [encode(lut, r), encode(lut, g), encode(lut, bl)]
}

#[inline]
fn u16_to_u8(v: u16) -> u8 {
    ((v as u32 * 255 + 32895) >> 16) as u8
}

/// Whether float samples of a colour plane in `mode` are linear light that needs the sRGB curve.
fn float_is_linear(mode: ColorMode, is_alpha: bool) -> bool {
    !is_alpha
        && matches!(
            mode,
            ColorMode::Rgb | ColorMode::Grayscale | ColorMode::Duotone | ColorMode::Multichannel
        )
}

#[inline]
fn f32_to_u8(v: f32, linear: bool, lut: &[f32]) -> u8 {
    if linear {
        encode(lut, v)
    } else {
        (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
    }
}

/// One stored (big-endian) row at `depth` → 8-bit samples (`out.len()` = width).
pub(crate) fn to_u8_row(raw: &[u8], depth: u8, mode: ColorMode, is_alpha: bool, out: &mut [u8]) {
    match depth {
        1 => {
            for (x, v) in out.iter_mut().enumerate() {
                *v = if raw[x >> 3] & (0x80 >> (x & 7)) != 0 {
                    0
                } else {
                    255
                };
            }
        }
        8 => out.copy_from_slice(&raw[..out.len()]),
        16 => {
            for (v, b) in out.iter_mut().zip(raw.chunks_exact(2)) {
                *v = u16_to_u8(u16::from_be_bytes([b[0], b[1]]));
            }
        }
        _ => {
            let lut = srgb_lut();
            let lin = float_is_linear(mode, is_alpha);
            for (v, b) in out.iter_mut().zip(raw.chunks_exact(4)) {
                *v = f32_to_u8(f32::from_be_bytes([b[0], b[1], b[2], b[3]]), lin, lut);
            }
        }
    }
}

/// One stored 32-bit colour row, un-matted from white in linear light with the composite's
/// transparency `alpha` (0..1), to 8-bit codes.
pub(crate) fn unmatte_f32_row(raw: &[u8], alpha: &[f32], mode: ColorMode, out: &mut [u8]) {
    let lut = srgb_lut();
    let lin = float_is_linear(mode, false);
    for (x, (v, b)) in out.iter_mut().zip(raw.chunks_exact(4)).enumerate() {
        let c = f32::from_be_bytes([b[0], b[1], b[2], b[3]]);
        let c = match alpha.get(x) {
            Some(&a) if a > 0.0 && a < 1.0 => (c - (1.0 - a)) / a,
            _ => c,
        };
        *v = f32_to_u8(c, lin, lut);
    }
}

pub(crate) struct Compose<'a> {
    pub mode: ColorMode,
    pub color_channels: usize,
    pub alpha: bool,
    pub palette: Option<&'a [[u8; 3]]>,
    pub transparent_index: Option<u8>,
    pub format: PixelFormat,
}

/// 8-bit plane rows (colour planes, then alpha) → one interleaved output row.
pub(crate) fn compose_row(ctx: &Compose, rows: &[&[u8]], out: &mut [u8]) {
    let cc = ctx.color_channels;
    let alpha = if ctx.alpha {
        rows.get(cc).copied()
    } else {
        None
    };
    if ctx.format == PixelFormat::Gray8 {
        out.copy_from_slice(&rows[0][..out.len()]);
        return;
    }
    let px = out.chunks_exact_mut(4);
    match (ctx.mode, cc) {
        (ColorMode::Rgb | ColorMode::Multichannel, 3) => {
            let (r, g, b) = (rows[0], rows[1], rows[2]);
            match alpha {
                Some(a) => {
                    for ((((o, &r), &g), &b), &a) in px.zip(r).zip(g).zip(b).zip(a) {
                        o.copy_from_slice(&[r, g, b, a]);
                    }
                }
                None => {
                    for (((o, &r), &g), &b) in px.zip(r).zip(g).zip(b) {
                        o.copy_from_slice(&[r, g, b, 255]);
                    }
                }
            }
        }
        (ColorMode::Indexed, _) => {
            let pal = ctx.palette.unwrap_or(&[]);
            let ti = ctx.transparent_index.map_or(256, |t| t as usize);
            for (x, (o, &i)) in px.zip(rows[0]).enumerate() {
                let [r, g, b] = pal.get(i as usize).copied().unwrap_or([0; 3]);
                let a = if i as usize == ti {
                    0
                } else {
                    alpha.map_or(255, |a| a[x])
                };
                o.copy_from_slice(&[r, g, b, a]);
            }
        }
        (ColorMode::Cmyk, _) => {
            let (c, m, y, k) = (rows[0], rows[1], rows[2], rows[3]);
            let mul = |a: u8, b: u8| ((a as u32 * b as u32 + 127) / 255) as u8;
            for (x, o) in px.enumerate() {
                let kk = k[x];
                let a = alpha.map_or(255, |a| a[x]);
                o.copy_from_slice(&[mul(c[x], kk), mul(m[x], kk), mul(y[x], kk), a]);
            }
        }
        (ColorMode::Lab, _) => {
            let lut = srgb_lut();
            let (l, a_, b_) = (rows[0], rows[1], rows[2]);
            for (x, o) in px.enumerate() {
                let [r, g, b] = lab_to_srgb8_with(
                    lut,
                    l[x] as f32 * (100.0 / 255.0),
                    a_[x] as f32 - 128.0,
                    b_[x] as f32 - 128.0,
                );
                o.copy_from_slice(&[r, g, b, alpha.map_or(255, |a| a[x])]);
            }
        }
        _ => {
            // Grey-like with alpha.
            for (x, o) in px.enumerate() {
                let g = rows[0][x];
                o.copy_from_slice(&[g, g, g, alpha.map_or(255, |a| a[x])]);
            }
        }
    }
}

/// Undo Photoshop's flattening of transparent pixels onto white: c = (c' − (1 − a)) / a.
pub(crate) fn unmatte_row(row: &mut [u8]) {
    for p in row.chunks_exact_mut(4) {
        let a = p[3] as i32;
        if a == 255 || a == 0 {
            continue;
        }
        for c in &mut p[..3] {
            let v = ((*c as i32 - (255 - a)) * 255 + a / 2) / a;
            *c = v.clamp(0, 255) as u8;
        }
    }
}

/// The 16-bit interleaved format for `mode` (RGB and grey-like only).
pub(crate) fn wide_format(
    mode: ColorMode,
    color_channels: usize,
    alpha: bool,
) -> Option<PixelFormat> {
    match (mode, color_channels, alpha) {
        (ColorMode::Rgb | ColorMode::Multichannel, 3, _) => Some(PixelFormat::Rgba16),
        (ColorMode::Grayscale | ColorMode::Duotone | ColorMode::Multichannel, 1, false) => {
            Some(PixelFormat::Gray16)
        }
        (ColorMode::Grayscale | ColorMode::Duotone | ColorMode::Multichannel, 1, true) => {
            Some(PixelFormat::GrayAlpha16)
        }
        _ => None,
    }
}

/// [`unmatte_row`] at 16 bits, on pixels of `n` samples whose last is alpha.
fn unmatte_row16(row: &mut [u16], n: usize) {
    for p in row.chunks_exact_mut(n) {
        let a = p[n - 1] as i64;
        if a == 65535 || a == 0 {
            continue;
        }
        for c in &mut p[..n - 1] {
            let v = ((*c as i64 - (65535 - a)) * 65535 + a / 2) / a;
            *c = v.clamp(0, 65535) as u16;
        }
    }
}

impl Planes {
    /// Interleave 16-bit RGB or grey-like planes at 16 bits ([`Planes::wide_format`]),
    /// undoing the white matte like [`Planes::to_rgba8`]; RGB without alpha is opaque. `None`
    /// for other modes and depths.
    pub fn to_wide(&self, parallel: bool) -> Option<Image> {
        let format = self.wide_format()?;
        let Samples::U16(s) = &self.samples else {
            return None;
        };
        let (w, h) = (self.width as usize, self.height as usize);
        let n = format.channels();
        let cc = self.color_channels;
        // Source plane of each output sample: colour, then alpha (none: opaque).
        let src: Vec<Option<usize>> = (0..n)
            .map(|c| {
                if c < cc {
                    Some(c)
                } else {
                    self.alpha.then_some(cc)
                }
            })
            .collect();
        let plane = w * h;
        let grey = GREY as u16 * 257;
        let mut out = vec![0u8; plane * n * 2];
        for_rows(
            &mut out,
            w * n * 2,
            parallel,
            || vec![0u16; w * n],
            |px, y, o| {
                for (c, p) in src.iter().enumerate() {
                    let row = p.and_then(|p| s.get(p * plane + y * w..p * plane + (y + 1) * w));
                    let dst = px[c..].iter_mut().step_by(n);
                    match row {
                        Some(r) => dst.zip(r).for_each(|(d, &v)| *d = v),
                        None if p.is_none() || c == cc => dst.for_each(|d| *d = 65535),
                        None => dst.for_each(|d| *d = grey),
                    }
                }
                if self.matted_on_white && self.alpha {
                    unmatte_row16(px, n);
                }
                for (b, v) in o.chunks_exact_mut(2).zip(px.iter()) {
                    b.copy_from_slice(&v.to_ne_bytes());
                }
            },
        );
        Some(Image {
            width: self.width,
            height: self.height,
            format,
            data: out,
            truncated: self.truncated,
        })
    }

    /// Naive conversion to 8-bit for previews (see the module notes): grey without alpha →
    /// [`PixelFormat::Gray8`], everything else → [`PixelFormat::Rgba8`].
    pub fn to_rgba8(&self) -> Image {
        let (w, h) = (self.width as usize, self.height as usize);
        let np = self.plane_count().min(5);
        let format = out_format(self.mode, self.color_channels, self.alpha);
        let bpp = format.channels();
        let mut out = vec![0u8; w * h * bpp];
        let ctx = Compose {
            mode: self.mode,
            color_channels: self.color_channels,
            alpha: self.alpha,
            palette: self.palette.as_deref(),
            transparent_index: self.transparent_index,
            format,
        };
        let plane = w * h;
        let lut = srgb_lut();
        for_rows(
            &mut out,
            w * bpp,
            true,
            || vec![vec![0u8; w]; np],
            |bufs, y, o| {
                let mut rows: [&[u8]; 5] = [&[]; 5];
                for (c, b) in bufs.iter_mut().enumerate() {
                    let at = c * plane + y * w;
                    match &self.samples {
                        Samples::U8(s) => {
                            rows[c] = s.get(at..at + w).unwrap_or(&[]);
                            if rows[c].len() == w {
                                continue;
                            }
                            b.fill(GREY);
                        }
                        Samples::U16(s) => {
                            if let Some(src) = s.get(at..at + w) {
                                for (d, &v) in b.iter_mut().zip(src) {
                                    *d = u16_to_u8(v);
                                }
                            } else {
                                b.fill(GREY);
                            }
                        }
                        Samples::F32(s) => {
                            let colour = c < self.color_channels;
                            let lin = float_is_linear(self.mode, !colour);
                            // 32-bit documents are matted in linear light: undo it there,
                            // before the sRGB curve.
                            let alpha = (self.matted_on_white && self.alpha && colour)
                                .then(|| {
                                    let a = self.color_channels * plane + y * w;
                                    s.get(a..a + w)
                                })
                                .flatten();
                            if let Some(src) = s.get(at..at + w) {
                                for (x, (d, &v)) in b.iter_mut().zip(src).enumerate() {
                                    let v = match alpha.map(|a| a[x]) {
                                        Some(a) if a > 0.0 && a < 1.0 => (v - (1.0 - a)) / a,
                                        _ => v,
                                    };
                                    *d = f32_to_u8(v, lin, lut);
                                }
                            } else {
                                b.fill(GREY);
                            }
                        }
                    }
                }
                for (c, b) in bufs.iter().enumerate() {
                    if rows[c].is_empty() {
                        rows[c] = b;
                    }
                }
                compose_row(&ctx, &rows[..np], o);
                if self.matted_on_white
                    && format == PixelFormat::Rgba8
                    && !matches!(self.samples, Samples::F32(_))
                {
                    unmatte_row(o);
                }
            },
        );
        Image {
            width: self.width,
            height: self.height,
            format,
            data: out,
            truncated: self.truncated,
        }
    }
}
