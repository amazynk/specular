//! Inter prediction sample generation (8.4.2.2): quarter-sample luma interpolation with the
//! six-tap filter, eighth-sample bilinear chroma, and weighted sample prediction (8.4.2.3).
//!
//! Reference samples come from a [`View`] (a frame or one field of a reference); positions
//! outside it are clamped to its edges, and the lines needed are waited for, so a reference that is still being
//! decoded on another thread can be read from its finished top part.
//!
//! The kernels are monomorphised per block width (luma 4, 8 or 16, chroma 2, 4 or 8): rows
//! are copied with fixed-size moves rather than `memmove` calls, and each row's arithmetic is
//! a straight loop of known length that the compiler vectorises for both sample types.

use crate::picture::View;
use crate::pixel::Pix;
use std::simd::Simd;

/// Window stride: blocks are at most 16 wide, plus 5 samples of filter support.
const WS: usize = 24;

/// Chroma window stride: blocks are at most 8 wide, plus 1 sample.
const CS: usize = 9;

/// Interpolation windows, kept between blocks so they are not cleared for each one: every
/// sample read was written for the same block first.
pub struct Scratch<P> {
    /// Luma reference samples from (x - 2, y - 2): up to 21 rows of `WS`.
    win: [P; WS * 21],
    /// Chroma reference samples: up to 17 rows of `CS`.
    cwin: [P; CS * 17],
    /// Horizontal six-tap intermediates for the centre luma positions.
    mid: [[i32; 16]; 21],
    /// Chroma sums fit 16 bits (bit depth at most 10: 64 * 1023 + 32 < 2^16).
    cnarrow: bool,
}

impl<P: Pix> Scratch<P> {
    /// `max_c`: the largest chroma sample value.
    pub fn new(max_c: i32) -> Box<Scratch<P>> {
        Box::new(Scratch {
            win: [P::default(); WS * 21],
            cwin: [P::default(); CS * 17],
            mid: [[0; 16]; 21],
            cnarrow: max_c <= 1023,
        })
    }
}

#[inline(always)]
fn tap(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32) -> i32 {
    a - 5 * b + 20 * c + 20 * d - 5 * e + f
}

/// Six-tap filter of `W` adjacent positions of a row: `out[c]` uses `s[c..c + 6]`.
#[inline(always)]
fn htap<P: Pix, const W: usize>(s: &[P], out: &mut [i32; W]) {
    let s = &s[..W + 5];
    for c in 0..W {
        out[c] = tap(
            s[c].i(),
            s[c + 1].i(),
            s[c + 2].i(),
            s[c + 3].i(),
            s[c + 4].i(),
            s[c + 5].i(),
        );
    }
}

/// Six-tap filter down `W` adjacent columns: `out[c]` uses row `k` element `c` of `rows[k]`.
#[inline(always)]
fn vtap<P: Pix, const W: usize>(rows: [&[P]; 6], out: &mut [i32; W]) {
    let [a, b, c_, d, e, f] = rows.map(|r| &r[..W]);
    for c in 0..W {
        out[c] = tap(a[c].i(), b[c].i(), c_[c].i(), d[c].i(), e[c].i(), f[c].i());
    }
}

#[inline(always)]
fn avg(a: i32, b: i32) -> i32 {
    (a + b + 1) >> 1
}

/// Luma prediction of the `w`x`h` block whose top-left sample is at (`x`, `y`), displaced
/// by the quarter-sample vector `mv`, into `dst` (stride `ds`). `w` is 4, 8 or 16.
#[allow(clippy::too_many_arguments)]
#[inline]
pub fn luma<P: Pix>(
    refp: View,
    x: i32,
    y: i32,
    mv: [i16; 2],
    w: usize,
    h: usize,
    dst: &mut [P],
    ds: usize,
    max: i32,
    sc: &mut Scratch<P>,
) {
    luma_plane(refp, 0, x, y, mv, w, h, dst, ds, max, sc);
}

