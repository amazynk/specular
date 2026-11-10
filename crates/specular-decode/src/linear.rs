//! Linear-light float images (OpenEXR, Radiance, PFM): conversion to `Rgba16F` rasters
//! (half-float, linear, 1.0 = SDR reference white, straight alpha) and box downscaling of
//! 16-bit rasters. The GPU converts the primaries and tone-maps (see `specular_core::hdr`).

use std::ops::Range;
use std::sync::atomic::{AtomicBool, Ordering};

use parking_lot::Mutex;
use specular_core::{CancelToken, PixelLayout, Primaries, Raster, RasterPlane, hdr};

/// Largest finite half.
const F16_MAX: f32 = 65504.0;

/// Rows per band for the threaded conversion.
const BAND_ROWS: usize = 64;

fn one_plane(
    width: u32,
    height: u32,
    layout: PixelLayout,
    data: Vec<u8>,
    truncated: bool,
) -> Raster {
    Raster {
        width,
        height,
        origin: (0, 0),
        scale: 1,
        layout,
        planes: vec![RasterPlane {
            width,
            height,
            stride: width as usize * 8,
            data,
            h_div: 1,
            v_div: 1,
        }],
        truncated,
    }
}

/// Runs `f(first_row, rows_out)` over bands of `out` (row length `row_bytes`), on several
/// threads when `parallel` and the image is large enough to pay for them.
fn bands(out: &mut [u8], row_bytes: usize, parallel: bool, f: impl Fn(usize, &mut [u8]) + Sync) {
    let rows = out.len() / row_bytes.max(1);
    let threads = if parallel && rows * row_bytes > (1 << 20) {
        std::thread::available_parallelism()
            .map_or(1, |n| n.get())
            .min(16)
    } else {
        1
    };
    if threads <= 1 {
        f(0, out);
        return;
    }
    let band = BAND_ROWS.max(rows.div_ceil(threads * 4)) * row_bytes;
    let chunks: Vec<(usize, &mut [u8])> = out.chunks_mut(band).enumerate().collect();
    let chunks = std::sync::Mutex::new(chunks.into_iter());
    std::thread::scope(|s| {
        for wi in 0..threads {
            specular_thread::spawn_scoped(s, format!("dec-linear-{wi}"), || {
                while let Some((i, c)) = chunks.lock().ok().and_then(|mut it| it.next()) {
                    f(i * band / row_bytes, c);
                }
            });
        }
    });
}

fn finite(v: f32) -> f32 {
    if v.is_nan() {
        0.0
    } else {
        v.clamp(-F16_MAX, F16_MAX)
    }
}

/// Interleaved RGBA `f32` (`channels` = 3 or 4 per pixel) → `Rgba16F`. `matrix` (row-major,
/// to linear BT.709) is applied when given, and the raster is then tagged BT.709; otherwise it
/// is tagged `primaries`. NaN becomes 0, infinities the largest half; alpha is clamped to 0..1.
#[allow(clippy::too_many_arguments)]
pub(crate) fn f32_to_raster(
    width: u32,
    height: u32,
    channels: usize,
    data: &[f32],
    matrix: Option<[[f32; 3]; 3]>,
    primaries: Primaries,
    truncated: bool,
    parallel: bool,
) -> Raster {
    let row = width as usize * 8;
    let mut out = vec![0u8; row * height as usize];
    bands(&mut out, row, parallel, |y0, band| {
        let src = &data[y0 * width as usize * channels..];
        for (o, s) in band.chunks_exact_mut(8).zip(src.chunks_exact(channels)) {
            let mut c = [s[0], s[1], s[2]];
            if let Some(m) = matrix {
                c = hdr::mat_mul(m, c);
            }
            let a = if channels == 4 { s[3] } else { 1.0 };
            let a = if a.is_nan() { 1.0 } else { a.clamp(0.0, 1.0) };
            for (i, v) in [finite(c[0]), finite(c[1]), finite(c[2]), a]
                .into_iter()
                .enumerate()
            {
                o[2 * i..2 * i + 2].copy_from_slice(&hdr::f32_to_f16(v).to_ne_bytes());
            }
        }
    });
    let p = if matrix.is_some() {
        Primaries::Bt709
    } else {
        primaries
    };
    one_plane(width, height, PixelLayout::Rgba16F(p), out, truncated)
}

