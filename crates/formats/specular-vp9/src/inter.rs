//! Block inter prediction (section 8.5.2.4): 8-tap sub-sample interpolation from a reference
//! plane, with the reference clamped at its edges and optional scaling.

use std::simd::{Simd, cmp::SimdOrd, num::SimdInt, num::SimdUint};

use crate::frame::{Pixel, Plane};
use crate::tables::SUBPEL_FILTERS;

/// Scratch buffers for one tile.
pub(crate) struct McScratch<P> {
    /// Edge-emulated reference samples.
    emu: Vec<P>,
    /// The horizontally filtered rows.
    inter: Vec<u16>,
}

impl<P: Pixel> McScratch<P> {
    pub(crate) fn new() -> Self {
        McScratch {
            emu: vec![P::default(); 80 * 80],
            inter: vec![0; 64 * 330],
        }
    }
}

/// Predicts a `w` x `h` block (w, h <= 64) into `out` (stride 64) from position (`x`, `y`)
/// of `rp` in 1/16 samples, advancing `step_x` / `step_y` per sample.
#[allow(clippy::too_many_arguments)]
pub(crate) fn predict<P: Pixel>(
    rp: &Plane<P>,
    x: i64,
    y: i64,
    step_x: i64,
    step_y: i64,
    w: usize,
    h: usize,
    filter: usize,
    bit_depth: u8,
    out: &mut [u16],
    s: &mut McScratch<P>,
) {
    let last_x = rp.w as i64 - 1;
    let last_y = rp.h as i64 - 1;
    let max = (1i32 << bit_depth) - 1;
    let kern = &SUBPEL_FILTERS[filter & 3];
    if step_x == 16 && step_y == 16 {
        let ix = x >> 4;
        let iy = y >> 4;
        let fx = (x & 15) as usize;
        let fy = (y & 15) as usize;
        // The samples the filters read: 3 before and 4 after along a filtered direction.
        let (x_lo, x_hi) = if fx != 0 {
            (ix - 3, ix + w as i64 + 4)
        } else {
            (ix, ix + w as i64)
        };
        let (y_lo, y_hi) = if fy != 0 {
            (iy - 3, iy + h as i64 + 4)
        } else {
            (iy, iy + h as i64)
        };
        let inside = x_lo >= 0 && y_lo >= 0 && x_hi - 1 <= last_x && y_hi - 1 <= last_y;
        let (src, sstride, ox, oy): (&[P], usize, usize, usize) = if inside {
            (&rp.data[..], rp.stride, x_lo as usize, y_lo as usize)
        } else {
            let ew = (x_hi - x_lo) as usize;
            let eh = (y_hi - y_lo) as usize;
            for r in 0..eh {
                let sy = (y_lo + r as i64).clamp(0, last_y) as usize;
                let srow = rp.row(sy);
                let drow = &mut s.emu[r * 80..r * 80 + ew];
                for (c, d) in drow.iter_mut().enumerate() {
                    *d = srow[(x_lo + c as i64).clamp(0, last_x) as usize];
                }
            }
            (&s.emu[..], 80, 0, 0)
        };
        let at = |r: usize, c: usize| (oy + r) * sstride + ox + c;
        match (fx != 0, fy != 0) {
            (false, false) => {
                for r in 0..h {
                    let srow = &src[at(r, 0)..at(r, 0) + w];
                    for (d, v) in out[r * 64..r * 64 + w].iter_mut().zip(srow) {
                        *d = v.get() as u16;
                    }
                }
            }
            (true, false) => {
                for r in 0..h {
                    hfilter(&src[at(r, 0)..], &kern[fx], w, max, &mut out[r * 64..]);
                }
            }
            (false, true) => {
                let f = &kern[fy];
                for r in 0..h {
                    vfilter_px(src, at(r, 0), sstride, f, w, max, &mut out[r * 64..]);
                }
            }
            (true, true) => {
                let ih = h + 7;
                for r in 0..ih {
                    hfilter(&src[at(r, 0)..], &kern[fx], w, max, &mut s.inter[r * 64..]);
                }
                let f = &kern[fy];
                for r in 0..h {
                    vfilter16(&s.inter, r * 64, f, w, max, &mut out[r * 64..]);
                }
            }
        }
        return;
    }
    // Scaled reference: the general process of the specification.
    let ih = ((((h as i64 - 1) * step_y + 15) >> 4) + 8) as usize;
    let iy0 = (y >> 4) - 3;
    for r in 0..ih {
        let row = rp.row((iy0 + r as i64).clamp(0, last_y) as usize);
        for c in 0..w {
            let p = x + step_x * c as i64;
            let f = &kern[(p & 15) as usize];
            let px = (p >> 4) - 3;
            let mut acc = 0i32;
            for t in 0..8 {
                acc += f[t] as i32 * row[(px + t as i64).clamp(0, last_x) as usize].get();
            }
            s.inter[r * 64 + c] = ((acc + 64) >> 7).clamp(0, max) as u16;
        }
    }
    for r in 0..h {
        let p = (y & 15) + step_y * r as i64;
        let f = &kern[(p & 15) as usize];
        let base = (p >> 4) as usize;
        for c in 0..w {
            let mut acc = 0i32;
            for t in 0..8 {
                acc += f[t] as i32 * s.inter[(base + t) * 64 + c] as i32;
            }
            out[r * 64 + c] = ((acc + 64) >> 7).clamp(0, max) as u16;
        }
    }
}

