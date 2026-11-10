//! XPS and OpenXPS (ECMA-388) fixed documents.
//!
//! [`Document::open`] reads the package (own ZIP and OPC reader: ZIP64, interleaved pieces,
//! content types, relationships), follows the fixed representation to the document sequence,
//! its documents and their pages, and reads the core properties. Pages are parsed on first
//! use into an owned tree (canvases, paths, glyph runs, with resource dictionaries and markup
//! compatibility resolved) and kept in a small cache. [`Document::render`] draws any region of
//! a page at any scale with vello_cpu onto opaque white: tiles and deep zoom cost only the
//! pixels asked for.
//!
//! Drawn: path geometry (abbreviated syntax and `PathGeometry`), fills and strokes (dashes,
//! caps, joins, miter limits), solid, linear and radial gradient, image and visual brushes
//! (tiling and flipping), glyph runs with embedded (obfuscated) fonts and the bold and italic
//! simulations, transforms, clips, opacity and opacity masks. Images go through the caller's
//! decoder ([`ImageDecoder`]); their resolution is read from their headers. Colour profiles
//! (`ContextColor`, `ColorConvertedBitmap`) are not applied: their colours are approximated.
//! Signatures, thumbnails, print tickets and document structure are ignored.
//!
//! Bounds: part sizes, XML nodes, elements, nesting, glyphs, image pixels and offscreen renders
//! are capped, so hostile files fail or draw less instead of exhausting memory or time; the
//! caller's cancel flag is polled while parsing and drawing.

#[doc(hidden)]
pub mod fixture;
mod font;
mod geom;
mod image;
mod model;
mod opc;
mod page;
mod render;
mod xml;
mod zip;

#[cfg(test)]
mod tests;

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use vello_cpu::kurbo::Affine;
use vello_cpu::{PixmapMut, RasterizerSettings, RenderContext, Resources, TargetInit};

use crate::font::Font;
use crate::image::Img;
use crate::model::Page;
use crate::opc::{MAX_XML, Package};
use crate::page::{Dict, Parser};

/// Decodes an embedded image (JPEG, PNG, TIFF, JPEG XR) to straight RGBA8: width, height,
/// pixels.
pub type ImageDecoder = Arc<dyn Fn(&[u8]) -> Option<(u32, u32, Vec<u8>)> + Send + Sync>;

/// The largest side of a single render (vello_cpu's pixmaps are `u16` wide).
pub const MAX_RENDER_SIDE: u32 = u16::MAX as u32;
/// The largest single render, in pixels.
pub const MAX_RENDER_PIXELS: u64 = 256 * 1024 * 1024;
/// Most pages read from a package.
pub const MAX_PAGES: usize = 100_000;
/// Parsed pages kept.
const PAGE_CACHE: usize = 4;
/// Decoded image bytes kept before the image cache is emptied.
const IMAGE_CACHE_BYTES: usize = 768 << 20;
/// Largest font part read.
const MAX_FONT: u64 = 64 << 20;
/// Remote resource dictionaries read inside each other (a dictionary naming another).
const MAX_REMOTE_DEPTH: u32 = 8;

/// Why a document cannot be opened.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OpenError {
    /// Not an XPS or OpenXPS package.
    #[error("not an XPS document")]
    NotXps,
    /// A rights-managed (DRM) document: an encrypted package in a compound file.
    #[error("rights-managed (protected) XPS document: not shown")]
    Protected,
    /// Encrypted ZIP members.
    #[error("encrypted XPS package: not shown")]
    Encrypted,
    /// The package structure is broken.
    #[error("damaged XPS package")]
    Damaged,
    /// A readable package without pages.
    #[error("XPS document without pages")]
    NoPages,
}

/// Why a render failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RenderError {
    #[error("cancelled")]
    Cancelled,
    /// The requested raster is empty or larger than the render bounds.
    #[error("page raster {0}×{1} is outside the render bounds")]
    TooLarge(u32, u32),
    #[error("page {0} does not exist")]
    NoSuchPage(usize),
    /// The page could not be read.
    #[error("{0}")]
    Failed(String),
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
    pub width: u32,
    pub height: u32,
}

