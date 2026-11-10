//! Inverse discrete wavelet transform (Annex F): reversible 5/3 (integer) and irreversible 9/7
//! (float) lifting, applied to windows of a resolution level.
//!
//! A level is synthesised from a buffer in *quadrant layout* (low columns first, then high
//! columns; low rows first, then high rows), which is what the code-block scatter produces.
//! The horizontal pass works in place, row-parallel; the vertical pass writes the natural layout
//! into a second buffer, in parallel row bands that each recompute a few margin rows (the lifting
//! steps only reach `MARGIN` samples, so bands are independent).

use rayon::prelude::*;

pub(crate) trait Coef: Copy + Default + Send + Sync + 'static {
    /// Number of lifting steps: how far a wrong boundary value can propagate.
    const MARGIN: u32;
    /// Lifting synthesis on padded low/high arrays: `lp` holds `nl + 2` rows of `cw` lanes
    /// (rows 0 and nl + 1 are padding), `hp` likewise. `odd`: the first sample is high-pass.
    fn synth(lp: &mut [Self], hp: &mut [Self], nl: usize, nh: usize, cw: usize, odd: bool);
    /// A signal of one odd-positioned (high-pass) sample.
    fn single_odd(v: Self) -> Self;
}

#[inline]
fn pad(v: &mut [impl Copy], n: usize, cw: usize) {
    v.copy_within(cw..2 * cw, 0);
    v.copy_within(n * cw..(n + 1) * cw, (n + 1) * cw);
}

/// `dst[cw + i] op= f(src[a * cw + i], src[(a + 1) * cw + i])` for `i < n * cw`.
#[inline(always)]
fn step<T: Copy>(
    dst: &mut [T],
    src: &[T],
    n: usize,
    cw: usize,
    a: usize,
    f: impl Fn(T, T, T) -> T,
) {
    let len = n * cw;
    let d = &mut dst[cw..cw + len];
    let s0 = &src[a * cw..a * cw + len];
    let s1 = &src[(a + 1) * cw..(a + 1) * cw + len];
    for ((d, &x), &y) in d.iter_mut().zip(s0).zip(s1) {
        *d = f(*d, x, y);
    }
}

impl Coef for i32 {
    const MARGIN: u32 = 2;
    fn synth(lp: &mut [i32], hp: &mut [i32], nl: usize, nh: usize, cw: usize, odd: bool) {
        let (al, ah) = if odd { (1, 0) } else { (0, 1) };
        pad(hp, nh, cw);
        step(lp, hp, nl, cw, al, |l, a, b| {
            l.wrapping_sub((a.wrapping_add(b).wrapping_add(2)) >> 2)
        });
        pad(lp, nl, cw);
        step(hp, lp, nh, cw, ah, |h, a, b| {
            h.wrapping_add(a.wrapping_add(b) >> 1)
        });
    }
    fn single_odd(v: i32) -> i32 {
        v / 2
    }
}

/// The 5/3 kernel on 64 bits, for components too deep for `i32` (32-bit samples).
impl Coef for i64 {
    const MARGIN: u32 = 2;
    fn synth(lp: &mut [i64], hp: &mut [i64], nl: usize, nh: usize, cw: usize, odd: bool) {
        let (al, ah) = if odd { (1, 0) } else { (0, 1) };
        pad(hp, nh, cw);
        step(lp, hp, nl, cw, al, |l, a, b| {
            l.wrapping_sub((a.wrapping_add(b).wrapping_add(2)) >> 2)
        });
        pad(lp, nl, cw);
        step(hp, lp, nh, cw, ah, |h, a, b| {
            h.wrapping_add(a.wrapping_add(b) >> 1)
        });
    }
    fn single_odd(v: i64) -> i64 {
        v / 2
    }
}

const ALPHA: f32 = -1.586_134_3;
const BETA: f32 = -0.052_980_118;
const GAMMA: f32 = 0.882_911_1;
const DELTA: f32 = 0.443_506_85;
const KAPPA: f32 = 1.230_174_1;

