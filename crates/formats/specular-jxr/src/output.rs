//! Output formatting (T.832 9.10): chroma upsampling, colour conversion, bias, scaling,
//! postscaling, clipping, packing and windowing, into the stored sample layout of the pixel
//! format ("native" samples).
//!
//! 4:2:0 / 4:2:2 chroma is upsampled the way the reference software does (the standard leaves
//! it open): even positions copy the co-sited sample, odd ones average their neighbours rounding
//! up, and the last row/column repeats; 4:2:0 is interpolated vertically first.

use rayon::prelude::*;

use crate::header::{BitDepth, ClrFmt, ImageHeader, OutFmt, PlaneHeader};
use crate::parse::Plane;

/// Stored samples, interleaved: BD1 and BD8 and RGBE as bytes (BD1 one 0/1 value per sample),
/// BD16/BD16S/BD16F and the packed BD5/BD565 as 16-bit words (BD16S as two's complement,
/// BD16F as half-float bits), BD32S/BD32F as 32-bit words (BD32F as float bits) and BD10 as
/// 16-bit words or, packed RGB, 32-bit words.
#[derive(Clone, Debug, PartialEq)]
pub enum Samples {
    U8(Vec<u8>),
    U16(Vec<u16>),
    U32(Vec<u32>),
}

/// Channels the plane formats to (after packing).
pub(crate) fn channels(out: OutFmt, bd: BitDepth, nc: usize) -> usize {
    match out {
        OutFmt::Rgb if matches!(bd, BitDepth::Bd5 | BitDepth::Bd565 | BitDepth::Bd10) => 1,
        OutFmt::Rgb => 3,
        OutFmt::Rgbe | OutFmt::Cmyk | OutFmt::CmykDirect => 4,
        _ => nc,
    }
}

pub(crate) enum Width {
    W8,
    W16,
    W32,
}

pub(crate) fn sample_width(out: OutFmt, bd: BitDepth) -> Width {
    match bd {
        BitDepth::Bd1White1 | BitDepth::Bd1Black1 | BitDepth::Bd8 => Width::W8,
        _ if out == OutFmt::Rgbe => Width::W8,
        BitDepth::Bd32S | BitDepth::Bd32F => Width::W32,
        BitDepth::Bd10 if out == OutFmt::Rgb => Width::W32,
        _ => Width::W16,
    }
}

struct Fmt<'a> {
    ih: &'a ImageHeader,
    ph: &'a PlaneHeader,
    out: OutFmt,
    bd: BitDepth,
    comp: Vec<&'a [i32]>,
    cw: Vec<usize>,
    /// Chroma rows per component plane (4:2:0 / 4:2:2 only).
    ch: usize,
    ext_w: usize,
    /// Set (to the primary plane's header) when formatting an interleaved alpha plane: the
    /// reference software formats it with the primary plane's SCALED, SHIFT_BITS and float
    /// parameters, rounds with 4 (not 3) when scaled, and does not pre-shift the BD16 bias.
    primary: Option<&'a PlaneHeader>,
}

