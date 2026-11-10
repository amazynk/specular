//! DjVu documents: single pages (`FORM:DJVU`), bundled and indirect multi-page documents
//! (`FORM:DJVM` with its `DIRM` directory), shared dictionaries and annotations included with
//! `INCL`, and the page layers: the IW44 background and foreground (`BG44`, `FG44`, or JPEG
//! `BGjp`/`FGjp`), the JB2 mask (`Sjbz`, with `Djbz` dictionaries, or a G4 `Smmr`) and the
//! FGbz colour palette. Pages composite as the format describes: mask pixels take the
//! foreground colour (the FG44 picture or the palette colour of their shape, else black) over
//! the background (white when there is none), at the page's own pixel size, turned as the
//! INFO chunk says. The hidden text (`TXTa`/`TXTz`), annotations (`ANTa`/`ANTz`), outline
//! (`NAVM`) and embedded thumbnails (`TH44`) are read too. IW44 image files (`FORM:BM44`
//! grey, `FORM:PM44` colour) open as one page holding their picture, at 100 dpi.
//!
//! Decoders are our own (ZP-coder, BZZ, JB2, IW44, written from the DjVu v3 specification);
//! JPEG layers and G4 masks go through hooks the caller supplies. Every size, count and
//! depth is bounded, so hostile files fail or draw less instead of panicking or hanging, and
//! long decodes poll a cancel callback.

mod bzz;
mod iff;
pub mod iw44;
pub mod jb2;
pub mod text;
mod zp;

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, OnceLock};

pub use iff::looks_like_djvu;
pub use iw44::Picture;
pub use text::{Annotations, Bookmark, TextLayer, Zone, ZoneKind};

/// Most components in a multi-page document.
pub const MAX_COMPONENTS: usize = 1 << 16;
/// Largest decompressed BZZ chunk (directories, text, annotations, palettes).
const MAX_BZZ: usize = 64 << 20;
/// Largest page, in pixels.
pub const MAX_PAGE_PIXELS: u64 = 1 << 31;
/// Deepest chain of included files.
const MAX_INCLUDE_DEPTH: usize = 8;
/// Pages whose decoded layers are kept.
const PAGE_CACHE: usize = 3;

/// Why a document or page cannot be shown. The text is a plain-language reason.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("damaged DjVu file: {0}")]
    Malformed(&'static str),
    #[error("DjVu feature not supported: {0}")]
    Unsupported(&'static str),
    #[error("DjVu page is too large to show")]
    TooLarge,
    /// The memory budget cannot hold the page.
    #[error("{0}")]
    NeedsMemory(#[from] specular_core::NeedsMemory),
    #[error("cancelled")]
    Cancelled,
    /// A file of an indirect document, or a shared file, that cannot be found.
    #[error("DjVu document part \"{0}\" is missing")]
    Missing(String),
    #[error("DjVu page {0} does not exist")]
    NoSuchPage(usize),
    /// The page holds no layer this decoder reads, only chunks it does not know (for example
    /// secured or encrypted content).
    #[error("DjVu page without readable image data (unknown chunks: {0})")]
    NoImage(String),
}

/// The bytes of a file.
pub type Bytes = Arc<dyn AsRef<[u8]> + Send + Sync>;
/// Finds a file of an indirect document by its name (relative to the index file).
pub type Resolver = Arc<dyn Fn(&str) -> Option<Bytes> + Send + Sync>;
/// Decodes a JPEG (`BGjp`, `FGjp`) to straight RGBA8.
pub type JpegDecoder = Arc<dyn Fn(&[u8]) -> Option<(u32, u32, Vec<u8>)> + Send + Sync>;
/// Decodes G4 (MMR) data of a `width`×`height` bitmap to one byte per pixel, 1 = black.
pub type MmrDecoder = Arc<dyn Fn(&[u8], u32, u32) -> Option<Vec<u8>> + Send + Sync>;

/// What the caller provides beyond the main file.
#[derive(Clone, Default)]
pub struct Options {
    pub resolver: Option<Resolver>,
    pub jpeg: Option<JpegDecoder>,
    pub mmr: Option<MmrDecoder>,
}

/// A byte range of a file.
#[derive(Clone)]
struct Blob {
    bytes: Bytes,
    start: usize,
    end: usize,
}

impl Blob {
    fn data(&self) -> &[u8] {
        let d = (*self.bytes).as_ref();
        &d[self.start.min(d.len())..self.end.min(d.len())]
    }
}

/// What a component file of a document is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentKind {
    Include,
    Page,
    Thumbnails,
    SharedAnnotations,
}

struct Component {
    id: String,
    name: String,
    title: String,
    kind: ComponentKind,
    /// Bundled: known at open. Indirect: resolved on first use.
    blob: OnceLock<Option<Blob>>,
}

/// How a page is turned for display (the INFO flags).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Rotation {
    #[default]
    None,
    /// 90° counter-clockwise.
    Ccw90,
    Half,
    /// 90° clockwise.
    Cw90,
}

/// The facts of a page's INFO chunk and its layers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageInfo {
    /// Stored (unturned) size in pixels.
    pub width: u32,
    pub height: u32,
    pub dpi: u32,
    /// Gamma × 10 the page was made for (22 for 2.2).
    pub gamma: u8,
    pub rotation: Rotation,
    /// Encoder version (INFO minor version, e.g. 26).
    pub version: u8,
    /// The layers present, e.g. "JB2 mask, IW44 background (1/3)".
    pub layers: String,
    pub has_text: bool,
    /// The page title from the directory, when it differs from the file name.
    pub title: Option<String>,
}

impl PageInfo {
    /// The displayed size: width and height swapped for a quarter turn.
    pub fn display_size(&self) -> (u32, u32) {
        match self.rotation {
            Rotation::Ccw90 | Rotation::Cw90 => (self.height, self.width),
            _ => (self.width, self.height),
        }
    }
}

/// Document-wide facts.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Facts {
    pub pages: usize,
    /// "single page", "bundled" or "indirect".
    pub structure: &'static str,
    pub components: usize,
    pub shared_dictionaries: usize,
    pub outline_entries: usize,
    pub thumbnails: usize,
    /// Metadata of the shared annotations (or of the first page): Title, Author, Producer…
    pub metadata: Vec<(String, String)>,
}