/// What the package says about itself, for the info panel.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Facts {
    /// OpenXPS (ECMA-388) rather than Microsoft XPS 1.0.
    pub openxps: bool,
    pub pages: usize,
    /// Fixed documents in the sequence.
    pub documents: usize,
    pub title: Option<String>,
    pub subject: Option<String>,
    pub creator: Option<String>,
    pub keywords: Option<String>,
    pub description: Option<String>,
    pub last_modified_by: Option<String>,
    pub revision: Option<String>,
    pub category: Option<String>,
    /// W3C date-times as stored.
    pub created: Option<String>,
    pub modified: Option<String>,
    /// Font parts in the package.
    pub fonts: usize,
    /// Image parts in the package.
    pub images: usize,
    /// The package carries digital signatures (not verified).
    pub signed: bool,
}

#[derive(Debug, Clone)]
struct PageRef {
    part: String,
    /// The `PageContent` size hint, 1/96 inch.
    hint: Option<(f64, f64)>,
}

/// Fonts by (part, face index); `None` for parts that are not usable fonts.
type FontCache = HashMap<(String, u32), Option<Arc<Font>>>;

/// What every handle to a document shares.
pub(crate) struct Inner {
    pkg: Package,
    pages: Vec<PageRef>,
    sizes: Vec<OnceLock<Option<(f64, f64)>>>,
    decoder: Option<ImageDecoder>,
    facts: Facts,
    parsed: Mutex<VecDeque<(usize, Arc<Page>)>>,
    fonts: Mutex<FontCache>,
    images: Mutex<HashMap<String, Option<Arc<Img>>>>,
    dicts: Mutex<HashMap<String, Option<Arc<Dict>>>>,
}

/// An XPS or OpenXPS document.
#[derive(Clone)]
pub struct Document {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for Document {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Document")
            .field("pages", &self.page_count())
            .finish()
    }
}

/// Whether `head` could be an XPS package: a ZIP local header (the package check happens at
/// open), or a compound file (rights-managed XPS).
pub fn looks_like_zip(head: &[u8]) -> bool {
    head.starts_with(b"PK\x03\x04")
}

