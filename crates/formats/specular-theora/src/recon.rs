//! Pixel work: predictors (7.9.1), the inverse DCT (7.9.3), the loop filter (7.10) and
//! reference padding. Planes are padded buffers addressed as `origin + y * stride + x`,
//! with y counting up from the bottom row as in the specification.

use crate::layout::{BORDER, PlaneGeom};

const C1: i32 = 64277;
const C2: i32 = 60547;
const C3: i32 = 54491;
const C4: i32 = 46341;
const C5: i32 = 36410;
const C6: i32 = 25080;
const C7: i32 = 12785;
const S7: i32 = C1;
const S6: i32 = C2;
const S3: i32 = C5;

#[inline(always)]
fn t16(v: i32) -> i32 {
    v as i16 as i32
}

/// The 1D inverse DCT (7.9.3.1) over `x[i * step]`, i in 0..8.
#[inline(always)]
fn idct1(x: &mut [i32; 64], base: usize, step: usize) {
    let y = |i: usize| x[base + i * step];
    let (y0, y1, y2, y3, y4, y5, y6, y7) = (y(0), y(1), y(2), y(3), y(4), y(5), y(6), y(7));
    let mut t0 = (C4 * t16(y0 + y4)) >> 16;
    let mut t1 = (C4 * t16(y0 - y4)) >> 16;
    let mut t2 = ((C6 * y2) >> 16) - ((S6 * y6) >> 16);
    let mut t3 = ((S6 * y2) >> 16) + ((C6 * y6) >> 16);
    let mut t4 = ((C7 * y1) >> 16) - ((S7 * y7) >> 16);
    let mut t5 = ((C3 * y5) >> 16) - ((S3 * y3) >> 16);
    let mut t6 = ((S3 * y5) >> 16) + ((C3 * y3) >> 16);
    let mut t7 = ((S7 * y1) >> 16) + ((C7 * y7) >> 16);
    let r = t4 + t5;
    t5 = (C4 * t16(t4 - t5)) >> 16;
    t4 = r;
    let r = t7 + t6;
    t6 = (C4 * t16(t7 - t6)) >> 16;
    t7 = r;
    let r = t0 + t3;
    t3 = t0 - t3;
    t0 = r;
    let r = t1 + t2;
    t2 = t1 - t2;
    t1 = r;
    let r = t6 + t5;
    t5 = t6 - t5;
    t6 = r;
    x[base] = t16(t0 + t7);
    x[base + step] = t16(t1 + t6);
    x[base + 2 * step] = t16(t2 + t5);
    x[base + 3 * step] = t16(t3 + t4);
    x[base + 4 * step] = t16(t3 - t4);
    x[base + 5 * step] = t16(t2 - t5);
    x[base + 6 * step] = t16(t1 - t6);
    x[base + 7 * step] = t16(t0 - t7);
}

/// The 2D inverse DCT (7.9.3.2) of dequantised coefficients in natural order, in place;
/// returns the residual, row `by` (from the bottom) at `by * 8`.
pub(crate) fn idct2(x: &mut [i32; 64]) {
    for r in 0..8 {
        let row = &x[r * 8..r * 8 + 8];
        if row.iter().any(|&v| v != 0) {
            idct1(x, r * 8, 1);
        }
    }
    for c in 0..8 {
        idct1(x, c, 8);
    }
    for v in x.iter_mut() {
        *v = (*v + 8) >> 4;
    }
}

/// Splits a motion vector component (half pixels, or quarter pixels on a subsampled axis)
/// into the whole-pixel offsets truncated towards and away from zero.
#[inline(always)]
pub(crate) fn split_mv(m: i32, sub: u32) -> (isize, isize) {
    let unit = 2 << sub;
    let t = m / unit;
    let a = if m % unit != 0 { t + m.signum() } else { t };
    (t as isize, a as isize)
}

/// Forms the inter predictor of the block at pixel offset `pos` (already including the
/// origin) from `refp`.
#[inline(always)]
pub(crate) fn predict(
    refp: &[u8],
    stride: usize,
    pos: usize,
    mv: [i8; 2],
    g: &PlaneGeom,
    out: &mut [u8; 64],
) {
    let (x1, x2) = split_mv(i32::from(mv[0]), g.sx);
    let (y1, y2) = split_mv(i32::from(mv[1]), g.sy);
    let s = stride as isize;
    let o1 = (pos as isize + y1 * s + x1) as usize;
    let o2 = (pos as isize + y2 * s + x2) as usize;
    if o1 == o2 {
        for r in 0..8 {
            let src = &refp[o1 + r * stride..o1 + r * stride + 8];
            out[r * 8..r * 8 + 8].copy_from_slice(src);
        }
    } else {
        for r in 0..8 {
            let a = &refp[o1 + r * stride..o1 + r * stride + 8];
            let b = &refp[o2 + r * stride..o2 + r * stride + 8];
            let d = &mut out[r * 8..r * 8 + 8];
            for i in 0..8 {
                d[i] = ((u16::from(a[i]) + u16::from(b[i])) >> 1) as u8;
            }
        }
    }
}