/// The decoded layers of one page.
struct Decoded {
    info: PageInfo,
    bg: Option<Picture>,
    fg: Option<Picture>,
    mask: Option<Mask>,
    palette: Option<Palette>,
    /// Blits by 128-row bin, for regions.
    bins: Vec<Vec<u32>>,
    /// The memory budget's share of the pictures, mask and bins, held while the page is
    /// cached (the layers' reservations end when they are made into pictures).
    _held: specular_core::Reservation,
}

impl Decoded {
    fn memory_bytes(&self) -> usize {
        self.bg.as_ref().map_or(0, |p| p.data.len())
            + self.fg.as_ref().map_or(0, |p| p.data.len())
            + match &self.mask {
                Some(Mask::Jb2(i)) => i.memory_bytes(),
                Some(Mask::Bitmap { bits, .. }) => bits.len(),
                None => 0,
            }
            + self.bins.iter().map(|b| b.len() * 4).sum::<usize>()
    }
}

enum Mask {
    Jb2(jb2::Image),
    Bitmap {
        width: u32,
        height: u32,
        bits: Vec<u8>,
    },
}

struct Palette {
    colors: Vec<[u8; 3]>,
    /// Colour index of each blit.
    indices: Vec<u16>,
}

const BIN_ROWS: i32 = 128;
/// Most blit entries over all bins of a page: a tall shape is listed in every bin it
/// crosses, so 4M page-high blits would take 8 GB.
const MAX_BIN_ENTRIES: usize = 1 << 24;

/// Chunks of a FORM: header offset and id.
type ChunkList = Vec<(usize, [u8; 4])>;

/// A DjVu document.
pub struct Document {
    opts: Options,
    structure: &'static str,
    components: Vec<Component>,
    /// Component index of each page.
    pages: Vec<usize>,
    /// Thumbnail chunk (component, chunk offset in its data) of each page, when embedded.
    thumbs: Vec<Option<(usize, usize)>>,
    outline: Vec<Bookmark>,
    infos: Vec<OnceLock<Result<PageInfo, Error>>>,
    /// Shared dictionaries by component id; `None`: the component holds none.
    dicts: Mutex<HashMap<String, Option<Arc<jb2::Dict>>>>,
    cache: Mutex<VecDeque<(usize, Arc<Decoded>)>>,
    /// Serialises decoding of the same page by concurrent region requests.
    decoding: Vec<Mutex<()>>,
}

impl std::fmt::Debug for Document {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Document")
            .field("structure", &self.structure)
            .field("pages", &self.pages.len())
            .finish()
    }
}

fn be16(d: &[u8], p: usize) -> Option<u16> {
    Some(u16::from_be_bytes([*d.get(p)?, *d.get(p + 1)?]))
}

fn be32(d: &[u8], p: usize) -> Option<u32> {
    Some(u32::from_be_bytes([
        *d.get(p)?,
        *d.get(p + 1)?,
        *d.get(p + 2)?,
        *d.get(p + 3)?,
    ]))
}

impl Document {
    /// Parses a DjVu file: a page, or a document's index (bundled or indirect).
    pub fn open(data: Bytes, opts: Options) -> Result<Document, Error> {
        let bytes = (*data).as_ref();
        if !bytes.starts_with(b"AT&TFORM") {
            return Err(Error::Malformed("not a DjVu file"));
        }
        let top = iff::top_form(bytes, 0)?;
        let (kind, _, _) = top.form().ok_or(Error::Malformed("no DjVu FORM chunk"))?;
        match &kind {
            b"DJVU" | b"BM44" | b"PM44" => {
                let comp = Component {
                    id: String::new(),
                    name: String::new(),
                    title: String::new(),
                    kind: ComponentKind::Page,
                    blob: OnceLock::from(Some(Blob {
                        start: top.offset,
                        end: top.offset + 8 + top.data.len(),
                        bytes: data.clone(),
                    })),
                };
                let structure = if &kind == b"DJVU" {
                    "single page"
                } else {
                    "IW44 image"
                };
                Ok(Document::build(opts, structure, vec![comp], Vec::new()))
            }
            b"DJVM" => Document::open_djvm(data.clone(), &top, opts),
            b"DJVI" => Err(Error::Unsupported(
                "a shared part of a multi-page document (open the document itself)",
            )),
            b"THUM" => Err(Error::Unsupported(
                "a thumbnail part of a multi-page document (open the document itself)",
            )),
            _ => Err(Error::Malformed("not a DjVu file")),
        }
    }

    fn open_djvm(data: Bytes, top: &iff::Chunk<'_>, opts: Options) -> Result<Document, Error> {
        let (_, body, base) = top.form().ok_or(Error::Malformed("FORM"))?;
        let mut it = iff::chunks(body, base);
        let dirm = it
            .next()
            .ok_or(Error::Malformed("document without a directory"))?;
        if &dirm.id != b"DIRM" {
            if &dirm.id == b"NAVM" || &dirm.id == b"FORM" {
                return Err(Error::Unsupported(
                    "old-style multi-page document (no directory)",
                ));
            }
            return Err(Error::Malformed("document without a directory"));
        }
        let d = dirm.data;
        let flags = *d.first().ok_or(Error::Malformed("directory"))?;
        let bundled = flags & 0x80 != 0;
        if flags & 0x7f > 1 {
            return Err(Error::Unsupported("document directory of a newer version"));
        }
        let n = usize::from(be16(d, 1).ok_or(Error::Malformed("directory"))?);
        if n == 0 || n > MAX_COMPONENTS {
            return Err(Error::Malformed("directory without files"));
        }
        let mut p = 3;
        let mut offsets = Vec::new();
        if bundled {
            for i in 0..n {
                offsets.push(be32(d, p + 4 * i).ok_or(Error::Malformed("directory"))? as usize);
            }
            p += 4 * n;
        }
        let rest = bzz::decode(d.get(p..).unwrap_or(&[]), MAX_BZZ)?;
        // Sizes (3 bytes each), flags (1 byte each), then the strings.
        let need = n * 4;
        if rest.len() < need {
            return Err(Error::Malformed("directory"));
        }
        let fl = &rest[n * 3..n * 4];
        let mut strings = rest[need..].split(|&b| b == 0);
        let mut next = || {
            strings
                .next()
                .map(|s| String::from_utf8_lossy(s).into_owned())
                .ok_or(Error::Malformed("directory names"))
        };
        let file_bytes = (*data).as_ref();
        let mut comps = Vec::with_capacity(n);
        for i in 0..n {
            let f = fl[i];
            let id = next()?;
            let name = if f & 0x80 != 0 { next()? } else { id.clone() };
            let title = if f & 0x40 != 0 { next()? } else { id.clone() };
            let kind = match f & 0x3f {
                1 => ComponentKind::Page,
                2 => ComponentKind::Thumbnails,
                3 => ComponentKind::SharedAnnotations,
                _ => ComponentKind::Include,
            };
            let blob = OnceLock::new();
            if bundled {
                let off = offsets[i];
                let c = iff::top_form(file_bytes, off)?;
                let _ = blob.set(Some(Blob {
                    start: c.offset,
                    end: c.offset + 8 + c.data.len(),
                    bytes: data.clone(),
                }));
            }
            comps.push(Component {
                id,
                name,
                title,
                kind,
                blob,
            });
        }
        // The outline follows the directory.
        let mut outline = Vec::new();
        if let Some(c) = it.next()
            && &c.id == b"NAVM"
            && let Ok(b) = bzz::decode(c.data, MAX_BZZ)
        {
            outline = text::decode_outline(&b).unwrap_or_default();
        }
        let structure = if bundled { "bundled" } else { "indirect" };
        Ok(Document::build(opts, structure, comps, outline))
    }

