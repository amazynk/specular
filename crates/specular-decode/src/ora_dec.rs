//! OpenRaster (`.ora`) and Krita (`.kra`) documents through the in-tree `specular-ora`
//! reader: their flattened `mergedimage.png`, decoded by the PNG decoder.
//!
//! Opening reads only the ZIP directory and the start of the merged PNG (its header chunks,
//! inflated as a prefix), so the size, ICC profile and depth are known without inflating
//! the image. A stored member is decoded in place; a deflated one is inflated on the first
//! decode and dropped again by [`Session::trim_memory`]. The small preview (`Thumbnails/
//! thumbnail.png`, Krita's `preview.png`) is [`Session::decode_preview`].
//!
//! A Krita file without a usable merged image is composited from its layer tree by
//! `specular-ora` (paint and group layers, transparency masks, Krita's blend modes but a
//! few, `invert` filter layers; what it leaves out is listed in the coding), else it shows
//! its preview, marked "embedded preview only" like a camera RAW's. An OpenRaster file
//! without one is composited from its PNG layers with the specification's blend modes and
//! compositing operators.
//!
//! Pages (OpenRaster): page 0 is the merged (or composited) image, pages 1‥ the PNG layers of
//! `stack.xml`, top-most first, hidden ones too (labelled), each alone on the canvas at its
//! offset with its alpha times its opacity; blend modes are not applied, stacks are not pages.
//! The ZIP directory and the layer list are parsed once at open and shared by every page; a
//! layer page inflates and decodes only its own member.
//!
//! Pages (Krita): page 0 is the merged image (or the composite, or the preview), pages 1‥ the
//! paint and group layers of `maindoc.xml` in the Layers docker's order, each alone on the
//! canvas with its transparency masks and opacity (blend mode not applied): a paint layer
//! decoded from Krita's tiled LZF layer data over its default pixel, a group its children
//! composited. Vector, filter, fill, clone and file layers and masks are not pages. 8-bit
//! RGBA comes out as RGBA8, 16-bit and gamma-coded float as 16-bit RGBA in the layer's ICC
//! profile, linear float as half floats, grey as 16-bit grey and alpha.

use crate::whole::{self, Levels};
use crate::{
    Alpha, DecodeError, DecodeRequest, Decoder, Format, ImageInfo, PageLabel, Result, Session,
};
use parking_lot::Mutex;
use specular_core::{
    CancelToken, HdrEncoding, Orientation, PixelLayout, Primaries, Raster, Source,
};
use specular_ora as so;
use std::cell::Cell;
use std::path::PathBuf;
use std::sync::Arc;

/// What a Krita composite needs from the viewer: vector layers drawn with the SVG renderer,
/// file layers read from beside the document (the folder rules of external references:
/// that folder and below, or a file the user opened).
pub(crate) struct KraHost {
    doc: PathBuf,
}

/// Largest file a file layer reads.
const MAX_FILE_LAYER: u64 = 1 << 30;

thread_local! {
    /// File layers being read on this thread (a file layer of a document whose file layer
    /// is the first one).
    static FILE_DEPTH: Cell<u32> = const { Cell::new(0) };
}

impl KraHost {
    pub(crate) fn new(source: &Source) -> KraHost {
        KraHost {
            doc: source.path().to_path_buf(),
        }
    }

    fn find(&self, source: &str) -> std::result::Result<PathBuf, String> {
        // Krita stores paths with forward slashes, relative to the document.
        let name = source.replace('\\', "/");
        crate::hdf5_dec::resolve_link(&self.doc, &name).map_err(|r| match r {
            crate::hdf5_dec::LinkRefusal::Missing => {
                format!("{source} not found beside the document")
            }
            crate::hdf5_dec::LinkRefusal::Outside => {
                format!("{source} is outside the document's folder (open it once to allow it)")
            }
        })
    }
}

impl so::KraHost for KraHost {
    fn svg(&self, svg: &[u8], w: u32, h: u32) -> Option<Vec<u8>> {
        crate::svg::render_canvas(svg, w, h)
    }

    fn file(&self, source: &str) -> std::result::Result<(u32, u32, Vec<u8>), String> {
        let p = self.find(source)?;
        if std::fs::metadata(&p).map_or(0, |m| m.len()) > MAX_FILE_LAYER {
            return Err(format!("{source} is too large to read as a file layer"));
        }
        if FILE_DEPTH.with(Cell::get) >= 4 {
            return Err(format!("{source}: file layers nested too deeply"));
        }
        let data = std::fs::read(&p).map_err(|_| format!("{source} could not be read"))?;
        FILE_DEPTH.with(|d| d.set(d.get() + 1));
        let img = crate::svg::decode_rgba(&data);
        FILE_DEPTH.with(|d| d.set(d.get() - 1));
        img.ok_or_else(|| format!("{source} could not be decoded"))
    }

    fn locate(&self, source: &str) -> std::result::Result<(), String> {
        self.find(source).map(|_| ())
    }
}

/// Prefix of a deflated merged image inflated first for its header chunks; grown 4× until
/// the first IDAT shows (PNG writers put large iCCP / text chunks first at most).
const HEADER_PREFIX: usize = 64 << 10;
const HEADER_PREFIX_MAX: usize = 16 << 20;
pub struct OraDecoder(pub so::Kind);

fn map_err(e: so::Error) -> DecodeError {
    match e {
        so::Error::NotDocument => DecodeError::UnsupportedFormat,
        so::Error::Truncated(what) => DecodeError::Truncated(what.into()),
        so::Error::Unsupported(what) => DecodeError::Unsupported(what),
        so::Error::TooLarge(_) => DecodeError::TooLarge,
        so::Error::Cancelled => DecodeError::Cancelled,
        e @ so::Error::NoImage(_) => DecodeError::Unsupported(e.to_string()),
        e => DecodeError::Malformed(e.to_string()),
    }
}

impl Decoder for OraDecoder {
    fn format(&self) -> Format {
        match self.0 {
            so::Kind::Ora => Format::Ora,
            so::Kind::Kra => Format::Kra,
        }
    }
    fn probe(&self, head: &[u8]) -> bool {
        so::detect(head) == Some(self.0)
    }
    fn probe_file(&self, data: &[u8]) -> bool {
        so::detect_file(data) == Some(self.0)
    }
    fn extensions(&self) -> &'static [&'static str] {
        match self.0 {
            so::Kind::Ora => &["ora"],
            // `.krz`: Krita's archive (a `.kra` without the layer cache).
            so::Kind::Kra => &["kra", "krz"],
        }
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let doc = so::Document::open(source.bytes()).map_err(map_err)?;
        open_first(source, Arc::new(doc), self.format())
    }
}

