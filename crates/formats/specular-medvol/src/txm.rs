//! Zeiss Xradia TXM / TXRM / XRM files: Compound File Binary containers (read with
//! `specular-cfb`) holding an `ImageInfo` storage of facts and the images as streams
//! `ImageData{⌈n/100⌉}/Image{n}`, n from 1.
//!
//! * `ImageInfo/ImageWidth`, `ImageHeight` (i32), `ImagesTaken` (u32; `NoOfImages` when it
//!   is missing or 0), `DataType` (i32 XrmDataType: 2 int8, 3 uint8, 4 int16, 5 uint16,
//!   6 int32, 7 uint32, 8 int64, 9 uint64, 10 float32, 11 float64; missing: uint16 or
//!   float32 by the first image's length), `PixelSize` (f32, µm).
//! * Images are stored bottom row first relative to how Zeiss's XRM Data Explorer shows
//!   them: the stored rows run upward (the affine's j axis is negative), so the canonical
//!   order shows them as the vendor does.
//! * A mosaic (`MosiacColumns` × `MosiacRows`, sic) is one image stitched from its tiles:
//!   each band of `MosiacRows` tiles side by side, `MosiacColumns` bands one under the other
//!   (as txrm2tiff stitches them).
//! * `.txrm` holds projections (one per rotation angle), `.txm` a reconstruction (slices);
//!   files named otherwise are told apart by whether their angles vary.

use std::path::Path;

use specular_cfb::Cfb;

use crate::{
    DataFile, DataRef, DataType, Encoding, Error, Header, Kind, Layout, MedVolume,
    OrientationSource, Param, ParamValue, Result, Skip,
};

/// Whether `data` (the whole file) is an Xradia file: a compound file with image facts.
pub fn sniff(data: &[u8]) -> bool {
    specular_cfb::sniff(data) && Cfb::open(data).is_ok_and(|c| c.exists("ImageInfo/ImageWidth"))
}

fn cfb_err(e: specular_cfb::Error) -> Error {
    match e {
        specular_cfb::Error::Truncated => Error::Truncated("compound file".into()),
        e => Error::Malformed(e.to_string()),
    }
}

fn f32s(c: &Cfb, path: &str) -> Vec<f32> {
    c.read(path)
        .map(|b| {
            b.chunks_exact(4)
                .map(|x| f32::from_le_bytes([x[0], x[1], x[2], x[3]]))
                .collect()
        })
        .unwrap_or_default()
}

fn i32_of(c: &Cfb, path: &str) -> Option<i32> {
    let b = c.read(path).ok()?;
    let x = b.get(..4)?;
    Some(i32::from_le_bytes([x[0], x[1], x[2], x[3]]))
}

fn text_of(c: &Cfb, path: &str) -> Option<String> {
    let b = c.read(path).ok()?;
    let end = b.iter().position(|&x| x == 0).unwrap_or(b.len());
    let s: String = String::from_utf8_lossy(&b[..end])
        .chars()
        .filter(|c| !c.is_control())
        .collect();
    let s = s.trim();
    (!s.is_empty()).then(|| s.to_owned())
}

fn dtype_of(code: i32) -> Option<DataType> {
    Some(match code {
        2 => DataType::I8,
        3 => DataType::U8,
        4 => DataType::I16,
        5 => DataType::U16,
        6 => DataType::I32,
        7 => DataType::U32,
        8 => DataType::I64,
        9 => DataType::U64,
        10 => DataType::F32,
        11 => DataType::F64,
        _ => return None,
    })
}

/// The stream of image `n` (from 1).
fn image_path(n: usize) -> String {
    format!("ImageData{}/Image{n}", n.div_ceil(100))
}

/// What the file's facts say, beyond the header.
struct Facts {
    width: usize,
    height: usize,
    images: usize,
    /// Tiles across one band and bands, for a mosaic.
    mosaic: Option<(usize, usize)>,
}

