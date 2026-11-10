//! EPUB 2 and 3 books.
//!
//! [`Book::open`] reads the package only (container, OPF, table of contents, encryption):
//! metadata, cover and contents come at once, and a book locked with DRM is refused with the
//! scheme named. Fonts obfuscated with the IDPF or Adobe algorithm are de-obfuscated when
//! read; any other encrypted part refuses the whole book.
//!
//! [`Book::paginate`] lays the spine out into pages of a fixed size with an own block layout
//! (paragraphs, headings, emphasis, lists, simple tables, images, a small CSS cascade), the
//! text stack's shaping and line breaking (parley) inside each paragraph, and the book's
//! embedded fonts. Fixed-layout (pre-paginated) documents keep their page size; a page that
//! is just a picture is that picture, fitted. Page 0 is the cover. [`Pages::render`] draws a
//! page, or a region of it, at any scale on the CPU (glyph outlines through skrifa, filled
//! with tiny-skia), from any thread.

mod convert;
mod css;
mod fonts;
mod layout;
mod package;
mod render;
mod style;
mod xml;

use std::sync::Arc;

use specular_archive::{Archive, ByteSource};

pub use convert::fb2::is_fb2;
pub use convert::html::{HtmlDoc, HtmlSection, escape as html_escape};
pub use convert::mobi::{is_kfx, is_mobi};
pub use layout::Pages;

/// Largest XHTML, CSS, NCX or OPF part read.
const MAX_TEXT_PART: u64 = 64 << 20;
/// Largest font or image part read.
const MAX_BINARY_PART: u64 = 256 << 20;

/// Why a book cannot be shown, in plain words.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum Error {
    #[error("this is not an EPUB book")]
    NotEpub,
    #[error("the book is damaged: {0}")]
    Damaged(String),
    #[error("the book has no pages that can be shown")]
    NoPages,
    #[error("{}", .0.message())]
    Drm(DrmScheme),
    #[error("the book is too large: {0}")]
    TooLarge(String),
    #[error("cancelled")]
    Cancelled,
    #[error("there is no page {0}")]
    NoSuchPage(usize),
    /// A book of a kind this build can't read, said why.
    #[error("{0}")]
    Unsupported(String),
}

/// A DRM scheme a locked book uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrmScheme {
    /// Adobe ADEPT (Adobe Digital Editions; also Barnes & Noble, Google Play, library loans).
    Adobe,
    /// Apple FairPlay (Apple Books).
    FairPlay,
    /// Readium LCP.
    Lcp,
    /// Kobo's own DRM.
    Kobo,
    /// Mobipocket or Amazon Kindle DRM.
    Kindle,
    /// Encrypted, scheme not recognised.
    Unknown,
}

impl DrmScheme {
    pub fn name(self) -> &'static str {
        match self {
            DrmScheme::Adobe => "Adobe DRM (ADEPT)",
            DrmScheme::FairPlay => "Apple FairPlay DRM",
            DrmScheme::Lcp => "Readium LCP DRM",
            DrmScheme::Kobo => "Kobo DRM",
            DrmScheme::Kindle => "Kindle DRM",
            DrmScheme::Unknown => "an unknown DRM scheme",
        }
    }

    fn message(self) -> String {
        match self {
            DrmScheme::Unknown => {
                "the book is encrypted with an unknown scheme and cannot be opened".into()
            }
            s => format!(
                "the book is locked with {} and can only be read in the shop's own app",
                s.name()
            ),
        }
    }
}

