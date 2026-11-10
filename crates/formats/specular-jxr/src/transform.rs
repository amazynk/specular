//! Inverse transform and overlap filtering (T.832 clause 9.9).
//!
//! Coefficients live in full-resolution `i32` planes: the DC/LP coefficient of every 4x4 block
//! sits at the block's top-left sample, the HP coefficients at the remaining 15 positions in
//! raster order, so the "second level coefficient combination" of 9.9.4 is implicit. The first
//! level operates on the DC grid, a view of every fourth sample in each direction.

use rayon::prelude::*;

#[inline(always)]
fn t2x2h(a: &mut i32, b: &mut i32, c: &mut i32, d: &mut i32, round: i32) {
    *a += *d;
    *b -= *c;
    let t1 = (*a - *b + round) >> 1;
    let t2 = *c;
    *c = t1 - *d;
    *d = t1 - t2;
    *a -= *d;
    *b += *c;
}

#[inline(always)]
fn inv_todd(a: &mut i32, b: &mut i32, c: &mut i32, d: &mut i32) {
    *b += *d;
    *a -= *c;
    *d -= *b >> 1;
    *c += (*a + 1) >> 1;
    *a -= (3 * *b + 4) >> 3;
    *b += (3 * *a + 4) >> 3;
    *c -= (3 * *d + 4) >> 3;
    *d += (3 * *c + 4) >> 3;
    *c -= (*b + 1) >> 1;
    *d = ((*a + 1) >> 1) - *d;
    *b += *c;
    *a -= *d;
}

#[inline(always)]
fn inv_toddodd(a: &mut i32, b: &mut i32, c: &mut i32, d: &mut i32) {
    *d += *a;
    *c -= *b;
    let t1 = *d >> 1;
    let t2 = *c >> 1;
    *a -= t1;
    *b += t2;
    *a -= (*b * 3 + 3) >> 3;
    *b += (*a * 3 + 3) >> 2;
    *a -= (*b * 3 + 4) >> 3;
    *b -= t2;
    *a += t1;
    *c += *b;
    *d -= *a;
    *b = -*b;
    *c = -*c;
}

#[inline(always)]
fn inv_toddodd_post(a: &mut i32, b: &mut i32, c: &mut i32, d: &mut i32) {
    *d += *a;
    *c -= *b;
    let t1 = *d >> 1;
    let t2 = *c >> 1;
    *a -= t1;
    *b += t2;
    *a -= (*b * 3 + 6) >> 3;
    *b += (*a * 3 + 2) >> 2;
    *a -= (*b * 3 + 4) >> 3;
    *b -= t2;
    *a += t1;
    *c += *b;
    *d -= *a;
}

#[inline(always)]
fn t2x2h_post(a: &mut i32, b: &mut i32, c: &mut i32, d: &mut i32) {
    *b -= *c;
    *a += (*d * 3 + 4) >> 3;
    *d -= *b >> 1;
    *c = ((*a - *b) >> 1) - *c;
    std::mem::swap(c, d);
    *a -= *d;
    *b += *c;
}

#[inline(always)]
fn inv_rotate(a: &mut i32, b: &mut i32) {
    *a -= (*b + 1) >> 1;
    *b += (*a + 1) >> 1;
}

#[inline(always)]
fn inv_scale(a: &mut i32, b: &mut i32) {
    *a += *b;
    *b = (*a >> 1) - *b;
    *a += (*b * 3) >> 3;
    *b += (*a * 3) >> 4;
    *b += *a >> 7;
    *b -= *a >> 10;
}

/// T2pt (9.9.7.7).
#[inline(always)]
pub(crate) fn t2pt(a: &mut i32, b: &mut i32) {
    *a -= (*b + 1) >> 1;
    *b += *a;
}

macro_rules! q {
    ($f:ident, $c:ident, $i:expr, $j:expr, $k:expr, $l:expr $(, $r:expr)?) => {{
        let (mut a, mut b, mut c, mut d) = ($c[$i], $c[$j], $c[$k], $c[$l]);
        $f(&mut a, &mut b, &mut c, &mut d $(, $r)?);
        $c[$i] = a;
        $c[$j] = b;
        $c[$k] = c;
        $c[$l] = d;
    }};
}

