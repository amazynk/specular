//! EPUB 2 and 3 books (`specular-epub`): every laid-out page is a session page, from the
//! cover. Reflowable books are laid out at 600 × 900 CSS px with the bundled fonts and the
//! book's own (the same pages on every machine); fixed-layout pages keep their own size.
//! Pictures in the book are decoded with our own decoders.
//!
//! Like PDF, a page's full raster is the page at 288 dpi (CSS px are 0.75 pt), doubled
//! until crisp and capped ([`crate::pdf_dec::raster_size`]); every scale and region renders
//! from the laid-out page. The table of contents ([`Session::contents`]) gives each entry's
//! page. Locked books (DRM) are refused with the scheme's name.

use std::sync::{Arc, OnceLock};

use specular_core::{CancelToken, Orientation, PixelLayout, Raster, RasterPlane, Source};
use specular_epub::{Book, Direction, Layout, PageSetup, Pages};

use crate::{
    BookModel, ContentsEntry, DecodeError, DecodeRequest, Decoder, Format, ImageInfo, PageLabel,
    Rect, Result, Session,
};

/// CSS px per point.
const PX_PER_PT: f32 = 4.0 / 3.0;

/// The largest picture decoded for a page (pixels).
const MAX_PICTURE_PIXELS: u64 = 64_000_000;

/// RGBA rows `w` × `h` turned upright as `o` says (a flip first, then quarter turns
/// clockwise).
fn oriented(o: Orientation, w: u32, h: u32, rgba: Vec<u8>) -> (u32, u32, Vec<u8>) {
    let (turns, flip) = o.to_rotation_flip();
    if turns == 0 && !flip {
        return (w, h, rgba);
    }
    let (w, h) = (w as usize, h as usize);
    let px = |x: usize, y: usize| {
        let x = if flip { w - 1 - x } else { x };
        let i = (y * w + x) * 4;
        [rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]]
    };
    let (ow, oh) = if turns % 2 == 1 { (h, w) } else { (w, h) };
    let mut out = Vec::with_capacity(ow * oh * 4);
    for oy in 0..oh {
        for ox in 0..ow {
            // The stored pixel that lands at (ox, oy).
            let (x, y) = match turns {
                1 => (oy, h - 1 - ox),
                2 => (w - 1 - ox, h - 1 - oy),
                3 => (w - 1 - oy, ox),
                _ => (ox, oy),
            };
            out.extend_from_slice(&px(x, y));
        }
    }
    (ow as u32, oh as u32, out)
}

/// The book's pictures through our decoders, at the smallest native scale still as large as
/// asked.
/// Decodes the book's pictures; `0` is the book (pictures are nested inside it).
struct Pictures(Arc<Source>);

/// A file name for a picture of `media_type`, so formats chosen by extension (SVG) open.
fn picture_name(media_type: &str) -> &'static str {
    match media_type {
        "image/svg+xml" => "picture.svg",
        "image/jpeg" => "picture.jpg",
        "image/png" => "picture.png",
        "image/gif" => "picture.gif",
        "image/webp" => "picture.webp",
        "image/avif" => "picture.avif",
        "image/jxl" => "picture.jxl",
        _ => "picture",
    }
}

impl specular_epub::ImageDecoder for Pictures {
    fn decode(
        &self,
        bytes: &[u8],
        media_type: &str,
        max_w: u32,
        max_h: u32,
    ) -> Option<specular_epub::Image> {
        let session = crate::svg::registry()
            .open(Arc::new(
                Source::from_vec(picture_name(media_type), bytes.to_vec()).within(&self.0),
            ))
            .ok()?;
        let info = session.info();
        let (w, h) = info.display_size();
        if u64::from(w) * u64::from(h) > MAX_PICTURE_PIXELS {
            specular_log::debug!(target: specular_log::DECODE, "EPUB picture of {w}×{h} px not loaded: too large");
            return None;
        }
        // The coarsest native scale that still covers the box asked for.
        let scale = info
            .scales
            .iter()
            .copied()
            .filter(|&s| w.div_ceil(s) >= max_w.min(w) && h.div_ceil(s) >= max_h.min(h))
            .max()
            .unwrap_or(1);
        let r = session
            .decode(&DecodeRequest::full(scale), &CancelToken::new())
            .ok()?;
        let (rw, rh, rgba) = crate::svg::to_rgba8(r)?;
        let (rw, rh, rgba) = oriented(info.orientation, rw, rh, rgba);
        Some(specular_epub::Image {
            width: rw,
            height: rh,
            rgba,
        })
    }

