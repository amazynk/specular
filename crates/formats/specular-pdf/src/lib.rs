//! PDF and Adobe Illustrator pages through hayro, and EPS previews.
//!
//! [`Document`] wraps a parsed `hayro` PDF: page count, page sizes in points, document info and
//! XMP, and [`Document::render`], which rasterises any rectangle of a page at any scale onto an
//! opaque white background. Regions render directly (a translated transform), so deep zoom and
//! tiles cost only the pixels asked for.
//!
//! Bounds: hayro reads only the bytes it is given. It has no JavaScript, no network and no
//! external-file access, and with the default settings it never looks up a system font (the
//! standard 14 fonts and the predefined CMaps are compiled in). Renders run on their own thread
//! so a pathological page can be timed out or cancelled. hayro cannot be interrupted, so a
//! timed-out render is abandoned: it finishes in the background and is counted, and while
//! [`MAX_ABANDONED`] such renders are still running new ones wait, then fail.
//!
//! [`eps`] reads the previews of Encapsulated PostScript files: the TIFF or WMF section of a DOS
//! EPS binary header, and the EPSI hex preview in the comments. PostScript itself is never run.

pub mod annotate;
pub mod eps;
#[doc(hidden)]
pub mod fixture;
pub mod geo;
mod guard;
pub mod measure;
mod text;
pub mod textdoc;

use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use hayro::hayro_interpret::{InterpreterSettings, TransformExt};
use hayro::hayro_syntax::object::{Dict, Stream};
use hayro::hayro_syntax::{DecryptionError, LoadPdfError, Pdf, PdfVersion};
use hayro::kurbo::Affine;
use hayro::vello_cpu::{self, PixmapMut, RasterizerSettings, RenderContext, TargetInit};
use hayro::{RenderCache, RenderSettings};

pub use text::{Info, decode_text_string};

/// The largest side a single render may have (vello_cpu's pixmaps are `u16` wide).
pub const MAX_RENDER_SIDE: u32 = u16::MAX as u32;
/// The largest single render, in pixels (1 GiB of RGBA).
pub const MAX_RENDER_PIXELS: u64 = 256 * 1024 * 1024;
/// Default time one render may take before it is abandoned.
pub const RENDER_TIMEOUT: Duration = Duration::from_secs(30);
/// Abandoned (timed-out or cancelled) renders that may still be running before new renders are
/// refused.
pub const MAX_ABANDONED: usize = 4;
/// The largest XMP packet read from the document catalog.
const MAX_XMP_BYTES: usize = 4 * 1024 * 1024;
/// Stack of a render thread: content streams nest (forms in patterns in forms…).
const RENDER_STACK: usize = 64 * 1024 * 1024;

/// Why a document cannot be opened.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OpenError {
    /// The document needs a password to be read.
    #[error("password-protected PDF: not shown")]
    Protected,
    /// The document is encrypted in a way that cannot be read.
    #[error("encrypted PDF with an unsupported or damaged encryption")]
    UnsupportedEncryption,
    /// Not a readable PDF.
    #[error("damaged or invalid PDF")]
    Invalid,
    /// A readable PDF without pages.
    #[error("PDF without pages")]
    NoPages,
}

/// Why a render failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RenderError {
    /// No such page.
    #[error("page {0} does not exist")]
    NoSuchPage(usize),
    /// The requested raster is empty or larger than the render bounds.
    #[error("page raster {0}×{1} is outside the render bounds")]
    TooLarge(u32, u32),
    /// The page took longer than the timeout.
    #[error("page took longer than {0} s to render")]
    Timeout(u64),
    /// The caller cancelled the render.
    #[error("cancelled")]
    Cancelled,
    /// Earlier renders are still running after being abandoned.
    #[error("earlier pages are still rendering")]
    Busy,
    /// The renderer failed (panicked) on this page.
    #[error("the PDF renderer failed on this page")]
    Failed,
    /// The page draws the same shapes too many times over to render (nested forms).
    #[error("the page is too complex to draw")]
    TooComplex,
}

/// The rectangle of a page to render, in output pixels, and the page's scale.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderRequest {
    /// Zero-based page index.
    pub page: usize,
    /// Output pixels per point, horizontally.
    pub scale_x: f64,
    /// Output pixels per point, vertically.
    pub scale_y: f64,
    /// Left edge of the region in the scaled page raster.
    pub x: u32,
    /// Top edge of the region in the scaled page raster.
    pub y: u32,
    /// Region width in pixels.
    pub width: u32,
    /// Region height in pixels.
    pub height: u32,
}

/// A parsed PDF (or PDF-compatible Illustrator) document.
pub struct Document {
    pdf: Arc<Pdf>,
    /// Whether every function is shallow enough to render (see `guard`), found on first render.
    bounded: Arc<OnceLock<bool>>,
    /// Whether each page's work fits [`guard::MAX_PAGE_WORK`], found on its first render.
    work: Arc<std::sync::Mutex<HashMap<usize, bool>>>,
}

