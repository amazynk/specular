//! The in-loop deblocking filter (8.7) over a whole reconstructed picture, macroblocks in
//! raster order, vertical edges before horizontal ones — the order a decoder uses, so the
//! encoder's reference equals the decoder's.

// Block geometry: one index addresses several parallel per-block arrays.
#![allow(clippy::needless_range_loop)]

use std::simd::cmp::{SimdOrd, SimdPartialEq, SimdPartialOrd};
use std::simd::num::{SimdInt, SimdUint};
use std::simd::{Select, Simd};

use crate::mb::MbInfo;
use crate::tables::{ALPHA, BETA, CHROMA_QP, TC0};

/// Boundary strength between 4×4 luma block `pb` of `p` and `qb` of `q` (raster indices).
#[inline]
fn strength(p: &MbInfo, pb: usize, q: &MbInfo, qb: usize, mb_edge: bool) -> u8 {
    if p.is_intra() || q.is_intra() {
        if mb_edge { 4 } else { 3 }
    } else if p.nz[pb] > 0 || q.nz[qb] > 0 {
        2
    } else if motion_differs(p, q) {
        1
    } else {
        0
    }
}

/// Different reference pictures or motion vectors at least one sample apart (8.7.2.1). The
/// two lists always refer to different pictures.
#[inline]
fn motion_differs(p: &MbInfo, q: &MbInfo) -> bool {
    p.pred != q.pred
        || (0..2).any(|l| {
            p.pred & (1 << l) != 0
                && ((p.mv[l][0] - q.mv[l][0]).abs() >= 4 || (p.mv[l][1] - q.mv[l][1]).abs() >= 4)
        })
}

type I16x16 = Simd<i16, 16>;
type U8x16 = Simd<u8, 16>;

#[derive(Clone, Copy)]
struct Thresholds {
    alpha: i32,
    beta: i32,
    tc0: [i32; 3],
}

impl Thresholds {
    fn new(qp: u8) -> Thresholds {
        let i = usize::from(qp.min(51));
        Thresholds {
            alpha: i32::from(ALPHA[i]),
            beta: i32::from(BETA[i]),
            tc0: TC0[i].map(i32::from),
        }
    }
}

/// Filters one line of luma samples across an edge; `q0` is the index of q0, `step` the
/// distance between successive samples across the edge. The scalar reference of
/// [`luma_edge16`].
#[cfg(test)]
fn luma_line(s: &mut [u8], q0: usize, step: usize, bs: u8, t: &Thresholds) {
    let at = |k: isize| q0.wrapping_add_signed(k * step as isize);
    let p = [s[at(-1)], s[at(-2)], s[at(-3)], s[at(-4)]].map(i32::from);
    let q = [s[at(0)], s[at(1)], s[at(2)], s[at(3)]].map(i32::from);
    if (p[0] - q[0]).abs() >= t.alpha
        || (p[1] - p[0]).abs() >= t.beta
        || (q[1] - q[0]).abs() >= t.beta
    {
        return;
    }
    let ap = (p[2] - p[0]).abs();
    let aq = (q[2] - q[0]).abs();
    let c8 = |v: i32| v.clamp(0, 255) as u8;
    if bs < 4 {
        let tc0 = t.tc0[usize::from(bs) - 1];
        let tc = tc0 + i32::from(ap < t.beta) + i32::from(aq < t.beta);
        let delta = ((((q[0] - p[0]) << 2) + (p[1] - q[1]) + 4) >> 3).clamp(-tc, tc);
        s[at(-1)] = c8(p[0] + delta);
        s[at(0)] = c8(q[0] - delta);
        if ap < t.beta {
            s[at(-2)] = (p[1]
                + ((p[2] + ((p[0] + q[0] + 1) >> 1) - (p[1] << 1)) >> 1).clamp(-tc0, tc0))
                as u8;
        }
        if aq < t.beta {
            s[at(1)] = (q[1]
                + ((q[2] + ((p[0] + q[0] + 1) >> 1) - (q[1] << 1)) >> 1).clamp(-tc0, tc0))
                as u8;
        }
    } else {
        let strong = (p[0] - q[0]).abs() < ((t.alpha >> 2) + 2);
        if ap < t.beta && strong {
            s[at(-1)] = ((p[2] + 2 * p[1] + 2 * p[0] + 2 * q[0] + q[1] + 4) >> 3) as u8;
            s[at(-2)] = ((p[2] + p[1] + p[0] + q[0] + 2) >> 2) as u8;
            s[at(-3)] = ((2 * p[3] + 3 * p[2] + p[1] + p[0] + q[0] + 4) >> 3) as u8;
        } else {
            s[at(-1)] = ((2 * p[1] + p[0] + q[1] + 2) >> 2) as u8;
        }
        if aq < t.beta && strong {
            s[at(0)] = ((p[1] + 2 * p[0] + 2 * q[0] + 2 * q[1] + q[2] + 4) >> 3) as u8;
            s[at(1)] = ((p[0] + q[0] + q[1] + q[2] + 2) >> 2) as u8;
            s[at(2)] = ((2 * q[3] + 3 * q[2] + q[1] + q[0] + p[0] + 4) >> 3) as u8;
        } else {
            s[at(0)] = ((2 * q[1] + q[0] + p[1] + 2) >> 2) as u8;
        }
    }
}