macro_rules! p {
    ($f:ident, $c:ident, $i:expr, $j:expr) => {{
        let (mut a, mut b) = ($c[$i], $c[$j]);
        $f(&mut a, &mut b);
        $c[$i] = a;
        $c[$j] = b;
    }};
}

const INV_PERM: [usize; 16] = [0, 8, 4, 13, 2, 15, 3, 14, 1, 12, 5, 9, 7, 11, 6, 10];

/// ICT4x4 (9.9.7.1) on 16 coefficients in raster order.
#[inline(always)]
pub(crate) fn ict4x4(x: &mut [i32; 16]) {
    let mut c = [0i32; 16];
    for i in 0..16 {
        c[INV_PERM[i]] = x[i];
    }
    q!(t2x2h, c, 0, 1, 4, 5, 1);
    q!(inv_todd, c, 2, 3, 6, 7);
    q!(inv_todd, c, 8, 12, 9, 13);
    q!(inv_toddodd, c, 10, 11, 14, 15);
    q!(t2x2h, c, 0, 3, 12, 15, 0);
    q!(t2x2h, c, 5, 6, 9, 10, 0);
    q!(t2x2h, c, 1, 2, 13, 14, 0);
    q!(t2x2h, c, 4, 7, 8, 11, 0);
    *x = c;
}

/// OverlapPostFilter4x4 (9.9.8.1).
#[inline(always)]
fn post4x4(c: &mut [i32; 16]) {
    q!(t2x2h, c, 0, 3, 12, 15, 0);
    q!(t2x2h, c, 1, 2, 13, 14, 0);
    q!(t2x2h, c, 4, 7, 8, 11, 0);
    q!(t2x2h, c, 5, 6, 9, 10, 0);
    p!(inv_rotate, c, 13, 12);
    p!(inv_rotate, c, 9, 8);
    p!(inv_rotate, c, 7, 3);
    p!(inv_rotate, c, 6, 2);
    q!(inv_toddodd_post, c, 10, 11, 14, 15);
    p!(inv_scale, c, 0, 15);
    p!(inv_scale, c, 1, 14);
    p!(inv_scale, c, 4, 11);
    p!(inv_scale, c, 5, 10);
    q!(t2x2h_post, c, 0, 3, 12, 15);
    q!(t2x2h_post, c, 1, 2, 13, 14);
    q!(t2x2h_post, c, 4, 7, 8, 11);
    q!(t2x2h_post, c, 5, 6, 9, 10);
}

/// OverlapPostFilter4 (9.9.8.2).
#[inline(always)]
fn post4(c: &mut [i32; 4]) {
    c[0] += c[3];
    c[1] += c[2];
    c[3] -= (c[0] + 1) >> 1;
    c[2] -= (c[1] + 1) >> 1;
    p!(inv_scale, c, 0, 3);
    p!(inv_scale, c, 1, 2);
    c[0] += (c[3] * 3 + 4) >> 3;
    c[1] += (c[2] * 3 + 4) >> 3;
    c[3] -= c[0] >> 1;
    c[2] -= c[1] >> 1;
    c[0] += c[3];
    c[1] += c[2];
    c[3] = -c[3];
    c[2] = -c[2];
    p!(inv_rotate, c, 2, 3);
    c[3] += (c[0] + 1) >> 1;
    c[2] += (c[1] + 1) >> 1;
    c[0] -= c[3];
    c[1] -= c[2];
}

