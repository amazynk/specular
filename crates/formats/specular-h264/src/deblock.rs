//! The deblocking filter (8.7), one macroblock row at a time.

use crate::mb::MbInfo;
use crate::pixel::{Format, Pix};
use crate::spec_tables::{ALPHA, BETA, TC0};
use std::simd::{
    Simd,
    num::{SimdInt, SimdUint},
    simd_swizzle,
};

type U16x8 = Simd<u16, 8>;

/// Transposes the 8x8 matrix `m` (rows to columns): three perfect shuffles of the rows,
/// each rotating the six bits of a sample's index by one.
#[inline(always)]
fn transpose8(m: [U16x8; 8]) -> [U16x8; 8] {
    let mut m = m;
    for _ in 0..3 {
        let mut n = m;
        for k in 0..4 {
            (n[2 * k], n[2 * k + 1]) = m[k].interleave(m[k + 4]);
        }
        m = n;
    }
    m
}

/// Loads samples `o - 4..o + 4` of the 16 lines `o = off + i * along` into the lanes of
/// `s` (lane `i` of `s[r]` is sample `r` of line `i`), through two 8x8 transposes.
#[inline(always)]
fn load_across<P: Pix>(pix: &[P], off: usize, along: usize, s: &mut [[i32; 16]; 8]) {
    let mut m = [[U16x8::splat(0); 8]; 2];
    for i in 0..16 {
        let o = off + i * along;
        let l: &[P; 8] = pix[o - 4..o + 4].try_into().unwrap();
        m[i / 8][i % 8] = P::wide8(l);
    }
    let (a, b) = (transpose8(m[0]), transpose8(m[1]));
    for r in 0..8 {
        let v: Simd<u16, 16> = simd_swizzle!(
            a[r],
            b[r],
            [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]
        );
        s[r] = v.cast::<i32>().to_array();
    }
}

/// Stores the lanes of `s` back as `load_across` loaded them (p3 and q3 unchanged).
#[inline(always)]
fn store_across<P: Pix>(pix: &mut [P], off: usize, along: usize, s: &[[i32; 16]; 8]) {
    let mut m = [[U16x8::splat(0); 8]; 2];
    for r in 0..8 {
        let v = Simd::<i32, 16>::from_array(s[r]).cast::<u16>();
        m[0][r] = simd_swizzle!(v, [0, 1, 2, 3, 4, 5, 6, 7]);
        m[1][r] = simd_swizzle!(v, [8, 9, 10, 11, 12, 13, 14, 15]);
    }
    for h in 0..2 {
        let m = transpose8(m[h]);
        for i in 0..8 {
            let o = off + (8 * h + i) * along;
            let l: &mut [P; 8] = (&mut pix[o - 4..o + 4]).try_into().unwrap();
            *l = P::narrow8(m[i]);
        }
    }
}

/// Filter controls of one slice.
#[derive(Clone, Copy, Debug)]
pub struct DbParams {
    pub idc: u32,
    pub alpha: i32,
    pub beta: i32,
}

pub const DB_OFF: DbParams = DbParams {
    idc: 1,
    alpha: 0,
    beta: 0,
};