    fn size(&self, bytes: &[u8], media_type: &str) -> Option<(u32, u32)> {
        let session = crate::svg::registry()
            .open(Arc::new(
                Source::from_vec(picture_name(media_type), bytes.to_vec()).within(&self.0),
            ))
            .ok()?;
        Some(session.info().display_size())
    }
}

pub struct EpubDecoder;

impl Decoder for EpubDecoder {
    fn format(&self) -> Format {
        Format::Epub
    }
    /// No signature of its own (a ZIP package): the `mimetype` member or the container.
    fn probe(&self, head: &[u8]) -> bool {
        specular_epub::is_epub(head)
    }
    fn probe_file(&self, data: &[u8]) -> bool {
        specular_epub::is_epub(data)
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["epub"]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let book = Book::open(source.clone(), &Default::default()).map_err(map_err)?;
        open_book(book, source, Format::Epub)
    }
}

fn map_err(e: specular_epub::Error) -> DecodeError {
    use specular_epub::Error as E;
    match e {
        E::Drm(_) | E::NotEpub | E::Unsupported(_) => DecodeError::Unsupported(e.to_string()),
        E::TooLarge(_) => DecodeError::TooLarge,
        E::Cancelled => DecodeError::Cancelled,
        e => DecodeError::Malformed(e.to_string()),
    }
}

/// Whether `d` is a ZIP whose first member is an `.fb2` (a `.fb2.zip`).
fn zipped_fb2(d: &[u8]) -> bool {
    if !d.starts_with(b"PK\x03\x04") || d.len() < 30 {
        return false;
    }
    let n = usize::from(u16::from_le_bytes([d[26], d[27]]));
    d.get(30..30 + n)
        .is_some_and(|name| name.to_ascii_lowercase().ends_with(b".fb2"))
}

/// Mobipocket, Kindle and FictionBook books: rewritten as EPUB (`specular-epub`), then shown
/// like one.
pub struct EbookDecoder(pub Format);

impl EbookDecoder {
    fn book(&self, data: &[u8]) -> std::result::Result<Book, specular_epub::Error> {
        match self.0 {
            Format::Fb2 => Book::open_fb2(data, &Default::default()),
            _ => Book::open_mobi(data, &Default::default()),
        }
    }
}

impl Decoder for EbookDecoder {
    fn format(&self) -> Format {
        self.0
    }
    fn probe(&self, head: &[u8]) -> bool {
        match self.0 {
            Format::Fb2 => specular_epub::is_fb2(head) || zipped_fb2(head),
            _ => specular_epub::is_mobi(head) || specular_epub::is_kfx(head),
        }
    }
    /// The Palm database's type sits at byte 60, past the usual probe head.
    fn probe_file(&self, data: &[u8]) -> bool {
        self.probe(&data[..data.len().min(4096)])
    }
    fn extensions(&self) -> &'static [&'static str] {
        match self.0 {
            Format::Fb2 => &["fb2", "fb2.zip"],
            _ => &["mobi", "prc", "azw", "azw3"],
        }
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let book = self.book(source.bytes()).map_err(map_err)?;
        open_book(book, source, self.0)
    }
}

/// A Mobipocket, Kindle or FictionBook book's cover standing for it (thumbnails), as
/// [`open_cover`] does for EPUB.
pub(crate) fn ebook_cover(source: &Arc<Source>) -> Option<Arc<dyn Session>> {
    let d = source.bytes();
    let format = if specular_epub::is_mobi(d) {
        Format::Mobi
    } else if specular_epub::is_fb2(d) || zipped_fb2(d) {
        Format::Fb2
    } else {
        return None;
    };
    let book = EbookDecoder(format).book(d).ok()?;
    cover_session(source, &book, format)
}

pub(crate) fn open_book(
    book: Book,
    source: Arc<Source>,
    format: Format,
) -> Result<Arc<dyn Session>> {
    let setup = PageSetup::default();
    let pages = book
        .paginate(&setup, Arc::new(Pictures(source.clone())), &|| false)
        .map_err(map_err)?;
    if pages.is_empty() {
        return Err(DecodeError::Malformed(
            "the book has no pages that can be shown".into(),
        ));
    }
    let contents: Arc<[ContentsEntry]> = book
        .toc()
        .iter()
        .enumerate()
        .map(|(i, e)| ContentsEntry {
            title: e.title.trim().into(),
            depth: e.depth,
            page: pages.toc_page(i).and_then(|p| u32::try_from(p).ok()),
        })
        .collect();
    let model = book_model(&book, &pages);
    let shared = Arc::new(Shared {
        book,
        pages,
        contents,
        model,
        source,
        format,
        chapters: OnceLock::new(),
    });
    Ok(Arc::new(EpubSession::new(shared, 0)?))
}