impl std::fmt::Debug for Document {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Document")
            .field("pages", &self.page_count())
            .finish()
    }
}

/// Whether `head` starts like a PDF: `%PDF-` within the first 1024 bytes (the specification
/// allows leading garbage, and Illustrator files start with it directly).
pub fn looks_like_pdf(head: &[u8]) -> bool {
    let head = &head[..head.len().min(1024)];
    head.windows(5).any(|w| w == b"%PDF-")
}

impl Document {
    /// Parses a document. An empty user password is tried, so files that are only
    /// owner-protected (printing or copying restrictions) open; anything that needs a password is
    /// [`OpenError::Protected`].
    pub fn open<T: AsRef<[u8]> + Send + Sync + 'static>(data: Arc<T>) -> Result<Self, OpenError> {
        let pdf = Pdf::new(data).map_err(|e| match e {
            LoadPdfError::Decryption(DecryptionError::PasswordProtected) => OpenError::Protected,
            LoadPdfError::Decryption(_) => OpenError::UnsupportedEncryption,
            LoadPdfError::Invalid => OpenError::Invalid,
        })?;
        if pdf.pages().is_empty() {
            return Err(OpenError::NoPages);
        }
        Ok(Self {
            pdf: Arc::new(pdf),
            bounded: Arc::default(),
            work: Arc::default(),
        })
    }

    /// Number of pages (at least one).
    pub fn page_count(&self) -> usize {
        self.pdf.pages().len()
    }

    /// Size of page `index` in points (1/72 inch), with the page's rotation applied; `None` for
    /// a missing page. Degenerate boxes come back as hayro reports them; callers clamp.
    pub fn page_size(&self, index: usize) -> Option<(f32, f32)> {
        self.pdf.pages().get(index).map(|p| p.render_dimensions())
    }

    /// The PDF version, e.g. `"1.7"`.
    pub fn version(&self) -> &'static str {
        match self.pdf.version() {
            PdfVersion::Pdf10 => "1.0",
            PdfVersion::Pdf11 => "1.1",
            PdfVersion::Pdf12 => "1.2",
            PdfVersion::Pdf13 => "1.3",
            PdfVersion::Pdf14 => "1.4",
            PdfVersion::Pdf15 => "1.5",
            PdfVersion::Pdf16 => "1.6",
            PdfVersion::Pdf17 => "1.7",
            PdfVersion::Pdf20 => "2.0",
        }
    }

    /// The document information dictionary, decoded.
    pub fn info(&self) -> Info {
        Info::from_metadata(self.pdf.metadata())
    }

    /// The document's XMP packet: the catalog's `/Metadata` stream when there is one, otherwise
    /// one synthesised from the information dictionary (title, author, subject, keywords,
    /// creator, producer). `None` when there is neither.
    pub fn xmp(&self) -> Option<Vec<u8>> {
        self.catalog_xmp().or_else(|| self.info().to_xmp())
    }

    fn catalog_xmp(&self) -> Option<Vec<u8>> {
        let xref = self.pdf.xref();
        let catalog = xref.get::<Dict<'_>>(xref.root_id())?;
        let stream = catalog.get::<Stream<'_>>(b"Metadata")?;
        if stream.raw_data().len() > MAX_XMP_BYTES {
            return None;
        }
        let data = stream.decoded().ok()?;
        if data.len() > MAX_XMP_BYTES || !data.windows(9).any(|w| w == b"x:xmpmeta") {
            return None;
        }
        Some(data.into_owned())
    }

    /// Renders `req` with the default timeout. See [`Document::render_with`].
    pub fn render(
        &self,
        req: RenderRequest,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, RenderError> {
        self.render_with(req, cancelled, RENDER_TIMEOUT)
    }

    /// Renders a region of a page to straight RGBA8 (`width × height × 4` bytes, every pixel
    /// opaque: pages are composited on white, as printed). `cancelled` is polled while the page
    /// renders; on cancellation or timeout the render is abandoned (see the crate docs).
    pub fn render_with(
        &self,
        req: RenderRequest,
        cancelled: &dyn Fn() -> bool,
        timeout: Duration,
    ) -> Result<Vec<u8>, RenderError> {
        if req.page >= self.page_count() {
            return Err(RenderError::NoSuchPage(req.page));
        }
        let (w, h) = (req.width, req.height);
        if w == 0
            || h == 0
            || w > MAX_RENDER_SIDE
            || h > MAX_RENDER_SIDE
            || u64::from(w) * u64::from(h) > MAX_RENDER_PIXELS
            || !(req.scale_x.is_finite() && req.scale_x > 0.0)
            || !(req.scale_y.is_finite() && req.scale_y > 0.0)
        {
            return Err(RenderError::TooLarge(w, h));
        }
        let start = Instant::now();
        // Throttle behind abandoned renders still burning CPU.
        while ABANDONED.load(Ordering::Acquire) >= MAX_ABANDONED {
            if cancelled() {
                return Err(RenderError::Cancelled);
            }
            if start.elapsed() >= timeout {
                return Err(RenderError::Busy);
            }
            std::thread::sleep(Duration::from_millis(5));
        }

        let pdf = Arc::clone(&self.pdf);
        let bounded = Arc::clone(&self.bounded);
        let work = Arc::clone(&self.work);
        let state = Arc::new(AtomicU8::new(RUNNING));
        let thread_state = Arc::clone(&state);
        let (tx, rx) = mpsc::sync_channel(1);
        let spawned = specular_thread::builder("dec-pdf")
            .stack_size(RENDER_STACK)
            .spawn(move || {
                let _done = DoneGuard(thread_state);
                let out = if !*bounded.get_or_init(|| guard::functions_bounded(&pdf)) {
                    Err(RenderError::Failed)
                } else if !page_fits(&pdf, &work, req.page) {
                    Err(RenderError::TooComplex)
                } else {
                    render_page(&pdf, req).ok_or(RenderError::Failed)
                };
                let _ = tx.send(out);
            });
        if spawned.is_err() {
            return Err(RenderError::Failed);
        }

        let poll = Duration::from_millis(10);
        loop {
            match rx.recv_timeout(poll) {
                Ok(out) => return out,
                Err(mpsc::RecvTimeoutError::Disconnected) => return Err(RenderError::Failed),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
            let why = if cancelled() {
                RenderError::Cancelled
            } else if start.elapsed() >= timeout {
                RenderError::Timeout(timeout.as_secs())
            } else {
                continue;
            };
            // Abandon the thread unless it finished in the meantime.
            if state
                .compare_exchange(RUNNING, ABANDON, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                ABANDONED.fetch_add(1, Ordering::AcqRel);
            }
            return Err(why);
        }
    }
}

const RUNNING: u8 = 0;
const DONE: u8 = 1;
const ABANDON: u8 = 2;

/// Renders abandoned by their caller that are still running.
static ABANDONED: AtomicUsize = AtomicUsize::new(0);

/// Marks a render thread finished (also when it unwinds), releasing its abandoned slot.
struct DoneGuard(Arc<AtomicU8>);

impl Drop for DoneGuard {
    fn drop(&mut self) {
        if self.0.swap(DONE, Ordering::AcqRel) == ABANDON {
            ABANDONED.fetch_sub(1, Ordering::AcqRel);
        }
    }
}

/// Renders abandoned renders still running (for tests and diagnostics).
pub fn abandoned_renders() -> usize {
    ABANDONED.load(Ordering::Acquire)
}

/// Whether page `index` runs at most [`guard::MAX_PAGE_WORK`] instructions (found once per
/// page).
fn page_fits(pdf: &Pdf, known: &std::sync::Mutex<HashMap<usize, bool>>, index: usize) -> bool {
    if let Some(&fits) = known
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(&index)
    {
        return fits;
    }
    let fits = pdf
        .pages()
        .get(index)
        .is_none_or(|p| guard::page_work(p) <= guard::MAX_PAGE_WORK);
    known
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(index, fits);
    fits
}

fn render_page(pdf: &Pdf, req: RenderRequest) -> Option<Vec<u8>> {
    let page = pdf.pages().get(req.page)?;
    let (w, h) = (
        u16::try_from(req.width).ok()?,
        u16::try_from(req.height).ok()?,
    );
    let transform = Affine::translate((-f64::from(req.x), -f64::from(req.y)))
        * Affine::scale_non_uniform(req.scale_x, req.scale_y)
        * page.initial_transform(true).to_kurbo();

    let len = usize::from(w) * usize::from(h) * 4;
    let mut buf = Vec::new();
    buf.try_reserve_exact(len).ok()?;
    buf.resize(len, 0);

    let cache = RenderCache::new();
    let settings = InterpreterSettings::default();
    let mut ctx = RenderContext::new(w, h);
    hayro::render_into(
        page,
        &cache,
        &settings,
        &RenderSettings::default(),
        &mut ctx,
        transform,
    );
    ctx.flush();
    ctx.render_with(
        PixmapMut::new(w, h, &mut buf)?,
        &mut vello_cpu::Resources::default(),
        RasterizerSettings {
            target_init: TargetInit::Clear(vello_cpu::color::palette::css::WHITE),
            ..Default::default()
        },
    );
    // Premultiplied over opaque white is already straight; pin alpha so nothing downstream
    // treats a page as transparent.
    for px in buf.chunks_exact_mut(4) {
        px[3] = 255;
    }
    Some(buf)
}

#[cfg(test)]
mod tests;
