//! FLIF through the in-tree `specular-flif` decoder.
//!
//! The image (an animation's first frame) is decoded once into 8-bit grey or RGBA, or 16-bit
//! for files above 8 bits; coarser levels are box-filtered from it. An interlaced file is
//! progressive: a thumbnail-sized request stops after the zoom level that gives it, so the
//! gallery reads only the start of the file. Animations decode all frames in one pass (FLIF
//! interleaves them) and play from memory. ICC, EXIF and XMP chunks go to the colour pipeline
//! and the info panel.

use crate::whole::{self, Levels};
use crate::{
    Alpha, AnimationInfo, DecodeError, DecodeRequest, Decoder, Format, Frame, Frames, ImageInfo,
    Result, Session,
};
use specular_core::{CancelToken, HdrEncoding, Orientation, PixelLayout, Raster, Source};
use specular_flif as sf;
use std::sync::Arc;
use std::time::Duration;

pub struct FlifDecoder;

fn map_err(e: sf::Error) -> DecodeError {
    match e {
        sf::Error::NotFlif => DecodeError::Malformed("not a FLIF file".into()),
        sf::Error::Unsupported(s) => DecodeError::Unsupported(s),
        sf::Error::Truncated(s) => DecodeError::Truncated(s),
        sf::Error::Malformed(s) => DecodeError::Malformed(s),
        sf::Error::TooLarge => DecodeError::TooLarge,
        sf::Error::NeedsMemory(n) => DecodeError::NeedsMemory(n),
        sf::Error::Cancelled => DecodeError::Cancelled,
    }
}

fn layout(f: sf::PixelFormat) -> PixelLayout {
    match f {
        sf::PixelFormat::Gray8 => PixelLayout::Gray,
        sf::PixelFormat::Rgba8 => PixelLayout::Rgba,
        sf::PixelFormat::Gray16 => PixelLayout::Gray16,
        sf::PixelFormat::Rgba16 => PixelLayout::Rgba16(HdrEncoding::SRGB),
    }
}

fn coding(i: &sf::Info) -> String {
    let model = match i.channels {
        1 => "grey",
        3 => "RGB",
        _ => "RGBA",
    };
    let mut s = format!(
        "FLIF {}-bit {model}, {}",
        i.significant_bits,
        if i.interlaced {
            "interlaced"
        } else {
            "non-interlaced"
        }
    );
    if i.frames > 1 {
        s += &format!(", {} frames", i.frames);
    }
    s
}

impl Decoder for FlifDecoder {
    fn format(&self) -> Format {
        Format::Flif
    }
    fn probe(&self, head: &[u8]) -> bool {
        sf::probe(head)
    }
    /// FLIF archives (`ar` with an `__image.flif` member) too.
    fn probe_file(&self, data: &[u8]) -> bool {
        sf::probe_file(data)
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["flif"]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let i = sf::read_info(source.bytes()).map_err(map_err)?;
        let animation = (i.frames > 1).then_some(AnimationInfo {
            frame_count: Some(i.frames),
            loop_count: i.loop_count,
        });
        let info = ImageInfo {
            format: Format::Flif,
            width: i.width,
            height: i.height,
            orientation: Orientation::NORMAL,
            icc_profile: i.icc.clone().map(Arc::new),
            has_alpha: i.channels == 4,
            bit_depth: i.significant_bits,
            progressive: i.interlaced,
            coding: coding(&i),
            embedded_preview: None,
            fast_regions: false,
            scales: &[1, 2, 4, 8],
        };
        Ok(Arc::new(FlifSession {
            source,
            info,
            flif: i,
            animation,
            levels: Levels::new(),
        }))
    }
}

struct FlifSession {
    source: Arc<Source>,
    info: ImageInfo,
    flif: sf::Info,
    animation: Option<AnimationInfo>,
    levels: Levels,
}

impl FlifSession {
    /// Decodes level 1/`scale` (an interlaced file stops at its zoom level).
    fn decode_level(&self, scale: u32, cancel: &CancelToken) -> Result<Raster> {
        let stop = || cancel.is_cancelled();
        let img = sf::decode(
            self.source.bytes(),
            &sf::Options {
                cancel: Some(&stop),
                scale,
                first_frame_only: true,
            },
        )
        .map_err(map_err)?;
        let f = img
            .frames
            .into_iter()
            .next()
            .ok_or_else(|| DecodeError::Malformed("FLIF without a frame".into()))?;
        let mut r = whole::interleaved(
            img.width,
            img.height,
            layout(img.format),
            f.data,
            img.truncated,
        );
        let (w, h) = (self.info.width, self.info.height);
        if (r.width, r.height) == (w.div_ceil(scale), h.div_ceil(scale)) {
            r.scale = scale;
            return Ok(r);
        }
        // A tiny image can stop at a finer zoom level than asked: box-filter the full image.
        if scale > 1 {
            let full = self.decode_level(1, cancel)?;
            return Ok(whole::downscale(&full, w, h, scale));
        }
        r.scale = 1;
        Ok(r)
    }
}

