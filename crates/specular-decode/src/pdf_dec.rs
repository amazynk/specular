//! PDF and Adobe Illustrator pages through hayro (`specular-pdf`), and EPS through its preview.
//! GeoPDF pages (ISO 32000 measures, OGC LGIDicts) are georeferenced ([`crate::geopdf`]).
//!
//! A page has no pixel size, so as for SVG the session picks one: the "full" (scale 1) raster
//! is the page at 288 dpi (4 px per point), doubled until its long side is at least
//! [`MIN_LONG_SIDE`] px, and capped at [`MAX_SIDE`] px per side and [`MAX_PIXELS`]. Scales
//! 1/2/4/8 and regions render directly from the page description, never resampled, so tiles
//! and deep zoom cost only the pixels shown. Pages are opaque (composited on white, as
//! printed). Every page of a document is a page of the session ([`Session::open_page`]),
//! sharing one parsed document.
//!
//! Illustrator files saved PDF-compatible (the default since Illustrator 9) are PDFs; older,
//! PostScript-only ones are shown through their preview like EPS. EPS is never interpreted:
//! the TIFF preview of a DOS EPS binary header goes through the TIFF decoder, an EPSI hex
//! preview is decoded here, a Windows Metafile preview goes through the metafile decoder, and
//! files with none are reported as unsupported.

use crate::whole::{self, Levels};
use crate::{DecodeError, DecodeRequest, Decoder, Format, ImageInfo, Result, Session};
use specular_core::{CancelToken, Orientation, PixelLayout, Raster, RasterPlane, Source};
use specular_pdf::{Document, OpenError, RenderError, RenderRequest, eps};
use std::sync::Arc;

/// Long side the full raster reaches at least (small pages render crisp at normal zoom).
pub const MIN_LONG_SIDE: f32 = 2048.0;
/// Pixels per point of the full raster before [`MIN_LONG_SIDE`] (288 dpi).
pub const BASE_PX_PER_PT: f32 = 4.0;
/// Largest side of the full raster.
pub const MAX_SIDE: f32 = 32768.0;
/// Largest full raster, in pixels.
pub const MAX_PIXELS: f32 = 256e6;

pub struct PdfDecoder;

fn has_ext(source: &Source, ext: &str) -> bool {
    source
        .path()
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case(ext))
}

impl Decoder for PdfDecoder {
    fn format(&self) -> Format {
        Format::Pdf
    }
    fn probe(&self, head: &[u8]) -> bool {
        head.starts_with(b"%PDF-")
    }
    /// The specification lets a header follow up to 1 KiB of leading bytes.
    fn probe_file(&self, data: &[u8]) -> bool {
        specular_pdf::looks_like_pdf(data)
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["pdf", "ai"]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let ai = has_ext(&source, "ai");
        let format = if ai { Format::Illustrator } else { Format::Pdf };
        if specular_pdf::looks_like_pdf(source.bytes()) {
            return open_pdf(source, format);
        }
        if ai && eps::looks_like_postscript(source.bytes()) {
            // A pre-PDF (PostScript) Illustrator file: its preview, if it has one.
            return open_eps(source, Format::Illustrator).map_err(|e| match e {
                DecodeError::Unsupported(_) => DecodeError::Unsupported(
                    "PostScript Illustrator file without a preview: PostScript is not rendered"
                        .into(),
                ),
                e => e,
            });
        }
        Err(DecodeError::Malformed("not a PDF".into()))
    }
}

pub struct EpsDecoder;

/// Why a PostScript document is not shown.
const PS_REFUSAL: &str = "PostScript documents are programs, and Specular has no PostScript \
interpreter: only the previews saved inside EPS files are shown. Convert it to PDF to view it.";