/// The scalar reference of [`chroma_edge16`].
#[cfg(test)]
fn chroma_line(s: &mut [u8], q0: usize, step: usize, bs: u8, t: &Thresholds) {
    let p0 = i32::from(s[q0 - step]);
    let p1 = i32::from(s[q0 - 2 * step]);
    let q0v = i32::from(s[q0]);
    let q1 = i32::from(s[q0 + step]);
    if (p0 - q0v).abs() >= t.alpha || (p1 - p0).abs() >= t.beta || (q1 - q0v).abs() >= t.beta {
        return;
    }
    if bs < 4 {
        let tc = t.tc0[usize::from(bs) - 1] + 1;
        let delta = ((((q0v - p0) << 2) + (p1 - q1) + 4) >> 3).clamp(-tc, tc);
        s[q0 - step] = (p0 + delta).clamp(0, 255) as u8;
        s[q0] = (q0v - delta).clamp(0, 255) as u8;
    } else {
        s[q0 - step] = ((2 * p1 + p0 + q1 + 2) >> 2) as u8;
        s[q0] = ((2 * q1 + q0v + p1 + 2) >> 2) as u8;
    }
}

/// Filters sixteen lines across one luma edge at once (8.7.2.3–8.7.2.4): lane `i` of `p[k]`
/// / `q[k]` is sample pk / qk of line `i`, and `bs[i / 4]` its boundary strength. Updates
/// p0..p2 and q0..q2 in place; returns whether any line passed the thresholds.
#[inline(always)]
fn luma_edge16(p: &mut [I16x16; 4], q: &mut [I16x16; 4], bs: [u8; 4], t: &Thresholds) -> bool {
    let bsl = I16x16::from_array(std::array::from_fn(|i| i16::from(bs[i / 4])));
    let alpha = I16x16::splat(t.alpha as i16);
    let beta = I16x16::splat(t.beta as i16);
    let [p0, p1, p2, p3] = *p;
    let [q0, q1, q2, q3] = *q;
    let on = bsl.simd_gt(I16x16::splat(0))
        & (p0 - q0).abs().simd_lt(alpha)
        & (p1 - p0).abs().simd_lt(beta)
        & (q1 - q0).abs().simd_lt(beta);
    if !on.any() {
        return false;
    }
    let ap = (p2 - p0).abs().simd_lt(beta);
    let aq = (q2 - q0).abs().simd_lt(beta);
    let four = bsl.simd_eq(I16x16::splat(4));
    let (two, three) = (I16x16::splat(2), I16x16::splat(3));
    let (mut np, mut nq) = (*p, *q);
    let normal = on & !four;
    if normal.any() {
        let tc0 = I16x16::from_array(std::array::from_fn(|i| match bs[i / 4] {
            b @ 1..=3 => t.tc0[usize::from(b) - 1] as i16,
            _ => 0,
        }));
        let (z, o1) = (I16x16::splat(0), I16x16::splat(1));
        let tc = tc0 + ap.select(o1, z) + aq.select(o1, z);
        let delta = ((((q0 - p0) << two) + (p1 - q1) + I16x16::splat(4)) >> three)
            .simd_max(-tc)
            .simd_min(tc);
        let (lo, hi) = (I16x16::splat(0), I16x16::splat(255));
        np[0] = normal.select((p0 + delta).simd_max(lo).simd_min(hi), np[0]);
        nq[0] = normal.select((q0 - delta).simd_max(lo).simd_min(hi), nq[0]);
        let avg = (p0 + q0 + I16x16::splat(1)) >> I16x16::splat(1);
        let one = I16x16::splat(1);
        let dp = ((p2 + avg - (p1 << one)) >> one)
            .simd_max(-tc0)
            .simd_min(tc0);
        let dq = ((q2 + avg - (q1 << one)) >> one)
            .simd_max(-tc0)
            .simd_min(tc0);
        np[1] = (normal & ap).select(p1 + dp, np[1]);
        nq[1] = (normal & aq).select(q1 + dq, nq[1]);
    }
    let strong = on & four;
    if strong.any() {
        let s = (p0 - q0).abs().simd_lt((alpha >> two) + two);
        let sp = strong & ap & s;
        let sq = strong & aq & s;
        let r4 = I16x16::splat(4);
        let p0s = (p2 + two * p1 + two * p0 + two * q0 + q1 + r4) >> three;
        let p0w = (two * p1 + p0 + q1 + two) >> two;
        np[0] = strong.select(sp.select(p0s, p0w), np[0]);
        np[1] = sp.select((p2 + p1 + p0 + q0 + two) >> two, np[1]);
        np[2] = sp.select((two * p3 + three * p2 + p1 + p0 + q0 + r4) >> three, np[2]);
        let q0s = (p1 + two * p0 + two * q0 + two * q1 + q2 + r4) >> three;
        let q0w = (two * q1 + q0 + p1 + two) >> two;
        nq[0] = strong.select(sq.select(q0s, q0w), nq[0]);
        nq[1] = sq.select((p0 + q0 + q1 + q2 + two) >> two, nq[1]);
        nq[2] = sq.select((two * q3 + three * q2 + q1 + q0 + p0 + r4) >> three, nq[2]);
    }
    *p = np;
    *q = nq;
    true
}

