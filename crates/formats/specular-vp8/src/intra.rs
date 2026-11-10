//! Intra prediction (RFC 6386 section 12) into macroblock workspaces whose first row and
//! column hold the edge pixels.

use crate::modes::{DC_PRED, H_PRED, TM_PRED, V_PRED};

/// Luma workspace stride: column 0 is the left edge, row 0 the above row (index 0 the
/// above-left pixel, 17..21 the above-right pixels).
pub(crate) const YS: usize = 32;
/// Chroma workspace stride (same layout, 8x8).
pub(crate) const CS: usize = 16;

/// Fills a `size`-square block (origin at `ws[ws_stride + 1]`) with a 16x16 or 8x8 mode.
pub(crate) fn predict_mb(
    ws: &mut [u8],
    stride: usize,
    size: usize,
    mode: u8,
    have_above: bool,
    have_left: bool,
) {
    let o = stride + 1;
    match mode {
        DC_PRED => {
            let shift = size.trailing_zeros();
            let above: u32 = ws[1..1 + size].iter().map(|&v| v as u32).sum();
            let left: u32 = (0..size).map(|i| ws[o - 1 + i * stride] as u32).sum();
            let v = match (have_above, have_left) {
                (true, true) => (above + left + (1 << shift)) >> (shift + 1),
                (true, false) => (above + (1 << (shift - 1))) >> shift,
                (false, true) => (left + (1 << (shift - 1))) >> shift,
                (false, false) => 128,
            } as u8;
            for y in 0..size {
                ws[o + y * stride..o + y * stride + size].fill(v);
            }
        }
        V_PRED => {
            let (top, rest) = ws.split_at_mut(stride);
            for y in 0..size {
                rest[1 + y * stride..1 + y * stride + size].copy_from_slice(&top[1..1 + size]);
            }
        }
        H_PRED => {
            for y in 0..size {
                let l = ws[o - 1 + y * stride];
                ws[o + y * stride..o + y * stride + size].fill(l);
            }
        }
        TM_PRED => {
            let p = ws[0] as i32;
            for y in 0..size {
                let l = ws[o - 1 + y * stride] as i32 - p;
                for x in 0..size {
                    ws[o + y * stride + x] = (ws[1 + x] as i32 + l).clamp(0, 255) as u8;
                }
            }
        }
        _ => {}
    }
}

#[inline(always)]
fn avg3(x: u8, y: u8, z: u8) -> u8 {
    ((x as u32 + 2 * y as u32 + z as u32 + 2) >> 2) as u8
}

#[inline(always)]
fn avg2(x: u8, y: u8) -> u8 {
    ((x as u32 + y as u32 + 1) >> 1) as u8
}

