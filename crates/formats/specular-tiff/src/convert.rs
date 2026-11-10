//! Sample conversion to 8-bit grey / RGBA (or, for wide decodes, 16-bit grey / grey-alpha /
//! RGBA, or linear half-float RGBA for float images) and the gamma-correct box filter.

use crate::{Decoded, Ifd, LabCoding, LabSamples, Photometric, PixelFormat, SampleFormat};
use std::sync::OnceLock;

/// Converts rows of raw (decompressed, predictor-undone) samples to the output format.
pub(crate) struct Converter<'a> {
    pub ifd: &'a Ifd,
    pub target: PixelFormat,
    /// Byte order of multi-byte samples.
    pub le: bool,
    /// Colour-managed CMYK conversion ([`crate::set_cmyk_hook`]).
    pub cmyk: Option<crate::CmykRowConverter>,
    /// L*a*b* conversion ([`crate::set_lab_hook`]) and the samples' coding.
    pub lab: Option<crate::LabRowConverter>,
    pub lab_coding: Option<LabCoding>,
    /// The sample [`PixelFormat::GrayF32`] output takes, and the value that becomes NaN.
    pub band: usize,
    pub nodata: Option<f64>,
}

/// sRGB transfer tables.
struct Lut {
    /// sRGB 8-bit → linear 16-bit.
    to_lin: [u16; 256],
    /// linear 16-bit → sRGB 8-bit.
    to_srgb: Vec<u8>,
    /// linear 0..=4095 (float input) → sRGB 8-bit.
    float_to_srgb: [u8; 4096],
}

fn lut() -> &'static Lut {
    static L: OnceLock<Lut> = OnceLock::new();
    L.get_or_init(|| {
        let enc = |l: f64| {
            let c = if l <= 0.003_130_8 {
                l * 12.92
            } else {
                1.055 * l.powf(1.0 / 2.4) - 0.055
            };
            (c * 255.0 + 0.5).clamp(0.0, 255.0) as u8
        };
        let mut to_lin = [0u16; 256];
        for (i, v) in to_lin.iter_mut().enumerate() {
            let c = i as f64 / 255.0;
            let l = if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            };
            *v = (l * 65535.0 + 0.5) as u16;
        }
        let to_srgb = (0..65536).map(|i| enc(i as f64 / 65535.0)).collect();
        let mut float_to_srgb = [0u8; 4096];
        for (i, v) in float_to_srgb.iter_mut().enumerate() {
            *v = enc(i as f64 / 4095.0);
        }
        Lut {
            to_lin,
            to_srgb,
            float_to_srgb,
        }
    })
}

/// sRGB 16-bit code → linear light, for the 16-bit box filter.
fn to_lin16() -> &'static [f32] {
    static L: OnceLock<Vec<f32>> = OnceLock::new();
    L.get_or_init(|| {
        (0..65536)
            .map(|i| {
                let c = i as f64 / 65535.0;
                (if c <= 0.04045 {
                    c / 12.92
                } else {
                    ((c + 0.055) / 1.055).powf(2.4)
                }) as f32
            })
            .collect()
    })
}

/// Linear light → sRGB 16-bit code.
#[inline]
fn enc16(l: f64) -> u16 {
    let c = if l <= 0.003_130_8 {
        l * 12.92
    } else {
        1.055 * l.powf(1.0 / 2.4) - 0.055
    };
    (c * 65535.0 + 0.5).clamp(0.0, 65535.0) as u16
}

#[inline]
fn float8(v: f64, linear_out: bool) -> u8 {
    let v = if v.is_nan() { 0.0 } else { v.clamp(0.0, 1.0) };
    if linear_out {
        (v * 255.0 + 0.5) as u8
    } else {
        lut().float_to_srgb[(v * 4095.0 + 0.5) as usize]
    }
}

/// f32 → IEEE half, rounded to nearest even; NaN becomes 0 and the range is clamped to the
/// largest finite half (±65504).
pub(crate) fn f32_to_half(v: f32) -> u16 {
    let v = if v.is_nan() {
        0.0
    } else {
        v.clamp(-65504.0, 65504.0)
    };
    let b = v.to_bits();
    let s = ((b >> 16) & 0x8000) as u16;
    let e = ((b >> 23) & 0xff) as i32 - 127 + 15;
    let m = b & 0x7f_ffff;
    if e <= 0 {
        // Subnormal half (or zero).
        if e < -10 {
            return s;
        }
        let m = m | 0x80_0000;
        let shift = (14 - e) as u32;
        let r = (m + (1 << (shift - 1)) - 1 + ((m >> shift) & 1)) >> shift;
        return s | r as u16;
    }
    let r = m + 0xfff + ((m >> 13) & 1);
    let out = ((e as u32) << 10) + (r >> 13);
    s | out.min(0x7bff) as u16
}

/// A 24-bit float (sign, 7-bit exponent biased by 63, 16-bit mantissa) as f32.
pub(crate) fn f24_to_f32(v: u32) -> f32 {
    let s = (v >> 23) << 31;
    let e = (v >> 16) & 0x7f;
    let m = v & 0xffff;
    let bits = if e == 0 {
        // Subnormal: m · 2^(1 - 63 - 16).
        return f32::from_bits(s) + (if s != 0 { -1.0 } else { 1.0 }) * m as f32 * (-78f32).exp2();
    } else if e == 0x7f {
        s | 0x7f80_0000 | (m << 7)
    } else {
        s | ((e + 127 - 63) << 23) | (m << 7)
    };
    f32::from_bits(bits)
}

pub(crate) fn half_to_f32(h: u16) -> f32 {
    let s = ((h >> 15) as u32) << 31;
    let e = ((h >> 10) & 0x1f) as u32;
    let m = (h & 0x3ff) as u32;
    let bits = if e == 0 {
        if m == 0 {
            s
        } else {
            // subnormal
            let mut e = 127 - 15 + 1;
            let mut m = m;
            while m & 0x400 == 0 {
                m <<= 1;
                e -= 1;
            }
            s | (e << 23) | ((m & 0x3ff) << 13)
        }
    } else if e == 31 {
        s | 0x7f80_0000 | (m << 13)
    } else {
        s | ((e + 127 - 15) << 23) | (m << 13)
    };
    f32::from_bits(bits)
}

