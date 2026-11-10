//! Computer Graphics Metafile (ISO/IEC 8632) pictures, one page per picture.
//!
//! [`Document::open`] parses a binary (8632-3) or clear-text (8632-4) metafile once into a
//! display list per picture (`list`): geometry in page points, colours resolved, cell arrays
//! as images. [`Document::render`] draws any rectangle of a picture at any scale with
//! vello_cpu onto the picture's background, so tiles and deep zoom cost only the pixels asked
//! for. Text goes through the drawing stack's fonts (`specular-vector`): the named family
//! when installed, else a metric twin (Helvetica → Arial/Liberation Sans, Times → Liberation
//! Serif, Courier → Liberation Mono), else the bundled Inter.
//!
//! Page size: a metric picture (SCALING MODE metric) is its VDC extent in millimetres; an
//! abstract one is fitted to 11 inches on its long side. Pictures over 200 inches are scaled
//! down to it.
//!
//! Drawn: polylines, disjoint polylines, polymarkers, polygons and polygon sets (with edge
//! flags), rectangles, circles, ellipses, circular and elliptical arcs (open, pie, chord),
//! polybéziers, cell arrays (packed and run-length, 1–32-bit colours, transparent cell
//! colour), text, restricted text and appended text (font list, height, expansion, spacing,
//! orientation, path, alignment), interior styles hollow, solid, pattern (pattern tables),
//! hatch (the six standard hatches) and empty, edges, line and edge types (standard and
//! defined), widths in all four specification modes, line caps and joins, indexed and
//! direct colour with colour tables and value extents, clip rectangles, auxiliary colour and
//! transparency, metafile defaults replacement. Not drawn, counted in [`Facts::skipped`]:
//! tile arrays, generalized drawing primitives, hyperbolic and parabolic arcs, B-splines,
//! polysymbols, hatch style and geometric pattern definitions.
//!
//! Bounds: every list is limited by the bytes holding it; [`interp::MAX_OPS`] display-list
//! entries, [`interp::MAX_POINTS`] points and [`interp::MAX_CELLS`] cell array pixels per
//! document. Hostile files draw less or fail; they never panic or loop.

mod args;
mod binary;
mod clear;
mod interp;
mod list;
mod render;

use std::sync::{Arc, OnceLock};

use specular_vector::fonts::{Face, Fonts};

pub use interp::{MAX_CELLS, MAX_OPS, MAX_POINTS};

/// The largest side of a single render (vello_cpu's pixmaps are `u16` wide).
pub const MAX_RENDER_SIDE: u32 = u16::MAX as u32;
/// The largest single render, in pixels (1 GiB of RGBA).
pub const MAX_RENDER_PIXELS: u64 = 256 * 1024 * 1024;

/// Why a metafile cannot be opened.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OpenError {
    /// Not a CGM at all.
    #[error("not a CGM file")]
    NotCgm,
    /// The character encoding (8632-2), which nothing writes any more.
    #[error("CGM in the character encoding: not supported, only binary and clear text")]
    CharacterEncoding,
    /// A metafile that ends before its first picture is complete.
    #[error("damaged CGM: no complete picture")]
    NoPictures,
}

/// Why a render failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RenderError {
    /// The caller cancelled the render.
    #[error("cancelled")]
    Cancelled,
    /// The requested raster is empty or larger than the render bounds.
    #[error("picture raster {0}×{1} is outside the render bounds")]
    TooLarge(u32, u32),
    /// No such picture.
    #[error("picture {0} does not exist")]
    NoSuchPage(usize),
    /// The renderer could not draw the picture.
    #[error("the picture could not be drawn: {0}")]
    Failed(String),
}

/// The rectangle of a picture to render, in output pixels, and the picture's scale.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderRequest {
    /// Zero-based picture index.
    pub page: usize,
    /// Output pixels per point, horizontally.
    pub scale_x: f64,
    /// Output pixels per point, vertically.
    pub scale_y: f64,
    /// Left edge of the region in the scaled picture raster.
    pub x: u32,
    /// Top edge of the region in the scaled picture raster.
    pub y: u32,
    /// Region width in pixels.
    pub width: u32,
    /// Region height in pixels.
    pub height: u32,
}

