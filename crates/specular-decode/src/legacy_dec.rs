//! Small legacy formats through the in-tree `specular-legacy` decoders: PCX, DCX (multi-page
//! PCX, each page a [`Session`] page), SGI, Sun Raster, XBM, XPM, WBMP, farbfeld, and FLI/FLC
//! animations.
//!
//! Still images are decoded whole once (to 8-bit grey or RGBA); coarser levels are
//! box-filtered from it and regions are cropped. FLI/FLC play through [`Frames`] (every frame
//! is the full composited canvas); `decode` shows the first frame.

use crate::whole::{self, Levels};
use crate::{
    Alpha, AnimationInfo, DecodeError, DecodeRequest, Decoder, Format, Frame, Frames, ImageInfo,
    PageLabel, Result, Session,
};
use specular_core::{CancelToken, Orientation, PixelLayout, Raster, Source};
use specular_legacy as lg;
use std::sync::Arc;

/// How many bytes the content probes look at (PCX needs 128, XBM/XPM up to 512).
const PROBE_BYTES: usize = 512;

pub struct LegacyDecoder(pub lg::Format);

fn map_err(e: lg::Error) -> DecodeError {
    match e {
        lg::Error::NotRecognised => DecodeError::UnsupportedFormat,
        lg::Error::Unsupported(s) => DecodeError::Unsupported(s),
        lg::Error::Truncated(s) => DecodeError::Truncated(s),
        lg::Error::Malformed(s) => DecodeError::Malformed(s),
        lg::Error::TooLarge => DecodeError::TooLarge,
    }
}

fn probe(kind: lg::Format, head: &[u8]) -> bool {
    match kind {
        lg::Format::Pcx => lg::pcx::probe(head),
        lg::Format::Dcx => lg::dcx::probe(head),
        lg::Format::Sgi => lg::sgi::probe(head),
        lg::Format::SunRaster => lg::sun::probe(head),
        lg::Format::Xbm => lg::xbm::probe(head),
        lg::Format::Xpm => lg::xpm::probe(head),
        // No magic: only the whole file can be checked (by its exact size).
        lg::Format::Wbmp => lg::wbmp::probe(head),
        lg::Format::Farbfeld => lg::farbfeld::probe(head),
        lg::Format::Fli => lg::fli::probe(head),
    }
}

fn info(kind: lg::Format, data: &[u8], page: u32) -> lg::Result<lg::Info> {
    match kind {
        lg::Format::Pcx => lg::pcx::info(data),
        lg::Format::Dcx => lg::dcx::info(data, page as usize),
        lg::Format::Sgi => lg::sgi::info(data),
        lg::Format::SunRaster => lg::sun::info(data),
        lg::Format::Xbm => lg::xbm::info(data),
        lg::Format::Xpm => lg::xpm::info(data),
        lg::Format::Wbmp => lg::wbmp::info(data),
        lg::Format::Farbfeld => lg::farbfeld::info(data),
        lg::Format::Fli => lg::fli::info(data),
    }
}

fn decode(kind: lg::Format, data: &[u8], page: u32) -> lg::Result<lg::Image> {
    match kind {
        lg::Format::Pcx => lg::pcx::decode(data),
        lg::Format::Dcx => lg::dcx::decode(data, page as usize),
        lg::Format::Sgi => lg::sgi::decode(data),
        lg::Format::SunRaster => lg::sun::decode(data),
        lg::Format::Xbm => lg::xbm::decode(data),
        lg::Format::Xpm => lg::xpm::decode(data),
        lg::Format::Wbmp => lg::wbmp::decode(data),
        lg::Format::Farbfeld => lg::farbfeld::decode(data),
        lg::Format::Fli => lg::fli::decode_first(data),
    }
}

fn to_raster(img: lg::Image) -> Raster {
    let layout = match img.format {
        lg::PixelFormat::Gray8 => PixelLayout::Gray,
        lg::PixelFormat::Rgba8 => PixelLayout::Rgba,
    };
    whole::interleaved(img.width, img.height, layout, img.data, img.truncated)
}

impl Decoder for LegacyDecoder {
    fn format(&self) -> Format {
        match self.0 {
            lg::Format::Pcx => Format::Pcx,
            lg::Format::Dcx => Format::Dcx,
            lg::Format::Sgi => Format::Sgi,
            lg::Format::SunRaster => Format::SunRaster,
            lg::Format::Xbm => Format::Xbm,
            lg::Format::Xpm => Format::Xpm,
            lg::Format::Wbmp => Format::Wbmp,
            lg::Format::Farbfeld => Format::Farbfeld,
            lg::Format::Fli => Format::Fli,
        }
    }
    fn probe(&self, head: &[u8]) -> bool {
        probe(self.0, head)
    }
    fn probe_file(&self, data: &[u8]) -> bool {
        match self.0 {
            lg::Format::Wbmp => probe(self.0, data),
            k => probe(k, &data[..data.len().min(PROBE_BYTES)]),
        }
    }
    fn extensions(&self) -> &'static [&'static str] {
        match self.0 {
            lg::Format::Pcx => &["pcx"],
            lg::Format::Dcx => &["dcx"],
            lg::Format::Sgi => &["sgi", "rgb", "rgba", "bw", "int", "inta"],
            lg::Format::SunRaster => &["ras", "sun", "rs", "im1", "im8", "im24", "im32"],
            lg::Format::Xbm => &["xbm"],
            lg::Format::Xpm => &["xpm"],
            lg::Format::Wbmp => &["wbmp"],
            lg::Format::Farbfeld => &["ff"],
            lg::Format::Fli => &["fli", "flc"],
        }
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        open_page(self.0, source, 0)
    }
}

