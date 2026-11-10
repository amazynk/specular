//! Safe `std::simd` 2D inverse DCT for blocks of 8×8 up to 256×256 samples.
//!
//! The arithmetic is exactly that of the scalar `generic::dct` (same operations in the same
//! order, applied to eight independent lanes), so the output is bit-identical; only the data
//! movement differs: columns are transformed eight at a time straight from the rows, and rows
//! are transformed eight at a time after an 8×8 in-register transpose.

use std::simd::f32x8;

use crate::grid::MutableSubgrid;

use super::dct_common;

const SQRT2: f32 = std::f32::consts::SQRT_2;

#[inline(always)]
fn dct4_inverse(i: [f32x8; 4]) -> [f32x8; 4] {
    let sec0 = f32x8::splat(0.5411961);
    let sec1 = f32x8::splat(1.306563);
    let tmp0 = i[1] * f32x8::splat(SQRT2);
    let tmp1 = i[1] + i[3];
    let out0 = (tmp0 + tmp1) * sec0;
    let out1 = (tmp0 - tmp1) * sec1;
    let sum02 = i[0] + i[2];
    let sub02 = i[0] - i[2];
    [sum02 + out0, sub02 + out1, sub02 - out1, sum02 - out0]
}

#[inline(always)]
fn idct8(io: &mut [f32x8; 8]) {
    let sec = dct_common::sec_half_small(8);
    let input0 = [io[0], io[2], io[4], io[6]];
    let input1 = [
        io[1] * f32x8::splat(SQRT2),
        io[3] + io[1],
        io[5] + io[3],
        io[7] + io[5],
    ];
    let output0 = dct4_inverse(input0);
    let output1 = dct4_inverse(input1);
    for idx in 0..4 {
        let r = output1[idx] * f32x8::splat(sec[idx]);
        io[idx] = output0[idx] + r;
        io[7 - idx] = output0[idx] - r;
    }
}

/// In-place inverse DCT of `io` (length a power of two, at least 8), lane-wise.
fn idct_1d(io: &mut [f32x8], scratch: &mut [f32x8]) {
    let n = io.len();
    if n == 8 {
        let arr: &mut [f32x8; 8] = io.try_into().expect("length 8");
        idct8(arr);
        return;
    }
    let half = n / 2;
    let scratch = &mut scratch[..n];
    let (input0, input1) = scratch.split_at_mut(half);
    for idx in 0..half {
        input0[idx] = io[idx * 2];
        input1[idx] = io[idx * 2 + 1];
    }
    for idx in 1..half {
        input1[half - idx] += input1[half - idx - 1];
    }
    input1[0] *= f32x8::splat(SQRT2);
    {
        let (output0, output1) = io.split_at_mut(half);
        idct_1d(input0, output0);
        idct_1d(input1, output1);
    }
    for (v, &sec) in input1.iter_mut().zip(dct_common::sec_half(n)) {
        *v *= f32x8::splat(sec);
    }
    let (output0, output1) = io.split_at_mut(half);
    for idx in 0..half {
        output0[idx] = input0[idx] + input1[idx];
        output1[half - idx - 1] = input0[idx] - input1[idx];
    }
}

/// Transposes an 8×8 block held as eight row vectors.
#[inline(always)]
fn transpose8(r: [f32x8; 8]) -> [f32x8; 8] {
    // Three perfect shuffles of the eight rows give the transpose.
    let mut a = r;
    for _ in 0..3 {
        let mut b = a;
        for i in 0..4 {
            let (lo, hi) = a[i].interleave(a[i + 4]);
            b[2 * i] = lo;
            b[2 * i + 1] = hi;
        }
        a = b;
    }
    a
}

/// 2D inverse DCT of `io` in place, rows (along x) first, then columns. Returns `false`
/// (leaving `io` untouched) for sizes this path does not cover; the caller then falls back.
pub fn idct_2d(io: &mut MutableSubgrid<'_>) -> bool {
    let (w, h) = (io.width(), io.height());
    let ok = |n: usize| n.is_power_of_two() && (8..=256).contains(&n);
    if !ok(w) || !ok(h) {
        return false;
    }
    let n = w.max(h);
    if n <= 32 {
        let mut v = [f32x8::splat(0.0); 32];
        let mut s = [f32x8::splat(0.0); 32];
        run(io, &mut v, &mut s);
    } else {
        let mut v = vec![f32x8::splat(0.0); n];
        let mut s = vec![f32x8::splat(0.0); n];
        run(io, &mut v, &mut s);
    }
    true
}

fn run(io: &mut MutableSubgrid<'_>, v: &mut [f32x8], s: &mut [f32x8]) {
    let (w, h) = (io.width(), io.height());

    // Rows: eight at a time, transposed so that each vector holds one column of eight rows.
    for y0 in (0..h).step_by(8) {
        for xc in (0..w).step_by(8) {
            let rows = std::array::from_fn(|l| f32x8::from_slice(&io.get_row(y0 + l)[xc..]));
            let t = transpose8(rows);
            v[xc..xc + 8].copy_from_slice(&t);
        }
        idct_1d(&mut v[..w], s);
        for xc in (0..w).step_by(8) {
            let t: [f32x8; 8] = v[xc..xc + 8].try_into().expect("8 vectors");
            let rows = transpose8(t);
            for (l, r) in rows.into_iter().enumerate() {
                r.copy_to_slice(&mut io.get_row_mut(y0 + l)[xc..xc + 8]);
            }
        }
    }

    // Columns: eight at a time, straight from the rows.
    for xc in (0..w).step_by(8) {
        for (y, vy) in v[..h].iter_mut().enumerate() {
            *vy = f32x8::from_slice(&io.get_row(y)[xc..]);
        }
        idct_1d(&mut v[..h], s);
        for (y, vy) in v[..h].iter().enumerate() {
            vy.copy_to_slice(&mut io.get_row_mut(y)[xc..xc + 8]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::dct_common::DctDirection;
    use super::super::generic::dct_2d_scalar as dct_2d;
    use crate::grid::MutableSubgrid;

    #[test]
    fn transpose() {
        let r: [std::simd::f32x8; 8] = std::array::from_fn(|i| {
            std::simd::f32x8::from_array(std::array::from_fn(|j| (i * 8 + j) as f32))
        });
        let t = super::transpose8(r);
        for i in 0..8 {
            for j in 0..8 {
                assert_eq!(t[i][j], (j * 8 + i) as f32);
            }
        }
    }

    #[test]
    fn matches_generic_bit_exact() {
        let mut seed = 0x1234_5678u32;
        for lw in 3..=8 {
            for lh in 3..=8 {
                let (w, h) = (1usize << lw, 1usize << lh);
                if w * h > 128 * 256 {
                    continue;
                }
                let data: Vec<f32> = (0..w * h)
                    .map(|_| {
                        seed ^= seed << 13;
                        seed ^= seed >> 17;
                        seed ^= seed << 5;
                        (seed % 2001) as f32 / 1000.0 - 1.0
                    })
                    .collect();
                let mut a = data.clone();
                let mut b = data;
                dct_2d(
                    &mut MutableSubgrid::from_buf(&mut a, w, h, w),
                    DctDirection::Inverse,
                );
                assert!(super::idct_2d(&mut MutableSubgrid::from_buf(
                    &mut b, w, h, w
                )));
                for (x, y) in a.iter().zip(&b) {
                    assert_eq!(x.to_bits(), y.to_bits(), "{w}x{h}");
                }
            }
        }
    }
}