    fn build(
        opts: Options,
        structure: &'static str,
        components: Vec<Component>,
        outline: Vec<Bookmark>,
    ) -> Document {
        let pages: Vec<usize> = components
            .iter()
            .enumerate()
            .filter(|(_, c)| c.kind == ComponentKind::Page)
            .map(|(i, _)| i)
            .collect();
        let n = pages.len();
        let mut doc = Document {
            opts,
            structure,
            components,
            pages,
            thumbs: vec![None; n],
            outline,
            infos: (0..n).map(|_| OnceLock::new()).collect(),
            dicts: Mutex::new(HashMap::new()),
            cache: Mutex::new(VecDeque::new()),
            decoding: (0..n).map(|_| Mutex::new(())).collect(),
        };
        doc.map_thumbnails();
        doc
    }

    /// Embedded thumbnails: each THUM component holds the thumbnails of the pages after it.
    fn map_thumbnails(&mut self) {
        if self.structure != "bundled" {
            return;
        }
        let mut queue = VecDeque::new();
        let mut page = 0;
        for (ci, c) in self.components.iter().enumerate() {
            match c.kind {
                ComponentKind::Thumbnails => {
                    let Some(Some(b)) = c.blob.get() else {
                        continue;
                    };
                    let d = b.data();
                    let Ok(top) = iff::top_form(d, 0) else {
                        continue;
                    };
                    let Some((_, body, base)) = top.form() else {
                        continue;
                    };
                    for ch in iff::chunks(body, base) {
                        if &ch.id == b"TH44" {
                            queue.push_back((ci, ch.offset));
                        }
                    }
                }
                ComponentKind::Page => {
                    if page < self.thumbs.len() {
                        self.thumbs[page] = queue.pop_front();
                    }
                    page += 1;
                }
                _ => {}
            }
        }
    }

    /// Number of pages (0 only for a document whose directory lists no page).
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    /// The document outline (bookmarks).
    pub fn outline(&self) -> &[Bookmark] {
        &self.outline
    }

    /// The page a bookmark or link target (`#id`, `#title`, `#12`) points at.
    pub fn resolve_target(&self, url: &str) -> Option<usize> {
        let t = url.strip_prefix('#')?;
        if let Some(i) = self.pages.iter().position(|&c| {
            let c = &self.components[c];
            c.id == t || c.title == t || c.name == t
        }) {
            return Some(i);
        }
        let n: usize = t.parse().ok()?;
        (1..=self.pages.len()).contains(&n).then(|| n - 1)
    }

    fn blob(&self, ci: usize) -> Result<&Blob, Error> {
        let c = &self.components[ci];
        let b = c.blob.get_or_init(|| {
            let r = self.opts.resolver.as_ref()?;
            let bytes = r(&c.name)?;
            let d = (*bytes).as_ref();
            if !d.starts_with(b"AT&TFORM") {
                return None;
            }
            let top = iff::top_form(d, 0).ok()?;
            let (start, end) = (top.offset, top.offset + 8 + top.data.len());
            Some(Blob { bytes, start, end })
        });
        b.as_ref().ok_or_else(|| Error::Missing(c.name.clone()))
    }

    /// The chunks of component `ci`'s FORM, with its form kind.
    fn form_of(&self, ci: usize) -> Result<([u8; 4], ChunkList), Error> {
        let b = self.blob(ci)?;
        let d = b.data();
        let top = iff::top_form(d, 0)?;
        let (kind, body, base) = top.form().ok_or(Error::Malformed("FORM"))?;
        Ok((
            kind,
            iff::chunks(body, base).map(|c| (c.offset, c.id)).collect(),
        ))
    }

    fn chunk_at(&self, ci: usize, offset: usize) -> Result<&[u8], Error> {
        let d = self.blob(ci)?.data();
        let c = iff::chunks(&d[offset.min(d.len())..], offset)
            .next()
            .ok_or(Error::Malformed("chunk"))?;
        Ok(c.data)
    }

    fn component_by_id(&self, id: &str) -> Option<usize> {
        self.components
            .iter()
            .position(|c| c.id == id)
            .or_else(|| self.components.iter().position(|c| c.name == id))
    }

    /// Collects the chunks of a page and of everything it includes, in order.
    fn page_chunks(&self, page: usize) -> Result<Vec<(usize, usize, [u8; 4])>, Error> {
        let ci = *self.pages.get(page).ok_or(Error::NoSuchPage(page))?;
        let mut out = Vec::new();
        let mut seen = Vec::new();
        self.collect(ci, 0, &mut out, &mut seen)?;
        Ok(out)
    }