/// XYZ (D50) → linear sRGB, Bradford-adapted.
const XYZ_D50_TO_SRGB: [[f32; 3]; 3] = [
    [3.133_856, -1.616_867, -0.490_615],
    [-0.978_768, 1.916_142, 0.033_454],
    [0.071_945, -0.228_991, 1.405_243],
];
/// The D50 white (ICC PCS).
const D50: [f32; 3] = [0.9642, 1.0, 0.8249];

/// One L*a*b* (D50) colour → 8-bit sRGB (relative colorimetric, clipped): the built-in
/// conversion when no [`crate::set_lab_hook`] converter is registered.
pub(crate) fn lab_to_srgb8(lab: [f32; 3]) -> [u8; 3] {
    let fy = (lab[0] + 16.0) / 116.0;
    let f = [fy + lab[1] / 500.0, fy, fy - lab[2] / 200.0];
    const D: f32 = 6.0 / 29.0;
    let xyz: [f32; 3] = std::array::from_fn(|c| {
        let t = f[c];
        D50[c]
            * if t > D {
                t * t * t
            } else {
                3.0 * D * D * (t - 4.0 / 29.0)
            }
    });
    let l = lut();
    XYZ_D50_TO_SRGB.map(|m| {
        let v = m[0] * xyz[0] + m[1] * xyz[1] + m[2] * xyz[2];
        let v = if v.is_nan() { 0.0 } else { v.clamp(0.0, 1.0) };
        l.to_srgb[(v * 65535.0 + 0.5) as usize]
    })
}

/// Raw (unsigned) sample `i` bytes into `s` in `coding`'s width and byte order.
#[inline]
fn lab_raw(coding: &LabCoding, s: &[u8], i: usize) -> u32 {
    match (coding.wide, coding.big_endian) {
        (false, _) => s[i] as u32,
        (true, false) => u16::from_le_bytes([s[i], s[i + 1]]) as u32,
        (true, true) => u16::from_be_bytes([s[i], s[i + 1]]) as u32,
    }
}