impl Coef for f32 {
    const MARGIN: u32 = 4;
    fn synth(lp: &mut [f32], hp: &mut [f32], nl: usize, nh: usize, cw: usize, odd: bool) {
        let (al, ah) = if odd { (1, 0) } else { (0, 1) };
        for v in &mut lp[cw..(nl + 1) * cw] {
            *v *= KAPPA;
        }
        for v in &mut hp[cw..(nh + 1) * cw] {
            *v *= 1.0 / KAPPA;
        }
        pad(hp, nh, cw);
        step(lp, hp, nl, cw, al, |l, a, b| l - DELTA * (a + b));
        pad(lp, nl, cw);
        step(hp, lp, nh, cw, ah, |h, a, b| h - GAMMA * (a + b));
        pad(hp, nh, cw);
        step(lp, hp, nl, cw, al, |l, a, b| l - BETA * (a + b));
        pad(lp, nl, cw);
        step(hp, lp, nh, cw, ah, |h, a, b| h - ALPHA * (a + b));
    }
    /// T.800 F.3.7: a lone odd sample was coded as twice its value, for both kernels
    /// (Kakadu's reference decodes, OpenJPEG's and OpenJPH's decoders agree; OpenJPEG's
    /// encoder omits the doubling for 9/7).
    fn single_odd(v: f32) -> f32 {
        v * 0.5
    }
}

/// Number of low (even) positions in `[p0, p0 + n)`.
#[inline]
fn n_low(p0: u32, n: usize) -> usize {
    ((p0 as usize + n).div_ceil(2)) - (p0 as usize).div_ceil(2)
}

/// `dst.copy_from_slice(src)`, inlined as fixed-size moves for full strips.
#[inline(always)]
fn copy_lanes<T: Copy>(dst: &mut [T], src: &[T]) {
    match (
        <&mut [T; STRIP]>::try_from(&mut *dst),
        <&[T; STRIP]>::try_from(src),
    ) {
        (Ok(d), Ok(s)) => *d = *s,
        _ => dst.copy_from_slice(src),
    }
}

/// Column strip width of the vertical pass.
const STRIP: usize = 64;

/// Horizontal synthesis of one row in place (quadrant -> interleaved).
fn row_h<T: Coef>(row: &mut [T], x0: u32, lp: &mut Vec<T>, hp: &mut Vec<T>) {
    let n = row.len();
    if n == 0 {
        return;
    }
    let odd = x0 & 1 == 1;
    if n == 1 {
        if odd {
            row[0] = T::single_odd(row[0]);
        }
        return;
    }
    let nl = n_low(x0, n);
    let nh = n - nl;
    lp.clear();
    lp.resize(nl + 2, T::default());
    hp.clear();
    hp.resize(nh + 2, T::default());
    lp[1..=nl].copy_from_slice(&row[..nl]);
    hp[1..=nh].copy_from_slice(&row[nl..]);
    T::synth(lp, hp, nl, nh, 1, odd);
    let (ev, od) = if odd {
        (&hp[1..], &lp[1..])
    } else {
        (&lp[1..], &hp[1..])
    };
    let mut pairs = row.chunks_exact_mut(2);
    for ((p, &e), &o) in (&mut pairs).zip(ev).zip(od) {
        p[0] = e;
        p[1] = o;
    }
    if let ([last], Some(&e)) = (pairs.into_remainder(), ev.get(n / 2)) {
        *last = e;
    }
}

