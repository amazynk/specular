//! JPEG-LS (`.jls`, ITU-T T.87) through `specular-jpegls`.
//!
//! The image is decoded whole: grey as 8-bit `Gray` up to 8 bits per sample, else `Gray16`;
//! two components as grey + alpha, three as RGB, four as RGBA (8-bit `Rgba` or 16-bit
//! `Rgba16`). Samples are scaled from 0..=MAXVAL to the full output range, and subsampled
//! components are upsampled by repetition. Streams that index a mapping table are refused.

use std::sync::Arc;

use specular_core::{CancelToken, HdrEncoding, Orientation, PixelLayout, Raster, Source};
use specular_jpegls as jls;

use crate::whole::{self, Levels};
use crate::{DecodeError, DecodeRequest, Decoder, Format, ImageInfo, Result, Session};

pub struct JpegLsDecoder;

fn map_err(e: jls::Error) -> DecodeError {
    match e {
        jls::Error::NotJpegLs => DecodeError::UnsupportedFormat,
        jls::Error::Truncated => DecodeError::Truncated("JPEG-LS scan".into()),
        jls::Error::Malformed(s) => DecodeError::Malformed(s.into()),
        jls::Error::Unsupported(s) => DecodeError::Unsupported(format!("JPEG-LS {s}")),
        jls::Error::TooLarge(_) => DecodeError::TooLarge,
        jls::Error::Cancelled => DecodeError::Cancelled,
        jls::Error::NeedsMemory(n) => DecodeError::NeedsMemory(n),
    }
}

impl Decoder for JpegLsDecoder {
    fn format(&self) -> Format {
        Format::JpegLs
    }
    fn probe(&self, head: &[u8]) -> bool {
        jls::probe(head)
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["jls"]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let i = jls::read_info(source.bytes()).map_err(map_err)?;
        if i.width == 0 || i.height == 0 {
            return Err(DecodeError::Malformed("empty image".into()));
        }
        if i.mapped {
            return Err(DecodeError::Unsupported("JPEG-LS mapping tables".into()));
        }
        let n = i.components.len();
        let wide = i.precision > 8;
        let model = match n {
            1 => "Greyscale",
            2 => "Greyscale + alpha",
            3 => "RGB",
            _ => "RGBA",
        };
        let mut coding = if i.near == 0 {
            format!("lossless, {}-bit", i.precision)
        } else {
            format!("near-lossless (NEAR {}), {}-bit", i.near, i.precision)
        };
        if n > 1 {
            coding += [", planar", ", line-interleaved", ", sample-interleaved"]
                [i.interleave.min(2) as usize];
        }
        if i.colour_transform != jls::ColourTransform::None {
            coding += &format!(", HP colour transform {:?}", i.colour_transform);
        }
        let info = ImageInfo {
            format: Format::JpegLs,
            width: i.width,
            height: i.height,
            orientation: Orientation::NORMAL,
            icc_profile: None,
            has_alpha: n == 2 || n >= 4,
            bit_depth: i.precision,
            progressive: false,
            coding,
            embedded_preview: None,
            fast_regions: false,
            scales: &[1, 2, 4, 8],
        };
        let bpp = match (n, wide) {
            (1, false) => 1,
            (1, true) => 2,
            (_, false) => 4,
            (_, true) => 8,
        };
        Ok(Arc::new(JpegLsSession {
            source,
            info,
            model,
            bpp,
            maxval: i.maxval,
            levels: Levels::new(),
        }))
    }
}

struct JpegLsSession {
    source: Arc<Source>,
    info: ImageInfo,
    model: &'static str,
    /// Bytes per output pixel.
    bpp: u64,
    /// Largest sample value (MAXVAL), shown as full intensity.
    maxval: u16,
    levels: Levels,
}

/// The decoded planes as one interleaved raster.
fn to_raster(img: &jls::Image, maxval: u16) -> Result<Raster> {
    let (w, h) = (img.width as usize, img.height as usize);
    let maxval = maxval.max(1) as u32;
    let wide = img.precision > 8;
    let out_max = if wide { 65535 } else { 255 };
    let scale = |v: u16| -> u32 { (v as u32 * out_max + maxval / 2) / maxval };
    // Nearest-sample lookup that repeats subsampled components.
    let at = |c: usize, x: usize, y: usize| -> u16 {
        let p = &img.planes[c];
        let sx = x * p.width as usize / w;
        let sy = y * p.height as usize / h;
        p.data.get(sy * p.width as usize + sx).copied().unwrap_or(0)
    };
    let n = img.planes.len();
    if n == 0 {
        return Err(DecodeError::Malformed("no components".into()));
    }
    let comps: [usize; 4] = match n {
        1 => [0; 4],
        2 => [0, 0, 0, 1],
        3 => [0, 1, 2, usize::MAX],
        _ => [0, 1, 2, 3],
    };
    let channels = if n == 1 { 1 } else { 4 };
    let mut out = Vec::new();
    out.try_reserve_exact(w * h * channels * if wide { 2 } else { 1 })
        .map_err(|_| DecodeError::TooLarge)?;
    for y in 0..h {
        for x in 0..w {
            for &c in &comps[..channels] {
                let v = if c == usize::MAX {
                    out_max
                } else {
                    scale(at(c, x, y).min(maxval as u16))
                };
                if wide {
                    out.extend_from_slice(&(v as u16).to_ne_bytes());
                } else {
                    out.push(v as u8);
                }
            }
        }
    }
    let layout = match (channels, wide) {
        (1, false) => PixelLayout::Gray,
        (1, true) => PixelLayout::Gray16,
        (_, false) => PixelLayout::Rgba,
        (_, true) => PixelLayout::Rgba16(HdrEncoding::SRGB),
    };
    Ok(whole::interleaved(
        img.width,
        img.height,
        layout,
        out,
        img.truncated,
    ))
}

impl JpegLsSession {
    fn decode_full(&self, cancel: &CancelToken) -> Result<Raster> {
        let opts = jls::DecodeOptions {
            cancel: Some(cancel.flag()),
            ..Default::default()
        };
        let img = jls::decode(self.source.bytes(), &opts).map_err(map_err)?;
        to_raster(&img, self.maxval)
    }
}

impl Session for JpegLsSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn colour_model(&self) -> Option<&'static str> {
        Some(self.model)
    }
    fn source(&self) -> &Arc<Source> {
        &self.source
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        None
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        self.levels.decode(
            req,
            (self.info.width, self.info.height),
            |_| 1,
            |_| self.decode_full(cancel),
            cancel,
        )
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        let (w, h) = (self.info.width, self.info.height);
        let s = req.scale.max(1);
        let mut m = whole::level_bytes(w, h, s, self.bpp);
        if !self.levels.has(s) && !self.levels.has(1) {
            // The decoder's u16 planes, then the full raster.
            let comps = if self.bpp <= 2 { 1 } else { 4 };
            m += whole::level_bytes(w, h, 1, 2 * comps + self.bpp);
        }
        m
    }
    fn memory_bytes(&self) -> usize {
        self.levels.memory_bytes()
    }
    /// Drops the cached levels.
    fn trim_memory(&self) -> usize {
        self.levels.clear()
    }
}
