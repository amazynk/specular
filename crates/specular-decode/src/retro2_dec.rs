//! 8-bit home computer screens and Kodak Photo CD through `specular-retro2`: Amstrad CPC
//! (`.scr`, with an OCP `.pal` beside it when there is one), MSX (`.sc2`–`.scc`), Apple II
//! hi-res and double hi-res, BBC Micro (`.bb0`–`.bb5`), Atari 8-bit (`.mic`, `.gr8`,
//! `.gr9`) and Photo CD (`.pcd`, Base resolution; Base/16 is its quick preview).
//!
//! Screen dumps have no signature: only AMSDOS-headed CPC screens, BSAVE-headed MSX screens
//! and Photo CD's `PCD_IPI` are recognised by content; the others open by extension and are
//! checked by size. Each picture is decoded whole once; coarser levels are box-filtered.
//! CPC screens show their three screen modes as pages.

use crate::whole::{self, Levels};
use crate::{
    Confidence, DecodeError, DecodeRequest, Decoder, Format, HeaderFacts, ImageInfo, PageLabel,
    Result, Session,
};
use specular_core::{CancelToken, Orientation, PixelLayout, Raster, Source};
use specular_retro2::{self as r2, Kind};
use std::sync::Arc;

/// Largest `.pal` read beside a CPC screen.
const MAX_PAL: u64 = 4096;

pub struct Retro2Decoder(pub Kind);

pub fn format_of(kind: Kind) -> Format {
    match kind {
        Kind::CpcScreen => Format::CpcScreen,
        Kind::MsxScreen => Format::MsxScreen,
        Kind::AppleHgr => Format::AppleHgr,
        Kind::AppleDhr => Format::AppleDhr,
        Kind::BbcScreen => Format::BbcScreen,
        Kind::AtariMic => Format::AtariMic,
        Kind::AtariGr8 => Format::AtariGr8,
        Kind::AtariGr9 => Format::AtariGr9,
        Kind::PhotoCd => Format::PhotoCd,
    }
}

fn map_err(e: r2::Error) -> DecodeError {
    match e {
        r2::Error::NotRecognised => DecodeError::UnsupportedFormat,
        r2::Error::Unsupported(s) => DecodeError::Unsupported(s),
        r2::Error::Truncated(s) => DecodeError::Truncated(s),
        r2::Error::Malformed(s) => DecodeError::Malformed(s),
    }
}

impl Decoder for Retro2Decoder {
    fn format(&self) -> Format {
        format_of(self.0)
    }
    /// The signatures need more than the first bytes ([`Decoder::probe_file`]).
    fn probe(&self, _head: &[u8]) -> bool {
        false
    }
    fn probe_file(&self, data: &[u8]) -> bool {
        r2::signature(self.0, data)
    }
    /// An AMSDOS or BSAVE header is certain; a bare CPC screen (`.scr`, shared with ZX
    /// Spectrum screens) is only possible, at exactly 16 KB.
    fn confidence(&self, data: &[u8]) -> Option<Confidence> {
        if r2::signature(self.0, data) {
            Some(Confidence::Certain)
        } else if self.0 == Kind::CpcScreen && data.len() == 16 * 1024 {
            // Exactly 16 KB rules out the ZX Spectrum screen sharing `.scr`.
            Some(Confidence::Likely)
        } else {
            None
        }
    }
    fn extensions(&self) -> &'static [&'static str] {
        self.0.extensions()
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        open_page(self.0, source, 0)
    }
}

/// The file's extension and, for a CPC screen, the `.pal` beside it.
struct Context {
    ext: String,
    palette: Option<Vec<u8>>,
}

impl Context {
    fn of(kind: Kind, source: &Source) -> Context {
        let path = source.path();
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .unwrap_or_default();
        let palette = (kind == Kind::CpcScreen)
            .then(|| {
                ["pal", "PAL"].iter().find_map(|e| {
                    let p = path.with_extension(e);
                    let meta = std::fs::metadata(&p).ok()?;
                    (meta.is_file() && meta.len() <= MAX_PAL)
                        .then(|| std::fs::read(&p).ok())
                        .flatten()
                })
            })
            .flatten();
        Context { ext, palette }
    }

