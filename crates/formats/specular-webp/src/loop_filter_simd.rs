//! The VP8 in-loop deblocking filter (RFC 6386 §15) on 16 pixels at a time with `std::simd`.
//!
//! The arithmetic is the scalar filter's ([`crate::loop_filter`]) lane for lane, in `i16`
//! (signed pixels, -128..=127, kept clamped), so the output is bit-exact. Pixels along one edge
//! are independent, so the 16 pixels of a luma edge, or the 8 + 8 pixels of the matching U and
//! V edges, form one vector. Edges across rows load rows directly; edges across columns
//! transpose 16 rows × 8 bytes into 8 column vectors (and back), once per run of edges that
//! share columns. Edges are filtered in the scalar order within each plane.

use std::simd::prelude::*;
use std::simd::simd_swizzle;

type V = Simd<i16, 16>;
type B = Simd<u8, 16>;
type H = Simd<u8, 8>;
type M = Mask<i16, 16>;

/// Per-macroblock filter thresholds, splatted.
#[derive(Clone, Copy)]
pub(crate) struct Limits {
    mb_edge: V,
    sub_edge: V,
    interior: V,
    hev: V,
}

impl Limits {
    pub(crate) fn new(filter_level: u8, interior_limit: u8, hev_threshold: u8) -> Limits {
        let (l, i) = (i16::from(filter_level), i16::from(interior_limit));
        Limits {
            mb_edge: V::splat((l + 2) * 2 + i),
            sub_edge: V::splat(l * 2 + i),
            interior: V::splat(i),
            hev: V::splat(i16::from(hev_threshold)),
        }
    }
}

#[inline(always)]
fn c(x: V) -> V {
    x.simd_clamp(V::splat(-128), V::splat(127))
}

#[inline(always)]
fn ad(a: V, b: V) -> V {
    (a - b).abs()
}

#[inline(always)]
fn signed(b: B) -> V {
    b.cast::<i16>() - V::splat(128)
}

#[inline(always)]
fn unsigned(v: V) -> B {
    (v + V::splat(128)).cast::<u8>()
}

/// `|p0 - q0| * 2 + |p1 - q1| / 2 <= limit` (the simple filter's test, also part of the
/// normal filter's).
#[inline(always)]
fn simple_mask(p1: V, p0: V, q0: V, q1: V, limit: V) -> M {
    (ad(p0, q0) * V::splat(2) + (ad(p1, q1) >> 1)).simd_le(limit)
}

#[inline(always)]
fn normal_mask(x: &[V; 8], limit: V, interior: V) -> M {
    let [p3, p2, p1, p0, q0, q1, q2, q3] = *x;
    let m = ad(p3, p2)
        .simd_max(ad(p2, p1))
        .simd_max(ad(p1, p0))
        .simd_max(ad(q3, q2))
        .simd_max(ad(q2, q1))
        .simd_max(ad(q1, q0));
    simple_mask(p1, p0, q0, q1, limit) & m.simd_le(interior)
}

#[inline(always)]
fn hev(x: &[V; 8], t: V) -> M {
    let [_, _, p1, p0, q0, q1, _, _] = *x;
    ad(p1, p0).simd_gt(t) | ad(q1, q0).simd_gt(t)
}

/// The simple filter on `x = [p1, p0, q0, q1]`.
#[inline(always)]
fn simple_edge(x: &mut [V], limit: V) {
    let x: &mut [V; 4] = x.try_into().unwrap();
    let [p1, p0, q0, q1] = *x;
    let m = simple_mask(p1, p0, q0, q1, limit);
    if !m.any() {
        return;
    }
    let w = c(c(p1 - q1) + (q0 - p0) * V::splat(3));
    let a = c(w + V::splat(4)) >> 3;
    let b = c(w + V::splat(3)) >> 3;
    x[2] = m.select(c(q0 - a), q0);
    x[1] = m.select(c(p0 + b), p0);
}