/// Boundary strength between 4x4 luma block `bp` of `p` and `bq` of `q` (raster indices),
/// across a vertical (`vert`) or horizontal edge.
#[inline]
pub fn strength(p: &MbInfo, bp: usize, q: &MbInfo, bq: usize, mb_edge: bool, vert: bool) -> u8 {
    if p.intra() || q.intra() {
        // Horizontal macroblock edges next to a field macroblock get 3.
        return if mb_edge && (vert || !(p.field || q.field)) {
            4
        } else {
            3
        };
    }
    if (p.nzmask >> bp) & 1 != 0 || (q.nzmask >> bq) & 1 != 0 {
        return 2;
    }
    if p.field != q.field {
        // mixedModeEdgeFlag.
        return 1;
    }
    // Field vectors count in field lines: 4 quarter frame samples are 2 field ones.
    let ylim = if q.field { 2 } else { 4 };
    let (p8, q8) = (
        (bp >> 3) * 2 + ((bp & 3) >> 1),
        (bq >> 3) * 2 + ((bq & 3) >> 1),
    );
    let rp = [p.ref_id[0][p8], p.ref_id[1][p8]];
    let rq = [q.ref_id[0][q8], q.ref_id[1][q8]];
    let differ = |a: [i16; 2], b: [i16; 2]| (a[0] - b[0]).abs() >= 4 || (a[1] - b[1]).abs() >= ylim;
    let (mp0, mp1) = (p.mv[0][bp], p.mv[1][bp]);
    let (mq0, mq1) = (q.mv[0][bq], q.mv[1][bq]);
    let same_set = (rp[0] == rq[0] && rp[1] == rq[1]) || (rp[0] == rq[1] && rp[1] == rq[0]);
    if !same_set {
        return 1;
    }
    let n = (rp[0] != 0) as u32 + (rp[1] != 0) as u32;
    if n == 1 {
        let a = if rp[0] != 0 { mp0 } else { mp1 };
        let b = if rq[0] != 0 { mq0 } else { mq1 };
        return differ(a, b) as u8;
    }
    if n == 0 {
        return 0;
    }
    if rp[0] != rp[1] {
        // Two different pictures: compare the vectors referring to the same picture.
        let bad = if rp[0] == rq[0] {
            differ(mp0, mq0) || differ(mp1, mq1)
        } else {
            differ(mp0, mq1) || differ(mp1, mq0)
        };
        return bad as u8;
    }
    // Both vectors refer to the same picture: either pairing may match.
    let straight = differ(mp0, mq0) || differ(mp1, mq1);
    let cross = differ(mp0, mq1) || differ(mp1, mq0);
    (straight && cross) as u8
}

#[inline(always)]
fn clip3(lo: i32, hi: i32, v: i32) -> i32 {
    v.clamp(lo, hi)
}

/// Sample depth of the plane being filtered: thresholds scale by `1 << sh` (BitDepth - 8),
/// results clip to `max`.
#[derive(Clone, Copy)]
struct Dp {
    sh: u32,
    max: i32,
}

/// Filters one edge segment of `n` sample lines. `off` is q0 of the first line, `step`
/// crosses the edge and `along` moves to the next line.
#[allow(clippy::too_many_arguments)]
#[inline]
fn filter<P: Pix>(
    pix: &mut [P],
    off: usize,
    step: usize,
    along: usize,
    n: usize,
    bs: u8,
    qpav: i32,
    p: &DbParams,
    chroma: bool,
    d: Dp,
) {
    let ia = clip3(0, 51, qpav + p.alpha) as usize;
    let ib = clip3(0, 51, qpav + p.beta) as usize;
    let alpha = (ALPHA[ia] as i32) << d.sh;
    let beta = (BETA[ib] as i32) << d.sh;
    if alpha == 0 || beta == 0 {
        return;
    }
    let tc0 = if bs < 4 {
        (TC0[ia][bs as usize - 1] as i32) << d.sh
    } else {
        0
    };
    let clip = |v: i32| P::clip(v, d.max);
    for k in 0..n {
        let o = off + k * along;
        let p0 = pix[o - step].i();
        let q0 = pix[o].i();
        let p1 = pix[o - 2 * step].i();
        let q1 = pix[o + step].i();
        if (p0 - q0).abs() >= alpha || (p1 - p0).abs() >= beta || (q1 - q0).abs() >= beta {
            continue;
        }
        if chroma {
            if bs < 4 {
                let tc = tc0 + 1;
                let dd = clip3(-tc, tc, (((q0 - p0) << 2) + (p1 - q1) + 4) >> 3);
                pix[o - step] = clip(p0 + dd);
                pix[o] = clip(q0 - dd);
            } else {
                pix[o - step] = P::of((2 * p1 + p0 + q1 + 2) >> 2);
                pix[o] = P::of((2 * q1 + q0 + p1 + 2) >> 2);
            }
            continue;
        }
        let p2 = pix[o - 3 * step].i();
        let q2 = pix[o + 2 * step].i();
        let ap = (p2 - p0).abs() < beta;
        let aq = (q2 - q0).abs() < beta;
        if bs < 4 {
            let tc = tc0 + ap as i32 + aq as i32;
            let dd = clip3(-tc, tc, (((q0 - p0) << 2) + (p1 - q1) + 4) >> 3);
            pix[o - step] = clip(p0 + dd);
            pix[o] = clip(q0 - dd);
            if ap {
                pix[o - 2 * step] =
                    P::of(p1 + clip3(-tc0, tc0, (p2 + ((p0 + q0 + 1) >> 1) - (p1 << 1)) >> 1));
            }
            if aq {
                pix[o + step] =
                    P::of(q1 + clip3(-tc0, tc0, (q2 + ((p0 + q0 + 1) >> 1) - (q1 << 1)) >> 1));
            }
        } else {
            let strong = (p0 - q0).abs() < ((alpha >> 2) + 2);
            if ap && strong {
                let p3 = pix[o - 4 * step].i();
                pix[o - step] = P::of((p2 + 2 * p1 + 2 * p0 + 2 * q0 + q1 + 4) >> 3);
                pix[o - 2 * step] = P::of((p2 + p1 + p0 + q0 + 2) >> 2);
                pix[o - 3 * step] = P::of((2 * p3 + 3 * p2 + p1 + p0 + q0 + 4) >> 3);
            } else {
                pix[o - step] = P::of((2 * p1 + p0 + q1 + 2) >> 2);
            }
            if aq && strong {
                let q3 = pix[o + 3 * step].i();
                pix[o] = P::of((p1 + 2 * p0 + 2 * q0 + 2 * q1 + q2 + 4) >> 3);
                pix[o + step] = P::of((p0 + q0 + q1 + q2 + 2) >> 2);
                pix[o + 2 * step] = P::of((2 * q3 + 3 * q2 + q1 + q0 + p0 + 4) >> 3);
            } else {
                pix[o] = P::of((2 * q1 + q0 + p1 + 2) >> 2);
            }
        }
    }
}

