//! BPG through the in-tree `specular-bpg` decoder (HEVC by `specular-hevc`).
//!
//! The picture (an animation's first frame) is decoded once into RGBA8 or, above 8 bits,
//! 16-bit RGBA (grey without alpha stays grey); coarser levels are box-filtered from it.
//! Animations play through [`Frames`] with the durations the file signals. The ICC profile,
//! EXIF and XMP extensions are handed to the colour pipeline and the info panel; the
//! thumbnail extension is the embedded preview.

use crate::whole::{self, Levels};
use crate::{
    Alpha, AnimationInfo, DecodeError, DecodeRequest, Decoder, Format, Frame, Frames, ImageInfo,
    Result, Session,
};
use specular_bpg as sb;
use specular_core::{CancelToken, HdrEncoding, Orientation, PixelLayout, Raster, Source};
use std::sync::Arc;

pub struct BpgDecoder;

/// HEVC worker threads for a parallel decode (helps pictures with tiles or wavefronts).
const THREADS: usize = 4;

fn map_err(e: sb::Error) -> DecodeError {
    match e {
        sb::Error::NotBpg => DecodeError::Malformed("not a BPG file".into()),
        sb::Error::Unsupported(s) => DecodeError::Unsupported(s),
        sb::Error::Truncated(s) => DecodeError::Truncated(s),
        sb::Error::Malformed(s) => DecodeError::Malformed(s),
        sb::Error::TooLarge => DecodeError::TooLarge,
        sb::Error::NeedsMemory(n) => DecodeError::NeedsMemory(n),
        sb::Error::Cancelled => DecodeError::Cancelled,
    }
}

fn layout(f: sb::PixelFormat) -> PixelLayout {
    match f {
        sb::PixelFormat::Gray8 => PixelLayout::Gray,
        sb::PixelFormat::Rgba8 => PixelLayout::Rgba,
        sb::PixelFormat::Gray16 => PixelLayout::Gray16,
        sb::PixelFormat::Rgba16 => PixelLayout::Rgba16(HdrEncoding::SRGB),
    }
}

fn raster(img: sb::Image) -> Raster {
    whole::interleaved(
        img.width,
        img.height,
        layout(img.format),
        img.data,
        img.truncated,
    )
}

fn subsampling(c: sb::Chroma) -> Option<&'static str> {
    match c {
        sb::Chroma::Gray => None,
        sb::Chroma::C420 => Some("4:2:0"),
        sb::Chroma::C422 => Some("4:2:2"),
        sb::Chroma::C444 => Some("4:4:4"),
    }
}

/// "BPG 10-bit YCbCr BT.709 4:2:0, limited range, alpha, 12 frames".
fn coding(i: &sb::Info, frames: Option<u32>) -> String {
    let model = if i.cmyk {
        "CMYK".to_string()
    } else if i.chroma == sb::Chroma::Gray {
        "grey".to_string()
    } else {
        match i.colour_space {
            sb::ColourSpace::YCbCr601 => "YCbCr BT.601",
            sb::ColourSpace::YCbCr709 => "YCbCr BT.709",
            sb::ColourSpace::YCbCr2020 => "YCbCr BT.2020",
            sb::ColourSpace::Rgb => "RGB",
            sb::ColourSpace::YCgCo => "YCgCo",
        }
        .to_string()
    };
    let mut s = format!("BPG {}-bit {model}", i.bit_depth);
    if let Some(c) = subsampling(i.chroma).filter(|_| i.chroma != sb::Chroma::C444) {
        s += &format!(" {c}");
        if i.video_siting {
            s += " (video siting)";
        }
    }
    if i.limited_range {
        s += ", limited range";
    }
    match i.alpha {
        sb::AlphaKind::None => {}
        sb::AlphaKind::Straight => s += ", alpha",
        sb::AlphaKind::Premultiplied => s += ", premultiplied alpha",
    }
    if let Some(n) = frames {
        s += &format!(", {n} frames");
    }
    s
}

impl Decoder for BpgDecoder {
    fn format(&self) -> Format {
        Format::Bpg
    }
    fn probe(&self, head: &[u8]) -> bool {
        sb::probe(head)
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["bpg"]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let data = source.bytes();
        let i = sb::read_info(data).map_err(map_err)?;
        // Frames are counted by walking the stream (cheap: NAL headers only).
        let animation = match &i.animation {
            Some(a) => {
                let all: Arc<[u8]> = Arc::from(data);
                let n = sb::Frames::new(all, 1).map_err(map_err)?.frame_count();
                (n > 1).then_some(AnimationInfo {
                    frame_count: Some(n),
                    loop_count: a.loop_count,
                })
            }
            None => None,
        };
        let info = ImageInfo {
            format: Format::Bpg,
            width: i.width,
            height: i.height,
            orientation: Orientation::NORMAL,
            icc_profile: i.icc.clone().map(Arc::new),
            has_alpha: i.alpha != sb::AlphaKind::None,
            bit_depth: i.bit_depth,
            progressive: false,
            coding: coding(&i, animation.as_ref().and_then(|a| a.frame_count)),
            embedded_preview: None,
            fast_regions: false,
            scales: &[1, 2, 4, 8],
        };
        Ok(Arc::new(BpgSession {
            source,
            info,
            bpg: i,
            animation,
            levels: Levels::new(),
        }))
    }
}

