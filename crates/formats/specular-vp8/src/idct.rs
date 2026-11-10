//! Inverse transforms (RFC 6386 section 14): the Walsh-Hadamard transform of the Y2 block
//! and the 4x4 DCT, added to the prediction. Intermediate values are kept in 16 bits as
//! the specification's reference arithmetic does.

/// Inverse WHT of the Y2 block; output `i` is the DC of luma block `i`.
pub(crate) fn iwht(input: &[i16], out: &mut [i16; 16]) {
    let mut t = [0i16; 16];
    for i in 0..4 {
        let (i0, i1, i2, i3) = (
            input[i] as i32,
            input[4 + i] as i32,
            input[8 + i] as i32,
            input[12 + i] as i32,
        );
        let a1 = i0 + i3;
        let b1 = i1 + i2;
        let c1 = i1 - i2;
        let d1 = i0 - i3;
        t[i] = (a1 + b1) as i16;
        t[4 + i] = (c1 + d1) as i16;
        t[8 + i] = (a1 - b1) as i16;
        t[12 + i] = (d1 - c1) as i16;
    }
    for i in 0..4 {
        let r = &t[i * 4..i * 4 + 4];
        let a1 = r[0] as i32 + r[3] as i32;
        let b1 = r[1] as i32 + r[2] as i32;
        let c1 = r[1] as i32 - r[2] as i32;
        let d1 = r[0] as i32 - r[3] as i32;
        out[i * 4] = ((a1 + b1 + 3) >> 3) as i16;
        out[i * 4 + 1] = ((c1 + d1 + 3) >> 3) as i16;
        out[i * 4 + 2] = ((a1 - b1 + 3) >> 3) as i16;
        out[i * 4 + 3] = ((d1 - c1 + 3) >> 3) as i16;
    }
}

const C1: i32 = 20091; // cos(pi/8) * sqrt(2) - 1, 16-bit fixed point
const S1: i32 = 35468; // sin(pi/8) * sqrt(2)

#[inline(always)]
fn mul_c(x: i32) -> i32 {
    x + ((x * C1) >> 16)
}

#[inline(always)]
fn mul_s(x: i32) -> i32 {
    (x * S1) >> 16
}

/// Adds the inverse DCT of `c` to the 4x4 block at `dst[off..]` (row pitch `stride`).
#[inline]
pub(crate) fn idct_add(c: &[i16], dst: &mut [u8], off: usize, stride: usize) {
    let mut t = [0i16; 16];
    for i in 0..4 {
        let (i0, i1, i2, i3) = (
            c[i] as i32,
            c[4 + i] as i32,
            c[8 + i] as i32,
            c[12 + i] as i32,
        );
        let a1 = i0 + i2;
        let b1 = i0 - i2;
        let c1 = mul_s(i1) - mul_c(i3);
        let d1 = mul_c(i1) + mul_s(i3);
        t[i] = (a1 + d1) as i16;
        t[12 + i] = (a1 - d1) as i16;
        t[4 + i] = (b1 + c1) as i16;
        t[8 + i] = (b1 - c1) as i16;
    }
    for i in 0..4 {
        let r = &t[i * 4..i * 4 + 4];
        let (i0, i1, i2, i3) = (r[0] as i32, r[1] as i32, r[2] as i32, r[3] as i32);
        let a1 = i0 + i2;
        let b1 = i0 - i2;
        let c1 = mul_s(i1) - mul_c(i3);
        let d1 = mul_c(i1) + mul_s(i3);
        let res = [
            ((a1 + d1 + 4) >> 3) as i16,
            ((b1 + c1 + 4) >> 3) as i16,
            ((b1 - c1 + 4) >> 3) as i16,
            ((a1 - d1 + 4) >> 3) as i16,
        ];
        let row = &mut dst[off + i * stride..off + i * stride + 4];
        for (p, r) in row.iter_mut().zip(res) {
            *p = (*p as i32 + r as i32).clamp(0, 255) as u8;
        }
    }
}

/// Adds a DC-only inverse DCT (the same result as [`idct_add`] with only `c[0]` set).
#[inline]
pub(crate) fn dc_add(dc: i16, dst: &mut [u8], off: usize, stride: usize) {
    let v = (dc as i32 + 4) >> 3;
    for i in 0..4 {
        let row = &mut dst[off + i * stride..off + i * stride + 4];
        for p in row {
            *p = (*p as i32 + v).clamp(0, 255) as u8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dc_only_matches_full() {
        for dc in [-2048i16, -100, -5, -4, -3, 0, 3, 4, 5, 77, 2047] {
            let mut c = [0i16; 16];
            c[0] = dc;
            let mut a = [128u8; 16];
            let mut b = [128u8; 16];
            idct_add(&c, &mut a, 0, 4);
            dc_add(dc, &mut b, 0, 4);
            assert_eq!(a, b, "dc {dc}");
        }
    }

    #[test]
    fn wht_dc_only() {
        let mut c = [0i16; 16];
        c[0] = 1001;
        let mut out = [0i16; 16];
        iwht(&c, &mut out);
        assert!(out.iter().all(|&v| v == (1001 + 3) >> 3));
    }
}
