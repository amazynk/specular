//! From linear light to a recipe's file: crop and resize (Lanczos-3 in linear light), the
//! output colour space and its curve, output sharpening, the watermark, then 8 or 16 bits
//! and the encoder with the chosen metadata and profile.

use rayon::prelude::*;

use super::size::{self, Target};
use super::space::Primaries;
use super::watermark::{self, Mark};
use super::{Bits, Medium, Recipe, Sharpen, Strength};
use crate::encode::{self, Meta};
use crate::exif::Exif;
use crate::pixels::{Pixels, Samples};

/// A rendered picture: upright, linear light, premultiplied RGBA.
#[derive(Clone, Debug, PartialEq)]
pub struct Linear {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<f32>,
    pub primaries: Primaries,
}

impl Linear {
    /// Whether any pixel is less than opaque.
    pub fn has_alpha(&self) -> bool {
        self.rgba.chunks_exact(4).any(|p| p[3] < 0.999)
    }
}

/// Lanczos-3 resize of premultiplied RGBA, the part `crop` (fractions) of `src` to
/// `w × h`.
pub fn resize(src: &Linear, crop: [f64; 4], w: u32, h: u32) -> Linear {
    let (sw, sh) = (src.width as usize, src.height as usize);
    let x0 = ((crop[0] * sw as f64).round() as usize).min(sw - 1);
    let y0 = ((crop[1] * sh as f64).round() as usize).min(sh - 1);
    let cw = ((crop[2] * sw as f64).round() as usize).clamp(1, sw - x0);
    let ch = ((crop[3] * sh as f64).round() as usize).clamp(1, sh - y0);
    let (w, h) = (w.max(1) as usize, h.max(1) as usize);
    if (x0, y0, cw, ch) == (0, 0, sw, sh) && (w, h) == (sw, sh) {
        return src.clone();
    }
    let hx = crate::pixels::taps(cw, w);
    let vy = crate::pixels::taps(ch, h);
    // Horizontal pass over the rows the crop uses.
    let mut mid = vec![0f32; w * 4 * ch];
    mid.par_chunks_mut(w * 4).enumerate().for_each(|(y, out)| {
        let row = &src.rgba[((y0 + y) * sw + x0) * 4..((y0 + y) * sw + x0 + cw) * 4];
        for (o, t) in out.chunks_exact_mut(4).zip(&hx) {
            let mut a = [0f32; 4];
            for (j, &k) in t.w.iter().enumerate() {
                let p = (t.start + j) * 4;
                for c in 0..4 {
                    a[c] += k * row[p + c];
                }
            }
            o.copy_from_slice(&a);
        }
    });
    let mut out = vec![0f32; w * h * 4];
    out.par_chunks_mut(w * 4).zip(&vy).for_each(|(o, t)| {
        for (j, &k) in t.w.iter().enumerate() {
            let r = &mid[(t.start + j) * w * 4..(t.start + j + 1) * w * 4];
            for (a, v) in o.iter_mut().zip(r) {
                *a += k * v;
            }
        }
    });
    Linear {
        width: w as u32,
        height: h as u32,
        rgba: out,
        primaries: src.primaries,
    }
}

/// Encoded (0..1) RGB in the recipe's space, and alpha when kept.
#[derive(Clone, Debug, PartialEq)]
pub struct Encoded {
    pub width: u32,
    pub height: u32,
    pub rgb: Vec<f32>,
    pub alpha: Option<Vec<f32>>,
}