impl Fmt<'_> {
    fn sub(&self) -> bool {
        matches!(self.ph.clr, ClrFmt::Yuv420 | ClrFmt::Yuv422)
    }

    /// Upsampled chroma of component `i` for extended row `y` into `dst` (extended width).
    fn chroma_row(&self, i: usize, y: usize, tmp: &mut [i32], dst: &mut [i32]) {
        let c = self.comp[i];
        let cw = self.cw[i];
        if self.ph.clr == ClrFmt::Yuv420 {
            let k = y >> 1;
            let r0 = &c[k * cw..(k + 1) * cw];
            if y & 1 == 0 || k + 1 >= self.ch {
                tmp[..cw].copy_from_slice(r0);
            } else {
                let r1 = &c[(k + 1) * cw..(k + 2) * cw];
                for ((t, &a), &b) in tmp.iter_mut().zip(r0).zip(r1) {
                    *t = (a + b + 1) >> 1;
                }
            }
        } else {
            tmp[..cw].copy_from_slice(&c[y * cw..(y + 1) * cw]);
        }
        for (x, d) in dst[..self.ext_w].iter_mut().enumerate() {
            let k = x >> 1;
            *d = if x & 1 == 0 || k + 1 >= cw {
                tmp[k]
            } else {
                (tmp[k] + tmp[k + 1] + 1) >> 1
            };
        }
    }

    /// One output row (extended row `y`) of native samples into `out`.
    fn row(&self, y: usize, out: &mut [u32], scratch: &mut [Vec<i32>; 3]) {
        let ih = self.ih;
        let ph = self.ph;
        let nc = ph.nc;
        let up = self.sub() && self.out != OutFmt::YOnly;
        if up {
            let [tmp, u, v] = scratch;
            self.chroma_row(1, y, tmp, u);
            self.chroma_row(2, y, tmp, v);
        }
        let pp = self.primary.unwrap_or(ph);
        let scaled = pp.scaled;
        let iscale = if scaled { 3 } else { 0 };
        let mut bias = match self.bd {
            BitDepth::Bd5 => 1 << 4,
            BitDepth::Bd565 => 1 << 5,
            BitDepth::Bd8 => 1 << 7,
            BitDepth::Bd10 => 1 << 9,
            BitDepth::Bd16 => 1 << 15,
            _ => 0,
        };
        let shift = pp.shift as u32;
        if self.primary.is_none()
            && matches!(self.bd, BitDepth::Bd16 | BitDepth::Bd16S | BitDepth::Bd32S)
        {
            bias >>= shift;
        }
        let round = if !scaled {
            0
        } else if self.primary.is_some()
            || matches!(
                self.bd,
                BitDepth::Bd1White1 | BitDepth::Bd1Black1 | BitDepth::Bd16
            )
        {
            4
        } else {
            3
        };
        let nout = match self.out {
            OutFmt::Rgb | OutFmt::Rgbe => 3,
            OutFmt::Cmyk | OutFmt::CmykDirect => 4,
            _ => nc,
        };
        let nch = channels(self.out, self.bd, nc);
        // RED_BLUE_NOT_SWAPPED_FLAG: Table 185 swaps R and B before packing when the flag is 0,
        // which contradicts Note 3 of A.7.18 (flag 0 puts R in the most significant bits) and
        // every existing encoder, whose first colour component is the low field. The packed
        // fields are written in component order; `decode` maps them back to RGB.
        let x0 = ih.left as usize;
        for (k, o) in out.chunks_exact_mut(nch).enumerate() {
            let x = x0 + k;
            let mut a = [0i32; 16];
            for (i, v) in a.iter_mut().enumerate().take(nc) {
                *v = if i > 0 && up {
                    scratch[i][x]
                } else {
                    self.comp[i][y * self.cw[i] + x]
                };
            }
            // Colour conversion (9.10.4).
            match (ph.clr, self.out) {
                (ClrFmt::YOnly, OutFmt::Rgb | OutFmt::Rgbe) => {
                    a[1] = a[0];
                    a[2] = a[0];
                }
                (ClrFmt::Yuv420 | ClrFmt::Yuv422 | ClrFmt::Yuv444, OutFmt::Rgb | OutFmt::Rgbe) => {
                    let t = -a[1];
                    let g = a[0] - (t >> 1);
                    let r = t + g - ((a[2] + 1) >> 1);
                    let b = a[2] + r;
                    (a[0], a[1], a[2]) = (r, g, b);
                }
                (ClrFmt::Yuvk, OutFmt::Cmyk) => {
                    let k = a[3] + (a[0] >> 1);
                    let m = k - a[0] - (a[1] >> 1);
                    let c = a[1] + m + (a[2] >> 1);
                    let yy = c - a[2];
                    a[..4].copy_from_slice(&[c, m, yy, k]);
                }
                (ClrFmt::Yuvk, OutFmt::CmykDirect) => {
                    let (yv, u, v, kk) = (a[0], a[1], a[2], a[3]);
                    a[..4].copy_from_slice(&[u, v, kk, yv]);
                }
                _ => {}
            }
            // AddBias (9.10.5).
            if self.out == OutFmt::Cmyk {
                let b = (bias >> 1) << iscale;
                for v in &mut a[..3] {
                    *v += b;
                }
                a[3] -= b;
            } else if self.out != OutFmt::Rgbe {
                // RGBE takes no bias although its OUTPUT_BITDEPTH is BD8.
                for v in &mut a[..nout] {
                    *v += bias << iscale;
                }
            }
            // ComputeScaling (9.10.6).
            for (i, v) in a.iter_mut().enumerate().take(nout) {
                let j = if self.bd == BitDepth::Bd565 && i != 1 {
                    iscale + 1
                } else {
                    iscale
                };
                *v = (*v + round) >> j;
            }
            // Postscaling, clipping and packing (9.10.7, 9.10.8).
            match (self.out, self.bd) {
                (OutFmt::Rgbe, _) => {
                    let mut m = [0i32; 3];
                    let mut e = [0i32; 3];
                    for i in 0..3 {
                        (m[i], e[i]) = if a[i] <= 0 {
                            (0, 0)
                        } else if (a[i] >> 7) > 1 {
                            ((a[i] & 0x7F) + 128, a[i] >> 7)
                        } else {
                            (a[i], 1)
                        };
                    }
                    let emax = e[0].max(e[1]).max(e[2]);
                    for i in 0..3 {
                        if emax > e[i] {
                            let s = (emax - e[i]) as u32;
                            m[i] = (2 * m[i] + 1) >> (s + 1).min(31);
                        }
                        o[i] = m[i] as u32;
                    }
                    o[3] = emax as u32;
                }
                (_, BitDepth::Bd565) => {
                    o[0] = (a[0].clamp(0, 31)
                        + (a[1].clamp(0, 63) << 5)
                        + (a[2].clamp(0, 31) << 11)) as u32;
                }
                (_, BitDepth::Bd5) => {
                    o[0] = (a[0].clamp(0, 31)
                        + (a[1].clamp(0, 31) << 5)
                        + (a[2].clamp(0, 31) << 10)) as u32;
                }
                (OutFmt::Rgb, BitDepth::Bd10) => {
                    o[0] = a[0].clamp(0, 1023) as u32
                        | (a[1].clamp(0, 1023) as u32) << 10
                        | (a[2].clamp(0, 1023) as u32) << 20;
                }
                (_, bd) => {
                    for (&v, o) in a.iter().zip(o.iter_mut()) {
                        *o = match bd {
                            BitDepth::Bd8 => v.clamp(0, 255) as u32,
                            BitDepth::Bd16 => (v << shift).clamp(0, 65535) as u32,
                            BitDepth::Bd16S => (v << shift).clamp(-32768, 32767) as u16 as u32,
                            BitDepth::Bd32S => (v << shift) as u32,
                            BitDepth::Bd10 => v.clamp(0, 1023) as u32,
                            BitDepth::Bd16F => {
                                let s = if v < 0 { 0x8000 } else { 0 };
                                s | v.unsigned_abs().min(32767)
                            }
                            BitDepth::Bd32F => {
                                to_float_bits(v, pp.len_mantissa as u32, pp.exp_bias as i32)
                            }
                            BitDepth::Bd1White1 => v.clamp(0, 1) as u32,
                            BitDepth::Bd1Black1 => 1 - v.clamp(0, 1) as u32,
                            _ => 0,
                        };
                    }
                }
            }
        }
    }
}

