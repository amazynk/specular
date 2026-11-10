//! CPU colour conversion, the reference path of the path selector's YCbCr → RGB task
//! (DESIGN.md §12.3.1, `renderer/calibrate.rs`): YCbCr planes become R, G, B planes before
//! upload, with the same maths as `convert.wgsl` (BT.601 full range, centred bilinear chroma).
//! The result is still [`PixelLayout::Rgb`], so the ICC transform on the GPU applies unchanged.
//!
//! The selector takes it when the GPU path is slower (a software rasteriser, tiny images where
//! the GPU's setup dominates), failed its correctness check, or `performance.fast_yuv_upload`
//! is off. On GPUs measured so far the shader path wins from about a quarter megapixel up.

use specular_core::{PixelLayout, Raster, RasterPlane};

/// Column (or row) taps of a centred chroma upsampling by `div`: `(i0, i1, weight of i1 / 256)`.
fn taps(n: usize, div: u32, chroma_len: usize) -> Vec<(u32, u32, u32)> {
    let last = chroma_len.saturating_sub(1) as i32;
    let div = div.max(1) as i32;
    (0..n as i32)
        .map(|x| {
            // Chroma position in 1/256 samples: (x + 0.5) / div - 0.5.
            let f = (((2 * x + 1) * 128) / div - 128).max(0);
            let i0 = (f >> 8).min(last);
            let i1 = (i0 + 1).min(last);
            (i0 as u32, i1 as u32, (f & 255) as u32)
        })
        .collect()
}

/// R, G, B planes of a YCbCr raster; `None` for other layouts.
pub fn ycbcr_to_rgb(r: &Raster) -> Option<Raster> {
    if r.layout != PixelLayout::YCbCr || r.planes.len() != 3 {
        return None;
    }
    let (w, h) = (r.width as usize, r.height as usize);
    let (y, cb, cr) = (&r.planes[0], &r.planes[1], &r.planes[2]);
    let fits = |p: &RasterPlane, pw: usize, ph: usize| {
        p.width as usize == pw
            && p.height as usize == ph
            && p.stride >= pw
            && (ph == 0 || p.data.len() >= p.stride * (ph - 1) + pw)
    };
    let (cw, ch) = (cb.width as usize, cb.height as usize);
    if w == 0
        || h == 0
        || cw == 0
        || ch == 0
        || !fits(y, w, h)
        || !fits(cb, cw, ch)
        || !fits(cr, cw, ch)
    {
        return None;
    }
    let cols = taps(w, cb.h_div as u32, cb.width as usize);
    let rows = taps(h, cb.v_div as u32, cb.height as usize);
    let mut out: [Vec<u8>; 3] = [vec![0; w * h], vec![0; w * h], vec![0; w * h]];
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let band = h.div_ceil(threads).max(16);
    let [ro, go, bo] = &mut out;
    std::thread::scope(|s| {
        let chunks = ro
            .chunks_mut(band * w)
            .zip(go.chunks_mut(band * w))
            .zip(bo.chunks_mut(band * w));
        for (k, ((rc, gc), bc)) in chunks.enumerate() {
            let (cols, rows) = (&cols, &rows);
            specular_thread::spawn_scoped(s, format!("ren-colour-{k}"), move || {
                let at = |p: &RasterPlane, row: u32, col: u32| {
                    p.data[row as usize * p.stride + col as usize] as u32
                };
                for (i, ((rr, gr), br)) in rc
                    .chunks_exact_mut(w)
                    .zip(gc.chunks_exact_mut(w))
                    .zip(bc.chunks_exact_mut(w))
                    .enumerate()
                {
                    let yy = k * band + i;
                    let (r0, r1, fy) = rows[yy];
                    let yrow = &y.data[yy * y.stride..yy * y.stride + w];
                    for x in 0..w {
                        let (c0, c1, fx) = cols[x];
                        let bil = |p: &RasterPlane| {
                            let top = at(p, r0, c0) * (256 - fx) + at(p, r0, c1) * fx;
                            let bot = at(p, r1, c0) * (256 - fx) + at(p, r1, c1) * fx;
                            // Chroma in 1/16 codes: rounding it to whole codes would put
                            // up to 0.9 code of error into blue (Cb gain 1.772).
                            ((top * (256 - fy) + bot * fy + (1 << 11)) >> 12) as i32 - (128 << 4)
                        };
                        let (u, v) = (bil(cb), bil(cr));
                        // 12.20 fixed point; coefficients × 2^16 times chroma × 2^4.
                        let l = (yrow[x] as i32) << 20;
                        let c = |a: i32| ((a + (1 << 19)).clamp(0, 255 << 20) >> 20) as u8;
                        rr[x] = c(l + 91881 * v);
                        gr[x] = c(l - 22554 * u - 46802 * v);
                        br[x] = c(l + 116130 * u);
                    }
                }
            });
        }
    });
    let plane = |data: Vec<u8>| RasterPlane {
        width: r.width,
        height: r.height,
        stride: w,
        data,
        h_div: 1,
        v_div: 1,
    };
    let [a, b, c] = out;
    Some(Raster {
        width: r.width,
        height: r.height,
        origin: r.origin,
        scale: r.scale,
        layout: PixelLayout::Rgb,
        planes: vec![plane(a), plane(b), plane(c)],
        truncated: r.truncated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(w: u32, h: u32, d: u8, v: u8) -> RasterPlane {
        RasterPlane {
            width: w,
            height: h,
            stride: w as usize,
            data: vec![v; (w * h) as usize],
            h_div: d,
            v_div: d,
        }
    }

    #[test]
    fn grey_and_red() {
        let mut r = Raster {
            width: 5,
            height: 3,
            origin: (0, 0),
            scale: 1,
            layout: PixelLayout::YCbCr,
            planes: vec![p(5, 3, 1, 128), p(3, 2, 2, 128), p(3, 2, 2, 128)],
            truncated: false,
        };
        let g = ycbcr_to_rgb(&r).unwrap();
        assert!(g.planes.iter().all(|p| p.data.iter().all(|&v| v == 128)));
        // Pure red in JFIF: Y 76, Cb 85, Cr 255.
        r.planes = vec![p(5, 3, 1, 76), p(3, 2, 2, 85), p(3, 2, 2, 255)];
        let c = ycbcr_to_rgb(&r).unwrap();
        assert!(c.planes[0].data.iter().all(|&v| v >= 253));
        assert!(c.planes[1].data.iter().all(|&v| v <= 2));
        assert!(c.planes[2].data.iter().all(|&v| v <= 2));
    }

    #[test]
    fn centred_taps() {
        // 2× upsampling: output 0 sits on chroma 0, output 1 a quarter of the way to chroma 1.
        let t = taps(4, 2, 2);
        assert_eq!(t[0], (0, 1, 0));
        assert_eq!(t[1], (0, 1, 64));
        assert_eq!(t[2], (0, 1, 192));
        assert_eq!(t[3], (1, 1, 64));
        assert_eq!(taps(3, 1, 3), vec![(0, 1, 0), (1, 2, 0), (2, 2, 0)]);
    }
}