const CFB: &[u8] = &[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The `Width`/`Height` of the root element of a page part, read without parsing the rest.
fn root_size(text: &str) -> Option<(f64, f64)> {
    let start = text.find("<FixedPage")?;
    let end = start + text[start..].find('>')?;
    let tag = &text[start..end];
    let attr = |name: &str| -> Option<f64> {
        let mut rest = tag;
        loop {
            let i = rest.find(name)?;
            let before = rest[..i].chars().last();
            let after = rest[i + name.len()..].trim_start();
            rest = &rest[i + name.len()..];
            if before.is_some_and(char::is_whitespace)
                && let Some(v) = after.strip_prefix('=')
            {
                let v = v.trim_start();
                let q = v.chars().next()?;
                if q != '"' && q != '\'' {
                    return None;
                }
                let v = &v[1..];
                let val = &v[..v.find(q)?];
                return val.trim().parse().ok();
            }
        }
    };
    Some((attr("Width")?, attr("Height")?))
}

fn text_of(doc: &roxmltree::Document<'_>, name: &str) -> Option<String> {
    doc.descendants()
        .find(|n| n.is_element() && n.tag_name().name() == name)
        .and_then(|n| n.text())
        .map(|t| t.trim().to_owned())
        .filter(|t| !t.is_empty())
        .map(|t| t.chars().take(4096).collect())
}

impl Document {
    /// Opens a package. `images` decodes the images of image brushes; without it they are not
    /// drawn.
    pub fn open(
        data: Arc<dyn AsRef<[u8]> + Send + Sync>,
        images: Option<ImageDecoder>,
    ) -> Result<Document, OpenError> {
        if (*data).as_ref().starts_with(CFB) {
            return Err(OpenError::Protected);
        }
        let pkg = Package::open(data).map_err(|e| match e {
            zip::ZipError::NotZip => OpenError::NotXps,
            _ => OpenError::Damaged,
        })?;
        if pkg.any_encrypted() {
            return Err(OpenError::Encrypted);
        }
        let mut facts = Facts::default();
        let rels = pkg.rels("");
        let fixed = rels
            .iter()
            .find(|r| r.kind.ends_with("/fixedrepresentation"))
            .map(|r| {
                facts.openxps = r.kind.contains("openxps");
                r.target.clone()
            })
            .or_else(|| {
                let mut seqs: Vec<&str> = pkg.names().filter(|n| n.ends_with(".fdseq")).collect();
                seqs.sort_unstable();
                seqs.first().map(|s| (*s).to_owned())
            })
            .ok_or(OpenError::NotXps)?;
        facts.signed = rels
            .iter()
            .any(|r| r.kind.ends_with("digital-signature/origin"));
        let pages = read_sequence(&pkg, &fixed, &mut facts)?;
        if pages.is_empty() {
            return Err(OpenError::NoPages);
        }
        facts.pages = pages.len();
        if let Some(core) = rels.iter().find(|r| r.kind.ends_with("core-properties"))
            && let Ok(b) = pkg.read(&core.target, MAX_XML)
            && let Some(text) = opc::xml_text(&b)
            && let Ok(doc) = xml::parse(&text)
        {
            facts.title = text_of(&doc, "title");
            facts.subject = text_of(&doc, "subject");
            facts.creator = text_of(&doc, "creator");
            facts.keywords = text_of(&doc, "keywords");
            facts.description = text_of(&doc, "description");
            facts.last_modified_by = text_of(&doc, "lastModifiedBy");
            facts.revision = text_of(&doc, "revision");
            facts.category = text_of(&doc, "category");
            facts.created = text_of(&doc, "created");
            facts.modified = text_of(&doc, "modified");
        }
        for name in pkg.names() {
            let ct = pkg.content_type(name).unwrap_or("");
            let ext = name.rsplit_once('.').map_or("", |(_, e)| e);
            if ct.contains("font") || matches!(ext, "odttf" | "ttf" | "otf" | "ttc") {
                facts.fonts += 1;
            } else if ct.starts_with("image/")
                || ct.contains("ms-photo")
                || matches!(ext, "jpg" | "jpeg" | "png" | "tif" | "tiff" | "wdp" | "jxr")
            {
                facts.images += 1;
            }
        }
        let sizes = (0..pages.len()).map(|_| OnceLock::new()).collect();
        Ok(Document {
            inner: Arc::new(Inner {
                pkg,
                pages,
                sizes,
                decoder: images,
                facts,
                parsed: Mutex::new(VecDeque::new()),
                fonts: Mutex::new(HashMap::new()),
                images: Mutex::new(HashMap::new()),
                dicts: Mutex::new(HashMap::new()),
            }),
        })
    }

    /// Number of pages (at least one).
    pub fn page_count(&self) -> usize {
        self.inner.pages.len()
    }

    /// Size of page `page` in points (1/72 inch), from its `FixedPage` width and height.
    pub fn page_size(&self, page: usize) -> Option<(f64, f64)> {
        let r = self.inner.pages.get(page)?;
        let size = self.inner.sizes[page].get_or_init(|| {
            self.inner
                .pkg
                .read(&r.part, opc::MAX_PART)
                .ok()
                .and_then(|b| opc::xml_text(&b).and_then(|t| root_size(&t)))
                .or(r.hint)
                .filter(|(w, h)| w.is_finite() && h.is_finite() && *w > 0.0 && *h > 0.0)
        });
        size.map(|(w, h)| (w * 0.75, h * 0.75))
    }

    pub fn facts(&self) -> Facts {
        self.inner.facts.clone()
    }

    /// The text of page `page` (its glyph runs' Unicode strings in markup order, lines split
    /// where the baseline moves); `None` when the page cannot be read.
    pub fn page_text(&self, page: usize) -> Option<String> {
        self.inner
            .page(page, &|| false)
            .ok()
            .map(|p| p.text.clone())
    }

    /// Renders a region of a page to straight RGBA8 (`width × height × 4` bytes, every pixel
    /// opaque: pages are drawn on white). `cancelled` is polled while the page is read and
    /// drawn.
    pub fn render(
        &self,
        req: RenderRequest,
        cancelled: &dyn Fn() -> bool,
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
        let page = self.inner.page(req.page, cancelled)?;
        let len = w as usize * h as usize * 4;
        let mut buf = Vec::new();
        buf.try_reserve_exact(len)
            .map_err(|_| RenderError::TooLarge(w, h))?;
        buf.resize(len, 0);
        let (w16, h16) = (w as u16, h as u16);
        let mut ctx = RenderContext::new(w16, h16);
        // Page units are 1/96 inch; the scale is pixels per point.
        let base = Affine::translate((-f64::from(req.x), -f64::from(req.y)))
            * Affine::scale_non_uniform(req.scale_x * 0.75, req.scale_y * 0.75);
        let mut offscreen = 0;
        let mut r = render::Renderer::new(&self.inner, &mut ctx, cancelled, &mut offscreen);
        r.draw_nodes(&page.nodes, base, 0);
        if r.stop && cancelled() {
            return Err(RenderError::Cancelled);
        }
        ctx.flush();
        let target = PixmapMut::new(w16, h16, &mut buf).ok_or(RenderError::TooLarge(w, h))?;
        ctx.render_with(
            target,
            &mut Resources::default(),
            RasterizerSettings {
                target_init: TargetInit::Clear(vello_cpu::color::palette::css::WHITE),
                ..Default::default()
            },
        );
        for px in buf.chunks_exact_mut(4) {
            px[3] = 255;
        }
        Ok(buf)
    }

    /// Memory held by the document's caches (decoded images, parsed pages are not counted).
    pub fn memory_bytes(&self) -> usize {
        lock(&self.inner.images)
            .values()
            .flatten()
            .map(|i| i.memory())
            .sum()
    }

    /// Drops cached images and parsed pages; returns the image bytes released.
    pub fn trim_memory(&self) -> usize {
        let n = self.memory_bytes();
        lock(&self.inner.images).clear();
        lock(&self.inner.parsed).clear();
        n
    }
}

/// Reads the document sequence and its documents' page lists.
fn read_sequence(pkg: &Package, part: &str, facts: &mut Facts) -> Result<Vec<PageRef>, OpenError> {
    let b = pkg.read(part, MAX_XML).map_err(|_| OpenError::Damaged)?;
    let text = opc::xml_text(&b).ok_or(OpenError::Damaged)?;
    let doc = xml::parse(&text).map_err(|_| OpenError::Damaged)?;
    let root = doc.root_element();
    if root
        .tag_name()
        .namespace()
        .is_some_and(|n| n.contains("openxps"))
    {
        facts.openxps = true;
    }
    let docs: Vec<String> = match root.tag_name().name() {
        "FixedDocumentSequence" => xml::children(root)
            .into_iter()
            .filter(|n| n.tag_name().name() == "DocumentReference")
            .filter_map(|n| opc::resolve(part, n.attribute("Source")?))
            .collect(),
        // A package whose fixed representation is a single document.
        "FixedDocument" => vec![part.to_owned()],
        _ => return Err(OpenError::NotXps),
    };
    let mut pages = Vec::new();
    for d in docs {
        let Ok(b) = pkg.read(&d, MAX_XML) else {
            continue;
        };
        let Some(text) = opc::xml_text(&b) else {
            continue;
        };
        let Ok(doc) = xml::parse(&text) else { continue };
        facts.documents += 1;
        for pc in xml::children(doc.root_element()) {
            if pc.tag_name().name() != "PageContent" || pages.len() >= MAX_PAGES {
                continue;
            }
            let Some(src) = pc.attribute("Source").and_then(|s| opc::resolve(&d, s)) else {
                continue;
            };
            if !pkg.has(&src) {
                continue;
            }
            let num = |n: &str| pc.attribute(n).and_then(|v| v.trim().parse::<f64>().ok());
            pages.push(PageRef {
                part: src,
                hint: num("Width").zip(num("Height")),
            });
        }
    }
    Ok(pages)
}

impl Inner {
    /// Page `i`, parsed (or from the cache).
    fn page(&self, i: usize, cancelled: &dyn Fn() -> bool) -> Result<Arc<Page>, RenderError> {
        {
            let mut c = lock(&self.parsed);
            if let Some(pos) = c.iter().position(|(k, _)| *k == i) {
                let hit = c.remove(pos).ok_or(RenderError::NoSuchPage(i))?;
                let p = hit.1.clone();
                c.push_front(hit);
                return Ok(p);
            }
        }
        let r = self.pages.get(i).ok_or(RenderError::NoSuchPage(i))?;
        let b = self
            .pkg
            .read(&r.part, opc::MAX_PART)
            .map_err(|_| RenderError::Failed("damaged XPS page".into()))?;
        let text =
            opc::xml_text(&b).ok_or_else(|| RenderError::Failed("damaged XPS page".into()))?;
        let doc = xml::parse(&text).map_err(|_| RenderError::Failed("damaged XPS page".into()))?;
        let mut parser = Parser::new(self, &r.part, cancelled);
        let page = Arc::new(parser.page(doc.root_element())?);
        let mut c = lock(&self.parsed);
        c.push_front((i, page.clone()));
        c.truncate(PAGE_CACHE);
        Ok(page)
    }

    /// The font in `part` (face `index`), loaded once.
    pub(crate) fn font(&self, part: &str, index: u32) -> Option<Arc<Font>> {
        let key = (part.to_owned(), index);
        if let Some(f) = lock(&self.fonts).get(&key) {
            return f.clone();
        }
        let f = self.pkg.read(part, MAX_FONT).ok().and_then(|b| {
            let ext = part.rsplit_once('.').map_or("", |(_, e)| e);
            let obfuscated = ext == "odttf"
                || self
                    .pkg
                    .content_type(part)
                    .is_some_and(|t| t.contains("obfuscated"));
            let data = if obfuscated {
                font::deobfuscate(b.into_owned(), part)
            } else {
                b.into_owned()
            };
            Font::new(data, index).map(Arc::new)
        });
        lock(&self.fonts).insert(key, f.clone());
        f
    }

    /// The image in `part`, decoded once (while the cache holds it).
    pub(crate) fn image(&self, part: &str) -> Option<Arc<Img>> {
        if let Some(i) = lock(&self.images).get(part) {
            return i.clone();
        }
        let img = self.decoder.as_ref().and_then(|dec| {
            let b = self.pkg.read(part, opc::MAX_PART).ok()?;
            let dpi = image::dpi(&b);
            let (w, h, px) = (**dec)(&b)?;
            if u64::from(w) * u64::from(h) > image::MAX_IMAGE_PIXELS {
                return None;
            }
            Img::new(px, w, h, dpi).map(Arc::new)
        });
        let mut c = lock(&self.images);
        let held: usize = c.values().flatten().map(|i| i.memory()).sum();
        if held > IMAGE_CACHE_BYTES {
            c.clear();
        }
        c.insert(part.to_owned(), img.clone());
        img
    }

    /// The remote resource dictionary in `part`, read once. A dictionary may name further
    /// remote dictionaries: one being read counts as missing (a part naming itself, or a
    /// cycle), and at most [`MAX_REMOTE_DEPTH`] are read inside each other.
    pub(crate) fn dict(&self, part: &str, cancelled: &dyn Fn() -> bool) -> Option<Arc<Dict>> {
        thread_local! {
            static READING: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
        }
        {
            let mut dicts = lock(&self.dicts);
            if let Some(d) = dicts.get(part) {
                return d.clone();
            }
            if READING.with(|r| r.get()) >= MAX_REMOTE_DEPTH {
                return None;
            }
            // In progress: a reference back to this part finds nothing.
            dicts.insert(part.to_owned(), None);
        }
        struct Reading;
        impl Drop for Reading {
            fn drop(&mut self) {
                READING.with(|r| r.set(r.get() - 1));
            }
        }
        READING.with(|r| r.set(r.get() + 1));
        let reading = Reading;
        let d = (|| {
            let b = self.pkg.read(part, MAX_XML).ok()?;
            let text = opc::xml_text(&b)?;
            let doc = xml::parse(&text).ok()?;
            let root = doc.root_element();
            if root.tag_name().name() != "ResourceDictionary" {
                return None;
            }
            let mut p = Parser::new(self, part, cancelled);
            p.push_local();
            p.fill_dict(root, 0);
            Some(Arc::new(p.take_local()))
        })();
        drop(reading);
        lock(&self.dicts).insert(part.to_owned(), d.clone());
        d
    }
}