/// Writes predictor + residual, clamped, into the plane at `pos`.
#[inline(always)]
pub(crate) fn put_res(dst: &mut [u8], stride: usize, pos: usize, pred: &[u8; 64], res: &[i32; 64]) {
    for r in 0..8 {
        let d = &mut dst[pos + r * stride..pos + r * stride + 8];
        let p = &pred[r * 8..r * 8 + 8];
        let q = &res[r * 8..r * 8 + 8];
        for i in 0..8 {
            d[i] = (i32::from(p[i]) + q[i]).clamp(0, 255) as u8;
        }
    }
}

/// Writes predictor + a constant, clamped.
#[inline(always)]
pub(crate) fn put_dc(dst: &mut [u8], stride: usize, pos: usize, pred: &[u8; 64], dc: i32) {
    for r in 0..8 {
        let d = &mut dst[pos + r * stride..pos + r * stride + 8];
        let p = &pred[r * 8..r * 8 + 8];
        for i in 0..8 {
            d[i] = (i32::from(p[i]) + dc).clamp(0, 255) as u8;
        }
    }
}

/// Copies the co-located block of `src`.
#[inline(always)]
pub(crate) fn copy_block(dst: &mut [u8], src: &[u8], stride: usize, pos: usize) {
    for r in 0..8 {
        let o = pos + r * stride;
        dst[o..o + 8].copy_from_slice(&src[o..o + 8]);
    }
}

/// The loop filter response `lflim(R, L)` for R in -256..256, at index R + 256.
pub(crate) fn lflim_table(l: i32) -> [i8; 512] {
    let mut t = [0i8; 512];
    for (i, v) in t.iter_mut().enumerate() {
        let r = i as i32 - 256;
        let f = if r <= -2 * l {
            0
        } else if r <= -l {
            -r - 2 * l
        } else if r < l {
            r
        } else if r < 2 * l {
            -r + 2 * l
        } else {
            0
        };
        *v = f as i8;
    }
    t
}

#[inline(always)]
fn filt(lf: &[i8; 512], p0: u8, p1: u8, p2: u8, p3: u8) -> (u8, u8) {
    let r = (i32::from(p0) - 3 * i32::from(p1) + 3 * i32::from(p2) - i32::from(p3) + 4) >> 3;
    let f = i32::from(lf[(r + 256) as usize]);
    (
        (i32::from(p1) + f).clamp(0, 255) as u8,
        (i32::from(p2) - f).clamp(0, 255) as u8,
    )
}

/// Filters across a vertical edge: pixels `pos .. pos + 4` of 8 rows (7.10.1).
#[inline(always)]
fn filter_h(buf: &mut [u8], stride: usize, pos: usize, lf: &[i8; 512]) {
    for r in 0..8 {
        let o = pos + r * stride;
        let p = &mut buf[o..o + 4];
        let (a, b) = filt(lf, p[0], p[1], p[2], p[3]);
        p[1] = a;
        p[2] = b;
    }
}

/// Filters across a horizontal edge: rows `pos`, +1, +2, +3 of 8 columns (7.10.2).
#[inline(always)]
fn filter_v(buf: &mut [u8], stride: usize, pos: usize, lf: &[i8; 512]) {
    let (head, rest) = buf[pos..].split_at_mut(stride);
    let (r1, rest) = rest.split_at_mut(stride);
    let (r2, r3) = rest.split_at_mut(stride);
    let (r0, r1, r2, r3) = (&head[..8], &mut r1[..8], &mut r2[..8], &r3[..8]);
    for i in 0..8 {
        let (a, b) = filt(lf, r0[i], r1[i], r2[i], r3[i]);
        r1[i] = a;
        r2[i] = b;
    }
}

/// The complete loop filter of one plane (7.10.3); `coded` holds the plane's block flags.
pub(crate) fn loop_filter(buf: &mut [u8], g: &PlaneGeom, coded: &[u8], lf: &[i8; 512]) {
    let s = g.stride;
    for by in 0..g.bh {
        for bx in 0..g.bw {
            let b = by * g.bw + bx;
            if coded[b] == 0 {
                continue;
            }
            let pos = g.origin + by * 8 * s + bx * 8;
            if bx > 0 {
                filter_h(buf, s, pos - 2, lf);
            }
            if by > 0 {
                filter_v(buf, s, pos - 2 * s, lf);
            }
            if bx + 1 < g.bw && coded[b + 1] == 0 {
                filter_h(buf, s, pos + 6, lf);
            }
            if by + 1 < g.bh && coded[b + g.bw] == 0 {
                filter_v(buf, s, pos + 6 * s, lf);
            }
        }
    }
}

/// Repeats the edge pixels of a plane into its padding.
pub(crate) fn extend_borders(buf: &mut [u8], g: &PlaneGeom) {
    let s = g.stride;
    for y in 0..g.h {
        let row = g.origin + y * s;
        let l = buf[row];
        let r = buf[row + g.w - 1];
        buf[row - BORDER..row].fill(l);
        buf[row + g.w..row + g.w + BORDER].fill(r);
    }
    let first = g.origin - BORDER;
    let last = first + (g.h - 1) * s;
    for k in 1..=BORDER {
        buf.copy_within(first..first + s, first - k * s);
        buf.copy_within(last..last + s, last + k * s);
    }
}