/// [`luma`] of plane `plane` (0 = Y, 1 = Cb, 2 = Cr): 4:4:4 predicts Cb and Cr with the
/// luma interpolation (8.4.2.2).
#[allow(clippy::too_many_arguments)]
#[inline]
pub fn luma_plane<P: Pix>(
    refp: View,
    plane: usize,
    x: i32,
    y: i32,
    mv: [i16; 2],
    w: usize,
    h: usize,
    dst: &mut [P],
    ds: usize,
    max: i32,
    sc: &mut Scratch<P>,
) {
    match w {
        16 => luma_w::<P, 16, 21>(refp, plane, x, y, mv, h, dst, ds, max, sc),
        8 => luma_w::<P, 8, 13>(refp, plane, x, y, mv, h, dst, ds, max, sc),
        _ => {
            debug_assert_eq!(w, 4);
            luma_w::<P, 4, 9>(refp, plane, x, y, mv, h, dst, ds, max, sc)
        }
    }
}

/// `W5` is `W + 5`, the window width.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn luma_w<P: Pix, const W: usize, const W5: usize>(
    refp: View,
    plane: usize,
    x: i32,
    y: i32,
    mv: [i16; 2],
    h: usize,
    dst: &mut [P],
    ds: usize,
    max: i32,
    sc: &mut Scratch<P>,
) {
    let fx = (mv[0] & 3) as usize;
    let fy = (mv[1] & 3) as usize;
    let ix = x + (mv[0] as i32 >> 2);
    let iy = y + (mv[1] as i32 >> 2);
    let clip = |v: i32| P::clip(v, max);
    if fx == 0 && fy == 0 {
        refp.wait_luma((iy + h as i32 - 1) as isize);
        refp.fetch::<P, W>(plane, ix, iy, h, dst, ds);
        return;
    }
    // The window of samples from (ix - 2, iy - 2): G(c, r) = win[(r + 2) * WS + c + 2].
    let Scratch { win, mid, .. } = sc;
    refp.wait_luma((iy + h as i32 + 2) as isize);
    refp.fetch::<P, W5>(plane, ix - 2, iy - 2, h + 5, win, WS);
    let row = |r: usize| &win[r * WS..r * WS + WS];
    let mut t = [0i32; W];
    if fy == 0 {
        // b (and a, c): horizontal half samples, averaged with the full sample to the left
        // or right.
        for r in 0..h {
            let s = row(r + 2);
            htap::<P, W>(s, &mut t);
            let d = &mut dst[r * ds..r * ds + W];
            match fx {
                2 => {
                    for c in 0..W {
                        d[c] = clip((t[c] + 16) >> 5);
                    }
                }
                _ => {
                    let g = &s[fx / 2 + 2..fx / 2 + 2 + W];
                    for c in 0..W {
                        d[c] = P::of(avg(g[c].i(), clip((t[c] + 16) >> 5).i()));
                    }
                }
            }
        }
        return;
    }
    if fx == 0 {
        // h (and d, n): vertical half samples.
        for r in 0..h {
            let rows = [0, 1, 2, 3, 4, 5].map(|k| &row(r + k)[2..]);
            vtap::<P, W>(rows, &mut t);
            let d = &mut dst[r * ds..r * ds + W];
            match fy {
                2 => {
                    for c in 0..W {
                        d[c] = clip((t[c] + 16) >> 5);
                    }
                }
                _ => {
                    let g = &row(r + 2 + fy / 2)[2..2 + W];
                    for c in 0..W {
                        d[c] = P::of(avg(g[c].i(), clip((t[c] + 16) >> 5).i()));
                    }
                }
            }
        }
        return;
    }
    if fx & 1 == 1 && fy & 1 == 1 {
        // e, g, p, r: average of the nearest horizontal and vertical half samples.
        let rb = fy / 2;
        let cb = fx / 2;
        let mut u = [0i32; W];
        for r in 0..h {
            htap::<P, W>(row(r + 2 + rb), &mut t);
            let rows = [0, 1, 2, 3, 4, 5].map(|k| &row(r + k)[2 + cb..]);
            vtap::<P, W>(rows, &mut u);
            let d = &mut dst[r * ds..r * ds + W];
            for c in 0..W {
                d[c] = P::of(avg(clip((t[c] + 16) >> 5).i(), clip((u[c] + 16) >> 5).i()));
            }
        }
        return;
    }
    // The centre sample j from the horizontal intermediates (h + 5 rows, from row -2).
    for r in 0..h + 5 {
        htap::<P, W>(row(r), (&mut mid[r][..W]).try_into().unwrap());
    }
    let mid = &*mid;
    let mut u = [0i32; W];
    for r in 0..h {
        let m: [&[i32; W]; 6] = [0, 1, 2, 3, 4, 5].map(|k| (&mid[r + k][..W]).try_into().unwrap());
        let d = &mut dst[r * ds..r * ds + W];
        let j = |c: usize| {
            let j1 = tap(m[0][c], m[1][c], m[2][c], m[3][c], m[4][c], m[5][c]);
            clip((j1 + 512) >> 10).i()
        };
        match (fx, fy) {
            (2, 2) => {
                for c in 0..W {
                    d[c] = P::of(j(c));
                }
            }
            (2, _) => {
                // f, q: with b of the row above or below j.
                let b = m[2 + fy / 2];
                for c in 0..W {
                    d[c] = P::of(avg(j(c), clip((b[c] + 16) >> 5).i()));
                }
            }
            _ => {
                // i, k: with h of the column left or right of j.
                let rows = [0, 1, 2, 3, 4, 5].map(|k| &row(r + k)[2 + fx / 2..]);
                vtap::<P, W>(rows, &mut u);
                for c in 0..W {
                    d[c] = P::of(avg(j(c), clip((u[c] + 16) >> 5).i()));
                }
            }
        }
    }
}