impl Converter<'_> {
    /// Convert a run of L*a*b* samples to RGBA (alpha 255) through the hook's converter, or
    /// the built-in formula.
    pub(crate) fn lab_convert(&self, s: &LabSamples, rgba: &mut [u8]) {
        if let Some(h) = &self.lab {
            return h(s, rgba);
        }
        let b = if s.coding.wide { 2 } else { 1 };
        for (i, o) in rgba.chunks_exact_mut(4).enumerate() {
            let at = i * s.step;
            if s.channels.iter().any(|c| c.len() < at + b) {
                break;
            }
            let lab: [f32; 3] =
                std::array::from_fn(|c| s.coding.value(c, lab_raw(&s.coding, s.channels[c], at)));
            let [r, g, bl] = lab_to_srgb8(lab);
            o.copy_from_slice(&[r, g, bl, 255]);
        }
    }

    /// [`Converter::row`] for L*a*b*: the samples go to the converter where they lie (chunky
    /// rows with any extra samples, or planes); associated alpha is divided out of the
    /// decoded L*, a*, b* first.
    fn lab_row(&self, rows: &[&[u8]], x0: usize, x1: usize, dst: &mut [u8], scratch: &mut Vec<u8>) {
        let ifd = self.ifd;
        let n = x1 - x0;
        let dst = &mut dst[..n * 4];
        let Some(coding) = self.lab_coding else {
            return dst.fill(128);
        };
        let spp = ifd.samples as usize;
        let b = if coding.wide { 2 } else { 1 };
        // Pixel x0's sample `c` and the step between pixels.
        let at = |c: usize| -> (&[u8], usize) {
            if ifd.planar {
                let r = rows.get(c).copied().unwrap_or(&[]);
                (r.get(x0 * b..).unwrap_or(&[]), b)
            } else {
                (rows[0].get((x0 * spp + c) * b..).unwrap_or(&[]), spp * b)
            }
        };
        let (ai, premul) = match ifd.alpha {
            Some((i, p)) if (3..spp).contains(&i) => (Some(i), p),
            _ => (None, false),
        };
        let max = if coding.wide { 65535.0 } else { 255.0 };
        match ai {
            Some(ai) if premul => {
                // Un-premultiplied L*a*b* in the ICC v4 16-bit coding, native order (the
                // coded samples were multiplied by alpha, the sign of CIELab's a*/b* kept).
                let (ach, ast) = at(ai);
                let ch = [at(0), at(1), at(2)];
                scratch.clear();
                scratch.resize(n * 6, 0);
                for (x, o) in scratch.chunks_exact_mut(6).enumerate() {
                    let a = ach
                        .get(x * ast..)
                        .filter(|s| s.len() >= b)
                        .map_or(1.0, |s| lab_raw(&coding, s, 0) as f32 / max);
                    for (c, (s, st)) in ch.iter().enumerate() {
                        let mut v = s
                            .get(x * st..)
                            .filter(|s| s.len() >= b)
                            .map_or(0.0, |s| coding.value(c, lab_raw(&coding, s, 0)));
                        // The stored (coded) value is premultiplied, as RGB's is: undo it
                        // before the offset.
                        if a > 0.0 && a < 1.0 {
                            v = (v - coding.offset[c]) / a + coding.offset[c];
                        }
                        let e = if c == 0 {
                            v.clamp(0.0, 100.0) * (65535.0 / 100.0)
                        } else {
                            (v + 128.0).clamp(0.0, 255.0) * 257.0
                        };
                        o[2 * c..2 * c + 2].copy_from_slice(&((e + 0.5) as u16).to_ne_bytes());
                    }
                }
                let v4 = LabCoding {
                    wide: true,
                    big_endian: cfg!(target_endian = "big"),
                    signed: [false; 3],
                    scale: [100.0 / 65535.0, 1.0 / 257.0, 1.0 / 257.0],
                    offset: [0.0, -128.0, -128.0],
                };
                let s = LabSamples {
                    channels: [&scratch[..], &scratch[2..], &scratch[4..]],
                    step: 6,
                    coding: v4,
                };
                self.lab_convert(&s, dst);
            }
            _ => {
                let ((c0, step), (c1, _), (c2, _)) = (at(0), at(1), at(2));
                let s = LabSamples {
                    channels: [c0, c1, c2],
                    step,
                    coding,
                };
                self.lab_convert(&s, dst);
            }
        }
        if let Some(ai) = ai {
            let (ach, ast) = at(ai);
            for (x, d) in dst.chunks_exact_mut(4).enumerate() {
                if let Some(s) = ach.get(x * ast..).filter(|s| s.len() >= b) {
                    let v = lab_raw(&coding, s, 0);
                    d[3] = if coding.wide {
                        ((v * 255 + 32767) / 65535) as u8
                    } else {
                        v as u8
                    };
                }
            }
        }
    }

    /// Read sample `s` (index into the row's samples) as an unscaled unsigned integer.
    #[inline]
    fn raw(&self, row: &[u8], s: usize) -> u32 {
        let bits = self.ifd.bits as usize;
        match bits {
            8 => row[s] as u32,
            16 => {
                let b = [row[2 * s], row[2 * s + 1]];
                (if self.le {
                    u16::from_le_bytes(b)
                } else {
                    u16::from_be_bytes(b)
                }) as u32
            }
            32 => {
                let b = [row[4 * s], row[4 * s + 1], row[4 * s + 2], row[4 * s + 3]];
                if self.le {
                    u32::from_le_bytes(b)
                } else {
                    u32::from_be_bytes(b)
                }
            }
            _ => {
                // Bit-packed, MSB first (1..16 bits).
                let p = s * bits;
                let mut v = 0u32;
                let mut got = 0usize;
                while got < bits {
                    let byte = row[(p + got) / 8] as u32;
                    let off = (p + got) % 8;
                    let take = (8 - off).min(bits - got);
                    let x = (byte >> (8 - off - take)) & ((1 << take) - 1);
                    v = (v << take) | x;
                    got += take;
                }
                v
            }
        }
    }

    /// Sample `s` scaled to 8 bits. `linear` keeps float values linear (alpha).
    #[inline]
    fn sample8(&self, row: &[u8], s: usize, linear: bool) -> u8 {
        let ifd = self.ifd;
        if ifd.sample_format == SampleFormat::Float {
            return float8(self.float(row, s), linear);
        }
        let bits = ifd.bits as u32;
        let v = self.unsigned(row, s);
        match bits {
            8 => v as u8,
            16 => ((v * 255 + 32767) / 65535) as u8,
            32 => (v >> 24) as u8,
            b => {
                let max = (1u32 << b) - 1;
                ((v * 255 + max / 2) / max) as u8
            }
        }
    }

    /// Integer sample `s` scaled to 16 bits (signed formats offset to unsigned).
    #[inline]
    fn sample16(&self, row: &[u8], s: usize) -> u16 {
        let v = self.unsigned(row, s) as u64;
        match self.ifd.bits {
            16 => v as u16,
            b => {
                let max = (1u64 << b) - 1;
                ((v * 65535 + max / 2) / max) as u16
            }
        }
    }

    /// [`Converter::row`] for the 16-bit output formats: integer grey or RGB(A) samples deeper
    /// than 8 bits scaled to 16, MinIsWhite inverted, associated alpha divided out at 16 bits.
    fn row16(&self, rows: &[&[u8]], x0: usize, x1: usize, dst: &mut [u8]) {
        let ifd = self.ifd;
        let spp = ifd.samples as usize;
        let n = x1 - x0;
        let get = |x: usize, c: usize| -> u16 {
            if ifd.planar {
                rows.get(c).map_or(0, |r| self.sample16(r, x))
            } else {
                self.sample16(rows[0], x * spp + c)
            }
        };
        let inv = ifd.photometric == Photometric::MinIsWhite;
        let (ai, premul) = match ifd.alpha {
            Some((i, p)) if i < spp => (Some(i), p),
            _ => (None, false),
        };
        let unpremul = |c: u16, a: u16| -> u16 {
            if premul && a != 0 && a != 65535 {
                ((c as u32 * 65535 + a as u32 / 2) / a as u32).min(65535) as u16
            } else {
                c
            }
        };
        let put = |d: &mut [u8], v: &[u16]| {
            for (d, v) in d.chunks_exact_mut(2).zip(v) {
                d.copy_from_slice(&v.to_ne_bytes());
            }
        };
        match self.target {
            PixelFormat::Gray16 => {
                let dst = &mut dst[..n * 2];
                if spp == 1
                    && !inv
                    && ifd.bits == 16
                    && ifd.sample_format == SampleFormat::Uint
                    && self.le == cfg!(target_endian = "little")
                {
                    dst.copy_from_slice(&rows[0][x0 * 2..x1 * 2]);
                    return;
                }
                for (x, d) in (x0..x1).zip(dst.chunks_exact_mut(2)) {
                    let g = get(x, 0);
                    put(d, &[if inv { 65535 - g } else { g }]);
                }
            }
            PixelFormat::GrayAlpha16 => {
                for (x, d) in (x0..x1).zip(dst[..n * 4].chunks_exact_mut(4)) {
                    let a = ai.map_or(65535, |i| get(x, i));
                    // Inversion applies to the stored (possibly premultiplied) grey.
                    let g = get(x, 0);
                    let g = match (inv, premul) {
                        (false, _) => g,
                        (true, false) => 65535 - g,
                        (true, true) => a.saturating_sub(g),
                    };
                    put(d, &[unpremul(g, a), a]);
                }
            }
            PixelFormat::Rgba16 => {
                let rgb = ifd.photometric == Photometric::Rgb;
                for (x, d) in (x0..x1).zip(dst[..n * 8].chunks_exact_mut(8)) {
                    let a = ai.map_or(65535, |i| get(x, i));
                    let c = if rgb {
                        [get(x, 0), get(x, 1), get(x, 2)]
                    } else {
                        let g = get(x, 0);
                        [if inv { 65535 - g } else { g }; 3]
                    };
                    put(
                        d,
                        &[unpremul(c[0], a), unpremul(c[1], a), unpremul(c[2], a), a],
                    );
                }
            }
            // `row` handles the 8-bit formats, `row_f16` the half floats.
            PixelFormat::Gray8
            | PixelFormat::Rgba8
            | PixelFormat::Rgba16F
            | PixelFormat::GrayF32 => {}
        }
    }

    /// [`Converter::row`] for [`PixelFormat::Rgba16F`]: float grey or RGB(A) kept linear and
    /// unclamped (NaN as 0), MinIsWhite as 1 − v, associated alpha divided out.
    fn row_f16(&self, rows: &[&[u8]], x0: usize, x1: usize, dst: &mut [u8]) {
        let ifd = self.ifd;
        let spp = ifd.samples as usize;
        let n = x1 - x0;
        let get = |x: usize, c: usize| -> f32 {
            if ifd.planar {
                rows.get(c)
                    .filter(|r| !r.is_empty())
                    .map_or(0.0, |r| self.unit(r, x) as f32)
            } else {
                self.unit(rows[0], x * spp + c) as f32
            }
        };
        let inv = ifd.photometric == Photometric::MinIsWhite;
        let rgb = ifd.photometric == Photometric::Rgb && spp >= 3;
        let (ai, premul) = match ifd.alpha {
            Some((i, p)) if i < spp => (Some(i), p),
            _ => (None, false),
        };
        for (x, d) in (x0..x1).zip(dst[..n * 8].chunks_exact_mut(8)) {
            let a = ai.map_or(1.0, |i| {
                let a = get(x, i);
                if a.is_nan() { 0.0 } else { a.clamp(0.0, 1.0) }
            });
            let mut c = if rgb {
                [get(x, 0), get(x, 1), get(x, 2)]
            } else {
                let g = get(x, 0);
                let g = match (inv, premul) {
                    (false, _) => g,
                    (true, false) => 1.0 - g,
                    (true, true) => a - g,
                };
                [g; 3]
            };
            if premul && a > 0.0 && a < 1.0 {
                for v in &mut c {
                    *v /= a;
                }
            }
            for (o, v) in d.chunks_exact_mut(2).zip([c[0], c[1], c[2], a]) {
                o.copy_from_slice(&f32_to_half(v).to_ne_bytes());
            }
        }
    }

    /// [`Converter::row`] for [`PixelFormat::GrayF32`]: sample `band` as stored (signed
    /// integers as signed), the no-data value as NaN.
    fn row_values(&self, rows: &[&[u8]], x0: usize, x1: usize, dst: &mut [u8]) {
        let ifd = self.ifd;
        let spp = ifd.samples as usize;
        let n = x1 - x0;
        let (row, step, off) = if ifd.planar {
            (rows.get(self.band).copied().unwrap_or(&[]), 1, 0)
        } else {
            (rows[0], spp, self.band)
        };
        let bits = ifd.bits as u32;
        // Integers widened to 32 bits: their stored width.
        let wide = match ifd.stored {
            Some(crate::variants::Stored::WideInt { bits }) => Some(bits),
            _ => None,
        };
        let nodata = self.nodata;
        for (x, d) in (x0..x1).zip(dst[..n * 4].chunks_exact_mut(4)) {
            let s = x * step + off;
            let v = if row.is_empty() {
                f64::NAN
            } else {
                match ifd.sample_format {
                    SampleFormat::Float => self.float(row, s),
                    _ if wide.is_some() => crate::variants::unwiden(
                        self.raw(row, s),
                        wide.unwrap_or(32),
                        ifd.sample_format == SampleFormat::Int,
                    ),
                    SampleFormat::Int => {
                        let r = self.raw(row, s) as u64;
                        let sh = 64 - bits;
                        (((r << sh) as i64) >> sh) as f64
                    }
                    SampleFormat::Uint => self.raw(row, s) as f64,
                }
            };
            let v = match nodata {
                Some(nd) if v == nd || (nd.is_finite() && (v as f32) == (nd as f32)) => f32::NAN,
                _ => v as f32,
            };
            d.copy_from_slice(&v.to_ne_bytes());
        }
    }

    /// 1-bit grey without alpha to [`PixelFormat::Gray8`] (bilevel scans: CCITT, PackBits).
    fn row_bilevel(&self, row: &[u8], x0: usize, x1: usize, dst: &mut [u8]) {
        let flip = if self.ifd.photometric == Photometric::MinIsWhite {
            0xff
        } else {
            0
        };
        let mut x = x0;
        let mut d = 0usize;
        // Leading pixels up to a byte boundary, then whole bytes, then the rest.
        while x < x1 && !x.is_multiple_of(8) {
            dst[d] = 0u8.wrapping_sub((row[x / 8] >> (7 - x % 8)) & 1) ^ flip;
            x += 1;
            d += 1;
        }
        let whole = (x1 - x) / 8;
        for (o, &b) in dst[d..d + whole * 8]
            .chunks_exact_mut(8)
            .zip(&row[x / 8..x / 8 + whole])
        {
            for (i, o) in o.iter_mut().enumerate() {
                *o = 0u8.wrapping_sub((b >> (7 - i)) & 1) ^ flip;
            }
        }
        x += whole * 8;
        d += whole * 8;
        while x < x1 {
            dst[d] = 0u8.wrapping_sub((row[x / 8] >> (7 - x % 8)) & 1) ^ flip;
            x += 1;
            d += 1;
        }
    }

    /// Float sample `s` as stored.
    #[inline]
    fn float(&self, row: &[u8], s: usize) -> f64 {
        match self.ifd.bits {
            16 => half_to_f32(self.raw(row, s) as u16) as f64,
            24 => {
                let b = &row[3 * s..3 * s + 3];
                let v = if self.le {
                    u32::from_le_bytes([b[0], b[1], b[2], 0])
                } else {
                    u32::from_be_bytes([0, b[0], b[1], b[2]])
                };
                f24_to_f32(v) as f64
            }
            32 => f32::from_bits(self.raw(row, s)) as f64,
            _ => {
                let mut b = [0u8; 8];
                b.copy_from_slice(&row[8 * s..8 * s + 8]);
                if self.le {
                    f64::from_le_bytes(b)
                } else {
                    f64::from_be_bytes(b)
                }
            }
        }
    }

    /// Integer sample `s`, signed formats offset to unsigned.
    #[inline]
    fn unsigned(&self, row: &[u8], s: usize) -> u32 {
        let mut v = self.raw(row, s);
        if self.ifd.sample_format == SampleFormat::Int {
            v ^= 1 << (self.ifd.bits as u32 - 1);
        }
        v
    }

    /// Sample `s` as 0..1 (floats as stored: linear light, may exceed 1).
    #[inline]
    fn unit(&self, row: &[u8], s: usize) -> f64 {
        if self.ifd.sample_format == SampleFormat::Float {
            self.float(row, s)
        } else {
            let max = (1u64 << self.ifd.bits) - 1;
            self.unsigned(row, s) as f64 / max as f64
        }
    }

    /// Associated alpha deeper than 8 bits: every sample of pixels `x0..x1` un-premultiplied at
    /// full precision (floats in linear light, before the sRGB encoding) and only then
    /// reduced to 8 bits, into `out` (`spp` per pixel).
    fn unpremultiplied8(&self, rows: &[&[u8]], x0: usize, x1: usize, ai: usize, out: &mut [u8]) {
        let ifd = self.ifd;
        let spp = ifd.samples as usize;
        let float = ifd.sample_format == SampleFormat::Float;
        let at = |x: usize, c: usize| {
            if ifd.planar {
                (rows.get(c).copied().unwrap_or(&[]), x)
            } else {
                (rows[0], x * spp + c)
            }
        };
        for (x, o) in (x0..x1).zip(out.chunks_exact_mut(spp)) {
            let (r, i) = at(x, ai);
            if r.is_empty() {
                continue;
            }
            let a = self.unit(r, i);
            let a = if a.is_nan() { 0.0 } else { a.clamp(0.0, 1.0) };
            for (c, o) in o.iter_mut().enumerate() {
                let (r, i) = at(x, c);
                if r.is_empty() {
                    continue;
                }
                let mut v = self.unit(r, i);
                if c != ai && a > 0.0 && a < 1.0 {
                    v /= a;
                }
                *o = if float {
                    float8(v, c == ai)
                } else {
                    (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
                };
            }
        }
    }

    /// Convert pixels `x0..x1` of one unit row into `dst` (output format, `x1 - x0` pixels).
    /// `rows` holds one row for chunky data, or one row per sample (planar).
    pub fn row(&self, rows: &[&[u8]], x0: usize, x1: usize, dst: &mut [u8], scratch: &mut Vec<u8>) {
        if self.target.is_wide() {
            return self.row16(rows, x0, x1, dst);
        }
        if self.target == PixelFormat::Rgba16F {
            return self.row_f16(rows, x0, x1, dst);
        }
        if self.target == PixelFormat::GrayF32 {
            return self.row_values(rows, x0, x1, dst);
        }
        let ifd = self.ifd;
        let spp = ifd.samples as usize;
        let n = x1 - x0;
        if self.target == PixelFormat::Gray8
            && ifd.bits == 1
            && spp == 1
            && matches!(
                ifd.photometric,
                Photometric::MinIsBlack | Photometric::MinIsWhite
            )
        {
            return self.row_bilevel(rows[0], x0, x1, dst);
        }
        if ifd.photometric.is_lab() {
            return self.lab_row(rows, x0, x1, dst, scratch);
        }
        if ifd.photometric == Photometric::Palette {
            let map = ifd.colormap.as_deref().unwrap_or(&[]);
            // Palettes deeper than 16 bits are read widened to 32 bits: their top bits.
            let bits = ifd.stored_bits();
            let shift = ifd.bits - bits;
            let entries = 1usize << bits;
            for (x, d) in (x0..x1).zip(dst.chunks_exact_mut(4)) {
                let s = if ifd.planar { x } else { x * spp };
                let i = ((self.raw(rows[0], s) >> shift) as usize).min(entries - 1);
                let a = match ifd.alpha {
                    Some((ai, _)) if ai < spp => {
                        if ifd.planar {
                            rows.get(ai).map_or(255, |r| self.sample8(r, x, true))
                        } else {
                            self.sample8(rows[0], x * spp + ai, true)
                        }
                    }
                    _ => 255,
                };
                d.copy_from_slice(&[
                    (map[i] >> 8) as u8,
                    (map[entries + i] >> 8) as u8,
                    (map[2 * entries + i] >> 8) as u8,
                    a,
                ]);
            }
            return;
        }
        // Stage 1: 8-bit samples, interleaved.
        let alpha_idx = ifd.alpha.map(|a| a.0);
        let uint8 = ifd.bits == 8 && ifd.sample_format == SampleFormat::Uint;
        // Associated alpha deeper than 8 bits is divided out here, before the reduction.
        let deep_premul = match ifd.alpha {
            Some((i, true)) if i < spp && !uint8 => Some(i),
            _ => None,
        };
        let samples: &[u8] = if let Some(ai) = deep_premul {
            scratch.clear();
            scratch.resize(n * spp, 0);
            self.unpremultiplied8(rows, x0, x1, ai, scratch);
            scratch
        } else if uint8 && !ifd.planar {
            &rows[0][x0 * spp..x1 * spp]
        } else {
            scratch.clear();
            scratch.resize(n * spp, 0);
            if ifd.planar {
                for (c, r) in rows.iter().enumerate().take(spp) {
                    let lin = Some(c) == alpha_idx;
                    for x in x0..x1 {
                        scratch[(x - x0) * spp + c] = self.sample8(r, x, lin);
                    }
                }
            } else if ifd.bits == 16
                && ifd.sample_format == SampleFormat::Uint
                && self.le == cfg!(target_endian = "little")
            {
                let src = &rows[0][x0 * spp * 2..x1 * spp * 2];
                for (d, s) in scratch.iter_mut().zip(src.chunks_exact(2)) {
                    let v = u16::from_ne_bytes([s[0], s[1]]) as u32;
                    *d = ((v * 255 + 32767) / 65535) as u8;
                }
            } else {
                for x in x0..x1 {
                    for c in 0..spp {
                        scratch[(x - x0) * spp + c] =
                            self.sample8(rows[0], x * spp + c, Some(c) == alpha_idx);
                    }
                }
            }
            scratch
        };
        // Stage 2: colour model → output.
        let (ai, premul) = match ifd.alpha {
            Some((i, p)) if i < spp => (Some(i), p),
            _ => (None, false),
        };
        match self.target {
            PixelFormat::Gray8 => {
                let inv = ifd.photometric == Photometric::MinIsWhite;
                for (d, s) in dst[..n].iter_mut().zip(samples.chunks_exact(spp)) {
                    *d = if inv { 255 - s[0] } else { s[0] };
                }
            }
            PixelFormat::Rgba8 => {
                let dst = &mut dst[..n * 4];
                match ifd.photometric {
                    Photometric::Rgb if spp == 3 => rgb_to_rgba(samples, dst),
                    Photometric::Rgb if spp == 4 && ai == Some(3) && !premul => {
                        dst.copy_from_slice(samples);
                    }
                    Photometric::Rgb => {
                        for (d, s) in dst.chunks_exact_mut(4).zip(samples.chunks_exact(spp)) {
                            d.copy_from_slice(&[s[0], s[1], s[2], ai.map_or(255, |i| s[i])]);
                        }
                    }
                    Photometric::MinIsBlack | Photometric::MinIsWhite => {
                        let inv = ifd.photometric == Photometric::MinIsWhite;
                        for (d, s) in dst.chunks_exact_mut(4).zip(samples.chunks_exact(spp)) {
                            let g = if inv { 255 - s[0] } else { s[0] };
                            d.copy_from_slice(&[g, g, g, ai.map_or(255, |i| s[i])]);
                        }
                    }
                    Photometric::Cmyk if ifd.inks.is_some() => {
                        // Each ink filters the light by its colour, in proportion to its
                        // amount (for C, M, Y, K this is the naive CMYK formula below).
                        let inks = ifd.inks.as_deref().unwrap_or(&[]);
                        for (d, s) in dst.chunks_exact_mut(4).zip(samples.chunks_exact(spp)) {
                            let mut rgb = [255u32 * 255; 3];
                            for (&t, ink) in s.iter().zip(inks) {
                                for (v, &c) in rgb.iter_mut().zip(ink) {
                                    let keep = 255 * 255 - t as u32 * (255 - c as u32);
                                    *v = (*v as u64 * keep as u64 / (255 * 255)) as u32;
                                }
                            }
                            let f = |v: u32| ((v + 127) / 255) as u8;
                            d.copy_from_slice(&[
                                f(rgb[0]),
                                f(rgb[1]),
                                f(rgb[2]),
                                ai.map_or(255, |i| s[i]),
                            ]);
                        }
                    }
                    Photometric::Cmyk => {
                        if let Some(conv) = &self.cmyk {
                            if spp == 4 {
                                conv(samples, dst);
                            } else {
                                let ink: Vec<u8> = samples
                                    .chunks_exact(spp)
                                    .flat_map(|s| [s[0], s[1], s[2], s[3]])
                                    .collect();
                                conv(&ink, dst);
                                if let Some(i) = ai {
                                    for (d, s) in
                                        dst.chunks_exact_mut(4).zip(samples.chunks_exact(spp))
                                    {
                                        d[3] = s[i];
                                    }
                                }
                            }
                        } else {
                            for (d, s) in dst.chunks_exact_mut(4).zip(samples.chunks_exact(spp)) {
                                let k = 255 - s[3] as u32;
                                let f = |c: u8| (((255 - c as u32) * k + 127) / 255) as u8;
                                d.copy_from_slice(&[
                                    f(s[0]),
                                    f(s[1]),
                                    f(s[2]),
                                    ai.map_or(255, |i| s[i]),
                                ]);
                            }
                        }
                    }
                    Photometric::YCbCr => {
                        for (d, s) in dst.chunks_exact_mut(4).zip(samples.chunks_exact(spp)) {
                            let (y, cb, cr) =
                                (s[0] as f32, s[1] as f32 - 128.0, s[2] as f32 - 128.0);
                            let c = |v: f32| (v + 0.5).clamp(0.0, 255.0) as u8;
                            d.copy_from_slice(&[
                                c(y + 1.402 * cr),
                                c(y - 0.344_136 * cb - 0.714_136 * cr),
                                c(y + 1.772 * cb),
                                ai.map_or(255, |i| s[i]),
                            ]);
                        }
                    }
                    _ => dst.fill(128),
                }
                if premul && deep_premul.is_none() {
                    for d in dst.chunks_exact_mut(4) {
                        let a = d[3] as u32;
                        if a != 0 && a != 255 {
                            for c in &mut d[..3] {
                                *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
                            }
                        }
                    }
                }
            }
            // Handled by `row16` / `row_f16` above.
            PixelFormat::Gray16
            | PixelFormat::GrayAlpha16
            | PixelFormat::Rgba16
            | PixelFormat::Rgba16F
            | PixelFormat::GrayF32 => {}
        }
    }
}

