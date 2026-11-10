//! Dequantised coefficients to residual: the 2D inverse transform (section 8.7.2) and the
//! reconstruction (8.6.2), with the 1D transforms of `txfm_gen` run on several rows or
//! columns at once.

use std::ops::{Add, Neg, Sub};
use std::simd::{Simd, num::SimdInt};

use crate::frame::Pixel;
use crate::txfm_gen::*;

/// Lane-wise integer arithmetic for the generated transforms.
pub(crate) trait Lane:
    Copy + Add<Output = Self> + Sub<Output = Self> + Neg<Output = Self>
{
    fn zero() -> Self;
    fn mulc(self, c: i32) -> Self;
    /// Round2( x, 14 ).
    fn r14(self) -> Self;
}

impl<const N: usize> Lane for Simd<i32, N> {
    #[inline(always)]
    fn zero() -> Self {
        Simd::splat(0)
    }
    #[inline(always)]
    fn mulc(self, c: i32) -> Self {
        self * Simd::splat(c)
    }
    #[inline(always)]
    fn r14(self) -> Self {
        (self + Simd::splat(1 << 13)) >> Simd::splat(14)
    }
}

impl<const N: usize> Lane for Simd<i64, N> {
    #[inline(always)]
    fn zero() -> Self {
        Simd::splat(0)
    }
    #[inline(always)]
    fn mulc(self, c: i32) -> Self {
        self * Simd::splat(c as i64)
    }
    #[inline(always)]
    fn r14(self) -> Self {
        (self + Simd::splat(1 << 13)) >> Simd::splat(14)
    }
}

/// Conversion between lanes and `i32` coefficients.
pub(crate) trait Io<const N: usize>: Lane {
    fn load(s: &[i32]) -> Self;
    fn from_arr(a: [i32; N]) -> Self;
    fn to_arr(self) -> [i32; N];
}

impl<const N: usize> Io<N> for Simd<i32, N> {
    #[inline(always)]
    fn load(s: &[i32]) -> Self {
        Simd::from_slice(&s[..N])
    }
    #[inline(always)]
    fn from_arr(a: [i32; N]) -> Self {
        Simd::from_array(a)
    }
    #[inline(always)]
    fn to_arr(self) -> [i32; N] {
        self.to_array()
    }
}

impl<const N: usize> Io<N> for Simd<i64, N> {
    #[inline(always)]
    fn load(s: &[i32]) -> Self {
        Simd::<i32, N>::from_slice(&s[..N]).cast()
    }
    #[inline(always)]
    fn from_arr(a: [i32; N]) -> Self {
        Simd::<i32, N>::from_array(a).cast()
    }
    #[inline(always)]
    fn to_arr(self) -> [i32; N] {
        self.cast::<i32>().to_array()
    }
}

pub(crate) const DCT_DCT: u8 = 0;
pub(crate) const ADST_DCT: u8 = 1;
pub(crate) const DCT_ADST: u8 = 2;

/// Where the residual goes: rows of a plane, columns from `x`.
pub(crate) struct Dst<'r, 'a, P> {
    pub rows: &'r mut [&'a mut [P]],
    pub x: usize,
    pub y: usize,
}

#[inline(always)]
fn add_px<P: Pixel>(d: &mut P, v: i32, max: i32) {
    *d = P::put((d.get() + v).clamp(0, max));
}

macro_rules! tx1d {
    ($n:expr, $row:expr, $adst:expr, $t:expr) => {
        match ($n, $adst) {
            (4, false) => idct4(($t).as_mut_slice().try_into().unwrap()),
            (4, true) => iadst4(($t).as_mut_slice().try_into().unwrap()),
            (8, false) => idct8(($t).as_mut_slice().try_into().unwrap()),
            (8, true) => iadst8(($t).as_mut_slice().try_into().unwrap()),
            (16, false) => idct16(($t).as_mut_slice().try_into().unwrap()),
            (16, true) => iadst16(($t).as_mut_slice().try_into().unwrap()),
            _ => idct32(($t).as_mut_slice().try_into().unwrap()),
        }
    };
}