fn open_page(kind: lg::Format, source: Arc<Source>, page: u32) -> Result<Arc<dyn Session>> {
    {
        let data = source.bytes();
        let pages = match kind {
            lg::Format::Dcx => lg::dcx::pages(data).map_err(map_err)?.len() as u32,
            _ => 1,
        };
        let i = info(kind, data, page).map_err(map_err)?;
        let anim = if kind == lg::Format::Fli {
            let fi = lg::fli::fli_info(data).map_err(map_err)?;
            (fi.frame_count > 1).then_some(AnimationInfo {
                frame_count: Some(fi.frame_count),
                loop_count: fi.loop_count,
            })
        } else {
            None
        };
        let info = ImageInfo {
            format: LegacyDecoder(kind).format(),
            width: i.width,
            height: i.height,
            orientation: Orientation::NORMAL,
            icc_profile: None,
            has_alpha: i.has_alpha,
            bit_depth: i.bit_depth,
            progressive: false,
            coding: i.coding,
            embedded_preview: None,
            fast_regions: false,
            scales: &[1, 2, 4, 8],
        };
        let model = match i.model {
            lg::Model::Grey => "Greyscale",
            lg::Model::Rgb => "RGB",
            lg::Model::Palette => "Palette",
        };
        let bpp = match i.pixel_format {
            lg::PixelFormat::Gray8 => 1,
            lg::PixelFormat::Rgba8 => 4,
        };
        Ok(Arc::new(LegacySession {
            kind,
            page,
            pages,
            source,
            info,
            model,
            bpp,
            anim,
            levels: Levels::new(),
        }))
    }
}

struct LegacySession {
    kind: lg::Format,
    /// The page shown (DCX) and how many there are.
    page: u32,
    pages: u32,
    source: Arc<Source>,
    info: ImageInfo,
    model: &'static str,
    /// Bytes per decoded pixel.
    bpp: u64,
    anim: Option<AnimationInfo>,
    levels: Levels,
}

impl Session for LegacySession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn colour_model(&self) -> Option<&'static str> {
        Some(self.model)
    }
    /// An alpha channel (SGI, PCX, farbfeld), or XPM's transparent colour "None".
    fn alpha(&self) -> Option<Alpha> {
        self.info.has_alpha.then_some(if self.model == "Palette" {
            Alpha::Transparency
        } else {
            Alpha::Straight
        })
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
            |_| {
                decode(self.kind, self.source.bytes(), self.page)
                    .map(to_raster)
                    .map_err(map_err)
            },
            cancel,
        )
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        let (w, h) = (self.info.width, self.info.height);
        let s = req.scale.max(1);
        let mut m = whole::level_bytes(w, h, s, self.bpp);
        if !self.levels.has(s) && !self.levels.has(1) {
            // The full image plus the decoder's index / canvas buffer.
            m += whole::level_bytes(w, h, 1, self.bpp + 1);
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
    fn animation(&self) -> Option<&AnimationInfo> {
        self.anim.as_ref()
    }
    fn page_count(&self) -> u32 {
        self.pages
    }
    fn page_index(&self) -> u32 {
        self.page
    }
    fn page_label(&self) -> Option<PageLabel> {
        (self.pages > 1).then_some(PageLabel::Image {
            width: self.info.width,
            height: self.info.height,
            bits: None,
        })
    }
    fn open_page(&self, page: u32) -> Result<Arc<dyn Session>> {
        if page >= self.pages {
            return Err(DecodeError::Malformed("no such page".into()));
        }
        open_page(self.kind, self.source.clone(), page)
    }
    fn frames(&self, scale: u32) -> Result<Box<dyn Frames>> {
        if self.anim.is_none() {
            return Err(DecodeError::Unsupported("not animated".into()));
        }
        let fli = lg::fli::FliFrames::new(SourceBytes(self.source.clone())).map_err(map_err)?;
        Ok(Box::new(FliFrames {
            fli,
            width: self.info.width,
            height: self.info.height,
            scale: scale.clamp(1, 8),
        }))
    }
}

struct SourceBytes(Arc<Source>);

impl AsRef<[u8]> for SourceBytes {
    fn as_ref(&self) -> &[u8] {
        self.0.bytes()
    }
}

struct FliFrames {
    fli: lg::fli::FliFrames<SourceBytes>,
    width: u32,
    height: u32,
    scale: u32,
}

impl Frames for FliFrames {
    fn next_frame(&mut self, cancel: &CancelToken) -> Option<Result<Frame>> {
        if cancel.is_cancelled() {
            return Some(Err(DecodeError::Cancelled));
        }
        let f = match self.fli.next_frame()? {
            Ok(f) => f,
            Err(e) => return Some(Err(map_err(e))),
        };
        let mut raster = to_raster(f.image);
        if self.scale > 1 {
            raster = whole::downscale(&raster, self.width, self.height, self.scale);
        }
        Some(Ok(Frame {
            index: f.index,
            raster,
            delay: f.delay,
        }))
    }
    fn rewind(&mut self) -> Result<()> {
        self.fli.rewind().map_err(map_err)
    }
}
