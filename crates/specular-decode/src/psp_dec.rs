//! PaintShop Pro `.psp`, `.pspimage` and `.tub` through the in-tree `specular-psp` reader.
//!
//! The block tree is parsed once at open. Page 0 is the image: the file's own composite when
//! it stores one (fast, and the only way to show vector and art media layers), else the
//! raster layers composited with their blend modes, opacity and masks. Pages 1‥ are the
//! layers with pixels, top-most first (hidden ones too, labelled), each alone on the canvas.
//! The thumbnail block is the embedded preview, which the gallery uses when it is big enough.

use crate::whole::{self, Levels};
use crate::{
    Alpha, DecodeError, DecodeRequest, Decoder, Format, ImageInfo, PageLabel, Result, Session,
};
use specular_core::{CancelToken, HdrEncoding, Orientation, PixelLayout, Raster, Source};
use specular_psp as sp;
use std::sync::Arc;

pub struct PspDecoder;

/// Layer names listed in the coding line before "…".
const NAMES_SHOWN: usize = 6;

fn map_err(e: sp::Error) -> DecodeError {
    match e {
        sp::Error::NotPsp => DecodeError::Malformed("not a PaintShop Pro file".into()),
        sp::Error::Unsupported(s) => DecodeError::Unsupported(s),
        sp::Error::Truncated(s) => DecodeError::Truncated(s),
        sp::Error::Malformed(s) => DecodeError::Malformed(s),
        sp::Error::TooLarge => DecodeError::TooLarge,
        sp::Error::NeedsMemory(n) => DecodeError::NeedsMemory(n),
        sp::Error::Cancelled => DecodeError::Cancelled,
    }
}

fn layout(f: sp::PixelFormat) -> PixelLayout {
    match f {
        sp::PixelFormat::Gray8 => PixelLayout::Gray,
        sp::PixelFormat::Rgba8 => PixelLayout::Rgba,
        sp::PixelFormat::Rgba16 => PixelLayout::Rgba16(HdrEncoding::SRGB),
    }
}

fn raster(img: sp::Image) -> Raster {
    whole::interleaved(
        img.width,
        img.height,
        layout(img.format),
        img.data,
        img.truncated,
    )
}

fn plural(n: usize, one: &str) -> String {
    format!("{n} {one}{}", if n == 1 { "" } else { "s" })
}

/// "PaintShop Pro file format 10.0, 24-bit, LZ77, 3 layers: …; vector layers are not drawn".
fn coding(p: &sp::Psp) -> String {
    let i = p.info();
    let mut s = format!("PaintShop Pro file format {}.{}", i.major, i.minor);
    if let Some(c) = i.creator.as_deref().filter(|c| !c.is_empty()) {
        s += &format!(" ({c})");
    }
    s += &match (i.greyscale, i.palette) {
        (true, _) => format!(", {}-bit grey", i.bit_depth),
        (false, true) => format!(", {}-bit palette", i.bit_depth),
        _ => format!(", {}-bit", i.bit_depth),
    };
    s += match i.compression {
        sp::Compression::None => ", uncompressed",
        sp::Compression::Rle => ", RLE",
        sp::Compression::Lz77 => ", LZ77",
        sp::Compression::Jpeg => ", JPEG",
    };
    if i.layer_count > 0 {
        s += &format!(", {}", plural(i.layer_count, "layer"));
        let names: Vec<&str> = p
            .layers()
            .iter()
            .filter(|l| l.kind != sp::LayerKind::Group)
            .map(|l| l.name.as_str())
            .collect();
        if !names.is_empty() {
            s += ": ";
            s += &names[..names.len().min(NAMES_SHOWN)].join(", ");
            if names.len() > NAMES_SHOWN {
                s += ", …";
            }
        }
    }
    if let Some(why) = &i.not_drawn {
        s += &format!("; {why}");
        if i.has_composite {
            s += " (the image is the file's stored composite)";
        }
    }
    s
}

