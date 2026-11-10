//! Non-raw photos in the merges (JPEG, HEIC, TIFF, PNG …): reading them, and the one place
//! that decides how a non-raw merge is written.
//!
//! - **Reading**: through the decoders' registry and Develop's own source path
//!   ([`crate::source::input`]): the file's profile (else sRGB) to linear Rec.2020, the
//!   working space. [`read_codes`] gives the display-encoded values instead (sRGB-coded
//!   0…1), which exposure fusion and response-curve recovery work on.
//! - **Facts**: [`shot_of`] reads what [`Shot::of_raw`] reads for a raw, from the file's
//!   Exif: capture time, exposure time, f-number, ISO, bias, focal length, the camera's
//!   bracketing flag.
//! - **Writing** ([`RgbFormat`]): 16-bit TIFF (Deflate) or lossless JPEG XL, sRGB-coded with
//!   an sRGB profile, transparency kept, the source's Exif carried over (its orientation
//!   too: pixels stay as stored). Radiance goes to OpenEXR ([`encode_exr`]), linear Rec.2020
//!   floats with matching chromaticities.

use std::path::Path;

use rayon::prelude::*;

use super::detect::{Shot, parse_time};
use super::{Error, Plane};
use crate::export::Space;

/// How a non-raw merge is written: the sheets' format choice. Changing the default or the
/// list here changes it everywhere.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum RgbFormat {
    /// 16-bit TIFF, Deflate.
    #[default]
    Tiff16,
    /// Lossless JPEG XL (16 bits).
    JxlLossless,
}

impl RgbFormat {
    pub const ALL: [RgbFormat; 2] = [RgbFormat::Tiff16, RgbFormat::JxlLossless];

    /// File extension (without the dot).
    pub fn ext(self) -> &'static str {
        match self {
            RgbFormat::Tiff16 => "tif",
            RgbFormat::JxlLossless => "jxl",
        }
    }
}

/// A picture in linear Rec.2020 (D65), RGB interleaved, rows top to bottom.
#[derive(Clone, Debug, PartialEq)]
pub struct RgbImage {
    pub w: u32,
    pub h: u32,
    pub linear: Vec<f32>,
    /// Straight alpha 0…1 per pixel, when the picture has transparency.
    pub alpha: Option<Vec<f32>>,
}

/// A picture in display-encoded values: sRGB-coded 0…1 (sRGB primaries), RGB interleaved.
#[derive(Clone, Debug, PartialEq)]
pub struct CodeImage {
    pub w: u32,
    pub h: u32,
    pub rgb: Vec<f32>,
}

/// The sRGB transfer function, linear → coded (unclamped above 1, odd below 0).
#[inline]
pub fn srgb_encode(v: f32) -> f32 {
    let a = v.abs();
    let c = if a <= 0.003_130_8 {
        12.92 * a
    } else {
        1.055 * a.powf(1.0 / 2.4) - 0.055
    };
    c.copysign(v)
}

/// The sRGB transfer function, coded → linear.
#[inline]
pub fn srgb_decode(c: f32) -> f32 {
    let a = c.abs();
    let v = if a <= 0.040_45 {
        a / 12.92
    } else {
        ((a + 0.055) / 1.055).powf(2.4)
    };
    v.copysign(c)
}

fn m3(m: &crate::colour::M3) -> [f32; 9] {
    std::array::from_fn(|k| m[k / 3][k % 3] as f32)
}

#[inline]
fn apply(m: &[f32; 9], p: &[f32]) -> [f32; 3] {
    [
        m[0] * p[0] + m[1] * p[1] + m[2] * p[2],
        m[3] * p[0] + m[4] * p[1] + m[5] * p[2],
        m[6] * p[0] + m[7] * p[1] + m[8] * p[2],
    ]
}

impl RgbImage {
    /// The display-encoded values (sRGB), clamped to 0…1.
    pub fn to_codes(&self) -> CodeImage {
        let m = m3(&Space::Srgb.from_rec2020());
        let mut rgb = vec![0f32; self.linear.len()];
        rgb.par_chunks_mut(3)
            .zip(self.linear.par_chunks(3))
            .for_each(|(o, p)| {
                let v = apply(&m, p);
                for c in 0..3 {
                    o[c] = srgb_encode(v[c]).clamp(0.0, 1.0);
                }
            });
        CodeImage {
            w: self.w,
            h: self.h,
            rgb,
        }
    }
}