/// Premultiplied → straight colour in place on an `Rgba16F` raster (OpenEXR stores colour
/// associated with alpha). Floats keep their relative precision through the division, so
/// nothing is lost but the colour under alpha 0, which stays as stored (an emissive pixel; the
/// straight-alpha pipeline cannot show it). JPEG XR's premultiplied float formats use it on
/// whole images; the streaming HDR decoders do the same through [`Prep`] in [`reduce`], and it
/// is the reference for that in tests.
pub(crate) fn unpremultiply(r: &mut Raster, parallel: bool) {
    if !matches!(r.layout, PixelLayout::Rgba16F(_)) {
        return;
    }
    let row = r.width as usize * 8;
    let p = &mut r.planes[0];
    debug_assert_eq!(p.stride, row);
    let half = |b: &[u8]| hdr::f16_to_f32(u16::from_ne_bytes([b[0], b[1]]));
    bands(&mut p.data, row, parallel, |_, band| {
        for o in band.chunks_exact_mut(8) {
            let a = half(&o[6..8]);
            if a <= 0.0 || a >= 1.0 {
                continue;
            }
            for i in 0..3 {
                let c = finite(half(&o[2 * i..2 * i + 2]) / a);
                o[2 * i..2 * i + 2].copy_from_slice(&hdr::f32_to_f16(c).to_ne_bytes());
            }
        }
    });
}

/// Interleaved RGBA half bits → `Rgba16F`, replacing NaN with 0 and infinities with the
/// largest half (the shaders' clamps are undefined for NaN).
/// The reference for [`Prep`] (decoders stream through [`reduce`]).
#[cfg(test)]
pub(crate) fn f16_to_raster(
    width: u32,
    height: u32,
    data: &[u16],
    primaries: Primaries,
    truncated: bool,
) -> Raster {
    let mut out = Vec::with_capacity(data.len() * 2);
    for &h in data {
        let h = if h & 0x7c00 == 0x7c00 {
            if h & 0x03ff != 0 {
                0
            } else {
                (h & 0x8000) | 0x7bff
            }
        } else {
            h
        };
        out.extend_from_slice(&h.to_ne_bytes());
    }
    one_plane(
        width,
        height,
        PixelLayout::Rgba16F(primaries),
        out,
        truncated,
    )
}

/// Box-filter a whole wide raster (`Rgba16F`, `Rgba16`, `Gray16` or `GrayAlpha16`) to level
/// 1/`scale` of a `full_w`×`full_h` image, exactly
/// `full_w.div_ceil(scale)`×`full_h.div_ceil(scale)`, like `whole::downscale` (short sources
/// repeat their last row/column). All are weighted by
/// (straight) alpha, if any, so colour under transparent pixels adds nothing: half floats are
/// averaged in linear light, 16-bit codes as stored (gamma space, like the 8-bit path). Colour
/// is 0 where the whole box is transparent.
pub(crate) fn downscale_wide(src: &Raster, full_w: u32, full_h: u32, scale: u32) -> Raster {
    let f = (scale / src.scale.max(1)).max(1) as usize;
    let (ow, oh) = (full_w.div_ceil(scale), full_h.div_ceil(scale));
    let half = matches!(src.layout, PixelLayout::Rgba16F(_));
    // Colour samples per pixel, and the index of alpha (none for grey).
    let (nc, alpha) = match src.layout {
        PixelLayout::Gray16 => (1, None),
        PixelLayout::GrayAlpha16 => (1, Some(1)),
        _ => (3, Some(3)),
    };
    let bpp = src.layout.pixel_bytes();
    let p = &src.planes[0];
    let (pw, ph) = (p.width as usize, p.height as usize);
    let stride = ow as usize * bpp;
    let mut data = vec![0u8; stride * oh as usize];
    if pw > 0 && ph > 0 {
        let span = |o: usize, n: usize| {
            let a = (o * f).min(n - 1);
            (a, (o * f + f).clamp(a + 1, n.max(a + 1)))
        };
        let get = |row: &[u8], x: usize, c: usize| {
            let v = u16::from_ne_bytes([row[x * bpp + 2 * c], row[x * bpp + 2 * c + 1]]);
            if half {
                f64::from(hdr::f16_to_f32(v))
            } else {
                f64::from(v)
            }
        };
        let one = if half { 1.0 } else { 65535.0 };
        bands(&mut data, stride, true, |y0, band| {
            for (dy, out) in band.chunks_exact_mut(stride).enumerate() {
                let (sy0, sy1) = span(y0 + dy, ph);
                for ox in 0..ow as usize {
                    let (sx0, sx1) = span(ox, pw);
                    let mut sum = [0f64; 3];
                    let (mut sa, mut n) = (0f64, 0f64);
                    for y in sy0..sy1 {
                        let row = &p.data[y * p.stride..];
                        for x in sx0..sx1 {
                            let a = alpha.map_or(one, |i| get(row, x, i));
                            for (c, s) in sum.iter_mut().take(nc).enumerate() {
                                *s += get(row, x, c) * a;
                            }
                            sa += a;
                            n += 1.0;
                        }
                    }
                    let o = &mut out[ox * bpp..(ox + 1) * bpp];
                    let c = |i: usize| if sa > 0.0 { sum[i] / sa } else { 0.0 };
                    let mut v = [0u16; 4];
                    for (i, v) in v.iter_mut().take(nc).enumerate() {
                        *v = if half {
                            hdr::f32_to_f16(c(i) as f32)
                        } else {
                            c(i).round() as u16
                        };
                    }
                    if let Some(i) = alpha {
                        v[i] = if half {
                            hdr::f32_to_f16((sa / n) as f32)
                        } else {
                            (sa / n).round() as u16
                        };
                    }
                    for (i, v) in v.iter().take(bpp / 2).enumerate() {
                        o[2 * i..2 * i + 2].copy_from_slice(&v.to_ne_bytes());
                    }
                }
            }
        });
    }
    Raster {
        width: ow,
        height: oh,
        origin: (0, 0),
        scale,
        layout: src.layout,
        planes: vec![RasterPlane {
            width: ow,
            height: oh,
            stride,
            data,
            h_div: 1,
            v_div: 1,
        }],
        truncated: src.truncated,
    }
}

