//! Output stage of the arithmetic lossless decoder (`crate::arith::lossless`): full-size RGBA
//! from 8-bit samples, then box-downscale and crop. It gives what `jpeg-decoder` gave when it
//! was the fallback for Huffman lossless files; the native Huffman lossless decoder
//! (`crate::lossless`) follows the same rule, so a SOF11 file and its SOF3 twin give the same
//! planes.

use crate::error::{JpegError, Result};
use crate::geom::alloc_zeroed;
use crate::{DecodedPlanes, Plane, PlanesLayout, Region, Scale};

/// A sample of `precision` (> 8) bits reduced to 8 bits.
#[inline]
pub(crate) fn to_8bit(v: u16, precision: u32) -> u8 {
    let max = (1u32 << precision) - 1;
    (((v as u32).min(max) * 255 + max / 2) / max) as u8
}

/// The output rectangle (x0, y0, x1, y1) of a `w` x `h` frame at `scale`, in output pixels.
pub(crate) fn out_rect(
    w: usize,
    h: usize,
    scale: Scale,
    region: Option<Region>,
) -> Result<(usize, usize, usize, usize)> {
    let s = scale as usize;
    let (ow, oh) = (w.div_ceil(s), h.div_ceil(s));
    match region {
        None => Ok((0, 0, ow, oh)),
        Some(r) => {
            if r.w == 0 || r.h == 0 || r.x as usize >= ow || r.y as usize >= oh {
                return Err(JpegError::Unsupported("empty or out-of-bounds region"));
            }
            Ok((
                r.x as usize,
                r.y as usize,
                (r.x as usize + r.w as usize).min(ow),
                (r.y as usize + r.h as usize).min(oh),
            ))
        }
    }
}

/// Full-size samples (`nc` per pixel, 8-bit, `sample(pixel * nc + component)`) to RGBA, then
/// box-downscaled to `scale` and cropped to `region`.
pub(crate) fn finish(
    w: usize,
    h: usize,
    nc: usize,
    sample: impl Fn(usize) -> u8,
    scale: Scale,
    region: Option<Region>,
    truncated: bool,
) -> Result<DecodedPlanes> {
    let npx = w * h;
    let (x0, y0, x1, y1) = out_rect(w, h, scale, region)?;
    let mut rgba = alloc_zeroed(npx * 4)?;
    for i in 0..npx {
        let o = &mut rgba[i * 4..i * 4 + 4];
        match nc {
            1 => {
                let v = sample(i);
                o.copy_from_slice(&[v, v, v, 255]);
            }
            3 => o.copy_from_slice(&[sample(3 * i), sample(3 * i + 1), sample(3 * i + 2), 255]),
            _ => {
                // jpeg-decoder emits ink amounts (0 = no ink).
                let k = 255 - sample(4 * i + 3) as u32;
                let f = |c: u8| (((255 - c as u32) * k + 127) / 255) as u8;
                o.copy_from_slice(&[
                    f(sample(4 * i)),
                    f(sample(4 * i + 1)),
                    f(sample(4 * i + 2)),
                    255,
                ]);
            }
        }
    }
    let s = scale as usize;
    let (rw, rh) = (x1 - x0, y1 - y0);
    let mut out = alloc_zeroed(rw * rh * 4)?;
    for y in 0..rh {
        for x in 0..rw {
            let (sx0, sy0) = ((x0 + x) * s, (y0 + y) * s);
            let (sx1, sy1) = ((sx0 + s).min(w), (sy0 + s).min(h));
            let mut acc = [0u32; 4];
            for sy in sy0..sy1 {
                for sx in sx0..sx1 {
                    let p = &rgba[(sy * w + sx) * 4..][..4];
                    for c in 0..4 {
                        acc[c] += p[c] as u32;
                    }
                }
            }
            let n = ((sx1 - sx0) * (sy1 - sy0)) as u32;
            let o = &mut out[(y * rw + x) * 4..][..4];
            for c in 0..4 {
                o[c] = ((acc[c] + n / 2) / n) as u8;
            }
        }
    }
    Ok(DecodedPlanes {
        width: rw as u32,
        height: rh as u32,
        origin: (x0 as u32, y0 as u32),
        layout: PlanesLayout::Rgba,
        planes: vec![Plane {
            width: rw as u32,
            height: rh as u32,
            stride: rw * 4,
            data: out,
            h_samp: 1,
            v_samp: 1,
        }],
        truncated,
    })
}