impl Decoder for EpsDecoder {
    fn format(&self) -> Format {
        Format::Eps
    }
    fn probe(&self, head: &[u8]) -> bool {
        eps::looks_like_eps(head)
    }
    /// `.ps` documents open only when they carry an EPS preview; the others are refused.
    fn extensions(&self) -> &'static [&'static str] {
        &["eps", "epsf", "epsi", "ps"]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        // An old Illustrator file can carry an EPSF header and be probed here.
        let format = if has_ext(&source, "ai") {
            Format::Illustrator
        } else {
            Format::Eps
        };
        if has_ext(&source, "ps") && !eps::looks_like_eps(source.bytes()) {
            return Err(DecodeError::Unsupported(
                if eps::looks_like_postscript(source.bytes()) {
                    PS_REFUSAL.into()
                } else {
                    "this .ps file is not PostScript".into()
                },
            ));
        }
        open_eps(source, format)
    }
}

fn open_error(e: OpenError) -> DecodeError {
    match e {
        OpenError::Protected | OpenError::UnsupportedEncryption => {
            DecodeError::Unsupported(e.to_string())
        }
        OpenError::Invalid | OpenError::NoPages => DecodeError::Malformed(e.to_string()),
    }
}

struct SourceBytes(Arc<Source>);

impl AsRef<[u8]> for SourceBytes {
    fn as_ref(&self) -> &[u8] {
        self.0.bytes()
    }
}

pub(crate) fn open_pdf(source: Arc<Source>, format: Format) -> Result<Arc<dyn Session>> {
    let doc = Document::open(Arc::new(SourceBytes(source.clone()))).map_err(open_error)?;
    let doc = Arc::new(Shared {
        doc,
        source,
        format,
    });
    Ok(Arc::new(PdfSession::new(doc, 0)?))
}

/// What every page session of a document shares.
struct Shared {
    doc: Document,
    source: Arc<Source>,
    format: Format,
}

struct PdfSession {
    shared: Arc<Shared>,
    page: u32,
    info: ImageInfo,
    /// Page size in points.
    points: (f32, f32),
    /// The page's GeoPDF georeference, read on first use.
    geo: std::sync::OnceLock<Option<Arc<crate::geo::GeoRef>>>,
    /// The page's drawing scale (viewport measures, paper size), read on first use.
    scale: std::sync::OnceLock<Option<Arc<crate::drawing::DrawingScale>>>,
}

/// The full raster of a `pw`×`ph` pt page: its size and pixels per point.
pub fn raster_size(pw: f32, ph: f32) -> Option<(u32, u32, f32)> {
    if !(pw.is_finite() && ph.is_finite() && pw > 0.0 && ph > 0.0) {
        return None;
    }
    let long = pw.max(ph);
    let mut k = BASE_PX_PER_PT;
    while long * k < MIN_LONG_SIDE {
        k *= 2.0;
    }
    k = k.min(MAX_SIDE / long).min((MAX_PIXELS / (pw * ph)).sqrt());
    let w = ((pw * k).round() as u32).clamp(1, MAX_SIDE as u32);
    let h = ((ph * k).round() as u32).clamp(1, MAX_SIDE as u32);
    Some((w, h, k))
}

impl PdfSession {
    fn new(shared: Arc<Shared>, page: u32) -> Result<PdfSession> {
        let (pw, ph) = shared
            .doc
            .page_size(page as usize)
            .ok_or_else(|| DecodeError::Unsupported(format!("no page {page}")))?;
        let (w, h, k) = raster_size(pw, ph)
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
                "PDF {}, {pw:.0}×{ph:.0} pt, rendered at {:.0} dpi",
                shared.doc.version(),
                k * 72.0
            ),
            embedded_preview: None,
            fast_regions: true,
            scales: &[1, 2, 4, 8],
        };
        Ok(PdfSession {
            shared,
            page,
            info,
            points: (pw, ph),
            geo: std::sync::OnceLock::new(),
            scale: std::sync::OnceLock::new(),
        })
    }
}