/// The 2D inverse transform of an n0 x n0 block with lanes of `L`: rows `0..rows` may be
/// non-zero. Adds the result to `dst` and clears `coef`.
#[inline(always)]
fn tx2d<V: Io<L>, P: Pixel, const L: usize, const N0: usize>(
    coef: &mut [i32],
    tx_type: u8,
    rows: usize,
    dst: &mut Dst<P>,
    max: i32,
) {
    let row_adst = tx_type == DCT_ADST || tx_type == 3;
    let col_adst = tx_type == ADST_DCT || tx_type == 3;
    let shift = match N0 {
        4 => 4,
        8 => 5,
        _ => 6,
    };
    let mut t = [V::zero(); N0];
    // Row transforms, L rows at a time.
    let mut i0 = 0;
    while i0 < rows {
        for (j, tj) in t.iter_mut().enumerate() {
            let mut a = [0i32; L];
            for (k, ak) in a.iter_mut().enumerate() {
                *ak = coef[(i0 + k) * N0 + j];
            }
            *tj = V::from_arr(a);
        }
        tx1d!(N0, true, row_adst, t);
        for (j, tj) in t.iter().enumerate() {
            let a = tj.to_arr();
            for (k, ak) in a.iter().enumerate() {
                coef[(i0 + k) * N0 + j] = *ak;
            }
        }
        i0 += L;
    }
    // Column transforms, L columns at a time, then rounding and reconstruction.
    let rnd = 1 << (shift - 1);
    let mut j0 = 0;
    while j0 < N0 {
        for (i, ti) in t.iter_mut().enumerate() {
            *ti = V::load(&coef[i * N0 + j0..]);
        }
        tx1d!(N0, false, col_adst, t);
        for (i, ti) in t.iter().enumerate() {
            let a = ti.to_arr();
            let row = &mut dst.rows[dst.y + i][dst.x + j0..dst.x + j0 + L];
            for (d, v) in row.iter_mut().zip(a) {
                add_px(d, (v.wrapping_add(rnd)) >> shift, max);
            }
        }
        j0 += L;
    }
    coef[..N0 * N0].fill(0);
}

/// Inverse Walsh-Hadamard transform (8.7.1.10) of a 4x4 block, added to `dst`.
fn wht<P: Pixel>(coef: &mut [i32], dst: &mut Dst<P>, max: i32) {
    fn one(t: &mut [i32; 4], shift: u32) {
        let mut a = t[0] >> shift;
        let mut c = t[1] >> shift;
        let mut d = t[2] >> shift;
        let mut b = t[3] >> shift;
        a = a.wrapping_add(c);
        d = d.wrapping_sub(b);
        let e = (a.wrapping_sub(d)) >> 1;
        b = e.wrapping_sub(b);
        c = e.wrapping_sub(c);
        a = a.wrapping_sub(b);
        d = d.wrapping_add(c);
        *t = [a, b, c, d];
    }
    for i in 0..4 {
        let mut t = [
            coef[i * 4],
            coef[i * 4 + 1],
            coef[i * 4 + 2],
            coef[i * 4 + 3],
        ];
        one(&mut t, 2);
        coef[i * 4..i * 4 + 4].copy_from_slice(&t);
    }
    for j in 0..4 {
        let mut t = [coef[j], coef[4 + j], coef[8 + j], coef[12 + j]];
        one(&mut t, 0);
        for (i, v) in t.iter().enumerate() {
            add_px(&mut dst.rows[dst.y + i][dst.x + j], *v, max);
        }
    }
    coef[..16].fill(0);
}

