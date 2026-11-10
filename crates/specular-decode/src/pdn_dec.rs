//! Paint.NET `.pdn` through the in-tree `specular-pdn` reader.
//!
//! The document stream is parsed once at open (the object graph and the index of the layer
//! chunks; pixels are not touched). Page 0 is the visible layers composited as Paint.NET
//! flattens them (its 14 blend modes, opacity), once, into RGBA8; coarser levels are
//! box-filtered from it. Pages 1‥ are the layers, top-most first as Paint.NET's Layers
//! window lists them (hidden ones too, labelled), each alone on the canvas with its opacity.
//! The thumbnail Paint.NET stores in the header (a PNG of at most 256 pixels) is the embedded
//! preview, which the gallery uses instead of compositing.

use crate::png_dec::PngDecoder;
use crate::whole::{self, Levels};
use crate::{
    Alpha, DecodeError, DecodeRequest, Decoder, Format, ImageInfo, PageLabel, Result, Session,
};
use specular_core::{CancelToken, Orientation, PixelLayout, Raster, Source};
use specular_pdn as sp;
use std::sync::Arc;

pub struct PdnDecoder;

/// Layer names listed in the coding line before "…".
const NAMES_SHOWN: usize = 6;

fn map_err(e: sp::Error) -> DecodeError {
    match e {
        sp::Error::NotPdn => DecodeError::Malformed("not a Paint.NET document".into()),
        sp::Error::Unsupported(s) => DecodeError::Unsupported(s),
        sp::Error::Truncated(s) => DecodeError::Truncated(s),
        sp::Error::Malformed(s) => DecodeError::Malformed(s),
        sp::Error::TooLarge => DecodeError::TooLarge,
        sp::Error::NeedsMemory(n) => DecodeError::NeedsMemory(n),
        sp::Error::Cancelled => DecodeError::Cancelled,
    }
}

fn plural(n: usize, one: &str) -> String {
    format!("{n} {one}{}", if n == 1 { "" } else { "s" })
}

/// "Paint.NET 4.21, 3 layers (1 hidden): Background, Shadow (Multiply), …".
fn coding(p: &sp::Pdn) -> String {
    let mut s = match p.header().saved_with.as_deref() {
        Some(v) => {
            // "4.21.6589.7045" → "4.21": the build and revision mean nothing to a reader.
            let short: Vec<&str> = v.split('.').take(2).collect();
            format!("Paint.NET {}", short.join("."))
        }
        None => "Paint.NET 2.x".into(),
    };
    let layers = p.layers();
    s += &format!(", {}", plural(layers.len(), "layer"));
    let hidden = layers.iter().filter(|l| !l.visible).count();
    if hidden > 0 {
        s += &format!(" ({hidden} hidden)");
    }
    let names: Vec<String> = layers
        .iter()
        .rev()
        .map(|l| {
            let mut n = l.name.clone();
            let mode = l.blend != sp::BlendMode::Normal;
            if mode || l.opacity < 255 {
                let mut extra = Vec::new();
                if mode {
                    extra.push(l.blend.name().to_string());
                }
                if l.opacity < 255 {
                    extra.push(format!("{} %", (u32::from(l.opacity) * 100 + 127) / 255));
                }
                n += &format!(" ({})", extra.join(", "));
            }
            n
        })
        .collect();
    if !names.is_empty() {
        s += ": ";
        s += &names[..names.len().min(NAMES_SHOWN)].join(", ");
        if names.len() > NAMES_SHOWN {
            s += ", …";
        }
    }
    let unknown: Vec<&str> = layers
        .iter()
        .filter_map(|l| l.unknown_blend.as_deref())
        .collect();
    if !unknown.is_empty() {
        s += &format!(" ({} drawn as Normal)", unknown.join(", "));
    }
    if p.truncated() {
        s += " (layer data missing)";
    }
    s
}