/// RGB → opaque RGBA, the hot path of 8-bit RGB images. Kept out of line so the loop always
/// sees two non-aliasing slice arguments and vectorises, whether or not [`Converter::row`]
/// (which has grown with L*a*b* and has two callers) is inlined.
#[inline(never)]
fn rgb_to_rgba(src: &[u8], dst: &mut [u8]) {
    for (d, s) in dst.chunks_exact_mut(4).zip(src.chunks_exact(3)) {
        d.copy_from_slice(&[s[0], s[1], s[2], 255]);
    }
}

/// Grey (or transparent black for sparse units) fill value for missing data.
pub(crate) fn fill_missing(dst: &mut [u8], fmt: PixelFormat, grey: bool) {
    match (fmt, grey) {
        (_, false) => dst.fill(0),
        (PixelFormat::Gray8, true) => dst.fill(128),
        (PixelFormat::Rgba8, true) => {
            for p in dst.chunks_exact_mut(4) {
                p.copy_from_slice(&[128, 128, 128, 255]);
            }
        }
        (PixelFormat::Gray16, true) => fill16(dst, &[32768]),
        (PixelFormat::GrayAlpha16, true) => fill16(dst, &[32768, 65535]),
        (PixelFormat::Rgba16, true) => fill16(dst, &[32768, 32768, 32768, 65535]),
        (PixelFormat::GrayF32, _) => {
            for d in dst.chunks_exact_mut(4) {
                d.copy_from_slice(&f32::NAN.to_ne_bytes());
            }
        }
        (PixelFormat::Rgba16F, true) => {
            // sRGB 50 % grey in linear light, opaque.
            let g = f32_to_half(0.2158);
            fill16(dst, &[g, g, g, f32_to_half(1.0)]);
        }
    }
}

