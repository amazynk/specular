//! Raster → small oriented RGBA8: area-average straight from the decoder's planes (no
//! full-size RGBA copy), then the EXIF orientation applied to the small result.
//!
//! Alpha conventions along the thumbnail path:
//! * decoders hand over straight alpha ([`PixelLayout`]);
//! * [`thumbnail`] averages premultiplied (each sample weighted by its alpha, so the colour
//!   under transparent pixels adds nothing: no dark or bright fringes) and stores the result
//!   straight, which is what the disk cache and the duplicate hashes read;
//! * [`Pixels::premultiplied`] is the form the GPU atlas takes, premultiplied in linear light
//!   and sRGB-encoded again (the atlas is an sRGB texture and the UI blends premultiplied
//!   linear colour). It is made once, on the worker that made the pixels; opaque thumbnails
//!   have none and are uploaded as they are.

use std::sync::OnceLock;

use specular_core::{Orientation, PixelLayout, Raster};

/// A finished thumbnail: straight-alpha sRGB RGBA8, orientation applied.
///
/// The pixels must not change after [`Pixels::premultiplied`] (or anything that calls it) has
/// run: the premultiplied copy is made once.
#[derive(Clone, Debug)]
pub struct Pixels {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    /// The premultiplied copy, `None` when every pixel is opaque.
    premul: OnceLock<Option<Box<[u8]>>>,
}

impl PartialEq for Pixels {
    fn eq(&self, o: &Pixels) -> bool {
        (self.width, self.height) == (o.width, o.height) && self.rgba == o.rgba
    }
}

impl Eq for Pixels {}

impl Pixels {
    pub fn new(width: u32, height: u32, rgba: Vec<u8>) -> Pixels {
        Pixels {
            width,
            height,
            rgba,
            premul: OnceLock::new(),
        }
    }

    /// Pixels known to be opaque (a JPEG cache entry): no scan for alpha.
    pub fn opaque(width: u32, height: u32, rgba: Vec<u8>) -> Pixels {
        let p = Pixels::new(width, height, rgba);
        let _ = p.premul.set(None);
        p
    }

    /// Mean colour (alpha-weighted, alpha set to 255): the placeholder before pixels arrive.
    pub fn average(&self) -> [u8; 4] {
        let mut acc = [0u64; 4];
        for p in self.rgba.chunks_exact(4) {
            let a = p[3] as u64;
            for c in 0..3 {
                acc[c] += p[c] as u64 * a;
            }
            acc[3] += a;
        }
        if acc[3] == 0 {
            return [128, 128, 128, 255];
        }
        [
            (acc[0] / acc[3]) as u8,
            (acc[1] / acc[3]) as u8,
            (acc[2] / acc[3]) as u8,
            255,
        ]
    }

    /// Some pixel is not fully opaque.
    pub fn has_alpha(&self) -> bool {
        self.premultiplied().is_some()
    }

    /// Premultiplied in linear light and sRGB-encoded again, same layout as `rgba`; `None`
    /// when the thumbnail is opaque (then `rgba` is already what the GPU wants).
    pub fn premultiplied(&self) -> Option<&[u8]> {
        self.premul
            .get_or_init(|| any_alpha(&self.rgba).then(|| premultiply(&self.rgba)))
            .as_deref()
    }

    /// Scans for alpha and makes the premultiplied copy now. Workers call this so the draw
    /// thread, which uploads the pixels, never does the work.
    pub fn prepare(&self) {
        self.premultiplied();
    }

    /// Whether the alpha scan has run (or was not needed).
    pub fn premul_known(&self) -> bool {
        self.premul.get().is_some()
    }

    /// Bytes held: the pixels and, once made, the premultiplied copy.
    pub fn bytes(&self) -> usize {
        self.rgba.len()
            + self
                .premul
                .get()
                .and_then(Option::as_ref)
                .map_or(0, |p| p.len())
    }
}

/// Any alpha below 255. Branch-free over whole chunks so it vectorises: this runs for every
/// thumbnail, and opaque ones (most) read all of their pixels.
fn any_alpha(rgba: &[u8]) -> bool {
    rgba.chunks(4096)
        .any(|c| c.chunks_exact(4).fold(255u8, |m, p| m & p[3]) != 255)
}