impl Decoder for PdnDecoder {
    fn format(&self) -> Format {
        Format::Pdn
    }
    fn probe(&self, head: &[u8]) -> bool {
        sp::probe(head)
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["pdn"]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let pdn = sp::parse(source.bytes()).map_err(map_err)?;
        let info = ImageInfo {
            format: Format::Pdn,
            width: pdn.width(),
            height: pdn.height(),
            orientation: Orientation::NORMAL,
            icc_profile: None,
            has_alpha: true,
            bit_depth: 8,
            progressive: false,
            coding: coding(&pdn),
            embedded_preview: None,
            fast_regions: false,
            scales: &[1, 2, 4, 8],
        };
        Ok(Arc::new(PdnSession {
            file: Arc::new(PdnFile {
                source,
                merged: info.clone(),
                pdn,
            }),
            page: 0,
            info,
            levels: Levels::new(),
        }))
    }
}

/// What every page of one file shares: the bytes and the parsed document.
struct PdnFile {
    source: Arc<Source>,
    merged: ImageInfo,
    pdn: sp::Pdn,
}

struct PdnSession {
    file: Arc<PdnFile>,
    /// 0 for the composite, else layer page `page` (top-most layer first).
    page: u32,
    info: ImageInfo,
    levels: Levels,
}

impl PdnSession {
    /// The layer this page shows (index into the bottom-first layer list).
    fn layer(&self) -> Option<usize> {
        let n = self.file.pdn.layers().len();
        (self.page as usize)
            .checked_sub(1)
            .filter(|&i| i < n)
            .map(|i| n - 1 - i)
    }

    fn decode_full(&self, parallel: bool, cancel: &CancelToken) -> Result<Raster> {
        if cancel.is_cancelled() {
            return Err(DecodeError::Cancelled);
        }
        let stop = || cancel.is_cancelled();
        let opts = sp::Options {
            parallel,
            cancel: Some(&stop),
        };
        let data = self.file.source.bytes();
        let img = match self.layer() {
            None => self.file.pdn.render(data, &opts),
            Some(i) => self.file.pdn.render_layer(data, i, &opts),
        }
        .map_err(map_err)?;
        Ok(whole::interleaved(
            img.width,
            img.height,
            PixelLayout::Rgba,
            img.data,
            img.truncated,
        ))
    }
}

impl Session for PdnSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn source(&self) -> &Arc<Source> {
        &self.file.source
    }
    fn colour_model(&self) -> Option<&'static str> {
        Some("RGB")
    }
    fn alpha(&self) -> Option<Alpha> {
        Some(Alpha::Straight)
    }
    /// The header's thumbnail (composite page only).
    fn decode_preview(&self) -> Option<Result<Raster>> {
        if self.page != 0 {
            return None;
        }
        let png = self.file.pdn.header().thumbnail_png.clone()?;
        let src = Source::from_vec(self.file.source.path(), png);
        Some(
            PngDecoder
                .open(Arc::new(src))
                .and_then(|s| s.decode(&DecodeRequest::full(1), &CancelToken::new())),
        )
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        self.levels.decode(
            req,
            (self.info.width, self.info.height),
            |_| 1,
            |_| self.decode_full(req.parallel, cancel),
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
        let n = self.file.pdn.layers().len() as u32;
        if n <= 1 { 1 } else { 1 + n }
    }
    fn page_index(&self) -> u32 {
        self.page
    }
    fn page_label(&self) -> Option<PageLabel> {
        if self.page_count() == 1 {
            return None;
        }
        Some(
            match self.layer().and_then(|i| self.file.pdn.layers().get(i)) {
                None => PageLabel::Merged,
                Some(l) => PageLabel::DocumentLayer {
                    name: l.name.as_str().into(),
                    index: self.page,
                    hidden: !l.visible,
                },
            },
        )
    }
    /// Page 0 is the composite, pages 1‥ the layers top-most first; nothing is parsed again.
    fn open_page(&self, page: u32) -> Result<Arc<dyn Session>> {
        if page >= self.page_count() {
            return Err(DecodeError::Malformed("no such page".into()));
        }
        let merged = &self.file.merged;
        let info = if page == 0 {
            merged.clone()
        } else {
            ImageInfo {
                coding: format!(
                    "{}; layer {page} of {}",
                    merged.coding,
                    self.file.pdn.layers().len()
                ),
                ..merged.clone()
            }
        };
        Ok(Arc::new(PdnSession {
            file: self.file.clone(),
            page,
            info,
            levels: Levels::new(),
        }))
    }
}
