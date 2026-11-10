//! Fast YCbCr → RGBA for display: fixed-point, row-parallel, with box
//! downscaling folded into the conversion and an output window, so a
//! 1/2, 1/4 or 1/8 level or a region costs only its own output (plus
//! reading its source rows), never a full-size RGBA intermediate.
//!
//! Same equations as the reference kernel ([`heif::rgb::for_each_pixel`],
//! H.273 §8.3): the matrix and range of [`Colour::ycbcr`], identity
//! (GBR) and monochrome; at 1:1 sub-sampled chroma is replicated to its
//! co-sited luma samples like the reference. At 1/s the s×s luma block
//! and the chroma samples under it are averaged before the (affine)
//! conversion, which equals averaging the converted pixels except where
//! they clip. With alpha the RGBA of each source pixel is computed first
//! and the block averaged with alpha weighting (transparent pixels do not
//! darken edges); pre-multiplied alpha is un-premultiplied.

use std::sync::atomic::{AtomicBool, Ordering};

use heif::{Chroma, HeifFrame};

use crate::colour::Ycbcr;
use crate::{Error, Image, Result};

/// What [`Image::to_rgba`] produces.
#[derive(Clone, Copy, Debug)]
pub struct ConvertOptions<'a> {
    /// Box-downscale factor (1 = full size). The output of the whole
    /// image is `width.div_ceil(scale)` × `height.div_ceil(scale)`.
    pub scale: u32,
    /// Output window `(x, y, w, h)` in output (scaled) pixels, clamped
    /// to the output; `None`: everything.
    pub window: Option<(u32, u32, u32, u32)>,
    /// 16-bit samples (native-endian `u16` RGBA, scaled to 0..=65535)
    /// instead of 8-bit.
    pub wide: bool,
    /// Rows in parallel on the current rayon pool.
    pub parallel: bool,
    /// Cooperative cancellation, polled per band of rows.
    pub cancel: Option<&'a AtomicBool>,
}

impl Default for ConvertOptions<'_> {
    fn default() -> Self {
        Self {
            scale: 1,
            window: None,
            wide: false,
            parallel: false,
            cancel: None,
        }
    }
}

/// Interleaved RGBA pixels (straight alpha; opaque without alpha).
#[derive(Clone, Debug)]
pub struct Rgba {
    /// Output width.
    pub width: u32,
    /// Output height.
    pub height: u32,
    /// Top-left of the window in output pixels.
    pub origin: (u32, u32),
    /// 16-bit samples ([`ConvertOptions::wide`]).
    pub wide: bool,
    /// `width × height × 4` bytes, or × 8 (native-endian `u16`) when
    /// `wide`.
    pub data: Vec<u8>,
}

impl Image {
    /// Convert to RGBA for display (see the [module docs](self)).
    pub fn to_rgba(&self, o: &ConvertOptions<'_>) -> Result<Rgba> {
        convert(
            &self.frame,
            self.colour.ycbcr(),
            self.premultiplied_alpha,
            o,
        )
    }
}

/// Fixed-point fraction bits of the conversion coefficients.
const FRAC: u32 = 16;

/// The conversion of one (averaged) YCbCr sample to RGB at `out_max`,
/// in fixed point.
#[derive(Clone, Copy, Debug)]
struct Coeffs {
    /// Luma offset (black) in source units.
    y_off: i64,
    /// Chroma midpoint in source units.
    mid: i64,
    /// Luma (and, for identity, every channel) gain.
    ky: i64,
    cr_r: i64,
    cb_g: i64,
    cr_g: i64,
    cb_b: i64,
    kind: Kind,
    out_max: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Mono,
    Identity,
    Matrix,
}

