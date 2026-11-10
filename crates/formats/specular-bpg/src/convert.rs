//! Decoded planes to RGB(A), as the BPG reference decoder converts them: chroma upsampled
//! with its Lanczos filters (7 taps at phase ½, 8 taps between co-sited samples; vertically
//! always at ½, horizontally at ½ for JPEG siting and co-sited for MPEG-2 siting), the
//! fixed-point matrices of BT.601/709/2020, YCgCo, GBR, the limited range, alpha (straight,
//! premultiplied, or the W plane of CMYK, multiplied in).

use crate::{Chroma, ColourSpace, PixelFormat};

/// 8 taps at phase 0 (symmetric half: centre, then outwards).
const IP0: [i32; 4] = [40, -11, 4, -1];
/// 7 taps at phase ½.
const IP1: [i32; 7] = [-1, 4, -10, 57, 18, -6, 2];

/// What the conversion needs from the header.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Params {
    pub width: u32,
    pub height: u32,
    pub chroma: Chroma,
    /// MPEG-2 chroma siting (horizontally co-sited).
    pub video_siting: bool,
    pub bit_depth: u8,
    pub colour_space: ColourSpace,
    pub limited_range: bool,
    pub alpha: Option<AlphaUse>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AlphaUse {
    Straight,
    Premultiplied,
    /// CMYK's W = 1 - K, multiplied into the colour.
    White,
}

/// A plane as the HEVC decoder hands it out.
pub(crate) struct Plane<'a> {
    pub data: &'a [u16],
    pub stride: usize,
}

impl Plane<'_> {
    fn row(&self, y: usize) -> &[u16] {
        let s = (y * self.stride).min(self.data.len());
        &self.data[s..]
    }
}

/// The reference decoder's fixed-point constants for one output depth.
struct Coeffs {
    c_r_cr: i64,
    c_g_cb: i64,
    c_g_cr: i64,
    c_b_cb: i64,
    c_one: i64,
    c_shift: u32,
    c_rnd: i64,
    c_center: i64,
    y_one: i64,
    y_offset: i64,
}

/// C's `lrint` (round half to even).
fn lrint(v: f64) -> i64 {
    v.round_ties_even() as i64
}

fn coeffs(in_bits: u8, out_bits: u32, cs: ColourSpace, limited: bool) -> Coeffs {
    let c_shift = 30 - out_bits;
    let in_max = f64::from((1u32 << in_bits) - 1);
    let out_max = f64::from((1u32 << out_bits) - 1);
    let scale = f64::from(1u32 << c_shift);
    let mult = out_max * scale / in_max;
    let (mult_y, mult_c) = if limited {
        (
            out_max * scale / f64::from(219u32 << (in_bits - 8)),
            out_max * scale / f64::from(224u32 << (in_bits - 8)),
        )
    } else {
        (mult, mult)
    };
    let (k_r, k_b) = match cs {
        ColourSpace::YCbCr709 => (0.2126, 0.0722),
        ColourSpace::YCbCr2020 => (0.2627, 0.0593),
        _ => (0.299, 0.114),
    };
    let c_rnd = 1i64 << (c_shift - 1);
    let c_one = lrint(mult);
    let (y_one, y_offset) = if limited {
        let y_one = lrint(mult_y);
        (y_one, -(16i64 << (in_bits - 8)) * y_one + c_rnd)
    } else {
        (c_one, c_rnd)
    };
    Coeffs {
        c_r_cr: lrint(2.0 * (1.0 - k_r) * mult_c),
        c_g_cb: lrint(2.0 * k_b * (1.0 - k_b) / (1.0 - k_b - k_r) * mult_c),
        c_g_cr: lrint(2.0 * k_r * (1.0 - k_r) / (1.0 - k_b - k_r) * mult_c),
        c_b_cb: lrint(2.0 * (1.0 - k_b) * mult_c),
        c_one,
        c_shift,
        c_rnd,
        c_center: 1 << (in_bits - 1),
        y_one,
        y_offset,
    }
}