/// The luma filter of `filter` on 16 lines at once, without branches so that it
/// vectorises: lanes of `s` hold p3, p2, p1, p0, q0, q1, q2 and q3 of each line, the lines
/// of segment k have strength `bs[k]` (lines left alone keep their samples).
#[inline(always)]
fn luma16(s: &mut [[i32; 16]; 8], bs: [u8; 4], tc0: [i32; 4], alpha: i32, beta: i32, max: i32) {
    let strong_lim = (alpha >> 2) + 2;
    for i in 0..16 {
        let (b, t0) = (bs[i / 4], tc0[i / 4]);
        let [p3, p2, p1, p0, q0, q1, q2, q3] = [0, 1, 2, 3, 4, 5, 6, 7].map(|r| s[r][i]);
        let filt =
            b != 0 && (p0 - q0).abs() < alpha && (p1 - p0).abs() < beta && (q1 - q0).abs() < beta;
        let ap = (p2 - p0).abs() < beta;
        let aq = (q2 - q0).abs() < beta;
        // bS < 4.
        let tc = t0 + ap as i32 + aq as i32;
        let dd = ((((q0 - p0) << 2) + (p1 - q1) + 4) >> 3).max(-tc).min(tc);
        let m = (p0 + q0 + 1) >> 1;
        let np0 = (p0 + dd).max(0).min(max);
        let nq0 = (q0 - dd).max(0).min(max);
        let np1 = if ap {
            p1 + ((p2 + m - (p1 << 1)) >> 1).max(-t0).min(t0)
        } else {
            p1
        };
        let nq1 = if aq {
            q1 + ((q2 + m - (q1 << 1)) >> 1).max(-t0).min(t0)
        } else {
            q1
        };
        // bS 4.
        let strong = (p0 - q0).abs() < strong_lim;
        let (sp, sq) = (ap && strong, aq && strong);
        let sp0 = if sp {
            (p2 + 2 * p1 + 2 * p0 + 2 * q0 + q1 + 4) >> 3
        } else {
            (2 * p1 + p0 + q1 + 2) >> 2
        };
        let sp1 = if sp { (p2 + p1 + p0 + q0 + 2) >> 2 } else { p1 };
        let sp2 = if sp {
            (2 * p3 + 3 * p2 + p1 + p0 + q0 + 4) >> 3
        } else {
            p2
        };
        let sq0 = if sq {
            (p1 + 2 * p0 + 2 * q0 + 2 * q1 + q2 + 4) >> 3
        } else {
            (2 * q1 + q0 + p1 + 2) >> 2
        };
        let sq1 = if sq { (p0 + q0 + q1 + q2 + 2) >> 2 } else { q1 };
        let sq2 = if sq {
            (2 * q3 + 3 * q2 + q1 + q0 + p0 + 4) >> 3
        } else {
            q2
        };
        let s4 = b == 4;
        let pick = |v4: i32, v: i32, old: i32| {
            if !filt {
                old
            } else if s4 {
                v4
            } else {
                v
            }
        };
        s[1][i] = pick(sp2, p2, p2);
        s[2][i] = pick(sp1, np1, p1);
        s[3][i] = pick(sp0, np0, p0);
        s[4][i] = pick(sq0, nq0, q0);
        s[5][i] = pick(sq1, nq1, q1);
        s[6][i] = pick(sq2, q2, q2);
    }
}