fn book_model(book: &Book, pages: &Pages) -> BookModel {
    let m = book.metadata();
    let fonts = book.font_paths().len();
    BookModel {
        title: m.title.clone(),
        authors: m
            .creators
            .iter()
            .filter(|c| c.role.as_deref().is_none_or(|r| r == "aut"))
            .map(|c| c.name.clone())
            .collect(),
        publisher: m.publisher.clone(),
        language: m.languages.first().cloned(),
        date: m.date.clone(),
        identifier: m.identifier.clone(),
        subjects: m.subjects.clone(),
        rights: m.rights.clone(),
        version: Some(m.version.clone()).filter(|v| !v.is_empty()),
        fixed_layout: m.layout == Layout::PrePaginated,
        right_to_left: m.direction == Direction::Rtl,
        contents: book.toc().len(),
        pages: pages.len(),
        fonts,
        obfuscated_fonts: book.obfuscated_fonts().len(),
    }
}

struct Shared {
    book: Book,
    pages: Pages,
    contents: Arc<[ContentsEntry]>,
    model: BookModel,
    source: Arc<Source>,
    /// EPUB, or the format rewritten as EPUB (MOBI, FB2).
    format: Format,
    /// The chapter title of every page (from the contents), made once.
    chapters: OnceLock<Vec<Option<Arc<str>>>>,
}

impl Shared {
    fn chapter(&self, page: usize) -> Option<Arc<str>> {
        let titles = self.chapters.get_or_init(|| {
            let mut out: Vec<Option<Arc<str>>> = vec![None; self.pages.len()];
            // Entries in reading order: each one names the pages from its own to the next.
            let mut starts: Vec<(usize, Arc<str>)> = self
                .contents
                .iter()
                .filter_map(|e| Some((e.page? as usize, e.title.clone())))
                .filter(|(_, t)| !t.is_empty())
                .collect();
            starts.sort_by_key(|s| s.0);
            for (k, (p, t)) in starts.iter().enumerate() {
                let end = starts
                    .get(k + 1)
                    .map_or(out.len(), |n| n.0.max(*p + 1))
                    .min(out.len());
                for slot in out.iter_mut().take(end).skip(*p) {
                    *slot = Some(t.clone());
                }
            }
            out
        });
        titles.get(page).cloned().flatten()
    }
}

struct EpubSession {
    shared: Arc<Shared>,
    page: u32,
    info: ImageInfo,
    /// The page's size in CSS px.
    css: (f32, f32),
}

impl EpubSession {
    fn new(shared: Arc<Shared>, page: u32) -> Result<EpubSession> {
        let css = shared
            .pages
            .page_size(page as usize)
            .ok_or_else(|| DecodeError::Unsupported(format!("no page {page}")))?;
        let (pw, ph) = (css.0 / PX_PER_PT, css.1 / PX_PER_PT);
        let (w, h, k) = crate::pdf_dec::raster_size(pw, ph)
            .ok_or_else(|| DecodeError::Malformed("page without a size".into()))?;
        let m = shared.book.metadata();
        let layout = if m.layout == Layout::PrePaginated {
            "fixed layout"
        } else {
            "reflowable"
        };
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
                "{}, {layout}, {:.0}×{:.0} px page, rendered at {:.0} dpi",
                match shared.format {
                    Format::Epub => format!("EPUB {}", m.version),
                    f => f.name().to_owned(),
                },
                css.0,
                css.1,
                k * 72.0
            ),
            embedded_preview: None,
            fast_regions: true,
            scales: &[1, 2, 4, 8],
        };
        Ok(EpubSession {
            shared,
            page,
            info,
            css,
        })
    }
}

