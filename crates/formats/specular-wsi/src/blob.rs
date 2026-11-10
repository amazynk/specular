//! Stored images inside slide files (MIRAX tiles and slide images, VSI tiles): JPEG, PNG,
//! BMP and JPEG 2000 by their signatures, through the in-tree decoders.

use std::sync::atomic::AtomicBool;

use crate::{Error, Result, Rgba};
use specular_jpeg as jp;

/// Largest stored image decoded (pixels).
pub(crate) const MAX_PIXELS: u64 = 1 << 28;

/// What a blob is, by its first bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Jpeg,
    Png,
    Bmp,
    /// A JPEG 2000 codestream or JP2 file.
    Jp2,
}

pub(crate) fn kind(b: &[u8]) -> Option<Kind> {
    if b.starts_with(&[0xff, 0xd8]) {
        Some(Kind::Jpeg)
    } else if b.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some(Kind::Png)
    } else if b.starts_with(b"BM") {
        Some(Kind::Bmp)
    } else if b.starts_with(&[0xff, 0x4f, 0xff, 0x51]) || b.starts_with(b"\0\0\0\x0cjP  ") {
        Some(Kind::Jp2)
    } else {
        None
    }
}

/// Decode a stored image (any of [`Kind`]) as RGBA8.
pub(crate) fn rgba(b: &[u8], cancel: Option<&AtomicBool>) -> Result<Rgba> {
    match kind(b).ok_or_else(|| Error::Malformed("unknown image coding".into()))? {
        Kind::Jpeg => {
            let info = jp::read_info(b)?;
            if u64::from(info.width) * u64::from(info.height) > MAX_PIXELS {
                return Err(Error::TooLarge);
            }
            let p = jp::decode(
                b,
                &jp::DecodeOptions {
                    cancel,
                    parallelism: jp::Parallelism::Single,
                    ..Default::default()
                },
            )?;
            Ok(Rgba {
                width: p.width,
                height: p.height,
                data: jp::planes_to_rgba(&p),
                truncated: p.truncated,
            })
        }
        Kind::Png => {
            let img = specular_png::decode(
                b,
                &specular_png::DecodeOptions {
                    output: specular_png::Output::Rgba8,
                    parallelism: specular_png::Parallelism::Single,
                    cancel,
                    ..Default::default()
                },
            )
            .map_err(|e| Error::Malformed(format!("PNG: {e}")))?;
            Ok(Rgba {
                width: img.width,
                height: img.height,
                data: img.to_rgba8(),
                truncated: img.truncated,
            })
        }
        Kind::Bmp => {
            use specular_bitmap::Decode;
            let d = specular_bitmap::bmp::Bmp::parse(b)
                .map_err(|e| Error::Malformed(format!("BMP: {e}")))?;
            let img = d
                .decode(
                    b,
                    &specular_bitmap::Options {
                        parallel: false,
                        ..Default::default()
                    },
                    &|| false,
                )
                .map_err(|e| Error::Malformed(format!("BMP: {e}")))?;
            let data = match img.format {
                specular_bitmap::PixelFormat::Rgba8 => img.data,
                specular_bitmap::PixelFormat::Gray8 => {
                    img.data.iter().flat_map(|&g| [g, g, g, 255]).collect()
                }
                f => return Err(Error::Unsupported(format!("BMP pixels {f:?}"))),
            };
            Ok(Rgba {
                width: img.width,
                height: img.height,
                data,
                truncated: img.truncated,
            })
        }
        Kind::Jp2 => {
            let dec = specular_jp2::Decoder::new(b)?;
            let img = dec.decode(&specular_jp2::DecodeOptions::default())?;
            let n = img.width as usize * img.height as usize;
            let data = match (&img.pixels, img.format) {
                (specular_jp2::Pixels::U8(v), specular_jp2::PixelFormat::Rgba8) => v.clone(),
                (specular_jp2::Pixels::U8(v), _) => {
                    v.iter().take(n).flat_map(|&g| [g, g, g, 255]).collect()
                }
                (specular_jp2::Pixels::U16(v), specular_jp2::PixelFormat::Rgba16) => {
                    v.iter().map(|&s| (s >> 8) as u8).collect()
                }
                (specular_jp2::Pixels::U16(v), _) => v
                    .iter()
                    .take(n)
                    .flat_map(|&g| {
                        let g = (g >> 8) as u8;
                        [g, g, g, 255]
                    })
                    .collect(),
            };
            Ok(Rgba {
                width: img.width,
                height: img.height,
                data,
                truncated: img.truncated,
            })
        }
    }
}

