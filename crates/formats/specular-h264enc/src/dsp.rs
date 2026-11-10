//! Block distortion measures.
//!
//! The Hadamard sums work on 16 lanes of `i16`: the transform's butterflies pair lanes
//! (`x ^ 1`, `x ^ 2`, …) or whole rows, and the last stage is left out because
//! `|a + b| + |a − b| = 2·max(|a|, |b|)`: each lane takes the larger magnitude of itself and
//! its last-stage partner, which counts every pair twice and so sums to exactly the
//! transform's absolute sum.

use std::simd::cmp::SimdOrd;
use std::simd::num::{SimdInt, SimdUint};
use std::simd::{Simd, simd_swizzle};

type I16x16 = Simd<i16, 16>;
type I16x8 = Simd<i16, 8>;

/// Sum of absolute differences of a 16×16 block.
#[inline]
pub fn sad16(a: &[u8], astride: usize, b: &[u8], bstride: usize) -> u32 {
    // Per lane at most 16 · 255, so u16 lanes hold the column sums and their total.
    let mut acc = Simd::<u16, 16>::splat(0);
    for y in 0..16 {
        let ra = Simd::<u8, 16>::from_slice(&a[y * astride..y * astride + 16]);
        let rb = Simd::<u8, 16>::from_slice(&b[y * bstride..y * bstride + 16]);
        acc += (ra.simd_max(rb) - ra.simd_min(rb)).cast::<u16>();
    }
    u32::from(acc.reduce_sum())
}

/// Differences of 16 pixels as lanes.
#[inline(always)]
fn diff16(a: &[u8; 16], b: &[u8; 16]) -> I16x16 {
    Simd::<u8, 16>::from_array(*a).cast::<i16>() - Simd::<u8, 16>::from_array(*b).cast::<i16>()
}

/// Signs of the butterflies: +1 where a lane takes the sum, −1 where it takes the difference
/// (the partner's value minus its own).
const SIGN1: I16x16 = Simd::from_array([1, -1, 1, -1, 1, -1, 1, -1, 1, -1, 1, -1, 1, -1, 1, -1]);
const SIGN2: I16x16 = Simd::from_array([1, 1, -1, -1, 1, 1, -1, -1, 1, 1, -1, -1, 1, 1, -1, -1]);
const SIGN4: I16x16 = Simd::from_array([1, 1, 1, 1, -1, -1, -1, -1, 1, 1, 1, 1, -1, -1, -1, -1]);

/// Butterflies between lanes `x` and `x ^ 1`: sums in the lower, differences in the upper.
#[inline(always)]
fn bfly1(v: I16x16) -> I16x16 {
    let p = simd_swizzle!(v, [1, 0, 3, 2, 5, 4, 7, 6, 9, 8, 11, 10, 13, 12, 15, 14]);
    p + v * SIGN1
}

#[inline(always)]
fn bfly2(v: I16x16) -> I16x16 {
    let p = simd_swizzle!(v, [2, 3, 0, 1, 6, 7, 4, 5, 10, 11, 8, 9, 14, 15, 12, 13]);
    p + v * SIGN2
}

#[inline(always)]
fn bfly4(v: I16x16) -> I16x16 {
    let p = simd_swizzle!(v, [4, 5, 6, 7, 0, 1, 2, 3, 12, 13, 14, 15, 8, 9, 10, 11]);
    p + v * SIGN4
}

/// The left-out last stage between lanes `x` and `x ^ 2`.
#[inline(always)]
fn last2(v: I16x16) -> I16x16 {
    let p = simd_swizzle!(v, [2, 3, 0, 1, 6, 7, 4, 5, 10, 11, 8, 9, 14, 15, 12, 13]);
    v.abs().simd_max(p.abs())
}

#[inline(always)]
fn last8(v: I16x16) -> I16x16 {
    let p = simd_swizzle!(v, [8, 9, 10, 11, 12, 13, 14, 15, 0, 1, 2, 3, 4, 5, 6, 7]);
    v.abs().simd_max(p.abs())
}

