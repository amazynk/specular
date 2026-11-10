//! Luma/chroma sensor data: Canon sRAW/mRAW (CR2) and Sony lossless compressed M/S sizes.
//!
//! The cameras demosaic in-camera and store luma at full resolution with chroma subsampled
//! 2x1 or 2x2 in lossless JPEG. Each MCU of the frame holds the block's luma samples (row
//! major), then Cb and Cr, both offset by half the sample range. The chroma of a block belongs
//! to its top-left pixel. Canon: the other pixels get chroma averaged from their neighbours
//! (above and below, then left and right), rounding half up, and luma/chroma convert to
//! camera RGB by the camera's integer formula, then per-channel levels undo the camera's sRAW
//! white balance. Sony: the block's chroma applies to all its pixels and BT.601 converts.
//! Both match LibRaw's `raw_image` bit for bit on the samples.

use crate::ljpeg::{self, SubPrediction, Subsampled};

/// The camera's luma/chroma to RGB conversion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Conv {
    /// Early Canon bodies: luma carries a 512 offset; R = Y + Cr, B = Y + Cb,
    /// G = Y - (778 Cb + 2048 Cr) / 4096.
    CanonEarly,
    /// The same formula without the luma offset.
    CanonPlain,
    /// Canon bodies with scaled chroma: chroma is multiplied by four and offset by `hue`, then
    /// a full 3x3 matrix (in 1/16384) maps to RGB.
    CanonMatrix { hue: i32 },
    /// Sony lossless M/S: BT.601 (the YCbCrCoefficients) in single precision.
    Sony,
}

/// Decode one Sony YCbCr lossless JPEG tile (a `tw x th` frame) into the camera RGB rows
/// `rows` (each up to `tw` pixels, 3 samples per pixel). Returns true when the tile was
/// damaged or short.
pub(crate) fn decode_tile(
    src: &[u8],
    half: i32,
    tw: usize,
    th: usize,
    rows: &mut [&mut [u16]],
) -> bool {
    let max = (tw as u64 * th as u64) * 2 + 65536;
    let Ok((h, lay, buf, short)) = ljpeg::decode_subsampled(src, max, SubPrediction::Standard)
    else {
        return true;
    };
    if h.width as usize != tw || h.height as usize != th {
        return true;
    }
    let px = place(&buf, lay, tw, th, half, false);
    for (row, line) in rows.iter_mut().zip(px.chunks_exact(tw)) {
        for (o, &p) in row.chunks_exact_mut(3).zip(line) {
            o.copy_from_slice(&to_rgb(p, Conv::Sony, [1024; 3]));
        }
    }
    short
}

/// Full-resolution luma and chroma of a `w x h` image from the MCU sample stream of `lay`
/// (`lay.rows` lines of `lay.cols * clrs` samples), as `[Y, Cb, Cr]` per pixel with chroma
/// re-centred on zero (`half` = the offset of the stored chroma).
pub(crate) fn place(
    stream: &[u16],
    lay: Subsampled,
    w: usize,
    h: usize,
    half: i32,
    interpolate: bool,
) -> Vec<[i32; 3]> {
    let clrs = lay.clrs();
    let ny = lay.h * lay.v;
    let mut px = vec![[0i32; 3]; w * h];
    for (r, line) in stream.chunks(lay.cols * clrs).take(lay.rows).enumerate() {
        for (c, mcu) in line.chunks_exact(clrs).enumerate() {
            let (y0, x0) = (r * lay.v, c * lay.h);
            let (cb, cr) = (mcu[ny] as i32 - half, mcu[ny + 1] as i32 - half);
            for (k, &luma) in mcu[..ny].iter().enumerate() {
                let (y, x) = (y0 + k / lay.h, x0 + k % lay.h);
                if y < h && x < w {
                    let p = &mut px[y * w + x];
                    p[0] = luma as i32;
                    if k == 0 || !interpolate {
                        (p[1], p[2]) = (cb, cr);
                    }
                }
            }
        }
    }
    if interpolate {
        interpolate_chroma(&mut px, w, h, lay.v);
    }
    px
}

/// Fill the chroma of pixels without their own: odd rows (2x2 blocks) from the rows above and
/// below, then odd columns from their left and right neighbours.
#[allow(clippy::needless_range_loop)]
fn interpolate_chroma(px: &mut [[i32; 3]], w: usize, h: usize, v: usize) {
    for y in 0..h {
        if v == 2 && y % 2 == 1 {
            for x in (0..w).step_by(2) {
                for c in 1..3 {
                    px[y * w + x][c] = if y + 1 == h {
                        px[(y - 1) * w + x][c]
                    } else {
                        (px[(y - 1) * w + x][c] + px[(y + 1) * w + x][c] + 1) >> 1
                    };
                }
            }
        }
        let row = &mut px[y * w..(y + 1) * w];
        for x in (1..w).step_by(2) {
            for c in 1..3 {
                row[x][c] = if x + 1 == w {
                    row[x - 1][c]
                } else {
                    (row[x - 1][c] + row[x + 1][c] + 1) >> 1
                };
            }
        }
    }
}