/// How a decoder's `f32` RGBA becomes the raster's half floats, pixel by pixel, exactly as
/// the whole-image path does it ([`f32_to_raster`] or [`f16_to_raster`], then
/// [`unpremultiply`]): every value is rounded to half before it is averaged, so a reduced
/// level equals [`downscale_wide`] of the full-size raster.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Prep {
    /// To linear BT.709 (row-major), applied first.
    pub matrix: Option<[[f32; 3]; 3]>,
    /// Alpha clamped to 0..1 with NaN as 1, as [`f32_to_raster`] does; otherwise alpha is only
    /// made finite, as [`f16_to_raster`] does.
    pub clamp_alpha: bool,
    /// Premultiplied colour divided by alpha, as [`unpremultiply`] does.
    pub unpremultiply: bool,
}

/// `v` rounded to the nearest half (ties to even), as `f32`: [`hdr::f32_to_f16`] then back,
/// with the common case (a normal half below 2¹⁵) done on the bits.
fn r16(v: f32) -> f32 {
    let b = v.to_bits();
    if (113..=141).contains(&((b >> 23) & 0xff)) {
        f32::from_bits((b + 0xfff + ((b >> 13) & 1)) & !0x1fff)
    } else {
        hdr::f16_to_f32(hdr::f32_to_f16(v))
    }
}

/// [`hdr::f32_to_f16`], with the common case (a normal half below 2¹⁵) done on the bits.
fn h16(v: f32) -> u16 {
    let b = v.to_bits();
    if (113..=141).contains(&((b >> 23) & 0xff)) {
        let r = b + 0xfff + ((b >> 13) & 1);
        (((b >> 16) & 0x8000) | ((((r >> 23) & 0xff) - 112) << 10) | ((r >> 13) & 0x3ff)) as u16
    } else {
        hdr::f32_to_f16(v)
    }
}

impl Prep {
    fn apply(&self, p: &mut [f32]) {
        let mut c = [p[0], p[1], p[2]];
        if let Some(m) = self.matrix {
            c = hdr::mat_mul(m, c);
        }
        let a = if !self.clamp_alpha {
            finite(p[3])
        } else if p[3].is_nan() {
            1.0
        } else {
            p[3].clamp(0.0, 1.0)
        };
        let a = r16(a);
        let mut c = c.map(|v| r16(finite(v)));
        if self.unpremultiply && a > 0.0 && a < 1.0 {
            c = c.map(|v| r16(finite(v / a)));
        }
        p.copy_from_slice(&[c[0], c[1], c[2], a]);
    }
}

/// The level a reduction reads, and how.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Grid {
    /// Size of the level the box filter runs over (`ceil(full / base)`).
    pub size: (u32, u32),
    /// Size of the level the reader has: a mip level can be a pixel short (its last row and
    /// column are repeated) or long (the extra is ignored).
    pub real: (u32, u32),
    /// Box size: the output is level 1/`f` of `size`.
    pub f: u32,
    /// Rows the reader decodes together; groups of rows are aligned to it when cheap.
    pub band: u32,
    /// The reader's own buffers per thread, for a group of [`Grid::group_rows`] rows.
    pub scratch: u64,
}