/// The normal filter's macroblock-edge variant on `x = [p3 .. q3]`.
#[inline(always)]
fn mb_edge(x: &mut [V], l: &Limits) {
    let x: &mut [V; 8] = x.try_into().unwrap();
    let m = normal_mask(x, l.mb_edge, l.interior);
    if !m.any() {
        return;
    }
    let hv = hev(x, l.hev);
    let [_, p2, p1, p0, q0, q1, q2, _] = *x;
    let w = c(c(p1 - q1) + (q0 - p0) * V::splat(3));
    // High edge variance: adjust p0 and q0 only.
    let a = c(w + V::splat(4)) >> 3;
    let b = c(w + V::splat(3)) >> 3;
    // Otherwise: p2..q2 with weights 27, 18, 9.
    let tap = |k: i16| c((w * V::splat(k) + V::splat(63)) >> 7);
    let (a27, a18, a9) = (tap(27), tap(18), tap(9));
    let full = m & !hv;
    let hv = m & hv;
    x[4] = full.select(c(q0 - a27), hv.select(c(q0 - a), q0));
    x[3] = full.select(c(p0 + a27), hv.select(c(p0 + b), p0));
    x[5] = full.select(c(q1 - a18), q1);
    x[2] = full.select(c(p1 + a18), p1);
    x[6] = full.select(c(q2 - a9), q2);
    x[1] = full.select(c(p2 + a9), p2);
}

/// The normal filter's inner (subblock) edge variant on `x = [p3 .. q3]`.
#[inline(always)]
fn sub_edge(x: &mut [V], l: &Limits) {
    let x: &mut [V; 8] = x.try_into().unwrap();
    let m = normal_mask(x, l.sub_edge, l.interior);
    if !m.any() {
        return;
    }
    let hv = hev(x, l.hev);
    let [_, _, p1, p0, q0, q1, _, _] = *x;
    let outer = hv.select(c(p1 - q1), V::splat(0));
    let w = c(outer + (q0 - p0) * V::splat(3));
    let a = c(w + V::splat(4)) >> 3;
    let b = c(w + V::splat(3)) >> 3;
    let a2 = (a + V::splat(1)) >> 1;
    let inner = m & !hv;
    x[4] = m.select(c(q0 - a), q0);
    x[3] = m.select(c(p0 + b), p0);
    x[5] = inner.select(c(q1 - a2), q1);
    x[2] = inner.select(c(p1 + a2), p1);
}

// ---------------------------------------------------------------------------------------------
// 16 × 8 byte transposes (interleave stages, as unpack instructions).

const ILV8: [usize; 16] = [0, 8, 1, 9, 2, 10, 3, 11, 4, 12, 5, 13, 6, 14, 7, 15];
const ILV8_LO: [usize; 16] = [0, 16, 1, 17, 2, 18, 3, 19, 4, 20, 5, 21, 6, 22, 7, 23];
const ILV8_HI: [usize; 16] = [8, 24, 9, 25, 10, 26, 11, 27, 12, 28, 13, 29, 14, 30, 15, 31];
const ILV16_LO: [usize; 16] = [0, 1, 16, 17, 2, 3, 18, 19, 4, 5, 20, 21, 6, 7, 22, 23];
const ILV16_HI: [usize; 16] = [8, 9, 24, 25, 10, 11, 26, 27, 12, 13, 28, 29, 14, 15, 30, 31];
const ILV32_LO: [usize; 16] = [0, 1, 2, 3, 16, 17, 18, 19, 4, 5, 6, 7, 20, 21, 22, 23];
const ILV32_HI: [usize; 16] = [8, 9, 10, 11, 24, 25, 26, 27, 12, 13, 14, 15, 28, 29, 30, 31];
const ILV64_LO: [usize; 16] = [0, 1, 2, 3, 4, 5, 6, 7, 16, 17, 18, 19, 20, 21, 22, 23];
const ILV64_HI: [usize; 16] = [8, 9, 10, 11, 12, 13, 14, 15, 24, 25, 26, 27, 28, 29, 30, 31];

