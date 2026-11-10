//! The loop filter (RFC 6386 section 15): edge filters over a buffer, addressed by the index
//! of the first pixel after the edge (`q0`), the step across the edge and the step along it.

use crate::header::{Header, LfDeltas, Segmentation};
use crate::modes::{B_PRED, MbInfo, SPLITMV, ZEROMV};

use std::simd::cmp::{SimdOrd, SimdPartialOrd};
use std::simd::num::{SimdInt, SimdUint};
use std::simd::{Mask, Select, Simd, simd_swizzle};

/// Thresholds for one macroblock.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Params {
    pub interior: i32,
    pub hev: i32,
    pub mb_limit: i32,
    pub sub_limit: i32,
}

/// The filter level of a macroblock (section 15.4 and the segment/delta adjustments).
pub(crate) fn level(h: &Header, seg: &Segmentation, lf: &LfDeltas, mb: &MbInfo) -> i32 {
    let mut l = h.filter_level;
    if seg.enabled {
        let v = seg.lf[mb.segment as usize & 3];
        l = if seg.absolute { v } else { l + v };
        l = l.clamp(0, 63);
    }
    if lf.enabled {
        l += lf.by_ref[mb.ref_frame as usize & 3];
        if mb.ref_frame == 0 {
            if mb.y_mode == B_PRED {
                l += lf.by_mode[0];
            }
        } else if mb.y_mode == ZEROMV {
            l += lf.by_mode[1];
        } else if mb.y_mode == SPLITMV {
            l += lf.by_mode[3];
        } else {
            l += lf.by_mode[2];
        }
        l = l.clamp(0, 63);
    }
    l
}

pub(crate) fn params(level: i32, sharpness: i32, key: bool) -> Params {
    let mut interior = level;
    if sharpness > 0 {
        interior >>= if sharpness > 4 { 2 } else { 1 };
        interior = interior.min(9 - sharpness);
    }
    let interior = interior.max(1);
    let hev = if key {
        (level >= 40) as i32 + (level >= 15) as i32
    } else {
        (level >= 40) as i32 + (level >= 20) as i32 + (level >= 15) as i32
    };
    Params {
        interior,
        hev,
        mb_limit: (level + 2) * 2 + interior,
        sub_limit: level * 2 + interior,
    }
}

type V = Simd<i16, 16>;

/// Swizzles of 16 rows of 8 pixels (rows 0..8 in the first vector, 8..16 in the second)
/// into columns 0..4 and 4..8 (16 lanes each), and back.
const TO_COLS: [[usize; 64]; 2] = [cols(0), cols(1)];
const TO_ROWS: [[usize; 64]; 2] = [rows(0), rows(1)];

const fn cols(half: usize) -> [usize; 64] {
    let mut t = [0; 64];
    let mut l = 0;
    while l < 64 {
        let (k, r) = (half * 4 + l / 16, l % 16);
        t[l] = if r < 8 {
            r * 8 + k
        } else {
            64 + (r - 8) * 8 + k
        };
        l += 1;
    }
    t
}

const fn rows(half: usize) -> [usize; 64] {
    let mut t = [0; 64];
    let mut l = 0;
    while l < 64 {
        let (r, k) = (half * 8 + l / 8, l % 8);
        t[l] = if k < 4 {
            k * 16 + r
        } else {
            64 + (k - 4) * 16 + r
        };
        l += 1;
    }
    t
}
type M = Mask<i16, 16>;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Simple,
    Mb,
    Sub,
}