/// Horizontal 8-tap filter of one row; `src[0]` is the sample 3 before the first output.
#[inline(always)]
fn hfilter<P: Pixel>(src: &[P], f: &[i16; 8], w: usize, max: i32, out: &mut [u16]) {
    if w >= 8 {
        let maxv = Simd::splat(max);
        let mut c = 0;
        while c < w {
            let mut acc = Simd::<i32, 8>::splat(64);
            for t in 0..8 {
                acc += P::load8(&src[c + t..]) * Simd::splat(f[t] as i32);
            }
            let v = (acc >> Simd::splat(7)).simd_clamp(Simd::splat(0), maxv);
            out[c..c + 8].copy_from_slice(v.cast::<u16>().as_array());
            c += 8;
        }
    } else {
        for c in 0..w {
            let mut acc = 64;
            for t in 0..8 {
                acc += f[t] as i32 * src[c + t].get();
            }
            out[c] = (acc >> 7).clamp(0, max) as u16;
        }
    }
}

/// Vertical 8-tap filter of one output row straight from the reference samples.
#[inline(always)]
fn vfilter_px<P: Pixel>(
    src: &[P],
    start: usize,
    stride: usize,
    f: &[i16; 8],
    w: usize,
    max: i32,
    out: &mut [u16],
) {
    if w >= 8 {
        let maxv = Simd::splat(max);
        let mut c = 0;
        while c < w {
            let mut acc = Simd::<i32, 8>::splat(64);
            for t in 0..8 {
                acc += P::load8(&src[start + t * stride + c..]) * Simd::splat(f[t] as i32);
            }
            let v = (acc >> Simd::splat(7)).simd_clamp(Simd::splat(0), maxv);
            out[c..c + 8].copy_from_slice(v.cast::<u16>().as_array());
            c += 8;
        }
    } else {
        for c in 0..w {
            let mut acc = 64;
            for t in 0..8 {
                acc += f[t] as i32 * src[start + t * stride + c].get();
            }
            out[c] = (acc >> 7).clamp(0, max) as u16;
        }
    }
}

/// Vertical 8-tap filter of one output row from the intermediate rows (stride 64).
#[inline(always)]
fn vfilter16(src: &[u16], start: usize, f: &[i16; 8], w: usize, max: i32, out: &mut [u16]) {
    if w >= 8 {
        let maxv = Simd::splat(max);
        let mut c = 0;
        while c < w {
            let mut acc = Simd::<i32, 8>::splat(64);
            for t in 0..8 {
                let v: Simd<i32, 8> =
                    Simd::<u16, 8>::from_slice(&src[start + t * 64 + c..start + t * 64 + c + 8])
                        .cast();
                acc += v * Simd::splat(f[t] as i32);
            }
            let v = (acc >> Simd::splat(7)).simd_clamp(Simd::splat(0), maxv);
            out[c..c + 8].copy_from_slice(v.cast::<u16>().as_array());
            c += 8;
        }
    } else {
        for c in 0..w {
            let mut acc = 64;
            for t in 0..8 {
                acc += f[t] as i32 * src[start + t * 64 + c] as i32;
            }
            out[c] = (acc >> 7).clamp(0, max) as u16;
        }
    }
}

/// Writes a prediction (or the rounded average of two) to the frame.
#[inline]
pub(crate) fn store<P: Pixel>(
    rows: &mut [&mut [P]],
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    a: &[u16],
    b: Option<&[u16]>,
) {
    for r in 0..h {
        let d = &mut rows[y + r][x..x + w];
        let pa = &a[r * 64..r * 64 + w];
        match b {
            None => {
                for (d, v) in d.iter_mut().zip(pa) {
                    *d = P::put(*v as i32);
                }
            }
            Some(b) => {
                let pb = &b[r * 64..r * 64 + w];
                for ((d, u), v) in d.iter_mut().zip(pa).zip(pb) {
                    *d = P::put((*u as i32 + *v as i32 + 1) >> 1);
                }
            }
        }
    }
}
