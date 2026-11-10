//! Stacks of single-slice image files (a reconstruction written as BMP, PNG or TIFF slices),
//! decoded with the tree's own decoders, in parallel, into one sample buffer.

use std::path::{Path, PathBuf};

use rayon::prelude::*;

use crate::{DataType, Error, Result};

/// One decoded slice.
pub(crate) struct Slice {
    pub width: usize,
    pub height: usize,
    pub dtype: DataType,
    /// Little-endian samples, rows top-down.
    pub bytes: Vec<u8>,
}

fn named(p: &Path, e: impl std::fmt::Display) -> Error {
    Error::Malformed(format!(
        "{}: {e}",
        p.file_name().unwrap_or(p.as_os_str()).to_string_lossy()
    ))
}

/// Decodes a grey BMP, PNG or TIFF slice (a colour one by its first channel).
pub(crate) fn decode(data: &[u8], path: &Path) -> Result<Slice> {
    if data.starts_with(b"BM") {
        use specular_bitmap::Decode;
        let b = specular_bitmap::bmp::Bmp::parse(data).map_err(|e| named(path, e))?;
        let opts = specular_bitmap::Options {
            parallel: false,
            ..Default::default()
        };
        let im = b
            .decode(data, &opts, &|| false)
            .map_err(|e| named(path, e))?;
        let n = im.width as usize * im.height as usize;
        let ch = im.format.bytes_per_pixel();
        let bytes = match im.format {
            specular_bitmap::PixelFormat::Gray8 => im.data,
            specular_bitmap::PixelFormat::Rgba8 => im.data.iter().step_by(ch).copied().collect(),
            f => return Err(named(path, format!("BMP pixels {f:?}"))),
        };
        if bytes.len() < n {
            return Err(Error::Truncated(path.display().to_string()));
        }
        return Ok(Slice {
            width: im.width as usize,
            height: im.height as usize,
            dtype: DataType::U8,
            bytes,
        });
    }
    if data.starts_with(&[0x89, b'P', b'N', b'G']) {
        // 16-bit slices keep their samples; 8-bit ones (palette slices too) stay 8-bit.
        let info = specular_png::read_info(data).map_err(|e| named(path, e))?;
        let opts = specular_png::DecodeOptions {
            output: if info.bit_depth > 8 {
                specular_png::Output::Auto16
            } else {
                specular_png::Output::Auto8
            },
            parallelism: specular_png::Parallelism::Single,
            ..Default::default()
        };
        let im = specular_png::decode(data, &opts).map_err(|e| named(path, e))?;
        let (w, h) = (im.width as usize, im.height as usize);
        let p = &im.planes[0];
        let (dtype, size) = match im.layout {
            specular_png::Layout::Gray8 | specular_png::Layout::Rgb8Planar => (DataType::U8, 1),
            specular_png::Layout::Rgba8 => (DataType::U8, 4),
            specular_png::Layout::Gray16 => (DataType::U16, 2),
            specular_png::Layout::GrayAlpha16 => (DataType::U16, 4),
            specular_png::Layout::Rgba16 => (DataType::U16, 8),
        };
        let mut bytes = Vec::with_capacity(w * h * dtype.size());
        for y in 0..h {
            let row = &p.data[y * p.stride..];
            for x in 0..w {
                let s = &row[x * size..];
                match dtype {
                    DataType::U8 => bytes.push(s[0]),
                    _ => bytes.extend(u16::from_ne_bytes([s[0], s[1]]).to_le_bytes()),
                }
            }
        }
        return Ok(Slice {
            width: w,
            height: h,
            dtype,
            bytes,
        });
    }
    if data.starts_with(b"II") || data.starts_with(b"MM") {
        let t = specular_tiff::Tiff::open(data).map_err(|e| named(path, e))?;
        let ifd = t
            .pages()
            .first()
            .ok_or_else(|| named(path, "no image"))?
            .ifd();
        let dtype = match (ifd.sample_format, ifd.bits) {
            (specular_tiff::SampleFormat::Uint, 1..=8) => DataType::U8,
            (specular_tiff::SampleFormat::Uint, 9..=16) => DataType::U16,
            (specular_tiff::SampleFormat::Int, 16) => DataType::I16,
            (specular_tiff::SampleFormat::Int, 8) => DataType::I8,
            _ => DataType::F32,
        };
        let opts = specular_tiff::DecodeOptions {
            parallel: false,
            ..Default::default()
        };
        let d = t
            .decode_values(data, 0, 0, None, &opts)
            .map_err(|e| named(path, e))?;
        let (w, h) = (d.width as usize, d.height as usize);
        let mut bytes = Vec::with_capacity(w * h * dtype.size());
        for y in 0..h {
            let row = &d.data[y * d.stride..];
            for x in 0..w {
                let b = &row[x * 4..x * 4 + 4];
                let v = f32::from_ne_bytes([b[0], b[1], b[2], b[3]]);
                match dtype {
                    DataType::U8 => bytes.push(v as u8),
                    DataType::I8 => bytes.push(v as i8 as u8),
                    DataType::U16 => bytes.extend((v as u16).to_le_bytes()),
                    DataType::I16 => bytes.extend((v as i16).to_le_bytes()),
                    _ => bytes.extend(v.to_le_bytes()),
                }
            }
        }
        return Ok(Slice {
            width: w,
            height: h,
            dtype,
            bytes,
        });
    }
    Err(Error::Unsupported(format!(
        "{}: slices must be BMP, PNG or TIFF",
        path.file_name()
            .unwrap_or(path.as_os_str())
            .to_string_lossy()
    )))
}

fn read(p: &Path) -> Result<Vec<u8>> {
    crate::read_named(p)
}

/// The first slice of a stack (for its size and sample type).
pub(crate) fn first(paths: &[PathBuf]) -> Result<Slice> {
    let p = paths
        .first()
        .ok_or_else(|| Error::Malformed("no slices".into()))?;
    decode(&read(p)?, p)
}

/// Every slice decoded in parallel, each `w × h` of `dtype`, one after another, with the
/// memory budget's reservation for the stack (the workers decode under the same budget).
pub(crate) fn stack(
    paths: &[PathBuf],
    w: usize,
    h: usize,
    dtype: DataType,
) -> Result<(Vec<u8>, specular_core::Reservation)> {
    let plane = w * h * dtype.size();
    let need = plane.checked_mul(paths.len()).ok_or(Error::TooLarge)?;
    if need > crate::data::MAX_DATA {
        return Err(Error::TooLarge);
    }
    let budget = specular_core::current_budget();
    let (mut out, res) = crate::data::alloc(need)?;
    out.resize(need, 0);
    out.par_chunks_mut(plane)
        .zip(paths.par_iter())
        .try_for_each(|(dst, p)| -> Result<()> {
            let s = budget.run(|| decode(&read(p)?, p))?;
            if (s.width, s.height, s.dtype) != (w, h, dtype) {
                return Err(named(
                    p,
                    format!(
                        "{}×{} {} where the first slice is {w}×{h} {}",
                        s.width,
                        s.height,
                        s.dtype.name(),
                        dtype.name()
                    ),
                ));
            }
            dst.copy_from_slice(&s.bytes[..plane]);
            Ok(())
        })?;
    Ok((out, res))
}
