static CONST1: i64 = 20091;
static CONST2: i64 = 35468;

pub(crate) fn idct4x4(block: &mut [i32]) {
    // The intermediate results may overflow the types, so we stretch the type.
    fn fetch(block: &[i32], idx: usize) -> i64 {
        i64::from(block[idx])
    }

    // Perform one lenght check up front to avoid subsequent bounds checks in this function
    assert!(block.len() >= 16);

    for i in 0usize..4 {
        let a1 = fetch(block, i) + fetch(block, 8 + i);
        let b1 = fetch(block, i) - fetch(block, 8 + i);

        let t1 = (fetch(block, 4 + i) * CONST2) >> 16;
        let t2 = fetch(block, 12 + i) + ((fetch(block, 12 + i) * CONST1) >> 16);
        let c1 = t1 - t2;

        let t1 = fetch(block, 4 + i) + ((fetch(block, 4 + i) * CONST1) >> 16);
        let t2 = (fetch(block, 12 + i) * CONST2) >> 16;
        let d1 = t1 + t2;

        block[i] = (a1 + d1) as i32;
        block[4 + i] = (b1 + c1) as i32;
        block[4 * 3 + i] = (a1 - d1) as i32;
        block[4 * 2 + i] = (b1 - c1) as i32;
    }

    for i in 0usize..4 {
        let a1 = fetch(block, 4 * i) + fetch(block, 4 * i + 2);
        let b1 = fetch(block, 4 * i) - fetch(block, 4 * i + 2);

        let t1 = (fetch(block, 4 * i + 1) * CONST2) >> 16;
        let t2 = fetch(block, 4 * i + 3) + ((fetch(block, 4 * i + 3) * CONST1) >> 16);
        let c1 = t1 - t2;

        let t1 = fetch(block, 4 * i + 1) + ((fetch(block, 4 * i + 1) * CONST1) >> 16);
        let t2 = (fetch(block, 4 * i + 3) * CONST2) >> 16;
        let d1 = t1 + t2;

        block[4 * i] = ((a1 + d1 + 4) >> 3) as i32;
        block[4 * i + 3] = ((a1 - d1 + 4) >> 3) as i32;
        block[4 * i + 1] = ((b1 + c1 + 4) >> 3) as i32;
        block[4 * i + 2] = ((b1 - c1 + 4) >> 3) as i32;
    }
}

// 14.3
pub(crate) fn iwht4x4(block: &mut [i32]) {
    // Perform one length check up front to avoid subsequent bounds checks in this function
    assert!(block.len() >= 16);

    for i in 0usize..4 {
        let a1 = block[i] + block[12 + i];
        let b1 = block[4 + i] + block[8 + i];
        let c1 = block[4 + i] - block[8 + i];
        let d1 = block[i] - block[12 + i];

        block[i] = a1 + b1;
        block[4 + i] = c1 + d1;
        block[8 + i] = a1 - b1;
        block[12 + i] = d1 - c1;
    }

    for block in block.chunks_exact_mut(4) {
        let a1 = block[0] + block[3];
        let b1 = block[1] + block[2];
        let c1 = block[1] - block[2];
        let d1 = block[0] - block[3];

        let a2 = a1 + b1;
        let b2 = c1 + d1;
        let c2 = a1 - b1;
        let d2 = d1 - c1;

        block[0] = (a2 + 3) >> 3;
        block[1] = (b2 + 3) >> 3;
        block[2] = (c2 + 3) >> 3;
        block[3] = (d2 + 3) >> 3;
    }
}

type I4 = std::simd::Simd<i32, 4>;

/// `(x * c) >> 16` for any `i32` `x` and `c < 2^16`, exact in 32-bit lanes: with
/// `x = hi * 2^16 + lo` (`0 <= lo < 2^16`) it is `hi * c + ((lo * c) >> 16)`, and `lo * c`
/// fits in `u32`.
#[inline(always)]
fn mul_q16(x: I4, c: u16) -> I4 {
    use std::simd::num::{SimdInt, SimdUint};
    let hi = x >> 16;
    let lo = (x & I4::splat(0xffff)).cast::<u32>();
    hi * I4::splat(i32::from(c)) + ((lo * std::simd::Simd::splat(u32::from(c))) >> 16).cast::<i32>()
}