/// Predicts the 4x4 subblock whose top-left pixel is `ws[o]` (luma workspace) with a
/// subblock mode (section 12.3).
pub(crate) fn predict_sub(ws: &mut [u8], o: usize, mode: u8) {
    let s = YS;
    let t = o - s;
    let p = ws[t - 1];
    let a: [u8; 8] = ws[t..t + 8].try_into().unwrap_or([0; 8]);
    let l = [
        ws[o - 1],
        ws[o - 1 + s],
        ws[o - 1 + 2 * s],
        ws[o - 1 + 3 * s],
    ];
    // Edge pixels from bottom-left round to top-right.
    let e = [l[3], l[2], l[1], l[0], p, a[0], a[1], a[2], a[3]];
    let mut b = [[0u8; 4]; 4];
    match mode {
        0 => {
            // B_DC_PRED
            let v = (a[..4].iter().chain(&l).map(|&v| v as u32).sum::<u32>() + 4) >> 3;
            b = [[v as u8; 4]; 4];
        }
        1 => {
            // B_TM_PRED
            for (r, row) in b.iter_mut().enumerate() {
                for (c, px) in row.iter_mut().enumerate() {
                    *px = (l[r] as i32 + a[c] as i32 - p as i32).clamp(0, 255) as u8;
                }
            }
        }
        2 => {
            // B_VE_PRED
            let v = [
                avg3(p, a[0], a[1]),
                avg3(a[0], a[1], a[2]),
                avg3(a[1], a[2], a[3]),
                avg3(a[2], a[3], a[4]),
            ];
            b = [v; 4];
        }
        3 => {
            // B_HE_PRED
            let v = [
                avg3(p, l[0], l[1]),
                avg3(l[0], l[1], l[2]),
                avg3(l[1], l[2], l[3]),
                avg3(l[2], l[3], l[3]),
            ];
            for r in 0..4 {
                b[r] = [v[r]; 4];
            }
        }
        4 => {
            // B_LD_PRED
            for r in 0..4 {
                for c in 0..4 {
                    let i = r + c;
                    b[r][c] = if i < 6 {
                        avg3(a[i], a[i + 1], a[i + 2])
                    } else {
                        avg3(a[6], a[7], a[7])
                    };
                }
            }
        }
        5 => {
            // B_RD_PRED: diagonal from e[4 - r + c].
            for r in 0..4 {
                for c in 0..4 {
                    let i = 4 + c - r;
                    b[r][c] = avg3(e[i - 1], e[i], e[i + 1]);
                }
            }
        }
        6 => {
            // B_VR_PRED
            b[3][0] = avg3(e[1], e[2], e[3]);
            b[2][0] = avg3(e[2], e[3], e[4]);
            b[3][1] = avg3(e[3], e[4], e[5]);
            b[1][0] = b[3][1];
            b[2][1] = avg2(e[4], e[5]);
            b[0][0] = b[2][1];
            b[3][2] = avg3(e[4], e[5], e[6]);
            b[1][1] = b[3][2];
            b[2][2] = avg2(e[5], e[6]);
            b[0][1] = b[2][2];
            b[3][3] = avg3(e[5], e[6], e[7]);
            b[1][2] = b[3][3];
            b[2][3] = avg2(e[6], e[7]);
            b[0][2] = b[2][3];
            b[1][3] = avg3(e[6], e[7], e[8]);
            b[0][3] = avg2(e[7], e[8]);
        }
        7 => {
            // B_VL_PRED
            b[0][0] = avg2(a[0], a[1]);
            b[1][0] = avg3(a[0], a[1], a[2]);
            b[2][0] = avg2(a[1], a[2]);
            b[0][1] = b[2][0];
            b[1][1] = avg3(a[1], a[2], a[3]);
            b[3][0] = b[1][1];
            b[2][1] = avg2(a[2], a[3]);
            b[0][2] = b[2][1];
            b[3][1] = avg3(a[2], a[3], a[4]);
            b[1][2] = b[3][1];
            b[2][2] = avg2(a[3], a[4]);
            b[0][3] = b[2][2];
            b[3][2] = avg3(a[3], a[4], a[5]);
            b[1][3] = b[3][2];
            b[2][3] = avg3(a[4], a[5], a[6]);
            b[3][3] = avg3(a[5], a[6], a[7]);
        }
        8 => {
            // B_HD_PRED
            b[3][0] = avg2(e[0], e[1]);
            b[3][1] = avg3(e[0], e[1], e[2]);
            b[2][0] = avg2(e[1], e[2]);
            b[3][2] = b[2][0];
            b[2][1] = avg3(e[1], e[2], e[3]);
            b[3][3] = b[2][1];
            b[2][2] = avg2(e[2], e[3]);
            b[1][0] = b[2][2];
            b[2][3] = avg3(e[2], e[3], e[4]);
            b[1][1] = b[2][3];
            b[1][2] = avg2(e[3], e[4]);
            b[0][0] = b[1][2];
            b[1][3] = avg3(e[3], e[4], e[5]);
            b[0][1] = b[1][3];
            b[0][2] = avg3(e[4], e[5], e[6]);
            b[0][3] = avg3(e[5], e[6], e[7]);
        }
        _ => {
            // B_HU_PRED
            b[0][0] = avg2(l[0], l[1]);
            b[0][1] = avg3(l[0], l[1], l[2]);
            b[0][2] = avg2(l[1], l[2]);
            b[1][0] = b[0][2];
            b[0][3] = avg3(l[1], l[2], l[3]);
            b[1][1] = b[0][3];
            b[1][2] = avg2(l[2], l[3]);
            b[2][0] = b[1][2];
            b[1][3] = avg3(l[2], l[3], l[3]);
            b[2][1] = b[1][3];
            b[2][2] = l[3];
            b[2][3] = l[3];
            b[3] = [l[3]; 4];
        }
    }
    for (r, row) in b.iter().enumerate() {
        ws[o + r * s..o + r * s + 4].copy_from_slice(row);
    }
}