/// Filters a whole luma edge of a macroblock (16 lines, segment k of strength `bs[k]`) like
/// `filter`: `off` is q0 of the first line, `step` crosses the edge and `along` moves to
/// the next line (1: a horizontal edge).
#[allow(clippy::too_many_arguments)]
#[inline(always)]
fn luma_edge<P: Pix>(
    pix: &mut [P],
    off: usize,
    step: usize,
    along: usize,
    bs: [u8; 4],
    qpav: i32,
    p: &DbParams,
    d: Dp,
) {
    if bs == [0; 4] {
        return;
    }
    let ia = clip3(0, 51, qpav + p.alpha) as usize;
    let ib = clip3(0, 51, qpav + p.beta) as usize;
    let alpha = (ALPHA[ia] as i32) << d.sh;
    let beta = (BETA[ib] as i32) << d.sh;
    if alpha == 0 || beta == 0 {
        return;
    }
    let tc0 = bs.map(|b| match b {
        1..=3 => (TC0[ia][b as usize - 1] as i32) << d.sh,
        _ => 0,
    });
    // p3 and q3 are read only by bS 4 (they may lie outside the picture otherwise).
    let outer = bs.contains(&4);
    let mut s = [[0i32; 16]; 8];
    if along == 1 {
        for (r, lane) in s.iter_mut().enumerate() {
            if (r == 0 || r == 7) && !outer {
                continue;
            }
            let b = off + r * step - 4 * step;
            for (v, x) in lane.iter_mut().zip(&pix[b..b + 16]) {
                *v = x.i();
            }
        }
        luma16(&mut s, bs, tc0, alpha, beta, d.max);
        for (r, lane) in s.iter().enumerate().take(7).skip(1) {
            let b = off + r * step - 4 * step;
            for (x, v) in pix[b..b + 16].iter_mut().zip(lane) {
                *x = P::of(*v);
            }
        }
    } else {
        // A vertical edge: p3..q3 of a line are 8 adjacent samples, all inside the picture
        // (the edge has a macroblock on its left, and q3 is the block's last column), so
        // each line is moved as one fixed-size array and transposed into the lanes.
        load_across(pix, off, along, &mut s);
        luma16(&mut s, bs, tc0, alpha, beta, d.max);
        store_across(pix, off, along, &s);
    }
}