/// Reconstructs a transform block: `coef` holds the dequantised coefficients (row-major,
/// n0 x n0), `eob` the number of coefficients read and `rows` one more than the last row
/// holding a non-zero coefficient. `coef` is all zeros afterwards.
#[allow(clippy::too_many_arguments)]
pub(crate) fn reconstruct<P: Pixel>(
    coef: &mut [i32],
    tx_sz: usize,
    tx_type: u8,
    lossless: bool,
    eob: usize,
    rows: usize,
    dst: &mut Dst<P>,
    bit_depth: u8,
) {
    let max = (1 << bit_depth) - 1;
    if lossless {
        wht(coef, dst, max);
        return;
    }
    let n0 = 4 << tx_sz;
    if eob == 1 && tx_type == DCT_DCT {
        // Only the DC coefficient: every row, then every column, is a constant.
        let r14 = |v: i64| (v * 11585 + (1 << 13)) >> 14;
        let shift = if tx_sz == 0 {
            4
        } else if tx_sz == 1 {
            5
        } else {
            6
        };
        let v = r14(r14(coef[0] as i64));
        let v = ((v + (1 << (shift - 1))) >> shift) as i32;
        coef[0] = 0;
        for i in 0..n0 {
            for d in &mut dst.rows[dst.y + i][dst.x..dst.x + n0] {
                add_px(d, v, max);
            }
        }
        return;
    }
    if !P::HBD {
        match tx_sz {
            0 => tx2d::<Simd<i32, 4>, P, 4, 4>(coef, tx_type, 4, dst, max),
            1 => tx2d::<Simd<i32, 8>, P, 8, 8>(coef, tx_type, rows.div_ceil(8) * 8, dst, max),
            2 => tx2d::<Simd<i32, 8>, P, 8, 16>(coef, tx_type, rows.div_ceil(8) * 8, dst, max),
            _ => tx2d::<Simd<i32, 8>, P, 8, 32>(coef, tx_type, rows.div_ceil(8) * 8, dst, max),
        }
    } else {
        match tx_sz {
            0 => tx2d::<Simd<i64, 4>, P, 4, 4>(coef, tx_type, 4, dst, max),
            1 => tx2d::<Simd<i64, 8>, P, 8, 8>(coef, tx_type, rows.div_ceil(8) * 8, dst, max),
            2 => tx2d::<Simd<i64, 8>, P, 8, 16>(coef, tx_type, rows.div_ceil(8) * 8, dst, max),
            _ => tx2d::<Simd<i64, 8>, P, 8, 32>(coef, tx_type, rows.div_ceil(8) * 8, dst, max),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The inverse DCT of a unit impulse follows the cosine basis.
    #[test]
    fn idct_basis() {
        for (n, log2) in [(4usize, 2u32), (8, 3), (16, 4), (32, 5)] {
            for k in 0..n {
                let mut t = vec![Simd::<i64, 1>::splat(0); n];
                t[k] = Simd::splat(1 << 12);
                match n {
                    4 => idct4(t.as_mut_slice().try_into().unwrap()),
                    8 => idct8(t.as_mut_slice().try_into().unwrap()),
                    16 => idct16(t.as_mut_slice().try_into().unwrap()),
                    _ => idct32(t.as_mut_slice().try_into().unwrap()),
                }
                for (i, v) in t.iter().enumerate() {
                    let c = if k == 0 {
                        std::f64::consts::FRAC_1_SQRT_2
                    } else {
                        (std::f64::consts::PI * (2 * i + 1) as f64 * k as f64 / (2 * n) as f64)
                            .cos()
                    };
                    let want = 4096.0 * c;
                    let got = v[0] as f64;
                    assert!((got - want).abs() < 2.0 + log2 as f64, "n={n} k={k} i={i}");
                }
            }
        }
    }

    /// The inverse ADSTs are orthogonal: the impulse responses have equal energy and are
    /// mutually orthogonal (up to rounding).
    #[test]
    fn iadst_orthogonal() {
        for n in [4usize, 8, 16] {
            let mut basis = Vec::new();
            for k in 0..n {
                let mut t = vec![Simd::<i64, 1>::splat(0); n];
                t[k] = Simd::splat(1 << 14);
                match n {
                    4 => iadst4(t.as_mut_slice().try_into().unwrap()),
                    8 => iadst8(t.as_mut_slice().try_into().unwrap()),
                    _ => iadst16(t.as_mut_slice().try_into().unwrap()),
                }
                basis.push(t.iter().map(|v| v[0] as f64 / 16384.0).collect::<Vec<_>>());
            }
            let e0: f64 = basis[0].iter().map(|v| v * v).sum();
            for a in 0..n {
                for b in 0..n {
                    let dot: f64 = (0..n).map(|i| basis[a][i] * basis[b][i]).sum::<f64>() / e0;
                    let want = if a == b { 1.0 } else { 0.0 };
                    assert!((dot - want).abs() < 0.02, "n={n} a={a} b={b} dot={dot}");
                }
            }
        }
    }
}
