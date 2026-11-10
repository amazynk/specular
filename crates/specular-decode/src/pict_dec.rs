//! Apple QuickDraw PICT through the in-tree `specular-pict` decoder.
//!
//! A picture that is a single QuickTime JPEG covering the canvas (what "Save as PICT" with
//! JPEG compression wrote) opens zero-copy through the JPEG decoder, with its scaled
//! decodes. Anything else (bitmaps, QuickDraw drawing and text, other QuickTime images) is
//! drawn whole to RGBA once; coarser levels are box-filtered from it. A picture that draws
//! nothing, or only in a QuickTime codec without a decoder, is refused at open with the
//! reason.

use crate::whole::{self, Levels};
use crate::{DecodeError, DecodeRequest, Decoder, Format, ImageInfo, Result, Session};
use specular_core::{CancelToken, Orientation, PixelLayout, Raster, Source};
use specular_pict as pict;
use std::sync::Arc;

pub struct PictDecoder;

fn map_err(e: pict::Error) -> DecodeError {
    match e {
        pict::Error::NotPict => DecodeError::UnsupportedFormat,
        pict::Error::Unsupported(s) => DecodeError::Unsupported(s),
        pict::Error::Truncated(s) => DecodeError::Truncated(s.into()),
        pict::Error::Malformed(s) => DecodeError::Malformed(s.into()),
        pict::Error::TooLarge => DecodeError::TooLarge,
    }
}

impl Decoder for PictDecoder {
    fn format(&self) -> Format {
        Format::Pict
    }
    /// Version 2 pictures without the 512-byte header; with it (and version 1 pictures) the
    /// signature is beyond the probe window or too weak, see `probe_file` and the extension.
    fn probe(&self, head: &[u8]) -> bool {
        pict::probe(head)
    }
    fn probe_file(&self, data: &[u8]) -> bool {
        pict::probe_file(data)
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["pict", "pct"]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let data = source.bytes();
        // Chosen by extension: only a plausible header goes on.
        if !pict::plausible(data) {
            return Err(DecodeError::UnsupportedFormat);
        }
        let i = pict::info(data).map_err(map_err)?;
        let scan = pict::scan(data).map_err(map_err)?;
        let version = format!("PICT version {}", i.version);
        if let Some(range) = scan.sole_jpeg {
            let sub = Source::from_slice(source.path(), source.clone(), &data[range])
                .map(Arc::new)
                .ok_or(DecodeError::Malformed("PICT JPEG outside the file".into()))?;
            let inner = crate::jpeg::JpegDecoder.open(sub)?;
            let mut info = inner.info().clone();
            info.format = Format::Pict;
            info.coding = format!("{version}, QuickTime JPEG ({})", info.coding);
            return Ok(Arc::new(PictSession {
                source,
                info,
                inner: Some(inner),
                levels: Levels::new(),
            }));
        }
        let mut what = Vec::new();
        if scan.bitmaps > 0 {
            what.push(match scan.bitmaps {
                1 => "1 bitmap".to_string(),
                n => format!("{n} bitmaps"),
            });
        }
        if scan.jpegs > 0 {
            what.push(match scan.jpegs {
                1 => "1 QuickTime JPEG".to_string(),
                n => format!("{n} QuickTime JPEGs"),
            });
        }
        if scan.quicktime > 0 {
            what.push(match scan.quicktime {
                1 => "1 QuickTime image".to_string(),
                n => format!("{n} QuickTime images"),
            });
        }
        if scan.shapes > 0 {
            what.push(match scan.shapes {
                1 => "1 drawing operation".to_string(),
                n => format!("{n} drawing operations"),
            });
        }
        let info = ImageInfo {
            format: Format::Pict,
            width: i.width,
            height: i.height,
            orientation: Orientation::NORMAL,
            icc_profile: None,
            has_alpha: scan.alpha,
            bit_depth: 8,
            progressive: false,
            coding: format!("{version}, {}", what.join(" + ")),
            embedded_preview: None,
            fast_regions: false,
            scales: &[1, 2, 4, 8],
        };
        Ok(Arc::new(PictSession {
            source,
            info,
            inner: None,
            levels: Levels::new(),
        }))
    }
}

struct PictSession {
    source: Arc<Source>,
    info: ImageInfo,
    /// The sole QuickTime JPEG's own session.
    inner: Option<Arc<dyn Session>>,
    levels: Levels,
}

impl Session for PictSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn source(&self) -> &Arc<Source> {
        &self.source
    }
    fn colour_model(&self) -> Option<&'static str> {
        match &self.inner {
            Some(s) => s.colour_model(),
            None => Some("RGB"),
        }
    }
    fn chroma_subsampling(&self) -> Option<&'static str> {
        self.inner.as_ref().and_then(|s| s.chroma_subsampling())
    }
    fn embedded_icc(&self) -> Option<&[u8]> {
        self.inner.as_ref().and_then(|s| s.embedded_icc())
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        self.inner.as_ref().and_then(|s| s.decode_preview())
    }
    fn prepare_regions(&self, cancel: &CancelToken) -> Result<()> {
        match &self.inner {
            Some(s) => s.prepare_regions(cancel),
            None => Ok(()),
        }
    }
    fn regions_ready(&self) -> bool {
        self.inner.as_ref().is_none_or(|s| s.regions_ready())
    }
    fn open_memory(&self, levels: &[u32]) -> u64 {
        if let Some(s) = &self.inner {
            return s.open_memory(levels);
        }
        levels
            .iter()
            .map(|&scale| {
                self.decode_memory(&DecodeRequest {
                    scale,
                    region: None,
                    parallel: true,
                    prepare_regions: true,
                })
            })
            .sum()
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        if let Some(s) = &self.inner {
            return s.decode(req, cancel);
        }
        self.levels.decode(
            req,
            (self.info.width, self.info.height),
            |_| 1,
            |_| {
                let img = pict::decode(self.source.bytes()).map_err(map_err)?;
                Ok(whole::interleaved(
                    img.width,
                    img.height,
                    PixelLayout::Rgba,
                    img.pixels,
                    img.truncated,
                ))
            },
            cancel,
        )
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        if let Some(s) = &self.inner {
            return s.decode_memory(req);
        }
        let (w, h) = (self.info.width, self.info.height);
        let s = req.scale.max(1);
        let mut m = whole::level_bytes(w, h, s, 4);
        if !self.levels.has(s) && !self.levels.has(1) {
            m += whole::level_bytes(w, h, 1, 4);
        }
        m
    }
    fn memory_bytes(&self) -> usize {
        self.levels.memory_bytes() + self.inner.as_ref().map_or(0, |s| s.memory_bytes())
    }
    /// Drops the cached levels (the JPEG session's own).
    fn trim_memory(&self) -> usize {
        self.levels.clear() + self.inner.as_ref().map_or(0, |s| s.trim_memory())
    }
    fn scaled_decode_cheap(&self) -> bool {
        self.inner.as_ref().is_some_and(|s| s.scaled_decode_cheap())
    }
    fn want_levels(&self, scales: &[u32]) {
        if let Some(s) = &self.inner {
            s.want_levels(scales);
        }
    }
}