/// Source rows read in one go at most, before threads × rows is checked against the budget.
const GROUP_ROWS: u32 = 16;
/// Working memory the threads of one reduction may hold together.
const WORK_BUDGET: u64 = 96 << 20;

impl Grid {
    /// Source rows read together (a multiple of the box size, and of the reader's band when
    /// that is cheap).
    pub(crate) fn group_rows(&self) -> u32 {
        let f = self.f.max(1);
        let band = self.band.clamp(1, 4096);
        let lcm = f / gcd(f, band) * band;
        if lcm <= 256 {
            lcm * GROUP_ROWS.max(band).div_ceil(lcm)
        } else {
            GROUP_ROWS.max(band).next_multiple_of(f)
        }
    }

    /// Output rows per group, and threads.
    fn plan(&self, out: crate::Rect, parallel: bool) -> (u32, usize) {
        let f = self.f.max(1);
        let g = (self.group_rows() / f).max(1);
        let groups = out.h.div_ceil(g).max(1) as usize;
        let threads =
            if parallel && u64::from(out.w) * u64::from(out.h) * u64::from(f * f) > 1 << 16 {
                let per = (self.group_bytes(out, g) + self.scratch).max(1);
                std::thread::available_parallelism()
                    .map_or(1, |n| n.get())
                    .min(16)
                    .min((WORK_BUDGET / per).max(1) as usize)
                    .min(groups)
            } else {
                1
            };
        (g, threads)
    }

    /// Bytes one thread holds for a group of `g` output rows (its `f32` rows and sums).
    fn group_bytes(&self, out: crate::Rect, g: u32) -> u64 {
        let cols = u64::from(out.w) * u64::from(self.f) + u64::from(self.f);
        u64::from(g * self.f) * cols * 16 + u64::from(out.w) * 32
    }

    /// Peak memory of [`reduce`] for `out`: the output, and per thread a group of rows and
    /// the reader's scratch.
    pub(crate) fn memory(&self, out: crate::Rect, parallel: bool) -> u64 {
        let (g, threads) = self.plan(out, parallel);
        let px = u64::from(out.w) * u64::from(out.h) * 8;
        px + threads as u64 * (self.group_bytes(out, g) + self.scratch)
    }
}