impl Coeffs {
    fn new(depth: u8, chroma: Chroma, y: Ycbcr, out_max: u32) -> Result<Self> {
        let max = f64::from((1u32 << depth) - 1);
        let sh = f64::from(1u32 << (depth - 8));
        let (y_off, y_scale, c_scale) = if y.full_range {
            (0.0, max, max)
        } else {
            (16.0 * sh, 219.0 * sh, 224.0 * sh)
        };
        let one = f64::from(1u32 << FRAC) * f64::from(out_max);
        let q = |v: f64| v.round() as i64;
        let kind = if chroma == Chroma::Mono {
            Kind::Mono
        } else if y.matrix == 0 {
            Kind::Identity
        } else {
            Kind::Matrix
        };
        let (kr, kb) = if kind == Kind::Matrix {
            heif::rgb::matrix_kr_kb(y.matrix).ok_or_else(|| {
                Error::Unsupported(format!("matrix_coefficients {} conversion", y.matrix))
            })?
        } else {
            (0.0, 0.0)
        };
        let kg = 1.0 - kr - kb;
        let c = one / c_scale;
        Ok(Self {
            y_off: q(y_off),
            mid: 1 << (depth - 1),
            ky: q(one / y_scale),
            cr_r: q(2.0 * (1.0 - kr) * c),
            cb_g: q(2.0 * kb * (1.0 - kb) / kg * c),
            cr_g: q(2.0 * kr * (1.0 - kr) / kg * c),
            cb_b: q(2.0 * (1.0 - kb) * c),
            kind,
            out_max: i64::from(out_max),
        })
    }

    /// RGB of one sample; `y`, `cb`, `cr` are `n`-sample sums (averages
    /// times `n`), so a block average needs no division before here.
    #[inline(always)]
    fn rgb(&self, y: i64, cb: i64, cr: i64, n: i64, nc: i64) -> [i64; 3] {
        let half = (1i64 << FRAC) / 2;
        let fin = |v: i64, d: i64| ((v / d + half) >> FRAC).clamp(0, self.out_max);
        match self.kind {
            Kind::Mono => {
                let g = fin((y - self.y_off * n) * self.ky, n);
                [g, g, g]
            }
            Kind::Identity => [
                fin((cr - self.y_off * nc) * self.ky, nc),
                fin((y - self.y_off * n) * self.ky, n),
                fin((cb - self.y_off * nc) * self.ky, nc),
            ],
            Kind::Matrix => {
                // Luma and chroma sums may cover different sample counts:
                // bring both to a common denominator n·nc.
                let ey = (y - self.y_off * n) * self.ky * nc;
                let pb = (cb - self.mid * nc) * n;
                let pr = (cr - self.mid * nc) * n;
                let d = n * nc;
                [
                    fin(ey + pr * self.cr_r, d),
                    fin(ey - pb * self.cb_g - pr * self.cr_g, d),
                    fin(ey + pb * self.cb_b, d),
                ]
            }
        }
    }
}

/// Read sample `x` of a plane row (8-bit bytes or little-endian `u16`).
#[inline(always)]
fn at(row: &[u8], x: usize, wide: bool) -> i64 {
    if wide {
        i64::from(u16::from_le_bytes([row[2 * x], row[2 * x + 1]]))
    } else {
        i64::from(row[x])
    }
}

/// Convert `frame` with `ycbcr`; `premultiplied`: the alpha plane is
/// pre-multiplied into the colour.
pub fn convert(
    frame: &HeifFrame,
    ycbcr: Ycbcr,
    premultiplied: bool,
    o: &ConvertOptions<'_>,
) -> Result<Rgba> {
    frame.validate()?;
    let s = o.scale.max(1);
    let (fw, fh) = (frame.width, frame.height);
    let (ow, oh) = (fw.div_ceil(s), fh.div_ceil(s));
    let (wx, wy, ww, wh) = match o.window {
        Some((x, y, w, h)) => {
            let x = x.min(ow);
            let y = y.min(oh);
            (x, y, w.min(ow - x), h.min(oh - y))
        }
        None => (0, 0, ow, oh),
    };
    let out_max: u32 = if o.wide { 65535 } else { 255 };
    let fmt = frame.format;
    let c = Coeffs::new(fmt.bit_depth, fmt.chroma, ycbcr, out_max)?;
    let src_wide = fmt.bytes_per_sample() == 2;
    let bpp = if o.wide { 8 } else { 4 };
    let stride = ww as usize * bpp;
    let mut data = vec![0u8; stride * wh as usize];
    if ww == 0 || wh == 0 {
        return Ok(Rgba {
            width: ww,
            height: wh,
            origin: (wx, wy),
            wide: o.wide,
            data,
        });
    }
    let alpha = fmt.alpha_plane();
    let k = Kernel {
        frame,
        c,
        s,
        src_wide,
        out_wide: o.wide,
        window: (wx, ww),
        alpha,
        premultiplied: premultiplied && alpha.is_some(),
        a_max: i64::from(fmt.max_value()),
    };
    let cancelled = || o.cancel.is_some_and(|c| c.load(Ordering::Relaxed));
    // Bands of output rows: big enough to amortise the scratch, small
    // enough to balance and to poll cancellation often.
    let band_rows = (64 / s as usize).max(1);
    let band = |(i, out): (usize, &mut [u8])| -> Result<()> {
        if cancelled() {
            return Err(Error::Cancelled);
        }
        let mut scratch = Scratch::default();
        for (j, row) in out.chunks_exact_mut(stride).enumerate() {
            k.row(wy + (i * band_rows + j) as u32, row, &mut scratch);
        }
        Ok(())
    };
    if o.parallel && wh as usize > band_rows {
        use rayon::prelude::*;
        data.par_chunks_mut(stride * band_rows)
            .enumerate()
            .try_for_each(band)?;
    } else {
        data.chunks_mut(stride * band_rows)
            .enumerate()
            .try_for_each(band)?;
    }
    Ok(Rgba {
        width: ww,
        height: wh,
        origin: (wx, wy),
        wide: o.wide,
        data,
    })
}

