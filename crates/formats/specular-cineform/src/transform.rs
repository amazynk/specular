//! The inverse wavelets: the spatial 2/6 wavelet (vertical, then horizontal), the frame wavelet
//! of interlaced pictures (horizontal, then a sum and difference of the two fields) and the
//! temporal wavelet between the two frames of a group.
//!
//! Every stage keeps 16-bit coefficients (saturating), as the format does. The 2/6 filter
//! predicts each output pair from three lowpass values (`(l[i-1] − l[i+1] + 4) >> 3`, with
//! special taps `11, −4, 1` and `5, 4, −1` at the edges) and adds or subtracts the highpass
//! value. The vertical stage halves its results; the horizontal stage halves them too, except
//! in a prescaled wavelet, which undoes the encoder's prescaling (a doubling from 10 bits up).

use rayon::prelude::*;

#[inline(always)]
pub(crate) fn sat(v: i32) -> i16 {
    v.clamp(i16::MIN as i32, i16::MAX as i32) as i16
}

/// The even and odd outputs at position `i` of a line of `n` lowpass values and the highpass
/// value `hi` (before any final shift).
#[inline(always)]
fn pair(l: impl Fn(usize) -> i32, n: usize, i: usize, hi: i32) -> (i32, i32) {
    if n < 3 {
        // Too short for the filter: repeat the value.
        let v = l(i.min(n - 1));
        return (v + hi, v - hi);
    }
    if i == 0 {
        (
            ((11 * l(0) - 4 * l(1) + l(2) + 4) >> 3) + hi,
            ((5 * l(0) + 4 * l(1) - l(2) + 4) >> 3) - hi,
        )
    } else if i + 1 == n {
        (
            ((5 * l(i) + 4 * l(i - 1) - l(i - 2) + 4) >> 3) + hi,
            ((11 * l(i) - 4 * l(i - 1) + l(i - 2) + 4) >> 3) - hi,
        )
    } else {
        (
            ((l(i - 1) - l(i + 1) + 4) >> 3) + l(i) + hi,
            ((l(i + 1) - l(i - 1) + 4) >> 3) + l(i) - hi,
        )
    }
}

/// Four bands of `w`×`h` (LL, LH, HL, HH, row-major).
pub(crate) struct Bands<'a> {
    pub w: usize,
    pub h: usize,
    pub b: [&'a [i16]; 4],
}

/// How the horizontal stage ends.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Scale {
    /// Halve (an ordinary wavelet).
    Half,
    /// Keep (a prescaled wavelet at 8 bits).
    Keep,
    /// Double (a prescaled wavelet from 10 bits up).
    Double,
}

/// The vertical stage for output rows `2r` and `2r + 1`: the horizontal lowpass (from LL, HL)
/// and highpass (from LH, HH) lines.
fn vertical(s: &Bands, r: usize, lo: &mut [[i16; 2]], hi: &mut [[i16; 2]]) {
    let (w, h) = (s.w, s.h);
    let [ll, lh, hl, hh] = s.b;
    for c in 0..w {
        let (e, o) = pair(|y| i32::from(ll[y * w + c]), h, r, i32::from(hl[r * w + c]));
        lo[c] = [sat(e >> 1), sat(o >> 1)];
        let (e, o) = pair(|y| i32::from(lh[y * w + c]), h, r, i32::from(hh[r * w + c]));
        hi[c] = [sat(e >> 1), sat(o >> 1)];
    }
}

/// Inverts a spatial wavelet into `ow`×`oh` (at most twice the band size).
pub(crate) fn spatial(s: &Bands, ow: usize, oh: usize, scale: Scale) -> Vec<i16> {
    let w = s.w;
    let mut out = vec![0i16; ow * oh];
    if w == 0 || s.h == 0 || ow == 0 {
        return out;
    }
    let rows = |(r, dst): (usize, &mut [i16])| {
        let mut lo = vec![[0i16; 2]; w];
        let mut hi = vec![[0i16; 2]; w];
        vertical(s, r, &mut lo, &mut hi);
        for (k, line) in dst.chunks_mut(ow).enumerate() {
            for c in 0..w {
                let (e, o) = pair(|x| i32::from(lo[x][k]), w, c, i32::from(hi[c][k]));
                let fin = |v: i32| match scale {
                    Scale::Half => sat(v >> 1),
                    Scale::Keep => sat(v),
                    Scale::Double => sat(v << 1),
                };
                if 2 * c < ow {
                    line[2 * c] = fin(e);
                }
                if 2 * c + 1 < ow {
                    line[2 * c + 1] = fin(o);
                }
            }
        }
    };
    out.par_chunks_mut(2 * ow)
        .take(s.h)
        .enumerate()
        .for_each(rows);
    out
}