/// Filters a whole chroma edge of a macroblock like `filter`: `N` lines in four segments
/// of strength `bs[k]` (8 lines across a horizontal edge or a 4:2:0 vertical one, 16 for a
/// 4:2:2 vertical one), branch-free over the lines so that it vectorises.
#[allow(clippy::too_many_arguments)]
#[inline(always)]
fn chroma_edge<P: Pix, const N: usize>(
    pix: &mut [P],
    off: usize,
    step: usize,
    along: usize,
    bs: [u8; 4],
    qpav: i32,
    p: &DbParams,
    d: Dp,
) {
    if bs == [0; 4] {
        return;
    }
    let ia = clip3(0, 51, qpav + p.alpha) as usize;
    let ib = clip3(0, 51, qpav + p.beta) as usize;
    let alpha = (ALPHA[ia] as i32) << d.sh;
    let beta = (BETA[ib] as i32) << d.sh;
    if alpha == 0 || beta == 0 {
        return;
    }
    let tc = bs.map(|b| match b {
        1..=3 => ((TC0[ia][b as usize - 1] as i32) << d.sh) + 1,
        _ => 0,
    });
    let mut s = [[0i32; N]; 4];
    if along == 1 {
        for (r, lane) in s.iter_mut().enumerate() {
            let b = off + r * step - 2 * step;
            for (v, x) in lane.iter_mut().zip(&pix[b..b + N]) {
                *v = x.i();
            }
        }
    } else {
        for i in 0..N {
            let o = off + i * along;
            let l: &[P; 4] = pix[o - 2..o + 2].try_into().unwrap();
            for r in 0..4 {
                s[r][i] = l[r].i();
            }
        }
    }
    let seg = N / 4;
    for i in 0..N {
        let (b, tc) = (bs[i / seg], tc[i / seg]);
        let [p1, p0, q0, q1] = [s[0][i], s[1][i], s[2][i], s[3][i]];
        let filt =
            b != 0 && (p0 - q0).abs() < alpha && (p1 - p0).abs() < beta && (q1 - q0).abs() < beta;
        let dd = ((((q0 - p0) << 2) + (p1 - q1) + 4) >> 3).max(-tc).min(tc);
        let (np0, nq0) = if b == 4 {
            ((2 * p1 + p0 + q1 + 2) >> 2, (2 * q1 + q0 + p1 + 2) >> 2)
        } else {
            ((p0 + dd).max(0).min(d.max), (q0 - dd).max(0).min(d.max))
        };
        s[1][i] = if filt { np0 } else { p0 };
        s[2][i] = if filt { nq0 } else { q0 };
    }
    if along == 1 {
        for r in 1..3 {
            let b = off + r * step - 2 * step;
            for (x, v) in pix[b..b + N].iter_mut().zip(&s[r]) {
                *x = P::of(*v);
            }
        }
    } else {
        for i in 0..N {
            let o = off + i * along;
            let l: &mut [P; 2] = (&mut pix[o - 1..o + 1]).try_into().unwrap();
            *l = [P::of(s[1][i]), P::of(s[2][i])];
        }
    }
}

/// Where a macroblock's samples sit: luma and chroma offsets and line strides (twice the
/// picture's for a field macroblock of an MBAFF frame).
#[derive(Clone, Copy)]
struct Geo {
    lo: usize,
    lst: usize,
    co: usize,
    cst: usize,
}

/// A plane filtered with the luma filter: samples, the macroblock's offset and line stride,
/// the picture's line stride, the filter depth and the plane (0 = Y, 1 = Cb, 2 = Cr).
type PlaneEdges<'a, P> = (&'a mut [P], usize, usize, usize, Dp, usize);

/// Luma and chroma filter depths of a picture format.
fn depths(fmt: Format) -> [Dp; 2] {
    [0, 1].map(|c| Dp {
        sh: fmt.depth[c] as u32 - 8,
        max: fmt.max(c),
    })
}

/// The macroblock(s) above the top edge.
#[derive(Clone, Copy)]
enum Top<'a> {
    No,
    One(&'a MbInfo),
    /// A frame macroblock below a field pair: the edge is filtered once per field, against
    /// the field macroblock of that parity.
    Pair(&'a MbInfo, &'a MbInfo),
}