/// Fill `dst` with repeats of the native-endian 16-bit pixel `px`.
fn fill16(dst: &mut [u8], px: &[u16]) {
    let px: Vec<u8> = px.iter().flat_map(|v| v.to_ne_bytes()).collect();
    for d in dst.chunks_exact_mut(px.len()) {
        d.copy_from_slice(&px);
    }
}

/// Box-filter `src` (`sw`×`sh` pixels of `fmt`, row stride `ss`) by `f`, averaging in linear
/// light with alpha weighting, into output rows `dst` (each `ceil(sw / f)` pixels). Output row
/// `j` covers source rows `j*f..min((j+1)*f, sh)`.
pub(crate) fn box_reduce(
    src: &[u8],
    ss: usize,
    sw: usize,
    sh: usize,
    fmt: PixelFormat,
    f: usize,
    dst: &mut [&mut [u8]],
) {
    if fmt.is_wide() {
        return box_reduce16(src, ss, sw, sh, fmt, f, dst);
    }
    if fmt == PixelFormat::Rgba16F {
        return box_reduce_f16(src, ss, sw, sh, f, dst);
    }
    if fmt == PixelFormat::GrayF32 {
        return box_reduce_f32(src, ss, sw, sh, f, dst);
    }
    let bpp = fmt.bytes_per_pixel();
    let l = lut();
    let ow = sw.div_ceil(f);
    let mut acc = vec![0u64; ow * 5];
    for (j, d) in dst.iter_mut().enumerate() {
        let y0 = j * f;
        let y1 = ((j + 1) * f).min(sh);
        if y0 >= y1 {
            break;
        }
        acc.fill(0);
        for y in y0..y1 {
            let row = &src[y * ss..y * ss + sw * bpp];
            if bpp == 1 {
                for (x, &v) in row.iter().enumerate() {
                    let a = &mut acc[(x / f) * 5..];
                    a[0] += l.to_lin[v as usize] as u64;
                    a[4] += 1;
                }
            } else {
                for (x, p) in row.chunks_exact(4).enumerate() {
                    let a = &mut acc[(x / f) * 5..(x / f) * 5 + 5];
                    let al = p[3] as u64;
                    if al == 255 {
                        a[0] += l.to_lin[p[0] as usize] as u64 * 255;
                        a[1] += l.to_lin[p[1] as usize] as u64 * 255;
                        a[2] += l.to_lin[p[2] as usize] as u64 * 255;
                    } else if al != 0 {
                        a[0] += l.to_lin[p[0] as usize] as u64 * al;
                        a[1] += l.to_lin[p[1] as usize] as u64 * al;
                        a[2] += l.to_lin[p[2] as usize] as u64 * al;
                    }
                    a[3] += al;
                    a[4] += 1;
                }
            }
        }
        for (x, a) in acc.chunks_exact(5).enumerate() {
            let cnt = a[4].max(1);
            if bpp == 1 {
                d[x] = l.to_srgb[((a[0] + cnt / 2) / cnt) as usize];
            } else {
                let o = &mut d[x * 4..x * 4 + 4];
                if a[3] == 0 {
                    o.copy_from_slice(&[0, 0, 0, 0]);
                } else {
                    let sa = a[3];
                    for c in 0..3 {
                        o[c] = l.to_srgb[((a[c] + sa / 2) / sa).min(65535) as usize];
                    }
                    o[3] = ((a[3] + cnt / 2) / cnt) as u8;
                }
            }
        }
    }
}