/// Camera RGB of one pixel, each channel scaled by `mul` (in 1/1024) and clamped to
/// 0..=65535.
pub(crate) fn to_rgb(p: [i32; 3], conv: Conv, mul: [u32; 3]) -> [u16; 3] {
    let [y, cb, cr] = p;
    let rgb = match conv {
        Conv::CanonEarly | Conv::CanonPlain => {
            let y = if conv == Conv::CanonEarly { y - 512 } else { y };
            [y + cr, y + ((-778 * cb - (cr << 11)) >> 12), y + cb]
        }
        Conv::CanonMatrix { hue } => {
            let (cb, cr) = ((cb << 2) + hue, (cr << 2) + hue);
            [
                y + ((50 * cb + 22929 * cr) >> 14),
                y + ((-5640 * cb - 11751 * cr) >> 14),
                y + ((29040 * cb - 101 * cr) >> 14),
            ]
        }
        Conv::Sony => {
            // Single-precision BT.601 on chroma centred one below the nominal offset, rounded
            // down (bit-exact with LibRaw on the 7 IV and 1 II samples).
            let (y, cb, cr) = (y as f32, (cb + 1) as f32, (cr + 1) as f32);
            [
                y + 1.402 * cr,
                y - 0.34414 * cb - 0.71414 * cr,
                y + 1.772 * cb,
            ]
            .map(|v| v.floor().clamp(-1.0, 65536.0) as i32)
        }
    };
    let mut out = [0u16; 3];
    for ((o, v), m) in out.iter_mut().zip(rgb).zip(mul) {
        *o = ((v as i64 * m as i64) >> 10).clamp(0, 65535) as u16;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn places_and_interpolates_2x1() {
        // 4x1 image, two MCUs: Y Y Cb Cr each, chroma offset 100.
        let lay = Subsampled {
            h: 2,
            v: 1,
            cols: 2,
            rows: 1,
        };
        let s = [10, 11, 110, 90, 20, 21, 130, 70];
        let p = place(&s, lay, 4, 1, 100, true);
        assert_eq!(p[0], [10, 10, -10]);
        assert_eq!(p[1], [11, 20, -20]);
        assert_eq!(p[2], [20, 30, -30]);
        // Last column copies its left neighbour.
        assert_eq!(p[3], [21, 30, -30]);
    }

    #[test]
    fn places_and_interpolates_2x2() {
        let lay = Subsampled {
            h: 2,
            v: 2,
            cols: 1,
            rows: 2,
        };
        // MCU rows: luma 2x2 then Cb, Cr.
        let s = [1, 2, 3, 4, 10, 20, 5, 6, 7, 8, 30, 41];
        let p = place(&s, lay, 2, 4, 0, true);
        assert_eq!(
            p.iter().map(|v| v[0]).collect::<Vec<_>>(),
            [1, 2, 3, 4, 5, 6, 7, 8]
        );
        assert_eq!(p[0][1..], [10, 20]);
        assert_eq!(p[2][1..], [20, 31]);
        assert_eq!(p[4][1..], [30, 41]);
        assert_eq!(p[6][1..], [30, 41]);
        assert_eq!(p[7][1..], [30, 41]);
    }

    #[test]
    fn short_stream_and_odd_sizes() {
        let lay = Subsampled {
            h: 2,
            v: 2,
            cols: 2,
            rows: 2,
        };
        let p = place(&[7; 6], lay, 3, 3, 0, false);
        assert_eq!(p.len(), 9);
        assert_eq!(p[0][0], 7);
        assert_eq!(
            to_rgb([100, 0, 0], Conv::CanonPlain, [1024; 3]),
            [100, 100, 100]
        );
        assert_eq!(
            to_rgb([100, 0, 0], Conv::CanonPlain, [957, 1336, 577]),
            [93, 130, 56]
        );
        assert_eq!(to_rgb([-5, 0, 0], Conv::Sony, [1024; 3]), [0, 0, 0]);
        assert_eq!(
            to_rgb([i32::MAX / 2, 0, 0], Conv::CanonEarly, [1024; 3]),
            [65535; 3]
        );
    }
}