/// PostScalingFl for BD32F (Table 192).
fn to_float_bits(x: i32, len: u32, bias: i32) -> u32 {
    let s = (x < 0) as u32;
    let x = x.unsigned_abs() as i64;
    let mut e = x >> len;
    let mut m = (x & ((1 << len) - 1)) | (1 << len);
    if e == 0 {
        m ^= 1 << len;
        e = 1;
    }
    e = e - bias as i64 + 127;
    while m < (1 << len) && e > 1 && m > 0 {
        e -= 1;
        m <<= 1;
    }
    if m < (1 << len) {
        e = 0;
    } else {
        m ^= 1 << len;
    }
    m <<= 23 - len;
    (s << 31) | ((e as u32 & 0xFF) << 23) | (m as u32 & 0x7F_FFFF)
}

/// Formats a reconstructed plane into `nch`-channel native samples written at channel offset
/// `ch0` of `stride`-channel interleaved rows. `out` is YONLY for alpha planes.
#[allow(clippy::too_many_arguments)]
fn fill<T: Copy + Send>(
    f: &Fmt,
    dst: &mut [T],
    stride: usize,
    ch0: usize,
    nch: usize,
    conv: fn(u32) -> T,
    parallel: bool,
) {
    let w = f.ih.width as usize;
    let top = f.ih.top as usize;
    let ext_w = f.ext_w;
    let job = |(r, row): (usize, &mut [T])| {
        let mut scratch = [vec![0i32; ext_w], vec![0i32; ext_w], vec![0i32; ext_w]];
        let mut vals = vec![0u32; w * nch];
        f.row(top + r, &mut vals, &mut scratch);
        for (px, v) in row.chunks_exact_mut(stride).zip(vals.chunks_exact(nch)) {
            for (d, &s) in px[ch0..ch0 + nch].iter_mut().zip(v) {
                *d = conv(s);
            }
        }
    };
    if parallel {
        dst.par_chunks_mut(w * stride).enumerate().for_each(job);
    } else {
        dst.chunks_mut(w * stride).enumerate().for_each(job);
    }
}