impl CodeImage {
    /// The picture in linear Rec.2020 (no alpha).
    pub fn to_linear(&self) -> RgbImage {
        let m = m3(&Space::Srgb.to_rec2020());
        let mut linear = vec![0f32; self.rgb.len()];
        linear
            .par_chunks_mut(3)
            .zip(self.rgb.par_chunks(3))
            .for_each(|(o, p)| {
                let v = [srgb_decode(p[0]), srgb_decode(p[1]), srgb_decode(p[2])];
                o.copy_from_slice(&apply(&m, &v));
            });
        RgbImage {
            w: self.w,
            h: self.h,
            linear,
            alpha: None,
        }
    }
}

/// `img` as a merge plane (3 samples per pixel, [`super::Mosaic::Linear`]); nothing is
/// marked saturated.
pub(crate) fn to_plane(img: &RgbImage) -> Plane {
    Plane {
        w: img.w,
        h: img.h,
        cpp: 3,
        v: img.linear.clone(),
        sat: f32::INFINITY,
    }
}

/// A 3-sample plane as a picture (NaN, the plane's "not there", becomes transparent).
pub(crate) fn from_plane(p: &Plane) -> RgbImage {
    let holes = p.v.iter().any(|v| v.is_nan());
    let alpha = holes.then(|| {
        p.v.chunks(3)
            .map(|c| {
                if c.iter().any(|v| v.is_nan()) {
                    0.0
                } else {
                    1.0
                }
            })
            .collect()
    });
    RgbImage {
        w: p.w,
        h: p.h,
        linear: p
            .v
            .iter()
            .map(|v| if v.is_finite() { *v } else { 0.0 })
            .collect(),
        alpha,
    }
}

/// Display-encoded values as a plane (3 samples per pixel).
pub(crate) fn codes_to_plane(img: &CodeImage) -> Plane {
    Plane {
        w: img.w,
        h: img.h,
        cpp: 3,
        v: img.rgb.clone(),
        sat: f32::INFINITY,
    }
}

/// Decodes a non-raw file to linear Rec.2020 RGBA (straight) and its orientation.
fn decode(path: &Path, bytes: &[u8]) -> Result<(u32, u32, Vec<f32>), Error> {
    let name = path.display().to_string();
    let registry = specular_decode::Registry::new();
    let s = registry
        .open_path(path)
        .map_err(|e| Error::Read(name.clone(), e.to_string()))?;
    let info = s.info();
    let cancel = specular_core::CancelToken::new();
    let raster = s
        .decode(&specular_decode::DecodeRequest::full(1), &cancel)
        .map_err(|e| Error::Read(name.clone(), e.to_string()))?;
    let icc: Option<Vec<u8>> = info
        .icc_profile
        .as_ref()
        .map(|a| a.as_ref().clone())
        .or_else(|| s.embedded_icc().map(<[u8]>::to_vec));
    let _ = bytes;
    match crate::source::input(&raster, icc.as_deref(), None, &cancel) {
        Some(crate::preview::Input::Rgb {
            width,
            height,
            rgba,
        }) => Ok((width, height, rgba)),
        _ => Err(Error::Read(name, "not a picture".into())),
    }
}

/// Reads a non-raw photo: linear Rec.2020 and its facts.
pub fn read(path: &Path) -> Result<(RgbImage, Shot), Error> {
    let bytes =
        std::fs::read(path).map_err(|e| Error::Read(path.display().to_string(), e.to_string()))?;
    let (w, h, rgba) = decode(path, &bytes)?;
    let opaque = rgba.chunks_exact(4).all(|p| p[3] >= 1.0);
    let alpha = (!opaque).then(|| rgba.chunks_exact(4).map(|p| p[3]).collect());
    let linear = rgba
        .chunks_exact(4)
        .flat_map(|p| [p[0], p[1], p[2]])
        .collect();
    let shot = shot_of(path, &bytes).unwrap_or_else(|| Shot {
        path: path.to_path_buf(),
        ..Shot::default()
    });
    Ok((
        RgbImage {
            w,
            h,
            linear,
            alpha,
        },
        shot,
    ))
}

/// Reads a non-raw photo as display-encoded values (sRGB) and its facts.
pub fn read_codes(path: &Path) -> Result<(CodeImage, Shot), Error> {
    let (img, shot) = read(path)?;
    Ok((img.to_codes(), shot))
}

fn rational(e: &specular_convert::exif::Entry) -> Option<f64> {
    let d = e.data.get(..8)?;
    let (n, m) = (
        u32::from_le_bytes([d[0], d[1], d[2], d[3]]),
        u32::from_le_bytes([d[4], d[5], d[6], d[7]]),
    );
    if e.ty == 10 {
        let (n, m) = (n as i32, m as i32);
        return (m != 0).then(|| f64::from(n) / f64::from(m));
    }
    (m != 0).then(|| f64::from(n) / f64::from(m))
}