/// Per-thread buffers of the kernel.
#[derive(Default)]
struct Scratch {
    /// Column sums of each plane over the block's rows.
    cols: [Vec<i64>; 4],
    /// One source row of RGBA (alpha images at 1/s).
    rgba: Vec<[i64; 4]>,
    /// Alpha-weighted RGBA column sums (alpha images at 1/s).
    acc: Vec<[i64; 4]>,
}

struct Kernel<'f> {
    frame: &'f HeifFrame,
    c: Coeffs,
    s: u32,
    src_wide: bool,
    out_wide: bool,
    /// First output column and output width.
    window: (u32, u32),
    alpha: Option<usize>,
    premultiplied: bool,
    a_max: i64,
}

impl Kernel<'_> {
    fn plane_row(&self, p: usize, y: u32) -> &[u8] {
        let pl = &self.frame.planes[p];
        &pl.data[y as usize * pl.stride..]
    }

    fn put(&self, out: &mut [u8], x: usize, px: [i64; 4]) {
        if self.out_wide {
            for (c, v) in px.iter().enumerate() {
                out[x * 8 + 2 * c..x * 8 + 2 * c + 2].copy_from_slice(&(*v as u16).to_ne_bytes());
            }
        } else {
            for (c, v) in px.iter().enumerate() {
                out[x * 4 + c] = *v as u8;
            }
        }
    }

    /// Alpha scaled to the output range, and the colour un-premultiplied.
    fn finish_alpha(&self, rgb: [i64; 3], a: i64) -> [i64; 4] {
        let m = self.c.out_max;
        let ao = (a * m + self.a_max / 2) / self.a_max;
        if self.premultiplied {
            if ao == 0 {
                return [0, 0, 0, 0];
            }
            let u = |v: i64| ((v * m + ao / 2) / ao).min(m);
            [u(rgb[0]), u(rgb[1]), u(rgb[2]), ao]
        } else {
            [rgb[0], rgb[1], rgb[2], ao]
        }
    }

    /// One full-resolution RGBA row `y`, columns `x0..x1`, into `out`.
    fn source_rgba(&self, y: u32, x0: u32, x1: u32, out: &mut Vec<[i64; 4]>) {
        out.clear();
        let fmt = self.frame.format;
        let (sx, sy) = fmt.chroma.shift();
        let w = self.src_wide;
        let ly = self.plane_row(0, y);
        let (cb, cr) = if fmt.chroma == Chroma::Mono {
            (ly, ly)
        } else {
            (self.plane_row(1, y >> sy), self.plane_row(2, y >> sy))
        };
        let ar = self.alpha.map(|a| self.plane_row(a, y));
        for x in x0..x1 {
            let xi = x as usize;
            let xc = (x >> sx) as usize;
            let rgb = self
                .c
                .rgb(at(ly, xi, w), at(cb, xc, w), at(cr, xc, w), 1, 1);
            let a = ar.map_or(self.a_max, |r| at(r, xi, w));
            out.push(self.finish_alpha(rgb, a));
        }
    }

    /// One opaque 1:1 output row: the same fixed-point equations as [`Coeffs::rgb`] with
    /// one sample per sum (no division), monomorphised per source and output width, the
    /// kind matched once per row and the pixels written straight into `out`.
    fn row_full<const SW: bool, const OW: bool>(&self, y: u32, out: &mut [u8]) {
        let (wx, ww) = self.window;
        let (x0, n) = (wx as usize, ww as usize);
        let fmt = self.frame.format;
        let (sx, sy) = fmt.chroma.shift();
        let c = self.c;
        let m = c.out_max;
        let half = (1i64 << FRAC) / 2;
        let fin = |v: i64| ((v + half) >> FRAC).clamp(0, m);
        let rd = |row: &[u8], x: usize| at(row, x, SW);
        let put = |out: &mut [u8], i: usize, r: i64, g: i64, b: i64| {
            if OW {
                let o = &mut out[i * 8..i * 8 + 8];
                o[0..2].copy_from_slice(&(r as u16).to_ne_bytes());
                o[2..4].copy_from_slice(&(g as u16).to_ne_bytes());
                o[4..6].copy_from_slice(&(b as u16).to_ne_bytes());
                o[6..8].copy_from_slice(&(m as u16).to_ne_bytes());
            } else {
                let o = &mut out[i * 4..i * 4 + 4];
                o.copy_from_slice(&[r as u8, g as u8, b as u8, m as u8]);
            }
        };
        let ly = self.plane_row(0, y);
        if c.kind == Kind::Mono {
            for i in 0..n {
                let g = fin((rd(ly, x0 + i) - c.y_off) * c.ky);
                put(out, i, g, g, g);
            }
            return;
        }
        let cb = self.plane_row(1, y >> sy);
        let cr = self.plane_row(2, y >> sy);
        if c.kind == Kind::Identity {
            for i in 0..n {
                let (x, xc) = (x0 + i, (x0 + i) >> sx);
                put(
                    out,
                    i,
                    fin((rd(cr, xc) - c.y_off) * c.ky),
                    fin((rd(ly, x) - c.y_off) * c.ky),
                    fin((rd(cb, xc) - c.y_off) * c.ky),
                );
            }
            return;
        }
        if !OW {
            // 8-bit output: every product and sum stays below 2^31 (gains are at most
            // 2^16 · 255 / 219 per source step of 8-bit, smaller in proportion for deeper
            // sources), so i32 lanes, which the compiler vectorises.
            let k = |v: i64| v as i32;
            let (yo, ky, mid) = (k(c.y_off), k(c.ky), k(c.mid));
            let (cr_r, cb_g, cr_g, cb_b) = (k(c.cr_r), k(c.cb_g), k(c.cr_g), k(c.cb_b));
            let h = k(half);
            let f = |v: i32| ((v + h) >> FRAC).clamp(0, 255) as u8;
            let rd32 = |row: &[u8], x: usize| at(row, x, SW) as i32;
            for (i, o) in out[..n * 4].chunks_exact_mut(4).enumerate() {
                let (x, xc) = (x0 + i, (x0 + i) >> sx);
                let ey = (rd32(ly, x) - yo) * ky;
                let pb = rd32(cb, xc) - mid;
                let pr = rd32(cr, xc) - mid;
                o.copy_from_slice(&[
                    f(ey + pr * cr_r),
                    f(ey - pb * cb_g - pr * cr_g),
                    f(ey + pb * cb_b),
                    255,
                ]);
            }
            return;
        }
        for i in 0..n {
            let (x, xc) = (x0 + i, (x0 + i) >> sx);
            let ey = (rd(ly, x) - c.y_off) * c.ky;
            let pb = rd(cb, xc) - c.mid;
            let pr = rd(cr, xc) - c.mid;
            put(
                out,
                i,
                fin(ey + pr * c.cr_r),
                fin(ey - pb * c.cb_g - pr * c.cr_g),
                fin(ey + pb * c.cb_b),
            );
        }
    }

    fn row(&self, oy: u32, out: &mut [u8], sc: &mut Scratch) {
        let (wx, ww) = self.window;
        let (fw, fh) = (self.frame.width, self.frame.height);
        let s = self.s;
        if s == 1 && self.alpha.is_none() {
            match (self.src_wide, self.out_wide) {
                (false, false) => self.row_full::<false, false>(oy, out),
                (false, true) => self.row_full::<false, true>(oy, out),
                (true, false) => self.row_full::<true, false>(oy, out),
                (true, true) => self.row_full::<true, true>(oy, out),
            }
            return;
        }
        if s == 1 {
            self.source_rgba(oy, wx, wx + ww, &mut sc.rgba);
            for (x, px) in sc.rgba.iter().enumerate() {
                self.put(out, x, *px);
            }
            return;
        }
        let (y0, y1) = (oy * s, ((oy + 1) * s).min(fh));
        let (x0, x1) = (wx * s, ((wx + ww) * s).min(fw));
        if self.alpha.is_some() {
            // Alpha-weighted: RGBA per source pixel, then the block.
            let n = (x1 - x0) as usize;
            sc.acc.clear();
            sc.acc.resize(n, [0; 4]);
            for y in y0..y1 {
                self.source_rgba(y, x0, x1, &mut sc.rgba);
                for (a, p) in sc.acc.iter_mut().zip(&sc.rgba) {
                    for c in 0..3 {
                        a[c] += p[c] * p[3];
                    }
                    a[3] += p[3];
                }
            }
            let rows = i64::from(y1 - y0);
            for ox in 0..ww {
                let bx0 = (ox * s) as usize;
                let bx1 = (((ox + 1) * s) as usize).min(n);
                let mut t = [0i64; 4];
                for a in &sc.acc[bx0..bx1] {
                    for c in 0..4 {
                        t[c] += a[c];
                    }
                }
                let cnt = rows * (bx1 - bx0) as i64;
                let px = if t[3] == 0 {
                    [0, 0, 0, 0]
                } else {
                    [
                        (t[0] + t[3] / 2) / t[3],
                        (t[1] + t[3] / 2) / t[3],
                        (t[2] + t[3] / 2) / t[3],
                        (t[3] + cnt / 2) / cnt,
                    ]
                };
                self.put(out, ox as usize, px);
            }
            return;
        }
        // Column sums of luma over the block's rows, and of each chroma
        // plane over the chroma rows under them.
        let fmt = self.frame.format;
        let w = self.src_wide;
        let mono = fmt.chroma == Chroma::Mono;
        let (sx, sy) = fmt.chroma.shift();
        let (cx0, cx1) = (x0 >> sx, ((x1 - 1) >> sx) + 1);
        let (cy0, cy1) = (y0 >> sy, ((y1 - 1) >> sy) + 1);
        let [ly, cb, cr, _] = &mut sc.cols;
        ly.clear();
        ly.resize((x1 - x0) as usize, 0);
        for y in y0..y1 {
            let r = self.plane_row(0, y);
            for (i, a) in ly.iter_mut().enumerate() {
                *a += at(r, x0 as usize + i, w);
            }
        }
        if !mono {
            for (p, cols) in [(1usize, &mut *cb), (2, &mut *cr)] {
                cols.clear();
                cols.resize((cx1 - cx0) as usize, 0);
                for y in cy0..cy1 {
                    let r = self.plane_row(p, y);
                    for (i, a) in cols.iter_mut().enumerate() {
                        *a += at(r, cx0 as usize + i, w);
                    }
                }
            }
        }
        let rows = i64::from(y1 - y0);
        let crows = i64::from(cy1 - cy0);
        for ox in 0..ww {
            let bx0 = ox * s;
            let bx1 = ((ox + 1) * s).min(x1 - x0);
            let yl: i64 = ly[bx0 as usize..bx1 as usize].iter().sum();
            let n = rows * i64::from(bx1 - bx0);
            let rgb = if mono {
                self.c.rgb(yl, 0, 0, n, 1)
            } else {
                let c0 = ((x0 + bx0) >> sx) - cx0;
                let c1 = ((x0 + bx1 - 1) >> sx) - cx0 + 1;
                let span = c0 as usize..c1 as usize;
                let nc = crows * i64::from(c1 - c0);
                let b: i64 = cb[span.clone()].iter().sum();
                let r: i64 = cr[span].iter().sum();
                self.c.rgb(yl, b, r, n, nc)
            };
            self.put(out, ox as usize, [rgb[0], rgb[1], rgb[2], self.c.out_max]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use heif::{HeifPixelFormat, Plane};

    /// A pseudo-random frame of `chroma` / `depth`, optionally with alpha.
    fn frame(w: u32, h: u32, chroma: Chroma, depth: u8, alpha: bool, tame: bool) -> HeifFrame {
        let fmt = HeifPixelFormat::new(chroma, depth, alpha).unwrap();
        let mut seed = 0x1234_5678u32;
        let max = u32::from(fmt.max_value());
        let planes = (0..fmt.plane_count())
            .map(|p| {
                let (pw, ph) = fmt.plane_dims(p, w, h);
                let bps = fmt.bytes_per_sample();
                let mut data = Vec::new();
                for y in 0..ph {
                    for x in 0..pw {
                        seed ^= seed << 13;
                        seed ^= seed >> 17;
                        seed ^= seed << 5;
                        // Smooth plus noise, so averages are meaningful.
                        let v = ((x * 7 + y * 3) * max / (pw * 7 + ph * 3)).min(max);
                        let v = (v + seed % (max / 8 + 1)).min(max);
                        // In gamut: luma inside the range, chroma near neutral, so
                        // nothing clips (where averaging before and after the
                        // conversion would differ).
                        let v = if tame && p < 3 && Some(p) != fmt.alpha_plane() {
                            if p == 0 || chroma == Chroma::Mono {
                                max / 5 + v * 3 / 5
                            } else {
                                max * 7 / 16 + v / 8
                            }
                        } else {
                            v
                        };
                        let v = if Some(p) == fmt.alpha_plane() && x % 5 == 0 {
                            0
                        } else {
                            v
                        };
                        if bps == 1 {
                            data.push(v as u8);
                        } else {
                            data.extend_from_slice(&(v as u16).to_le_bytes());
                        }
                    }
                }
                Plane {
                    stride: pw as usize * bps,
                    data,
                }
            })
            .collect();
        HeifFrame {
            width: w,
            height: h,
            format: fmt,
            planes,
        }
    }

    fn reference(f: &HeifFrame, y: Ycbcr) -> Vec<u8> {
        let colr = heif::props::Colr::Nclx {
            primaries: 1,
            transfer: 13,
            matrix: y.matrix,
            full_range: y.full_range,
        };
        heif::rgb::planes_to_rgb8(f.width, f.height, f.format, &f.planes, Some(&colr), true)
            .unwrap()
    }

    #[test]
    fn full_size_matches_the_reference_kernel() {
        for chroma in [Chroma::Mono, Chroma::Yuv420, Chroma::Yuv422, Chroma::Yuv444] {
            for depth in [8, 10, 12] {
                for (matrix, full_range) in [(6, false), (1, true), (9, false), (0, true)] {
                    let f = frame(37, 21, chroma, depth, false, false);
                    let y = Ycbcr { matrix, full_range };
                    let want = reference(&f, y);
                    let got = convert(&f, y, false, &ConvertOptions::default()).unwrap();
                    let worst = want
                        .iter()
                        .zip(&got.data)
                        .map(|(a, b)| a.abs_diff(*b))
                        .max()
                        .unwrap();
                    assert!(worst <= 1, "{chroma:?} {depth} {matrix}: {worst}");
                    // 16-bit output agrees with 8-bit.
                    let wide = convert(
                        &f,
                        y,
                        false,
                        &ConvertOptions {
                            wide: true,
                            ..Default::default()
                        },
                    )
                    .unwrap();
                    for (a, b) in got.data.iter().zip(wide.data.chunks_exact(2)) {
                        let v = u16::from_ne_bytes([b[0], b[1]]);
                        assert!((i32::from(*a) - (i32::from(v) + 128) / 257).abs() <= 1);
                    }
                }
            }
        }
    }

    /// Box average of the reference RGBA (alpha-weighted).
    fn reference_scaled(f: &HeifFrame, y: Ycbcr, s: u32) -> Vec<u8> {
        let full = reference(f, y);
        let (w, h) = (f.width, f.height);
        let (ow, oh) = (w.div_ceil(s), h.div_ceil(s));
        let mut out = Vec::new();
        for oy in 0..oh {
            for ox in 0..ow {
                let mut t = [0u64; 4];
                let mut n = 0;
                for y in oy * s..((oy + 1) * s).min(h) {
                    for x in ox * s..((ox + 1) * s).min(w) {
                        let p = &full[((y * w + x) * 4) as usize..][..4];
                        let a = u64::from(p[3]);
                        for c in 0..3 {
                            t[c] += u64::from(p[c]) * a;
                        }
                        t[3] += a;
                        n += 1;
                    }
                }
                for c in 0..3 {
                    out.push((t[c] + t[3] / 2).checked_div(t[3]).unwrap_or(0) as u8);
                }
                out.push(((t[3] + n / 2) / n) as u8);
            }
        }
        out
    }

    #[test]
    fn scaled_levels_and_windows_match_the_box_filtered_reference() {
        for (chroma, alpha) in [
            (Chroma::Yuv420, false),
            (Chroma::Yuv444, true),
            (Chroma::Mono, false),
            (Chroma::Yuv422, false),
        ] {
            let f = frame(75, 43, chroma, 8, alpha, true);
            let y = Ycbcr {
                matrix: 6,
                full_range: false,
            };
            for s in [1u32, 2, 4, 8] {
                // At 1:1 a transparent pixel keeps its colour (straight alpha); the reference
                // weights it to black, so only opaque frames compare there.
                if s == 1 && alpha {
                    continue;
                }
                let want = reference_scaled(&f, y, s);
                let o = ConvertOptions {
                    scale: s,
                    parallel: true,
                    ..Default::default()
                };
                let got = convert(&f, y, false, &o).unwrap();
                assert_eq!(
                    (got.width, got.height),
                    (75u32.div_ceil(s), 43u32.div_ceil(s))
                );
                let worst = want
                    .iter()
                    .zip(&got.data)
                    .map(|(a, b)| a.abs_diff(*b))
                    .max()
                    .unwrap();
                // Averaging before the conversion (and chroma averaged over
                // its own samples) differs from averaging after it a little.
                assert!(worst <= 2, "{chroma:?} alpha {alpha} 1/{s}: {worst}");
                // A window is the same pixels as the whole level.
                let (wx, wy, ww, wh) = (1, 2, 5, 3);
                let win = convert(
                    &f,
                    y,
                    false,
                    &ConvertOptions {
                        window: Some((wx, wy, ww, wh)),
                        ..o
                    },
                )
                .unwrap();
                let ww = ww.min(got.width - wx);
                let wh = wh.min(got.height - wy);
                assert_eq!((win.width, win.height, win.origin), (ww, wh, (wx, wy)));
                for r in 0..wh {
                    let a = ((wy + r) * got.width + wx) as usize * 4;
                    let b = (r * ww) as usize * 4;
                    assert_eq!(
                        &got.data[a..a + ww as usize * 4],
                        &win.data[b..b + ww as usize * 4]
                    );
                }
            }
        }
    }

    /// Timing of the 1:1 display conversion of a 24 MP 4:2:0 frame (one thread), for paired
    /// before/after runs: `cargo test --release -p specular-heic rgba_full_size_timing -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn rgba_full_size_timing() {
        let f = frame(6000, 4000, Chroma::Yuv420, 8, false, false);
        let y = Ycbcr {
            matrix: 1,
            full_range: false,
        };
        for wide in [false, true] {
            let o = ConvertOptions {
                wide,
                ..Default::default()
            };
            let mut best = f64::MAX;
            for _ in 0..5 {
                let t = std::time::Instant::now();
                let r = convert(&f, y, false, &o).unwrap();
                best = best.min(t.elapsed().as_secs_f64() * 1e3);
                std::hint::black_box(r);
            }
            eprintln!(
                "24 MP 4:2:0 to {} bit: best of 5 {best:.1} ms",
                if wide { 16 } else { 8 }
            );
        }
    }

    #[test]
    fn premultiplied_alpha_is_undone_and_cancel_stops() {
        let mut f = frame(16, 8, Chroma::Mono, 8, true, false);
        // Grey 100 at alpha 128 → straight 199.
        f.planes[0].data.fill(100);
        f.planes[1].data.fill(128);
        let y = Ycbcr {
            matrix: 6,
            full_range: true,
        };
        let r = convert(&f, y, true, &ConvertOptions::default()).unwrap();
        assert_eq!(&r.data[..4], &[199, 199, 199, 128]);
        let stop = AtomicBool::new(true);
        let o = ConvertOptions {
            cancel: Some(&stop),
            ..Default::default()
        };
        assert!(matches!(convert(&f, y, false, &o), Err(Error::Cancelled)));
    }
}