/// Transposes 16 rows of 8 samples, held two rows per vector, into 8 columns of 16 (four
/// perfect shuffles, each rotating the seven bits of (row, column) by one).
#[inline(always)]
fn rows_to_cols(mut v: [U8x16; 8]) -> [U8x16; 8] {
    for _ in 0..4 {
        v = shuffle8(v);
    }
    v
}

/// The inverse of [`rows_to_cols`] (three more shuffles complete the rotation).
#[inline(always)]
fn cols_to_rows(mut v: [U8x16; 8]) -> [U8x16; 8] {
    for _ in 0..3 {
        v = shuffle8(v);
    }
    v
}

#[inline(always)]
fn shuffle8(v: [U8x16; 8]) -> [U8x16; 8] {
    let mut x = [U8x16::splat(0); 8];
    for i in 0..4 {
        (x[2 * i], x[2 * i + 1]) = v[i].interleave(v[i + 4]);
    }
    x
}

#[inline(always)]
fn widen(v: U8x16) -> I16x16 {
    v.cast()
}

/// Filters the vertical luma edge whose q0 column starts at `o` (16 rows, stride `ls`).
#[inline]
fn luma_vertical(y: &mut [u8], o: usize, ls: usize, bs: [u8; 4], t: &Thresholds) {
    let base = o - 4;
    let v: [U8x16; 8] = std::array::from_fn(|i| {
        let a = base + 2 * i * ls;
        let mut r = [0u8; 16];
        r[..8].copy_from_slice(&y[a..a + 8]);
        r[8..].copy_from_slice(&y[a + ls..a + ls + 8]);
        U8x16::from_array(r)
    });
    let c = rows_to_cols(v);
    let mut p = [widen(c[3]), widen(c[2]), widen(c[1]), widen(c[0])];
    let mut q = [widen(c[4]), widen(c[5]), widen(c[6]), widen(c[7])];
    if !luma_edge16(&mut p, &mut q, bs, t) {
        return;
    }
    let n = |v: I16x16| v.cast::<u8>();
    let r = cols_to_rows([
        c[0],
        n(p[2]),
        n(p[1]),
        n(p[0]),
        n(q[0]),
        n(q[1]),
        n(q[2]),
        c[7],
    ]);
    for (i, v) in r.into_iter().enumerate() {
        let a = base + 2 * i * ls;
        let v = v.to_array();
        y[a..a + 8].copy_from_slice(&v[..8]);
        y[a + ls..a + ls + 8].copy_from_slice(&v[8..]);
    }
}