/// [`box_reduce`] for the 16-bit formats: the same linear-light, alpha-weighted mean, in
/// floating point.
fn box_reduce16(
    src: &[u8],
    ss: usize,
    sw: usize,
    sh: usize,
    fmt: PixelFormat,
    f: usize,
    dst: &mut [&mut [u8]],
) {
    let lin = to_lin16();
    // Colour samples and whether one more sample, alpha, follows.
    let (nc, alpha) = match fmt {
        PixelFormat::Gray16 => (1, false),
        PixelFormat::GrayAlpha16 => (1, true),
        _ => (3, true),
    };
    let spp = nc + alpha as usize;
    let ow = sw.div_ceil(f);
    // Per output pixel: alpha-weighted linear colour sums, alpha sum, pixel count.
    let mut acc = vec![0f64; ow * 5];
    let s16 = |b: &[u8], i: usize| u16::from_ne_bytes([b[2 * i], b[2 * i + 1]]);
    for (j, d) in dst.iter_mut().enumerate() {
        let y0 = j * f;
        let y1 = ((j + 1) * f).min(sh);
        if y0 >= y1 {
            break;
        }
        acc.fill(0.0);
        for y in y0..y1 {
            let row = &src[y * ss..y * ss + sw * spp * 2];
            for (x, p) in row.chunks_exact(spp * 2).enumerate() {
                let a = &mut acc[(x / f) * 5..(x / f) * 5 + 5];
                let w = if alpha { s16(p, nc) as f64 } else { 65535.0 };
                if w != 0.0 {
                    for c in 0..nc {
                        a[c] += lin[s16(p, c) as usize] as f64 * w;
                    }
                }
                a[3] += w;
                a[4] += 1.0;
            }
        }
        for (x, a) in acc.chunks_exact(5).enumerate() {
            let o = &mut d[x * spp * 2..(x + 1) * spp * 2];
            let mut px = [0u16; 4];
            if a[3] > 0.0 {
                for c in 0..nc {
                    px[c] = enc16(a[c] / a[3]);
                }
                if alpha {
                    px[nc] = (a[3] / a[4].max(1.0) + 0.5) as u16;
                }
            }
            for (o, v) in o.chunks_exact_mut(2).zip(&px) {
                o.copy_from_slice(&v.to_ne_bytes());
            }
        }
    }
}