/// Page 0 of a parsed document: the merged image, Krita's preview standing in for it, or the
/// OpenRaster layers composited.
fn open_first(
    source: Arc<Source>,
    doc: Arc<so::Document>,
    format: Format,
) -> Result<Arc<dyn Session>> {
    let data = source.bytes();
    let canvas = doc
        .canvas
        .map_or(String::new(), |(w, h)| format!(", canvas {w}×{h}"));
    let preview = doc.preview().cloned();
    let composite = doc.kind == so::Kind::Kra && doc.kra_can_composite();
    if let Some(merged) = doc.merged().cloned() {
        match header_session(&source, &doc, &merged) {
            Ok((header, full)) => {
                let info = wrap_info(&header, format, format!("merged image{canvas}"));
                return Ok(Arc::new(OraSession {
                    source,
                    info,
                    doc,
                    shown: Shown::Member {
                        entry: merged,
                        header,
                        full: Mutex::new(full),
                    },
                    preview,
                    want: Mutex::new(None),
                    levels: Levels::new(),
                    page: 0,
                }));
            }
            // A damaged merged image of a Krita file: its layers instead.
            Err(_) if composite => {}
            Err(e) => return Err(e),
        }
    }
    if composite {
        return Ok(kra_composite_session(source, doc, format, preview));
    }
    if doc.kind == so::Kind::Kra {
        let Some(p) = preview else {
            return Err(map_err(so::Error::NoImage(so::Kind::Kra)));
        };
        let (header, full) = header_session(&source, &doc, &p)?;
        let info = wrap_info(
            &header,
            format,
            format!("embedded preview only (no merged image){canvas}"),
        );
        return Ok(Arc::new(OraSession {
            source,
            info,
            doc,
            shown: Shown::Member {
                entry: p,
                header,
                full: Mutex::new(full),
            },
            preview: None,
            want: Mutex::new(None),
            levels: Levels::new(),
            page: 0,
        }));
    }
    // OpenRaster without a merged image: the layers, flattened on first decode.
    let stack = doc.stack(data).map_err(map_err)?;
    let px = u64::from(stack.width) * u64::from(stack.height);
    if px > so::MAX_CANVAS_PIXELS {
        return Err(DecodeError::TooLarge);
    }
    let info = ImageInfo {
        format,
        width: stack.width,
        height: stack.height,
        orientation: Orientation::NORMAL,
        icc_profile: None,
        has_alpha: true,
        bit_depth: 8,
        progressive: false,
        coding: "layers composited, no merged image".into(),
        embedded_preview: None,
        fast_regions: false,
        scales: &[1, 2, 4, 8],
    };
    Ok(Arc::new(OraSession {
        source,
        info,
        doc,
        shown: Shown::Composite,
        preview,
        want: Mutex::new(None),
        levels: Levels::new(),
        page: 0,
    }))
}

fn wrap_info(header: &Arc<dyn Session>, format: Format, what: String) -> ImageInfo {
    let mut i = header.info().clone();
    i.format = format;
    i.coding = format!("PNG {}, {what}", i.coding);
    i.fast_regions = false;
    i
}

/// A PNG session over the member's header chunks, and the full session when that came for
/// free (a stored member, or one whole member that was small enough to inflate at once).
fn header_session(
    source: &Arc<Source>,
    doc: &so::Document,
    e: &so::Entry,
) -> Result<(Arc<dyn Session>, FullSession)> {
    let data = source.bytes();
    if let Some(r) = doc.stored_range(data, e).map_err(map_err)? {
        let sub = Source::from_slice(source.path(), source.clone(), &data[r])
            .ok_or(DecodeError::Malformed("ZIP member outside the file".into()))?;
        let s = crate::png_dec::PngDecoder.open(Arc::new(sub))?;
        return Ok((s.clone(), Some(s)));
    }
    let mut n = HEADER_PREFIX;
    loop {
        let prefix = doc.read_prefix(data, e, n).map_err(map_err)?;
        if prefix.len() as u64 >= e.size {
            // The whole member.
            let s = open_member(source, prefix.into_owned())?;
            return Ok((s.clone(), Some(s)));
        }
        if specular_png::read_info(&prefix).is_ok() {
            let sub = Source::from_entry(source.path(), source, prefix.into_owned());
            return Ok((crate::png_dec::PngDecoder.open(Arc::new(sub))?, None));
        }
        if n >= HEADER_PREFIX_MAX || prefix.len() < n {
            let whole = doc.read(data, e).map_err(map_err)?;
            let s = open_member(source, whole.into_owned())?;
            return Ok((s.clone(), Some(s)));
        }
        n *= 4;
    }
}

type FullSession = Option<Arc<dyn Session>>;

fn open_member(source: &Arc<Source>, bytes: Vec<u8>) -> Result<Arc<dyn Session>> {
    crate::png_dec::PngDecoder.open(Arc::new(Source::from_entry(source.path(), source, bytes)))
}

enum Shown {
    /// A PNG member: `mergedimage.png`, or a Krita `preview.png` standing in for it.
    Member {
        entry: so::Entry,
        /// Over the header chunks (or the whole member): info, colour model, estimates.
        header: Arc<dyn Session>,
        /// Over the whole member; dropped by `trim_memory` when it holds inflated bytes.
        full: Mutex<Option<Arc<dyn Session>>>,
    },
    /// OpenRaster layers flattened into [`OraSession::levels`].
    Composite,
    /// Layer page: `doc.layers[i]` alone on the canvas, in [`OraSession::levels`].
    Layer(usize),
    /// Krita layer page: `doc.kra_layers[i]` alone on the canvas, in [`OraSession::levels`].
    KraLayer(usize),
    /// Krita's layer tree composited, in [`OraSession::levels`].
    KraComposite,
}

struct OraSession {
    source: Arc<Source>,
    info: ImageInfo,
    /// Shared by every page of the file.
    doc: Arc<so::Document>,
    shown: Shown,
    /// The small preview, when it is not the shown image.
    preview: Option<so::Entry>,
    /// Levels announced before the full session existed.
    want: Mutex<Option<Vec<u32>>>,
    levels: Levels,
    /// 0 for the merged image, else layer page `page` (`doc.layers[page - 1]`).
    page: u32,
}

impl OraSession {
    /// Layer pages: OpenRaster or Krita layers on a canvas of known size within the limits.
    fn layer_pages(&self) -> u32 {
        match self.doc.canvas {
            Some((w, h))
                if w > 0 && h > 0 && u64::from(w) * u64::from(h) <= so::MAX_CANVAS_PIXELS =>
            {
                match self.doc.kind {
                    so::Kind::Ora => self.doc.layers.len() as u32,
                    so::Kind::Kra => self.doc.kra_layers.len() as u32,
                }
            }
            _ => 0,
        }
    }