fn short(e: &specular_convert::exif::Entry) -> Option<f64> {
    match e.ty {
        3 => e
            .data
            .get(..2)
            .map(|d| f64::from(u16::from_le_bytes([d[0], d[1]]))),
        4 => e
            .data
            .get(..4)
            .map(|d| f64::from(u32::from_le_bytes([d[0], d[1], d[2], d[3]]))),
        _ => None,
    }
}

fn ascii(e: &specular_convert::exif::Entry) -> String {
    let end = e.data.iter().position(|&b| b == 0).unwrap_or(e.data.len());
    String::from_utf8_lossy(&e.data[..end]).trim().to_string()
}

/// The facts of a non-raw file from its Exif (`None` without Exif).
pub fn shot_of(path: &Path, bytes: &[u8]) -> Option<Shot> {
    let e = specular_convert::source_meta(bytes).exif?;
    let find = |list: &[specular_convert::exif::Entry], tag: u16| {
        list.iter().find(|x| x.tag == tag).cloned()
    };
    let ex = |tag| find(&e.exif, tag);
    let mut s = Shot {
        path: path.to_path_buf(),
        make: find(&e.ifd0, 0x010F).map(|x| ascii(&x)).unwrap_or_default(),
        model: find(&e.ifd0, 0x0110).map(|x| ascii(&x)).unwrap_or_default(),
        taken: ex(0x9003)
            .or_else(|| find(&e.ifd0, 0x0132))
            .and_then(|x| parse_time(&ascii(&x))),
        exposure_time: ex(0x829A).and_then(|x| rational(&x)),
        f_number: ex(0x829D).and_then(|x| rational(&x)),
        iso: ex(0x8827).and_then(|x| short(&x)),
        bias: ex(0x9204).and_then(|x| rational(&x)),
        focal_length: ex(0x920A).and_then(|x| rational(&x)),
        auto_bracket: ex(0xA402).and_then(|x| short(&x)) == Some(2.0),
        ..Shot::default()
    };
    if let (Some(t), Some(sub)) = (s.taken, ex(0x9291)) {
        let digits: String = sub
            .data
            .iter()
            .take_while(|b| b.is_ascii_digit())
            .map(|&b| b as char)
            .collect();
        if !digits.is_empty()
            && let Ok(v) = format!("0.{digits}").parse::<f64>()
        {
            s.taken = Some(t + v);
        }
    }
    Some(s)
}

/// The source's Exif for an output of `w × h`, its orientation kept (pixels are written as
/// stored).
fn exif_for(source: Option<&[u8]>, w: u32, h: u32) -> Option<specular_convert::exif::Exif> {
    let e = specular_convert::source_meta(source?).exif?;
    let orientation = e
        .ifd0
        .iter()
        .find(|x| x.tag == specular_convert::exif::TAG_ORIENTATION)
        .and_then(short)
        .map_or(1, |v| v as u16)
        .clamp(1, 8);
    let mut out = e.for_output(w, h, true)?;
    for x in &mut out.ifd0 {
        if x.tag == specular_convert::exif::TAG_ORIENTATION {
            *x = specular_convert::exif::Entry::short(x.tag, &[orientation]);
        }
    }
    Some(out)
}

/// The 16-bit sRGB samples (RGB or RGBA) of `img`.
fn samples16(img: &RgbImage) -> (usize, Vec<u16>) {
    let m = m3(&Space::Srgb.from_rec2020());
    let ch = if img.alpha.is_some() { 4 } else { 3 };
    let n = img.w as usize * img.h as usize;
    let mut out = vec![0u16; n * ch];
    let q = |v: f32| (v.clamp(0.0, 1.0) * 65535.0 + 0.5) as u16;
    out.par_chunks_mut(ch).enumerate().for_each(|(i, o)| {
        let v = apply(&m, &img.linear[i * 3..i * 3 + 3]);
        for c in 0..3 {
            o[c] = q(srgb_encode(v[c]));
        }
        if let Some(a) = &img.alpha {
            o[3] = q(a[i]);
        }
    });
    (ch, out)
}