fn facts(c: &Cfb) -> Result<Facts> {
    let dim = |p: &str| {
        i32_of(c, p)
            .filter(|&x| x > 0)
            .map(|x| x as usize)
            .ok_or_else(|| Error::Malformed(format!("Xradia {p}")))
    };
    let width = dim("ImageInfo/ImageWidth")?;
    let height = dim("ImageInfo/ImageHeight")?;
    let taken = i32_of(c, "ImageInfo/ImagesTaken")
        .filter(|&x| x > 0)
        .or_else(|| i32_of(c, "ImageInfo/NoOfImages").filter(|&x| x > 0))
        .unwrap_or(1) as usize;
    // Only the images that are there (an aborted scan stops early).
    let mut images = 0;
    while images < taken && c.exists(&image_path(images + 1)) {
        images += 1;
    }
    if images == 0 {
        return Err(Error::Malformed("Xradia file without images".into()));
    }
    let r = i32_of(c, "ImageInfo/MosiacRows").unwrap_or(0).max(0) as usize;
    let k = i32_of(c, "ImageInfo/MosiacColumns").unwrap_or(0).max(0) as usize;
    let mosaic = (r.max(1) * k.max(1) > 1).then_some((r.max(1), k.max(1)));
    Ok(Facts {
        width,
        height,
        images,
        mosaic,
    })
}

pub fn parse(data: &[u8], path: &Path) -> Result<Header> {
    let c = Cfb::open(data).map_err(cfb_err)?;
    let f = facts(&c)?;
    let pixels = f.width.checked_mul(f.height).ok_or(Error::TooLarge)?;
    let dtype = match i32_of(&c, "ImageInfo/DataType").and_then(dtype_of) {
        Some(t) => t,
        None => {
            let n = c.entry(&image_path(1)).map_or(0, |e| e.size) as usize;
            if n == pixels * 2 {
                DataType::U16
            } else if n == pixels * 4 {
                DataType::F32
            } else {
                return Err(Error::Unsupported(
                    "Xradia images of an unknown type".into(),
                ));
            }
        }
    };
    let (w, h, n) = match f.mosaic {
        Some((across, bands)) => (f.width * across, f.height * bands, 1),
        None => (f.width, f.height, f.images),
    };
    let um = f32s(&c, "ImageInfo/PixelSize")
        .first()
        .copied()
        .filter(|x| x.is_finite() && *x > 0.0)
        .map_or(1000.0, f64::from);
    let px = um / 1000.0;
    let layout = Layout::dense([w, h, n], &[], 1).ok_or(Error::TooLarge)?;
    let affine = [
        [px, 0.0, 0.0, 0.0],
        [0.0, -px, 0.0, (h - 1) as f64 * px],
        [0.0, 0.0, px, 0.0],
    ];
    Ok(Header {
        kind: Kind::Txm,
        dtype,
        layout,
        affine,
        orientation: OrientationSource::Object,
        dims: vec![w, h, n],
        spacing: [px, px, px],
        scale: None,
        cal: None,
        description: None,
        intent: None,
        anatomical: None,
        params: params(&c, path, f.images),
        data: DataRef {
            file: DataFile::Attached,
            skip: Skip::Start { lines: 0, bytes: 0 },
            encoding: Encoding::Container,
            big_endian: false,
        },
    })
}

fn params(c: &Cfb, path: &Path, images: usize) -> Vec<Param> {
    let mut out = Vec::new();
    let angles = f32s(c, "ImageInfo/Angles");
    let angles = &angles[..angles.len().min(images)];
    let (lo, hi) = angles
        .iter()
        .filter(|a| a.is_finite())
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(l, h), &a| {
            (l.min(a), h.max(a))
        });
    let turning = hi - lo > 1e-3;
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    let content = match ext.as_str() {
        "txrm" => Some("projections"),
        "txm" => Some("reconstruction"),
        _ if images > 1 && turning => Some("projections"),
        _ if images > 1 => Some("reconstruction"),
        _ => None,
    };
    if let Some(c) = content {
        out.push(Param {
            key: "content",
            value: ParamValue::Text(c.into()),
        });
    }
    if content == Some("projections") {
        out.push(Param {
            key: "projections",
            value: ParamValue::Number(images as f64, ""),
        });
        if turning {
            out.push(Param {
                key: "angles",
                value: ParamValue::Text(format!("{}° – {}°", round(lo), round(hi))),
            });
        }
    }
    let first = |p: &str| {
        f32s(c, p)
            .first()
            .copied()
            .filter(|x| x.is_finite() && *x != 0.0)
            .map(f64::from)
    };
    let mut num = |key: &'static str, stream: &str, unit: &'static str, k: f64| {
        if let Some(x) = first(stream) {
            out.push(Param {
                key,
                value: ParamValue::Number(x * k, unit),
            });
        }
    };
    num("kv", "ImageInfo/Voltage", "kV", 1.0);
    num("current", "ImageInfo/Current", "µA", 1.0);
    num("power", "ImageInfo/Power", "W", 1.0);
    // Exposure in seconds, shown in milliseconds.
    if first("ImageInfo/ExpTimes").is_some() {
        num("exposure", "ImageInfo/ExpTimes", "ms", 1000.0);
    } else {
        num("exposure", "ImageInfo/ExpTime", "ms", 1000.0);
    }
    // Distances to the rotation axis; the source's is stored negative.
    num("sod", "ImageInfo/StoRADistance", "mm", 1.0);
    num("odd", "ImageInfo/DtoRADistance", "mm", 1.0);
    num("magnification", "ImageInfo/OpticalMagnification", "×", 1.0);
    let mut text = |key: &'static str, stream: &str| {
        if let Some(t) = text_of(c, stream) {
            out.push(Param {
                key,
                value: ParamValue::Text(t),
            });
        }
    };
    text("objective", "ImageInfo/ObjectiveName");
    text("filter", "AcquisitionSettings/SourceFilterName");
    text("filter", "ImageInfo/SourceFilterName");
    text("camera", "ImageInfo/CameraName");
    text("date", "ImageInfo/Date");
    if let Some(b) = i32_of(c, "ImageInfo/CameraBinning").filter(|&b| b > 1) {
        out.push(Param {
            key: "binning",
            value: ParamValue::Number(b as f64, ""),
        });
    }
    for p in &mut out {
        if let ParamValue::Number(x, "mm") = &mut p.value {
            *x = x.abs();
        }
    }
    out.retain(|p| !matches!(p.value, ParamValue::Number(x, _) if x <= 0.0));
    let mut seen = Vec::new();
    out.retain(|p| {
        let new = !seen.contains(&p.key);
        seen.push(p.key);
        new
    });
    out
}