/// Opening options.
#[derive(Debug, Clone, Default)]
pub struct Options {
    /// Archive limits (entries, sizes, ratios).
    pub archive: specular_archive::Limits,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Creator {
    pub name: String,
    /// MARC relator code (`aut`, `ill`, `edt`…) when given.
    pub role: Option<String>,
    pub file_as: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Layout {
    #[default]
    Reflowable,
    /// Fixed layout: every document is one page of its own size.
    PrePaginated,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Direction {
    #[default]
    Default,
    Ltr,
    /// Pages turn right to left (manga, Arabic, Hebrew, vertical Japanese).
    Rtl,
}

/// What the package says about the book.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Metadata {
    pub title: Option<String>,
    pub creators: Vec<Creator>,
    pub publisher: Option<String>,
    pub languages: Vec<String>,
    pub date: Option<String>,
    /// The package's unique identifier.
    pub identifier: Option<String>,
    pub identifiers: Vec<String>,
    /// Plain text (markup removed).
    pub description: Option<String>,
    pub subjects: Vec<String>,
    pub rights: Option<String>,
    pub series: Option<String>,
    /// The package version: "2.0", "3.0", "3.3"…
    pub version: String,
    pub layout: Layout,
    pub direction: Direction,
    /// `dcterms:modified` (EPUB 3).
    pub modified: Option<String>,
}

/// One entry of the table of contents.
#[derive(Debug, Clone, PartialEq)]
pub struct TocEntry {
    pub title: String,
    /// 0 for the top level.
    pub depth: u32,
    /// The spine document it points into (index into the reading order); `None` when the
    /// target is not in the spine (or the entry is a heading without a link).
    pub spine: Option<usize>,
    pub fragment: Option<String>,
    /// The target's path in the archive ("" without a target).
    pub href: String,
}

/// A decoded picture, straight (not premultiplied) RGBA8.
#[derive(Debug, Clone, PartialEq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Decodes the book's pictures (the viewer's own decoders).
pub trait ImageDecoder: Send + Sync {
    /// The picture in `bytes`, at most about `max_w` × `max_h` (it may be larger; the
    /// page scales it), or `None` when it cannot be decoded.
    fn decode(&self, bytes: &[u8], media_type: &str, max_w: u32, max_h: u32) -> Option<Image>;

    /// The picture's pixel size, cheaply if the format allows (by default it is decoded).
    fn size(&self, bytes: &[u8], media_type: &str) -> Option<(u32, u32)> {
        self.decode(bytes, media_type, 4096, 4096)
            .map(|i| (i.width, i.height))
    }
}

/// How pages are laid out.
#[derive(Debug, Clone, PartialEq)]
pub struct PageSetup {
    /// Reflowable page size in CSS px.
    pub width: f32,
    pub height: f32,
    /// Page margin (CSS px) of reflowable pages.
    pub margin: f32,
    /// Base font size (CSS px).
    pub font_size: f32,
    /// Look in the platform's fonts (else the bundled fonts and the book's only: the same
    /// pages everywhere).
    pub system_fonts: bool,
}

impl Default for PageSetup {
    fn default() -> Self {
        PageSetup {
            width: 600.0,
            height: 900.0,
            margin: 40.0,
            font_size: 16.0,
            system_fonts: false,
        }
    }
}

/// Whether `bytes` look like an EPUB: a ZIP whose first entry is `mimetype` holding
/// `application/epub+zip`, or a ZIP whose directory names `META-INF/container.xml` and an
/// `.opf` package.
pub fn is_epub(bytes: &[u8]) -> bool {
    if !bytes.starts_with(b"PK\x03\x04") {
        return false;
    }
    if bytes.len() > 58 && &bytes[30..38] == b"mimetype" {
        let tail = &bytes[38..bytes.len().min(38 + 80)];
        if tail
            .windows(20)
            .take(40)
            .any(|w| w == b"application/epub+zip")
        {
            return true;
        }
    }
    // The central directory sits at the end.
    let from = bytes.len().saturating_sub(1 << 20);
    let dir = &bytes[from..];
    let has = |n: &[u8]| dir.windows(n.len()).any(|w| w == n);
    has(b"META-INF/container.xml") && has(b".opf") && !has(b"ComicInfo.xml")
}

pub(crate) struct Inner {
    archive: Archive,
    pkg: package::Package,
}

/// An entry's bytes, with the memory budget's share of them: the reservation is held as
/// long as the bytes are (a font kept by the text stack keeps it too).
pub struct Part {
    bytes: Vec<u8>,
    held: specular_core::Reservation,
}

impl Part {
    /// The bytes, giving up the reservation.
    pub fn into_vec(self) -> Vec<u8> {
        self.bytes
    }

    /// The bytes and their reservation.
    pub fn into_parts(self) -> (Vec<u8>, specular_core::Reservation) {
        (self.bytes, self.held)
    }

    /// Bytes reserved for this part.
    pub fn reserved(&self) -> u64 {
        self.held.bytes()
    }
}

impl std::ops::Deref for Part {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        &self.bytes
    }
}

impl std::fmt::Debug for Part {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Part")
            .field("len", &self.bytes.len())
            .field("reserved", &self.held.bytes())
            .finish()
    }
}

impl PartialEq<Vec<u8>> for Part {
    fn eq(&self, other: &Vec<u8>) -> bool {
        self.bytes == *other
    }
}

/// Entry `i` of `archive` if it is at most `max` bytes, read with its reservation.
fn read_part(archive: &Archive, i: usize, max: u64) -> Option<Part> {
    if archive.entries()[i].size > max {
        return None;
    }
    let (bytes, held) = archive.read_reserved(i).ok()?;
    Some(Part { bytes, held })
}

impl Inner {
    /// An entry's bytes, de-obfuscated when it is an obfuscated font.
    pub(crate) fn read(&self, path: &str, max: u64) -> Option<Part> {
        let i = self.archive.find(path)?;
        let mut d = read_part(&self.archive, i, max)?;
        if let Some(o) = self.pkg.obfuscated.get(path) {
            o.undo(&mut d.bytes);
        }
        Some(d)
    }
}

struct ArchiveReader<'a>(&'a Archive);

impl package::Reader for ArchiveReader<'_> {
    fn read(&self, path: &str) -> Option<Part> {
        read_part(self.0, self.0.find(path)?, MAX_TEXT_PART)
    }

    fn exists(&self, path: &str) -> bool {
        self.0.find(path).is_some()
    }
}

/// An opened book: package, metadata, contents. Cheap to clone (shared).
#[derive(Clone)]
pub struct Book {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for Book {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Book")
            .field("title", &self.inner.pkg.metadata.title)
            .field("spine", &self.inner.pkg.spine.len())
            .finish()
    }
}