impl Session for PdfSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    /// Pages are composited on white: no transparency.
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
        let scale = req.scale.clamp(1, 8);
        let (sw, sh) = (
            self.info.width.div_ceil(scale),
            self.info.height.div_ceil(scale),
        );
        let (x, y, w, h) = match req.region {
            Some(r) => {
                let x = r.x.min(sw);
                let y = r.y.min(sh);
                (x, y, r.w.min(sw - x), r.h.min(sh - y))
            }
            None => (0, 0, sw, sh),
        };
        if w == 0 || h == 0 {
            return Err(DecodeError::Unsupported("empty region".into()));
        }
        let (pw, ph) = self.points;
        let rr = RenderRequest {
            page: self.page as usize,
            scale_x: f64::from(sw) / f64::from(pw),
            scale_y: f64::from(sh) / f64::from(ph),
            x,
            y,
            width: w,
            height: h,
        };
        let data = self
            .shared
            .doc
            .render(rr, &|| cancel.is_cancelled())
            .map_err(|e| match e {
                RenderError::Cancelled => DecodeError::Cancelled,
                RenderError::TooLarge(..) => DecodeError::TooLarge,
                e => DecodeError::Malformed(e.to_string()),
            })?;
        Ok(Raster {
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
        })
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        let s = req.scale.clamp(1, 8);
        let px = match req.region {
            Some(r) => r.w as u64 * r.h as u64,
            None => self.info.width.div_ceil(s) as u64 * self.info.height.div_ceil(s) as u64,
        };
        // The output plus the rasteriser's strips, layers and decoded page images.
        px * 4 * 2 + (16 << 20)
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
        Ok(Arc::new(PdfSession::new(self.shared.clone(), page)?))
    }
    fn embedded_xmp(&self) -> Option<Vec<u8>> {
        self.shared.doc.xmp()
    }
    fn page_points(&self) -> Option<(f32, f32)> {
        Some(self.points)
    }
    fn drawing_scale(&self) -> Option<Arc<crate::drawing::DrawingScale>> {
        self.scale
            .get_or_init(|| {
                use specular_vector::kurbo::Affine;
                let (pw, ph) = self.points;
                let (sx, sy) = (
                    f64::from(self.info.width) / f64::from(pw),
                    f64::from(self.info.height) / f64::from(ph),
                );
                let index = self.page as usize;
                let user_to_page = Affine::new(self.shared.doc.user_to_page(index)?);
                let stored_to_user =
                    user_to_page.inverse() * Affine::scale_non_uniform(1.0 / sx, 1.0 / sy);
                let vps = self.shared.doc.scale_viewports(index);
                Some(Arc::new(crate::drawing::pdf_scale(
                    &vps,
                    stored_to_user,
                    (sx * sy).sqrt(),
                )))
            })
            .clone()
    }
    fn geo(&self) -> Option<Arc<crate::geo::GeoRef>> {
        self.geo
            .get_or_init(|| {
                let (pw, ph) = self.points;
                let (w, h) = (self.info.width, self.info.height);
                crate::geopdf::page_georef(
                    &self.shared.doc,
                    self.page as usize,
                    (f64::from(w) / f64::from(pw), f64::from(h) / f64::from(ph)),
                    (w, h),
                )
                .map(Arc::new)
            })
            .clone()
    }
}

// ----- EPS -------------------------------------------------------------------------------

