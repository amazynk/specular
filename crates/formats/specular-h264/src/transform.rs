//! Inverse transforms (8.5.12.2, 8.5.13.2) adding the residual to the prediction in place.

use crate::pixel::Pix;

/// 4x4 inverse transform of `c` (raster) added to the 4x4 block at `dst[off..]`; clears `c`.
#[inline]
pub fn idct4_add<P: Pix>(c: &mut [i32], dst: &mut [P], off: usize, stride: usize, max: i32) {
    let c: &mut [i32; 16] = (&mut c[..16]).try_into().unwrap();
    if c[1..].iter().all(|&v| v == 0) {
        let dc = (c[0] + 32) >> 6;
        c[0] = 0;
        for y in 0..4 {
            let row = &mut dst[off + y * stride..off + y * stride + 4];
            for p in row {
                *p = P::clip(p.i() + dc, max);
            }
        }
        return;
    }
    let mut t = [0i32; 16];
    for i in 0..4 {
        let d = &c[i * 4..i * 4 + 4];
        let e = d[0] + d[2];
        let f = d[0] - d[2];
        let g = (d[1] >> 1) - d[3];
        let h = d[1] + (d[3] >> 1);
        t[i * 4] = e + h;
        t[i * 4 + 1] = f + g;
        t[i * 4 + 2] = f - g;
        t[i * 4 + 3] = e - h;
    }
    for j in 0..4 {
        let e = t[j] + t[8 + j];
        let f = t[j] - t[8 + j];
        let g = (t[4 + j] >> 1) - t[12 + j];
        let h = t[4 + j] + (t[12 + j] >> 1);
        let r = [e + h, f + g, f - g, e - h];
        for (y, v) in r.iter().enumerate() {
            let p = &mut dst[off + y * stride + j];
            *p = P::clip(p.i() + ((v + 32) >> 6), max);
        }
    }
    *c = [0; 16];
}

#[inline(always)]
fn idct8_1d(d: [i32; 8]) -> [i32; 8] {
    let a0 = d[0] + d[4];
    let a4 = d[0] - d[4];
    let a2 = (d[2] >> 1) - d[6];
    let a6 = d[2] + (d[6] >> 1);
    let b0 = a0 + a6;
    let b2 = a4 + a2;
    let b4 = a4 - a2;
    let b6 = a0 - a6;
    let a1 = -d[3] + d[5] - d[7] - (d[7] >> 1);
    let a3 = d[1] + d[7] - d[3] - (d[3] >> 1);
    let a5 = -d[1] + d[7] + d[5] + (d[5] >> 1);
    let a7 = d[3] + d[5] + d[1] + (d[1] >> 1);
    let b1 = a1 + (a7 >> 2);
    let b7 = a7 - (a1 >> 2);
    let b3 = a3 + (a5 >> 2);
    let b5 = (a3 >> 2) - a5;
    [
        b0 + b7,
        b2 + b5,
        b4 + b3,
        b6 + b1,
        b6 - b1,
        b4 - b3,
        b2 - b5,
        b0 - b7,
    ]
}

/// 8x8 inverse transform of `c` (raster) added to the block at `dst[off..]`; clears `c`.
pub fn idct8_add<P: Pix>(c: &mut [i32], dst: &mut [P], off: usize, stride: usize, max: i32) {
    let c: &mut [i32; 64] = (&mut c[..64]).try_into().unwrap();
    if c[1..].iter().all(|&v| v == 0) {
        let dc = (c[0] + 32) >> 6;
        c[0] = 0;
        for y in 0..8 {
            for p in &mut dst[off + y * stride..off + y * stride + 8] {
                *p = P::clip(p.i() + dc, max);
            }
        }
        return;
    }
    let mut t = [0i32; 64];
    for i in 0..8 {
        let r = idct8_1d(c[i * 8..i * 8 + 8].try_into().unwrap());
        t[i * 8..i * 8 + 8].copy_from_slice(&r);
    }
    for j in 0..8 {
        let col = [
            t[j],
            t[8 + j],
            t[16 + j],
            t[24 + j],
            t[32 + j],
            t[40 + j],
            t[48 + j],
            t[56 + j],
        ];
        let r = idct8_1d(col);
        for (y, v) in r.iter().enumerate() {
            let p = &mut dst[off + y * stride + j];
            *p = P::clip(p.i() + ((v + 32) >> 6), max);
        }
    }
    *c = [0; 64];
}

/// Lossless (TransformBypassModeFlag): adds the residual samples `c` (raster, `w`x`h`) to
/// the block at `dst[off..]`, accumulated first down the columns (`dpcm` 1: vertical intra
/// prediction) or along the rows (2: horizontal) as 8.5.15 specifies; clears `c`.
#[allow(clippy::too_many_arguments)]
pub fn bypass_add<P: Pix>(
    c: &mut [i32],
    w: usize,
    h: usize,
    dst: &mut [P],
    off: usize,
    stride: usize,
    max: i32,
    dpcm: u8,
) {
    let c = &mut c[..w * h];
    match dpcm {
        1 => {
            for y in 1..h {
                for x in 0..w {
                    c[y * w + x] += c[(y - 1) * w + x];
                }
            }
        }
        2 => {
            for row in c.chunks_exact_mut(w) {
                for x in 1..w {
                    row[x] += row[x - 1];
                }
            }
        }
        _ => {}
    }
    for y in 0..h {
        let row = &mut dst[off + y * stride..off + y * stride + w];
        for (p, &r) in row.iter_mut().zip(&c[y * w..y * w + w]) {
            *p = P::clip(p.i() + r, max);
        }
    }
    c.fill(0);
}

/// Inverse 4x4 Hadamard of the Intra16x16 DC coefficients `c` (raster of blocks).
pub fn hadamard4(c: &[i32; 16]) -> [i32; 16] {
    let mut t = [0i32; 16];
    for i in 0..4 {
        let d = &c[i * 4..i * 4 + 4];
        let s01 = d[0] + d[1];
        let d01 = d[0] - d[1];
        let s23 = d[2] + d[3];
        let d23 = d[2] - d[3];
        t[i * 4] = s01 + s23;
        t[i * 4 + 1] = s01 - s23;
        t[i * 4 + 2] = d01 - d23;
        t[i * 4 + 3] = d01 + d23;
    }
    let mut f = [0i32; 16];
    for j in 0..4 {
        let s01 = t[j] + t[4 + j];
        let d01 = t[j] - t[4 + j];
        let s23 = t[8 + j] + t[12 + j];
        let d23 = t[8 + j] - t[12 + j];
        f[j] = s01 + s23;
        f[4 + j] = s01 - s23;
        f[8 + j] = d01 - d23;
        f[12 + j] = d01 + d23;
    }
    f
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hadamard_matches_matrix_product() {
        let h = [[1, 1, 1, 1], [1, 1, -1, -1], [1, -1, -1, 1], [1, -1, 1, -1]];
        let c: [i32; 16] = core::array::from_fn(|i| (i as i32 * 7 - 30) * ((i % 3) as i32 - 1));
        let mut want = [0i32; 16];
        for i in 0..4 {
            for j in 0..4 {
                let mut s = 0;
                for k in 0..4 {
                    for l in 0..4 {
                        s += h[i][k] * c[k * 4 + l] * h[l][j];
                    }
                }
                want[i * 4 + j] = s;
            }
        }
        assert_eq!(hadamard4(&c), want);
    }
}