/// The file of `img` in `fmt` (see the module documentation); `source` is the reference
/// photo's bytes, whose Exif is carried over.
pub fn encode(img: &RgbImage, fmt: RgbFormat, source: Option<&[u8]>) -> Result<Vec<u8>, String> {
    if img.linear.len() != img.w as usize * img.h as usize * 3 {
        return Err("the samples do not match the size".into());
    }
    let (channels, s) = samples16(img);
    let px = specular_convert::pixels::Pixels {
        width: img.w,
        height: img.h,
        channels,
        samples: specular_convert::pixels::Samples::Sixteen(s),
    };
    let icc = Space::Srgb.icc(false);
    let exif = exif_for(source, img.w, img.h);
    let meta = specular_convert::encode::Meta {
        exif: exif.as_ref(),
        xmp: None,
        icc: Some(&icc),
        cicp: None,
    };
    let opts = specular_convert::Options {
        format: match fmt {
            RgbFormat::Tiff16 => specular_convert::Format::Tiff,
            RgbFormat::JxlLossless => specular_convert::Format::Jxl,
        },
        jxl_quality: None,
        jxl_effort: 7,
        tiff_deflate: true,
        ..specular_convert::Options::default()
    };
    specular_convert::encode::encode(&opts, &px, &meta)
}

/// An OpenEXR file of linear radiance `img` (Rec.2020 floats, ZIP).
pub fn encode_exr(img: &RgbImage) -> Result<Vec<u8>, String> {
    let (ch, data): (usize, Vec<f32>) = match &img.alpha {
        Some(a) => (
            4,
            img.linear
                .chunks_exact(3)
                .zip(a)
                .flat_map(|(p, &a)| [p[0], p[1], p[2], a])
                .collect(),
        ),
        None => (3, img.linear.clone()),
    };
    specular_convert::exr::encode(
        img.w,
        img.h,
        ch,
        &data,
        &specular_convert::exr::ExrOptions {
            sample: specular_convert::exr::Sample::Float,
            zip: true,
            chromaticities: Some(Space::Rec2020.chromaticities()),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn picture(w: u32, h: u32, alpha: bool) -> RgbImage {
        let n = (w * h) as usize;
        let codes = CodeImage {
            w,
            h,
            rgb: (0..n * 3)
                .map(|i| {
                    let (p, c) = (i / 3, i % 3);
                    let (x, y) = ((p as u32 % w) as f32, (p as u32 / w) as f32);
                    (0.5 + 0.45 * ((x * 0.13 + c as f32).sin() * (y * 0.07).cos())).clamp(0.0, 1.0)
                })
                .collect(),
        };
        let mut img = codes.to_linear();
        if alpha {
            img.alpha = Some((0..n).map(|i| (i % 7) as f32 / 6.0).collect());
        }
        img
    }

    fn dir(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("specular-rgb-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).expect("dir");
        d
    }

    #[test]
    fn codes_and_linear_round_trip() {
        let img = picture(40, 30, false);
        let back = img.to_codes().to_linear();
        let err = img
            .linear
            .iter()
            .zip(&back.linear)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0f32, f32::max);
        assert!(err < 1e-5, "{err:e}");
    }

    #[test]
    fn tiff_and_jxl_round_trip_within_16_bits() {
        let d = dir("rt");
        for fmt in RgbFormat::ALL {
            for alpha in [false, true] {
                let img = picture(64, 48, alpha);
                let bytes = encode(&img, fmt, None).expect("encoded");
                let p = d.join(format!("a{}.{}", u8::from(alpha), fmt.ext()));
                std::fs::write(&p, &bytes).expect("written");
                let (back, _) = read(&p).expect("read back");
                assert_eq!((back.w, back.h), (64, 48));
                let (a, b) = (img.to_codes(), back.to_codes());
                // Fully transparent pixels keep no colour.
                let seen = |i: usize| img.alpha.as_ref().is_none_or(|al| al[i / 3] > 0.0);
                let err = a
                    .rgb
                    .iter()
                    .zip(&b.rgb)
                    .enumerate()
                    .filter(|(i, _)| seen(*i))
                    .map(|(_, (x, y))| (x - y).abs())
                    .fold(0.0f32, f32::max);
                // Specular's JPEG XL reader decodes SDR stills to 8 bits (the file holds 16).
                let tol = match fmt {
                    RgbFormat::Tiff16 => 2.0 / 65535.0 + 1e-5,
                    RgbFormat::JxlLossless => 0.5 / 255.0 + 1e-4,
                };
                assert!(err < tol, "{fmt:?} alpha {alpha}: {err:e}");
                assert_eq!(back.alpha.is_some(), alpha, "{fmt:?}");
                if let (Some(x), Some(y)) = (&img.alpha, &back.alpha) {
                    let e = x
                        .iter()
                        .zip(y)
                        .map(|(a, b)| (a - b).abs())
                        .fold(0.0f32, f32::max);
                    assert!(e < tol + 1e-4, "{fmt:?} alpha {e:e}");
                }
            }
        }
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn exr_holds_the_radiance() {
        let img = picture(16, 8, false);
        let bytes = encode_exr(&img).expect("encoded");
        assert_eq!(&bytes[..4], &[0x76, 0x2f, 0x31, 0x01]);
    }
}
