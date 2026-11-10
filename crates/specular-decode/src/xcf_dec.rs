//! GIMP XCF through the in-tree `specular-xcf` reader, plain or as `.xcf.gz`, `.xcf.bz2` or
//! `.xcf.xz` (decompressed once when the file is opened).
//!
//! XCF holds no merged image: the visible layer stack is composited (band-parallel) the way
//! GIMP's projection does, once, into 8-bit grey or RGBA for 8-bit perceptual images, else
//! 16-bit grey (+ alpha) or RGBA, or linear half floats for float images without a profile.
//! Coarser levels are box-filtered from it. The coding line names the version, precision,
//! tile compression, and the layers (count, groups, the first names). No thumbnail is used:
//! GIMP's is a small 8-bit parasite, and compositing is fast enough to show the real image.
//!
//! Pages: page 0 is the composited image, pages 1‥ the layers with pixels, top-most first as
//! GIMP's Layers dialog lists them (hidden ones too, labelled; groups are not pages). A layer
//! page is the layer alone on the canvas at its offsets, with its transparency, layer mask and
//! opacity; its mode, its groups and the visible channels are not applied. The decompressed
//! bytes and the parsed layer tree are shared by every page ([`Session::open_page`] neither
//! re-reads, re-decompresses nor re-parses); a page reads only its own layer's tiles and stops
//! between bands when cancelled.

use crate::whole::{self, Levels};
use crate::{
    Alpha, DecodeError, DecodeRequest, Decoder, Format, ImageInfo, PageLabel, Result, Session,
};
use specular_core::{
    CancelToken, HdrEncoding, Orientation, PixelLayout, Primaries, Raster, Source,
};
use specular_xcf as sx;
use std::sync::Arc;

pub struct XcfDecoder;

/// Layer names listed in the coding line before "…".
const NAMES_SHOWN: usize = 6;

fn map_err(e: sx::Error) -> DecodeError {
    match e {
        sx::Error::NotXcf => DecodeError::Malformed("not a GIMP XCF file".into()),
        sx::Error::Unsupported(s) => DecodeError::Unsupported(s),
        sx::Error::Truncated(s) => DecodeError::Truncated(s),
        sx::Error::Malformed(s) => DecodeError::Malformed(s),
        sx::Error::TooLarge => DecodeError::TooLarge,
        sx::Error::NeedsMemory(n) => DecodeError::NeedsMemory(n),
        sx::Error::Cancelled => DecodeError::Cancelled,
    }
}

fn plural(n: usize, one: &str) -> String {
    format!("{n} {one}{}", if n == 1 { "" } else { "s" })
}

fn coding(x: &sx::Xcf, container: sx::Container) -> String {
    let i = x.info();
    let p = i.precision;
    let mut s = format!(
        "XCF v{}, {}-bit {}{}",
        i.version,
        p.component.bits(),
        match p.component {
            sx::Component::Half | sx::Component::Float | sx::Component::Double => "float",
            _ => "integer",
        },
        if p.linear { " linear" } else { "" }
    );
    s += match i.compression {
        sx::Compression::None => ", uncompressed",
        sx::Compression::Rle => ", RLE",
        sx::Compression::Zlib => ", zlib",
    };
    s += match container {
        sx::Container::Gzip => " in gzip",
        sx::Container::Bzip2 => " in bzip2",
        sx::Container::Xz => " in xz",
        sx::Container::Raw => "",
    };
    s += &format!(", {}", plural(i.layer_count, "layer"));
    if i.group_count > 0 {
        s += &format!(" in {}", plural(i.group_count, "group"));
    }
    let names: Vec<&str> = x
        .layers()
        .iter()
        .filter(|l| !l.group)
        .map(|l| l.name.as_str())
        .collect();
    if !names.is_empty() {
        s += ": ";
        s += &names[..names.len().min(NAMES_SHOWN)].join(", ");
        if names.len() > NAMES_SHOWN {
            s += ", …";
        }
    }
    if !i.undrawn_filters.is_empty() {
        s += &format!(
            " (layer effects not drawn: {})",
            i.undrawn_filters.join(", ")
        );
    }
    s
}

impl Decoder for XcfDecoder {
    fn format(&self) -> Format {
        Format::Xcf
    }
    fn probe(&self, head: &[u8]) -> bool {
        sx::probe(head)
    }
    /// Also looks inside gzip, bzip2 and xz files (only the start is decompressed).
    fn probe_file(&self, data: &[u8]) -> bool {
        sx::probe_file(data)
    }
    fn extensions(&self) -> &'static [&'static str] {
        &[
            "xcf", "xcf.gz", "xcf.bz2", "xcf.xz", "xcfgz", "xcfbz2", "xcfxz",
        ]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let raw = source.bytes();
        let container = sx::container(raw);
        let unpacked = sx::decompress(raw).map_err(map_err)?;
        let data = unpacked.as_deref().unwrap_or(raw);
        let xcf = sx::parse(data).map_err(map_err)?;
        let i = xcf.info();
        let icc = xcf.icc(data).map(|b| Arc::new(b.to_vec()));
        let info = ImageInfo {
            format: Format::Xcf,
            width: i.width,
            height: i.height,
            orientation: Orientation::NORMAL,
            icc_profile: icc.clone(),
            has_alpha: i.has_alpha,
            bit_depth: i.precision.component.bits(),
            progressive: false,
            coding: coding(&xcf, container),
            embedded_preview: None,
            fast_regions: false,
            scales: &[1, 2, 4, 8],
        };
        let layers = xcf.pixel_layers().into();
        Ok(Arc::new(XcfSession {
            file: Arc::new(XcfFile {
                source,
                merged: info.clone(),
                icc,
                unpacked,
                xcf,
                layers,
            }),
            page: 0,
            info,
            levels: Levels::new(),
        }))
    }
}

