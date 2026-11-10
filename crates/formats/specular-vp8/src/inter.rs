//! Motion-compensated prediction (RFC 6386 section 18): six-tap or bilinear sub-pixel
//! interpolation from a reference plane whose edges repeat without end.

use std::simd::Simd;
use std::simd::cmp::SimdOrd;
use std::simd::num::{SimdInt, SimdUint};

use crate::frame::{Plane, copy_n};
use crate::tables::{BILINEAR_FILTERS, SIXTAP_FILTERS};

/// The six taps for an eighth-pixel offset.
#[inline(always)]
fn taps(bilinear: bool, frac: i32) -> [i32; 6] {
    let t = if bilinear {
        &BILINEAR_FILTERS
    } else {
        &SIXTAP_FILTERS
    };
    let f = &t[frac as usize * 6..frac as usize * 6 + 6];
    [
        f[0] as i32,
        f[1] as i32,
        f[2] as i32,
        f[3] as i32,
        f[4] as i32,
        f[5] as i32,
    ]
}

/// Two-tap form for the bilinear filters (their other taps are zero).
#[inline(always)]
fn apply2(f0: i32, f1: i32, a: u8, b: u8) -> u8 {
    ((a as i32 * f0 + b as i32 * f1 + 64) >> 7) as u8
}

/// Rows and columns read around a block: 2 before, 3 after.
const PAD: usize = 5;

/// Predicts a `w`x`h` block at (`x`, `y`) in plane coordinates displaced by the vector
/// (`mx`, `my`) in eighth pixels of this plane, into `dst[off..]` (row pitch `stride`).
#[allow(clippy::too_many_arguments)]
pub(crate) fn predict(
    r: &Plane,
    dst: &mut [u8],
    off: usize,
    stride: usize,
    x: i32,
    y: i32,
    mx: i32,
    my: i32,
    w: usize,
    h: usize,
    bilinear: bool,
) {
    let fx = mx & 7;
    let fy = my & 7;
    let px = x + (mx >> 3);
    let py = y + (my >> 3);
    // The source area, from 2 pixels before the block to 3 after.
    let (sx, sy) = (px - 2, py - 2);
    let (aw, ah) = (w + PAD, h + PAD);
    let b = r.border as i32;
    let mut tmp = [0u8; 21 * 21];
    let (src, sstride, base): (&[u8], usize, usize) = if sx >= -b
        && sy >= -b
        && sx + aw as i32 <= r.width as i32 + b
        && sy + ah as i32 <= r.height as i32 + b
    {
        let base = (sy + b) as usize * r.stride + (sx + b) as usize;
        (&r.data, r.stride, base)
    } else {
        // Far outside the frame: gather with clamped coordinates.
        let (mw, mh) = (r.width as i32 - 1, r.height as i32 - 1);
        for j in 0..ah {
            let yy = (sy + j as i32).clamp(0, mh) as usize;
            let row = (yy + r.border) * r.stride + r.border;
            for i in 0..aw {
                let xx = (sx + i as i32).clamp(0, mw) as usize;
                tmp[j * aw + i] = r.data[row + xx];
            }
        }
        (&tmp[..], aw, 0)
    };
    // `base` is the area's top-left; the block starts 2 rows and 2 columns in.
    let start = base + 2 * sstride + 2;
    if fx == 0 && fy == 0 {
        for j in 0..h {
            copy_n(&mut dst[off + j * stride..], &src[start + j * sstride..], w);
        }
        return;
    }
    if bilinear {
        let (h0, h1) = (128 - 16 * fx, 16 * fx);
        let (v0, v1) = (128 - 16 * fy, 16 * fy);
        let rows = if fy != 0 { h + 1 } else { h };
        let mut mid = [0u8; 17 * 16];
        for j in 0..rows {
            let s = &src[start + j * sstride..start + j * sstride + w + 1];
            let m = &mut mid[j * w..j * w + w];
            for i in 0..w {
                m[i] = if fx != 0 {
                    apply2(h0, h1, s[i], s[i + 1])
                } else {
                    s[i]
                };
            }
        }
        for j in 0..h {
            let d = &mut dst[off + j * stride..off + j * stride + w];
            for i in 0..w {
                d[i] = if fy != 0 {
                    apply2(v0, v1, mid[j * w + i], mid[(j + 1) * w + i])
                } else {
                    mid[j * w + i]
                };
            }
        }
        return;
    }
    let fh = taps(false, fx);
    let fv = taps(false, fy);
    match w {
        16 => sixtap::<16>(src, sstride, base, dst, off, stride, h, fx, fy, &fh, &fv),
        8 => sixtap::<8>(src, sstride, base, dst, off, stride, h, fx, fy, &fh, &fv),
        _ => sixtap::<4>(src, sstride, base, dst, off, stride, h, fx, fy, &fh, &fv),
    }
}

/// One output row of a six-tap pass: `rows[k]` holds the inputs for tap `k`.
#[inline(always)]
fn taps6<const N: usize>(rows: [&[u8]; 6], f: &[i32; 6]) -> [u8; N] {
    let mut acc = Simd::<i32, N>::splat(64);
    for k in 0..6 {
        if f[k] != 0 {
            acc += Simd::<u8, N>::from_slice(rows[k]).cast::<i32>() * Simd::splat(f[k]);
        }
    }
    (acc >> Simd::splat(7))
        .simd_clamp(Simd::splat(0), Simd::splat(255))
        .cast::<u8>()
        .to_array()
}

#[allow(clippy::too_many_arguments)]
#[inline(always)]
fn sixtap<const N: usize>(
    src: &[u8],
    ss: usize,
    base: usize,
    dst: &mut [u8],
    off: usize,
    stride: usize,
    h: usize,
    fx: i32,
    fy: i32,
    fh: &[i32; 6],
    fv: &[i32; 6],
) {
    let hrow = |o: usize| -> [u8; N] {
        taps6::<N>(
            [
                &src[o..],
                &src[o + 1..],
                &src[o + 2..],
                &src[o + 3..],
                &src[o + 4..],
                &src[o + 5..],
            ],
            fh,
        )
    };
    let start = base + 2 * ss + 2;
    if fy == 0 {
        for j in 0..h {
            let r = hrow(start - 2 + j * ss);
            dst[off + j * stride..off + j * stride + N].copy_from_slice(&r);
        }
        return;
    }
    if fx == 0 {
        for j in 0..h {
            let o = start + j * ss;
            let r = taps6::<N>(
                [
                    &src[o - 2 * ss..],
                    &src[o - ss..],
                    &src[o..],
                    &src[o + ss..],
                    &src[o + 2 * ss..],
                    &src[o + 3 * ss..],
                ],
                fv,
            );
            dst[off + j * stride..off + j * stride + N].copy_from_slice(&r);
        }
        return;
    }
    // Horizontal pass over h + 5 rows, then vertical.
    let mut mid = [[0u8; N]; 21];
    for j in 0..h + PAD {
        mid[j] = hrow(base + j * ss);
    }
    for j in 0..h {
        let r = taps6::<N>(
            [
                &mid[j],
                &mid[j + 1],
                &mid[j + 2],
                &mid[j + 3],
                &mid[j + 4],
                &mid[j + 5],
            ],
            fv,
        );
        dst[off + j * stride..off + j * stride + N].copy_from_slice(&r);
    }
}
