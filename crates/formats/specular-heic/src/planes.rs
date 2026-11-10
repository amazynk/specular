//! The decoded YCbCr planes for display, unconverted: a [`PixelLayout::Yuv`] raster the GPU
//! converts (`specular_core::yuv::to_rgb` is the CPU twin), so showing an image costs a copy
//! of 1.5 bytes a pixel (3 at 10 bits) instead of a colour pass and 4–8 bytes.
//!
//! - At 1:1 the planes are copied as they are; chroma keeps its subsampling and its siting
//!   (the SPS VUI chroma sample location, carried through `irot` / `imir`), and is
//!   interpolated where it is shown. A window is widened by one chroma sample each side, so
//!   the pixels it asked for interpolate across its edges like the whole image's.
//! - At 1/s the s×s luma block and the chroma samples under it are averaged (alpha-weighted
//!   with alpha), giving 4:4:4 planes at the output size: subsampled chroma needs no
//!   interpolation there.

use heif::{Chroma, HeifFrame, Site};
use rayon::prelude::*;
use specular_core::hdr::HdrEncoding;
use specular_core::{PixelLayout, Raster, RasterPlane, YuvEncoding, YuvMatrix};

use crate::rgba::ConvertOptions;
use crate::{Error, Image, Result};

/// Output rows per band between cancellation checks.
const BAND: usize = 16;

impl Image {
    /// The encoding of the planes [`Image::to_yuv`] hands over: matrix and range from
    /// [`crate::Colour::ycbcr`], the frame's bit depth and chroma siting, `colour` for the
    /// R'G'B' (PQ / HLG, or [`HdrEncoding::SRGB`]). `None` for monochrome images.
    pub fn yuv_encoding(&self, colour: HdrEncoding) -> Option<YuvEncoding> {
        let f = &self.frame.format;
        if f.chroma == Chroma::Mono {
            return None;
        }
        let y = self.colour.ycbcr();
        let matrix = match y.matrix {
            0 => YuvMatrix::Identity,
            1 => YuvMatrix::Bt709,
            4 => YuvMatrix::Fcc,
            7 => YuvMatrix::Smpte240,
            9 => YuvMatrix::Bt2020,
            // 5, 6 and everything `ycbcr` resolves to BT.601.
            _ => YuvMatrix::Bt601,
        };
        let s = f.siting;
        Some(YuvEncoding {
            matrix,
            full_range: y.full_range,
            bits: f.bit_depth,
            cosited_x: s.x == Site::Start,
            right_x: s.x == Site::End,
            cosited_y: s.y == Site::Start,
            bottom_y: s.y == Site::End,
            alpha: f.has_alpha,
            colour,
        })
    }

    /// The planes as a [`PixelLayout::Yuv`] raster at `o.scale`, for `o.window` (see the
    /// [module docs](self)); `o.wide` is not used (the samples keep their bits). Alpha is
    /// as stored: pre-multiplied when [`Image::premultiplied_alpha`]. `Ok(None)` for
    /// monochrome images, which [`Image::to_rgba`] converts.
    pub fn to_yuv(&self, o: &ConvertOptions<'_>, colour: HdrEncoding) -> Result<Option<Raster>> {
        let Some(e) = self.yuv_encoding(colour) else {
            return Ok(None);
        };
        let s = o.scale.max(1);
        let r = if s == 1 {
            window_planes(&self.frame, e, o.window)
        } else {
            reduced_planes(&self.frame, e, s, o)?
        };
        Ok(Some(r))
    }
}

/// Copies `w × h` samples of plane `p` at `(x, y)` (in that plane's samples), native-endian.
fn copy_rect(f: &HeifFrame, p: usize, (x, y): (u32, u32), (w, h): (u32, u32)) -> Vec<u8> {
    let bps = f.format.bytes_per_sample();
    let src = &f.planes[p];
    let row = w as usize * bps;
    let mut out = Vec::with_capacity(row * h as usize);
    for r in y..y + h {
        let start = r as usize * src.stride + x as usize * bps;
        out.extend_from_slice(&src.data[start..start + row]);
    }
    if bps == 2 && cfg!(target_endian = "big") {
        // HeifFrame samples are little-endian.
        for s in out.chunks_exact_mut(2) {
            s.swap(0, 1);
        }
    }
    out
}