/// Filters the edges of macroblock `q`: the left edge against `left` (a macroblock of the
/// same frame/field structure; mixed left edges are filtered beforehand), the internal
/// edges, and the top edge against `top`.
#[allow(clippy::too_many_arguments)]
#[inline(always)]
fn filter_mb<P: Pix>(
    y: &mut [P],
    cb: &mut [P],
    cr: &mut [P],
    q: &MbInfo,
    p: &DbParams,
    g: Geo,
    ls: usize,
    cs: usize,
    left: Option<&MbInfo>,
    top: Top,
    fmt: Format,
) {
    let Geo { lo, lst, co, cst } = g;
    let [dl, dc] = depths(fmt);
    // 4:2:2: chroma is as tall as luma, and its horizontal edges 1 and 3 (chroma rows 4
    // and 12) are filtered even in a macroblock with the 8x8 transform.
    let c422 = fmt.cshift() == 0;
    let qcq = [q.qpc[0] as i32, q.qpc[1] as i32];
    // bS[edge][segment]; the second field of a Pair top edge uses `bst`.
    let mut bsv = [[0u8; 4]; 4];
    let mut bsh = [[0u8; 4]; 4];
    let mut bst = [0u8; 4];
    let step = if q.t8 { 2 } else { 1 };
    // An inter macroblock with one motion for all its blocks: inside it only coefficients
    // make an edge strong (bS 2), everything else gives 0.
    let uniform = !q.intra()
        && (0..2).all(|l| {
            q.mv[l].iter().all(|&m| m == q.mv[l][0])
                && q.ref_id[l].iter().all(|&r| r == q.ref_id[l][0])
        });
    let inner = |bp: usize, bq: usize, vert: bool| {
        if uniform {
            2 * ((q.nzmask >> bp | q.nzmask >> bq) & 1) as u8
        } else {
            strength(q, bp, q, bq, false, vert)
        }
    };
    for e in 0..4 {
        let luma_edge = e % step == 0;
        if !luma_edge && !c422 {
            continue;
        }
        for k in 0..4 {
            if luma_edge {
                bsv[e][k] = if e == 0 {
                    match left {
                        Some(lm) => strength(lm, k * 4 + 3, q, k * 4, true, true),
                        None => 0,
                    }
                } else {
                    inner(k * 4 + e - 1, k * 4 + e, true)
                };
            }
            bsh[e][k] = if e == 0 {
                match top {
                    Top::One(tm) => strength(tm, 12 + k, q, k, true, false),
                    Top::Pair(t0, t1) => {
                        bst[k] = strength(t1, 12 + k, q, k, true, false);
                        strength(t0, 12 + k, q, k, true, false)
                    }
                    Top::No => 0,
                }
            } else {
                inner((e - 1) * 4 + k, e * 4 + k, false)
            };
        }
    }
    let avg = |a: i8, b: i32| (a as i32 + b + 1) >> 1;
    // Luma vertical edges, then horizontal; 4:4:4 chroma the same way (8.7: its edges are
    // the luma ones and chromaStyleFilteringFlag is 0), with QP_C of each component.
    let c444 = fmt.cfi == 3;
    let planes: [PlaneEdges<P>; 3] = [
        (&mut *y, lo, lst, ls, dl, 0),
        (&mut *cb, co, cst, cs, dc, 1),
        (&mut *cr, co, cst, cs, dc, 2),
    ];
    for (plane, o0, st, ps, dp, c) in planes.into_iter().take(if c444 { 3 } else { 1 }) {
        let qp_of = |m: &MbInfo| if c == 0 { m.qp } else { m.qpc[c - 1] };
        let qy = qp_of(q) as i32;
        for e in (0..4).step_by(step) {
            let qpav = if e == 0 {
                match left {
                    Some(lm) => avg(qp_of(lm), qy),
                    None => continue,
                }
            } else {
                qy
            };
            luma_edge(plane, o0 + e * 4, 1, st, bsv[e], qpav, p, dp);
        }
        for e in (0..4).step_by(step) {
            if e == 0 {
                match top {
                    Top::No => {}
                    Top::One(tm) => {
                        luma_edge(plane, o0, st, 1, bsh[0], avg(qp_of(tm), qy), p, dp);
                    }
                    Top::Pair(t0, t1) => {
                        for (j, (tm, bs)) in [(t0, bsh[0]), (t1, bst)].into_iter().enumerate() {
                            let qpav = avg(qp_of(tm), qy);
                            luma_edge(plane, o0 + j * ps, 2 * ps, 1, bs, qpav, p, dp);
                        }
                    }
                }
                continue;
            }
            luma_edge(plane, o0 + e * 4 * st, st, 1, bsh[e], qy, p, dp);
        }
    }
    if c444 {
        return;
    }
    // Chroma: vertical edges 0 and 2 of the luma grid (chroma 0 and 4), each segment of 2
    // (4:2:0) or 4 (4:2:2) lines; horizontal edges at chroma rows 0 and 4 (4:2:0, luma
    // edges 0 and 2) or 0, 4, 8 and 12 (4:2:2, luma edges 0..4).
    let sl = if c422 { 4 } else { 2 };
    let hedges: &[usize] = if c422 { &[0, 1, 2, 3] } else { &[0, 2] };
    for (ci, plane) in [&mut *cb, &mut *cr].into_iter().enumerate() {
        for e in [0usize, 2] {
            let qpav = if e == 0 {
                match left {
                    Some(lm) => avg(lm.qpc[ci], qcq[ci]),
                    None => continue,
                }
            } else {
                qcq[ci]
            };
            if c422 {
                chroma_edge::<P, 16>(plane, co + e * 2, 1, cst, bsv[e], qpav, p, dc);
            } else {
                chroma_edge::<P, 8>(plane, co + e * 2, 1, cst, bsv[e], qpav, p, dc);
            }
        }
        for &e in hedges {
            if e == 0 {
                match top {
                    Top::No => {}
                    Top::One(tm) => {
                        let qpav = avg(tm.qpc[ci], qcq[ci]);
                        chroma_edge::<P, 8>(plane, co, cst, 1, bsh[0], qpav, p, dc);
                    }
                    Top::Pair(t0, t1) => {
                        for (j, (tm, bs)) in [(t0, bsh[0]), (t1, bst)].into_iter().enumerate() {
                            let qpav = avg(tm.qpc[ci], qcq[ci]);
                            chroma_edge::<P, 8>(plane, co + j * cs, 2 * cs, 1, bs, qpav, p, dc);
                        }
                    }
                }
                continue;
            }
            let o = co + e * sl * cst;
            chroma_edge::<P, 8>(plane, o, cst, 1, bsh[e], qcq[ci], p, dc);
        }
    }
}