/// Straight sRGB → premultiplied in linear light, sRGB-encoded. Opaque pixels stay as they
/// are; fully transparent ones become zero.
fn premultiply(rgba: &[u8]) -> Box<[u8]> {
    let (to_lin, to_srgb) = srgb_tables();
    let mut out = rgba.to_vec().into_boxed_slice();
    for p in out.chunks_exact_mut(4) {
        match p[3] {
            255 => {}
            0 => p[..3].fill(0),
            a => {
                let af = a as f32 / 255.0;
                for c in &mut p[..3] {
                    let l = to_lin[*c as usize] * af;
                    *c = to_srgb[(l * 4095.0 + 0.5) as usize];
                }
            }
        }
    }
    out
}

/// sRGB byte → linear, and linear in 4096 steps → sRGB byte.
fn srgb_tables() -> &'static ([f32; 256], [u8; 4096]) {
    static T: OnceLock<([f32; 256], [u8; 4096])> = OnceLock::new();
    T.get_or_init(|| {
        let mut lin = [0f32; 256];
        for (i, v) in lin.iter_mut().enumerate() {
            *v = specular_core::hdr::srgb_to_linear(i as f32 / 255.0);
        }
        let mut enc = [0u8; 4096];
        for (i, v) in enc.iter_mut().enumerate() {
            let c = specular_core::hdr::linear_to_srgb(i as f32 / 4095.0);
            *v = (c * 255.0 + 0.5).clamp(0.0, 255.0) as u8;
        }
        (lin, enc)
    })
}

/// Size with the long side `size` (never larger than the source, never zero).
pub fn fit(w: u32, h: u32, size: u32) -> (u32, u32) {
    let long = w.max(h);
    if long <= size || long == 0 {
        return (w.max(1), h.max(1));
    }
    let s = size as f64 / long as f64;
    (
        ((w as f64 * s).round() as u32).clamp(1, size),
        ((h as f64 * s).round() as u32).clamp(1, size),
    )
}

/// The coarsest of `scales` whose result still has `size` px on the long side, or the
/// finest when the image is smaller than that (never upscale).
pub fn pick_scale(scales: &[u32], long_side: u32, size: u32) -> u32 {
    let finest = scales.iter().copied().min().unwrap_or(1).max(1);
    scales
        .iter()
        .copied()
        .filter(|&s| s > 0 && long_side.div_ceil(s) >= size)
        .max()
        .unwrap_or(finest)
}

/// Most source samples averaged per output pixel and axis; larger boxes are sampled on a
/// grid (a 12 MP PNG without scaled decoding costs ~1 M conversions instead of 12 M).
const MAX_TAPS: usize = 6;