    /// A whole-canvas result of the layers or one layer, cached in the levels.
    fn decode_canvas(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        self.levels.decode(
            req,
            (self.info.width, self.info.height),
            |_| 1,
            |_| {
                let data = self.source.bytes();
                let stop = || cancel.is_cancelled();
                if let Shown::KraLayer(_) | Shown::KraComposite = &self.shown {
                    let img = match &self.shown {
                        Shown::KraLayer(i) => self.doc.kra_layer_page_with(
                            data,
                            &self.doc.kra_layers[*i],
                            Some(&KraHost::new(&self.source)),
                            &stop,
                        ),
                        _ => self.doc.kra_composite_with(
                            data,
                            Some(&KraHost::new(&self.source)),
                            &stop,
                        ),
                    }
                    .map_err(map_err)?;
                    return Ok(whole::interleaved(
                        img.width,
                        img.height,
                        kra_layout(img.format, &self.doc.kra_profile),
                        img.data,
                        img.truncated,
                    ));
                }
                let img = match &self.shown {
                    Shown::Layer(i) => self.doc.layer_page(data, &self.doc.layers[*i], &stop),
                    _ => self.doc.composite(data, &stop),
                }
                .map_err(map_err)?;
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

    /// The full-member session, inflating the member on first use.
    fn full(&self) -> Result<Arc<dyn Session>> {
        let Shown::Member { entry, full, .. } = &self.shown else {
            unreachable!("composite sessions have no member");
        };
        let mut g = full.lock();
        if let Some(s) = g.as_ref() {
            return Ok(s.clone());
        }
        let bytes = self.doc.read(self.source.bytes(), entry).map_err(map_err)?;
        let s = open_member(&self.source, bytes.into_owned())?;
        if let Some(w) = self.want.lock().as_deref() {
            s.want_levels(w);
        }
        *g = Some(s.clone());
        Ok(s)
    }

    /// Whether the full session holds inflated bytes of its own (not a slice of the file).
    fn owns_bytes(&self) -> bool {
        matches!(&self.shown, Shown::Member { entry, .. } if !entry.is_stored())
    }
}

impl OraSession {
    /// Krita layer page `page`, `doc.kra_layers[i]`.
    fn kra_page(&self, page: u32, i: usize, width: u32, height: u32) -> Arc<dyn Session> {
        let l = &self.doc.kra_layers[i];
        let what = if l.group {
            "group layer"
        } else {
            "paint layer"
        };
        let coding = format!(
            "Krita {} {what}; layer {page} of {}{}",
            kra_pixel_name(l.pixel),
            self.layer_pages(),
            notes(self.doc.kra_notes(
                self.source.bytes(),
                Some(l.node),
                Some(&KraHost::new(&self.source))
            ))
        );
        let info = kra_info(
            &self.doc,
            self.source.bytes(),
            self.info.format,
            (width, height),
            l.pixel,
            Some(l),
            coding,
        );
        Arc::new(OraSession {
            source: self.source.clone(),
            info,
            doc: self.doc.clone(),
            shown: Shown::KraLayer(i),
            preview: None,
            want: Mutex::new(None),
            levels: Levels::new(),
            page,
        })
    }
}

/// Page 0 of a Krita file without a usable merged image: its layers composited.
fn kra_composite_session(
    source: Arc<Source>,
    doc: Arc<so::Document>,
    format: Format,
    preview: Option<so::Entry>,
) -> Arc<dyn Session> {
    let (pixel, size) = match (doc.kra_pixel, doc.canvas) {
        (Some(p), Some(s)) => (p, s),
        _ => unreachable!("kra_can_composite checked the colour model and canvas"),
    };
    let coding = format!(
        "Krita {} layers composited (no merged image){}",
        kra_pixel_name(pixel),
        notes(doc.kra_notes(source.bytes(), None, Some(&KraHost::new(&source))))
    );
    let info = kra_info(&doc, source.bytes(), format, size, pixel, None, coding);
    Arc::new(OraSession {
        source,
        info,
        doc,
        shown: Shown::KraComposite,
        preview,
        want: Mutex::new(None),
        levels: Levels::new(),
        page: 0,
    })
}

/// "; " and what a composite leaves out, if anything.
fn notes(n: Vec<String>) -> String {
    if n.is_empty() {
        String::new()
    } else {
        format!("; {}", n.join("; "))
    }
}

/// The info of Krita pixels of `pixel` on the canvas: a layer (`layer`) or the composite.
fn kra_info(
    doc: &so::Document,
    data: &[u8],
    format: Format,
    (width, height): (u32, u32),
    pixel: so::KraPixel,
    layer: Option<&so::KraLayer>,
    coding: String,
) -> ImageInfo {
    // Linear float pixels go out as scene-linear halves; the rest are as coded, in the
    // layer's (or the document's) profile.
    let linear = match layer {
        Some(l) => l.linear,
        None => doc.kra_output() == Some(so::KraOutput::Rgba16F),
    };
    let icc_profile = if pixel.is_float() && linear {
        None
    } else {
        doc.kra_layer_icc(data, layer).map(Arc::new)
    };
    let depth = match pixel {
        so::KraPixel::Rgba8 | so::KraPixel::GrayA8 => 8,
        so::KraPixel::Rgba16
        | so::KraPixel::GrayA16
        | so::KraPixel::RgbaF16
        | so::KraPixel::GrayAF16 => 16,
        so::KraPixel::RgbaF32 | so::KraPixel::GrayAF32 => 32,
    };
    ImageInfo {
        format,
        width,
        height,
        orientation: Orientation::NORMAL,
        icc_profile,
        has_alpha: true,
        bit_depth: depth,
        progressive: false,
        coding,
        embedded_preview: None,
        fast_regions: false,
        scales: &[1, 2, 4, 8],
    }
}

/// Krita's colour model and depth of a layer's pixels.
fn kra_pixel_name(p: so::KraPixel) -> &'static str {
    match p {
        so::KraPixel::Rgba8 => "8-bit RGBA",
        so::KraPixel::Rgba16 => "16-bit RGBA",
        so::KraPixel::RgbaF16 => "16-bit float RGBA",
        so::KraPixel::RgbaF32 => "32-bit float RGBA",
        so::KraPixel::GrayA8 => "8-bit grey",
        so::KraPixel::GrayA16 => "16-bit grey",
        so::KraPixel::GrayAF16 => "16-bit float grey",
        so::KraPixel::GrayAF32 => "32-bit float grey",
    }
}

/// The raster layout of a decoded Krita layer; linear halves take their primaries from the
/// document profile's name (Krita's built-in profiles name them: `…Rec2020…`, `…P3…`, else
/// sRGB's).
fn kra_layout(out: so::KraOutput, profile: &str) -> PixelLayout {
    match out {
        so::KraOutput::Rgba8 => PixelLayout::Rgba,
        so::KraOutput::Rgba16 => PixelLayout::Rgba16(HdrEncoding::SRGB),
        so::KraOutput::GrayAlpha16 => PixelLayout::GrayAlpha16,
        so::KraOutput::Rgba16F => {
            let p = profile.to_ascii_lowercase();
            PixelLayout::Rgba16F(if p.contains("2020") {
                Primaries::Bt2020
            } else if p.contains("p3") {
                Primaries::DisplayP3
            } else {
                Primaries::Bt709
            })
        }
    }
}

impl Session for OraSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn source(&self) -> &Arc<Source> {
        &self.source
    }
    fn colour_model(&self) -> Option<&'static str> {
        match &self.shown {
            Shown::Member { header, .. } => header.colour_model(),
            Shown::Composite | Shown::Layer(_) => Some("RGB"),
            Shown::KraLayer(i) if self.doc.kra_layers[*i].pixel.is_gray() => Some("Greyscale"),
            Shown::KraLayer(_) => Some("RGB"),
            Shown::KraComposite if self.doc.kra_pixel.is_some_and(|p| p.is_gray()) => {
                Some("Greyscale")
            }
            Shown::KraComposite => Some("RGB"),
        }
    }
    fn alpha(&self) -> Option<Alpha> {
        match &self.shown {
            Shown::Member { header, .. } => header.alpha(),
            Shown::Composite | Shown::Layer(_) | Shown::KraLayer(_) | Shown::KraComposite => {
                Some(Alpha::Straight)
            }
        }
    }
    /// The document's small preview (OpenRaster's at most 256×256 thumbnail, Krita's
    /// `preview.png`); layer pages have none.
    fn decode_preview(&self) -> Option<Result<Raster>> {
        if self.page > 0 {
            return None;
        }
        let e = self.preview.as_ref()?;
        Some((|| {
            let bytes = self.doc.read(self.source.bytes(), e).map_err(map_err)?;
            let s = open_member(&self.source, bytes.into_owned())?;
            s.decode(&DecodeRequest::full(1), &CancelToken::new())
        })())
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        if let Shown::Member { .. } = &self.shown {
            return self.full()?.decode(req, cancel);
        }
        self.decode_canvas(req, cancel)
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        match &self.shown {
            Shown::Member {
                entry,
                header,
                full,
            } => {
                let inflate = if full.lock().is_none() { entry.size } else { 0 };
                header.decode_memory(req) + inflate
            }
            Shown::Composite => {
                let (w, h) = (self.info.width, self.info.height);
                // The level, the result, the 16-bit canvas and a decoded layer.
                whole::level_bytes(w, h, req.scale.max(1), 4) + whole::level_bytes(w, h, 1, 16)
            }
            Shown::Layer(i) => {
                let (w, h) = (self.info.width, self.info.height);
                let mut m = whole::level_bytes(w, h, req.scale.max(1), 4);
                if !self.levels.has(1) {
                    // The canvas, the decoded PNG (at most the canvas when it fits) and its
                    // inflated member.
                    let member = self
                        .doc
                        .zip
                        .find(&self.doc.layers[*i].src)
                        .map_or(0, |e| e.size.min(u64::from(w) * u64::from(h) * 8));
                    m += whole::level_bytes(w, h, 1, 8) + member;
                }
                m
            }
            Shown::KraLayer(_) | Shown::KraComposite => {
                let (w, h) = (self.info.width, self.info.height);
                let (out, node) = match &self.shown {
                    Shown::KraLayer(i) => {
                        let l = &self.doc.kra_layers[*i];
                        (Some(l.output()), Some(l.node))
                    }
                    _ => (self.doc.kra_output(), None),
                };
                let px = out.map_or(8, |o| o.bytes() as u64);
                let mut m = whole::level_bytes(w, h, req.scale.max(1), px);
                if !self.levels.has(1) {
                    // The canvas and the inflated members (tiles decode in place; composites
                    // work in 64-row float bands).
                    let members = match &self.shown {
                        Shown::KraLayer(i) if !self.doc.kra_layers[*i].group => {
                            let l = &self.doc.kra_layers[*i];
                            let own = self.doc.zip.find(&l.member).map_or(0, |e| e.size);
                            own + l
                                .masks
                                .iter()
                                .filter_map(|m| self.doc.zip.find(&m.member))
                                .map(|e| e.size)
                                .sum::<u64>()
                        }
                        _ => self.doc.kra_member_bytes(node),
                    };
                    m += whole::level_bytes(w, h, 1, px) + members;
                }
                m
            }
        }
    }
    fn open_memory(&self, levels: &[u32]) -> u64 {
        match &self.shown {
            Shown::Member {
                entry,
                header,
                full,
            } => {
                let inflate = if full.lock().is_none() { entry.size } else { 0 };
                header.open_memory(levels) + inflate
            }
            Shown::Composite | Shown::Layer(_) | Shown::KraLayer(_) | Shown::KraComposite => levels
                .iter()
                .map(|&s| self.decode_memory(&DecodeRequest::full(s)))
                .sum(),
        }
    }
    fn memory_bytes(&self) -> usize {
        match &self.shown {
            Shown::Member { full, entry, .. } => full.lock().as_ref().map_or(0, |s| {
                s.memory_bytes()
                    + if self.owns_bytes() {
                        entry.size as usize
                    } else {
                        0
                    }
            }),
            Shown::Composite | Shown::Layer(_) | Shown::KraLayer(_) | Shown::KraComposite => {
                self.levels.memory_bytes()
            }
        }
    }
    /// Drops the inflated merged image with its session's levels (a stored one keeps its
    /// session and drops only the levels), or the composited levels.
    fn trim_memory(&self) -> usize {
        match &self.shown {
            Shown::Member { full, entry, .. } => {
                let mut g = full.lock();
                let Some(s) = g.as_ref() else { return 0 };
                let mut freed = s.trim_memory();
                if self.owns_bytes() {
                    freed += entry.size as usize;
                    *g = None;
                }
                freed
            }
            Shown::Composite | Shown::Layer(_) | Shown::KraLayer(_) | Shown::KraComposite => {
                self.levels.clear()
            }
        }
    }
    fn want_levels(&self, scales: &[u32]) {
        if let Shown::Member { full, .. } = &self.shown {
            *self.want.lock() = Some(scales.to_vec());
            if let Some(s) = full.lock().as_ref() {
                s.want_levels(scales);
            }
        }
    }
    fn page_count(&self) -> u32 {
        1 + self.layer_pages()
    }
    fn page_index(&self) -> u32 {
        self.page
    }
    fn page_label(&self) -> Option<PageLabel> {
        if self.layer_pages() == 0 {
            return None;
        }
        Some(match &self.shown {
            Shown::Layer(i) => {
                let l = &self.doc.layers[*i];
                PageLabel::DocumentLayer {
                    name: l.name.as_str().into(),
                    index: self.page,
                    hidden: !l.visible,
                }
            }
            Shown::KraLayer(i) => {
                let l = &self.doc.kra_layers[*i];
                PageLabel::DocumentLayer {
                    name: l.name.as_str().into(),
                    index: self.page,
                    hidden: !l.visible,
                }
            }
            _ => PageLabel::Merged,
        })
    }
    /// Page 0 is the merged image, pages 1‥ the PNG (OpenRaster) or paint and group (Krita)
    /// layers top-most first, from the ZIP directory and layer list parsed at open (page 0 parses
    /// its PNG's header again).
    fn open_page(&self, page: u32) -> Result<Arc<dyn Session>> {
        if page >= self.page_count() {
            return Err(DecodeError::Malformed("no such page".into()));
        }
        let format = self.info.format;
        if page == 0 {
            return open_first(self.source.clone(), self.doc.clone(), format);
        }
        let i = page as usize - 1;
        let (width, height) = self.doc.canvas.unwrap_or_default();
        if self.doc.kind == so::Kind::Kra {
            return Ok(self.kra_page(page, i, width, height));
        }
        let info = ImageInfo {
            format,
            width,
            height,
            orientation: Orientation::NORMAL,
            icc_profile: None,
            has_alpha: true,
            bit_depth: 8,
            progressive: false,
            coding: format!(
                "PNG layer {}; layer {page} of {}",
                self.doc.layers[i].src,
                self.layer_pages()
            ),
            embedded_preview: None,
            fast_regions: false,
            scales: &[1, 2, 4, 8],
        };
        Ok(Arc::new(OraSession {
            source: self.source.clone(),
            info,
            doc: self.doc.clone(),
            shown: Shown::Layer(i),
            preview: None,
            want: Mutex::new(None),
            levels: Levels::new(),
            page,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trim_check::assert_trims;

    /// A minimal ZIP: stored `mimetype` first, then the members (deflate = stored here; the
    /// integration tests cover deflated members).
    fn zip(members: &[(&str, &[u8])]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut cd = Vec::new();
        for (name, data) in members {
            let off = out.len() as u32;
            let crc = 0u32;
            let mut h = Vec::new();
            h.extend_from_slice(b"PK\x03\x04");
            h.extend_from_slice(&[20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
            h.extend_from_slice(&crc.to_le_bytes());
            h.extend_from_slice(&(data.len() as u32).to_le_bytes());
            h.extend_from_slice(&(data.len() as u32).to_le_bytes());
            h.extend_from_slice(&(name.len() as u16).to_le_bytes());
            h.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(&h);
            out.extend_from_slice(name.as_bytes());
            out.extend_from_slice(data);
            cd.extend_from_slice(b"PK\x01\x02");
            cd.extend_from_slice(&[20, 0, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
            cd.extend_from_slice(&crc.to_le_bytes());
            cd.extend_from_slice(&(data.len() as u32).to_le_bytes());
            cd.extend_from_slice(&(data.len() as u32).to_le_bytes());
            cd.extend_from_slice(&(name.len() as u16).to_le_bytes());
            cd.extend_from_slice(&[0; 12]);
            cd.extend_from_slice(&off.to_le_bytes());
            cd.extend_from_slice(name.as_bytes());
        }
        let cd_off = out.len() as u32;
        out.extend_from_slice(&cd);
        out.extend_from_slice(b"PK\x05\x06\0\0\0\0");
        out.extend_from_slice(&(members.len() as u16).to_le_bytes());
        out.extend_from_slice(&(members.len() as u16).to_le_bytes());
        out.extend_from_slice(&(cd.len() as u32).to_le_bytes());
        out.extend_from_slice(&cd_off.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out
    }

    fn png(w: u32, h: u32, rgba: [u8; 4]) -> Vec<u8> {
        let px: Vec<u8> = (0..w * h).flat_map(|_| rgba).collect();
        specular_png::encode(w, h, specular_png::ColorType::Rgba, &px).unwrap()
    }

    #[test]
    fn stored_merged_image_decodes_and_trims() {
        let merged = png(6, 4, [10, 20, 30, 255]);
        let f = zip(&[
            ("mimetype", b"image/openraster"),
            ("stack.xml", b"<image w=\"6\" h=\"4\"><stack/></image>"),
            ("mergedimage.png", &merged),
        ]);
        let s = OraDecoder(so::Kind::Ora)
            .open(Arc::new(Source::from_vec("t.ora", f)))
            .unwrap();
        assert_eq!(
            (s.info().width, s.info().height, s.info().format),
            (6, 4, Format::Ora)
        );
        let r = s
            .decode(&DecodeRequest::full(1), &CancelToken::new())
            .unwrap();
        assert_eq!(r.width, 6);
        // The PNG session caches only the level regions are cropped from.
        let region = DecodeRequest {
            region: Some(crate::Rect {
                x: 1,
                y: 1,
                w: 2,
                h: 2,
            }),
            ..DecodeRequest::full(1)
        };
        assert_trims(s.as_ref(), &region);
    }

    #[test]
    fn krita_preview_stands_in() {
        let preview = png(3, 2, [1, 2, 3, 255]);
        let f = zip(&[
            ("mimetype", b"application/x-krita"),
            (
                "maindoc.xml",
                b"<DOC><IMAGE width=\"300\" height=\"200\"/></DOC>",
            ),
            ("preview.png", &preview),
        ]);
        let s = OraDecoder(so::Kind::Kra)
            .open(Arc::new(Source::from_vec("t.kra", f)))
            .unwrap();
        assert_eq!((s.info().width, s.info().height), (3, 2));
        assert!(
            s.info().coding.contains("embedded preview only"),
            "{}",
            s.info().coding
        );
        assert!(s.info().coding.contains("300×200"), "{}", s.info().coding);
    }

    #[test]
    fn krita_without_images_is_refused() {
        let f = zip(&[
            ("mimetype", b"application/x-krita"),
            ("maindoc.xml", b"<DOC/>"),
        ]);
        let e = OraDecoder(so::Kind::Kra)
            .open(Arc::new(Source::from_vec("t.kra", f)))
            .err()
            .unwrap();
        assert!(
            e.to_string()
                .contains("neither mergedimage.png nor preview.png"),
            "{e}"
        );
    }

    #[test]
    fn composite_levels_trim() {
        let layer = png(2, 2, [255, 0, 0, 255]);
        let f = zip(&[
            ("mimetype", b"image/openraster"),
            (
                "stack.xml",
                b"<image w=\"4\" h=\"4\"><stack><layer src=\"data/a.png\" x=\"1\" y=\"1\"/></stack></image>",
            ),
            ("data/a.png", &layer),
        ]);
        let s = OraDecoder(so::Kind::Ora)
            .open(Arc::new(Source::from_vec("t.ora", f)))
            .unwrap();
        let r = s
            .decode(&DecodeRequest::full(1), &CancelToken::new())
            .unwrap();
        let px = |x: usize, y: usize| &r.planes[0].data[(y * 4 + x) * 4..(y * 4 + x) * 4 + 4];
        assert_eq!(px(0, 0), &[0, 0, 0, 0]);
        assert_eq!(px(1, 1), &[255, 0, 0, 255]);
        assert_eq!(px(2, 2), &[255, 0, 0, 255]);
        assert_eq!(px(3, 3), &[0, 0, 0, 0]);
        assert_trims(s.as_ref(), &DecodeRequest::full(1));
    }

    #[test]
    fn layers_are_pages() {
        let merged = png(4, 4, [9, 9, 9, 255]);
        let top = png(2, 2, [0, 0, 255, 255]);
        let inner = png(1, 1, [0, 255, 0, 255]);
        let back = png(4, 4, [255, 0, 0, 255]);
        let f = zip(&[
            ("mimetype", b"image/openraster"),
            (
                "stack.xml",
                br#"<image w="4" h="4"><stack>
                    <layer name="Top" src="data/t.png" x="1" visibility="hidden" opacity="0.5"/>
                    <stack x="2" y="2"><layer name="Inner" src="data/i.png" x="1" y="1"/></stack>
                    <layer name="Gone" src="data/missing.png"/>
                    <layer src="data/b.png"/>
                </stack></image>"#,
            ),
            ("data/t.png", &top),
            ("data/i.png", &inner),
            ("data/b.png", &back),
            ("mergedimage.png", &merged),
        ]);
        let s = OraDecoder(so::Kind::Ora)
            .open(Arc::new(Source::from_vec("t.ora", f)))
            .unwrap();
        assert_eq!((s.page_count(), s.page_index()), (4, 0));
        assert_eq!(s.page_label(), Some(PageLabel::Merged));
        let pixels = |s: &Arc<dyn Session>| {
            let r = s
                .decode(&DecodeRequest::full(1), &CancelToken::new())
                .unwrap();
            assert_eq!((r.width, r.height), (4, 4));
            r.planes[0].data.clone()
        };
        let at = |d: &[u8], x: usize, y: usize| d[(y * 4 + x) * 4..][..4].to_vec();
        let p1 = s.open_page(1).unwrap();
        assert_eq!(p1.page_index(), 1);
        assert_eq!(
            p1.page_label(),
            Some(PageLabel::DocumentLayer {
                name: "Top".into(),
                index: 1,
                hidden: true
            })
        );
        assert!(p1.decode_preview().is_none());
        let d = pixels(&p1);
        assert_eq!(at(&d, 1, 0), [0, 0, 255, 128]);
        assert_eq!(at(&d, 2, 1), [0, 0, 255, 128]);
        assert_eq!(at(&d, 0, 0), [0, 0, 0, 0]);
        assert_eq!(at(&d, 3, 0), [0, 0, 0, 0]);
        let p2 = p1.open_page(2).unwrap();
        assert_eq!(
            p2.page_label(),
            Some(PageLabel::DocumentLayer {
                name: "Inner".into(),
                index: 2,
                hidden: false
            })
        );
        let d = pixels(&p2);
        assert_eq!(at(&d, 3, 3), [0, 255, 0, 255]);
        assert_eq!(at(&d, 2, 2), [0, 0, 0, 0]);
        let p3 = p2.open_page(3).unwrap();
        assert_eq!(
            p3.page_label(),
            Some(PageLabel::DocumentLayer {
                name: "".into(),
                index: 3,
                hidden: false
            })
        );
        assert!(pixels(&p3).chunks(4).all(|p| p == [255, 0, 0, 255]));
        assert!(p3.open_page(4).is_err());
        assert!(s.open_page(4).is_err());
        let back0 = p3.open_page(0).unwrap();
        assert_eq!(back0.page_label(), Some(PageLabel::Merged));
        assert!(pixels(&back0).chunks(4).all(|p| p == [9, 9, 9, 255]));
        assert!(p1.decode_memory(&DecodeRequest::full(1)) >= 4 * 4 * 4);
        assert_trims(p1.as_ref(), &DecodeRequest::full(1));
        let gone = CancelToken::new();
        gone.cancel();
        let p = s.open_page(3).unwrap();
        assert!(matches!(
            p.decode(&DecodeRequest::full(1), &gone),
            Err(DecodeError::Cancelled)
        ));
    }

    #[test]
    fn layer_pages_without_merged_image_and_damaged_layers() {
        let good = png(3, 3, [1, 2, 3, 255]);
        let mut cut = png(3, 64, [5, 6, 7, 255]);
        cut.truncate(cut.len() - 20);
        let f = zip(&[
            ("mimetype", b"image/openraster"),
            (
                "stack.xml",
                br#"<image w="3" h="3"><stack><layer name="Cut" src="data/c.png"/><layer name="Good" src="data/g.png"/></stack></image>"#,
            ),
            ("data/c.png", &cut),
            ("data/g.png", &good),
        ]);
        let s = OraDecoder(so::Kind::Ora)
            .open(Arc::new(Source::from_vec("t.ora", f.clone())))
            .unwrap();
        assert_eq!(s.page_count(), 3);
        assert_eq!(s.page_label(), Some(PageLabel::Merged));
        for page in 0..3 {
            // A damaged layer reads as truncated or as an error, never a panic.
            let p = s.open_page(page).unwrap();
            let _ = p.decode(&DecodeRequest::full(1), &CancelToken::new());
        }
        let g = s.open_page(2).unwrap();
        let r = g
            .decode(&DecodeRequest::full(1), &CancelToken::new())
            .unwrap();
        assert!(!r.truncated);
        // Every prefix of the file opens and decodes its pages without panicking.
        for n in (0..f.len()).step_by(7) {
            let Ok(s) = OraDecoder(so::Kind::Ora)
                .open(Arc::new(Source::from_vec("t.ora", f[..n].to_vec())))
            else {
                continue;
            };
            for page in 0..s.page_count() {
                if let Ok(p) = s.open_page(page) {
                    let _ = p.decode(&DecodeRequest::full(1), &CancelToken::new());
                }
            }
        }
    }

    #[test]
    fn single_page_without_layers() {
        let merged = png(2, 2, [1, 1, 1, 255]);
        let f = zip(&[
            ("mimetype", b"image/openraster"),
            ("stack.xml", b"<image w=\"2\" h=\"2\"><stack/></image>"),
            ("mergedimage.png", &merged),
        ]);
        let s = OraDecoder(so::Kind::Ora)
            .open(Arc::new(Source::from_vec("t.ora", f)))
            .unwrap();
        assert_eq!(s.page_count(), 1);
        assert_eq!(s.page_label(), None);
        assert!(s.open_page(1).is_err());
    }

    /// LZF as Krita writes it, simply: literal runs, and runs of one repeated byte as
    /// overlapping back-references at distance 1.
    fn lzf(src: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut lit: Vec<u8> = Vec::new();
        let flush = |out: &mut Vec<u8>, lit: &mut Vec<u8>| {
            for c in lit.chunks(32) {
                out.push(c.len() as u8 - 1);
                out.extend_from_slice(c);
            }
            lit.clear();
        };
        let mut i = 0;
        while i < src.len() {
            let mut run = 0;
            while i > 0 && i + run < src.len() && src[i + run] == src[i - 1] && run < 264 {
                run += 1;
            }
            if run >= 3 {
                flush(&mut out, &mut lit);
                let len = run - 2;
                if len < 7 {
                    out.extend_from_slice(&[(len as u8) << 5, 0]);
                } else {
                    out.extend_from_slice(&[7 << 5, (len - 7) as u8, 0]);
                }
                i += run;
            } else {
                lit.push(src[i]);
                i += 1;
            }
        }
        flush(&mut out, &mut lit);
        out
    }

    /// A version-2 Krita layer file of 64×64 tiles `(x, y, interleaved pixels, compressed)`.
    fn kra_layer(pixel: usize, tiles: &[(i32, i32, Vec<u8>, bool)]) -> Vec<u8> {
        let mut out = format!(
            "VERSION 2\nTILEWIDTH 64\nTILEHEIGHT 64\nPIXELSIZE {pixel}\nDATA {}\n",
            tiles.len()
        )
        .into_bytes();
        for (x, y, px, packed) in tiles {
            assert_eq!(px.len(), 64 * 64 * pixel);
            let body = if *packed {
                let planar: Vec<u8> = (0..pixel)
                    .flat_map(|c| px.chunks(pixel).map(move |p| p[c]))
                    .collect();
                let mut b = vec![1];
                b.extend(lzf(&planar));
                b
            } else {
                let mut b = vec![0];
                b.extend_from_slice(px);
                b
            };
            out.extend_from_slice(format!("{x},{y},LZF,{}\n", body.len()).as_bytes());
            out.extend_from_slice(&body);
        }
        out
    }

    /// A 100×70 Krita document: a hidden half-opaque 8-bit layer, a 16-bit one in a group,
    /// a vector layer, a background with a green default pixel and a mask, and a paint
    /// layer whose data is missing; with a merged image of `[9, 9, 9, 255]` if `merged`.
    fn kra_doc(merged: bool) -> Vec<u8> {
        let maindoc = br#"<?xml version="1.0"?><DOC><IMAGE name="img" width="100" height="70" colorspacename="RGBA" profile="sRGB-elle-V2-srgbtrc.icc"><layers>
            <layer name="Top" nodetype="paintlayer" filename="layer1" x="10" y="-5" opacity="128" visible="0" colorspacename="RGBA"/>
            <layer name="Group" nodetype="grouplayer" filename="layer2" visible="1" x="0" y="0"><layers>
              <layer name="Deep" nodetype="paintlayer" filename="layer3" x="0" y="0" opacity="255" visible="1" colorspacename="RGBA16"/>
            </layers></layer>
            <layer name="Vector" nodetype="shapelayer" filename="layer4" visible="1"/>
            <layer name="Missing" nodetype="paintlayer" filename="layer9" colorspacename="RGBA" visible="1"/>
            <layer name="Back" nodetype="paintlayer" filename="layer5" x="0" y="0" opacity="255" visible="1" colorspacename="RGBA"><masks><mask name="m" nodetype="transparencymask" filename="mask1"/></masks></layer>
          </layers></IMAGE></DOC>"#;
        let blue: Vec<u8> = (0..64 * 64).flat_map(|_| [255, 0, 0, 255]).collect();
        let deep: Vec<u8> = (0..64 * 64)
            .flat_map(|_| [0x1000u16, 0x2000, 0xffff, 0xffff])
            .flat_map(u16::to_le_bytes)
            .collect();
        let ramp: Vec<u8> = (0..64 * 64u32)
            .flat_map(|i| [i as u8, (i * 3) as u8, (i * 7) as u8, 255])
            .collect();
        let merged_png = png(100, 70, [9, 9, 9, 255]);
        let icc = [7u8; 200];
        let (l1, l3, l5) = (
            kra_layer(4, &[(0, 0, blue, true)]),
            kra_layer(8, &[(64, 64, deep, false)]),
            kra_layer(4, &[(64, 0, ramp, true)]),
        );
        let mut members: Vec<(&str, &[u8])> = vec![
            ("mimetype", b"application/x-krita"),
            ("maindoc.xml", maindoc),
            ("img/layers/layer1", &l1),
            ("img/layers/layer3", &l3),
            ("img/layers/layer4", b"<svg/>"),
            ("img/layers/layer5", &l5),
            ("img/layers/layer5.defaultpixel", &[0, 255, 0, 255]),
            ("img/layers/mask1", b"junk"),
            ("img/annotations/icc", &icc),
        ];
        if merged {
            members.push(("mergedimage.png", &merged_png));
        }
        zip(&members)
    }

    #[test]
    fn krita_paint_layers_are_pages() {
        let f = kra_doc(true);
        let s = OraDecoder(so::Kind::Kra)
            .open(Arc::new(Source::from_vec("t.kra", f)))
            .unwrap();
        assert_eq!(
            (s.page_count(), s.page_label()),
            (5, Some(PageLabel::Merged))
        );
        let label = |p: &Arc<dyn Session>| match p.page_label() {
            Some(PageLabel::DocumentLayer {
                name,
                index,
                hidden,
            }) => (name.to_string(), index, hidden),
            l => panic!("{l:?}"),
        };
        let full = |p: &Arc<dyn Session>| {
            let r = p
                .decode(&DecodeRequest::full(1), &CancelToken::new())
                .unwrap();
            assert_eq!((r.width, r.height, r.truncated), (100, 70, false));
            r
        };

        let p1 = s.open_page(1).unwrap();
        assert_eq!(label(&p1), ("Top".into(), 1, true));
        assert_eq!(p1.info().bit_depth, 8);
        assert_eq!(
            p1.info().icc_profile.as_deref().map(|v| &v[..]),
            Some(&[7u8; 200][..])
        );
        assert!(p1.decode_preview().is_none());
        let r = full(&p1);
        assert_eq!(r.layout, PixelLayout::Rgba);
        let at = |d: &[u8], x: usize, y: usize| d[(y * 100 + x) * 4..][..4].to_vec();
        let d = &r.planes[0].data;
        assert_eq!(at(d, 10, 0), [0, 0, 255, 128]);
        assert_eq!(at(d, 73, 58), [0, 0, 255, 128]);
        assert_eq!(at(d, 9, 0), [0, 0, 0, 0]);
        assert_eq!(at(d, 74, 0), [0, 0, 0, 0]);
        assert_eq!(at(d, 10, 59), [0, 0, 0, 0]);
        assert_trims(p1.as_ref(), &DecodeRequest::full(1));

        // The group: its one child composited, in the image's 8-bit RGBA.
        let g = p1.open_page(2).unwrap();
        assert_eq!(label(&g), ("Group".into(), 2, false));
        assert!(
            g.info().coding.contains("8-bit RGBA group layer"),
            "{}",
            g.info().coding
        );
        let r = full(&g);
        assert_eq!(r.layout, PixelLayout::Rgba);
        let d = &r.planes[0].data;
        assert_eq!(at(d, 64, 64), [255, 32, 16, 255]);
        assert_eq!(at(d, 63, 64), [0, 0, 0, 0]);

        let p2 = g.open_page(3).unwrap();
        assert_eq!(label(&p2), ("Deep".into(), 3, false));
        assert_eq!(p2.info().bit_depth, 16);
        let r = full(&p2);
        assert_eq!(r.layout, PixelLayout::Rgba16(HdrEncoding::SRGB));
        let px16 = |x: usize, y: usize| -> Vec<u16> {
            r.planes[0].data[(y * 100 + x) * 8..][..8]
                .chunks(2)
                .map(|b| u16::from_ne_bytes([b[0], b[1]]))
                .collect()
        };
        assert_eq!(px16(64, 64), [0xffff, 0x2000, 0x1000, 0xffff]);
        assert_eq!(px16(99, 69), [0xffff, 0x2000, 0x1000, 0xffff]);
        assert_eq!(px16(63, 64), [0, 0, 0, 0]);
        assert_eq!(px16(64, 63), [0, 0, 0, 0]);
        assert!(p2.decode_memory(&DecodeRequest::full(1)) >= 100 * 70 * 8);

        let p3 = p2.open_page(4).unwrap();
        assert_eq!(label(&p3), ("Back".into(), 4, false));
        let r = full(&p3);
        let d = &r.planes[0].data;
        assert_eq!(at(d, 0, 0), [0, 255, 0, 255]);
        assert_eq!(at(d, 63, 69), [0, 255, 0, 255]);
        assert_eq!(
            at(d, 64 + 5, 2),
            [(133 * 7) as u8, (133 * 3) as u8, 133, 255]
        );
        assert_eq!(at(d, 99, 63), {
            let i = 63 * 64 + 35u32;
            [(i * 7) as u8, (i * 3) as u8, i as u8, 255]
        });
        assert_eq!(at(d, 99, 64), [0, 255, 0, 255]);

        assert!(p3.open_page(5).is_err());
        let back0 = p3.open_page(0).unwrap();
        assert_eq!(back0.page_label(), Some(PageLabel::Merged));
        assert!(
            full(&back0).planes[0]
                .data
                .chunks(4)
                .all(|p| p == [9, 9, 9, 255])
        );

        let gone = CancelToken::new();
        gone.cancel();
        assert!(matches!(
            s.open_page(3)
                .unwrap()
                .decode(&DecodeRequest::full(1), &gone),
            Err(DecodeError::Cancelled)
        ));
    }

    #[test]
    fn krita_without_merged_image_is_composited() {
        let s = OraDecoder(so::Kind::Kra)
            .open(Arc::new(Source::from_vec("t.kra", kra_doc(false))))
            .unwrap();
        let i = s.info();
        assert_eq!(
            (i.width, i.height, i.bit_depth, s.page_count()),
            (100, 70, 8, 5)
        );
        assert_eq!(i.icc_profile.as_deref().map(|v| v.len()), Some(200));
        assert_eq!(s.page_label(), Some(PageLabel::Merged));
        for want in [
            "8-bit RGBA layers composited (no merged image)",
            "vector layer “Vector” skipped",
        ] {
            assert!(i.coding.contains(want), "{}", i.coding);
        }
        let before = s.decode_memory(&DecodeRequest::full(1));
        assert!(before >= 2 * 100 * 70 * 4, "{before}");
        let r = s
            .decode(&DecodeRequest::full(1), &CancelToken::new())
            .unwrap();
        assert_eq!((r.width, r.height, r.layout), (100, 70, PixelLayout::Rgba));
        let at = |x: usize, y: usize| r.planes[0].data[(y * 100 + x) * 4..][..4].to_vec();
        // The group's 16-bit layer over the background's default pixel and tile; the hidden
        // top layer is left out.
        assert_eq!(at(64, 64), [255, 32, 16, 255]);
        assert_eq!(at(10, 0), [0, 255, 0, 255]);
        assert_eq!(at(64 + 5, 2), [(133 * 7) as u8, (133 * 3) as u8, 133, 255]);
        assert_trims(s.as_ref(), &DecodeRequest::full(1));
        // Back to page 0 from a layer page: the composite again.
        let p0 = s.open_page(4).unwrap().open_page(0).unwrap();
        assert!(p0.info().coding.contains("composited"));
    }

    #[test]
    fn krita_damaged_layers_never_panic() {
        let f = kra_doc(false);
        // Every prefix of the file opens and decodes its pages without panicking.
        for n in (0..f.len()).step_by(11) {
            let Ok(s) = OraDecoder(so::Kind::Kra)
                .open(Arc::new(Source::from_vec("t.kra", f[..n].to_vec())))
            else {
                continue;
            };
            for page in 0..s.page_count() {
                if let Ok(p) = s.open_page(page) {
                    let _ = p.decode(&DecodeRequest::full(1), &CancelToken::new());
                }
            }
        }
        // A layer file cut short (its tile line promises more bytes) reads as truncated.
        let mut layer = kra_layer(4, &[(0, 0, vec![200; 64 * 64 * 4], false)]);
        layer.truncate(layer.len() - 100);
        let f = zip(&[
            ("mimetype", b"application/x-krita"),
            (
                "maindoc.xml",
                br#"<DOC><IMAGE name="i" width="8" height="8"><layers><layer name="L" nodetype="paintlayer" filename="l" colorspacename="RGBA" visible="1"/></layers></IMAGE></DOC>"#,
            ),
            ("i/layers/l", &layer),
            ("preview.png", &png(2, 2, [1, 1, 1, 255])),
        ]);
        let s = OraDecoder(so::Kind::Kra)
            .open(Arc::new(Source::from_vec("t.kra", f)))
            .unwrap();
        assert_eq!(s.page_count(), 2);
        let r = s
            .open_page(1)
            .unwrap()
            .decode(&DecodeRequest::full(1), &CancelToken::new())
            .unwrap();
        assert!(r.truncated);
        assert!(r.planes[0].data.chunks(4).all(|p| p == [0, 0, 0, 0]));
    }
}