/// Lane sum (lanes hold magnitudes up to 4·4·2040 here).
#[inline(always)]
fn hsum(v: I16x16) -> u32 {
    v.cast::<u16>().cast::<u32>().reduce_sum()
}

/// Twice the SATD of four rows of 16 lanes (each four lanes one 4×4 block's row), per lane
/// (magnitudes up to 4·2040).
#[inline(always)]
fn satd_rows(r: [I16x16; 4]) -> I16x16 {
    let (s01, d01) = (r[0] + r[1], r[0] - r[1]);
    let (s23, d23) = (r[2] + r[3], r[2] - r[3]);
    let v = [s01 + s23, s01 - s23, d01 - d23, d01 + d23];
    let m = v.map(|v| last2(bfly1(v)));
    m[0] + m[1] + m[2] + m[3]
}

/// Sum of absolute Hadamard-transformed differences of a 4×4 block, halved.
#[inline]
pub fn satd4(a: &[u8], astride: usize, b: &[u8], bstride: usize) -> u32 {
    let mut x = [0u8; 16];
    let mut y = [0u8; 16];
    for r in 0..4 {
        x[r * 4..r * 4 + 4].copy_from_slice(&a[r * astride..r * astride + 4]);
        y[r * 4..r * 4 + 4].copy_from_slice(&b[r * bstride..r * bstride + 4]);
    }
    // Lane y·4 + x: rows by `bfly4`, then columns, the last against lane ^ 8.
    let d = Simd::from_array(x).cast::<i16>() - Simd::from_array(y).cast::<i16>();
    hsum(last8(bfly4(bfly2(bfly1(d))))) / 2
}

/// SATD of two 16×16 blocks stored with stride 16: the sum of [`satd4`] over the 4×4
/// blocks. The four row groups' lane sums stay in one vector (they fit `i16`) and are added
/// up once.
#[inline]
pub fn satd16(a: &[u8; 256], b: &[u8; 256]) -> u32 {
    let (a, _) = a.as_chunks::<16>();
    let (b, _) = b.as_chunks::<16>();
    let mut acc = I16x16::splat(0);
    for g in 0..4 {
        acc += satd_rows(std::array::from_fn(|k| {
            diff16(&a[g * 4 + k], &b[g * 4 + k])
        }));
    }
    hsum(acc) / 2
}

/// SATD of two 8×8 blocks stored with stride 8: rows `k` and `4 + k` side by side in the
/// lanes.
#[inline]
pub fn satd8(a: &[u8; 64], b: &[u8; 64]) -> u32 {
    let (a, _) = a.as_chunks::<16>();
    let (b, _) = b.as_chunks::<16>();
    // Row pairs (0, 1), (2, 3), (4, 5), (6, 7) as loaded.
    let d: [I16x16; 4] = std::array::from_fn(|k| diff16(&a[k], &b[k]));
    const LO: [usize; 16] = [0, 1, 2, 3, 4, 5, 6, 7, 16, 17, 18, 19, 20, 21, 22, 23];
    const HI: [usize; 16] = [8, 9, 10, 11, 12, 13, 14, 15, 24, 25, 26, 27, 28, 29, 30, 31];
    let r = [
        simd_swizzle!(d[0], d[2], LO),
        simd_swizzle!(d[0], d[2], HI),
        simd_swizzle!(d[1], d[3], LO),
        simd_swizzle!(d[1], d[3], HI),
    ];
    hsum(satd_rows(r)) / 2
}