    fn collect(
        &self,
        ci: usize,
        depth: usize,
        out: &mut Vec<(usize, usize, [u8; 4])>,
        seen: &mut Vec<usize>,
    ) -> Result<(), Error> {
        if depth > MAX_INCLUDE_DEPTH || seen.contains(&ci) {
            return Ok(());
        }
        seen.push(ci);
        let (_, chunks) = self.form_of(ci)?;
        for (off, id) in chunks {
            if &id == b"INCL" {
                let name = String::from_utf8_lossy(self.chunk_at(ci, off)?)
                    .trim()
                    .to_string();
                let inc = self
                    .component_by_id(&name)
                    .ok_or_else(|| Error::Missing(name.clone()))?;
                self.collect(inc, depth + 1, out, seen)?;
            } else {
                out.push((ci, off, id));
            }
        }
        Ok(())
    }

    /// Facts of page `page` (reads its INFO chunk and lists its layers).
    pub fn page_info(&self, page: usize) -> Result<PageInfo, Error> {
        let slot = self.infos.get(page).ok_or(Error::NoSuchPage(page))?;
        slot.get_or_init(|| self.read_info(page)).clone()
    }

    fn read_info(&self, page: usize) -> Result<PageInfo, Error> {
        let chunks = self.page_chunks(page)?;
        let mut info: Option<PageInfo> = None;
        let mut layers = Vec::new();
        let mut unknown = Vec::new();
        let mut has_text = false;
        let mut bg_first: Option<(usize, usize)> = None;
        let mut fg_first: Option<(usize, usize)> = None;
        for &(ci, off, id) in &chunks {
            match &id {
                b"INFO" => {
                    if info.is_none() {
                        info = Some(parse_info(self.chunk_at(ci, off)?)?);
                    }
                }
                b"Sjbz" => layers.push("JB2 mask".to_string()),
                b"Smmr" => layers.push("G4 mask".to_string()),
                b"FGbz" => layers.push("colour palette".to_string()),
                // IW44 image files (FORM:BM44 grey, FORM:PM44 colour) hold only the picture.
                b"BG44" | b"BM44" | b"PM44" => {
                    if bg_first.is_none() {
                        bg_first = Some((ci, off));
                    }
                }
                b"FG44" => fg_first = Some((ci, off)),
                b"BGjp" => layers.push("JPEG background".to_string()),
                b"FGjp" => layers.push("JPEG foreground".to_string()),
                b"TXTa" | b"TXTz" => has_text = true,
                b"Djbz" | b"ANTa" | b"ANTz" | b"CIDa" | b"WMRM" | b"NDIR" | b"LTAn" | b"FORM" => {}
                other => {
                    let s = String::from_utf8_lossy(other).into_owned();
                    if !unknown.contains(&s) {
                        unknown.push(s);
                    }
                }
            }
        }
        // An IW44 image file has no INFO chunk: the picture's own size, at 100 dpi (what its
        // encoder assumes).
        let iw44_file = chunks
            .iter()
            .any(|&(_, _, id)| matches!(&id, b"BM44" | b"PM44"));
        if info.is_none()
            && iw44_file
            && let Some((ci, off)) = bg_first
            && let Some(h) = iw44::header(self.chunk_at(ci, off)?)
        {
            info = Some(PageInfo {
                width: h.width,
                height: h.height,
                dpi: 100,
                gamma: 22,
                rotation: Rotation::None,
                version: 0,
                layers: String::new(),
                has_text: false,
                title: None,
            });
        }
        let mut info = info.ok_or(Error::Malformed("page without an INFO chunk"))?;
        let sub = |h: iw44::Header| {
            let r = reduction(info.width, info.height, h.width, h.height);
            format!("1/{r}")
        };
        if let Some((ci, off)) = fg_first
            && let Some(h) = iw44::header(self.chunk_at(ci, off)?)
        {
            layers.push(format!(
                "IW44 foreground ({}, {})",
                if h.color { "colour" } else { "grey" },
                sub(h)
            ));
        }
        if let Some((ci, off)) = bg_first
            && let Some(h) = iw44::header(self.chunk_at(ci, off)?)
        {
            layers.push(format!(
                "IW44 {} ({}, {})",
                if layers.iter().any(|l| l.contains("mask")) {
                    "background"
                } else {
                    "picture"
                },
                if h.color { "colour" } else { "grey" },
                sub(h)
            ));
        }
        if layers.is_empty() && !unknown.is_empty() {
            return Err(Error::NoImage(unknown.join(", ")));
        }
        info.layers = layers.join(", ");
        info.has_text = has_text;
        let comp = &self.components[self.pages[page]];
        if comp.title != comp.id && !comp.title.is_empty() {
            info.title = Some(comp.title.clone());
        }
        Ok(info)
    }

    /// Document facts (reads the first page's annotations for its metadata when the
    /// document has no shared annotations).
    pub fn facts(&self) -> Facts {
        let mut f = Facts {
            pages: self.pages.len(),
            structure: self.structure,
            components: self.components.len(),
            outline_entries: count_bookmarks(&self.outline),
            thumbnails: self.thumbs.iter().filter(|t| t.is_some()).count(),
            ..Facts::default()
        };
        for (ci, c) in self.components.iter().enumerate() {
            if c.kind == ComponentKind::Include
                && let Ok((_, chunks)) = self.form_of(ci)
                && chunks.iter().any(|(_, id)| id == b"Djbz")
            {
                f.shared_dictionaries += 1;
            }
        }
        if !self.pages.is_empty()
            && let Ok(a) = self.annotations(0)
        {
            f.metadata = a.metadata;
        }
        f
    }

    /// The annotations of a page (its own and the shared ones it includes).
    pub fn annotations(&self, page: usize) -> Result<Annotations, Error> {
        let mut a = Annotations::default();
        for (ci, off, id) in self.page_chunks(page)? {
            let d = self.chunk_at(ci, off)?;
            match &id {
                b"ANTa" => text::decode_annotations(d, &mut a),
                b"ANTz" => {
                    if let Ok(b) = bzz::decode(d, MAX_BZZ) {
                        text::decode_annotations(&b, &mut a);
                    }
                }
                _ => {}
            }
        }
        Ok(a)
    }