/// Deblocks macroblock row `r` (all rows above must be deblocked already).
#[allow(clippy::too_many_arguments)]
pub fn row<P: Pix>(
    y: &mut [P],
    cb: &mut [P],
    cr: &mut [P],
    mbs: &[MbInfo],
    params: &[DbParams],
    wmb: usize,
    r: usize,
    fmt: Format,
) {
    let ls = wmb * 16;
    let cw = fmt.mbcw();
    let cs = wmb * cw;
    let ch = 16 >> fmt.cshift();
    for mx in 0..wmb {
        let addr = r * wmb + mx;
        let q = &mbs[addr];
        let p = params.get(q.slice as usize).unwrap_or(&DB_OFF);
        if p.idc == 1 {
            continue;
        }
        let same = |n: &MbInfo| p.idc != 2 || n.slice == q.slice;
        let left = (mx > 0 && same(&mbs[addr - 1])).then(|| &mbs[addr - 1]);
        let top = if r > 0 && same(&mbs[addr - wmb]) {
            Top::One(&mbs[addr - wmb])
        } else {
            Top::No
        };
        let g = Geo {
            lo: r * 16 * ls + mx * 16,
            lst: ls,
            co: r * ch * cs + mx * cw,
            cst: cs,
        };
        filter_mb(y, cb, cr, q, p, g, ls, cs, left, top, fmt);
    }
}