/// Scales `r` so its long side (after orientation) is at most `size` and applies `orient`.
pub fn thumbnail(r: &Raster, orient: Orientation, size: u32) -> Pixels {
    // YCbCr planes (HEIC, lossy WebP) through the viewer's conversion: chroma interpolated
    // at its siting, alpha, PQ / HLG labelled for the tone mapping below.
    let rgb;
    let r = match r.layout {
        specular_core::PixelLayout::Yuv(_) => match specular_core::yuv::to_rgb(r) {
            Some(c) => {
                rgb = c;
                &rgb
            }
            None => r,
        },
        _ => r,
    };
    let (sw, sh) = (r.width.max(1), r.height.max(1));
    let (tw, th) = fit(sw, sh, size);
    let (tw_, th_) = (tw as usize, th as usize);
    let (w, h) = (r.width as usize, r.height as usize);
    let mut out = vec![0u8; tw_ * th_ * 4];
    if w == 0 || h == 0 || r.planes.is_empty() {
        out.fill(128);
        return Pixels::new(tw, th, out);
    }
    // Float rasters as the viewer shows them (their own peak, an exposure for data far
    // above reference white); PQ/HLG from the nominal peak.
    let (exposure, peak) = match r.layout {
        specular_core::PixelLayout::Rgba16F(_) => specular_core::hdr::float_exposure(r),
        _ => (1.0, specular_core::hdr::peak_from_nits(None)),
    };
    let fetch = |x: usize, y: usize| -> [u8; 4] { pixel(r, x, y, peak, exposure) };
    let span = |i: usize, n: usize, t: usize| {
        let a = i * n / t;
        (a, ((i + 1) * n).div_ceil(t).max(a + 1).min(n))
    };
    for ty in 0..th_ {
        let (y0, y1) = span(ty, h, th_);
        let ys = (y1 - y0).div_ceil(MAX_TAPS);
        for tx in 0..tw_ {
            let (x0, x1) = span(tx, w, tw_);
            let xs = (x1 - x0).div_ceil(MAX_TAPS);
            let mut acc = [0u64; 4];
            let mut n = 0u64;
            for y in (y0..y1).step_by(ys) {
                for x in (x0..x1).step_by(xs) {
                    let p = fetch(x, y);
                    let a = p[3] as u64;
                    acc[0] += p[0] as u64 * a;
                    acc[1] += p[1] as u64 * a;
                    acc[2] += p[2] as u64 * a;
                    acc[3] += a;
                    n += 1;
                }
            }
            let d = &mut out[(ty * tw_ + tx) * 4..][..4];
            for c in 0..3 {
                d[c] = (acc[c] + acc[3] / 2).checked_div(acc[3]).unwrap_or(0) as u8;
            }
            d[3] = ((acc[3] + n / 2) / n.max(1)) as u8;
        }
    }
    orient_rgba(tw, th, out, orient)
}

/// One straight RGBA8 pixel of a raster.
fn pixel(r: &Raster, x: usize, y: usize, peak: f32, exposure: f32) -> [u8; 4] {
    let sample = |p: usize| -> u8 {
        let pl = &r.planes[p.min(r.planes.len() - 1)];
        let px = (x / pl.h_div.max(1) as usize).min(pl.width.saturating_sub(1) as usize);
        let py = (y / pl.v_div.max(1) as usize).min(pl.height.saturating_sub(1) as usize);
        pl.data.get(py * pl.stride + px).copied().unwrap_or(128)
    };
    match r.layout {
        PixelLayout::Gray => {
            let v = sample(0);
            [v, v, v, 255]
        }
        PixelLayout::YCbCr => {
            let yy = sample(0) as f32;
            let cb = sample(1) as f32 - 128.0;
            let cr = sample(2) as f32 - 128.0;
            [
                clamp8(yy + 1.402 * cr),
                clamp8(yy - 0.344_136 * cb - 0.714_136 * cr),
                clamp8(yy + 1.772 * cb),
                255,
            ]
        }
        PixelLayout::Rgb => [sample(0), sample(1), sample(2), 255],
        // YCbCr planes are converted in `thumbnail`; this covers a raster whose planes are
        // missing (nearest chroma, the signal's matrix and range, its transfer left as is).
        PixelLayout::Yuv(e) => {
            let wide = e.bits > 8;
            let code = |p: usize| -> f32 {
                let pl = &r.planes[p.min(r.planes.len() - 1)];
                let px = (x / pl.h_div.max(1) as usize).min(pl.width.saturating_sub(1) as usize);
                let py = (y / pl.v_div.max(1) as usize).min(pl.height.saturating_sub(1) as usize);
                if wide {
                    let i = py * pl.stride + px * 2;
                    pl.data
                        .get(i..i + 2)
                        .map_or(0.0, |b| f32::from(u16::from_ne_bytes([b[0], b[1]])))
                } else {
                    f32::from(pl.data.get(py * pl.stride + px).copied().unwrap_or(128))
                }
            };
            let ([yo, ys], [co, cs]) = e.levels();
            let v = [
                (code(0) - yo) * ys,
                (code(1) - co) * cs,
                (code(2) - co) * cs,
            ];
            let m = e.matrix.to_rgb();
            let c = |row: [f32; 3]| clamp8((row[0] * v[0] + row[1] * v[1] + row[2] * v[2]) * 255.0);
            [c(m[0]), c(m[1]), c(m[2]), 255]
        }
        PixelLayout::Rgba => {
            let pl = &r.planes[0];
            let i = y * pl.stride + x * 4;
            match pl.data.get(i..i + 4) {
                Some(p) => [p[0], p[1], p[2], p[3]],
                None => [128, 128, 128, 255],
            }
        }
        PixelLayout::Rgba16(_)
        | PixelLayout::Rgba16F(_)
        | PixelLayout::Gray16
        | PixelLayout::GrayAlpha16 => {
            let pl = &r.planes[0];
            let n = r.layout.pixel_bytes();
            let i = y * pl.stride + x * n;
            match pl.data.get(i..i + n) {
                Some(p) => specular_core::hdr::wide_to_srgb8_exposed(r.layout, p, peak, exposure),
                None => [128, 128, 128, 255],
            }
        }
    }
}