impl Decoder for PspDecoder {
    fn format(&self) -> Format {
        Format::Psp
    }
    fn probe(&self, head: &[u8]) -> bool {
        sp::probe(head)
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["psp", "pspimage", "tub", "psptube", "pspframe", "pspmask"]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let psp = sp::parse(source.bytes()).map_err(map_err)?;
        let i = psp.info();
        let info = ImageInfo {
            format: Format::Psp,
            width: i.width,
            height: i.height,
            orientation: Orientation::NORMAL,
            icc_profile: None,
            has_alpha: !psp.layers().is_empty(),
            bit_depth: if i.bit_depth == 48 { 16 } else { 8 },
            progressive: false,
            coding: coding(&psp),
            embedded_preview: None,
            fast_regions: false,
            scales: &[1, 2, 4, 8],
        };
        let layers = psp.pixel_layers().into();
        Ok(Arc::new(PspSession {
            file: Arc::new(PspFile {
                source,
                merged: info.clone(),
                psp,
                layers,
            }),
            page: 0,
            info,
            levels: Levels::new(),
        }))
    }
}

struct PspFile {
    source: Arc<Source>,
    merged: ImageInfo,
    psp: sp::Psp,
    /// Indices into the layer list of the layer pages, page 1 first.
    layers: Arc<[usize]>,
}

struct PspSession {
    file: Arc<PspFile>,
    page: u32,
    info: ImageInfo,
    levels: Levels,
}

impl PspSession {
    fn layer(&self) -> Option<usize> {
        self.file
            .layers
            .get((self.page as usize).checked_sub(1)?)
            .copied()
    }

    fn decode_full(&self, parallel: bool, cancel: &CancelToken) -> Result<Raster> {
        if cancel.is_cancelled() {
            return Err(DecodeError::Cancelled);
        }
        let stop = || cancel.is_cancelled();
        let opts = sp::Options {
            parallel,
            cancel: Some(&stop),
            prefer_composite: true,
        };
        let data = self.file.source.bytes();
        let img = match self.layer() {
            None => self.file.psp.render(data, &opts),
            Some(i) => self.file.psp.render_layer(data, i, &opts),
        }
        .map_err(map_err)?;
        Ok(raster(img))
    }
}

impl Session for PspSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn source(&self) -> &Arc<Source> {
        &self.file.source
    }
    fn colour_model(&self) -> Option<&'static str> {
        let i = self.file.psp.info();
        Some(if i.greyscale {
            "Greyscale"
        } else if i.palette {
            "Palette"
        } else {
            "RGB"
        })
    }
    fn alpha(&self) -> Option<Alpha> {
        self.info.has_alpha.then_some(Alpha::Straight)
    }
    /// The thumbnail block (page 0 only).
    fn decode_preview(&self) -> Option<Result<Raster>> {
        if self.page != 0 {
            return None;
        }
        self.file
            .psp
            .thumbnail(self.file.source.bytes())
            .map(|r| r.map(raster).map_err(map_err))
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
        let bpp = if self.info.bit_depth > 8 { 8 } else { 4 };
        let s = req.scale.max(1);
        let mut m = whole::level_bytes(w, h, s, bpp);
        if !self.levels.has(s) && !self.levels.has(1) {
            // The output and one layer's channels at a time.
            m += whole::level_bytes(w, h, 1, 2 * bpp);
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
        // A single layer would be a page equal to the image.
        if self.file.layers.len() <= 1 {
            1
        } else {
            1 + self.file.layers.len() as u32
        }
    }
    fn page_index(&self) -> u32 {
        self.page
    }
    fn page_label(&self) -> Option<PageLabel> {
        if self.page_count() == 1 {
            return None;
        }
        Some(
            match self.layer().and_then(|i| self.file.psp.layers().get(i)) {
                None => PageLabel::Merged,
                Some(l) => PageLabel::DocumentLayer {
                    name: l.name.as_str().into(),
                    index: self.page,
                    hidden: !l.visible,
                },
            },
        )
    }
    fn open_page(&self, page: u32) -> Result<Arc<dyn Session>> {
        if page >= self.page_count() {
            return Err(DecodeError::Malformed("no such page".into()));
        }
        let merged = &self.file.merged;
        let info = if page == 0 {
            merged.clone()
        } else {
            ImageInfo {
                has_alpha: true,
                coding: format!(
                    "{}; layer {page} of {}",
                    merged.coding,
                    self.file.layers.len()
                ),
                ..merged.clone()
            }
        };
        Ok(Arc::new(PspSession {
            file: self.file.clone(),
            page,
            info,
            levels: Levels::new(),
        }))
    }
}