/// A JPEG 2000 image's samples as stored: width, height, samples per pixel, interleaved.
pub(crate) fn jp2_values(b: &[u8]) -> Result<(u32, u32, usize, Vec<f32>)> {
    let dec = specular_jp2::Decoder::new(b)?;
    let comps = dec.decode_components(0, None)?;
    let (w, h) = comps
        .first()
        .map(|c| (c.width, c.height))
        .ok_or_else(|| Error::Malformed("JPEG 2000 without components".into()))?;
    if comps.iter().any(|x| x.width != w || x.height != h) {
        return Err(Error::Unsupported(
            "JPEG 2000 with subsampled components".into(),
        ));
    }
    let c = comps.len();
    let n = w as usize * h as usize;
    let mut out = vec![0f32; n * c];
    for (k, comp) in comps.iter().enumerate() {
        for (i, &s) in comp.samples.iter().take(n).enumerate() {
            out[i * c + k] = f32::from(s);
        }
    }
    Ok((w, h, c, out))
}

/// A stored image's samples as values (deep PNG, lossless JPEG, JPEG 2000; 8-bit images as
/// they decode): width, height, samples per pixel, interleaved.
pub(crate) fn values(b: &[u8], cancel: Option<&AtomicBool>) -> Result<(u32, u32, usize, Vec<f32>)> {
    match kind(b).ok_or_else(|| Error::Malformed("unknown image coding".into()))? {
        Kind::Jp2 => jp2_values(b),
        Kind::Jpeg => {
            // Deep or lossless frames keep their stored values; 8-bit colour converts to RGB.
            if let Ok(w) = jp::decode_wide(b, cancel)
                && (w.lossless || w.precision > 8 || w.planes.len() == 1)
            {
                let c = w.planes.len();
                let n = w.width as usize * w.height as usize;
                if c > 0 && w.planes.iter().all(|p| p.data.len() >= n) {
                    let mut out = vec![0f32; n * c];
                    for (k, p) in w.planes.iter().enumerate() {
                        for (i, &s) in p.data.iter().take(n).enumerate() {
                            out[i * c + k] = f32::from(s);
                        }
                    }
                    return Ok((w.width, w.height, c, out));
                }
            }
            let r = rgba(b, cancel)?;
            Ok(rgba_values(&r))
        }
        Kind::Png => {
            let img = specular_png::decode(
                b,
                &specular_png::DecodeOptions {
                    output: specular_png::Output::Rgba16,
                    parallelism: specular_png::Parallelism::Single,
                    cancel,
                    ..Default::default()
                },
            )
            .map_err(|e| Error::Malformed(format!("PNG: {e}")))?;
            let info =
                specular_png::read_info(b).map_err(|e| Error::Malformed(format!("PNG: {e}")))?;
            let p = &img.planes[0];
            let c = if info.color_type.channels() >= 3 {
                3
            } else {
                1
            };
            let n = img.width as usize * img.height as usize;
            let mut out = Vec::with_capacity(n * c);
            for y in 0..img.height as usize {
                for x in 0..img.width as usize {
                    let o = y * p.stride + x * 8;
                    for k in 0..c {
                        let v = u16::from_ne_bytes([p.data[o + k * 2], p.data[o + k * 2 + 1]]);
                        out.push(if info.bit_depth > 8 {
                            f32::from(v)
                        } else {
                            f32::from(v >> 8)
                        });
                    }
                }
            }
            Ok((img.width, img.height, c, out))
        }
        Kind::Bmp => {
            let r = rgba(b, cancel)?;
            Ok(rgba_values(&r))
        }
    }
}

/// RGB samples of an RGBA8 raster (grey when every pixel is grey: one sample).
fn rgba_values(r: &Rgba) -> (u32, u32, usize, Vec<f32>) {
    let grey = r.data.chunks_exact(4).all(|p| p[0] == p[1] && p[1] == p[2]);
    let c = if grey { 1 } else { 3 };
    let v = r
        .data
        .chunks_exact(4)
        .flat_map(|p| p[..c].iter().map(|&s| f32::from(s)).collect::<Vec<_>>())
        .collect();
    (r.width, r.height, c, v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_by_signature() {
        assert_eq!(kind(&[0xff, 0xd8, 0xff]), Some(Kind::Jpeg));
        assert_eq!(kind(b"\x89PNG\r\n\x1a\n...."), Some(Kind::Png));
        assert_eq!(kind(b"BM...."), Some(Kind::Bmp));
        assert_eq!(kind(&[0xff, 0x4f, 0xff, 0x51]), Some(Kind::Jp2));
        assert_eq!(kind(b"GIF89a"), None);
        assert!(rgba(b"junk", None).is_err());
    }

    #[test]
    fn jpeg_round_trip() {
        let px: Vec<u8> = (0..16 * 8).flat_map(|i| [(i * 2) as u8, 100, 50]).collect();
        let mut j = Vec::new();
        jpeg_encoder::Encoder::new(&mut j, 100)
            .encode(&px, 16, 8, jpeg_encoder::ColorType::Rgb)
            .unwrap();
        let r = rgba(&j, None).unwrap();
        assert_eq!((r.width, r.height), (16, 8));
        assert!((i32::from(r.data[4 * 5]) - 10).abs() <= 3);
        let (w, h, c, v) = values(&j, None).unwrap();
        assert_eq!((w, h, c, v.len()), (16, 8, 3, 16 * 8 * 3));
    }
}