/// Synthesise one level: `a` (quadrant layout, `w x h`, window origin `(x0, y0)` in this
/// resolution's coordinates) -> `out` (natural layout). `a` is clobbered.
pub(crate) fn idwt_level<T: Coef>(
    a: &mut [T],
    w: usize,
    h: usize,
    x0: u32,
    y0: u32,
    out: &mut [T],
) {
    if w == 0 || h == 0 {
        return;
    }
    // Horizontal pass, in place.
    let rows_per = (16384 / w).clamp(1, 64);
    a[..w * h].par_chunks_mut(w * rows_per).for_each(|chunk| {
        let mut lp = Vec::new();
        let mut hp = Vec::new();
        for row in chunk.chunks_mut(w) {
            row_h(row, x0, &mut lp, &mut hp);
        }
    });
    // Vertical pass into `out`, by row bands.
    let a = &a[..w * h];
    if h == 1 {
        out[..w].copy_from_slice(a);
        if y0 & 1 == 1 {
            for v in &mut out[..w] {
                *v = T::single_odd(*v);
            }
        }
        return;
    }
    let nl_all = n_low(y0, h);
    let lo0 = (y0 as usize).div_ceil(2);
    let hi0 = y0 as usize / 2;
    // Row bands (parallel) split into column strips, so the lifting buffers stay in cache.
    let band = 128usize;
    let m = T::MARGIN as usize;
    out[..w * h]
        .par_chunks_mut(w * band)
        .enumerate()
        .for_each(|(bi, ochunk)| {
            let ya = bi * band;
            let yb = (ya + band).min(h);
            let ra = ya.saturating_sub(m);
            let rb = (yb + m).min(h);
            let pa = y0 as usize + ra;
            let pb = y0 as usize + rb;
            let (l0, l1) = (pa.div_ceil(2), pb.div_ceil(2));
            let (h0, h1) = (pa / 2, pb / 2);
            let (nl, nh) = (l1 - l0, h1 - h0);
            let odd = pa & 1 == 1;
            if rb - ra == 1 {
                // Only possible when h == 1, handled above.
                return;
            }
            let mut lp = vec![T::default(); (nl + 2) * STRIP];
            let mut hp = vec![T::default(); (nh + 2) * STRIP];
            let mut xs = 0;
            while xs < w {
                let cw = STRIP.min(w - xs);
                for k in 0..nl {
                    let src = (l0 + k - lo0) * w + xs;
                    copy_lanes(&mut lp[(k + 1) * cw..(k + 2) * cw], &a[src..src + cw]);
                }
                for k in 0..nh {
                    let src = (nl_all + h0 + k - hi0) * w + xs;
                    copy_lanes(&mut hp[(k + 1) * cw..(k + 2) * cw], &a[src..src + cw]);
                }
                T::synth(&mut lp, &mut hp, nl, nh, cw, odd);
                for y in ya..yb {
                    let j = y - ra; // local index in the sub-problem
                    let src = if (j & 1 == 0) != odd { &lp } else { &hp };
                    let k = j / 2 + 1;
                    let o = (y - ya) * w + xs;
                    copy_lanes(&mut ochunk[o..o + cw], &src[k * cw..(k + 1) * cw]);
                }
                xs += cw;
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Forward 5/3 (reversible) of an interleaved 1D signal starting at `p0`; returns quadrant
    /// layout (lows then highs).
    fn fwd53(x: &[i32], p0: u32) -> Vec<i32> {
        let n = x.len();
        let ext = |i: isize| -> i32 {
            let n = n as isize;
            let mut i = i;
            while i < 0 || i >= n {
                if i < 0 {
                    i = -i;
                }
                if i >= n {
                    i = 2 * (n - 1) - i;
                }
            }
            x[i as usize]
        };
        let mut y = vec![0i32; n];
        let even = |j: usize| (p0 as usize + j).is_multiple_of(2);
        // high first
        let hi = |j: isize| ext(j) - ((ext(j - 1) + ext(j + 1)) >> 1);
        #[allow(clippy::needless_range_loop)] // the index also feeds closures/other arrays
        for j in 0..n {
            if !even(j) {
                y[j] = hi(j as isize);
            }
        }
        let hext = |j: isize| -> i32 {
            // high values at odd positions with symmetric extension of the full signal
            let n2 = n as isize;
            let mut i = j;
            while i < 0 || i >= n2 {
                if i < 0 {
                    i = -i;
                }
                if i >= n2 {
                    i = 2 * (n2 - 1) - i;
                }
            }
            hi(i)
        };
        #[allow(clippy::needless_range_loop)] // the index also feeds closures/other arrays
        for j in 0..n {
            if even(j) {
                y[j] = ext(j as isize) + ((hext(j as isize - 1) + hext(j as isize + 1) + 2) >> 2);
            }
        }
        let mut q: Vec<i32> = (0..n).filter(|&j| even(j)).map(|j| y[j]).collect();
        q.extend((0..n).filter(|&j| !even(j)).map(|j| y[j]));
        q
    }

    #[test]
    fn inverse53_roundtrip_1d() {
        for p0 in 0..2u32 {
            for n in 2..20usize {
                let x: Vec<i32> = (0..n as i32).map(|i| (i * 37 + 11) % 23 - 9).collect();
                let mut q = fwd53(&x, p0);
                let mut lp = Vec::new();
                let mut hp = Vec::new();
                row_h(&mut q, p0, &mut lp, &mut hp);
                assert_eq!(q, x, "p0={p0} n={n}");
            }
        }
    }

    #[test]
    fn vertical_matches_horizontal() {
        // A column transform done by idwt_level must equal the 1D row transform.
        for y0 in 0..2u32 {
            for h in 2..40usize {
                let x: Vec<i32> = (0..h as i32).map(|i| (i * 53 + 7) % 31 - 15).collect();
                let q = fwd53(&x, y0);
                // w = 1 column; horizontal pass is a no-op on a 1-wide signal at even x0.
                let mut a = q.clone();
                let mut out = vec![0i32; h];
                idwt_level(&mut a, 1, h, 0, y0, &mut out);
                assert_eq!(out, x, "y0={y0} h={h}");
            }
        }
    }
}