/// 16 rows of 8 bytes → 8 columns of 16 bytes (lane = row).
#[inline(always)]
fn transpose_in(r: &[H; 16]) -> [B; 8] {
    let y: [B; 8] = std::array::from_fn(|j| simd_swizzle!(r[2 * j], r[2 * j + 1], ILV8));
    // u32 lane k of zl[m] / zh[m]: rows 4m..4m+4 of column k / 4 + k.
    let zl: [B; 4] = std::array::from_fn(|m| simd_swizzle!(y[2 * m], y[2 * m + 1], ILV16_LO));
    let zh: [B; 4] = std::array::from_fn(|m| simd_swizzle!(y[2 * m], y[2 * m + 1], ILV16_HI));
    // u64 lanes: rows 0..8 (w[0..2]) or 8..16 (w[2..4]) of two columns.
    let wl = [
        simd_swizzle!(zl[0], zl[1], ILV32_LO),
        simd_swizzle!(zl[0], zl[1], ILV32_HI),
        simd_swizzle!(zl[2], zl[3], ILV32_LO),
        simd_swizzle!(zl[2], zl[3], ILV32_HI),
    ];
    let wh = [
        simd_swizzle!(zh[0], zh[1], ILV32_LO),
        simd_swizzle!(zh[0], zh[1], ILV32_HI),
        simd_swizzle!(zh[2], zh[3], ILV32_LO),
        simd_swizzle!(zh[2], zh[3], ILV32_HI),
    ];
    [
        simd_swizzle!(wl[0], wl[2], ILV64_LO),
        simd_swizzle!(wl[0], wl[2], ILV64_HI),
        simd_swizzle!(wl[1], wl[3], ILV64_LO),
        simd_swizzle!(wl[1], wl[3], ILV64_HI),
        simd_swizzle!(wh[0], wh[2], ILV64_LO),
        simd_swizzle!(wh[0], wh[2], ILV64_HI),
        simd_swizzle!(wh[1], wh[3], ILV64_LO),
        simd_swizzle!(wh[1], wh[3], ILV64_HI),
    ]
}

/// The inverse of [`transpose_in`].
#[inline(always)]
fn transpose_out(c: &[B; 8]) -> [H; 16] {
    // u16 lane i: row i (q[2k]) or 8 + i (q[2k + 1]) of columns 2k, 2k + 1.
    let q: [B; 8] = std::array::from_fn(|i| {
        let k = i / 2;
        if i % 2 == 0 {
            simd_swizzle!(c[2 * k], c[2 * k + 1], ILV8_LO)
        } else {
            simd_swizzle!(c[2 * k], c[2 * k + 1], ILV8_HI)
        }
    });
    // u32 lane i: row 4h + i of columns 0..4 or 4..8.
    let r = |a: B, b: B| (simd_swizzle!(a, b, ILV16_LO), simd_swizzle!(a, b, ILV16_HI));
    let (r0, r1) = r(q[0], q[2]); // rows 0..4 / 4..8, columns 0..4
    let (r2, r3) = r(q[4], q[6]); // rows 0..4 / 4..8, columns 4..8
    let (r4, r5) = r(q[1], q[3]); // rows 8..12 / 12..16, columns 0..4
    let (r6, r7) = r(q[5], q[7]);
    let s = |a: B, b: B| (simd_swizzle!(a, b, ILV32_LO), simd_swizzle!(a, b, ILV32_HI));
    let pairs = [s(r0, r2), s(r1, r3), s(r4, r6), s(r5, r7)];
    let mut out = [H::splat(0); 16];
    for (t, (lo, hi)) in pairs.into_iter().enumerate() {
        for (k, v) in [lo, hi].into_iter().enumerate() {
            out[4 * t + 2 * k] = simd_swizzle!(v, [0, 1, 2, 3, 4, 5, 6, 7]);
            out[4 * t + 2 * k + 1] = simd_swizzle!(v, [8, 9, 10, 11, 12, 13, 14, 15]);
        }
    }
    out
}