fn round(x: f32) -> f64 {
    (f64::from(x) * 100.0).round() / 100.0
}

/// The images gathered into one buffer (a mosaic stitched).
pub fn load(header: Header, data: &[u8]) -> Result<MedVolume> {
    let c = Cfb::open(data).map_err(cfb_err)?;
    let f = facts(&c)?;
    let size = header.dtype.size();
    let plane = f
        .width
        .checked_mul(f.height)
        .and_then(|p| p.checked_mul(size))
        .ok_or(Error::TooLarge)?;
    let need = header.data_bytes().ok_or(Error::TooLarge)?;
    // The images are stored raw in the file: a volume (or stitched mosaic, zero-filled
    // where tiles are missing) larger than the file is not there.
    if need > data.len() {
        return Err(Error::Truncated("Xradia images".into()));
    }
    let (mut out, res) = crate::data::alloc(need)?;
    let tiles = header.layout.size[2].max(1) * f.mosaic.map_or(1, |(a, b)| a * b);
    let mut images = Vec::with_capacity(tiles.min(f.images));
    for n in 1..=tiles.min(f.images) {
        let b = c.read(&image_path(n)).map_err(cfb_err)?;
        if b.len() < plane {
            return Err(Error::Truncated(format!("Xradia image {n}")));
        }
        images.push(b);
    }
    match f.mosaic {
        None => {
            for b in &images {
                out.extend_from_slice(&b[..plane]);
            }
        }
        Some((across, bands)) => {
            let row = f.width * size;
            out.resize(need, 0);
            let stride = row * across;
            for (t, b) in images.iter().enumerate().take(across * bands) {
                let (band, at) = (t / across, t % across);
                for y in 0..f.height {
                    let dst = (band * f.height + y) * stride + at * row;
                    out[dst..dst + row].copy_from_slice(&b[y * row..(y + 1) * row]);
                }
            }
        }
    }
    if out.len() < need {
        return Err(Error::Truncated("Xradia images".into()));
    }
    Ok(MedVolume {
        header,
        bytes: crate::data::held(out, res),
        offset: 0,
        big_endian: false,
    })
}

#[cfg(test)]
mod hostile {
    use super::*;

    #[test]
    fn volume_larger_than_the_file_is_refused() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/txm_u16.txm");
        let data = std::fs::read(&path).unwrap();
        let mut h = parse(&data, &path).unwrap();
        assert!(load(h.clone(), &data).is_ok());
        // A header claiming 64 Ki × 64 Ki × 64 voxels: refused before reserving 512 GiB.
        h.layout = Layout::dense([65536, 65536, 64], &[], 1).unwrap();
        assert_eq!(
            load(h, &data).unwrap_err(),
            Error::Truncated("Xradia images".into())
        );
    }
}
