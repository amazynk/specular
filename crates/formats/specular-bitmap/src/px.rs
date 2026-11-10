//! Pixel conversions shared by the decoders (portable SIMD where it pays).

use std::simd::prelude::*;
use std::simd::simd_swizzle;

const ALPHA: u8x16 = u8x16::from_array([0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255]);

/// 3-byte pixels to RGBA (alpha 255). `swap` = the input is BGR.
pub(crate) fn rgb_to_rgba(src: &[u8], dst: &mut [u8], swap: bool) {
    let n = (dst.len() / 4).min(src.len() / 3);
    let mut i = 0;
    // Four pixels per step from a 16-byte load (12 used).
    while i + 4 <= n && i * 3 + 16 <= src.len() {
        let v = u8x16::from_slice(&src[i * 3..i * 3 + 16]);
        let s = if swap {
            simd_swizzle!(v, [2, 1, 0, 0, 5, 4, 3, 0, 8, 7, 6, 0, 11, 10, 9, 0])
        } else {
            simd_swizzle!(v, [0, 1, 2, 0, 3, 4, 5, 0, 6, 7, 8, 0, 9, 10, 11, 0])
        };
        (s | ALPHA).copy_to_slice(&mut dst[i * 4..i * 4 + 16]);
        i += 4;
    }
    for (o, p) in dst[i * 4..n * 4]
        .chunks_exact_mut(4)
        .zip(src[i * 3..n * 3].chunks_exact(3))
    {
        let (r, b) = if swap { (p[2], p[0]) } else { (p[0], p[2]) };
        o.copy_from_slice(&[r, p[1], b, 255]);
    }
}

/// 4-byte pixels to RGBA. `swap` = the input is BGRA; `opaque` forces alpha 255.
pub(crate) fn rgba_to_rgba(src: &[u8], dst: &mut [u8], swap: bool, opaque: bool) {
    let n = (dst.len() / 4).min(src.len() / 4);
    let mut i = 0;
    while i + 4 <= n {
        let v = u8x16::from_slice(&src[i * 4..i * 4 + 16]);
        let mut s = if swap {
            simd_swizzle!(v, [2, 1, 0, 3, 6, 5, 4, 7, 10, 9, 8, 11, 14, 13, 12, 15])
        } else {
            v
        };
        if opaque {
            s |= ALPHA;
        }
        s.copy_to_slice(&mut dst[i * 4..i * 4 + 16]);
        i += 4;
    }
    for (o, p) in dst[i * 4..n * 4]
        .chunks_exact_mut(4)
        .zip(src[i * 4..n * 4].chunks_exact(4))
    {
        let (r, b) = if swap { (p[2], p[0]) } else { (p[0], p[2]) };
        o.copy_from_slice(&[r, p[1], b, if opaque { 255 } else { p[3] }]);
    }
}

/// Palette indices to RGBA.
#[inline]
pub(crate) fn expand(idx: &[u8], pal: &[[u8; 4]; 256], dst: &mut [u8]) {
    for (o, &i) in dst.chunks_exact_mut(4).zip(idx) {
        o.copy_from_slice(&pal[i as usize]);
    }
}

/// Packed 1/2/4-bit indices (MSB first) of pixels `x0..x1` in a row to RGBA via `pal`.
pub(crate) fn expand_bits(
    row: &[u8],
    bits: u32,
    x0: u32,
    x1: u32,
    pal: &[[u8; 4]; 256],
    dst: &mut [u8],
) {
    let per = 8 / bits;
    let mask = (1u8 << bits) - 1;
    for (x, o) in (x0..x1).zip(dst.chunks_exact_mut(4)) {
        let b = row.get((x / per) as usize).copied().unwrap_or(0);
        let shift = 8 - bits - (x % per) * bits;
        o.copy_from_slice(&pal[((b >> shift) & mask) as usize]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversions() {
        let src: Vec<u8> = (0..30).collect();
        let mut a = vec![0u8; 40];
        rgb_to_rgba(&src, &mut a, false);
        let mut b = vec![0u8; 40];
        for (o, p) in b.chunks_exact_mut(4).zip(src.chunks_exact(3)) {
            o.copy_from_slice(&[p[0], p[1], p[2], 255]);
        }
        assert_eq!(a, b);
        rgb_to_rgba(&src, &mut a, true);
        for (o, p) in b.chunks_exact_mut(4).zip(src.chunks_exact(3)) {
            o.copy_from_slice(&[p[2], p[1], p[0], 255]);
        }
        assert_eq!(a, b);
        let src: Vec<u8> = (0..36).collect();
        let mut a = vec![0u8; 36];
        rgba_to_rgba(&src, &mut a, true, false);
        for (o, p) in a.chunks_exact(4).zip(src.chunks_exact(4)) {
            assert_eq!(o, [p[2], p[1], p[0], p[3]]);
        }
    }
}
