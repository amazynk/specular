//! Fixed-layout documents shown as pages: DjVu (`specular-djvu`), XPS and OpenXPS
//! (`specular-xps`) and CGM (`specular-cgm`).
//!
//! DjVu pages are rasters: the session's full size is the page's own pixel size (turned as
//! the page says), and scales 2/4/8 average blocks of page pixels. XPS and CGM pages are
//! vector: like PDF, the full raster is the page at 288 dpi, doubled until it is crisp and
//! capped ([`crate::pdf_dec::raster_size`]), and every scale and region renders directly
//! from the page description. Every page of a document is a page of the session, sharing one
//! parsed document. Pages are opaque (composited on white, or the picture's background).

use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use crate::{DecodeError, DecodeRequest, Decoder, Format, ImageInfo, Rect, Result, Session};
use specular_core::{CancelToken, Orientation, PixelLayout, Raster, RasterPlane, Source};

/// Largest file of an indirect DjVu document read beside the index.
const MAX_PART_BYTES: u64 = 1 << 30;

struct SourceBytes(Arc<Source>);

impl AsRef<[u8]> for SourceBytes {
    fn as_ref(&self) -> &[u8] {
        self.0.bytes()
    }
}

fn rgba_raster(data: Vec<u8>, x: u32, y: u32, w: u32, h: u32, scale: u32) -> Raster {
    Raster {
        width: w,
        height: h,
        origin: (x, y),
        scale,
        layout: PixelLayout::Rgba,
        planes: vec![RasterPlane {
            width: w,
            height: h,
            stride: w as usize * 4,
            data,
            h_div: 1,
            v_div: 1,
        }],
        truncated: false,
    }
}

/// The region of a request at its scale, clamped to the scaled image.
fn region(info: &ImageInfo, req: &DecodeRequest) -> Result<(u32, u32, u32, u32, u32)> {
    let scale = req.scale.clamp(1, 8);
    let (sw, sh) = (info.width.div_ceil(scale), info.height.div_ceil(scale));
    let (x, y, w, h) = match req.region {
        Some(Rect { x, y, w, h }) => {
            let x = x.min(sw);
            let y = y.min(sh);
            (x, y, w.min(sw - x), h.min(sh - y))
        }
        None => (0, 0, sw, sh),
    };
    if w == 0 || h == 0 {
        return Err(DecodeError::Unsupported("empty region".into()));
    }
    Ok((scale, x, y, w, h))
}

fn region_memory(info: &ImageInfo, req: &DecodeRequest) -> u64 {
    let s = req.scale.clamp(1, 8);
    match req.region {
        Some(r) => r.w as u64 * r.h as u64,
        None => info.width.div_ceil(s) as u64 * info.height.div_ceil(s) as u64,
    }
}

/// An XMP packet from document facts (title, author, subject, keywords, creator tool,
/// producer), through the PDF crate's writer.
fn facts_xmp(info: specular_pdf::Info) -> Option<Vec<u8>> {
    info.to_xmp()
}

// ----- DjVu ------------------------------------------------------------------------------

pub struct DjvuDecoder;

impl Decoder for DjvuDecoder {
    fn format(&self) -> Format {
        Format::Djvu
    }
    fn probe(&self, head: &[u8]) -> bool {
        specular_djvu::looks_like_djvu(head)
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["djvu", "djv"]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let opts = specular_djvu::Options {
            resolver: Some(sibling_resolver(source.path())),
            jpeg: Some(Arc::new(|d: &[u8]| crate::svg::decode_rgba(d))),
            mmr: Some(Arc::new(decode_mmr)),
        };
        let doc = specular_djvu::Document::open(Arc::new(SourceBytes(source.clone())), opts)
            .map_err(djvu_error)?;
        if doc.page_count() == 0 {
            return Err(DecodeError::Malformed("DjVu document without pages".into()));
        }
        let shared = Arc::new(DjvuShared { doc, source });
        Ok(Arc::new(DjvuSession::new(shared, 0)?))
    }
}