/// OverlapPostFilter2x2 (9.9.8.3).
#[inline(always)]
fn post2x2(c: &mut [i32; 4]) {
    c[0] += c[3];
    c[1] += c[2];
    c[3] -= (c[0] + 1) >> 1;
    c[2] -= (c[1] + 1) >> 1;
    c[1] += (c[0] + 2) >> 2;
    c[0] += (c[1] + 1) >> 1;
    c[0] += c[1] >> 5;
    c[0] += c[1] >> 9;
    c[0] += c[1] >> 13;
    c[1] += (c[0] + 2) >> 2;
    c[3] += (c[0] + 1) >> 1;
    c[2] += (c[1] + 1) >> 1;
    c[0] -= c[3];
    c[1] -= c[2];
}

/// OverlapPostFilter2 (9.9.8.4).
#[inline(always)]
fn post2(c: &mut [i32; 2]) {
    c[1] += (c[0] + 2) >> 2;
    c[0] += (c[1] + 1) >> 1;
    c[0] += c[1] >> 5;
    c[0] += c[1] >> 9;
    c[0] += c[1] >> 13;
    c[1] += (c[0] + 2) >> 2;
}

/// A strided view: sample (x, y) is at `y * ys + x * xs`.
#[derive(Clone, Copy)]
pub(crate) struct View {
    pub xs: usize,
    pub ys: usize,
}

impl View {
    #[inline(always)]
    fn at(self, x: usize, y: usize) -> usize {
        y * self.ys + x * self.xs
    }

    #[inline(always)]
    fn f4<const N: usize>(self, d: &mut [i32], pts: [(usize, usize); N], f: fn(&mut [i32; N])) {
        let mut c = [0i32; N];
        for (k, &(x, y)) in pts.iter().enumerate() {
            c[k] = d[self.at(x, y)];
        }
        f(&mut c);
        for (k, &(x, y)) in pts.iter().enumerate() {
            d[self.at(x, y)] = c[k];
        }
    }
}

/// Interior 4x4 windows and left/right edge filters for the four rows `y..y + 4` of every column
/// region in `cols` (half-open sample ranges).
fn ov4_band(d: &mut [i32], v: View, y: usize, cols: &[(usize, usize)]) {
    for &(l, r) in cols {
        let mut x = l + 2;
        while x + 2 < r {
            let mut c = [0i32; 16];
            for j in 0..4 {
                for i in 0..4 {
                    c[4 * j + i] = d[v.at(x + i, y + j)];
                }
            }
            post4x4(&mut c);
            for j in 0..4 {
                for i in 0..4 {
                    d[v.at(x + i, y + j)] = c[4 * j + i];
                }
            }
            x += 4;
        }
        for xe in [l, l + 1, r - 2, r - 1] {
            v.f4(d, [(xe, y), (xe, y + 1), (xe, y + 2), (xe, y + 3)], post4);
        }
    }
}

/// Top/bottom edge pairs of rows and the 2x2 corners of one region.
fn ov4_edges(d: &mut [i32], v: View, (l, r): (usize, usize), (t, b): (usize, usize)) {
    for ye in [t, t + 1, b - 2, b - 1] {
        let mut x = l + 2;
        while x + 2 < r {
            v.f4(d, [(x, ye), (x + 1, ye), (x + 2, ye), (x + 3, ye)], post4);
            x += 4;
        }
    }
    for (x, y) in [(l, t), (r - 2, t), (l, b - 2), (r - 2, b - 2)] {
        v.f4(d, [(x, y), (x + 1, y), (x, y + 1), (x + 1, y + 1)], post4);
    }
}

/// 4x4-based overlap filtering over `cols` x `rows` regions (the whole plane when tiles are soft,
/// every tile when they are hard). Used both for the second level (unit stride) and for the
/// first level of full-resolution components (stride 4 on the DC grid).
pub(crate) fn overlap4(
    d: &mut [i32],
    v: View,
    cols: &[(usize, usize)],
    rows: &[(usize, usize)],
    parallel: bool,
) {
    for &(t, b) in rows {
        if parallel && v.xs == 1 && b - t >= 8 {
            let start = (t + 2) * v.ys;
            let end = (b - 2) * v.ys;
            d[start..end]
                .par_chunks_mut(4 * v.ys)
                .for_each(|band| ov4_band(band, v, 0, cols));
        } else {
            let mut y = t + 2;
            while y + 2 < b {
                ov4_band(d, v, y, cols);
                y += 4;
            }
        }
        for &c in cols {
            ov4_edges(d, v, c, (t, b));
        }
    }
}

