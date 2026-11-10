#[repr(align(16))]
struct Aligned([u8; 16]);

const SRGB_POWTABLE_UPPER: Aligned = Aligned([
    0x00, 0x0a, 0x19, 0x26, 0x32, 0x41, 0x4d, 0x5c, 0x68, 0x75, 0x83, 0x8f, 0xa0, 0xaa, 0xb9, 0xc6,
]);
const SRGB_POWTABLE_LOWER: Aligned = Aligned([
    0x00, 0xb7, 0x04, 0x0d, 0xcb, 0xe7, 0x41, 0x68, 0x51, 0xd1, 0xeb, 0xf2, 0x00, 0xb7, 0x04, 0x0d,
]);

/// `0x4000_0000 | upper << 18 | lower << 10` for each table index, as f32 bit patterns.
const SRGB_POW_MUL: [u32; 16] = {
    let mut t = [0u32; 16];
    let mut i = 0;
    while i < 16 {
        t[i] = 0x4000_0000
            | ((SRGB_POWTABLE_UPPER.0[i] as u32) << 18)
            | ((SRGB_POWTABLE_LOWER.0[i] as u32) << 10);
        i += 1;
    }
    t
};

/// Converts the linear samples with the sRGB transfer curve.
// Fast linear to sRGB conversion, ported from libjxl; eight lanes at a time with `std::simd`
// (the table lookup is a gather), bit-identical to the scalar tail.
pub fn linear_to_srgb(samples: &mut [f32]) {
    use std::simd::Select;
    use std::simd::cmp::SimdPartialOrd;
    use std::simd::num::{SimdFloat, SimdUint};
    use std::simd::{f32x8, u32x8};

    let mut chunks = samples.chunks_exact_mut(8);
    for c in &mut chunks {
        let x = f32x8::from_slice(c);
        let v = x.to_bits() & u32x8::splat(0x7fff_ffff);
        let v_adj = f32x8::from_bits((v | u32x8::splat(0x3e80_0000)) & u32x8::splat(0x3eff_ffff));
        let pow = f32x8::splat(0.059914046);
        let pow = pow * v_adj - f32x8::splat(0.10889456);
        let pow = pow * v_adj + f32x8::splat(0.107963754);
        let pow = pow * v_adj + f32x8::splat(0.018092343);
        let idx = ((v >> 23) - u32x8::splat(118)) & u32x8::splat(0xf);
        let mul = u32x8::gather_or_default(&SRGB_POW_MUL, idx.cast::<usize>());
        let vf = f32x8::from_bits(v);
        let small = vf * f32x8::splat(12.92);
        let acc = pow * f32x8::from_bits(mul) - f32x8::splat(0.055);
        let r = vf.simd_le(f32x8::splat(0.0031308)).select(small, acc);
        r.copysign(x).copy_to_slice(c);
    }
    linear_to_srgb_scalar(chunks.into_remainder());
}

fn linear_to_srgb_scalar(samples: &mut [f32]) {
    for s in samples {
        let v = s.to_bits() & 0x7fff_ffff;
        let v_adj = f32::from_bits((v | 0x3e80_0000) & 0x3eff_ffff);
        let pow = 0.059914046f32;
        let pow = pow * v_adj - 0.10889456;
        let pow = pow * v_adj + 0.107963754;
        let pow = pow * v_adj + 0.018092343;

        // `mul` won't be used when `v` is small.
        let idx = (v >> 23).wrapping_sub(118) as usize & 0xf;
        let mul = SRGB_POW_MUL[idx];

        let v = f32::from_bits(v);
        let small = v * 12.92;
        let acc = pow * f32::from_bits(mul) - 0.055;

        *s = if v <= 0.0031308 { small } else { acc }.copysign(*s);
    }
}

/// Converts samples in sRGB transfer curve to linear. Inverse of `linear_to_srgb`.
pub fn srgb_to_linear(samples: &mut [f32]) {
    #[allow(clippy::excessive_precision)]
    const P: [f32; 5] = [
        2.200248328e-4,
        1.043637593e-2,
        1.624820318e-1,
        7.961564959e-1,
        8.210152774e-1,
    ];
    #[allow(clippy::excessive_precision)]
    const Q: [f32; 5] = [
        2.631846970e-1,
        1.076976492,
        4.987528350e-1,
        -5.512498495e-2,
        6.521209011e-3,
    ];

    for x in samples {
        let a = x.abs();
        *x = if a <= 0.04045 {
            a / 12.92
        } else {
            crate::color::fastmath::rational_poly::eval_generic(a, P, Q)
        }
        .copysign(*x);
    }
}

#[cfg(test)]
mod simd_tests {
    #[test]
    fn simd_matches_scalar() {
        let mut a: Vec<f32> = (0..20_000)
            .map(|i| (i as f32 - 2_000.0) / 15_000.0)
            .collect();
        a.extend([0.0, -0.0, 1.0, 0.0031308, 1e-9, 4.0, f32::MIN_POSITIVE]);
        let mut b = a.clone();
        super::linear_to_srgb(&mut a);
        super::linear_to_srgb_scalar(&mut b);
        for (x, y) in a.iter().zip(&b) {
            assert_eq!(x.to_bits(), y.to_bits());
        }
    }
}