fn open_eps(source: Arc<Source>, format: Format) -> Result<Arc<dyn Session>> {
    let data = source.bytes();
    let dos = eps::dos_header(data);
    if let Some(tiff) = dos.as_ref().and_then(|d| d.tiff.clone()) {
        let part = Source::from_slice(source.path(), source.clone(), &data[tiff])
            .ok_or_else(|| DecodeError::Malformed("EPS TIFF preview".into()))?;
        let inner = crate::tiff_dec::TiffDecoder.open(Arc::new(part))?;
        let mut info = inner.info().clone();
        info.format = format;
        info.coding = format!("TIFF preview, {}", info.coding);
        return Ok(Arc::new(EpsTiffSession {
            source,
            info,
            inner,
        }));
    }
    let ps = eps::postscript(data);
    let epsi = eps::epsi_preview(ps);
    // A Windows Metafile preview through the metafile decoder (EPSI, a bitmap, is preferred
    // when both are there: the metafile is often just a placeholder box).
    if epsi.is_none()
        && let Some(wmf) = dos.as_ref().and_then(|d| d.wmf.clone())
    {
        let part = Source::from_slice(source.path(), source.clone(), &data[wmf])
            .ok_or_else(|| DecodeError::Malformed("EPS WMF preview".into()))?;
        let inner = crate::metafile::MetafileDecoder(crate::metafile::Kind::Wmf)
            .open(Arc::new(part))
            .map_err(|e| {
                DecodeError::Unsupported(format!(
                    "EPS whose Windows Metafile preview can't be read ({e}): PostScript is not rendered"
                ))
            })?;
        let mut info = inner.info().clone();
        info.format = format;
        info.coding = format!("WMF preview, {}", info.coding);
        return Ok(Arc::new(EpsTiffSession {
            source,
            info,
            inner,
        }));
    }
    if let Some(p) = epsi {
        let info = ImageInfo {
            format,
            width: p.width,
            height: p.height,
            orientation: Orientation::NORMAL,
            icc_profile: None,
            has_alpha: false,
            bit_depth: p.depth,
            progressive: false,
            coding: format!("EPSI preview, {}-bit", p.depth),
            embedded_preview: None,
            fast_regions: false,
            scales: &[1, 2, 4, 8],
        };
        return Ok(Arc::new(EpsiSession {
            source,
            info,
            gray: p.gray,
            levels: Levels::new(),
        }));
    }
    Err(DecodeError::Unsupported(
        "EPS without a preview: PostScript is not rendered".into(),
    ))
}

/// A DOS EPS file shown through its TIFF preview.
struct EpsTiffSession {
    source: Arc<Source>,
    info: ImageInfo,
    inner: Arc<dyn Session>,
}

impl Session for EpsTiffSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn source(&self) -> &Arc<Source> {
        &self.source
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        None
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        self.inner.decode(req, cancel)
    }
    fn prepare_regions(&self, cancel: &CancelToken) -> Result<()> {
        self.inner.prepare_regions(cancel)
    }
    fn regions_ready(&self) -> bool {
        self.inner.regions_ready()
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        self.inner.decode_memory(req)
    }
    fn memory_bytes(&self) -> usize {
        self.inner.memory_bytes()
    }
    fn trim_memory(&self) -> usize {
        self.inner.trim_memory()
    }
    fn colour_model(&self) -> Option<&'static str> {
        self.inner.colour_model()
    }
    fn alpha(&self) -> Option<crate::Alpha> {
        self.inner.alpha()
    }
}

/// An EPS file shown through its EPSI preview (decoded at open; previews are small).
struct EpsiSession {
    source: Arc<Source>,
    info: ImageInfo,
    gray: Vec<u8>,
    levels: Levels,
}

impl Session for EpsiSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn alpha(&self) -> Option<crate::Alpha> {
        None
    }
    fn source(&self) -> &Arc<Source> {
        &self.source
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        None
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        let (w, h) = (self.info.width, self.info.height);
        self.levels.decode(
            req,
            (w, h),
            |_| 1,
            |_| {
                Ok(whole::interleaved(
                    w,
                    h,
                    PixelLayout::Gray,
                    self.gray.clone(),
                    false,
                ))
            },
            cancel,
        )
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        let (w, h) = (self.info.width, self.info.height);
        whole::level_bytes(w, h, req.scale.max(1), 1) + whole::level_bytes(w, h, 1, 1)
    }
    fn memory_bytes(&self) -> usize {
        self.gray.len() + self.levels.memory_bytes()
    }
    fn trim_memory(&self) -> usize {
        self.levels.clear()
    }
    fn colour_model(&self) -> Option<&'static str> {
        Some("Greyscale")
    }
}