/// The 1:1 planes of the whole frame or of `window` widened by a chroma sample each side.
fn window_planes(f: &HeifFrame, e: YuvEncoding, window: Option<(u32, u32, u32, u32)>) -> Raster {
    let (sx, sy) = f.format.chroma.shift();
    let (fw, fh) = (f.width, f.height);
    let (x0, y0, x1, y1) = match window {
        None => (0, 0, fw, fh),
        Some((x, y, w, h)) => {
            let x = x.min(fw);
            let y = y.min(fh);
            let (x1, y1) = (x.saturating_add(w).min(fw), y.saturating_add(h).min(fh));
            // Two luma samples (one chroma sample) of apron on subsampled axes, aligned to
            // whole chroma samples.
            let ax = 2 * sx;
            let ay = 2 * sy;
            (
                x.saturating_sub(ax) & !sx,
                y.saturating_sub(ay) & !sy,
                (x1 + ax).min(fw),
                (y1 + ay).min(fh),
            )
        }
    };
    let (w, h) = (x1.saturating_sub(x0), y1.saturating_sub(y0));
    let bps = f.format.bytes_per_sample();
    let mut planes = Vec::with_capacity(4);
    for p in 0..3 {
        let (hd, vd) = if p == 0 { (0, 0) } else { (sx, sy) };
        let (pw, ph) = (w.div_ceil(1 << hd), h.div_ceil(1 << vd));
        let (fpw, fph) = f.plane_dims(p);
        let (px, py) = (x0 >> hd, y0 >> vd);
        let (pw, ph) = (pw.min(fpw - px.min(fpw)), ph.min(fph - py.min(fph)));
        planes.push(RasterPlane {
            width: pw,
            height: ph,
            stride: pw as usize * bps,
            data: copy_rect(f, p, (px, py), (pw, ph)),
            h_div: 1 << hd,
            v_div: 1 << vd,
        });
    }
    if let Some(a) = f.format.alpha_plane() {
        planes.push(RasterPlane {
            width: w,
            height: h,
            stride: w as usize * bps,
            data: copy_rect(f, a, (x0, y0), (w, h)),
            h_div: 1,
            v_div: 1,
        });
    }
    Raster {
        width: w,
        height: h,
        origin: (x0, y0),
        scale: 1,
        layout: PixelLayout::Yuv(e),
        planes,
        truncated: false,
    }
}

/// One sample of a plane row (little-endian above 8 bits).
#[inline(always)]
fn at(row: &[u8], wide: bool, i: usize) -> u32 {
    if wide {
        u32::from(u16::from_le_bytes([row[2 * i], row[2 * i + 1]]))
    } else {
        u32::from(row[i])
    }
}