struct BpgSession {
    source: Arc<Source>,
    info: ImageInfo,
    bpg: sb::Info,
    animation: Option<AnimationInfo>,
    levels: Levels,
}

impl Session for BpgSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn source(&self) -> &Arc<Source> {
        &self.source
    }
    /// The thumbnail extension (a small BPG inside the file), when there is one.
    fn decode_preview(&self) -> Option<Result<Raster>> {
        if !self.bpg.has_thumbnail {
            return None;
        }
        sb::decode_thumbnail(self.source.bytes()).map(|r| r.map(raster).map_err(map_err))
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        self.levels.decode(
            req,
            (self.info.width, self.info.height),
            |_| 1,
            |_| {
                let img = sb::decode(
                    self.source.bytes(),
                    &sb::Options {
                        threads: if req.parallel { THREADS } else { 1 },
                        cancel: Some(cancel.flag()),
                    },
                )
                .map_err(map_err)?;
                Ok(raster(img))
            },
            cancel,
        )
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        let (w, h) = (self.info.width, self.info.height);
        let bpp = if self.bpg.bit_depth > 8 { 8 } else { 4 };
        let s = req.scale.max(1);
        let mut m = whole::level_bytes(w, h, s, bpp);
        if !self.levels.has(s) && !self.levels.has(1) {
            // The output plus the decoder's 16-bit planes (luma, chroma, alpha).
            m += whole::level_bytes(w, h, 1, bpp + 8);
        }
        m
    }
    fn memory_bytes(&self) -> usize {
        self.levels.memory_bytes()
    }
    fn trim_memory(&self) -> usize {
        self.levels.clear()
    }
    fn scaled_decode_cheap(&self) -> bool {
        false
    }
    fn animation(&self) -> Option<&AnimationInfo> {
        self.animation.as_ref()
    }
    fn frames(&self, scale: u32) -> Result<Box<dyn Frames>> {
        let data: Arc<[u8]> = Arc::from(self.source.bytes());
        let inner = sb::Frames::new(data, THREADS).map_err(map_err)?;
        Ok(Box::new(BpgFrames {
            inner,
            scale: scale.max(1),
            size: (self.info.width, self.info.height),
            index: 0,
        }))
    }
    fn colour_model(&self) -> Option<&'static str> {
        Some(if self.bpg.cmyk {
            "CMYK"
        } else if self.bpg.chroma == sb::Chroma::Gray {
            "Greyscale"
        } else {
            match self.bpg.colour_space {
                sb::ColourSpace::Rgb => "RGB",
                sb::ColourSpace::YCgCo => "YCgCo",
                _ => "YCbCr",
            }
        })
    }
    fn chroma_subsampling(&self) -> Option<&'static str> {
        match self.bpg.colour_space {
            sb::ColourSpace::Rgb => None,
            _ => subsampling(self.bpg.chroma),
        }
    }
    fn alpha(&self) -> Option<Alpha> {
        match self.bpg.alpha {
            sb::AlphaKind::None => None,
            sb::AlphaKind::Straight => Some(Alpha::Straight),
            sb::AlphaKind::Premultiplied => Some(Alpha::Premultiplied),
        }
    }
    fn embedded_icc(&self) -> Option<&[u8]> {
        self.bpg.icc.as_deref()
    }
    fn embedded_xmp(&self) -> Option<Vec<u8>> {
        self.bpg.xmp.clone()
    }
    fn embedded_exif(&self) -> Option<Vec<u8>> {
        crate::exif_tiff_of(self.bpg.exif.as_deref()?)
    }
}

struct BpgFrames {
    inner: sb::Frames,
    scale: u32,
    size: (u32, u32),
    index: u32,
}

impl Frames for BpgFrames {
    fn next_frame(&mut self, cancel: &CancelToken) -> Option<Result<Frame>> {
        let r = self.inner.next_frame(Some(cancel.flag()))?;
        let i = self.index;
        self.index += 1;
        Some(r.map_err(map_err).map(|(img, delay)| {
            let full = raster(img);
            let raster = if self.scale > 1 {
                whole::downscale(&full, self.size.0, self.size.1, self.scale)
            } else {
                full
            };
            Frame {
                index: i,
                raster,
                delay,
            }
        }))
    }
    fn rewind(&mut self) -> Result<()> {
        self.index = 0;
        self.inner.rewind().map_err(map_err)
    }
}