/// Filters the horizontal luma edge whose q0 row starts at `o` (16 columns, stride `ls`).
#[inline]
fn luma_horizontal(y: &mut [u8], o: usize, ls: usize, bs: [u8; 4], t: &Thresholds) {
    let row = |k: isize| o.wrapping_add_signed(k * ls as isize);
    let ld = |y: &[u8], k: isize| widen(U8x16::from_slice(&y[row(k)..row(k) + 16]));
    let mut p = [ld(y, -1), ld(y, -2), ld(y, -3), ld(y, -4)];
    let mut q = [ld(y, 0), ld(y, 1), ld(y, 2), ld(y, 3)];
    if !luma_edge16(&mut p, &mut q, bs, t) {
        return;
    }
    for (k, v) in [
        (-3, p[2]),
        (-2, p[1]),
        (-1, p[0]),
        (0, q[0]),
        (1, q[1]),
        (2, q[2]),
    ] {
        v.cast::<u8>().copy_to_slice(&mut y[row(k)..row(k) + 16]);
    }
}

/// Filters sixteen lines across one chroma edge at once (8.7.2.3–8.7.2.4, chroma): lane `i`
/// of `p[k]` / `q[k]` is sample pk / qk of line `i`, and `bs[i]` its boundary strength.
/// Updates p0 and q0 in place; returns whether any line passed the thresholds.
#[inline(always)]
fn chroma_edge16(p: &mut [I16x16; 2], q: &mut [I16x16; 2], bs: I16x16, t: &Thresholds) -> bool {
    let alpha = I16x16::splat(t.alpha as i16);
    let beta = I16x16::splat(t.beta as i16);
    let [p0, p1] = *p;
    let [q0, q1] = *q;
    let on = bs.simd_gt(I16x16::splat(0))
        & (p0 - q0).abs().simd_lt(alpha)
        & (p1 - p0).abs().simd_lt(beta)
        & (q1 - q0).abs().simd_lt(beta);
    if !on.any() {
        return false;
    }
    let four = bs.simd_eq(I16x16::splat(4));
    let two = I16x16::splat(2);
    let normal = on & !four;
    if normal.any() {
        let tc = I16x16::from_array(std::array::from_fn(|i| match bs[i] {
            b @ 1..=3 => t.tc0[b as usize - 1] as i16 + 1,
            _ => 0,
        }));
        let delta = ((((q0 - p0) << two) + (p1 - q1) + I16x16::splat(4)) >> I16x16::splat(3))
            .simd_max(-tc)
            .simd_min(tc);
        let (lo, hi) = (I16x16::splat(0), I16x16::splat(255));
        p[0] = normal.select((p0 + delta).simd_max(lo).simd_min(hi), p0);
        q[0] = normal.select((q0 - delta).simd_max(lo).simd_min(hi), q0);
    }
    let strong = on & four;
    if strong.any() {
        p[0] = strong.select((two * p1 + p0 + q1 + two) >> two, p[0]);
        q[0] = strong.select((two * q1 + q0 + p1 + two) >> two, q[0]);
    }
    true
}

/// Filters vertical chroma edge `ce` of both planes (`u` in lanes 0..8, `v` in 8..16).
#[inline]
fn chroma_vertical(planes: [&mut [u8]; 2], o: usize, cs: usize, bs: [u8; 4], t: &Thresholds) {
    let at = |i: usize, k: usize| o + (i % 8) * cs + k - 2;
    let col =
        |k: usize| I16x16::from_array(std::array::from_fn(|i| i16::from(planes[i / 8][at(i, k)])));
    let mut p = [col(1), col(0)];
    let mut q = [col(2), col(3)];
    let bsl = I16x16::from_array(std::array::from_fn(|i| i16::from(bs[(i % 8) / 2])));
    if !chroma_edge16(&mut p, &mut q, bsl, t) {
        return;
    }
    let (p0, q0) = (p[0].cast::<u8>().to_array(), q[0].cast::<u8>().to_array());
    for (n, plane) in planes.into_iter().enumerate() {
        for r in 0..8 {
            let i = n * 8 + r;
            plane[at(i, 1)] = p0[i];
            plane[at(i, 2)] = q0[i];
        }
    }
}