/// What a metafile says about itself, for the info panel.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Facts {
    /// "binary" or "clear text".
    pub encoding: &'static str,
    /// METAFILE VERSION (1–4).
    pub version: Option<u32>,
    /// METAFILE DESCRIPTION as stored (often a producer, a date and a profile).
    pub description: Option<String>,
    /// The profile the description names, e.g. "WebCGM 2.0", "ATA", "Model".
    pub profile: Option<String>,
    /// The metafile name of BEGIN METAFILE.
    pub name: Option<String>,
    /// Picture names, in order (empty strings where unnamed).
    pub pictures: Vec<String>,
    /// VDC are reals (else integers).
    pub vdc_real: bool,
    /// "indexed", "direct" or "indexed and direct".
    pub colour: &'static str,
    /// Some picture is in metric units (its size on the page is real).
    pub metric: bool,
    /// The FONT LIST.
    pub fonts: Vec<String>,
    /// Elements read.
    pub elements: u64,
    /// Drawing elements that are not drawn, by name, with counts.
    pub skipped: Vec<(String, u64)>,
    /// The file ends inside an element: what came before is drawn.
    pub truncated: bool,
    /// A document limit was hit (points, cells, display list): some content is left out.
    pub limited: bool,
    /// Some picture is larger than 200 inches and is shown scaled down.
    pub scaled_down: bool,
}

/// A parsed metafile.
pub struct Document {
    pictures: Vec<list::Picture>,
    facts: Facts,
    faces: OnceLock<Vec<Arc<Face>>>,
}

impl std::fmt::Debug for Document {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Document")
            .field("pictures", &self.pictures.len())
            .finish()
    }
}

/// Whether `head` starts like a CGM: the binary BEGIN METAFILE header word (class 0, id 1),
/// or the clear-text keyword `BEGMF` (after blanks and comments).
pub fn looks_like_cgm(head: &[u8]) -> bool {
    if head.len() >= 2 && head[0] == 0x00 && (head[1] & 0xe0) == 0x20 {
        // Class 0, id 1: a name string follows; length 0..=30 or long form. The two bytes
        // alone also start a QuickTime atom of 2.1 MB (`00 21 d4 00 mdat`, Minolta's
        // Motion JPEG movies): the string must fit and the next element be a metafile one.
        return binary_header_fits(head);
    }
    clear::starts_with_begmf(head)
}

/// The binary BEGIN METAFILE element's name string fits its length, and the element after it
/// is of class 0 (delimiters) or 1 (metafile descriptor). Long forms are taken on trust.
fn binary_header_fits(head: &[u8]) -> bool {
    let len = usize::from(head[1] & 0x1f);
    if len == 31 {
        return true;
    }
    if len > 0 {
        let Some(&s) = head.get(2) else {
            return true;
        };
        // A string of `s` bytes after its length byte (255: long string, not in a short
        // element).
        if s == 255 || usize::from(s) + 1 > len {
            return false;
        }
    }
    let next = 2 + len + (len & 1);
    match head.get(next) {
        Some(&b) => b >> 4 <= 1,
        None => true,
    }
}

/// Whether `head` is the character encoding (8632-2): BEGIN METAFILE is `ESC 0x30 0x20`.
fn is_character_encoding(head: &[u8]) -> bool {
    head.starts_with(&[0x30, 0x20])
}