fn djvu_error(e: specular_djvu::Error) -> DecodeError {
    use specular_djvu::Error as E;
    match e {
        E::Cancelled => DecodeError::Cancelled,
        E::TooLarge => DecodeError::TooLarge,
        E::NeedsMemory(n) => DecodeError::NeedsMemory(n),
        E::Unsupported(_) | E::NoImage(_) | E::Missing(_) => {
            DecodeError::Unsupported(e.to_string())
        }
        E::Malformed(_) | E::NoSuchPage(_) => DecodeError::Malformed(e.to_string()),
    }
}

/// Finds the parts of an indirect DjVu document: plain relative names in or below the index
/// file's folder (no absolute paths, `..` or links leaving it).
fn sibling_resolver(index: &Path) -> specular_djvu::Resolver {
    let dir: Option<PathBuf> = index.parent().map(Path::to_path_buf);
    Arc::new(move |name: &str| {
        let dir = dir.as_ref()?;
        let rel = Path::new(name);
        if name.is_empty() || !rel.components().all(|c| matches!(c, Component::Normal(_))) {
            return None;
        }
        let path = dir.join(rel);
        let base = dir.canonicalize().ok()?;
        let real = path.canonicalize().ok()?;
        if !real.starts_with(&base) {
            return None;
        }
        let meta = std::fs::metadata(&real).ok()?;
        if !meta.is_file() || meta.len() > MAX_PART_BYTES {
            return None;
        }
        let data = std::fs::read(&real).ok()?;
        Some(Arc::new(data) as specular_djvu::Bytes)
    })
}

/// G4 (MMR) data of a DjVu `Smmr` mask through the TIFF decoder: a one-strip bilevel TIFF
/// is built around it.
fn decode_mmr(data: &[u8], w: u32, h: u32) -> Option<Vec<u8>> {
    if u64::from(w) * u64::from(h) > 1 << 31 {
        return None;
    }
    let entries: [(u16, u16, u32); 9] = [
        (256, 4, w),
        (257, 4, h),
        (258, 3, 1),
        (259, 3, 4),
        (262, 3, 0),
        (273, 4, 0), // strip offset, filled below
        (277, 3, 1),
        (278, 4, h),
        (279, 4, data.len() as u32),
    ];
    let ifd_len = 2 + entries.len() * 12 + 4;
    let data_at = (8 + ifd_len) as u32;
    let mut t = Vec::with_capacity(data_at as usize + data.len());
    t.extend_from_slice(b"II*\0");
    t.extend_from_slice(&8u32.to_le_bytes());
    t.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    for (tag, ty, v) in entries {
        let v = if tag == 273 { data_at } else { v };
        t.extend_from_slice(&tag.to_le_bytes());
        t.extend_from_slice(&ty.to_le_bytes());
        t.extend_from_slice(&1u32.to_le_bytes());
        if ty == 3 {
            t.extend_from_slice(&(v as u16).to_le_bytes());
            t.extend_from_slice(&[0, 0]);
        } else {
            t.extend_from_slice(&v.to_le_bytes());
        }
    }
    t.extend_from_slice(&0u32.to_le_bytes());
    t.extend_from_slice(data);
    let (tw, th, rgba) = crate::svg::decode_rgba(&t)?;
    if (tw, th) != (w, h) {
        return None;
    }
    Some(rgba.chunks_exact(4).map(|p| u8::from(p[0] < 128)).collect())
}

struct DjvuShared {
    doc: specular_djvu::Document,
    source: Arc<Source>,
}

struct DjvuSession {
    shared: Arc<DjvuShared>,
    page: u32,
    info: ImageInfo,
    rotated: bool,
}

