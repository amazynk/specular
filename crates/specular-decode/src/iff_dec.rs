//! Amiga IFF pictures (ILBM, PBM, ACBM, RGB8, RGBN, DEEP) and animations (ANIM) through the
//! in-tree `specular-iff` decoder.
//!
//! Decoded whole to RGBA once; coarser levels are box-filtered from it and regions are
//! cropped. The pixel aspect is named in the coding line, not applied. Colour cycling
//! (`CRNG`, `CCRT`, `DRNG`) plays as an animation: each frame is the image drawn with that
//! moment's palette; the still is the file's own palette. An ANIM plays its frames (deltas
//! on the frames before, in order); the still is its first frame.

use crate::whole::{self, Levels};
use crate::{
    Alpha, AnimationInfo, DecodeError, DecodeRequest, Decoder, Format, Frame, Frames, ImageInfo,
    Result, Session,
};
use specular_core::{CancelToken, Orientation, PixelLayout, Raster, Source};
use specular_iff as iff;
use std::sync::Arc;
use std::time::Duration;

pub struct IffDecoder;

fn map_err(e: iff::Error) -> DecodeError {
    match e {
        iff::Error::NotIff => DecodeError::UnsupportedFormat,
        iff::Error::Unsupported(s) => DecodeError::Unsupported(s),
        iff::Error::Truncated(s) => DecodeError::Truncated(s.into()),
        iff::Error::Malformed(s) => DecodeError::Malformed(s.into()),
        iff::Error::TooLarge => DecodeError::TooLarge,
    }
}

impl Decoder for IffDecoder {
    fn format(&self) -> Format {
        Format::Iff
    }
    fn probe(&self, head: &[u8]) -> bool {
        iff::probe(head)
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["iff", "ilbm", "lbm", "anim", "deep", "rgb8", "rgbn"]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let i = iff::info(source.bytes()).map_err(map_err)?;
        let mut coding = i.coding();
        let cycle = if i.anim_frames > 0 {
            None
        } else if i.colour_cycling {
            iff::cycling(source.bytes()).ok().flatten().map(Arc::new)
        } else {
            None
        };
        match &cycle {
            Some(f) => coding.push_str(&format!(", colour cycling ({} frames)", f.len())),
            None if i.colour_cycling => coding.push_str(", colour cycling (no active range)"),
            None => {}
        }
        let info = ImageInfo {
            format: Format::Iff,
            width: i.width,
            height: i.height,
            orientation: Orientation::NORMAL,
            icc_profile: None,
            has_alpha: i.has_alpha,
            bit_depth: 8,
            progressive: false,
            coding,
            embedded_preview: None,
            fast_regions: false,
            scales: &[1, 2, 4, 8],
        };
        let model = match i.kind {
            iff::Kind::Indexed => "Palette",
            _ => "RGB",
        };
        // A mask plane is a 1-bit alpha channel; a transparent colour is a colour key.
        let alpha = match i.masking {
            iff::Masking::TransparentColor(_) => Alpha::Transparency,
            _ => Alpha::Straight,
        };
        let animation = match &cycle {
            Some(f) => Some(AnimationInfo {
                frame_count: Some(f.len() as u32),
                loop_count: 0,
            }),
            None if i.anim_frames > 1 => Some(AnimationInfo {
                frame_count: Some(i.anim_frames as u32),
                loop_count: 0,
            }),
            None => None,
        };
        Ok(Arc::new(IffSession {
            source,
            model,
            alpha: info.has_alpha.then_some(alpha),
            info,
            levels: Levels::new(),
            cycle,
            animation,
        }))
    }
}

struct IffSession {
    source: Arc<Source>,
    info: ImageInfo,
    model: &'static str,
    alpha: Option<Alpha>,
    levels: Levels,
    /// The colour-cycling frames (palettes and times), when the file has active ranges.
    cycle: Option<Arc<Vec<iff::CycleFrame>>>,
    animation: Option<AnimationInfo>,
}

/// Colour-cycling frames: the image drawn with each frame's palette.
struct IffFrames {
    source: Arc<Source>,
    cycle: Arc<Vec<iff::CycleFrame>>,
    scale: u32,
    index: usize,
}