impl Document {
    /// Parses a metafile: every picture into its display list.
    pub fn open(data: &[u8]) -> Result<Document, OpenError> {
        let mut it = interp::Interp::new();
        let mut facts = Facts::default();
        if data.len() >= 2 && data[0] == 0x00 && (data[1] & 0xe0) == 0x20 {
            facts.encoding = "binary";
            let mut els = binary::Elements::new(data);
            for e in els.by_ref() {
                if e.class == 1 && e.id == 12 {
                    // METAFILE DEFAULTS REPLACEMENT: elements in its parameters.
                    it.element(
                        1,
                        12,
                        &mut binary::Params::new(&e.data, &args::Prec::default()),
                    );
                    it.set_mdr(true);
                    let inner = binary::Elements::new(&e.data);
                    for ie in inner {
                        let prec = it.prec().clone();
                        let mut a = binary::Params::new(&ie.data, &prec);
                        it.element(ie.class, ie.id, &mut a);
                    }
                    it.set_mdr(false);
                    continue;
                }
                if e.class == 0 && e.id == 1 {
                    let prec = it.prec().clone();
                    let mut a = binary::Params::new(&e.data, &prec);
                    use args::Args as _;
                    facts.name = a
                        .string()
                        .map(|s| interp::latin1(&s))
                        .filter(|s| !s.is_empty());
                }
                let prec = it.prec().clone();
                let mut a = binary::Params::new(&e.data, &prec);
                it.element(e.class, e.id, &mut a);
                if it.ended {
                    break;
                }
            }
            if els.truncated {
                it.meta.truncated = true;
            }
        } else if clear::starts_with_begmf(data) {
            facts.encoding = "clear text";
            facts.name = clear::run(data, &mut it);
        } else if is_character_encoding(data) {
            return Err(OpenError::CharacterEncoding);
        } else {
            return Err(OpenError::NotCgm);
        }
        // A file cut short inside a picture still shows what it drew.
        if !it.ended {
            it.element(0, 5, &mut binary::Params::new(&[], &args::Prec::default()));
        }
        if it.pictures.is_empty() {
            return Err(OpenError::NoPictures);
        }
        let m = &it.meta;
        facts.version = m.version.and_then(|v| u32::try_from(v).ok());
        facts.description = m.description.clone().filter(|s| !s.trim().is_empty());
        facts.profile = facts.description.as_deref().and_then(profile);
        facts.pictures = it.pictures.iter().map(|p| p.name.clone()).collect();
        facts.vdc_real = m.vdc_real;
        facts.colour = match (m.indexed_colour || !m.direct_colour, m.direct_colour) {
            (true, true) => "indexed and direct",
            (false, true) => "direct",
            _ => "indexed",
        };
        facts.metric = m.metric;
        facts.fonts = m.fonts.clone();
        facts.elements = m.elements;
        facts.skipped = m
            .skipped
            .iter()
            .map(|(k, v)| ((*k).to_string(), *v))
            .collect();
        facts.truncated = m.truncated;
        facts.limited = m.limited;
        facts.scaled_down = m.scaled_down;
        Ok(Document {
            pictures: it.pictures,
            facts,
            faces: OnceLock::new(),
        })
    }

    /// Number of pictures (at least one).
    pub fn page_count(&self) -> usize {
        self.pictures.len()
    }

    /// Size of picture `page` in points (1/72 inch).
    pub fn page_size(&self, page: usize) -> Option<(f64, f64)> {
        self.pictures.get(page).map(|p| p.size)
    }

    /// What the metafile says about itself.
    pub fn facts(&self) -> Facts {
        self.facts.clone()
    }

    /// Display-list entries of picture `page` (for tests and diagnostics).
    pub fn op_count(&self, page: usize) -> usize {
        self.pictures.get(page).map_or(0, |p| p.ops.len())
    }

    /// Renders a region of a picture to straight RGBA8 (`width × height × 4` bytes, every
    /// pixel opaque). `cancelled` is polled between drawing operations.
    pub fn render(
        &self,
        req: RenderRequest,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, RenderError> {
        let pic = self
            .pictures
            .get(req.page)
            .ok_or(RenderError::NoSuchPage(req.page))?;
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
        if cancelled() {
            return Err(RenderError::Cancelled);
        }
        let fonts = Fonts::shared();
        let faces = self
            .faces
            .get_or_init(|| render::resolve_faces(&self.facts.fonts, &fonts));
        render::render(
            pic,
            faces,
            &fonts,
            (req.scale_x, req.scale_y),
            (req.x, req.y),
            (w as u16, h as u16),
            cancelled,
        )
        .ok_or_else(|| {
            if cancelled() {
                RenderError::Cancelled
            } else {
                RenderError::Failed("out of memory".into())
            }
        })
    }
}

/// The profile a METAFILE DESCRIPTION names: `ProfileId:WebCGM ProfileEd:2.0` → "WebCGM 2.0".
fn profile(desc: &str) -> Option<String> {
    let field = |key: &str| {
        let i = desc.find(key)? + key.len();
        let rest = desc[i..].trim_start_matches([' ', '"', '\'']);
        let v: String = rest
            .chars()
            .take_while(|c| !c.is_whitespace() && *c != '"' && *c != '\'' && *c != ',')
            .collect();
        (!v.is_empty()).then_some(v)
    };
    let id = field("ProfileId:")?;
    Some(match field("ProfileEd:") {
        Some(ed) => format!("{id} {ed}"),
        None => id,
    })
}

#[doc(hidden)]
pub mod fixture;
#[cfg(test)]
mod tests;
// Outside tests only the fixture uses it, and not every method.
#[cfg_attr(not(test), allow(dead_code))]
mod writer;