impl DjvuSession {
    fn new(shared: Arc<DjvuShared>, page: u32) -> Result<DjvuSession> {
        let p = shared.doc.page_info(page as usize).map_err(djvu_error)?;
        let (w, h) = p.display_size();
        let structure = shared.doc.facts().structure;
        let mut coding = format!("DjVu ({structure}), {} dpi", p.dpi);
        if !p.layers.is_empty() {
            coding.push_str(", ");
            coding.push_str(&p.layers);
        }
        if p.has_text {
            coding.push_str(", text layer");
        }
        let info = ImageInfo {
            format: Format::Djvu,
            width: w,
            height: h,
            orientation: Orientation::NORMAL,
            icc_profile: None,
            has_alpha: false,
            bit_depth: 8,
            progressive: false,
            coding,
            embedded_preview: None,
            fast_regions: true,
            scales: &[1, 2, 4, 8],
        };
        Ok(DjvuSession {
            shared,
            page,
            info,
            rotated: p.rotation != specular_djvu::Rotation::None,
        })
    }
}

impl Session for DjvuSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn alpha(&self) -> Option<crate::Alpha> {
        None
    }
    fn source(&self) -> &Arc<Source> {
        &self.shared.source
    }
    /// The page's embedded thumbnail (`TH44`), when the document has one.
    fn decode_preview(&self) -> Option<Result<Raster>> {
        if self.rotated {
            return None;
        }
        let p = self.shared.doc.thumbnail(self.page as usize, &|| false)?;
        let (w, h) = (p.width, p.height);
        let data = if p.channels == 3 {
            p.data
                .chunks_exact(3)
                .flat_map(|c| [c[0], c[1], c[2], 255])
                .collect()
        } else {
            p.data.iter().flat_map(|&g| [g, g, g, 255]).collect()
        };
        Some(Ok(rgba_raster(data, 0, 0, w, h, 1)))
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        if cancel.is_cancelled() {
            return Err(DecodeError::Cancelled);
        }
        let (scale, x, y, w, h) = region(&self.info, req)?;
        let data = self
            .shared
            .doc
            .render(
                specular_djvu::RenderRequest {
                    page: self.page as usize,
                    scale,
                    x,
                    y,
                    width: w,
                    height: h,
                },
                &|| cancel.is_cancelled(),
            )
            .map_err(djvu_error)?;
        Ok(rgba_raster(data, x, y, w, h, scale))
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        // The output, the full-resolution mask band and the page's decoded layers.
        region_memory(&self.info, req) * 4
            + (40 << 20)
            + u64::from(self.info.width) * u64::from(self.info.height) / 4
    }
    fn memory_bytes(&self) -> usize {
        self.shared.doc.memory_bytes()
    }
    fn trim_memory(&self) -> usize {
        self.shared.doc.trim()
    }
    fn colour_model(&self) -> Option<&'static str> {
        Some("RGB")
    }
    fn page_count(&self) -> u32 {
        u32::try_from(self.shared.doc.page_count()).unwrap_or(u32::MAX)
    }
    fn page_index(&self) -> u32 {
        self.page
    }
    fn open_page(&self, page: u32) -> Result<Arc<dyn Session>> {
        Ok(Arc::new(DjvuSession::new(self.shared.clone(), page)?))
    }
    fn embedded_xmp(&self) -> Option<Vec<u8>> {
        let f = self.shared.doc.facts();
        let get = |k: &str| {
            f.metadata
                .iter()
                .find(|(key, _)| key.eq_ignore_ascii_case(k))
                .map(|(_, v)| v.clone())
                .filter(|v| !v.trim().is_empty())
        };
        facts_xmp(specular_pdf::Info {
            title: get("Title"),
            author: get("Author"),
            subject: get("Subject"),
            keywords: get("Keywords"),
            creator: get("Creator"),
            producer: get("Producer"),
        })
    }
}

// ----- XPS and CGM -----------------------------------------------------------------------