/// Reads the eight pixels p3..q3 at each of `n` (8 or 16) positions as signed values.
#[inline(always)]
fn gather<const N: usize>(buf: &[u8], q0: usize, across: usize, along: usize) -> [V; 8] {
    let n = N;
    let mut px = [V::splat(0); 8];
    if across == 1 {
        // 16 rows of 8 pixels, transposed into 8 vectors of 16.
        let mut a = [128u8; 64];
        let mut b = [128u8; 64];
        for i in 0..n {
            let o = q0 + i * along - 4;
            let d = if i < 8 {
                &mut a[i * 8..i * 8 + 8]
            } else {
                &mut b[(i - 8) * 8..(i - 8) * 8 + 8]
            };
            d.copy_from_slice(&buf[o..o + 8]);
        }
        let (a, b) = (Simd::<u8, 64>::from_array(a), Simd::<u8, 64>::from_array(b));
        let lo: Simd<u8, 64> = simd_swizzle!(a, b, TO_COLS[0]);
        let hi: Simd<u8, 64> = simd_swizzle!(a, b, TO_COLS[1]);
        let (lo, hi) = (lo.to_array(), hi.to_array());
        for k in 0..8 {
            let src = if k < 4 { &lo } else { &hi };
            let o = (k & 3) * 16;
            px[k] = Simd::<u8, 16>::from_slice(&src[o..o + 16]).cast::<i16>() - V::splat(128);
        }
    } else {
        for (k, v) in px.iter_mut().enumerate() {
            let o = q0 + k * across - 4 * across;
            let mut a = [128u8; 16];
            if let (Some(d), Some(s)) = (a.first_chunk_mut::<N>(), buf[o..].first_chunk::<N>()) {
                *d = *s;
            }
            *v = Simd::<u8, 16>::from_array(a).cast::<i16>() - V::splat(128);
        }
    }
    px
}

#[inline(always)]
fn scatter<const N: usize>(
    buf: &mut [u8],
    q0: usize,
    across: usize,
    along: usize,
    px: &[V; 8],
    rows: std::ops::Range<usize>,
) {
    let n = N;
    if across == 1 {
        let mut lo = [0u8; 64];
        let mut hi = [0u8; 64];
        for k in 0..8 {
            let v = (px[k] + V::splat(128)).cast::<u8>().to_array();
            let d = if k < 4 { &mut lo } else { &mut hi };
            d[(k & 3) * 16..(k & 3) * 16 + 16].copy_from_slice(&v);
        }
        let (lo, hi) = (
            Simd::<u8, 64>::from_array(lo),
            Simd::<u8, 64>::from_array(hi),
        );
        let a: Simd<u8, 64> = simd_swizzle!(lo, hi, TO_ROWS[0]);
        let b: Simd<u8, 64> = simd_swizzle!(lo, hi, TO_ROWS[1]);
        let (a, b) = (a.to_array(), b.to_array());
        for i in 0..n {
            let o = q0 + i * along - 4;
            let src = if i < 8 {
                &a[i * 8..i * 8 + 8]
            } else {
                &b[(i - 8) * 8..(i - 8) * 8 + 8]
            };
            let d = &mut buf[o..o + 8];
            d[rows.clone()].copy_from_slice(&src[rows.clone()]);
        }
    } else {
        for k in rows {
            let o = q0 + k * across - 4 * across;
            let a = (px[k] + V::splat(128)).cast::<u8>().to_array();
            if let (Some(d), Some(s)) = (buf[o..].first_chunk_mut::<N>(), a.first_chunk::<N>()) {
                *d = *s;
            }
        }
    }
}

#[inline(always)]
fn clamp8(v: V) -> V {
    v.simd_clamp(V::splat(-128), V::splat(127))
}