/// Chroma prediction of the `w`x`h` block at chroma position (`x`, `y`) of both chroma
/// planes with the chroma vector `mv` in eighth-sample units (for 4:2:2 the vertical
/// component is the luma one doubled), into `dst[0]` (Cb) and `dst[1]` (Cr), stride `ds`. `w`
/// is 2, 4 or 8. The planes share the wait, the weights and the line positions.
#[allow(clippy::too_many_arguments)]
#[inline]
pub fn chroma<P: Pix>(
    refp: View,
    x: i32,
    y: i32,
    mv: [i32; 2],
    w: usize,
    h: usize,
    dst: [&mut [P]; 2],
    ds: usize,
    sc: &mut Scratch<P>,
) {
    match w {
        8 => chroma_w::<P, 8, 9>(refp, x, y, mv, h, dst, ds, sc),
        4 => chroma_w::<P, 4, 5>(refp, x, y, mv, h, dst, ds, sc),
        _ => {
            debug_assert_eq!(w, 2);
            chroma_w::<P, 2, 3>(refp, x, y, mv, h, dst, ds, sc)
        }
    }
}

/// `W1` is `W + 1`, the window width.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn chroma_w<P: Pix, const W: usize, const W1: usize>(
    refp: View,
    x: i32,
    y: i32,
    mv: [i32; 2],
    h: usize,
    dst: [&mut [P]; 2],
    ds: usize,
    sc: &mut Scratch<P>,
) {
    let fx = (mv[0] & 7) as u32;
    let fy = (mv[1] & 7) as u32;
    let ix = x + (mv[0] >> 3);
    let iy = y + (mv[1] >> 3);
    if fx == 0 && fy == 0 {
        refp.wait_chroma((iy + h as i32 - 1) as isize);
        for (c, dst) in dst.into_iter().enumerate() {
            refp.fetch::<P, W>(1 + c, ix, iy, h, dst, ds);
        }
        return;
    }
    refp.wait_chroma((iy + h as i32) as isize);
    let (wa, wb, wc, wd) = (
        ((8 - fx) * (8 - fy)) as i32,
        (fx * (8 - fy)) as i32,
        ((8 - fx) * fy) as i32,
        (fx * fy) as i32,
    );
    let win = &mut sc.cwin;
    if sc.cnarrow {
        // Eight 16-bit lanes: one row of 8, two rows of 4, or up to four rows of 2 (chroma
        // heights are even, and a multiple of 4 when blocks are 2 wide and taller than 2).
        let rows = if W == 2 && h == 2 { 2 } else { 8 / W };
        debug_assert!(h.is_multiple_of(rows));
        let wt = [wa, wb, wc, wd].map(|w| Simd::<u16, 8>::splat(w as u16));
        let load = |win: &[P; CS * 17], r: usize, o: usize| {
            let mut l = [P::default(); 8];
            for k in 0..rows {
                l[k * W..(k + 1) * W].copy_from_slice(&win[(r + k) * CS + o..][..W]);
            }
            P::wide8(&l)
        };
        for (c, dst) in dst.into_iter().enumerate() {
            refp.fetch::<P, W1>(1 + c, ix, iy, h + 1, win, CS);
            for r in (0..h).step_by(rows) {
                let v = wt[0] * load(win, r, 0)
                    + wt[1] * load(win, r, 1)
                    + wt[2] * load(win, r + 1, 0)
                    + wt[3] * load(win, r + 1, 1)
                    + Simd::splat(32);
                let o = P::narrow8(v >> 6);
                for k in 0..rows {
                    dst[(r + k) * ds..][..W].copy_from_slice(&o[k * W..(k + 1) * W]);
                }
            }
        }
        return;
    }
    for (c, dst) in dst.into_iter().enumerate() {
        refp.fetch::<P, W1>(1 + c, ix, iy, h + 1, win, CS);
        for r in 0..h {
            let a = &win[r * CS..r * CS + W + 1];
            let b = &win[(r + 1) * CS..(r + 1) * CS + W + 1];
            let d = &mut dst[r * ds..r * ds + W];
            for c in 0..W {
                let v = wa * a[c].i() + wb * a[c + 1].i() + wc * b[c].i() + wd * b[c + 1].i();
                d[c] = P::of((v + 32) >> 6);
            }
        }
    }
}