/// Horizontal ×2 interpolation of `n2` = ceil(`n`/2) samples (padded by edge repetition)
/// to `n`, at `phase` ½ (JPEG siting) or co-sited. `pre` is the precision shift of the
/// input (0 for samples, 14 − depth after the vertical pass) and `bits` the sample depth.
fn interp_h(dst: &mut [i32], src: &[i32], n: usize, half_phase: bool, pre: u32, bits: u8) {
    let n2 = n.div_ceil(2);
    let max = (1i32 << bits) - 1;
    let at = |i: isize| src[(i.clamp(0, n2 as isize - 1)) as usize];
    let shift = 6 + pre;
    let rnd = 1i32 << (shift - 1);
    let clamp = |v: i32| v.clamp(0, max);
    for k in 0..n2 {
        let i = k as isize;
        if half_phase {
            let a: [i32; 7] = std::array::from_fn(|j| at(i - 3 + j as isize));
            let lo = a[0] * IP1[6]
                + a[1] * IP1[5]
                + a[2] * IP1[4]
                + a[3] * IP1[3]
                + a[4] * IP1[2]
                + a[5] * IP1[1]
                + a[6] * IP1[0];
            dst[2 * k] = clamp((lo + rnd) >> shift);
            if 2 * k + 1 < n {
                let hi = a[0] * IP1[0]
                    + a[1] * IP1[1]
                    + a[2] * IP1[2]
                    + a[3] * IP1[3]
                    + a[4] * IP1[4]
                    + a[5] * IP1[5]
                    + a[6] * IP1[6];
                dst[2 * k + 1] = clamp((hi + rnd) >> shift);
            }
        } else {
            dst[2 * k] = if pre == 0 {
                at(i)
            } else {
                let r = (1i32 << pre) >> 1;
                clamp((at(i) + r) >> pre)
            };
            if 2 * k + 1 < n {
                let v = (at(i - 3) + at(i + 4)) * IP0[3]
                    + (at(i - 2) + at(i + 3)) * IP0[2]
                    + (at(i - 1) + at(i + 2)) * IP0[1]
                    + (at(i) + at(i + 1)) * IP0[0];
                dst[2 * k + 1] = clamp((v + rnd) >> shift);
            }
        }
    }
}