/// Box-averaged 4:4:4 planes at 1/`s` (the output window of `o`).
fn reduced_planes(f: &HeifFrame, e: YuvEncoding, s: u32, o: &ConvertOptions<'_>) -> Result<Raster> {
    let (fw, fh) = (f.width, f.height);
    let (ow, oh) = (fw.div_ceil(s), fh.div_ceil(s));
    let (wx, wy, ww, wh) = match o.window {
        None => (0, 0, ow, oh),
        Some((x, y, w, h)) => {
            let (x, y) = (x.min(ow), y.min(oh));
            (x, y, w.min(ow - x), h.min(oh - y))
        }
    };
    let (sx, sy) = f.format.chroma.shift();
    let wide = f.format.bit_depth > 8;
    let bps = f.format.bytes_per_sample();
    let alpha = f.format.alpha_plane();
    let nplanes = 3 + usize::from(alpha.is_some());
    let row_bytes = ww as usize * bps;
    let mut data: Vec<Vec<u8>> = (0..nplanes)
        .map(|_| vec![0u8; row_bytes * wh as usize])
        .collect();
    let cancelled = || {
        o.cancel
            .is_some_and(|c| c.load(std::sync::atomic::Ordering::Relaxed))
    };
    // One output row of every plane.
    let row = |oy: u32, out: &mut [&mut [u8]]| {
        let ys = (oy + wy) * s;
        let ye = (ys + s).min(fh);
        for ox in 0..ww {
            let xs = (ox + wx) * s;
            let xe = (xs + s).min(fw);
            // Alpha-weighted sums, and plain ones for a wholly transparent block.
            let (mut wsum, mut plain) = ([0u64; 4], [0u64; 3]);
            let mut n = 0u64;
            for y in ys..ye {
                let ly = f.row(0, y);
                let cb = f.row(1, y >> sy);
                let cr = f.row(2, y >> sy);
                let la = alpha.map(|a| f.row(a, y));
                for x in xs..xe {
                    let xi = x as usize;
                    let ci = (x >> sx) as usize;
                    let v = [at(ly, wide, xi), at(cb, wide, ci), at(cr, wide, ci)];
                    let a = la.map_or(1, |r| at(r, wide, xi));
                    for c in 0..3 {
                        wsum[c] += u64::from(v[c] * a);
                        plain[c] += u64::from(v[c]);
                    }
                    wsum[3] += u64::from(a);
                    n += 1;
                }
            }
            let i = ox as usize * bps;
            let put = |dst: &mut [u8], v: u64| {
                if wide {
                    dst[i..i + 2].copy_from_slice(&(v as u16).to_ne_bytes());
                } else {
                    dst[i] = v as u8;
                }
            };
            for c in 0..3 {
                let v = (wsum[c] + wsum[3] / 2)
                    .checked_div(wsum[3])
                    .unwrap_or_else(|| (plain[c] + n / 2) / n.max(1));
                put(out[c], v);
            }
            if alpha.is_some() {
                put(out[3], (wsum[3] + n / 2) / n.max(1));
            }
        }
    };
    // Bands of output rows: each band's rows of every plane.
    let bands = wh.div_ceil(BAND as u32) as usize;
    let mut chunks: Vec<Vec<&mut [u8]>> = (0..bands).map(|_| Vec::with_capacity(4)).collect();
    for plane in &mut data {
        for (b, c) in plane.chunks_mut(row_bytes * BAND).enumerate() {
            chunks[b].push(c);
        }
    }
    let run = |(b, band): (usize, &mut Vec<&mut [u8]>)| -> Result<()> {
        if cancelled() {
            return Err(Error::Cancelled);
        }
        let rows = band[0].len() / row_bytes.max(1);
        for r in 0..rows {
            let mut out: Vec<&mut [u8]> = band
                .iter_mut()
                .map(|p| &mut p[r * row_bytes..(r + 1) * row_bytes])
                .collect();
            row((b * BAND + r) as u32, &mut out);
        }
        Ok(())
    };
    if row_bytes > 0 {
        if o.parallel {
            chunks.par_iter_mut().enumerate().try_for_each(run)?;
        } else {
            chunks.iter_mut().enumerate().try_for_each(run)?;
        }
    }
    drop(chunks);
    let planes = data
        .into_iter()
        .map(|d| RasterPlane {
            width: ww,
            height: wh,
            stride: row_bytes,
            data: d,
            h_div: 1,
            v_div: 1,
        })
        .collect();
    Ok(Raster {
        width: ww,
        height: wh,
        origin: (wx, wy),
        scale: s,
        layout: PixelLayout::Yuv(e),
        planes,
        truncated: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Colour;
    use heif::{ChromaSiting, HeifPixelFormat};

    /// A pseudo-random `chroma` frame of `depth` bits, optionally with alpha.
    fn frame(w: u32, h: u32, chroma: Chroma, depth: u8, alpha: bool) -> HeifFrame {
        let fmt = HeifPixelFormat::new(chroma, depth, alpha).unwrap();
        let mut f = HeifFrame::zeroed(w, h, fmt).unwrap();
        let max = u32::from(fmt.max_value());
        let mut seed = 99u32;
        for p in 0..fmt.plane_count() {
            let (pw, ph) = f.plane_dims(p);
            for y in 0..ph {
                for x in 0..pw {
                    seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
                    let v = (seed >> 8) % (max + 1);
                    // Some fully transparent blocks.
                    let v = if Some(p) == fmt.alpha_plane() && x < 4 && y < 4 {
                        0
                    } else {
                        v
                    };
                    f.set_sample(p, x, y, v as u16);
                }
            }
        }
        f
    }

    fn image(f: HeifFrame) -> Image {
        Image {
            frame: f,
            colour: Colour::default(),
            premultiplied_alpha: false,
            warnings: Vec::new(),
        }
    }

    fn sample(p: &RasterPlane, x: u32, y: u32) -> u16 {
        let bps = p.bytes_per_pixel();
        let o = y as usize * p.stride + x as usize * bps;
        if bps == 2 {
            u16::from_ne_bytes([p.data[o], p.data[o + 1]])
        } else {
            u16::from(p.data[o])
        }
    }

    /// The encoding names the frame's siting, matrix, range and alpha.
    #[test]
    fn encoding_follows_the_frame() {
        let mut f = frame(8, 6, Chroma::Yuv420, 10, true);
        f.format.siting = ChromaSiting::from_h273(5);
        let e = image(f).yuv_encoding(HdrEncoding::SRGB).unwrap();
        assert!(!e.cosited_x && !e.right_x && e.bottom_y && !e.cosited_y);
        assert_eq!(
            (e.bits, e.alpha, e.matrix, e.full_range),
            (10, true, YuvMatrix::Bt601, false)
        );
        let mono = image(frame(8, 6, Chroma::Mono, 8, false));
        assert!(mono.yuv_encoding(HdrEncoding::SRGB).is_none());
        assert!(
            mono.to_yuv(&ConvertOptions::default(), HdrEncoding::SRGB)
                .unwrap()
                .is_none()
        );
    }

    /// A window comes back widened to whole chroma samples plus one each side, its planes
    /// cut from the whole image's.
    #[test]
    fn windows_are_cut_with_an_apron() {
        for chroma in [Chroma::Yuv420, Chroma::Yuv422, Chroma::Yuv444] {
            for depth in [8, 12] {
                let img = image(frame(23, 17, chroma, depth, true));
                let o = ConvertOptions::default();
                let whole = img.to_yuv(&o, HdrEncoding::SRGB).unwrap().unwrap();
                assert_eq!((whole.width, whole.height, whole.planes.len()), (23, 17, 4));
                let win = ConvertOptions {
                    window: Some((5, 3, 9, 7)),
                    ..o
                };
                let r = img.to_yuv(&win, HdrEncoding::SRGB).unwrap().unwrap();
                let (sx, sy) = chroma.shift();
                let (x0, y0) = (if sx == 1 { 2 } else { 5 }, if sy == 1 { 0 } else { 3 });
                assert_eq!(r.origin, (x0, y0), "{chroma:?}");
                assert!(r.origin.0 + r.width >= 14 && r.origin.1 + r.height >= 10);
                for (k, p) in r.planes.iter().enumerate() {
                    let q = &whole.planes[k];
                    let (dx, dy) = (x0 / u32::from(q.h_div), y0 / u32::from(q.v_div));
                    assert_eq!(p.width, r.width.div_ceil(u32::from(p.h_div)));
                    for y in 0..p.height {
                        for x in 0..p.width {
                            assert_eq!(sample(p, x, y), sample(q, x + dx, y + dy));
                        }
                    }
                }
            }
        }
    }

    /// At 1/s each output sample is its block's (alpha-weighted) mean, chroma replicated over
    /// the block's luma samples; a transparent block keeps its plain mean.
    #[test]
    fn reduced_levels_are_block_means() {
        for chroma in [Chroma::Yuv420, Chroma::Yuv422, Chroma::Yuv444] {
            for alpha in [false, true] {
                let f = frame(23, 17, chroma, 10, alpha);
                let img = image(f.clone());
                for s in [2u32, 4] {
                    for parallel in [false, true] {
                        let o = ConvertOptions {
                            scale: s,
                            parallel,
                            ..Default::default()
                        };
                        let r = img.to_yuv(&o, HdrEncoding::SRGB).unwrap().unwrap();
                        assert_eq!((r.width, r.height), (23u32.div_ceil(s), 17u32.div_ceil(s)));
                        let (sx, sy) = chroma.shift();
                        for oy in 0..r.height {
                            for ox in 0..r.width {
                                let (mut sum, mut plain, mut n) = ([0u64; 4], [0u64; 3], 0u64);
                                for y in oy * s..(oy * s + s).min(17) {
                                    for x in ox * s..(ox * s + s).min(23) {
                                        let v = [
                                            f.sample(0, x, y),
                                            f.sample(1, x >> sx, y >> sy),
                                            f.sample(2, x >> sx, y >> sy),
                                        ];
                                        let a = if alpha { f.sample(3, x, y) } else { 1 };
                                        for c in 0..3 {
                                            sum[c] += u64::from(v[c]) * u64::from(a);
                                            plain[c] += u64::from(v[c]);
                                        }
                                        sum[3] += u64::from(a);
                                        n += 1;
                                    }
                                }
                                for c in 0..3 {
                                    let want = (sum[c] + sum[3] / 2)
                                        .checked_div(sum[3])
                                        .unwrap_or((plain[c] + n / 2) / n);
                                    assert_eq!(u64::from(sample(&r.planes[c], ox, oy)), want);
                                }
                                if alpha {
                                    let a = sample(&r.planes[3], ox, oy);
                                    assert_eq!(u64::from(a), (sum[3] + n / 2) / n);
                                }
                            }
                        }
                        let win = ConvertOptions {
                            window: Some((1, 2, 3, 2)),
                            ..o
                        };
                        let w = img.to_yuv(&win, HdrEncoding::SRGB).unwrap().unwrap();
                        assert_eq!((w.origin, w.width, w.height), ((1, 2), 3, 2));
                        for (k, p) in w.planes.iter().enumerate() {
                            for y in 0..2 {
                                for x in 0..3 {
                                    assert_eq!(sample(p, x, y), sample(&r.planes[k], x + 1, y + 2));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