/// [`box_reduce`] for [`PixelFormat::Rgba16F`]: the samples are linear already; an
/// alpha-weighted mean.
fn box_reduce_f16(src: &[u8], ss: usize, sw: usize, sh: usize, f: usize, dst: &mut [&mut [u8]]) {
    let ow = sw.div_ceil(f);
    let mut acc = vec![0f64; ow * 5];
    let h = |p: &[u8], i: usize| half_to_f32(u16::from_ne_bytes([p[2 * i], p[2 * i + 1]])) as f64;
    for (j, d) in dst.iter_mut().enumerate() {
        let y0 = j * f;
        let y1 = ((j + 1) * f).min(sh);
        if y0 >= y1 {
            break;
        }
        acc.fill(0.0);
        for y in y0..y1 {
            let row = &src[y * ss..y * ss + sw * 8];
            for (x, p) in row.chunks_exact(8).enumerate() {
                let a = &mut acc[(x / f) * 5..(x / f) * 5 + 5];
                let w = h(p, 3);
                if w != 0.0 {
                    for (c, v) in a[..3].iter_mut().enumerate() {
                        *v += h(p, c) * w;
                    }
                }
                a[3] += w;
                a[4] += 1.0;
            }
        }
        for (x, a) in acc.chunks_exact(5).enumerate() {
            let o = &mut d[x * 8..(x + 1) * 8];
            let mut px = [0u16; 4];
            if a[3] > 0.0 {
                for c in 0..3 {
                    px[c] = f32_to_half((a[c] / a[3]) as f32);
                }
                px[3] = f32_to_half((a[3] / a[4].max(1.0)) as f32);
            }
            for (o, v) in o.chunks_exact_mut(2).zip(&px) {
                o.copy_from_slice(&v.to_ne_bytes());
            }
        }
    }
}