/// Filters horizontal chroma edge `ce` of both planes (`u` in lanes 0..8, `v` in 8..16).
#[inline]
fn chroma_horizontal(mut planes: [&mut [u8]; 2], o: usize, cs: usize, bs: [u8; 4], t: &Thresholds) {
    let row = |k: isize| o.wrapping_add_signed(k * cs as isize);
    let ld = |k: isize| {
        let a = row(k);
        let mut r = [0u8; 16];
        r[..8].copy_from_slice(&planes[0][a..a + 8]);
        r[8..].copy_from_slice(&planes[1][a..a + 8]);
        widen(U8x16::from_array(r))
    };
    let mut p = [ld(-1), ld(-2)];
    let mut q = [ld(0), ld(1)];
    let bsl = I16x16::from_array(std::array::from_fn(|i| i16::from(bs[(i % 8) / 2])));
    if !chroma_edge16(&mut p, &mut q, bsl, t) {
        return;
    }
    for (k, v) in [(-1, p[0]), (0, q[0])] {
        let v = v.cast::<u8>().to_array();
        let a = row(k);
        for (n, plane) in planes.iter_mut().enumerate() {
            plane[a..a + 8].copy_from_slice(&v[n * 8..n * 8 + 8]);
        }
    }
}

/// Deblocks the picture in place. All macroblocks share `qp` (no mb_qp_delta is coded).
pub fn deblock(
    y: &mut [u8],
    u: &mut [u8],
    v: &mut [u8],
    mb_w: usize,
    mb_h: usize,
    info: &[MbInfo],
    qp: u8,
) {
    let tl = Thresholds::new(qp);
    let tc = Thresholds::new(CHROMA_QP[usize::from(qp)]);
    if tl.alpha == 0 && tc.alpha == 0 {
        return; // nothing can pass the thresholds
    }
    let ls = mb_w * 16;
    let cs = mb_w * 8;
    for my in 0..mb_h {
        for mx in 0..mb_w {
            let q = &info[my * mb_w + mx];
            if q.is_skip_like() {
                // Skipped / uncoded inter blocks with no neighbour difference are common:
                // cheap early exit when every edge has bS 0.
                let left = (mx > 0).then(|| &info[my * mb_w + mx - 1]);
                let top = (my > 0).then(|| &info[(my - 1) * mb_w + mx]);
                let quiet = |n: Option<&MbInfo>| {
                    n.is_none_or(|n| {
                        !n.is_intra() && n.nz.iter().all(|&c| c == 0) && !motion_differs(n, q)
                    })
                };
                if quiet(left) && quiet(top) {
                    continue;
                }
            }
            // bS for [vertical, horizontal][edge][segment].
            let mut bs = [[[0u8; 4]; 4]; 2];
            for e in 0..4 {
                for k in 0..4 {
                    // Vertical edge e, rows 4k..4k+3.
                    if e > 0 {
                        bs[0][e][k] = strength(q, k * 4 + e - 1, q, k * 4 + e, false);
                    } else if mx > 0 {
                        bs[0][e][k] =
                            strength(&info[my * mb_w + mx - 1], k * 4 + 3, q, k * 4, true);
                    }
                    // Horizontal edge e, columns 4k..4k+3.
                    if e > 0 {
                        bs[1][e][k] = strength(q, (e - 1) * 4 + k, q, e * 4 + k, false);
                    } else if my > 0 {
                        bs[1][e][k] = strength(&info[(my - 1) * mb_w + mx], 12 + k, q, k, true);
                    }
                }
            }
            let x0 = mx * 16;
            let y0 = my * 16;
            if tl.alpha > 0 {
                for e in 0..4 {
                    if q.t8 && e % 2 == 1 {
                        continue; // no transform edge inside an 8×8 block
                    }
                    if bs[0][e] != [0; 4] {
                        luma_vertical(y, y0 * ls + x0 + e * 4, ls, bs[0][e], &tl);
                    }
                }
                for e in 0..4 {
                    if q.t8 && e % 2 == 1 {
                        continue;
                    }
                    if bs[1][e] != [0; 4] {
                        luma_horizontal(y, (y0 + e * 4) * ls + x0, ls, bs[1][e], &tl);
                    }
                }
            }
            if tc.alpha > 0 {
                let cx0 = mx * 8;
                let cy0 = my * 8;
                for ce in 0..2 {
                    let b = bs[0][ce * 2];
                    if b != [0; 4] {
                        let o = cy0 * cs + cx0 + ce * 4;
                        chroma_vertical([&mut *u, &mut *v], o, cs, b, &tc);
                    }
                }
                for ce in 0..2 {
                    let b = bs[1][ce * 2];
                    if b != [0; 4] {
                        let o = (cy0 + ce * 4) * cs + cx0;
                        chroma_horizontal([&mut *u, &mut *v], o, cs, b, &tc);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The vector luma edge filters equal the scalar line filter, line by line.
    #[test]
    fn vector_luma_edges_match_scalar() {
        let mut seed = 0x2545_f491u32;
        let mut rnd = move || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed
        };
        const LS: usize = 24;
        for it in 0..20000 {
            let t = Thresholds::new(16 + (it % 36) as u8);
            // Smooth-ish content so the thresholds pass often.
            let base = rnd() as u8;
            let spread = [4u32, 16, 64, 256][it % 4];
            let img: [u8; LS * 24] =
                std::array::from_fn(|_| base.wrapping_add((rnd() % spread) as u8));
            let bs: [u8; 4] = if it % 5 == 0 {
                [4; 4]
            } else {
                std::array::from_fn(|_| (rnd() % 4) as u8)
            };
            let (mut a, mut b) = (img, img);
            let o = 4 * LS + 4;
            luma_vertical(&mut a, o, LS, bs, &t);
            for r in 0..16 {
                if bs[r / 4] > 0 {
                    luma_line(&mut b, o + r * LS, 1, bs[r / 4], &t);
                }
            }
            assert_eq!(a, b);
            let (mut a, mut b) = (img, img);
            luma_horizontal(&mut a, o, LS, bs, &t);
            for c in 0..16 {
                if bs[c / 4] > 0 {
                    luma_line(&mut b, o + c, LS, bs[c / 4], &t);
                }
            }
            assert_eq!(a, b);
        }
    }

    /// The vector chroma edge filters equal the scalar line filter, line by line.
    #[test]
    fn vector_chroma_edges_match_scalar() {
        let mut seed = 0x1f12_3bb5u32;
        let mut rnd = move || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed
        };
        const CS: usize = 16;
        for it in 0..20000 {
            let t = Thresholds::new(16 + (it % 36) as u8);
            let base = rnd() as u8;
            let spread = [4u32, 16, 64, 256][it % 4];
            let mut px = || -> [u8; CS * 14] {
                std::array::from_fn(|_| base.wrapping_add((rnd() % spread) as u8))
            };
            let (u, v) = (px(), px());
            let bs: [u8; 4] = if it % 5 == 0 {
                [4; 4]
            } else {
                std::array::from_fn(|_| (rnd() % 4) as u8)
            };
            let o = 3 * CS + 4;
            let (mut a, mut b, mut c, mut d) = (u, v, u, v);
            chroma_vertical([&mut a, &mut b], o, CS, bs, &t);
            for pl in [&mut c, &mut d] {
                for r in 0..8 {
                    if bs[r / 2] > 0 {
                        chroma_line(pl, o + r * CS, 1, bs[r / 2], &t);
                    }
                }
            }
            assert_eq!((a, b), (c, d));
            let (mut a, mut b, mut c, mut d) = (u, v, u, v);
            chroma_horizontal([&mut a, &mut b], o, CS, bs, &t);
            for pl in [&mut c, &mut d] {
                for x in 0..8 {
                    if bs[x / 2] > 0 {
                        chroma_line(pl, o + x, CS, bs[x / 2], &t);
                    }
                }
            }
            assert_eq!((a, b), (c, d));
        }
    }
}