impl Book {
    /// Opens a Mobipocket, Kindle (MOBI, AZW, AZW3) or PalmDOC book, rewritten as EPUB.
    pub fn open_mobi(data: &[u8], opts: &Options) -> Result<Book, Error> {
        Book::open(convert::mobi::to_epub(data)?, opts)
    }

    /// Opens a FictionBook 2 book (FB2, or one zipped alone), rewritten as EPUB.
    pub fn open_fb2(data: &[u8], opts: &Options) -> Result<Book, Error> {
        Book::open(convert::fb2::to_epub(data)?, opts)
    }

    /// Opens a document another reader wrote as XHTML (Markdown, RTF), packaged as EPUB.
    pub fn open_html(doc: &HtmlDoc, opts: &Options) -> Result<Book, Error> {
        Book::open(convert::html::to_epub(doc)?, opts)
    }

    /// Reads the package of the EPUB in `data` (no page is laid out).
    pub fn open(data: impl ByteSource, opts: &Options) -> Result<Book, Error> {
        let archive = Archive::from_bytes(
            data,
            specular_archive::Options {
                limits: opts.archive.clone(),
                ..Default::default()
            },
        )
        .map_err(|e| match e {
            specular_archive::Error::NotAnArchive => Error::NotEpub,
            specular_archive::Error::Limit(m) => Error::TooLarge(m),
            e => Error::Damaged(e.to_string()),
        })?;
        if archive.format() != specular_archive::Format::Zip {
            return Err(Error::NotEpub);
        }
        let pkg = package::read_package(&ArchiveReader(&archive))?;
        if pkg.spine.is_empty() {
            return Err(Error::NoPages);
        }
        Ok(Book {
            inner: Arc::new(Inner { archive, pkg }),
        })
    }

    pub fn metadata(&self) -> &Metadata {
        &self.inner.pkg.metadata
    }

    /// The table of contents, flattened in reading order (see [`TocEntry::depth`]).
    pub fn toc(&self) -> &[TocEntry] {
        &self.inner.pkg.toc
    }

    /// The spine documents' archive paths, in reading order (linear ones, then the rest).
    pub fn spine(&self) -> Vec<&str> {
        let p = &self.inner.pkg;
        p.spine
            .iter()
            .map(|s| p.manifest[s.manifest].path.as_str())
            .collect()
    }

    /// The package file's path.
    pub fn package_path(&self) -> &str {
        &self.inner.pkg.opf_path
    }

    /// Paths of the fonts that were obfuscated (de-obfuscated when read).
    pub fn obfuscated_fonts(&self) -> Vec<&str> {
        let mut v: Vec<&str> = self
            .inner
            .pkg
            .obfuscated
            .keys()
            .map(String::as_str)
            .collect();
        v.sort_unstable();
        v
    }

    /// Paths of the book's embedded font files (manifest items with a font media type or a
    /// font extension), sorted.
    pub fn font_paths(&self) -> Vec<&str> {
        let mut v: Vec<&str> = self
            .inner
            .pkg
            .manifest
            .iter()
            .filter(|m| {
                let ext = m.path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
                m.media_type.starts_with("font/")
                    || m.media_type.starts_with("application/font")
                    || m.media_type.starts_with("application/x-font")
                    || m.media_type == "application/vnd.ms-opentype"
                    || matches!(
                        ext.as_str(),
                        "ttf" | "otf" | "woff" | "woff2" | "ttc" | "otc"
                    )
            })
            .map(|m| m.path.as_str())
            .collect();
        v.sort_unstable();
        v.dedup();
        v
    }

    /// An entry's bytes (fonts de-obfuscated), with their reservation.
    pub fn read(&self, path: &str) -> Option<Part> {
        self.inner.read(path, MAX_BINARY_PART)
    }

    /// The cover picture's path, without reading it.
    pub fn cover_path(&self) -> Option<String> {
        layout::cover_path(&self.inner)
    }

    /// The cover picture's bytes and media type.
    pub fn cover(&self) -> Option<(Part, String)> {
        let path = self.cover_path()?;
        let bytes = self.inner.read(&path, MAX_BINARY_PART)?;
        Some((bytes, self.media_type_of(&path)))
    }

    /// The media type the manifest gives `path`, else one guessed from its extension.
    pub fn media_type_of(&self, path: &str) -> String {
        let p = &self.inner.pkg;
        if let Some(i) = p.item_by_path(path)
            && !p.manifest[i].media_type.is_empty()
        {
            return p.manifest[i].media_type.clone();
        }
        media_type_from_name(path).to_owned()
    }

    /// Lays the book out into pages. Slow for long books (all of the text is shaped);
    /// `cancel` is polled between paragraphs.
    pub fn paginate(
        &self,
        setup: &PageSetup,
        images: Arc<dyn ImageDecoder>,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Pages, Error> {
        layout::paginate(self.inner.clone(), setup, images, cancel)
    }
}

pub(crate) fn media_type_from_name(path: &str) -> &'static str {
    let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "xhtml" | "xht" => "application/xhtml+xml",
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        _ => "application/octet-stream",
    }
}