/// Columns `x .. x + 8` of the 16 window rows `r0 ..`, as signed vectors.
#[inline(always)]
fn load_cols<const W: usize>(rows: [&[u8; W]; 16], x: usize, out: &mut [V]) {
    let rows: [H; 16] = std::array::from_fn(|r| H::from_slice(&rows[r][x..x + 8]));
    for (o, b) in out.iter_mut().zip(transpose_in(&rows)) {
        *o = signed(b);
    }
}

#[inline(always)]
fn store_cols<const W: usize>(rows: [&mut [u8; W]; 16], x: usize, cols: &[V]) {
    let c: [B; 8] = std::array::from_fn(|k| unsigned(cols[k]));
    for (row, h) in rows.into_iter().zip(transpose_out(&c)) {
        h.copy_to_slice(&mut row[x..x + 8]);
    }
}

/// A `W`×`W` copy of the pixels around a macroblock: window row `r`, column `c` is plane row
/// `r - 4`, column `c - 4` relative to the macroblock's top-left pixel at `buf[at]`.
///
/// Filtering a local copy keeps every access at a constant offset into fixed-size arrays (no
/// bounds checks); copying 20 (or 12) short rows in and out is cheaper than checking each
/// of the ~200 row accesses of a macroblock.
struct Window<const W: usize> {
    px: [[u8; W]; W],
}

impl<const W: usize> Window<W> {
    /// Copy the window in. Without a left neighbour columns `0 .. 4` may lie before the
    /// buffer (the first chroma macroblock); they are not used then. `None` when the window
    /// does not fit (never for the pipeline's filter buffers, which keep `LAG >= 4` rows above
    /// the macroblock row). Every copy has a constant length.
    #[inline(always)]
    fn load(buf: &[u8], at: usize, stride: usize) -> Option<Self> {
        let mut px = [[0u8; W]; W];
        // Window column 4 of window row 0.
        let base = at.checked_sub(4 * stride)?;
        if let Some(base) = base.checked_sub(4) {
            for (r, row) in px.iter_mut().enumerate() {
                let s = base + r * stride;
                row.copy_from_slice(buf.get(s..s + W)?);
            }
        } else {
            for (r, row) in px.iter_mut().enumerate() {
                let s = base + r * stride;
                row[4..].copy_from_slice(buf.get(s..s + W - 4)?);
            }
        }
        Some(Window { px })
    }

    /// Write back rows `r0 .. r1`. Columns `0 .. 4` belong to the macroblock on the left and
    /// are only written when there is one (`left`): otherwise they may lie before the buffer,
    /// or, with a stride narrower than the window, alias the end of the row above.
    #[inline(always)]
    fn store(
        &self,
        buf: &mut [u8],
        at: usize,
        stride: usize,
        (r0, r1): (usize, usize),
        left: bool,
    ) {
        // Window column 0 (`left`) or 4 of window row 0.
        let Some(base) = at.checked_sub(4 * stride + if left { 4 } else { 0 }) else {
            return;
        };
        for r in r0..r1 {
            let s = base + r * stride;
            if left {
                if let Some(d) = buf.get_mut(s..s + W) {
                    d.copy_from_slice(&self.px[r]);
                }
            } else if let Some(d) = buf.get_mut(s..s + W - 4) {
                d.copy_from_slice(&self.px[r][4..]);
            }
        }
    }

    /// Rows `r0 .. r0 + 16` (wrapping is impossible for the callers' `r0`).
    #[inline(always)]
    fn rows<const N: usize>(&self, r0: usize) -> [&[u8; W]; N] {
        std::array::from_fn(|r| &self.px[r0 + r])
    }