impl Session for EpubSession {
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
        let scale = req.scale.clamp(1, 8);
        let (sw, sh) = (
            self.info.width.div_ceil(scale),
            self.info.height.div_ceil(scale),
        );
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
        // Device px per CSS px at this scale.
        let k = sw as f32 / self.css.0;
        let img = self
            .shared
            .pages
            .render(self.page as usize, k, Some((x, y, w, h)))
            .map_err(|e| match e {
                specular_epub::Error::Cancelled => DecodeError::Cancelled,
                specular_epub::Error::TooLarge(_) => DecodeError::TooLarge,
                e => DecodeError::Malformed(e.to_string()),
            })?;
        if img.width != w || img.height != h || img.rgba.len() != w as usize * h as usize * 4 {
            return Err(DecodeError::Malformed(
                "page rendered at the wrong size".into(),
            ));
        }
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
                data: img.rgba,
                h_div: 1,
                v_div: 1,
            }],
            truncated: false,
        })
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        let s = req.scale.clamp(1, 8);
        let px = match req.region {
            Some(r) => u64::from(r.w) * u64::from(r.h),
            None => {
                u64::from(self.info.width.div_ceil(s)) * u64::from(self.info.height.div_ceil(s))
            }
        };
        // The output, the rasteriser's canvas and the page's decoded pictures.
        px * 4 * 2 + (32 << 20)
    }
    fn colour_model(&self) -> Option<&'static str> {
        Some("RGB")
    }
    fn page_count(&self) -> u32 {
        u32::try_from(self.shared.pages.len()).unwrap_or(u32::MAX)
    }
    fn page_index(&self) -> u32 {
        self.page
    }
    fn page_label(&self) -> Option<PageLabel> {
        self.shared
            .chapter(self.page as usize)
            .map(|title| PageLabel::Chapter { title })
    }
    fn open_page(&self, page: u32) -> Result<Arc<dyn Session>> {
        Ok(Arc::new(EpubSession::new(self.shared.clone(), page)?))
    }
    fn contents(&self) -> Option<Arc<[ContentsEntry]>> {
        (!self.shared.contents.is_empty()).then(|| self.shared.contents.clone())
    }
    fn book(&self) -> Option<&BookModel> {
        Some(&self.shared.model)
    }
    fn embedded_xmp(&self) -> Option<Vec<u8>> {
        let m = &self.shared.model;
        specular_pdf::Info {
            title: m.title.clone(),
            author: Some(m.authors.join(", ")).filter(|a| !a.is_empty()),
            subject: None,
            keywords: Some(m.subjects.join(", ")).filter(|s| !s.is_empty()),
            creator: None,
            producer: None,
        }
        .to_xmp()
    }
}

/// A book's cover picture standing for the book (thumbnails): no page is laid out, which
/// for a long book takes seconds. `None` when the book has no cover picture or it does not
/// decode (the first page is then laid out as usual).
pub(crate) fn open_cover(source: &Arc<Source>) -> Option<Arc<dyn Session>> {
    let book = Book::open(source.clone(), &Default::default()).ok()?;
    cover_session(source, &book, Format::Epub)
}

fn cover_session(source: &Arc<Source>, book: &Book, format: Format) -> Option<Arc<dyn Session>> {
    let (part, media_type) = book.cover()?;
    let name = source.path().with_file_name(picture_name(&media_type));
    // The picture's bytes keep their reservation for as long as the source holds them.
    let (bytes, held) = part.into_parts();
    let inner = crate::svg::registry()
        .open(Arc::new(
            Source::from_vec(name, bytes)
                .with_reservation(held)
                .within(source),
        ))
        .ok()?;
    let mut info = inner.info().clone();
    info.format = format;
    info.coding = match format {
        Format::Epub => format!("EPUB {}, cover", book.metadata().version),
        f => format!("{}, cover", f.name()),
    };
    Some(Arc::new(CoverSession {
        inner,
        info,
        source: source.clone(),
    }))
}

struct CoverSession {
    inner: Arc<dyn Session>,
    info: ImageInfo,
    source: Arc<Source>,
}

impl Session for CoverSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn source(&self) -> &Arc<Source> {
        &self.source
    }
    fn alpha(&self) -> Option<crate::Alpha> {
        self.inner.alpha()
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        self.inner.decode_preview()
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        self.inner.decode(req, cancel)
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        self.inner.decode_memory(req)
    }
    fn colour_model(&self) -> Option<&'static str> {
        self.inner.colour_model()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pictures_turn_upright() {
        // 2 × 1: red, green.
        let px = vec![255, 0, 0, 255, 0, 255, 0, 255];
        // A quarter turn clockwise (EXIF 6): red on top.
        let (w, h, d) = oriented(Orientation(6), 2, 1, px.clone());
        assert_eq!((w, h), (1, 2));
        assert_eq!(&d[..4], &[255, 0, 0, 255]);
        // Mirrored (EXIF 2): green first.
        let (_, _, d) = oriented(Orientation(2), 2, 1, px.clone());
        assert_eq!(&d[..4], &[0, 255, 0, 255]);
        // Three quarter turns (EXIF 8): green on top.
        let (_, _, d) = oriented(Orientation(8), 2, 1, px);
        assert_eq!(&d[..4], &[0, 255, 0, 255]);
    }
}