fn make<'a>(
    h: &'a ImageHeader,
    p: &'a Plane,
    out: OutFmt,
    primary: Option<&'a PlaneHeader>,
) -> Fmt<'a> {
    Fmt {
        ih: h,
        ph: &p.ph,
        out,
        bd: h.bit_depth,
        comp: p.comp.iter().map(|c| c.as_slice()).collect(),
        cw: p.cw.clone(),
        ch: if p.ph.clr == ClrFmt::Yuv420 {
            p.mbh * 8
        } else {
            p.mbh * 16
        },
        ext_w: h.ext_width() as usize,
        primary,
    }
}

/// Formats `planes` (primary, then an optional alpha plane formatted as YONLY) into one
/// interleaved native buffer with the alpha channel last. The alpha flag is true for an
/// interleaved alpha plane (second plane of the same codestream).
pub(crate) fn format(
    ih: &ImageHeader,
    primary: &Plane,
    alpha: Option<(&ImageHeader, &Plane, bool)>,
    parallel: bool,
) -> Result<(usize, Samples), crate::Error> {
    let nch = channels(ih.out_fmt, ih.bit_depth, primary.ph.nc);
    let ach = alpha.map_or(0, |(h, p, _)| channels(OutFmt::YOnly, h.bit_depth, p.ph.nc));
    let stride = nch + ach;
    let n = (ih.width as usize)
        .checked_mul(ih.height as usize)
        .and_then(|v| v.checked_mul(stride))
        .ok_or(crate::Error::TooLarge)?;
    macro_rules! run {
        ($t:ty, $conv:expr, $var:ident) => {{
            // Reserved while the planes it is made from are still held.
            let _res = specular_core::budget::reserve_n(n as u64, size_of::<$t>() as u64)?;
            let mut v: Vec<$t> = Vec::new();
            v.try_reserve_exact(n).map_err(|_| crate::Error::TooLarge)?;
            v.resize(n, 0 as $t);
            fill(
                &make(ih, primary, ih.out_fmt, None),
                &mut v,
                stride,
                0,
                nch,
                $conv,
                parallel,
            );
            if let Some((ah, ap, inter)) = alpha {
                let pp = inter.then_some(&primary.ph);
                fill(
                    &make(ah, ap, OutFmt::YOnly, pp),
                    &mut v,
                    stride,
                    nch,
                    ach,
                    $conv,
                    parallel,
                );
            }
            Samples::$var(v)
        }};
    }
    let s = match sample_width(ih.out_fmt, ih.bit_depth) {
        Width::W8 => run!(u8, |x| x as u8, U8),
        Width::W16 => run!(u16, |x| x as u16, U16),
        Width::W32 => run!(u32, |x| x, U32),
    };
    Ok((stride, s))
}