/// Deblocks macroblock pair row `pr` of an MBAFF frame (all pair rows above must be
/// deblocked already). `mbs` holds the top macroblock of pair (x, pr) at row 2 * pr and the
/// bottom one at row 2 * pr + 1.
#[allow(clippy::too_many_arguments)]
pub fn mbaff_row<P: Pix>(
    y: &mut [P],
    cb: &mut [P],
    cr: &mut [P],
    mbs: &[MbInfo],
    params: &[DbParams],
    wmb: usize,
    pr: usize,
    fmt: Format,
) {
    let ls = wmb * 16;
    let cw = fmt.mbcw();
    let cs = wmb * cw;
    let ch = 16 >> fmt.cshift();
    for mx in 0..wmb {
        for bot in 0..2 {
            let s = (2 * pr + bot) * wmb + mx;
            let q = &mbs[s];
            let p = params.get(q.slice as usize).unwrap_or(&DB_OFF);
            if p.idc == 1 {
                continue;
            }
            let same = |n: &MbInfo| p.idc != 2 || n.slice == q.slice;
            let field = q.field;
            let g = if field {
                Geo {
                    lo: (pr * 32 + bot) * ls + mx * 16,
                    lst: 2 * ls,
                    co: (pr * 2 * ch + bot) * cs + mx * cw,
                    cst: 2 * cs,
                }
            } else {
                Geo {
                    lo: (pr * 32 + bot * 16) * ls + mx * 16,
                    lst: ls,
                    co: (pr * 2 * ch + bot * ch) * cs + mx * cw,
                    cst: cs,
                }
            };
            // The left pair, by its top macroblock.
            let lt = 2 * pr * wmb + mx;
            let mut left = None;
            if mx > 0 && same(&mbs[lt - 1]) {
                if mbs[lt - 1].field == field {
                    left = Some(&mbs[s - 1]);
                } else {
                    mixed_left(y, cb, cr, mbs, q, p, g, wmb, lt - 1, bot, field, fmt);
                }
            }
            // Above: the pair's own top macroblock, or the pair above.
            let top = if !field && bot == 1 {
                Top::One(&mbs[s - wmb])
            } else if pr > 0 && same(&mbs[(2 * pr - 2) * wmb + mx]) {
                let at = (2 * pr - 2) * wmb + mx;
                match (field, mbs[at].field) {
                    (false, true) => Top::Pair(&mbs[at], &mbs[at + wmb]),
                    (true, true) => Top::One(&mbs[at + bot * wmb]),
                    _ => Top::One(&mbs[at + wmb]),
                }
            } else {
                Top::No
            };
            filter_mb(y, cb, cr, q, p, g, ls, cs, left, top, fmt);
        }
    }
}

/// The left edge of macroblock `q` when the left pair (top macroblock `lt`) is coded the
/// other way (frame/field): each line has its own neighbouring macroblock.
#[allow(clippy::too_many_arguments)]
fn mixed_left<P: Pix>(
    y: &mut [P],
    cb: &mut [P],
    cr: &mut [P],
    mbs: &[MbInfo],
    q: &MbInfo,
    p: &DbParams,
    g: Geo,
    wmb: usize,
    lt: usize,
    bot: usize,
    field: bool,
    fmt: Format,
) {
    let [dl, dc] = depths(fmt);
    let c422 = fmt.cshift() == 0;
    // Luma line k of the current macroblock: its left neighbour and bS.
    let bs_of = |k: usize| -> (&MbInfo, u8) {
        let (pm, row) = if field {
            let l = 2 * k + bot;
            (&mbs[lt + (l / 16) * wmb], l % 16)
        } else {
            let l = 16 * bot + k;
            (&mbs[lt + (l & 1) * wmb], l >> 1)
        };
        (
            pm,
            strength(pm, (row / 4) * 4 + 3, q, (k / 4) * 4, true, true),
        )
    };
    let qy = q.qp as i32;
    for k in 0..16 {
        let (pm, bs) = bs_of(k);
        if bs != 0 {
            let qpav = (pm.qp as i32 + qy + 1) >> 1;
            filter(y, g.lo + k * g.lst, 1, 0, 1, bs, qpav, p, false, dl);
        }
    }
    let c444 = fmt.cfi == 3;
    for (ci, plane) in [cb, cr].into_iter().enumerate() {
        for c in 0..if c422 || c444 { 16 } else { 8 } {
            // The luma line of the same field that holds this chroma line's bS.
            let k = if c422 || c444 {
                c
            } else if field {
                2 * c
            } else {
                4 * (c >> 1) + (c & 1)
            };
            let (pm, bs) = bs_of(k);
            if bs != 0 {
                let qpav = (pm.qpc[ci] as i32 + q.qpc[ci] as i32 + 1) >> 1;
                filter(plane, g.co + c * g.cst, 1, 0, 1, bs, qpav, p, !c444, dc);
            }
        }
    }
}