    #[inline(always)]
    fn rows_mut<const N: usize>(&mut self, r0: usize) -> [&mut [u8; W]; N] {
        let mut it = self.px[r0..].iter_mut();
        std::array::from_fn(|_| it.next().unwrap_or_else(|| unreachable!()))
    }
}

/// Which edges of a macroblock to filter.
#[derive(Clone, Copy)]
pub(crate) struct Edges {
    /// The left macroblock edge (not on the first column).
    pub left: bool,
    /// The top macroblock edge (not on the first row).
    pub top: bool,
    /// The inner (subblock) edges.
    pub inner: bool,
}

/// The window rows that filtering `e` may change: the column edges touch every macroblock
/// row (window rows `4..last`), the top edge rows `p2..=q2` (`1..7`).
#[inline(always)]
fn dirty_rows(e: Edges, last: usize) -> (usize, usize) {
    let r0 = if e.top { 1 } else { 4 };
    let r1 = if e.left || e.inner { last } else { 7 };
    (r0, r1)
}

/// Filter one luma macroblock whose top-left pixel is `buf[at]`. Needs 4 rows above and 4
/// bytes to the left in `buf` (their contents only matter when `top` / `left`).
pub(crate) fn luma_mb(
    buf: &mut [u8],
    at: usize,
    stride: usize,
    e: Edges,
    l: &Limits,
    simple: bool,
) {
    if !(e.left || e.top || e.inner) {
        return;
    }
    let Some(mut w) = Window::<20>::load(buf, at, stride) else {
        return;
    };
    // Edges across columns. cols[j] is window column j (macroblock column j - 4).
    if e.left || e.inner {
        let mut cols = [V::splat(0); 20];
        let r = w.rows::<16>(4);
        if e.left {
            load_cols(r, 0, &mut cols[0..8]);
        }
        if e.inner {
            if e.left {
                load_cols(r, 8, &mut cols[8..16]);
            } else {
                load_cols(r, 4, &mut cols[4..12]);
            }
            load_cols(r, 12, &mut cols[12..20]);
        }
        if simple {
            if e.left {
                simple_edge(&mut cols[2..6], l.mb_edge);
            }
            if e.inner {
                for j in [8, 12, 16] {
                    simple_edge(&mut cols[j - 2..j + 2], l.sub_edge);
                }
            }
        } else {
            if e.left {
                mb_edge(&mut cols[0..8], l);
            }
            if e.inner {
                for j in [8, 12, 16] {
                    sub_edge(&mut cols[j - 4..j + 4], l);
                }
            }
        }
        if e.left {
            store_cols(w.rows_mut::<16>(4), 0, &cols[0..8]);
        }
        if e.inner {
            if e.left {
                store_cols(w.rows_mut::<16>(4), 8, &cols[8..16]);
            } else {
                store_cols(w.rows_mut::<16>(4), 4, &cols[4..12]);
            }
            store_cols(w.rows_mut::<16>(4), 12, &cols[12..20]);
        }
    }
    // Edges across rows. rows[i] is window row i (macroblock row i - 4).
    if e.top || e.inner {
        let mut rows = [V::splat(0); 20];
        let (i0, i1) = (if e.top { 0 } else { 4 }, if e.inner { 20 } else { 8 });
        for (r, px) in rows[i0..i1].iter_mut().zip(&w.px[i0..i1]) {
            *r = signed(B::from_slice(&px[4..20]));
        }
        if simple {
            if e.top {
                simple_edge(&mut rows[2..6], l.mb_edge);
            }
            if e.inner {
                for i in [8, 12, 16] {
                    simple_edge(&mut rows[i - 2..i + 2], l.sub_edge);
                }
            }
        } else {
            if e.top {
                mb_edge(&mut rows[0..8], l);
            }
            if e.inner {
                for i in [8, 12, 16] {
                    sub_edge(&mut rows[i - 4..i + 4], l);
                }
            }
        }
        let (s0, s1) = (if e.top { 1 } else { 5 }, if e.inner { 19 } else { 7 });
        for (r, px) in rows[s0..s1].iter().zip(&mut w.px[s0..s1]) {
            unsigned(*r).copy_to_slice(&mut px[4..20]);
        }
    }
    w.store(buf, at, stride, dirty_rows(e, 20), e.left);
}

