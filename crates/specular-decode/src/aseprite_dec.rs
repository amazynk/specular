//! Aseprite sprites (`.aseprite`, `.ase`) through `specular-legacy2`: the first frame as the
//! image, every frame through the animation player with its own duration. Layers, tags and
//! the colour mode are Info facts. `.ase` is also Adobe Swatch Exchange's: told apart by
//! content (0xA5E0 at byte 4 against "ASEF").

use crate::whole::{self, Levels};
use crate::{
    Alpha, AnimationInfo, DecodeError, DecodeRequest, Decoder, Format, Frame, Frames, HeaderFacts,
    ImageInfo, Result, Session,
};
use specular_core::{CancelToken, Orientation, PixelLayout, Raster, Source};
use specular_legacy2::aseprite as ase;
use std::sync::Arc;
use std::time::Duration;

pub struct AsepriteDecoder;

fn map_err(e: specular_legacy2::Error) -> DecodeError {
    use specular_legacy2::Error as E;
    match e {
        E::NotRecognised => DecodeError::UnsupportedFormat,
        E::Unsupported(s) => DecodeError::Unsupported(s),
        E::Truncated(s) => DecodeError::Truncated(s),
        E::Malformed(s) => DecodeError::Malformed(s),
        E::TooLarge => DecodeError::TooLarge,
        E::NeedsMemory(n) => DecodeError::NeedsMemory(n),
    }
}

impl Decoder for AsepriteDecoder {
    fn format(&self) -> Format {
        Format::Aseprite
    }
    fn probe(&self, head: &[u8]) -> bool {
        ase::probe(head)
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["aseprite", "ase"]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let sprite = Arc::new(ase::parse(source.bytes()).map_err(map_err)?);
        let i = sprite.info();
        let budget = u64::from(sprite.width) * u64::from(sprite.height) * 16;
        if budget > 4 << 30 {
            return Err(DecodeError::TooLarge);
        }
        let info = ImageInfo {
            format: Format::Aseprite,
            width: i.width,
            height: i.height,
            orientation: Orientation::NORMAL,
            icc_profile: None,
            has_alpha: true,
            bit_depth: 8,
            progressive: false,
            coding: i.coding,
            embedded_preview: None,
            fast_regions: false,
            scales: &[1, 2, 4, 8],
        };
        let anim = (sprite.frames.len() > 1).then_some(AnimationInfo {
            frame_count: Some(sprite.frames.len() as u32),
            loop_count: 0,
        });
        Ok(Arc::new(AsepriteSession {
            source,
            sprite,
            info,
            header: HeaderFacts {
                summary: Vec::new(),
                fields: i.fields,
            },
            anim,
            levels: Levels::new(),
        }))
    }
}

struct AsepriteSession {
    source: Arc<Source>,
    sprite: Arc<ase::Sprite>,
    info: ImageInfo,
    header: HeaderFacts,
    anim: Option<AnimationInfo>,
    levels: Levels,
}

fn raster(s: &ase::Sprite, frame: usize) -> Raster {
    whole::interleaved(
        s.width,
        s.height,
        PixelLayout::Rgba,
        s.render(frame),
        s.truncated,
    )
}

impl Session for AsepriteSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn source(&self) -> &Arc<Source> {
        &self.source
    }
    fn colour_model(&self) -> Option<&'static str> {
        Some(match self.sprite.depth {
            32 => "RGB",
            16 => "Greyscale",
            _ => "Palette",
        })
    }
    fn alpha(&self) -> Option<Alpha> {
        Some(Alpha::Straight)
    }
    fn header(&self) -> Option<&HeaderFacts> {
        Some(&self.header)
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        None
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        self.levels.decode(
            req,
            (self.info.width, self.info.height),
            |_| 1,
            |_| Ok(raster(&self.sprite, 0)),
            cancel,
        )
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        let (w, h) = (self.info.width, self.info.height);
        let s = req.scale.max(1);
        let mut m = whole::level_bytes(w, h, s, 4);
        if !self.levels.has(s) && !self.levels.has(1) {
            // The float canvas and the full level.
            m += whole::level_bytes(w, h, 1, 20);
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
        self.anim.as_ref()
    }
    fn frames(&self, scale: u32) -> Result<Box<dyn Frames>> {
        if self.anim.is_none() {
            return Err(DecodeError::Unsupported("not animated".into()));
        }
        Ok(Box::new(AseFrames {
            sprite: self.sprite.clone(),
            next: 0,
            scale: scale.clamp(1, 8),
        }))
    }
}

struct AseFrames {
    sprite: Arc<ase::Sprite>,
    next: usize,
    scale: u32,
}

impl Frames for AseFrames {
    fn next_frame(&mut self, cancel: &CancelToken) -> Option<Result<Frame>> {
        if self.next >= self.sprite.frames.len() {
            return None;
        }
        if cancel.is_cancelled() {
            return Some(Err(DecodeError::Cancelled));
        }
        let i = self.next;
        self.next += 1;
        let mut r = raster(&self.sprite, i);
        if self.scale > 1 {
            r = whole::downscale(&r, self.sprite.width, self.sprite.height, self.scale);
        }
        Some(Ok(Frame {
            index: i as u32,
            raster: r,
            delay: Duration::from_millis(u64::from(self.sprite.frames[i].duration)),
        }))
    }
    fn rewind(&mut self) -> Result<()> {
        self.next = 0;
        Ok(())
    }
    /// Every frame is drawn whole from its cels: any frame can be the next.
    fn seek(&mut self, index: u32) -> Result<bool> {
        self.next = index as usize;
        Ok(true)
    }
    fn delays(&self) -> Option<Vec<Duration>> {
        Some(
            self.sprite
                .frames
                .iter()
                .map(|f| Duration::from_millis(u64::from(f.duration)))
                .collect(),
        )
    }
    fn sync_frames(&self) -> Option<Vec<u32>> {
        Some((0..self.sprite.frames.len() as u32).collect())
    }
}
