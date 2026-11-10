//! LabEye IM images (`.im`), IFUNC's format, also written by Pillow.
//!
//! A text header of `Key: value` lines (`Image type`, `Name`, `Image size (x*y)`,
//! `File size (no of images)`, `Lut`), padded to 512 bytes (ended by 0x1A). With `Lut`, a
//! 768-byte palette follows (256 reds, then greens, then blues). The pixels are row by row
//! from the bottom up; images with several channels store each row once per channel ("line
//! interleaved").
//!
//! Image types: `0 1` and `L 1` (1 bit, 1 = white), `Greyscale`/`Grayscale`/`L`/`P` (8-bit;
//! a palette with `Lut`), `LA` and `PA`, `RGB`, `RGBA`, `RGBX`, `CMYK` (shown uncalibrated),
//! `X 24` (packed RGB), `YCC` (YCbCr, full range), `L 16`/`L 16B` (16-bit little or big
//! endian), `L 8S`/`L 16S` (signed) and `L 32S`, `L 32` and `L 32F` (32-bit integers or
//! floats); signed and 32-bit values are stretched from their own range for display.
//! Several images in a file are pages. The 2- and 4-bit (`B2`, `B4`) and planar
//! (`RGB3`, `RYB3`) types of the oldest LabEye versions are refused by name.

use crate::swatch::short;
use crate::{Error, Image, Info, Model, Pixels, Result, check_dims, check_payload, stretch16};

const BLOCK: usize = 512;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Bit,
    Grey,
    GreyAlpha,
    Rgb,
    Rgba,
    Rgbx,
    /// RGB packed per pixel.
    Rgb24,
    Cmyk,
    Ycc,
    /// 16-bit grey; big endian when true.
    Grey16(bool),
    /// Signed 8- or 16-bit (little endian).
    Signed(u8),
    Int32,
    Float32,
}

impl Kind {
    fn of(t: &str) -> Option<Kind> {
        Some(match t {
            "0 1" | "L 1" | "1" | "B1" => Kind::Bit,
            "Greyscale" | "Grayscale" | "Gray" | "L" | "L 8" | "P" | "P 8" => Kind::Grey,
            "LA" | "PA" => Kind::GreyAlpha,
            "RGB" | "RGB3" => Kind::Rgb,
            "RGBA" | "RGB4" => Kind::Rgba,
            "RGBX" => Kind::Rgbx,
            "X 24" => Kind::Rgb24,
            "L 8S" => Kind::Signed(1),
            "L 16S" => Kind::Signed(2),
            "CMYK" => Kind::Cmyk,
            "YCC" => Kind::Ycc,
            "L 16" | "L 16L" => Kind::Grey16(false),
            "L 16B" => Kind::Grey16(true),
            "L 32S" | "L 32 S" | "L 32" | "I 32" | "I" => Kind::Int32,
            "L 32F" | "L 32 F" | "F 32" | "F" => Kind::Float32,
            _ => return None,
        })
    }

    /// (channels, bytes per sample); 0 bytes for 1 bit.
    fn layout(self) -> (usize, usize) {
        match self {
            Kind::Bit => (1, 0),
            Kind::Grey => (1, 1),
            Kind::GreyAlpha => (2, 1),
            Kind::Rgb | Kind::Ycc | Kind::Rgb24 => (3, 1),
            Kind::Rgba | Kind::Rgbx | Kind::Cmyk => (4, 1),
            Kind::Grey16(_) => (1, 2),
            Kind::Signed(b) => (1, b as usize),
            Kind::Int32 | Kind::Float32 => (1, 4),
        }
    }

    fn row_bytes(self, w: usize) -> usize {
        match self.layout() {
            (_, 0) => w.div_ceil(8),
            (c, b) => w * c * b,
        }
    }
}

struct Header {
    kind: Kind,
    type_name: String,
    name: String,
    width: u32,
    height: u32,
    images: u32,
    palette: Option<Vec<[u8; 3]>>,
    data: usize,
}

pub fn probe(head: &[u8]) -> bool {
    head.starts_with(b"Image type:")
        || (head.starts_with(b"Name:") && head.windows(11).any(|w| w == b"Image type:"))
}