    /// The hidden text of a page, with its zones (top-down rows of the unturned page).
    pub fn page_text(&self, page: usize) -> Result<Option<TextLayer>, Error> {
        let info = self.page_info(page)?;
        for (ci, off, id) in self.page_chunks(page)? {
            let d = self.chunk_at(ci, off)?;
            match &id {
                b"TXTa" => return text::decode_text(d, info.height).map(Some),
                b"TXTz" => {
                    let b = bzz::decode(d, MAX_BZZ)?;
                    return text::decode_text(&b, info.height).map(Some);
                }
                _ => {}
            }
        }
        Ok(None)
    }

    /// The embedded thumbnail of a page, when the document has one.
    pub fn thumbnail(&self, page: usize, cancel: &dyn Fn() -> bool) -> Option<Picture> {
        let (ci, off) = (*self.thumbs.get(page)?)?;
        let d = self.chunk_at(ci, off).ok()?;
        let mut l = iw44::Layer::new(d).ok()?;
        l.chunk(d, cancel).ok()?;
        Some(l.picture())
    }

    /// The memory held by cached page layers.
    pub fn memory_bytes(&self) -> usize {
        self.cache
            .lock()
            .map(|c| c.iter().map(|(_, d)| d.memory_bytes()).sum())
            .unwrap_or(0)
    }

    /// Drops the cached page layers; returns the bytes released.
    pub fn trim(&self) -> usize {
        let Ok(mut c) = self.cache.lock() else {
            return 0;
        };
        let n = c.iter().map(|(_, d)| d.memory_bytes()).sum();
        c.clear();
        n
    }