#[inline(always)]
fn edge_filter<const N: usize>(
    buf: &mut [u8],
    q0: usize,
    across: usize,
    along: usize,
    kind: Kind,
    p: &Params,
    limit: i32,
) {
    let mut px = gather::<N>(buf, q0, across, along);
    let [p3, p2, p1, p0, q0v, q1, q2, q3] = px;
    let ad = |a: V, b: V| (a - b).abs();
    let mut m: M =
        (ad(p0, q0v) * V::splat(2) + (ad(p1, q1) >> V::splat(1))).simd_le(V::splat(limit as i16));
    if kind != Kind::Simple {
        let i = V::splat(p.interior as i16);
        m &= ad(p3, p2).simd_le(i)
            & ad(p2, p1).simd_le(i)
            & ad(p1, p0).simd_le(i)
            & ad(q1, q0v).simd_le(i)
            & ad(q2, q1).simd_le(i)
            & ad(q3, q2).simd_le(i);
    }
    if !m.any() {
        return;
    }
    let zero = V::splat(0);
    let hev = if kind == Kind::Simple {
        M::splat(true)
    } else {
        let t = V::splat(p.hev as i16);
        ad(p1, p0).simd_gt(t) | ad(q1, q0v).simd_gt(t)
    };
    let three = V::splat(3);
    match kind {
        Kind::Simple | Kind::Sub => {
            let outer = hev.select(clamp8(p1 - q1), zero);
            let a = m.select(clamp8(outer + three * (q0v - p0)), zero);
            let f1 = clamp8(a + V::splat(4)) >> V::splat(3);
            let f2 = clamp8(a + three) >> V::splat(3);
            px[4] = clamp8(q0v - f1);
            px[3] = clamp8(p0 + f2);
            if kind == Kind::Sub {
                let a = (!hev & m).select((f1 + V::splat(1)) >> V::splat(1), zero);
                px[5] = clamp8(q1 - a);
                px[2] = clamp8(p1 + a);
                scatter::<N>(buf, q0, across, along, &px, 2..6);
            } else {
                scatter::<N>(buf, q0, across, along, &px, 3..5);
            }
        }
        Kind::Mb => {
            let w = m.select(clamp8(clamp8(p1 - q1) + three * (q0v - p0)), zero);
            // High edge variance: the common adjustment on p0, q0 only.
            let wh = hev.select(w, zero);
            let f1 = clamp8(wh + V::splat(4)) >> V::splat(3);
            let f2 = clamp8(wh + three) >> V::splat(3);
            let q0n = clamp8(q0v - f1);
            let p0n = clamp8(p0 + f2);
            // Otherwise the wide filter.
            let wl = (!hev).select(w, zero);
            let tap = |k: i16| clamp8((V::splat(k) * wl + V::splat(63)) >> V::splat(7));
            let a = tap(27);
            px[4] = clamp8(q0n - a);
            px[3] = clamp8(p0n + a);
            let a = tap(18);
            px[5] = clamp8(q1 - a);
            px[2] = clamp8(p1 + a);
            let a = tap(9);
            px[6] = clamp8(q2 - a);
            px[1] = clamp8(p2 + a);
            scatter::<N>(buf, q0, across, along, &px, 1..7);
        }
    }
}

/// The simple filter across `n` positions of an edge.
pub(crate) fn simple(buf: &mut [u8], q0: usize, across: usize, along: usize, n: usize, limit: i32) {
    let p = Params::default();
    if n == 16 {
        edge_filter::<16>(buf, q0, across, along, Kind::Simple, &p, limit);
    } else {
        edge_filter::<8>(buf, q0, across, along, Kind::Simple, &p, limit);
    }
}

/// The normal filter across a macroblock edge.
pub(crate) fn mb_edge(
    buf: &mut [u8],
    q0: usize,
    across: usize,
    along: usize,
    n: usize,
    p: &Params,
) {
    if n == 16 {
        edge_filter::<16>(buf, q0, across, along, Kind::Mb, p, p.mb_limit);
    } else {
        edge_filter::<8>(buf, q0, across, along, Kind::Mb, p, p.mb_limit);
    }
}

/// The normal filter across an edge between subblocks.
pub(crate) fn sub_edge(
    buf: &mut [u8],
    q0: usize,
    across: usize,
    along: usize,
    n: usize,
    p: &Params,
) {
    if n == 16 {
        edge_filter::<16>(buf, q0, across, along, Kind::Sub, p, p.sub_limit);
    } else {
        edge_filter::<8>(buf, q0, across, along, Kind::Sub, p, p.sub_limit);
    }
}