impl Session for FlifSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn source(&self) -> &Arc<Source> {
        &self.source
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        None
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        let interlaced = self.flif.interlaced;
        self.levels.decode(
            req,
            (self.info.width, self.info.height),
            // Interlaced files decode any level directly; others from the full image.
            |s| if interlaced { s } else { 1 },
            |s| self.decode_level(s, cancel),
            cancel,
        )
    }
    fn scaled_decode_cheap(&self) -> bool {
        self.flif.interlaced
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        let (w, h) = (self.info.width, self.info.height);
        let bpp = if self.flif.bit_depth > 8 { 8 } else { 4 };
        let s = req.scale.max(1);
        let base = if self.flif.interlaced { s } else { 1 };
        let mut m = whole::level_bytes(w, h, s, bpp);
        if !self.levels.has(s) && !self.levels.has(base) {
            // The decoder's planes (32-bit samples per channel) at the decoded level.
            m += whole::level_bytes(w, h, base, bpp + 4 * u64::from(self.flif.channels));
        }
        m
    }
    fn memory_bytes(&self) -> usize {
        self.levels.memory_bytes()
    }
    fn trim_memory(&self) -> usize {
        self.levels.clear()
    }
    fn animation(&self) -> Option<&AnimationInfo> {
        self.animation.as_ref()
    }
    fn frames(&self, scale: u32) -> Result<Box<dyn Frames>> {
        Ok(Box::new(FlifFrames {
            source: self.source.clone(),
            scale: scale.max(1),
            size: (self.info.width, self.info.height),
            frames: None,
            next: 0,
        }))
    }
    fn colour_model(&self) -> Option<&'static str> {
        Some(if self.flif.channels == 1 {
            "Greyscale"
        } else {
            "RGB"
        })
    }
    fn alpha(&self) -> Option<Alpha> {
        (self.flif.channels == 4).then_some(Alpha::Straight)
    }
    fn embedded_icc(&self) -> Option<&[u8]> {
        self.flif.icc.as_deref()
    }
    fn embedded_xmp(&self) -> Option<Vec<u8>> {
        self.flif.xmp.clone()
    }
    fn embedded_exif(&self) -> Option<Vec<u8>> {
        crate::exif_tiff_of(self.flif.exif.as_deref()?)
    }
}

/// All frames decoded on the first request (FLIF interleaves them), then handed out.
struct FlifFrames {
    source: Arc<Source>,
    scale: u32,
    size: (u32, u32),
    frames: Option<Vec<(Raster, Duration)>>,
    next: usize,
}

impl Frames for FlifFrames {
    fn next_frame(&mut self, cancel: &CancelToken) -> Option<Result<Frame>> {
        if self.frames.is_none() {
            let stop = || cancel.is_cancelled();
            let img = match sf::decode(
                self.source.bytes(),
                &sf::Options {
                    cancel: Some(&stop),
                    scale: 1,
                    first_frame_only: false,
                },
            ) {
                Ok(i) => i,
                Err(e) => return Some(Err(map_err(e))),
            };
            let (w, h) = self.size;
            let l = layout(img.format);
            let frames = img
                .frames
                .into_iter()
                .map(|f| {
                    let r = whole::interleaved(img.width, img.height, l, f.data, img.truncated);
                    let r = if self.scale > 1 {
                        whole::downscale(&r, w, h, self.scale)
                    } else {
                        r
                    };
                    (r, Duration::from_millis(u64::from(f.delay_ms)))
                })
                .collect();
            self.frames = Some(frames);
        }
        let frames = self.frames.as_ref()?;
        let (raster, delay) = frames.get(self.next)?.clone();
        let index = self.next as u32;
        self.next += 1;
        Some(Ok(Frame {
            index,
            raster,
            delay,
        }))
    }
    fn rewind(&mut self) -> Result<()> {
        self.next = 0;
        Ok(())
    }
}