/// A vector page document: page sizes in points and region rendering.
trait VectorDoc: Send + Sync {
    fn pages(&self) -> usize;
    fn size(&self, page: usize) -> Option<(f64, f64)>;
    #[allow(clippy::too_many_arguments)]
    fn draw(
        &self,
        page: usize,
        sx: f64,
        sy: f64,
        x: u32,
        y: u32,
        w: u32,
        h: u32,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>>;
    fn memory(&self) -> usize {
        0
    }
    fn trim(&self) -> usize {
        0
    }
    fn describe(&self, page: usize) -> String;
    fn xmp(&self) -> Option<Vec<u8>>;
}

struct Xps(specular_xps::Document);

impl VectorDoc for Xps {
    fn pages(&self) -> usize {
        self.0.page_count()
    }
    fn size(&self, page: usize) -> Option<(f64, f64)> {
        self.0.page_size(page)
    }
    fn draw(
        &self,
        page: usize,
        sx: f64,
        sy: f64,
        x: u32,
        y: u32,
        w: u32,
        h: u32,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>> {
        use specular_xps::RenderError as E;
        let req = specular_xps::RenderRequest {
            page,
            scale_x: sx,
            scale_y: sy,
            x,
            y,
            width: w,
            height: h,
        };
        self.0.render(req, cancel).map_err(|e| match e {
            E::Cancelled => DecodeError::Cancelled,
            E::TooLarge(..) => DecodeError::TooLarge,
            e => DecodeError::Malformed(e.to_string()),
        })
    }
    fn memory(&self) -> usize {
        self.0.memory_bytes()
    }
    fn trim(&self) -> usize {
        self.0.trim_memory()
    }
    fn describe(&self, _page: usize) -> String {
        let f = self.0.facts();
        let mut s = String::from(if f.openxps { "OpenXPS" } else { "XPS" });
        if f.fonts > 0 {
            s.push_str(&format!(", {} embedded fonts", f.fonts));
        }
        if f.signed {
            s.push_str(", digitally signed (not verified)");
        }
        s
    }
    fn xmp(&self) -> Option<Vec<u8>> {
        let f = self.0.facts();
        facts_xmp(specular_pdf::Info {
            title: f.title,
            author: f.creator,
            subject: f.subject.or(f.description),
            keywords: f.keywords,
            creator: None,
            producer: None,
        })
    }
}

struct Cgm(specular_cgm::Document);

impl VectorDoc for Cgm {
    fn pages(&self) -> usize {
        self.0.page_count()
    }
    fn size(&self, page: usize) -> Option<(f64, f64)> {
        self.0.page_size(page)
    }
    fn draw(
        &self,
        page: usize,
        sx: f64,
        sy: f64,
        x: u32,
        y: u32,
        w: u32,
        h: u32,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>> {
        use specular_cgm::RenderError as E;
        let req = specular_cgm::RenderRequest {
            page,
            scale_x: sx,
            scale_y: sy,
            x,
            y,
            width: w,
            height: h,
        };
        self.0.render(req, cancel).map_err(|e| match e {
            E::Cancelled => DecodeError::Cancelled,
            E::TooLarge(..) => DecodeError::TooLarge,
            e => DecodeError::Malformed(e.to_string()),
        })
    }
    fn describe(&self, page: usize) -> String {
        let f = self.0.facts();
        let mut s = format!("CGM ({})", f.encoding);
        if let Some(v) = f.version {
            s.push_str(&format!(", version {v}"));
        }
        if let Some(p) = &f.profile {
            s.push_str(&format!(", {p}"));
        }
        if let Some(name) = f.pictures.get(page).filter(|n| !n.is_empty()) {
            s.push_str(&format!(", picture \"{name}\""));
        }
        let skipped: u64 = f.skipped.iter().map(|(_, n)| n).sum();
        if skipped > 0 {
            s.push_str(&format!(", {skipped} elements not drawn"));
        }
        if f.truncated {
            s.push_str(", file ends early");
        }
        s
    }
    fn xmp(&self) -> Option<Vec<u8>> {
        let f = self.0.facts();
        facts_xmp(specular_pdf::Info {
            title: f.name.filter(|n| !n.trim().is_empty()),
            subject: f.description.filter(|d| !d.trim().is_empty()),
            ..specular_pdf::Info::default()
        })
    }
}

/// A drawing parsed into `specular-drawpage` pages (Visio, OpenDocument Graphics, WordPerfect
/// Graphics, Sketch): the reader's facts give the coding per page.
pub(crate) struct DrawDoc {
    pub pages: Vec<specular_drawpage::Page>,
    pub describe: Box<dyn Fn(usize) -> String + Send + Sync>,
    pub xmp: Option<Vec<u8>>,
}

impl VectorDoc for DrawDoc {
    fn pages(&self) -> usize {
        self.pages.len()
    }
    fn size(&self, page: usize) -> Option<(f64, f64)> {
        self.pages
            .get(page)
            .map(|p| p.size)
            .filter(|s| s.0 > 0.0 && s.1 > 0.0 && s.0.is_finite() && s.1.is_finite())
    }
    fn draw(
        &self,
        page: usize,
        sx: f64,
        sy: f64,
        x: u32,
        y: u32,
        w: u32,
        h: u32,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>> {
        use specular_drawpage::RenderError as E;
        let p = self
            .pages
            .get(page)
            .ok_or_else(|| DecodeError::Unsupported(format!("no page {page}")))?;
        let req = specular_drawpage::RenderRequest {
            scale_x: sx,
            scale_y: sy,
            x,
            y,
            width: w,
            height: h,
        };
        specular_drawpage::render(p, req, cancel).map_err(|e| match e {
            E::Cancelled => DecodeError::Cancelled,
            E::TooLarge(..) => DecodeError::TooLarge,
            e => DecodeError::Malformed(e.to_string()),
        })
    }
    fn describe(&self, page: usize) -> String {
        (self.describe)(page)
    }
    fn xmp(&self) -> Option<Vec<u8>> {
        self.xmp.clone()
    }
}

/// Opens a parsed drawing as a paged vector session of `format`.
pub(crate) fn open_drawing(
    doc: DrawDoc,
    source: Arc<Source>,
    format: Format,
) -> Result<Arc<dyn Session>> {
    open_vector(Arc::new(doc), source, format)
}

/// An XMP packet from title, author, subject and keywords.
pub(crate) fn doc_xmp(
    title: Option<String>,
    author: Option<String>,
    subject: Option<String>,
    keywords: Option<String>,
) -> Option<Vec<u8>> {
    facts_xmp(specular_pdf::Info {
        title,
        author,
        subject,
        keywords,
        ..specular_pdf::Info::default()
    })
}

pub struct XpsDecoder;

impl Decoder for XpsDecoder {
    fn format(&self) -> Format {
        Format::Xps
    }
    /// No signature of its own (a ZIP package): chosen by extension, or by the fixed pages
    /// named in the ZIP directory at the end of the file.
    fn probe(&self, _head: &[u8]) -> bool {
        false
    }
    fn probe_file(&self, data: &[u8]) -> bool {
        if !specular_xps::looks_like_zip(data) {
            return false;
        }
        let tail = &data[data.len().saturating_sub(1 << 20)..];
        memchr::memmem::find(tail, b".fpage").is_some()
            && memchr::memmem::find(tail, b".fdseq").is_some()
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["xps", "oxps"]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        use specular_xps::OpenError as E;
        let decode: specular_xps::ImageDecoder = Arc::new(|d: &[u8]| crate::svg::decode_rgba(d));
        let doc = specular_xps::Document::open(Arc::new(SourceBytes(source.clone())), Some(decode))
            .map_err(|e| match e {
                E::Protected | E::Encrypted => DecodeError::Unsupported(e.to_string()),
                e => DecodeError::Malformed(e.to_string()),
            })?;
        open_vector(Arc::new(Xps(doc)), source, Format::Xps)
    }
}

pub struct CgmDecoder;

impl Decoder for CgmDecoder {
    fn format(&self) -> Format {
        Format::Cgm
    }
    fn probe(&self, head: &[u8]) -> bool {
        specular_cgm::looks_like_cgm(head)
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["cgm"]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        use specular_cgm::OpenError as E;
        let doc = specular_cgm::Document::open(source.bytes()).map_err(|e| match e {
            E::CharacterEncoding => DecodeError::Unsupported(e.to_string()),
            e => DecodeError::Malformed(e.to_string()),
        })?;
        open_vector(Arc::new(Cgm(doc)), source, Format::Cgm)
    }
}

fn open_vector(
    doc: Arc<dyn VectorDoc>,
    source: Arc<Source>,
    format: Format,
) -> Result<Arc<dyn Session>> {
    if doc.pages() == 0 {
        return Err(DecodeError::Malformed("document without pages".into()));
    }
    let shared = Arc::new(VectorShared {
        doc,
        source,
        format,
    });
    Ok(Arc::new(VectorSession::new(shared, 0)?))
}

struct VectorShared {
    doc: Arc<dyn VectorDoc>,
    source: Arc<Source>,
    format: Format,
}

struct VectorSession {
    shared: Arc<VectorShared>,
    page: u32,
    info: ImageInfo,
    points: (f64, f64),
}

impl VectorSession {
    fn new(shared: Arc<VectorShared>, page: u32) -> Result<VectorSession> {
        let (pw, ph) = shared
            .doc
            .size(page as usize)
            .ok_or_else(|| DecodeError::Unsupported(format!("no page {page}")))?;
        let (w, h, k) = crate::pdf_dec::raster_size(pw as f32, ph as f32)
            .ok_or_else(|| DecodeError::Malformed("page without a size".into()))?;
        let info = ImageInfo {
            format: shared.format,
            width: w,
            height: h,
            orientation: Orientation::NORMAL,
            icc_profile: None,
            has_alpha: false,
            bit_depth: 8,
            progressive: false,
            coding: format!(
                "{}, {pw:.0}×{ph:.0} pt, rendered at {:.0} dpi",
                shared.doc.describe(page as usize),
                k * 72.0
            ),
            embedded_preview: None,
            fast_regions: true,
            scales: &[1, 2, 4, 8],
        };
        Ok(VectorSession {
            shared,
            page,
            info,
            points: (pw, ph),
        })
    }
}

impl Session for VectorSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn alpha(&self) -> Option<crate::Alpha> {
        None
    }
    fn source(&self) -> &Arc<Source> {
        &self.shared.source
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        None
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        if cancel.is_cancelled() {
            return Err(DecodeError::Cancelled);
        }
        let (scale, x, y, w, h) = region(&self.info, req)?;
        let (sw, sh) = (
            self.info.width.div_ceil(scale),
            self.info.height.div_ceil(scale),
        );
        let (pw, ph) = self.points;
        let data = self.shared.doc.draw(
            self.page as usize,
            f64::from(sw) / pw,
            f64::from(sh) / ph,
            x,
            y,
            w,
            h,
            &|| cancel.is_cancelled(),
        )?;
        Ok(rgba_raster(data, x, y, w, h, scale))
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        // The output plus the rasteriser's strips, layers and decoded images.
        region_memory(&self.info, req) * 4 * 2 + (16 << 20)
    }
    fn memory_bytes(&self) -> usize {
        self.shared.doc.memory()
    }
    fn trim_memory(&self) -> usize {
        self.shared.doc.trim()
    }
    fn colour_model(&self) -> Option<&'static str> {
        Some("RGB")
    }
    fn page_count(&self) -> u32 {
        u32::try_from(self.shared.doc.pages()).unwrap_or(u32::MAX)
    }
    fn page_index(&self) -> u32 {
        self.page
    }
    fn open_page(&self, page: u32) -> Result<Arc<dyn Session>> {
        Ok(Arc::new(VectorSession::new(self.shared.clone(), page)?))
    }
    fn embedded_xmp(&self) -> Option<Vec<u8>> {
        self.shared.doc.xmp()
    }
}