/// Filter the U and V blocks of one macroblock (normal filter only) at `u[at]` / `v[at]`.
pub(crate) fn chroma_mb(
    u: &mut [u8],
    v: &mut [u8],
    at: usize,
    stride: usize,
    e: Edges,
    l: &Limits,
) {
    if !(e.left || e.top || e.inner) {
        return;
    }
    let (Some(mut wu), Some(mut wv)) = (
        Window::<12>::load(u, at, stride),
        Window::<12>::load(v, at, stride),
    ) else {
        return;
    };
    // Vectors hold U rows (or columns) in lanes 0..8 and V in 8..16.
    if e.left || e.inner {
        let mut cols = [V::splat(0); 12];
        let load = |wu: &Window<12>, wv: &Window<12>, x: usize, out: &mut [V]| {
            let (a, b) = (wu.rows::<8>(4), wv.rows::<8>(4));
            load_cols(
                std::array::from_fn(|r| if r < 8 { a[r] } else { b[r - 8] }),
                x,
                out,
            );
        };
        if e.left {
            load(&wu, &wv, 0, &mut cols[0..8]);
        }
        if e.inner {
            load(&wu, &wv, 4, &mut cols[4..12]);
        }
        if e.left {
            mb_edge(&mut cols[0..8], l);
        }
        if e.inner {
            sub_edge(&mut cols[4..12], l);
        }
        let mut store = |x: usize, c: &[V]| {
            let mut a = wu.rows_mut::<8>(4).into_iter();
            let mut b = wv.rows_mut::<8>(4).into_iter();
            let rows = std::array::from_fn(|r| {
                let row = if r < 8 { a.next() } else { b.next() };
                row.unwrap_or_else(|| unreachable!())
            });
            store_cols(rows, x, c);
        };
        if e.left {
            store(0, &cols[0..8]);
        }
        if e.inner {
            store(4, &cols[4..12]);
        }
    }
    if e.top || e.inner {
        let mut rows = [V::splat(0); 12];
        let (i0, i1) = (if e.top { 0 } else { 4 }, if e.inner { 12 } else { 8 });
        for ((r, pu), pv) in rows[i0..i1]
            .iter_mut()
            .zip(&wu.px[i0..i1])
            .zip(&wv.px[i0..i1])
        {
            let a = H::from_slice(&pu[4..12]);
            let b = H::from_slice(&pv[4..12]);
            *r = signed(simd_swizzle!(
                a,
                b,
                [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]
            ));
        }
        if e.top {
            mb_edge(&mut rows[0..8], l);
        }
        if e.inner {
            sub_edge(&mut rows[4..12], l);
        }
        let (s0, s1) = (if e.top { 1 } else { 5 }, if e.inner { 11 } else { 7 });
        for ((r, pu), pv) in rows[s0..s1]
            .iter()
            .zip(&mut wu.px[s0..s1])
            .zip(&mut wv.px[s0..s1])
        {
            let b = unsigned(*r);
            simd_swizzle!(b, [0, 1, 2, 3, 4, 5, 6, 7]).copy_to_slice(&mut pu[4..12]);
            simd_swizzle!(b, [8, 9, 10, 11, 12, 13, 14, 15]).copy_to_slice(&mut pv[4..12]);
        }
    }
    let r = dirty_rows(e, 12);
    wu.store(u, at, stride, r, e.left);
    wv.store(v, at, stride, r, e.left);
}