/// Sum of absolute 8×8 Hadamard-transformed differences of one 8×8 block, quartered so it
/// sits on the scale of [`satd16`].
pub fn sa8d8(a: &[u8], astride: usize, b: &[u8], bstride: usize) -> u32 {
    let row = |y: usize| -> I16x8 {
        Simd::<u8, 8>::from_slice(&a[y * astride..y * astride + 8]).cast::<i16>()
            - Simd::<u8, 8>::from_slice(&b[y * bstride..y * bstride + 8]).cast::<i16>()
    };
    let mut d: [I16x8; 8] = std::array::from_fn(row);
    // Columns: three butterfly stages between rows.
    for h in [1, 2, 4] {
        for i in (0..8).step_by(h * 2) {
            for j in i..i + h {
                let (x, y) = (d[j], d[j + h]);
                d[j] = x + y;
                d[j + h] = x - y;
            }
        }
    }
    // Rows two at a time: lanes x ^ 1, x ^ 2, then the last stage against x ^ 4.
    let mut s = 0;
    for k in 0..4 {
        let v = simd_swizzle!(
            d[2 * k],
            d[2 * k + 1],
            [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]
        );
        let t = bfly2(bfly1(v));
        let p = simd_swizzle!(t, [4, 5, 6, 7, 0, 1, 2, 3, 12, 13, 14, 15, 8, 9, 10, 11]);
        s += hsum(t.abs().simd_max(p.abs()));
    }
    (s + 2) >> 2
}