/// How the one or two predictions of a partition combine (8.4.2.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Weighting {
    Default,
    /// logWD, w0, w1, o0, o1 (explicit, or implicit with logWD 5 and zero offsets).
    Weighted(u32, i32, i32, i32, i32),
}

/// Writes the prediction of a `w`x`h` block into `out` (stride `os`) from `p0`/`p1`
/// (stride `ps`); at least one is present. `w` is 2, 4, 8 or 16.
#[allow(clippy::too_many_arguments)]
#[inline]
pub fn combine<P: Pix>(
    out: &mut [P],
    os: usize,
    p0: Option<&[P]>,
    p1: Option<&[P]>,
    ps: usize,
    w: usize,
    h: usize,
    wt: Weighting,
    max: i32,
) {
    match w {
        16 => combine_w::<P, 16>(out, os, p0, p1, ps, h, wt, max),
        8 => combine_w::<P, 8>(out, os, p0, p1, ps, h, wt, max),
        4 => combine_w::<P, 4>(out, os, p0, p1, ps, h, wt, max),
        _ => {
            debug_assert_eq!(w, 2);
            combine_w::<P, 2>(out, os, p0, p1, ps, h, wt, max)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn combine_w<P: Pix, const W: usize>(
    out: &mut [P],
    os: usize,
    p0: Option<&[P]>,
    p1: Option<&[P]>,
    ps: usize,
    h: usize,
    wt: Weighting,
    max: i32,
) {
    let clip = |v: i32| P::clip(v, max);
    match (p0, p1, wt) {
        (Some(a), Some(b), Weighting::Default) => {
            for r in 0..h {
                let (o, s) = (r * os, r * ps);
                let (a, b) = (&a[s..s + W], &b[s..s + W]);
                let d = &mut out[o..o + W];
                for c in 0..W {
                    d[c] = P::of(avg(a[c].i(), b[c].i()));
                }
            }
        }
        (Some(a), Some(b), Weighting::Weighted(lwd, w0, w1, o0, o1)) => {
            let rnd = 1 << lwd;
            let off = (o0 + o1 + 1) >> 1;
            for r in 0..h {
                let (o, s) = (r * os, r * ps);
                let (a, b) = (&a[s..s + W], &b[s..s + W]);
                let d = &mut out[o..o + W];
                for c in 0..W {
                    let v = ((a[c].i() * w0 + b[c].i() * w1 + rnd) >> (lwd + 1)) + off;
                    d[c] = clip(v);
                }
            }
        }
        (Some(a), None, wt) | (None, Some(a), wt) => {
            let (wgt, o_) = match (wt, p0.is_some()) {
                (Weighting::Weighted(l, w0, _, o0, _), true) => (Some((l, w0)), o0),
                (Weighting::Weighted(l, _, w1, _, o1), false) => (Some((l, w1)), o1),
                _ => (None, 0),
            };
            match wgt {
                None => {
                    for r in 0..h {
                        out[r * os..r * os + W].copy_from_slice(&a[r * ps..r * ps + W]);
                    }
                }
                Some((l, wv)) => {
                    let rnd = if l >= 1 { 1 << (l - 1) } else { 0 };
                    for r in 0..h {
                        let (o, s) = (r * os, r * ps);
                        let a = &a[s..s + W];
                        let d = &mut out[o..o + W];
                        for c in 0..W {
                            let v = ((a[c].i() * wv + rnd) >> l) + o_;
                            d[c] = clip(v);
                        }
                    }
                }
            }
        }
        (None, None, _) => {}
    }
}