/// Linear light into `r`'s space and curve; transparency kept where the format holds it,
/// else flattened onto white.
pub fn encode_space(img: &Linear, r: &Recipe) -> Encoded {
    let keep_alpha = r.format.alpha() && img.has_alpha();
    let m = r.space.from_linear(img.primaries);
    let space = r.space;
    let n = (img.width * img.height) as usize;
    let mut rgb = vec![0f32; n * 3];
    let mut alpha = keep_alpha.then(|| vec![0f32; n]);
    rgb.par_chunks_mut(3)
        .zip(img.rgba.par_chunks_exact(4))
        .for_each(|(o, p)| {
            let a = p[3].clamp(0.0, 1.0);
            let c = if keep_alpha {
                if a > 1e-6 {
                    [p[0] / a, p[1] / a, p[2] / a]
                } else {
                    [0.0; 3]
                }
            } else {
                // Onto white.
                [p[0] + 1.0 - a, p[1] + 1.0 - a, p[2] + 1.0 - a]
            };
            let t = specular_color::math::mul3(&m, c);
            for k in 0..3 {
                o[k] = space.encode(t[k]);
            }
        });
    if let Some(al) = alpha.as_mut() {
        for (o, p) in al.iter_mut().zip(img.rgba.chunks_exact(4)) {
            *o = p[3].clamp(0.0, 1.0);
        }
    }
    Encoded {
        width: img.width,
        height: img.height,
        rgb,
        alpha,
    }
}

/// Gaussian radius (σ in pixels) and amount of output sharpening for a picture whose
/// longer side is `long` px. Prints sharpen more and wider, more so for matte paper and
/// for bigger files (printed larger or at a higher resolution).
pub fn sharpen_params(s: Sharpen, long: u32) -> Option<(f32, f32)> {
    let k = match s.strength {
        Strength::Low => 0,
        Strength::Standard => 1,
        Strength::High => 2,
    };
    let print_scale = (long as f32 / 3600.0).clamp(0.6, 2.0);
    match s.medium {
        Medium::Off => None,
        Medium::Screen => Some((0.6, [0.4, 0.7, 1.0][k])),
        Medium::Glossy => Some((0.9 * print_scale, [0.5, 0.8, 1.1][k])),
        Medium::Matte => Some((1.3 * print_scale, [0.7, 1.0, 1.4][k])),
    }
}

/// Unsharp mask on the luma of encoded RGB: the detail of luma against its Gaussian blur
/// (σ), times `amount`, added to each channel. Differences under one 8-bit code are left
/// alone so flat areas and fine noise stay as they are.
pub fn sharpen(img: &mut Encoded, sigma: f32, amount: f32) {
    let (w, h) = (img.width as usize, img.height as usize);
    if w < 3 || h < 3 || amount <= 0.0 {
        return;
    }
    let r = (sigma * 3.0).ceil().max(1.0) as isize;
    let kernel: Vec<f32> = {
        let k: Vec<f32> = (-r..=r)
            .map(|i| (-(i * i) as f32 / (2.0 * sigma * sigma)).exp())
            .collect();
        let s: f32 = k.iter().sum();
        k.into_iter().map(|v| v / s).collect()
    };
    let luma: Vec<f32> = img
        .rgb
        .par_chunks_exact(3)
        .map(|p| 0.2126 * p[0] + 0.7152 * p[1] + 0.0722 * p[2])
        .collect();
    let clamp = |i: isize, n: usize| i.clamp(0, n as isize - 1) as usize;
    let mut tmp = vec![0f32; w * h];
    tmp.par_chunks_mut(w).enumerate().for_each(|(y, out)| {
        let row = &luma[y * w..(y + 1) * w];
        for (x, o) in out.iter_mut().enumerate() {
            *o = kernel
                .iter()
                .enumerate()
                .map(|(j, k)| k * row[clamp(x as isize + j as isize - r, w)])
                .sum();
        }
    });
    let threshold = 0.5 / 255.0;
    img.rgb
        .par_chunks_mut(w * 3)
        .enumerate()
        .for_each(|(y, out)| {
            for x in 0..w {
                let blur: f32 = kernel
                    .iter()
                    .enumerate()
                    .map(|(j, k)| k * tmp[clamp(y as isize + j as isize - r, h) * w + x])
                    .sum();
                let d = luma[y * w + x] - blur;
                if d.abs() < threshold {
                    continue;
                }
                let add = amount * d;
                for c in &mut out[x * 3..x * 3 + 3] {
                    *c = (*c + add).clamp(0.0, 1.0);
                }
            }
        });
}