/// One 1-D pass of [`idct4x4`] over four vectors at once (lane `i` is line `i`).
#[inline(always)]
fn idct_pass(x: [I4; 4]) -> [I4; 4] {
    let a = x[0] + x[2];
    let b = x[0] - x[2];
    let c = mul_q16(x[1], 35468) - (x[3] + mul_q16(x[3], 20091));
    let d = x[1] + mul_q16(x[1], 20091) + mul_q16(x[3], 35468);
    [a + d, b + c, b - c, a - d]
}

#[inline(always)]
fn transpose4(o: [I4; 4]) -> [I4; 4] {
    use std::simd::simd_swizzle;
    let ab_lo = simd_swizzle!(o[0], o[1], [0, 4, 1, 5]);
    let ab_hi = simd_swizzle!(o[0], o[1], [2, 6, 3, 7]);
    let cd_lo = simd_swizzle!(o[2], o[3], [0, 4, 1, 5]);
    let cd_hi = simd_swizzle!(o[2], o[3], [2, 6, 3, 7]);
    [
        simd_swizzle!(ab_lo, cd_lo, [0, 1, 4, 5]),
        simd_swizzle!(ab_lo, cd_lo, [2, 3, 6, 7]),
        simd_swizzle!(ab_hi, cd_hi, [0, 1, 4, 5]),
        simd_swizzle!(ab_hi, cd_hi, [2, 3, 6, 7]),
    ]
}

/// [`idct4x4`] of `block` added to the 4×4 pixels at `pos` (clamped), in SIMD.
///
/// Matches the scalar 64-bit version whenever no intermediate leaves `i32`, which holds for
/// every coefficient a VP8 stream can code (|coefficient| < 2^21, sums < 2^27).
#[inline(always)]
pub(crate) fn idct4x4_add(block: &[i32; 16], ws: &mut [u8], pos: usize, stride: usize) {
    let row = |k: usize| {
        I4::from_array([
            block[4 * k],
            block[4 * k + 1],
            block[4 * k + 2],
            block[4 * k + 3],
        ])
    };
    // Vertical pass: lane i is column i.
    let v = idct_pass([row(0), row(1), row(2), row(3)]);
    // Horizontal pass: lane i is row i; the result is column-major.
    let four = I4::splat(4);
    let h = idct_pass(transpose4(v)).map(|c| (c + four) >> 3);
    add_rows(transpose4(h), ws, pos, stride);
}

/// A DC-only block's inverse transform, `(dc + 4) >> 3` everywhere, added to the 4×4 pixels at
/// `pos` (clamped).
#[inline(always)]
pub(crate) fn dc_add(dc: i32, ws: &mut [u8], pos: usize, stride: usize) {
    add_rows([I4::splat((dc + 4) >> 3); 4], ws, pos, stride);
}

#[inline(always)]
fn add_rows(rows: [I4; 4], ws: &mut [u8], pos: usize, stride: usize) {
    use std::simd::Simd;
    use std::simd::cmp::SimdOrd;
    use std::simd::num::{SimdInt, SimdUint};
    for (r, res) in rows.into_iter().enumerate() {
        let px = &mut ws[pos + r * stride..][..4];
        let p = Simd::<u8, 4>::from_slice(px).cast::<i32>();
        let out = (p + res)
            .simd_clamp(I4::splat(0), I4::splat(255))
            .cast::<u8>();
        px.copy_from_slice(out.as_array());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simd_idct_matches_scalar() {
        let mut seed = 0x1234_5678_9abc_def0u64;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        for round in 0..20_000 {
            // Small, typical and extreme-but-legal magnitudes.
            let range = [64i64, 4096, 1 << 21][round % 3];
            let mut block = [0i32; 16];
            for c in &mut block {
                if next() % 3 != 0 {
                    *c = (next() % (2 * range as u64)) as i32 - range as i32;
                }
            }
            let mut px = [0u8; 4 * 7];
            for p in &mut px {
                *p = next() as u8;
            }
            let mut want = px;
            let mut scalar = block;
            idct4x4(&mut scalar);
            for r in 0..4 {
                for c in 0..4 {
                    let p = &mut want[r * 7 + 2 + c];
                    *p = (i32::from(*p) + scalar[r * 4 + c]).clamp(0, 255) as u8;
                }
            }
            idct4x4_add(&block, &mut px, 2, 7);
            assert_eq!(px, want, "{block:?}");
        }
    }
}