/// [`box_reduce`] for [`PixelFormat::GrayF32`]: the mean of the finite values (NaN if none).
fn box_reduce_f32(src: &[u8], ss: usize, sw: usize, sh: usize, f: usize, dst: &mut [&mut [u8]]) {
    let ow = sw.div_ceil(f);
    let mut acc = vec![(0f64, 0u32); ow];
    for (j, d) in dst.iter_mut().enumerate() {
        let y0 = j * f;
        let y1 = ((j + 1) * f).min(sh);
        if y0 >= y1 {
            break;
        }
        acc.fill((0.0, 0));
        for y in y0..y1 {
            let row = &src[y * ss..y * ss + sw * 4];
            for (x, p) in row.chunks_exact(4).enumerate() {
                let v = f32::from_ne_bytes([p[0], p[1], p[2], p[3]]);
                if v.is_finite() {
                    let a = &mut acc[x / f];
                    a.0 += v as f64;
                    a.1 += 1;
                }
            }
        }
        for (o, a) in d.chunks_exact_mut(4).zip(&acc) {
            let v = if a.1 == 0 {
                f32::NAN
            } else {
                (a.0 / a.1 as f64) as f32
            };
            o.copy_from_slice(&v.to_ne_bytes());
        }
    }
}

/// Box-filter a decoded image by `f` (gamma-correct, alpha-weighted), e.g. to derive coarser
/// levels from a cached full-resolution decode. The origin is divided by `f` (it should be a
/// multiple of `f`).
pub fn downscale(src: &Decoded, f: u32) -> Decoded {
    let f = f.max(1) as usize;
    let bpp = src.format.bytes_per_pixel();
    let (w, h) = (
        (src.width as usize).div_ceil(f),
        (src.height as usize).div_ceil(f),
    );
    let mut data = vec![0u8; w * h * bpp];
    {
        let mut rows: Vec<&mut [u8]> = data.chunks_mut((w * bpp).max(1)).collect();
        box_reduce(
            &src.data,
            src.stride,
            src.width as usize,
            src.height as usize,
            src.format,
            f,
            &mut rows,
        );
    }
    Decoded {
        width: w as u32,
        height: h as u32,
        origin: (src.origin.0 / f as u32, src.origin.1 / f as u32),
        format: src.format,
        stride: w * bpp,
        data,
        truncated: src.truncated,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_half() {
        for v in [
            0.0f32,
            1.0,
            -2.0,
            0.333_251_95,
            65504.0,
            5.960_464_5e-8,
            // Smallest normal half (2^-14) and a subnormal (3 · 2^-24).
            6.103_515_6e-5,
            1.788_139_3e-7,
            1234.0,
            -0.5,
        ] {
            assert_eq!(half_to_f32(f32_to_half(v)), v, "{v}");
        }
        assert_eq!(f32_to_half(1e9), 0x7bff);
        assert_eq!(f32_to_half(f32::NAN), 0);
        assert_eq!(f32_to_half(1.0 + 1.0 / 4096.0), 0x3c00);
        // Ties go to even: 1234.5 → 1234, 1235.5 → 1236.
        assert_eq!(half_to_f32(f32_to_half(1234.5)), 1234.0);
        assert_eq!(half_to_f32(f32_to_half(1235.5)), 1236.0);
    }

    #[test]
    fn float24() {
        // 1.0: exponent 63, mantissa 0; -2.5: exponent 64, mantissa 0x4000.
        assert_eq!(f24_to_f32(63 << 16), 1.0);
        assert_eq!(f24_to_f32((1 << 23) | (64 << 16) | 0x4000), -2.5);
        assert_eq!(f24_to_f32(1), (-78f32).exp2());
    }

    #[test]
    fn half() {
        assert_eq!(half_to_f32(0x3c00), 1.0);
        assert_eq!(half_to_f32(0xc000), -2.0);
        assert_eq!(half_to_f32(0x0001), 5.960_464_5e-8);
        assert_eq!(half_to_f32(0x3555), 0.333_251_95);
    }

    #[test]
    fn box_flat() {
        let src = vec![200u8; 5 * 3 * 4];
        let mut out = [0u8; 3 * 2 * 4];
        let mut rows: Vec<&mut [u8]> = out.chunks_mut(12).collect();
        box_reduce(&src, 20, 5, 3, PixelFormat::Rgba8, 2, &mut rows);
        assert!(out.iter().all(|&v| v == 200));
    }

    #[test]
    fn box_flat16() {
        for (fmt, px) in [
            (PixelFormat::Gray16, vec![51234u16]),
            (PixelFormat::GrayAlpha16, vec![51234, 40000]),
            (PixelFormat::Rgba16, vec![1, 30000, 65535, 65535]),
        ] {
            let bpp = fmt.bytes_per_pixel();
            let one: Vec<u8> = px.iter().flat_map(|v| v.to_ne_bytes()).collect();
            let src = one.repeat(5 * 3);
            let mut out = vec![0u8; 3 * 2 * bpp];
            let mut rows: Vec<&mut [u8]> = out.chunks_mut(3 * bpp).collect();
            box_reduce(&src, 5 * bpp, 5, 3, fmt, 2, &mut rows);
            assert_eq!(out, one.repeat(6), "{fmt:?}");
        }
    }

    #[test]
    fn box_mean16() {
        // Black and white average to linear 0.5, sRGB 0.73536 (48192 of 65535); fully
        // transparent pixels do not count.
        let px = |v: &[u16]| v.iter().flat_map(|v| v.to_ne_bytes()).collect::<Vec<u8>>();
        let src = [px(&[0, 65535]), px(&[65535, 65535])].concat();
        let mut out = [0u8; 4];
        box_reduce(
            &src,
            8,
            2,
            1,
            PixelFormat::GrayAlpha16,
            2,
            &mut [&mut out[..]],
        );
        let g = u16::from_ne_bytes([out[0], out[1]]);
        assert_eq!(g, 48192);
        assert_eq!(u16::from_ne_bytes([out[2], out[3]]), 65535);
        let src = [px(&[0, 0]), px(&[1000, 65535])].concat();
        box_reduce(
            &src,
            8,
            2,
            1,
            PixelFormat::GrayAlpha16,
            2,
            &mut [&mut out[..]],
        );
        let g = u16::from_ne_bytes([out[0], out[1]]);
        assert!(g.abs_diff(1000) <= 1, "{g}");
        assert_eq!(u16::from_ne_bytes([out[2], out[3]]), 32768);
    }
}