/// Draws `mark` on `img` as `r`'s watermark says (its colours, sRGB, into the recipe's
/// space and curve). Returns where it went (x, y, w, h).
pub fn draw_mark(img: &mut Encoded, mark: &Mark, r: &Recipe) -> (u32, u32, u32, u32) {
    let wm = &r.watermark;
    let (x0, y0, mw, mh) = watermark::place(img.width, img.height, (mark.w, mark.h), wm);
    let px = watermark::scaled(mark, mw, mh);
    let m = r.space.from_linear(Primaries::Bt709);
    let space = r.space;
    let opacity = f32::from(wm.opacity.clamp(1, 100)) / 100.0;
    let w = img.width as usize;
    for y in 0..mh as usize {
        let iy = y0 as usize + y;
        if iy >= img.height as usize {
            break;
        }
        for x in 0..mw as usize {
            let ix = x0 as usize + x;
            if ix >= w {
                break;
            }
            let s = &px[(y * mw as usize + x) * 4..][..4];
            let a = f32::from(s[3]) / 255.0 * opacity;
            if a <= 0.0 {
                continue;
            }
            let lin = [s[0], s[1], s[2]]
                .map(|v| specular_color::math::srgb_to_linear(f32::from(v) / 255.0));
            let t = specular_color::math::mul3(&m, lin);
            let o = &mut img.rgb[(iy * w + ix) * 3..][..3];
            for k in 0..3 {
                o[k] = o[k] * (1.0 - a) + space.encode(t[k]) * a;
            }
            if let Some(al) = img.alpha.as_mut() {
                let d = &mut al[iy * w + ix];
                *d = *d + a * (1.0 - *d);
            }
        }
    }
    (x0, y0, mw, mh)
}

/// 8 or 16-bit samples.
pub fn quantise(img: &Encoded, bits: Bits) -> Pixels {
    let c = if img.alpha.is_some() { 4 } else { 3 };
    let n = (img.width * img.height) as usize;
    let mut v = vec![0f32; n * c];
    for i in 0..n {
        v[i * c..i * c + 3].copy_from_slice(&img.rgb[i * 3..i * 3 + 3]);
        if let Some(a) = &img.alpha {
            v[i * c + 3] = a[i];
        }
    }
    let samples = match bits {
        Bits::Eight => Samples::Eight(
            v.iter()
                .map(|x| (x.clamp(0.0, 1.0) * 255.0 + 0.5) as u8)
                .collect(),
        ),
        Bits::Sixteen => Samples::Sixteen(
            v.iter()
                .map(|x| (x.clamp(0.0, 1.0) * 65535.0 + 0.5) as u16)
                .collect(),
        ),
    };
    Pixels {
        width: img.width,
        height: img.height,
        channels: c,
        samples,
    }
}

/// What [`finish`] made.
pub struct Finished {
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// The output size of `r` for a picture `full` (upright, full size).
pub fn target_of(r: &Recipe, full: (u32, u32)) -> Target {
    size::target(full.0, full.1, r.size, r.enlarge)
}

/// The file `r` makes of `img` (rendered from a picture of `full` size, upright), with
/// `exif` and `xmp` (already filtered) and the watermark `mark` drawn for the recipe.
pub fn finish(
    img: &Linear,
    full: (u32, u32),
    r: &Recipe,
    mark: Option<&Mark>,
    exif: Option<&Exif>,
    xmp: Option<&[u8]>,
) -> Result<Finished, String> {
    let t = target_of(r, full);
    let sized = resize(img, t.crop, t.width, t.height);
    let mut enc = encode_space(&sized, r);
    drop(sized);
    if let Some((sigma, amount)) = sharpen_params(r.sharpen, t.width.max(t.height)) {
        sharpen(&mut enc, sigma, amount);
    }
    if let Some(m) = mark {
        draw_mark(&mut enc, m, r);
    }
    let bits = if r.format.sixteen() {
        r.bits
    } else {
        Bits::Eight
    };
    let px = quantise(&enc, bits);
    drop(enc);
    let meta = Meta {
        exif,
        xmp,
        icc: r.embed_icc.then(|| r.space.icc()),
        cicp: None,
    };
    let bytes = encode::encode(&r.options(), &px, &meta)?;
    Ok(Finished {
        bytes,
        width: px.width,
        height: px.height,
    })
}