/// Sum of squared differences of a `w`×`h` block.
#[inline]
pub fn ssd(a: &[u8], astride: usize, b: &[u8], bstride: usize, w: usize, h: usize) -> u64 {
    if w == 16 || w == 8 {
        // Squares up to 255², summed per lane over at most 16 rows (× 2 for 8 wide): i32.
        let mut acc = Simd::<i32, 16>::splat(0);
        let mut row = |ra: &[u8], rb: &[u8]| {
            let d = Simd::<u8, 16>::from_slice(ra).cast::<i32>()
                - Simd::<u8, 16>::from_slice(rb).cast::<i32>();
            acc += d * d;
        };
        if w == 16 {
            for y in 0..h {
                row(
                    &a[y * astride..y * astride + 16],
                    &b[y * bstride..y * bstride + 16],
                );
            }
        } else {
            for y in (0..h).step_by(2) {
                let mut ra = [0u8; 16];
                let mut rb = [0u8; 16];
                ra[..8].copy_from_slice(&a[y * astride..y * astride + 8]);
                rb[..8].copy_from_slice(&b[y * bstride..y * bstride + 8]);
                if y + 1 < h {
                    ra[8..].copy_from_slice(&a[(y + 1) * astride..(y + 1) * astride + 8]);
                    rb[8..].copy_from_slice(&b[(y + 1) * bstride..(y + 1) * bstride + 8]);
                }
                row(&ra, &rb);
            }
        }
        return acc.reduce_sum() as u64;
    }
    let mut s = 0u32;
    for y in 0..h {
        let ra = &a[y * astride..y * astride + w];
        let rb = &b[y * bstride..y * bstride + w];
        s += ra
            .iter()
            .zip(rb)
            .map(|(&x, &y)| {
                let d = u32::from(x.abs_diff(y));
                d * d
            })
            .sum::<u32>();
    }
    u64::from(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The vector SAD and SSD equal their scalar definitions.
    #[test]
    fn sad_ssd_match_scalar() {
        let mut seed = 0x7f4a_7c15u32;
        let mut px = move || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed as u8
        };
        for _ in 0..200 {
            let a: Vec<u8> = (0..40 * 20).map(|_| px()).collect();
            let b: Vec<u8> = (0..24 * 20).map(|_| px()).collect();
            let at = |x: usize, y: usize| i64::from(a[y * 40 + x + 3]);
            let bt = |x: usize, y: usize| i64::from(b[y * 24 + x + 1]);
            let (mut sad, mut ssd16, mut ssd8) = (0, 0, 0);
            for y in 0..16 {
                for x in 0..16 {
                    let d = at(x, y) - bt(x, y);
                    sad += d.abs();
                    ssd16 += d * d;
                    if x < 8 && y < 8 {
                        ssd8 += d * d;
                    }
                }
            }
            assert_eq!(i64::from(sad16(&a[3..], 40, &b[1..], 24)), sad);
            assert_eq!(ssd(&a[3..], 40, &b[1..], 24, 16, 16) as i64, ssd16);
            assert_eq!(ssd(&a[3..], 40, &b[1..], 24, 8, 8) as i64, ssd8);
        }
    }

    fn satd4_ref(a: &[u8], astride: usize, b: &[u8], bstride: usize) -> u32 {
        let mut d = [[0i32; 4]; 4];
        for (y, row) in d.iter_mut().enumerate() {
            for (x, v) in row.iter_mut().enumerate() {
                *v = i32::from(a[y * astride + x]) - i32::from(b[y * bstride + x]);
            }
            *row = hadamard(*row);
        }
        let s: u32 = (0..4)
            .map(|x| {
                let col: [i32; 4] = hadamard(std::array::from_fn(|y| d[y][x]));
                col.iter().map(|v| v.unsigned_abs()).sum::<u32>()
            })
            .sum();
        s.div_ceil(2)
    }

    fn sa8d8_ref(a: &[u8], astride: usize, b: &[u8], bstride: usize) -> u32 {
        let mut d = [[0i32; 8]; 8];
        for (y, row) in d.iter_mut().enumerate() {
            for (x, v) in row.iter_mut().enumerate() {
                *v = i32::from(a[y * astride + x]) - i32::from(b[y * bstride + x]);
            }
            *row = hadamard(*row);
        }
        let s: u32 = (0..8)
            .map(|x| {
                let col: [i32; 8] = hadamard(std::array::from_fn(|y| d[y][x]));
                col.iter().map(|v| v.unsigned_abs()).sum::<u32>()
            })
            .sum();
        (s + 2) >> 2
    }

    fn hadamard<const N: usize>(mut v: [i32; N]) -> [i32; N] {
        let mut h = 1;
        while h < N {
            for i in (0..N).step_by(h * 2) {
                for j in i..i + h {
                    let (x, y) = (v[j], v[j + h]);
                    v[j] = x + y;
                    v[j + h] = x - y;
                }
            }
            h *= 2;
        }
        v
    }

    /// Random pictures, and the extremes (0 against 255).
    fn pictures(seed: u32) -> (Vec<u8>, Vec<u8>) {
        let mut x = seed;
        let mut next = move || {
            x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (x >> 24) as u8
        };
        match seed % 4 {
            0 => (vec![0; 24 * 20], vec![255; 24 * 20]),
            1 => (vec![255; 24 * 20], vec![0; 24 * 20]),
            _ => (
                (0..24 * 20).map(|_| next()).collect(),
                (0..24 * 20).map(|_| next()).collect(),
            ),
        }
    }

    #[test]
    fn hadamard_sums_match_the_plain_transforms() {
        for seed in 0..40 {
            let (a, b) = pictures(seed);
            let o = (seed as usize % 5) + 24 * (seed as usize % 3);
            assert_eq!(
                satd4(&a[o..], 24, &b[o + 1..], 24),
                satd4_ref(&a[o..], 24, &b[o + 1..], 24)
            );
            assert_eq!(
                sa8d8(&a[o..], 24, &b[o..], 24),
                sa8d8_ref(&a[o..], 24, &b[o..], 24)
            );
            for n in [16, 8] {
                let mut want = 0;
                for y in (0..n).step_by(4) {
                    for x in (0..n).step_by(4) {
                        want += satd4_ref(&a[o + y * 24 + x..], 24, &b[y * 24 + x..], 24);
                    }
                }
                // The blocks packed at stride n.
                let pack =
                    |p: &[u8]| -> Vec<u8> { (0..n * n).map(|i| p[i / n * 24 + i % n]).collect() };
                let (x, y) = (pack(&a[o..]), pack(&b));
                let got = if n == 16 {
                    satd16(
                        x.as_slice().try_into().unwrap(),
                        y.as_slice().try_into().unwrap(),
                    )
                } else {
                    satd8(
                        x.as_slice().try_into().unwrap(),
                        y.as_slice().try_into().unwrap(),
                    )
                };
                assert_eq!(got, want, "{n}×{n}");
            }
        }
    }
}