fn parse(d: &[u8]) -> Result<Header> {
    if !probe(d) {
        return Err(Error::NotRecognised);
    }
    let end = d
        .iter()
        .take(64 * BLOCK)
        .position(|&c| c == 0 || c == 0x1A)
        .ok_or_else(|| Error::Truncated("header".into()))?;
    let text = String::from_utf8_lossy(&d[..end]);
    let (mut ty, mut name, mut size, mut images, mut lut) = (None, String::new(), None, 1, false);
    for line in text.split(['\r', '\n']).filter(|l| !l.is_empty()) {
        let Some((k, v)) = line.split_once(':') else {
            continue;
        };
        let v = v.trim();
        match k.trim() {
            "Image type" => ty = Some(v.trim_end_matches("image").trim().to_owned()),
            "Name" => name = short(v),
            "Image size (x*y)" => {
                size = v.split_once('*').and_then(|(a, b)| {
                    Some((a.trim().parse::<u32>().ok()?, b.trim().parse::<u32>().ok()?))
                })
            }
            "File size (no of images)" => images = v.parse::<u32>().unwrap_or(1).max(1),
            "Lut" => lut = true,
            _ => {}
        }
    }
    let type_name = ty.ok_or_else(|| Error::Malformed("no image type".into()))?;
    let kind = Kind::of(&type_name).ok_or_else(|| {
        Error::Unsupported(format!("LabEye image type {type_name:?} is not supported"))
    })?;
    let (width, height) = size.ok_or_else(|| Error::Malformed("no image size".into()))?;
    check_dims(width.into(), height.into())?;
    let mut data = end.div_ceil(BLOCK).max(1) * BLOCK;
    let mut palette = None;
    if lut && kind == Kind::Grey {
        let p = d
            .get(data..data + 768)
            .ok_or_else(|| Error::Truncated("palette".into()))?;
        let pal: Vec<[u8; 3]> = (0..256).map(|i| [p[i], p[256 + i], p[512 + i]]).collect();
        // A grey ramp is no palette.
        if pal.iter().enumerate().any(|(i, c)| *c != [i as u8; 3]) {
            palette = Some(pal);
        }
        data += 768;
    } else if lut {
        data += 768;
    }
    Ok(Header {
        kind,
        type_name,
        name,
        width,
        height,
        images,
        palette,
        data,
    })
}

pub fn info(data: &[u8], page: u32) -> Result<Info> {
    let h = parse(data)?;
    if page >= h.images {
        return Err(Error::Malformed("no such image".into()));
    }
    let (model, depth, alpha) = match h.kind {
        Kind::Bit => (Model::Bilevel, 1, false),
        Kind::Grey if h.palette.is_some() => (Model::Palette, 8, false),
        Kind::Grey => (Model::Grey, 8, false),
        Kind::GreyAlpha => (Model::Grey, 8, true),
        Kind::Rgb | Kind::Rgbx | Kind::Rgb24 | Kind::Cmyk => (Model::Rgb, 8, false),
        Kind::Rgba => (Model::Rgb, 8, true),
        Kind::Ycc => (Model::YCbCr, 8, false),
        Kind::Grey16(_) => (Model::Grey, 16, false),
        Kind::Int32 | Kind::Float32 | Kind::Signed(_) => (Model::Grey, 16, false),
    };
    let mut fields = vec![("Image type".to_string(), h.type_name.clone())];
    if h.images > 1 {
        fields.push(("Images".into(), h.images.to_string()));
    }
    let mut summary = Vec::new();
    if !h.name.is_empty() {
        fields.push(("Name".into(), h.name.clone()));
    }
    let coding = match h.kind {
        Kind::Int32 => "32-bit integer grey, stretched for display".to_string(),
        Kind::Float32 => "32-bit float grey, stretched for display".to_string(),
        Kind::Signed(b) => format!("{}-bit signed grey, stretched for display", b * 8),
        Kind::Cmyk => "CMYK, shown uncalibrated".to_string(),
        _ => format!("{} image", h.type_name),
    };
    if h.kind == Kind::Cmyk {
        summary.push(("colour", "CMYK".to_string()));
    }
    Ok(Info {
        width: h.width,
        height: h.height,
        pages: h.images,
        bit_depth: depth,
        model,
        has_alpha: alpha,
        orientation: 1,
        coding,
        summary,
        fields,
    })
}