/// Converts the planes of one frame. `planes` are Y, Cb, Cr (Y only for grey); `alpha` is
/// the alpha (or W) plane. The planes may be larger than the picture (coded size).
pub(crate) fn to_pixels(
    p: &Params,
    planes: &[Plane<'_>],
    alpha: Option<&Plane<'_>>,
    out: &mut Vec<u8>,
) -> PixelFormat {
    let wide = p.bit_depth > 8;
    let grey_out = p.chroma == Chroma::Gray && p.alpha.is_none();
    let format = match (wide, grey_out) {
        (false, true) => PixelFormat::Gray8,
        (false, false) => PixelFormat::Rgba8,
        (true, true) => PixelFormat::Gray16,
        (true, false) => PixelFormat::Rgba16,
    };
    let out_bits = if wide { 16 } else { 8 };
    let out_max = (1i64 << out_bits) - 1;
    let k = coeffs(p.bit_depth, out_bits, p.colour_space, p.limited_range);
    let (w, h) = (p.width as usize, p.height as usize);
    let channels = if grey_out { 1 } else { 4 };
    let sample_bytes = if wide { 2 } else { 1 };
    out.clear();
    out.resize(w * h * channels * sample_bytes, 0);
    let row_bytes = w * channels * sample_bytes;
    let w2 = w.div_ceil(2);
    let h2 = h.div_ceil(2);
    let half_h = !p.video_siting;
    let mut cb = vec![0i32; w];
    let mut cr = vec![0i32; w];
    let mut tmp = vec![0i32; w2];
    let mut px = vec![[0i64; 4]; w];
    let bits = p.bit_depth;
    let vshift = u32::from(bits) - 8;
    let vrnd = (1i32 << vshift) >> 1;
    for (y, orow) in out.chunks_exact_mut(row_bytes.max(1)).enumerate().take(h) {
        let yrow = planes[0].row(y);
        match p.chroma {
            Chroma::Gray => {}
            Chroma::C444 => {
                for (c, plane) in [(&mut cb, &planes[1]), (&mut cr, &planes[2])] {
                    let r = plane.row(y);
                    for (x, v) in c.iter_mut().enumerate() {
                        *v = i32::from(r.get(x).copied().unwrap_or(0));
                    }
                }
            }
            Chroma::C422 => {
                for (c, plane) in [(&mut cb, &planes[1]), (&mut cr, &planes[2])] {
                    let r = plane.row(y);
                    for (x, t) in tmp.iter_mut().enumerate() {
                        *t = i32::from(r.get(x).copied().unwrap_or(0));
                    }
                    interp_h(c, &tmp, w, half_h, 0, bits);
                }
            }
            Chroma::C420 => {
                let y2 = (y >> 1) as isize;
                let odd = y & 1 == 1;
                for (c, plane) in [(&mut cb, &planes[1]), (&mut cr, &planes[2])] {
                    let rows: [&[u16]; 7] = std::array::from_fn(|j| {
                        plane.row((y2 - 3 + j as isize).clamp(0, h2 as isize - 1) as usize)
                    });
                    for (x, t) in tmp.iter_mut().enumerate() {
                        let mut s = 0;
                        for (j, r) in rows.iter().enumerate() {
                            let v = i32::from(r.get(x).copied().unwrap_or(0));
                            s += v * if odd { IP1[j] } else { IP1[6 - j] };
                        }
                        // The reference keeps this in 16 bits.
                        *t = i32::from(((s + vrnd) >> vshift) as i16);
                    }
                    interp_h(c, &tmp, w, half_h, 14 - u32::from(bits), bits);
                }
            }
        }
        // Colour, at the output depth.
        for x in 0..w {
            let yv = i64::from(yrow.get(x).copied().unwrap_or(0));
            let (cbv, crv) = (i64::from(cb[x]), i64::from(cr[x]));
            let clamp = |v: i64| v.clamp(0, out_max);
            let s = k.c_shift;
            px[x] = if p.chroma == Chroma::Gray {
                let g = clamp((yv * k.y_one + k.y_offset) >> s);
                [g, g, g, out_max]
            } else {
                match p.colour_space {
                    ColourSpace::Rgb => [
                        clamp((crv * k.y_one + k.y_offset) >> s),
                        clamp((yv * k.y_one + k.y_offset) >> s),
                        clamp((cbv * k.y_one + k.y_offset) >> s),
                        out_max,
                    ],
                    ColourSpace::YCgCo => {
                        let (cg, co) = (cbv - k.c_center, crv - k.c_center);
                        [
                            clamp(((yv - cg + co) * k.y_one + k.y_offset) >> s),
                            clamp(((yv + cg) * k.y_one + k.y_offset) >> s),
                            clamp(((yv - cg - co) * k.y_one + k.y_offset) >> s),
                            out_max,
                        ]
                    }
                    _ => {
                        let yy = yv * k.y_one;
                        let (cbv, crv) = (cbv - k.c_center, crv - k.c_center);
                        [
                            clamp((yy + k.c_r_cr * crv + k.y_offset) >> s),
                            clamp((yy - k.c_g_cb * cbv - k.c_g_cr * crv + k.y_offset) >> s),
                            clamp((yy + k.c_b_cb * cbv + k.y_offset) >> s),
                            out_max,
                        ]
                    }
                }
            };
        }
        if let (Some(a), Some(kind)) = (alpha, p.alpha) {
            let arow = a.row(y);
            let ashift = u32::from(bits);
            let arnd = 1i64 << (ashift - 1);
            for (x, q) in px.iter_mut().enumerate() {
                let av = i64::from(arow.get(x).copied().unwrap_or(0));
                match kind {
                    AlphaUse::White => {
                        for c in &mut q[..3] {
                            *c = (*c * av + arnd) >> ashift;
                        }
                    }
                    AlphaUse::Straight | AlphaUse::Premultiplied => {
                        q[3] = if !wide && bits == 8 {
                            av
                        } else {
                            (av * k.c_one + k.c_rnd) >> k.c_shift
                        };
                        if kind == AlphaUse::Premultiplied {
                            unpremultiply(q, wide);
                        }
                    }
                }
            }
        }
        // Store.
        match format {
            PixelFormat::Gray8 => {
                for (o, q) in orow.iter_mut().zip(&px) {
                    *o = q[0] as u8;
                }
            }
            PixelFormat::Rgba8 => {
                for (o, q) in orow.chunks_exact_mut(4).zip(&px) {
                    for c in 0..4 {
                        o[c] = q[c] as u8;
                    }
                }
            }
            PixelFormat::Gray16 => {
                for (o, q) in orow.chunks_exact_mut(2).zip(&px) {
                    o.copy_from_slice(&(q[0] as u16).to_ne_bytes());
                }
            }
            PixelFormat::Rgba16 => {
                for (o, q) in orow.chunks_exact_mut(8).zip(&px) {
                    for c in 0..4 {
                        o[2 * c..2 * c + 2].copy_from_slice(&(q[c] as u16).to_ne_bytes());
                    }
                }
            }
        }
    }
    format
}

/// The reference decoder's division of premultiplied colour by alpha (white where alpha
/// is 0).
fn unpremultiply(q: &mut [i64; 4], wide: bool) {
    let a = q[3] as u64;
    let (max, bits): (u64, u32) = if wide { (65535, 15) } else { (255, 16) };
    if a == 0 {
        q[0] = max as i64;
        q[1] = max as i64;
        q[2] = max as i64;
        return;
    }
    let inv = if wide {
        ((65535u64 << bits) + a / 2) / a
    } else {
        ((255u64 << bits) + a / 2 + 128) / a
    };
    for c in &mut q[..3] {
        let v = *c as u64;
        *c = if v >= a {
            max as i64
        } else {
            ((v * inv + (1 << (bits - 1))) >> bits) as i64
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_range_coefficients() {
        let k = coeffs(8, 8, ColourSpace::YCbCr601, false);
        // mult = 255 * 2^22 / 255 = 2^22.
        assert_eq!(k.c_one, 1 << 22);
        assert_eq!(k.c_r_cr, lrint(1.402 * f64::from(1 << 22)));
        assert_eq!(k.y_offset, 1 << 21);
    }

    #[test]
    fn unpremultiply_matches_reference_rules() {
        let mut q = [64, 128, 200, 128];
        unpremultiply(&mut q, false);
        assert_eq!(q, [128, 255, 255, 128]);
        let mut q = [10, 10, 10, 0];
        unpremultiply(&mut q, false);
        assert_eq!(q[..3], [255, 255, 255]);
    }
}