/// C division by two (towards zero).
#[inline(always)]
fn half(v: i32) -> i32 {
    v / 2
}

/// Inverts a frame wavelet (an interlaced picture: the horizontal inverse of the field sum
/// and difference, then the two fields) into `ow`×`oh` samples clipped to `0..=max`.
pub(crate) fn frame(s: &Bands, ow: usize, oh: usize, max: i32) -> Vec<i16> {
    let (w, h) = (s.w, s.h);
    let [ll, lh, hl, hh] = s.b;
    let mut out = vec![0i16; ow * oh];
    if w == 0 || h == 0 || ow == 0 {
        return out;
    }
    let rows = |(r, dst): (usize, &mut [i16])| {
        let line = |a: &[i16], b: &[i16], x: usize| -> (i32, i32) {
            pair(|i| i32::from(a[r * w + i]), w, x, i32::from(b[r * w + x]))
        };
        let mut low = vec![0i32; 2 * w];
        let mut high = vec![0i32; 2 * w];
        for x in 0..w {
            let (e, o) = line(ll, lh, x);
            low[2 * x] = i32::from(sat(e >> 1));
            low[2 * x + 1] = i32::from(sat(o >> 1));
            let (e, o) = line(hl, hh, x);
            high[2 * x] = i32::from(sat(e >> 1));
            high[2 * x + 1] = i32::from(sat(o >> 1));
        }
        let (even, odd) = dst.split_at_mut(ow.min(dst.len()));
        for x in 0..ow.min(2 * w) {
            even[x] = half(low[x] - high[x]).clamp(0, max) as i16;
            if x < odd.len() {
                odd[x] = half(low[x] + high[x]).clamp(0, max) as i16;
            }
        }
    };
    out.par_chunks_mut(2 * ow)
        .take(h)
        .enumerate()
        .for_each(rows);
    out
}

/// The temporal inverse: the first and second frame's lowpass bands from the temporal sum and
/// difference. 8-bit groups add a checkerboard of rounding to the second frame.
pub(crate) fn temporal(low: &[i16], high: &[i16], w: usize, dither: bool) -> (Vec<i16>, Vec<i16>) {
    let mut f0 = vec![0i16; low.len()];
    let mut f1 = vec![0i16; low.len()];
    for (i, (&l, &hv)) in low.iter().zip(high).enumerate() {
        let (l, hv) = (i32::from(l), i32::from(hv));
        f0[i] = sat((l - hv) >> 1);
        let d = if dither && w > 0 {
            ((i / w + i % w) & 1) as i32
        } else {
            0
        };
        f1[i] = sat((l + hv + d) >> 1);
    }
    (f0, f1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_bands_invert_to_flat() {
        let (w, h) = (5, 4);
        let ll = vec![200i16; w * h];
        let z = vec![0i16; w * h];
        let s = Bands {
            w,
            h,
            b: [&ll, &z, &z, &z],
        };
        // Two halvings of a flat lowpass.
        assert!(spatial(&s, 10, 8, Scale::Half).iter().all(|&v| v == 50));
        assert!(spatial(&s, 9, 7, Scale::Double).iter().all(|&v| v == 200));
        // Field sum 200, no difference: both fields 50.
        assert!(frame(&s, 10, 8, 255).iter().all(|&v| v == 50));
        let (a, b) = temporal(&[10, 10], &[4, -4], 2, false);
        assert_eq!((a, b), (vec![3, 7], vec![7, 3]));
    }

    #[test]
    fn tiny_bands_do_not_panic() {
        let one = vec![7i16; 1];
        let s = Bands {
            w: 1,
            h: 1,
            b: [&one, &one, &one, &one],
        };
        assert_eq!(spatial(&s, 2, 2, Scale::Half).len(), 4);
        assert_eq!(frame(&s, 2, 2, 1023).len(), 4);
    }
}