/// First-level overlap of 4:2:0 / 4:2:2 chroma on the DC grid (9.9.3.3, 9.9.3.4): 2x2 windows
/// between blocks, two-point filters along region edges and the corner differences.
pub(crate) fn overlap2(d: &mut [i32], v: View, cols: &[(usize, usize)], rows: &[(usize, usize)]) {
    for &(t, b) in rows {
        // Corner differences (OverlapPostFilter1).
        let corners = |d: &mut [i32], sign: i32| {
            for &(l, r) in cols {
                for y in [t, b - 1] {
                    if sign < 0 {
                        d[v.at(l, y)] -= d[v.at(l + 1, y)];
                        d[v.at(r - 1, y)] -= d[v.at(r - 2, y)];
                    } else {
                        d[v.at(l, y)] += d[v.at(l + 1, y)];
                        d[v.at(r - 1, y)] += d[v.at(r - 2, y)];
                    }
                }
            }
        };
        corners(d, -1);
        for &(l, r) in cols {
            let mut y = t + 1;
            while y + 2 < b {
                let mut x = l + 1;
                while x + 2 < r {
                    v.f4(d, [(x, y), (x + 1, y), (x, y + 1), (x + 1, y + 1)], post2x2);
                    x += 2;
                }
                for xe in [l, r - 1] {
                    v.f4(d, [(xe, y), (xe, y + 1)], post2);
                }
                y += 2;
            }
            for ye in [t, b - 1] {
                let mut x = l + 1;
                while x + 2 < r {
                    v.f4(d, [(x, ye), (x + 1, ye)], post2);
                    x += 2;
                }
            }
        }
        corners(d, 1);
    }
}

/// Second-level ICT of every 4x4 block of a plane `w` samples wide.
pub(crate) fn ict_plane(d: &mut [i32], w: usize, parallel: bool) {
    let band = |rows: &mut [i32]| {
        for x in (0..w).step_by(4) {
            let mut c = [0i32; 16];
            for j in 0..4 {
                c[4 * j..4 * j + 4].copy_from_slice(&rows[j * w + x..j * w + x + 4]);
            }
            ict4x4(&mut c);
            for j in 0..4 {
                rows[j * w + x..j * w + x + 4].copy_from_slice(&c[4 * j..4 * j + 4]);
            }
        }
    };
    if parallel {
        d.par_chunks_mut(4 * w).for_each(band);
    } else {
        d.chunks_mut(4 * w).for_each(band);
    }
}

/// First-level inverse transform of one macroblock's DC/LP coefficients, gathered in `c` in the
/// MbDCLP order (raster over the macroblock's blocks; 4, 2x4 or 2x2 entries per row).
pub(crate) fn first_level_mb(c: &mut [i32; 16], kind: Chroma, scaled_chroma: bool) {
    match kind {
        Chroma::Full => ict4x4(c),
        Chroma::C420 => {
            q!(t2x2h, c, 0, 1, 2, 3, 0);
            c.swap(1, 2);
        }
        Chroma::C422 => {
            p!(t2pt, c, 0, 4);
            q!(t2x2h, c, 0, 1, 2, 3, 0);
            c.swap(1, 2);
            q!(t2x2h, c, 4, 6, 5, 7, 0);
            c.swap(5, 6);
        }
    }
    if scaled_chroma {
        let n = match kind {
            Chroma::Full => 16,
            Chroma::C420 => 4,
            Chroma::C422 => 8,
        };
        for v in &mut c[..n] {
            *v *= 2;
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Chroma {
    Full,
    C420,
    C422,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dc_only_block_is_flat() {
        let mut c = [0i32; 16];
        c[0] = 64;
        ict4x4(&mut c);
        assert!(c.iter().all(|&v| v == c[0]), "{c:?}");
    }
}