impl Frames for IffFrames {
    fn next_frame(&mut self, cancel: &CancelToken) -> Option<Result<Frame>> {
        let f = self.cycle.get(self.index)?;
        let i = self.index;
        self.index += 1;
        if cancel.is_cancelled() {
            return Some(Err(DecodeError::Cancelled));
        }
        Some(
            iff::decode_with_cmap(self.source.bytes(), &f.cmap)
                .map_err(map_err)
                .map(|img| {
                    let full = whole::interleaved(
                        img.width,
                        img.height,
                        PixelLayout::Rgba,
                        img.pixels,
                        img.truncated,
                    );
                    let raster = if self.scale > 1 {
                        whole::downscale(&full, img.width, img.height, self.scale)
                    } else {
                        full
                    };
                    Frame {
                        index: i as u32,
                        raster,
                        delay: Duration::from_millis(f.delay_ms.into()),
                    }
                }),
        )
    }
    fn rewind(&mut self) -> Result<()> {
        self.index = 0;
        Ok(())
    }
    fn delays(&self) -> Option<Vec<Duration>> {
        Some(
            self.cycle
                .iter()
                .map(|f| Duration::from_millis(f.delay_ms.into()))
                .collect(),
        )
    }
}

/// ANIM frames, played in order (each a delta on the ones before).
struct AnimFrames {
    source: Arc<Source>,
    player: iff::Player,
    scale: u32,
    index: usize,
    delays: Vec<Duration>,
}

impl Frames for AnimFrames {
    fn next_frame(&mut self, cancel: &CancelToken) -> Option<Result<Frame>> {
        if cancel.is_cancelled() {
            return Some(Err(DecodeError::Cancelled));
        }
        let i = self.index;
        let f = self.player.next_frame(self.source.bytes())?;
        self.index += 1;
        Some(f.map_err(map_err).map(|f| {
            let img = f.image;
            let full = whole::interleaved(
                img.width,
                img.height,
                PixelLayout::Rgba,
                img.pixels,
                img.truncated,
            );
            let raster = if self.scale > 1 {
                whole::downscale(&full, img.width, img.height, self.scale)
            } else {
                full
            };
            Frame {
                index: i as u32,
                raster,
                delay: Duration::from_millis(f.delay_ms.into()),
            }
        }))
    }
    fn rewind(&mut self) -> Result<()> {
        self.player.rewind();
        self.index = 0;
        Ok(())
    }
    fn delays(&self) -> Option<Vec<Duration>> {
        Some(self.delays.clone())
    }
}

impl Session for IffSession {
    fn animation(&self) -> Option<&AnimationInfo> {
        self.animation.as_ref()
    }
    fn frames(&self, scale: u32) -> Result<Box<dyn Frames>> {
        if self.cycle.is_none() && self.animation.is_some() {
            let data = self.source.bytes();
            let player = iff::Player::new(data).map_err(map_err)?;
            return Ok(Box::new(AnimFrames {
                delays: iff::anim_delays(data)
                    .map_err(map_err)?
                    .into_iter()
                    .map(|ms| Duration::from_millis(ms.into()))
                    .collect(),
                source: self.source.clone(),
                player,
                scale: scale.max(1),
                index: 0,
            }));
        }
        let Some(cycle) = self.cycle.clone() else {
            return Err(DecodeError::Unsupported("not animated".into()));
        };
        Ok(Box::new(IffFrames {
            source: self.source.clone(),
            cycle,
            scale: scale.max(1),
            index: 0,
        }))
    }
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn source(&self) -> &Arc<Source> {
        &self.source
    }
    fn colour_model(&self) -> Option<&'static str> {
        Some(self.model)
    }
    /// A mask plane or transparent colour (on or off), or 32-bit deep alpha.
    fn alpha(&self) -> Option<Alpha> {
        self.alpha
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        None
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        self.levels.decode(
            req,
            (self.info.width, self.info.height),
            |_| 1,
            |_| {
                let img = iff::decode(self.source.bytes()).map_err(map_err)?;
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
        let (w, h) = (self.info.width, self.info.height);
        let s = req.scale.max(1);
        let mut m = whole::level_bytes(w, h, s, 4);
        if !self.levels.has(s) && !self.levels.has(1) {
            // The full image plus the decoder's row and index buffers.
            m += whole::level_bytes(w, h, 1, 5);
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