/// What every page of one file shares: the bytes (decompressed once) and the parsed tree.
struct XcfFile {
    source: Arc<Source>,
    /// The composited image's facts; a layer page's are derived from them.
    merged: ImageInfo,
    icc: Option<Arc<Vec<u8>>>,
    /// The XCF inside a compressed container.
    unpacked: Option<Vec<u8>>,
    xcf: sx::Xcf,
    /// Indices into the layer list of the layer pages, page 1 first.
    layers: Arc<[usize]>,
}

struct XcfSession {
    file: Arc<XcfFile>,
    /// 0 for the composited image, else layer page `page` (`file.layers[page - 1]`).
    page: u32,
    info: ImageInfo,
    levels: Levels,
}

impl XcfSession {
    fn data(&self) -> &[u8] {
        self.file
            .unpacked
            .as_deref()
            .unwrap_or_else(|| self.file.source.bytes())
    }

    /// The layer this page shows (index into the layer list), None on the composite.
    fn layer(&self) -> Option<usize> {
        self.file
            .layers
            .get((self.page as usize).checked_sub(1)?)
            .copied()
    }

    /// The pixel format this page decodes to.
    fn format(&self) -> sx::PixelFormat {
        if self.page == 0 {
            self.file.xcf.output_format()
        } else {
            self.file.xcf.layer_format()
        }
    }

    fn layout(&self, f: sx::PixelFormat) -> PixelLayout {
        match f {
            sx::PixelFormat::Gray8 => PixelLayout::Gray,
            sx::PixelFormat::Rgba8 => PixelLayout::Rgba,
            sx::PixelFormat::Gray16 => PixelLayout::Gray16,
            sx::PixelFormat::GrayAlpha16 => PixelLayout::GrayAlpha16,
            // Linear-precision images with a profile stay linear (the profile's primaries).
            sx::PixelFormat::Rgba16 if self.linear_rgba16() => {
                PixelLayout::Rgba16(HdrEncoding::LINEAR)
            }
            sx::PixelFormat::Rgba16 => PixelLayout::Rgba16(HdrEncoding::SRGB),
            sx::PixelFormat::Rgba16F => PixelLayout::Rgba16F(Primaries::Bt709),
        }
    }

    fn linear_rgba16(&self) -> bool {
        let i = self.file.xcf.info();
        self.file.icc.is_some() && i.precision.linear && i.base != sx::BaseType::Indexed
    }

    fn decode_full(&self, parallel: bool, cancel: &CancelToken) -> Result<Raster> {
        if cancel.is_cancelled() {
            return Err(DecodeError::Cancelled);
        }
        let stop = || cancel.is_cancelled();
        let opts = sx::Options {
            parallel,
            cancel: Some(&stop),
        };
        let img = match self.layer() {
            None => self.file.xcf.render(self.data(), &opts),
            Some(i) => self.file.xcf.render_layer(self.data(), i, &opts),
        }
        .map_err(map_err)?;
        Ok(whole::interleaved(
            img.width,
            img.height,
            self.layout(img.format),
            img.data,
            img.truncated,
        ))
    }
}

impl Session for XcfSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn colour_model(&self) -> Option<&'static str> {
        Some(match self.file.xcf.info().base {
            sx::BaseType::Rgb => "RGB",
            sx::BaseType::Grey => "Greyscale",
            sx::BaseType::Indexed => "Palette",
        })
    }
    fn alpha(&self) -> Option<Alpha> {
        self.info.has_alpha.then_some(Alpha::Straight)
    }
    fn embedded_icc(&self) -> Option<&[u8]> {
        self.file.icc.as_deref().map(Vec::as_slice)
    }
    fn source(&self) -> &Arc<Source> {
        &self.file.source
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        None
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
        let bpp = self.format().bytes_per_pixel() as u64;
        let mut m = whole::level_bytes(w, h, s, bpp);
        if !self.levels.has(s) && !self.levels.has(1) {
            // The composite (or layer page), plus float bands of the layers (a fraction of
            // the canvas).
            m += whole::level_bytes(w, h, 1, bpp + 16);
        }
        m
    }
    fn memory_bytes(&self) -> usize {
        self.levels.memory_bytes()
    }
    /// Drops the cached levels (the next request composites the image again).
    fn trim_memory(&self) -> usize {
        self.levels.clear()
    }
    fn page_count(&self) -> u32 {
        if self.file.layers.is_empty() {
            1
        } else {
            1 + self.file.layers.len() as u32
        }
    }
    fn page_index(&self) -> u32 {
        self.page
    }
    fn page_label(&self) -> Option<PageLabel> {
        if self.file.layers.is_empty() {
            return None;
        }
        Some(
            match self.layer().and_then(|i| self.file.xcf.layers().get(i)) {
                None => PageLabel::Merged,
                Some(l) => PageLabel::DocumentLayer {
                    name: l.name.as_str().into(),
                    index: self.page,
                    hidden: !l.visible,
                },
            },
        )
    }
    /// Page 0 is the composited image, pages 1‥ the pixel layers top-most first: the
    /// decompressed bytes and parsed tree are shared, nothing is read or parsed again.
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
                // After the layer names the coding line lists.
                coding: format!(
                    "{}; layer {page} of {}",
                    merged.coding,
                    self.file.layers.len()
                ),
                ..merged.clone()
            }
        };
        Ok(Arc::new(XcfSession {
            file: self.file.clone(),
            page,
            info,
            levels: Levels::new(),
        }))
    }
}