fn clamp8(v: f32) -> u8 {
    v.round().clamp(0.0, 255.0) as u8
}

/// Applies an EXIF orientation to RGBA8 (flip first, then quarter turns clockwise: the
/// view's mapping).
pub fn orient_rgba(w: u32, h: u32, src: Vec<u8>, orient: Orientation) -> Pixels {
    if orient == Orientation::NORMAL || orient.0 == 0 {
        return Pixels::new(w, h, src);
    }
    let (w, h) = (w as usize, h as usize);
    let (q, flip) = orient.to_rotation_flip();
    let (dw, dh) = if q % 2 == 1 { (h, w) } else { (w, h) };
    let mut out = vec![0u8; src.len()];
    for y in 0..h {
        for x in 0..w {
            let (mut px, mut py, mut cw, mut ch) = (x, y, w, h);
            if flip {
                px = cw - 1 - px;
            }
            for _ in 0..q {
                let nx = ch - 1 - py;
                py = px;
                px = nx;
                std::mem::swap(&mut cw, &mut ch);
            }
            let s = (y * w + x) * 4;
            let d = (py * dw + px) * 4;
            out[d..d + 4].copy_from_slice(&src[s..s + 4]);
        }
    }
    Pixels::new(dw as u32, dh as u32, out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_core::RasterPlane;

    fn rgba_raster(w: u32, h: u32, f: impl Fn(u32, u32) -> [u8; 4]) -> Raster {
        let data = (0..w * h).flat_map(|i| f(i % w, i / w)).collect();
        Raster {
            width: w,
            height: h,
            origin: (0, 0),
            scale: 1,
            layout: PixelLayout::Rgba,
            planes: vec![RasterPlane {
                width: w,
                height: h,
                stride: w as usize * 4,
                data,
                h_div: 1,
                v_div: 1,
            }],
            truncated: false,
        }
    }

    #[test]
    fn scale_choice_and_fit() {
        let scales = [1, 2, 4, 8];
        assert_eq!(pick_scale(&scales, 4000, 256), 8);
        assert_eq!(pick_scale(&scales, 1000, 256), 2);
        assert_eq!(pick_scale(&scales, 100, 256), 1);
        assert_eq!(fit(4000, 3000, 256), (256, 192));
        assert_eq!(fit(30, 4000, 128), (1, 128));
        assert_eq!(fit(100, 50, 256), (100, 50));
    }

    #[test]
    fn averages_and_orients() {
        // Left half red, right half blue; 8x4 → 2x1.
        let r = rgba_raster(8, 4, |x, _| {
            if x < 4 {
                [255, 0, 0, 255]
            } else {
                [0, 0, 255, 255]
            }
        });
        let p = thumbnail(&r, Orientation::NORMAL, 2);
        assert_eq!((p.width, p.height), (2, 1));
        assert_eq!(&p.rgba, &[255, 0, 0, 255, 0, 0, 255, 255]);
        // Orientation 6 (90° clockwise): the 2x1 becomes 1x2 with red on top.
        let p = thumbnail(&r, Orientation(6), 2);
        assert_eq!((p.width, p.height), (1, 2));
        assert_eq!(&p.rgba[..4], &[255, 0, 0, 255]);
        let avg = p.average();
        assert_eq!(avg, [127, 0, 127, 255]);
    }

    #[test]
    fn transparent_pixels_add_no_colour() {
        let r = rgba_raster(2, 2, |x, y| {
            if (x, y) == (1, 1) {
                [0, 255, 0, 0]
            } else {
                [255, 0, 0, 255]
            }
        });
        let p = thumbnail(&r, Orientation::NORMAL, 1);
        assert_eq!(p.rgba, vec![255, 0, 0, 191]);
        assert!(p.has_alpha());
    }

    #[test]
    fn big_boxes_are_sampled() {
        // 1000x10 → 10x1 still gives the right means on a gradient.
        let r = rgba_raster(1000, 10, |x, _| [(x / 4) as u8, 0, 0, 255]);
        let p = thumbnail(&r, Orientation::NORMAL, 10);
        assert_eq!(p.width, 10);
        let firsts: Vec<u8> = p.rgba.chunks(4).map(|c| c[0]).collect();
        assert!(firsts.windows(2).all(|w| w[0] < w[1]), "{firsts:?}");
        assert!((firsts[0] as i32 - 12).abs() <= 3, "{firsts:?}");
    }

    #[test]
    fn red_edge_on_transparent_has_no_fringe() {
        // A hard red/transparent edge whose transparent side hides white and black colour:
        // a straight average would pull the edge pixels pink or dark.
        let r = rgba_raster(64, 64, |x, y| {
            if x < 29 {
                [255, 0, 0, 255]
            } else if (x + y) % 2 == 0 {
                [255, 255, 255, 0]
            } else {
                [0, 0, 0, 0]
            }
        });
        let p = thumbnail(&r, Orientation::NORMAL, 8);
        assert_eq!((p.width, p.height), (8, 8));
        for (i, c) in p.rgba.chunks_exact(4).enumerate() {
            let x = i % 8;
            if c[3] > 0 {
                // Every partly covered pixel is still pure red.
                assert_eq!(&c[..3], &[255, 0, 0], "x {x}: {c:?}");
            }
            match x {
                0..=2 => assert_eq!(c[3], 255),
                3 => assert!(c[3] > 0 && c[3] < 255, "edge alpha {}", c[3]),
                _ => assert_eq!(c[3], 0),
            }
        }
        // The upload form: opaque red stays, the edge is red scaled by its coverage in
        // linear light, the transparent side is zero.
        let pm = p.premultiplied().expect("has alpha");
        assert_eq!(&pm[..4], &[255, 0, 0, 255]);
        let e = &pm[12..16];
        let lin = specular_core::hdr::srgb_to_linear(e[0] as f32 / 255.0);
        assert!((lin - e[3] as f32 / 255.0).abs() < 0.01, "{e:?}");
        assert_eq!(&e[1..3], &[0, 0]);
        assert_eq!(&pm[16..20], &[0, 0, 0, 0]);
    }

    #[test]
    fn opaque_pixels_have_no_premultiplied_copy() {
        let r = rgba_raster(16, 16, |x, y| [x as u8 * 9, y as u8 * 9, 40, 255]);
        let p = thumbnail(&r, Orientation::NORMAL, 4);
        assert!(!p.premul_known());
        p.prepare();
        assert!(p.premul_known());
        assert!(!p.has_alpha());
        assert!(p.premultiplied().is_none());
        assert_eq!(p.bytes(), p.rgba.len());
        // Known opaque (a JPEG cache entry): no scan at all.
        let o = Pixels::opaque(2, 1, vec![1, 2, 3, 255, 4, 5, 6, 255]);
        assert!(o.premul_known() && !o.has_alpha());
        // One translucent pixel anywhere, even in the last chunk, is found.
        let mut v = vec![255u8; 4 * 5000];
        v[4 * 4999 + 3] = 254;
        let t = Pixels::new(5000, 1, v);
        assert!(t.has_alpha());
        assert_eq!(t.bytes(), 2 * 4 * 5000);
        // Equality ignores whether the copy was made.
        assert_eq!(t, Pixels::new(5000, 1, t.rgba.clone()));
    }
}