    /// The shared dictionary held by component `ci` (decoded once per document).
    fn dict(
        &self,
        ci: usize,
        depth: usize,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Option<Arc<jb2::Dict>>, Error> {
        self.dict_in(ci, depth, &mut Vec::new(), cancel)
    }

    /// [`Self::dict`] with the components being resolved (`path`): an include of one of
    /// them is a cycle and ignored. Every outcome is cached, including "none", so includes
    /// shared by many components are resolved once (not once per path to them).
    fn dict_in(
        &self,
        ci: usize,
        depth: usize,
        path: &mut Vec<usize>,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Option<Arc<jb2::Dict>>, Error> {
        let key = self.components[ci].id.clone();
        if let Ok(m) = self.dicts.lock()
            && let Some(d) = m.get(&key)
        {
            return Ok(d.clone());
        }
        if depth > MAX_INCLUDE_DEPTH || path.contains(&ci) {
            return Ok(None);
        }
        path.push(ci);
        let found = self.dict_uncached(ci, depth, path, cancel);
        path.pop();
        let found = found?;
        if let Ok(mut m) = self.dicts.lock() {
            m.insert(key, found.clone());
        }
        Ok(found)
    }

    fn dict_uncached(
        &self,
        ci: usize,
        depth: usize,
        path: &mut Vec<usize>,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Option<Arc<jb2::Dict>>, Error> {
        let (_, chunks) = self.form_of(ci)?;
        // A dictionary may inherit from one its file includes.
        let mut parent = None;
        for &(off, id) in &chunks {
            if &id == b"INCL" {
                let name = String::from_utf8_lossy(self.chunk_at(ci, off)?)
                    .trim()
                    .to_string();
                if let Some(inc) = self.component_by_id(&name)
                    && inc != ci
                    && let Some(p) = self.dict_in(inc, depth + 1, path, cancel)?
                {
                    parent = Some(p);
                }
            }
        }
        for &(off, id) in &chunks {
            if &id == b"Djbz" {
                let d = jb2::decode_dict(self.chunk_at(ci, off)?, parent.as_deref(), cancel)?;
                return Ok(Some(Arc::new(d)));
            }
        }
        Ok(parent)
    }

    fn decoded(&self, page: usize, cancel: &dyn Fn() -> bool) -> Result<Arc<Decoded>, Error> {
        let lock = self.decoding.get(page).ok_or(Error::NoSuchPage(page))?;
        let _g = lock.lock().map_err(|_| Error::Malformed("page lock"))?;
        if let Ok(mut c) = self.cache.lock()
            && let Some(i) = c.iter().position(|(p, _)| *p == page)
        {
            let e = c.remove(i).unwrap_or_else(|| unreachable!());
            let d = e.1.clone();
            c.push_front(e);
            return Ok(d);
        }
        let d = Arc::new(self.decode_page(page, cancel)?);
        if let Ok(mut c) = self.cache.lock() {
            c.push_front((page, d.clone()));
            c.truncate(PAGE_CACHE);
        }
        Ok(d)
    }

    fn decode_page(&self, page: usize, cancel: &dyn Fn() -> bool) -> Result<Decoded, Error> {
        let info = self.page_info(page)?;
        if u64::from(info.width) * u64::from(info.height) > MAX_PAGE_PIXELS {
            return Err(Error::TooLarge);
        }
        let chunks = self.page_chunks(page)?;
        let page_ci = self.pages[page];
        let mut bg: Option<iw44::Layer> = None;
        let mut bg_pic: Option<Picture> = None;
        let mut fg_pic: Option<Picture> = None;
        let mut mask = None;
        let mut fgbz = None;
        let mut dict: Option<Arc<jb2::Dict>> = None;
        // Dictionaries: one in the page itself, else the one an included file holds.
        for &(ci, off, id) in &chunks {
            if &id == b"Djbz" {
                if ci == page_ci {
                    dict = Some(Arc::new(jb2::decode_dict(
                        self.chunk_at(ci, off)?,
                        None,
                        cancel,
                    )?));
                } else if dict.is_none() {
                    dict = self.dict(ci, 0, cancel)?;
                }
            }
        }
        for &(ci, off, id) in &chunks {
            if ci != page_ci && !matches!(&id, b"FGbz") {
                // Included files contribute dictionaries, annotations and text only.
                continue;
            }
            let d = self.chunk_at(ci, off)?;
            match &id {
                b"BG44" | b"BM44" | b"PM44" => {
                    match &mut bg {
                        None => {
                            let mut l = iw44::Layer::new(d)?;
                            l.chunk(d, cancel)?;
                            bg = Some(l);
                        }
                        Some(l) => {
                            // A damaged refinement keeps the picture decoded so far.
                            if let Err(Error::Cancelled) = l.chunk(d, cancel) {
                                return Err(Error::Cancelled);
                            }
                        }
                    }
                }
                b"FG44" => {
                    let mut l = iw44::Layer::new(d)?;
                    l.chunk(d, cancel)?;
                    fg_pic = Some(l.picture());
                }
                b"BGjp" => bg_pic = Some(self.jpeg(d)?),
                b"FGjp" => fg_pic = Some(self.jpeg(d)?),
                b"Sjbz" => {
                    let img = jb2::decode_image(d, dict.as_deref(), cancel)?;
                    mask = Some(Mask::Jb2(img));
                }
                b"Smmr" => mask = Some(self.mmr(d)?),
                b"FGbz" => fgbz = Some(d),
                _ => {}
            }
        }
        if let Some(l) = bg {
            bg_pic = Some(l.picture());
        }
        let palette = match fgbz {
            Some(d) => Some(parse_fgbz(d)?),
            None => None,
        };
        let bins = match &mask {
            Some(Mask::Jb2(img)) => bin_blits(img, info.width, info.height)?,
            _ => Vec::new(),
        };
        let mut d = Decoded {
            info,
            bg: bg_pic,
            fg: fg_pic,
            mask,
            palette,
            bins,
            _held: specular_core::Reservation::none(),
        };
        d._held = specular_core::budget::reserve(d.memory_bytes() as u64)?;
        Ok(d)
    }

    fn jpeg(&self, d: &[u8]) -> Result<Picture, Error> {
        let dec = self
            .opts
            .jpeg
            .as_ref()
            .ok_or(Error::Unsupported("JPEG layers"))?;
        let (w, h, rgba) = dec(d).ok_or(Error::Malformed("JPEG layer"))?;
        if w == 0 || h == 0 || rgba.len() < w as usize * h as usize * 4 {
            return Err(Error::Malformed("JPEG layer"));
        }
        let data = rgba
            .chunks_exact(4)
            .flat_map(|p| [p[0], p[1], p[2]])
            .collect();
        Ok(Picture {
            width: w,
            height: h,
            channels: 3,
            data,
        })
    }

    fn mmr(&self, d: &[u8]) -> Result<Mask, Error> {
        let dec = self
            .opts
            .mmr
            .as_ref()
            .ok_or(Error::Unsupported("G4 masks"))?;
        if d.len() < 8 || &d[..3] != b"MMR" {
            return Err(Error::Malformed("G4 mask header"));
        }
        let flags = d[3];
        let w = u32::from(be16(d, 4).unwrap_or(0));
        let h = u32::from(be16(d, 6).unwrap_or(0));
        if w == 0 || h == 0 {
            return Err(Error::Malformed("G4 mask size"));
        }
        // A byte per pixel: the 8-byte header alone may declare 65535 × 65535.
        if u64::from(w) * u64::from(h) > iw44::MAX_PIXELS {
            return Err(Error::TooLarge);
        }
        let mut bits = Vec::with_capacity(w as usize * h as usize);
        if flags & 2 != 0 {
            let rps = u32::from(be16(d, 8).ok_or(Error::Malformed("G4 mask stripes"))?).max(1);
            let mut p = 10;
            let mut row = 0;
            while row < h {
                let n = be32(d, p).ok_or(Error::Malformed("G4 mask stripes"))? as usize;
                let s = d
                    .get(p + 4..p + 4 + n)
                    .ok_or(Error::Malformed("G4 mask stripes"))?;
                let rows = rps.min(h - row);
                let part = dec(s, w, rows).ok_or(Error::Malformed("G4 mask data"))?;
                bits.extend_from_slice(&part[..(w * rows) as usize]);
                row += rows;
                p += 4 + n;
            }
        } else {
            let part = dec(&d[8..], w, h).ok_or(Error::Malformed("G4 mask data"))?;
            bits.extend_from_slice(
                part.get(..(w * h) as usize)
                    .ok_or(Error::Malformed("G4 mask data"))?,
            );
        }
        if flags & 1 != 0 {
            for b in &mut bits {
                *b ^= 1;
            }
        }
        Ok(Mask::Bitmap {
            width: w,
            height: h,
            bits,
        })
    }

    /// Renders a region of page `page` as displayed (turned), at 1/`scale` of its pixel size:
    /// straight RGBA8, opaque. Region coordinates are in the scaled, turned page.
    pub fn render(&self, req: RenderRequest, cancel: &dyn Fn() -> bool) -> Result<Vec<u8>, Error> {
        let dec = self.decoded(req.page, cancel)?;
        let info = &dec.info;
        let (dw, dh) = info.display_size();
        let s = req.scale.max(1) as i64;
        let (sw, sh) = ((dw as i64 + s - 1) / s, (dh as i64 + s - 1) / s);
        let (x0, y0) = (i64::from(req.x), i64::from(req.y));
        let (w, h) = (i64::from(req.width), i64::from(req.height));
        if w == 0 || h == 0 || x0 + w > sw || y0 + h > sh {
            return Err(Error::TooLarge);
        }
        if (w * h) as u64 > 1 << 28 {
            return Err(Error::TooLarge);
        }
        let (pw, ph) = (i64::from(info.width), i64::from(info.height));
        // The block grid in the unturned page: first block's origin, and how output pixels
        // walk it.
        let (u0, v0, cols, rows) = match info.rotation {
            Rotation::None => (x0 * s, y0 * s, w, h),
            Rotation::Half => (pw - (x0 + w) * s, ph - (y0 + h) * s, w, h),
            Rotation::Ccw90 => (pw - (y0 + h) * s, x0 * s, h, w),
            Rotation::Cw90 => (y0 * s, ph - (x0 + w) * s, h, w),
        };
        let rgb = composite(&dec, u0, v0, s, cols as usize, rows as usize, cancel)?;
        let (w, h) = (w as usize, h as usize);
        let (cols, rows) = (cols as usize, rows as usize);
        let mut out = vec![255u8; w * h * 4];
        for oy in 0..h {
            for ox in 0..w {
                let (i, j) = match info.rotation {
                    Rotation::None => (ox, oy),
                    Rotation::Half => (cols - 1 - ox, rows - 1 - oy),
                    Rotation::Ccw90 => (cols - 1 - oy, ox),
                    Rotation::Cw90 => (oy, rows - 1 - ox),
                };
                let src = (j * cols + i) * 3;
                let dst = (oy * w + ox) * 4;
                out[dst..dst + 3].copy_from_slice(&rgb[src..src + 3]);
            }
        }
        Ok(out)
    }
}

/// A region of a page to render.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderRequest {
    pub page: usize,
    /// 1, 2, 4, 8…: output pixels are `scale`×`scale` page pixels averaged.
    pub scale: u32,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

fn count_bookmarks(b: &[Bookmark]) -> usize {
    b.iter().map(|x| 1 + count_bookmarks(&x.children)).sum()
}

fn parse_info(d: &[u8]) -> Result<PageInfo, Error> {
    if d.len() < 4 {
        return Err(Error::Malformed("INFO chunk"));
    }
    let width = u32::from(u16::from_be_bytes([d[0], d[1]]));
    let height = u32::from(u16::from_be_bytes([d[2], d[3]]));
    if width == 0 || height == 0 {
        return Err(Error::Malformed("page without pixels"));
    }
    let version = d.get(4).copied().unwrap_or(0);
    let mut dpi = match (d.get(6), d.get(7)) {
        (Some(&lo), Some(&hi)) => u32::from(u16::from_le_bytes([lo, hi])),
        _ => 300,
    };
    if !(25..=6000).contains(&dpi) {
        dpi = 300;
    }
    let mut gamma = d.get(8).copied().unwrap_or(22);
    if !(3..=50).contains(&gamma) {
        gamma = 22;
    }
    let rotation = match d.get(9).map(|f| f & 7) {
        Some(6) => Rotation::Ccw90,
        Some(2) => Rotation::Half,
        Some(5) => Rotation::Cw90,
        _ => Rotation::None,
    };
    Ok(PageInfo {
        width,
        height,
        dpi,
        gamma,
        rotation,
        version,
        layers: String::new(),
        has_text: false,
        title: None,
    })
}

/// The integer reduction of a layer of `lw`×`lh` against the page: the factor 1-12 whose
/// rounded-up quotients give the layer's size, else the nearest ratio.
fn reduction(pw: u32, ph: u32, lw: u32, lh: u32) -> u32 {
    for r in 1..=12 {
        if pw.div_ceil(r) == lw && ph.div_ceil(r) == lh {
            return r;
        }
    }
    (pw as f64 / lw.max(1) as f64).round().max(1.0) as u32
}

fn parse_fgbz(d: &[u8]) -> Result<Palette, Error> {
    let version = *d.first().ok_or(Error::Malformed("palette"))?;
    if version & 0x7f != 0 {
        return Err(Error::Unsupported("palette of a newer version"));
    }
    let n = usize::from(be16(d, 1).ok_or(Error::Malformed("palette"))?);
    let raw = d.get(3..3 + n * 3).ok_or(Error::Malformed("palette"))?;
    let colors = raw.chunks_exact(3).map(|c| [c[2], c[1], c[0]]).collect();
    let mut indices = Vec::new();
    if version & 0x80 != 0 {
        // The blit count (24 bits), then the BZZ-compressed 16-bit indices.
        let p = 3 + n * 3;
        let c = d.get(p..p + 3).ok_or(Error::Malformed("palette"))?;
        let m = (usize::from(c[0]) << 16) | (usize::from(c[1]) << 8) | usize::from(c[2]);
        let b = bzz::decode(&d[p + 3..], MAX_BZZ)?;
        indices = b
            .chunks_exact(2)
            .take(m)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect();
    }
    Ok(Palette { colors, indices })
}

/// Samples a layer picture stretched over the whole page at page pixel `(u, v)`
/// (centre-aligned bilinear).
#[inline]
fn sample(p: &Picture, pw: u32, ph: u32, u: f32, v: f32) -> [f32; 3] {
    let fx = (u + 0.5) * p.width as f32 / pw as f32 - 0.5;
    let fy = (v + 0.5) * p.height as f32 / ph as f32 - 0.5;
    let x0 = fx.floor();
    let y0 = fy.floor();
    let (ax, ay) = (fx - x0, fy - y0);
    let cl = |x: f32, m: u32| x.clamp(0.0, (m - 1) as f32) as u32;
    let (xa, xb) = (cl(x0, p.width), cl(x0 + 1.0, p.width));
    let (ya, yb) = (cl(y0, p.height), cl(y0 + 1.0, p.height));
    let c00 = p.rgb(xa, ya);
    let c10 = p.rgb(xb, ya);
    let c01 = p.rgb(xa, yb);
    let c11 = p.rgb(xb, yb);
    let mut o = [0f32; 3];
    for k in 0..3 {
        let t = f32::from(c00[k]) * (1.0 - ax) + f32::from(c10[k]) * ax;
        let b = f32::from(c01[k]) * (1.0 - ax) + f32::from(c11[k]) * ax;
        o[k] = t * (1.0 - ay) + b * ay;
    }
    o
}

/// Composites `cols`×`rows` output pixels, each the average of the `s`×`s` page pixels of
/// its block, blocks starting at page pixel `(u0, v0)` (may be outside the page: only the
/// pixels inside count). RGB, rows top-down.
fn composite(
    dec: &Decoded,
    u0: i64,
    v0: i64,
    s: i64,
    cols: usize,
    rows: usize,
    cancel: &dyn Fn() -> bool,
) -> Result<Vec<u8>, Error> {
    let info = &dec.info;
    let (pw, ph) = (i64::from(info.width), i64::from(info.height));
    let mut out = vec![255u8; cols * rows * 3];
    let fg_red = dec
        .fg
        .as_ref()
        .map_or(1, |p| reduction(info.width, info.height, p.width, p.height));
    // Band of output rows so the full-resolution mask buffer stays bounded.
    let fw = cols * s as usize;
    let band = ((8usize << 20) / fw.max(1) / s as usize).clamp(1, rows.max(1));
    // Per page pixel: 0 = background, else 1 + colour slot (palette index or 0 for "the
    // foreground colour at this pixel").
    let mut buf: Vec<u32> = Vec::new();
    let mut j0 = 0;
    while j0 < rows {
        if cancel() {
            return Err(Error::Cancelled);
        }
        let j1 = (j0 + band).min(rows);
        let bv0 = v0 + j0 as i64 * s;
        let bh = (j1 - j0) * s as usize;
        buf.clear();
        buf.resize(fw * bh, 0);
        paint_mask(dec, u0, bv0, fw, bh, &mut buf);
        for j in j0..j1 {
            for i in 0..cols {
                let bu = u0 + i as i64 * s;
                let bv = v0 + j as i64 * s;
                let (ua, ub) = (bu.max(0), (bu + s).min(pw));
                let (va, vb) = (bv.max(0), (bv + s).min(ph));
                if ua >= ub || va >= vb {
                    continue;
                }
                let n = ((ub - ua) * (vb - va)) as f32;
                let mut fg = [0f32; 3];
                let mut covered = 0u32;
                for v in va..vb {
                    let row = ((v - bv0) as usize) * fw;
                    for u in ua..ub {
                        let m = buf[row + (u - u0) as usize];
                        if m == 0 {
                            continue;
                        }
                        covered += 1;
                        let c = fg_colour(dec, fg_red, m, u as u32, v as u32);
                        fg[0] += f32::from(c[0]);
                        fg[1] += f32::from(c[1]);
                        fg[2] += f32::from(c[2]);
                    }
                }
                let alpha = covered as f32 / n;
                let bgc = match &dec.bg {
                    Some(p) => {
                        let cu = (ua + ub - 1) as f32 * 0.5;
                        let cv = (va + vb - 1) as f32 * 0.5;
                        if covered as f32 >= n {
                            [0.0; 3]
                        } else {
                            sample(p, info.width, info.height, cu, cv)
                        }
                    }
                    None => [255.0; 3],
                };
                let o = (j * cols + i) * 3;
                for k in 0..3 {
                    let v = bgc[k] * (1.0 - alpha) + fg[k] / n;
                    out[o + k] = v.round().clamp(0.0, 255.0) as u8;
                }
            }
        }
        j0 = j1;
    }
    Ok(out)
}

#[inline]
fn fg_colour(dec: &Decoded, red: u32, m: u32, u: u32, v: u32) -> [u8; 3] {
    if let Some(p) = &dec.palette {
        return p.colors.get((m - 1) as usize).copied().unwrap_or([0, 0, 0]);
    }
    if let Some(f) = &dec.fg {
        return f.rgb(u / red, v / red);
    }
    [0, 0, 0]
}

/// Paints the mask over page pixels `[u0, u0 + w) × [v0, v0 + h)` into `buf` (see
/// [`composite`]).
fn paint_mask(dec: &Decoded, u0: i64, v0: i64, w: usize, h: usize, buf: &mut [u32]) {
    match &dec.mask {
        None => {}
        Some(Mask::Bitmap {
            width,
            height,
            bits,
        }) => {
            for y in 0..h {
                let v = v0 + y as i64;
                if v < 0 || v >= i64::from(*height) {
                    continue;
                }
                for x in 0..w {
                    let u = u0 + x as i64;
                    if u < 0 || u >= i64::from(*width) {
                        continue;
                    }
                    if bits[v as usize * *width as usize + u as usize] != 0 {
                        buf[y * w + x] = 1;
                    }
                }
            }
        }
        Some(Mask::Jb2(img)) => {
            let (va, vb) = (v0.max(0), (v0 + h as i64).min(i64::from(dec.info.height)));
            if va >= vb {
                return;
            }
            let mut ids: Vec<u32> = Vec::new();
            for bin in (va / i64::from(BIN_ROWS))..=((vb - 1) / i64::from(BIN_ROWS)) {
                if let Some(b) = dec.bins.get(bin as usize) {
                    ids.extend_from_slice(b);
                }
            }
            ids.sort_unstable();
            ids.dedup();
            for id in ids {
                let b = img.blits[id as usize];
                let s = &img.shapes[b.shape as usize];
                let slot = match &dec.palette {
                    Some(p) => {
                        1 + u32::from(p.indices.get(id as usize).copied().unwrap_or(u16::MAX))
                    }
                    None => 1,
                };
                let (sw, sh) = (i64::from(s.width), i64::from(s.height));
                let (bx, by) = (i64::from(b.left), i64::from(b.top));
                let ya = (by.max(v0)).max(0);
                let yb = (by + sh).min(v0 + h as i64).min(i64::from(dec.info.height));
                let xa = (bx.max(u0)).max(0);
                let xb = (bx + sw).min(u0 + w as i64).min(i64::from(dec.info.width));
                for y in ya..yb {
                    let src = ((y - by) * sw) as usize;
                    let dst = ((y - v0) as usize) * w;
                    for x in xa..xb {
                        if s.bits[src + (x - bx) as usize] != 0 {
                            buf[dst + (x - u0) as usize] = slot;
                        }
                    }
                }
            }
        }
    }
}

/// The blits of a JB2 page by band of [`BIN_ROWS`] rows (for drawing a region).
fn bin_blits(img: &jb2::Image, width: u32, height: u32) -> Result<Vec<Vec<u32>>, Error> {
    let n = (height as i32 + BIN_ROWS - 1) / BIN_ROWS;
    let mut bins = vec![Vec::new(); n.max(1) as usize];
    let mut entries = 0usize;
    for (i, b) in img.blits.iter().enumerate() {
        let s = &img.shapes[b.shape as usize];
        if s.width == 0 || s.height == 0 {
            continue;
        }
        let y0 = b.top.max(0);
        let y1 = (b.top + s.height as i32).min(height as i32);
        if y1 <= y0 || b.left >= width as i32 || b.left + (s.width as i32) <= 0 {
            continue;
        }
        let span = (y0 / BIN_ROWS)..=((y1 - 1) / BIN_ROWS);
        entries += span.clone().count();
        if entries > MAX_BIN_ENTRIES {
            return Err(Error::TooLarge);
        }
        for bin in span {
            bins[bin as usize].push(i as u32);
        }
    }
    Ok(bins)
}

#[cfg(test)]
mod tests;