    fn hints(&self) -> r2::Hints<'_> {
        r2::Hints {
            ext: &self.ext,
            palette: self.palette.as_deref(),
        }
    }
}

fn open_page(kind: Kind, source: Arc<Source>, page: u32) -> Result<Arc<dyn Session>> {
    let ctx = Context::of(kind, &source);
    let i = r2::info(kind, source.bytes(), &ctx.hints(), page).map_err(map_err)?;
    let info = ImageInfo {
        format: format_of(kind),
        width: i.width,
        height: i.height,
        orientation: Orientation::from_exif(i.orientation),
        icc_profile: None,
        has_alpha: false,
        bit_depth: i.bit_depth,
        progressive: false,
        coding: i.coding.clone(),
        embedded_preview: None,
        fast_regions: false,
        scales: &[1, 2, 4, 8],
    };
    Ok(Arc::new(Retro2Session {
        kind,
        page,
        pages: i.pages,
        source,
        ctx,
        info,
        model: if kind == Kind::PhotoCd {
            "YCbCr"
        } else if i.palette {
            "Palette"
        } else {
            "RGB"
        },
        header: HeaderFacts {
            summary: Vec::new(),
            fields: i.fields,
        },
        levels: Levels::new(),
    }))
}

fn to_raster(img: r2::Image) -> Raster {
    whole::interleaved(
        img.width,
        img.height,
        PixelLayout::Rgba,
        img.rgba,
        img.truncated,
    )
}

struct Retro2Session {
    kind: Kind,
    page: u32,
    pages: u32,
    source: Arc<Source>,
    ctx: Context,
    info: ImageInfo,
    model: &'static str,
    header: HeaderFacts,
    levels: Levels,
}

impl Session for Retro2Session {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn source(&self) -> &Arc<Source> {
        &self.source
    }
    fn colour_model(&self) -> Option<&'static str> {
        Some(self.model)
    }
    fn header(&self) -> Option<&HeaderFacts> {
        (!self.header.fields.is_empty()).then_some(&self.header)
    }
    /// Photo CD's Base/16 image (192×128), stored whole at the start of the file.
    fn decode_preview(&self) -> Option<Result<Raster>> {
        (self.kind == Kind::PhotoCd).then(|| {
            r2::photocd::decode(self.source.bytes(), r2::photocd::Resolution::Base16)
                .map(to_raster)
                .map_err(map_err)
        })
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        self.levels.decode(
            req,
            (self.info.width, self.info.height),
            |_| 1,
            |_| {
                let img = r2::decode(self.kind, self.source.bytes(), &self.ctx.hints(), self.page)
                    .map_err(map_err)?;
                if (img.width, img.height) != (self.info.width, self.info.height) {
                    return Err(DecodeError::Internal(format!(
                        "decoder gave {}×{} for a {}×{} image",
                        img.width, img.height, self.info.width, self.info.height
                    )));
                }
                Ok(to_raster(img))
            },
            cancel,
        )
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        let (w, h) = (self.info.width, self.info.height);
        let s = req.scale.max(1);
        let mut m = whole::level_bytes(w, h, s, 4);
        if !self.levels.has(s) && !self.levels.has(1) {
            m += whole::level_bytes(w, h, 1, 4);
        }
        m
    }
    fn memory_bytes(&self) -> usize {
        self.levels.memory_bytes()
    }
    fn trim_memory(&self) -> usize {
        self.levels.clear()
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
            bits: Some(self.info.bit_depth as u32),
        })
    }
    fn open_page(&self, page: u32) -> Result<Arc<dyn Session>> {
        if page >= self.pages {
            return Err(DecodeError::Malformed("no such page".into()));
        }
        open_page(self.kind, self.source.clone(), page)
    }
}