pub fn decode(data: &[u8], page: u32) -> Result<Image> {
    let h = parse(data)?;
    if page >= h.images {
        return Err(Error::Malformed("no such image".into()));
    }
    let (w, hh) = (h.width as usize, h.height as usize);
    let rb = h.kind.row_bytes(w);
    let frame = rb * hh;
    let start = h.data.saturating_add(frame.saturating_mul(page as usize));
    let src = data.get(start.min(data.len())..).unwrap_or(&[]);
    check_payload(src.len(), frame as u64, 1)?;
    let truncated = src.len() < frame;
    // Rows are stored bottom-up: reorder them top-down.
    let mut flipped = Vec::with_capacity(frame);
    for y in (0..hh).rev() {
        let row = src.get(y * rb..(y + 1) * rb).unwrap_or(&[]);
        flipped.extend_from_slice(row);
        flipped.resize(flipped.len() + rb - row.len(), crate::GREY);
    }
    let src = &flipped[..];
    let (ch, _) = h.kind.layout();
    let n = w * hh;
    let pixels = match h.kind {
        Kind::Bit => Pixels::Gray8(
            (0..n)
                .map(|i| {
                    let (x, y) = (i % w, i / w);
                    if (src[y * rb + x / 8] >> (7 - x % 8)) & 1 == 1 {
                        255
                    } else {
                        0
                    }
                })
                .collect(),
        ),
        Kind::Grey => match &h.palette {
            None => Pixels::Gray8(src.to_vec()),
            Some(p) => Pixels::Rgba8(
                src.iter()
                    .flat_map(|&i| {
                        let c = p[i as usize];
                        [c[0], c[1], c[2], 255]
                    })
                    .collect(),
            ),
        },
        Kind::Grey16(be) => Pixels::Gray16(
            src.chunks_exact(2)
                .map(|b| {
                    if be {
                        u16::from_be_bytes([b[0], b[1]])
                    } else {
                        u16::from_le_bytes([b[0], b[1]])
                    }
                })
                .collect(),
        ),
        Kind::Int32 | Kind::Float32 | Kind::Signed(_) => {
            let (_, b) = h.kind.layout();
            let v: Vec<f64> = src
                .chunks_exact(b)
                .map(|b| match h.kind {
                    Kind::Signed(1) => f64::from(b[0] as i8),
                    Kind::Signed(_) => f64::from(i16::from_le_bytes([b[0], b[1]])),
                    Kind::Int32 => f64::from(i32::from_le_bytes([b[0], b[1], b[2], b[3]])),
                    _ => f64::from(f32::from_le_bytes([b[0], b[1], b[2], b[3]])),
                })
                .collect();
            Pixels::Gray16(stretch16(&v))
        }
        Kind::Rgb24 => Pixels::Rgba8(
            src.chunks_exact(3)
                .flat_map(|p| [p[0], p[1], p[2], 255])
                .collect(),
        ),
        _ => {
            let mut out = Vec::with_capacity(n * 4);
            for y in 0..hh {
                let row = &src[y * rb..(y + 1) * rb];
                for x in 0..w {
                    let s = |c: usize| row[c * w + x];
                    out.extend_from_slice(&match h.kind {
                        Kind::GreyAlpha => [s(0), s(0), s(0), s(1)],
                        Kind::Rgb | Kind::Rgbx => [s(0), s(1), s(2), 255],
                        Kind::Rgba => [s(0), s(1), s(2), s(3)],
                        Kind::Cmyk => {
                            let k = 255 - u32::from(s(3));
                            let f = |c: u8| ((255 - u32::from(c)) * k / 255) as u8;
                            [f(s(0)), f(s(1)), f(s(2)), 255]
                        }
                        _ => {
                            let (y, cb, cr) = (
                                f64::from(s(0)),
                                f64::from(s(1)) - 128.0,
                                f64::from(s(2)) - 128.0,
                            );
                            let c = |v: f64| (v + 0.5).clamp(0.0, 255.0) as u8;
                            [
                                c(y + 1.402 * cr),
                                c(y - 0.344_136 * cb - 0.714_136 * cr),
                                c(y + 1.772 * cb),
                                255,
                            ]
                        }
                    });
                }
            }
            let _ = ch;
            Pixels::Rgba8(out)
        }
    };
    Ok(Image {
        width: h.width,
        height: h.height,
        pixels,
        truncated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(ty: &str, w: u32, h: u32, n: u32, lut: bool) -> Vec<u8> {
        let mut t = format!(
            "Image type: {ty} image\r\nName: t\r\nImage size (x*y): {w}*{h}\r\nFile size (no of images): {n}\r\n"
        );
        if lut {
            t += "Lut: 1\r\n";
        }
        let mut d = t.into_bytes();
        d.resize(511, 0);
        d.push(0x1A);
        d
    }

    #[test]
    fn types_and_pages() {
        let mut d = file("RGB", 2, 1, 2, false);
        d.extend_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
        let i = info(&d, 0).unwrap();
        assert_eq!((i.width, i.pages), (2, 2));
        assert_eq!(
            decode(&d, 1).unwrap().pixels,
            Pixels::Rgba8(vec![7, 9, 11, 255, 8, 10, 12, 255])
        );
        let mut p = file("Greyscale", 1, 1, 1, true);
        let mut lut = vec![0u8; 768];
        lut[5] = 200;
        p.extend_from_slice(&lut);
        p.push(5);
        assert_eq!(
            decode(&p, 0).unwrap().pixels,
            Pixels::Rgba8(vec![200, 0, 0, 255])
        );
        let mut b = file("0 1", 9, 1, 1, false);
        b.extend_from_slice(&[0x80, 0x80]);
        let Pixels::Gray8(v) = decode(&b, 0).unwrap().pixels else {
            panic!()
        };
        assert_eq!(v, [255, 0, 0, 0, 0, 0, 0, 0, 255]);
        assert!(matches!(
            info(&file("HSV", 1, 1, 1, false), 0),
            Err(Error::Unsupported(_))
        ));
    }
}