fn gcd(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

/// Region `out` (level coordinates) of level 1/`grid.f` of `grid.size`, streamed from a
/// reader: `read(rows, cols, buf, scratch)` fills `buf` with `rows.len() * cols.len()` RGBA
/// `f32` pixels of the real level (and says whether any were damaged). Each pixel goes
/// through `prep`; boxes are then averaged weighted by alpha as [`downscale_wide`] does (in
/// the same order, so the result is identical). Only the output and a few rows per thread are
/// held. The raster has origin `(out.x, out.y)`, scale 1 (the caller sets it), and is
/// `Rgba16F` tagged `primaries` (BT.709 when `prep` has a matrix).
#[allow(clippy::too_many_arguments)]
pub(crate) fn reduce<S: Default>(
    grid: Grid,
    out: crate::Rect,
    prep: Prep,
    primaries: Primaries,
    truncated: bool,
    parallel: bool,
    cancel: &CancelToken,
    read: impl Fn(Range<u32>, Range<u32>, &mut [f32], &mut S) -> bool + Sync,
) -> crate::Result<Raster> {
    let f = grid.f.max(1);
    let (sw, sh) = grid.size;
    let (rw, rh) = grid.real;
    let stride = out.w as usize * 8;
    let len = stride * out.h as usize;
    let mut data = Vec::new();
    data.try_reserve_exact(len)
        .map_err(|_| crate::DecodeError::TooLarge)?;
    data.resize(len, 0u8);
    let (g, threads) = grid.plan(out, parallel);
    let damaged = AtomicBool::new(false);
    let stop = AtomicBool::new(false);
    if out.w > 0 && out.h > 0 && sw > 0 && sh > 0 && rw > 0 && rh > 0 {
        // Source columns: the same for every group.
        let (vx0, vx1) = crate::whole::source_span(out.x, out.w, f, sw);
        let cx0 = vx0.min(rw - 1);
        let cx1 = vx1.min(rw).max(cx0 + 1);
        let cols = (cx1 - cx0) as usize;
        let group =
            |i: usize, rows_out: &mut [u8], buf: &mut Vec<f32>, sums: &mut Vec<f64>, s: &mut S| {
                let oy0 = out.y + i as u32 * g;
                let n_rows = (rows_out.len() / stride) as u32;
                let (vy0, vy1) = crate::whole::source_span(oy0, n_rows, f, sh);
                let ry0 = vy0.min(rh - 1);
                let ry1 = vy1.min(rh).max(ry0 + 1);
                buf.clear();
                buf.resize((ry1 - ry0) as usize * cols * 4, 0.0);
                if read(ry0..ry1, cx0..cx1, buf, s) {
                    damaged.store(true, Ordering::Relaxed);
                }
                for p in buf.chunks_exact_mut(4) {
                    prep.apply(p);
                }
                let at = |vx: u32, vy: u32| {
                    let o = ((vy.min(rh - 1) - ry0) as usize * cols
                        + (vx.min(rw - 1) - cx0) as usize)
                        * 4;
                    &buf[o..o + 4]
                };
                let put = |o: &mut [u8], v: [f32; 4]| {
                    for (i, v) in v.into_iter().enumerate() {
                        o[2 * i..2 * i + 2].copy_from_slice(&h16(v).to_ne_bytes());
                    }
                };
                for (dy, row) in rows_out.chunks_exact_mut(stride).enumerate() {
                    let oy = oy0 + dy as u32;
                    if f == 1 {
                        for (dx, o) in row.chunks_exact_mut(8).enumerate() {
                            let p = at(out.x + dx as u32, oy);
                            put(o, [p[0], p[1], p[2], p[3]]);
                        }
                        continue;
                    }
                    let (y0, y1) = crate::whole::source_span(oy, 1, f, sh);
                    sums.clear();
                    sums.resize(out.w as usize * 5, 0.0);
                    for y in y0..y1 {
                        for (dx, s) in sums.chunks_exact_mut(5).enumerate() {
                            let (x0, x1) = crate::whole::source_span(out.x + dx as u32, 1, f, sw);
                            for x in x0..x1 {
                                let p = at(x, y);
                                let a = f64::from(p[3]);
                                for c in 0..3 {
                                    s[c] += f64::from(p[c]) * a;
                                }
                                s[3] += a;
                                s[4] += 1.0;
                            }
                        }
                    }
                    for (o, s) in row.chunks_exact_mut(8).zip(sums.chunks_exact(5)) {
                        let (sa, n) = (s[3], s[4]);
                        let c = |i: usize| if sa > 0.0 { (s[i] / sa) as f32 } else { 0.0 };
                        put(o, [c(0), c(1), c(2), (sa / n) as f32]);
                    }
                }
            };
        let chunk = stride * g as usize;
        let jobs = Mutex::new(data.chunks_mut(chunk).enumerate());
        let work = || {
            let (mut buf, mut sums, mut s) = (Vec::new(), Vec::new(), S::default());
            loop {
                if cancel.is_cancelled() {
                    stop.store(true, Ordering::Relaxed);
                    return;
                }
                let Some((i, rows)) = jobs.lock().next() else {
                    return;
                };
                group(i, rows, &mut buf, &mut sums, &mut s);
            }
        };
        if threads <= 1 {
            work();
        } else {
            std::thread::scope(|sc| {
                for wi in 0..threads {
                    specular_thread::spawn_scoped(sc, format!("dec-linear-{wi}"), work);
                }
            });
        }
    }
    if stop.into_inner() {
        return Err(crate::DecodeError::Cancelled);
    }
    let p = if prep.matrix.is_some() {
        Primaries::Bt709
    } else {
        primaries
    };
    let mut r = one_plane(
        out.w,
        out.h,
        PixelLayout::Rgba16F(p),
        data,
        truncated || damaged.into_inner(),
    );
    r.origin = (out.x, out.y);
    Ok(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn px(r: &Raster, x: usize, y: usize) -> [f32; 4] {
        let p = &r.planes[0];
        let o = y * p.stride + x * 8;
        std::array::from_fn(|i| {
            hdr::f16_to_f32(u16::from_ne_bytes([
                p.data[o + 2 * i],
                p.data[o + 2 * i + 1],
            ]))
        })
    }

    #[test]
    fn conversion_sanitises_and_applies_the_matrix() {
        let data = [2.0, f32::NAN, f32::INFINITY, 0.5, 1.0, 1.0, 1.0, f32::NAN];
        let r = f32_to_raster(2, 1, 4, &data, None, Primaries::Bt2020, false, false);
        assert_eq!(r.layout, PixelLayout::Rgba16F(Primaries::Bt2020));
        assert_eq!(px(&r, 0, 0), [2.0, 0.0, F16_MAX, 0.5]);
        assert_eq!(px(&r, 1, 0), [1.0, 1.0, 1.0, 1.0]);
        let swap = [[0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]];
        let r = f32_to_raster(
            1,
            1,
            3,
            &[1.0, 2.0, 3.0],
            Some(swap),
            Primaries::Bt2020,
            false,
            false,
        );
        assert_eq!(r.layout, PixelLayout::Rgba16F(Primaries::Bt709));
        assert_eq!(px(&r, 0, 0), [2.0, 1.0, 3.0, 1.0]);
        let r = f16_to_raster(
            1,
            1,
            &[0x7c00, 0x7e00, 0xfc00, 0x3c00],
            Primaries::Bt709,
            false,
        );
        assert_eq!(px(&r, 0, 0), [F16_MAX, 0.0, -F16_MAX, 1.0]);
    }

    #[test]
    fn unpremultiply_divides_partial_alpha_only() {
        // Half-transparent 0.25 grey, transparent emissive, opaque 2.0 (HDR).
        let data = [
            0.25, 0.125, 0.0, 0.5, 0.5, 0.5, 0.5, 0.0, 2.0, 2.0, 2.0, 1.0,
        ];
        let mut r = f32_to_raster(3, 1, 4, &data, None, Primaries::Bt709, false, false);
        unpremultiply(&mut r, false);
        assert_eq!(px(&r, 0, 0), [0.5, 0.25, 0.0, 0.5]);
        assert_eq!(px(&r, 1, 0), [0.5, 0.5, 0.5, 0.0]);
        assert_eq!(px(&r, 2, 0), [2.0, 2.0, 2.0, 1.0]);
    }

    #[test]
    fn threaded_conversion_matches() {
        let (w, h) = (300u32, 1000u32);
        let data: Vec<f32> = (0..w * h * 3).map(|i| (i % 977) as f32 * 0.01).collect();
        let a = f32_to_raster(w, h, 3, &data, None, Primaries::Bt709, false, false);
        let b = f32_to_raster(w, h, 3, &data, None, Primaries::Bt709, false, true);
        assert_eq!(a.planes[0].data, b.planes[0].data);
    }

    #[test]
    fn downscale_is_alpha_weighted_and_exact_size() {
        // 3×1: red 4.0 opaque, transparent, blue 1.0 opaque.
        let data = [4.0, 0.0, 0.0, 1.0, 9.0, 9.0, 9.0, 0.0, 0.0, 0.0, 1.0, 1.0];
        let r = f32_to_raster(3, 1, 4, &data, None, Primaries::Bt709, false, false);
        let d = downscale_wide(&r, 3, 1, 2);
        assert_eq!((d.width, d.height, d.scale), (2, 1, 2));
        assert_eq!(px(&d, 0, 0), [4.0, 0.0, 0.0, 0.5]);
        assert_eq!(px(&d, 1, 0), [0.0, 0.0, 1.0, 1.0]);
    }

    #[test]
    fn downscale_of_16_bit_codes_is_alpha_weighted() {
        // 3×2 Rgba16: opaque red, transparent white, a quarter-opaque blue; then a fully
        // transparent column (colour must not leak) and opaque grey.
        let px16: [[u16; 4]; 6] = [
            [65535, 0, 0, 65535],
            [65535, 65535, 65535, 0],
            [0, 0, 65535, 16384],
            [65535, 65535, 65535, 0],
            [65535, 65535, 65535, 0],
            [1000, 2000, 3000, 65535],
        ];
        let data: Vec<u8> = px16
            .iter()
            .flatten()
            .flat_map(|v| v.to_ne_bytes())
            .collect();
        let r = Raster {
            width: 3,
            height: 2,
            origin: (0, 0),
            scale: 1,
            layout: PixelLayout::Rgba16(specular_core::HdrEncoding::SRGB),
            planes: vec![RasterPlane {
                width: 3,
                height: 2,
                stride: 24,
                data,
                h_div: 1,
                v_div: 1,
            }],
            truncated: false,
        };
        let d = downscale_wide(&r, 3, 2, 2);
        assert_eq!((d.width, d.height, d.layout), (2, 1, r.layout));
        let get = |x: usize| -> [u16; 4] {
            let o = &d.planes[0].data[x * 8..x * 8 + 8];
            std::array::from_fn(|i| u16::from_ne_bytes([o[2 * i], o[2 * i + 1]]))
        };
        // Red and white over the two transparent white pixels below: only red has weight.
        assert_eq!(get(0), [65535, 0, 0, 16384]);
        // Blue (a quarter) and grey (opaque): weighted 1:4, alpha (16384 + 65535) / 2.
        let w = |b: f64, g: f64| ((b * 16384.0 + g * 65535.0) / 81919.0).round() as u16;
        assert_eq!(
            get(1),
            [w(0.0, 1000.0), w(0.0, 2000.0), w(65535.0, 3000.0), 40960]
        );
    }
    #[test]
    fn downscale_of_16_bit_grey() {
        let r = |layout, w: u32, px: &[u16]| Raster {
            width: w,
            height: 1,
            origin: (0, 0),
            scale: 1,
            layout,
            planes: vec![RasterPlane {
                width: w,
                height: 1,
                stride: px.len() * 2,
                data: px.iter().flat_map(|v| v.to_ne_bytes()).collect(),
                h_div: 1,
                v_div: 1,
            }],
            truncated: false,
        };
        let ne = |d: &Raster| -> Vec<u16> {
            d.planes[0]
                .data
                .chunks_exact(2)
                .map(|b| u16::from_ne_bytes([b[0], b[1]]))
                .collect()
        };
        let g = downscale_wide(&r(PixelLayout::Gray16, 3, &[1000, 2001, 50000]), 3, 1, 2);
        assert_eq!(
            (g.width, g.layout, g.planes[0].stride),
            (2, PixelLayout::Gray16, 4)
        );
        assert_eq!(ne(&g), [1501, 50000]);
        // Grey + alpha: the transparent pixel adds nothing to the grey.
        let ga = r(PixelLayout::GrayAlpha16, 2, &[40000, 65535, 9, 0]);
        let d = downscale_wide(&ga, 2, 1, 2);
        assert_eq!((d.width, d.planes[0].stride), (1, 4));
        assert_eq!(ne(&d), [40000, 32768]);
    }

    /// A `w`×`h` RGBA test image with HDR values, NaN, infinities and partial alpha.
    fn awkward(w: u32, h: u32) -> Vec<f32> {
        (0..w * h)
            .flat_map(|i| {
                let (x, y) = ((i % w) as f32, (i / w) as f32);
                let a = match i % 7 {
                    0 => 0.0,
                    1 => 0.25,
                    2 => 1.5,
                    3 => f32::NAN,
                    _ => 1.0,
                };
                let r = match i % 11 {
                    0 => f32::NAN,
                    1 => f32::INFINITY,
                    2 => 1e6,
                    _ => x * 0.37 + 0.01,
                };
                [r, y * 1.7 - 3.0, (x * y) * 0.001 + 1e-6, a]
            })
            .collect()
    }

    /// `reduce` over `data` (`real` pixels wide/high, box `f` over a level of `size`).
    #[allow(clippy::too_many_arguments)]
    fn streamed(
        data: &[f32],
        real: (u32, u32),
        size: (u32, u32),
        f: u32,
        band: u32,
        out: crate::Rect,
        prep: Prep,
        parallel: bool,
    ) -> Raster {
        let grid = Grid {
            size,
            real,
            f,
            band,
            scratch: 0,
        };
        reduce(
            grid,
            out,
            prep,
            Primaries::Bt2020,
            false,
            parallel,
            &CancelToken::new(),
            |rows, cols, buf, _: &mut ()| {
                let n = cols.len() * 4;
                for (y, o) in rows.zip(buf.chunks_exact_mut(n)) {
                    let s = (y * real.0 + cols.start) as usize * 4;
                    o.copy_from_slice(&data[s..s + n]);
                }
                false
            },
        )
        .unwrap()
    }

    fn whole(w: u32, h: u32, f: u32) -> crate::Rect {
        crate::Rect {
            x: 0,
            y: 0,
            w: w.div_ceil(f),
            h: h.div_ceil(f),
        }
    }

    #[test]
    fn fast_half_rounding_matches() {
        for i in (0..u32::MAX).step_by(997) {
            let v = f32::from_bits(i);
            if v.is_nan() {
                continue;
            }
            assert_eq!(h16(v), hdr::f32_to_f16(v), "{v}");
            assert_eq!(
                r16(v).to_bits(),
                hdr::f16_to_f32(hdr::f32_to_f16(v)).to_bits(),
                "{v}"
            );
        }
    }

    #[test]
    fn streamed_reduction_equals_the_whole_image_path() {
        let (w, h) = (37u32, 29u32);
        let data = awkward(w, h);
        let swap = [[0.0, 1.0, 0.0], [1.0, 0.0, 0.5], [0.0, 0.0, 1.0]];
        for (prep, name) in [
            (
                Prep {
                    matrix: None,
                    clamp_alpha: true,
                    unpremultiply: false,
                },
                "radiance",
            ),
            (
                Prep {
                    matrix: Some(swap),
                    clamp_alpha: true,
                    unpremultiply: true,
                },
                "exr matrix",
            ),
            (
                Prep {
                    matrix: None,
                    clamp_alpha: false,
                    unpremultiply: true,
                },
                "exr half",
            ),
        ] {
            // The whole-image path: convert, unpremultiply, box filter.
            let mut full = if prep.clamp_alpha {
                f32_to_raster(w, h, 4, &data, prep.matrix, Primaries::Bt2020, false, false)
            } else {
                let bits: Vec<u16> = data.iter().map(|&v| hdr::f32_to_f16(v)).collect();
                f16_to_raster(w, h, &bits, Primaries::Bt2020, false)
            };
            if prep.unpremultiply {
                unpremultiply(&mut full, false);
            }
            for f in [1, 2, 4, 8] {
                let want = if f == 1 {
                    full.clone()
                } else {
                    downscale_wide(&full, w, h, f)
                };
                for (band, parallel) in [(1, false), (16, true), (32, true)] {
                    let got = streamed(
                        &data,
                        (w, h),
                        (w, h),
                        f,
                        band,
                        whole(w, h, f),
                        prep,
                        parallel,
                    );
                    assert_eq!((got.width, got.height), (want.width, want.height));
                    assert_eq!(got.layout, want.layout, "{name}");
                    assert!(
                        got.planes[0].data == want.planes[0].data,
                        "{name} 1/{f} band {band}"
                    );
                }
                // Regions are the same pixels as the whole level.
                let l = whole(w, h, f);
                for (x, y, rw, rh) in [
                    (0, 0, 1, 1),
                    (1, 2, 3, 2),
                    (l.w - 1, l.h - 1, 1, 1),
                    (2, 1, l.w - 2, l.h - 1),
                ] {
                    let r = crate::Rect { x, y, w: rw, h: rh };
                    let got = streamed(&data, (w, h), (w, h), f, 4, r, prep, false);
                    assert_eq!(got.origin, (x, y));
                    for yy in 0..rh as usize {
                        let a = &got.planes[0].data[yy * rw as usize * 8..][..rw as usize * 8];
                        let o = ((y as usize + yy) * l.w as usize + x as usize) * 8;
                        assert_eq!(
                            a,
                            &want.planes[0].data[o..o + rw as usize * 8],
                            "{name} 1/{f} region {r:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn short_and_long_source_levels_repeat_or_ignore_the_edge() {
        // A mip level a pixel short (floor rounding) of the 11×7 grid at 1/2 (6×4), and one a
        // pixel long: as the old path, which padded or cut it to 6×4 first.
        let prep = Prep {
            matrix: None,
            clamp_alpha: true,
            unpremultiply: false,
        };
        for real in [(5u32, 3u32), (7, 5)] {
            let data = awkward(real.0, real.1);
            let mut base = f32_to_raster(
                real.0,
                real.1,
                4,
                &data,
                None,
                Primaries::Bt2020,
                false,
                false,
            );
            base.scale = 2;
            let padded = downscale_wide(&base, 11, 7, 2);
            for f in [1, 2, 4] {
                let want = if f == 1 {
                    padded.clone()
                } else {
                    downscale_wide(&padded, 11, 7, 2 * f)
                };
                let mut got = streamed(&data, real, (6, 4), f, 1, whole(6, 4, f), prep, false);
                // Padding a level by box filter zeroed the colour under alpha 0 (never shown);
                // the stream keeps it as stored.
                for p in got.planes[0].data.chunks_exact_mut(8) {
                    if p[6..8] == [0, 0] {
                        p[..6].fill(0);
                    }
                }
                assert_eq!((got.width, got.height), (want.width, want.height));
                assert!(got.planes[0].data == want.planes[0].data, "{real:?} 1/{f}");
            }
        }
    }

    #[test]
    fn reduction_is_cancellable_and_counts_its_memory() {
        let grid = Grid {
            size: (8192, 4096),
            real: (8192, 4096),
            f: 2,
            band: 1,
            scratch: 0,
        };
        let out = whole(8192, 4096, 2);
        let m = grid.memory(out, true);
        let px = 4096 * 2048 * 8;
        assert!(m >= px && m < px + WORK_BUDGET + (16 << 20), "{m}");
        let c = CancelToken::new();
        c.cancel();
        let r = reduce(
            grid,
            out,
            Prep {
                matrix: None,
                clamp_alpha: true,
                unpremultiply: false,
            },
            Primaries::Bt709,
            false,
            true,
            &c,
            |_, _, _, _: &mut ()| false,
        );
        assert!(matches!(r, Err(crate::DecodeError::Cancelled)));
    }
}
