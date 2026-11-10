//! Sketch documents and Figma local copies.
//!
//! **Sketch** (version 43 and later) is a ZIP of JSON: `document.json` (the page list and
//! the symbols of linked libraries), `meta.json` (the writing app's version),
//! `pages/<id>.json` (each page's layer tree), `images/` (bitmaps) and `previews/preview.png`
//! (a render of the page last viewed, for file browsers). [`open_sketch`] draws every
//! artboard (and symbol master) as a page of its own, at the artboard's size, and the layers
//! of a page outside any artboard as one more page sized to their extent. [`sketch_preview`]
//! finds the embedded preview. Files from before version 43 are a binary database and are
//! refused.
//!
//! Drawn: rectangles (fixed and per-corner radii), ovals, triangles, stars, polygons and
//! vector paths (Bézier points relative to the frame), shape groups (union as one fill;
//! subtract and difference as even-odd, intersect approximated the same way), groups with
//! rotation and flips, symbol instances (the master scaled to the instance, text and nested
//! symbol overrides), bitmaps, text (attributed runs: font, size, colour, alignment, line
//! height, kerning, underline and strike-through; auto-width or fixed boxes), fills (solid,
//! linear and radial gradients, image patterns), borders (centre, inside and outside,
//! dashes, caps, joins), layer and fill opacity, clipping masks. Not drawn, counted in
//! [`SketchFacts::not_drawn`]: shadows, blurs, noise fills, angular gradients (drawn as
//! linear), text stored in the old archived encoding (before Sketch 48), blend modes.
//!
//! **Figma** local copies (`.fig`, "Save local copy") are a ZIP of `canvas.fig` (Figma's
//! own binary "fig-kiwi" encoding, undocumented), `thumbnail.png` and `meta.json`. Only the
//! thumbnail is shown ([`figma_thumbnail`]); a bare `canvas.fig` without it is refused.
//!
//! Bounds: JSON members up to [`MAX_JSON`] bytes, [`MAX_LAYERS`] layers drawn per document,
//! [`MAX_DEPTH`] nesting levels, [`MAX_SYMBOL_DEPTH`] nested symbols (a symbol using itself
//! is drawn once), images up to [`specular_drawpage::MAX_IMAGE_PIXELS`] pixels in all.
//! Hostile files draw less or fail; they never panic or loop.

mod sketch;

use std::ops::Range;

use specular_ora::zip::Zip;

pub use sketch::{MAX_DEPTH, MAX_LAYERS, MAX_SYMBOL_DEPTH, open_sketch, open_sketch_overview};

/// Largest JSON member read.
pub const MAX_JSON: u64 = 128 << 20;
/// Largest preview or image member read.
pub const MAX_IMAGE_MEMBER: u64 = 256 << 20;

/// Why a document cannot be read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("not a Sketch document")]
    NotSketch,
    #[error("not a Figma file")]
    NotFigma,
    /// A variant or content that is recognised but not read, with the reason.
    #[error("{0}")]
    Unsupported(String),
    #[error("damaged file: {0}")]
    Malformed(String),
}

/// Where an embedded image is: a stored range of the file, or inflated bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Bytes {
    Range(Range<usize>),
    Owned(Vec<u8>),
}

impl Bytes {
    /// The bytes, given the file they came from.
    pub fn get<'a>(&'a self, data: &'a [u8]) -> &'a [u8] {
        match self {
            Bytes::Range(r) => data.get(r.clone()).unwrap_or(&[]),
            Bytes::Owned(v) => v,
        }
    }
}

/// What a Sketch document says about itself, for the info panel.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SketchFacts {
    /// `meta.json` appVersion ("70.3").
    pub app_version: Option<String>,
    /// `meta.json` version (the file format number, e.g. 136).
    pub version: Option<u64>,
    /// `meta.json` compatibilityVersion.
    pub compatibility_version: Option<u64>,
    /// The writing app's bundle id ("com.bohemiancoding.sketch3"); other apps write the
    /// format too.
    pub app: Option<String>,
    /// The document's page names, in order.
    pub sketch_pages: Vec<String>,
    /// One label per drawn page: "Page 1 / Home".
    pub page_labels: Vec<String>,
    /// For each drawn page: the artboard's name, or `None` for the layers outside artboards.
    pub artboards: Vec<Option<String>>,
    /// Layers by class, in first-seen order.
    pub layer_counts: Vec<(String, u64)>,
    /// What was not drawn (or only approximated), with counts.
    pub not_drawn: Vec<(String, u64)>,
    /// Font names the text uses (PostScript names as stored).
    pub fonts: Vec<String>,
    /// Images referenced but missing or undecodable.
    pub images_missing: u64,
    /// A document limit was hit: some content is left out.
    pub limited: bool,
    /// Symbol masters in the document. They are pages of their own only when the document
    /// has no artboards or other layers (a pure symbol library).
    pub symbol_masters: usize,
}

impl SketchFacts {
    /// The coding line for drawn page `page`.
    pub fn describe(&self, page: usize) -> String {
        let mut s = String::from("Sketch document");
        if let Some(v) = &self.app_version {
            s.push_str(&format!(" (Sketch {v})"));
        }
        match self.artboards.get(page) {
            Some(Some(a)) => s.push_str(&format!(", artboard \"{a}\"")),
            Some(None) => {
                if let Some(l) = self.page_labels.get(page) {
                    s.push_str(&format!(", page \"{l}\" outside artboards"));
                }
            }
            None => {}
        }
        if !self.not_drawn.is_empty() {
            let what: Vec<&str> = self.not_drawn.iter().map(|(k, _)| k.as_str()).collect();
            s.push_str(&format!(", not drawn: {}", what.join(", ")));
        }
        if self.images_missing > 0 {
            let n = self.images_missing;
            s.push_str(&format!(
                ", {n} image{} missing",
                if n == 1 { "" } else { "s" }
            ));
        }
        if self.limited {
            s.push_str(", too large: partly drawn");
        }
        s
    }
}

/// A drawn Sketch document: one page per artboard, then the loose layers' pages.
#[derive(Debug, Clone)]
pub struct SketchDocument {
    pub pages: Vec<specular_drawpage::Page>,
    pub facts: SketchFacts,
}

const SQLITE_MAGIC: &[u8] = b"SQLite format 3\0";
const OLD_SKETCH: &str =
    "Sketch file from before version 43 (2017, a binary database): not supported";

pub(crate) fn zip_of(data: &[u8], not: Error) -> Result<Zip, Error> {
    if data.starts_with(SQLITE_MAGIC) {
        return Err(Error::Unsupported(OLD_SKETCH.into()));
    }
    if !data.starts_with(b"PK") {
        return Err(not);
    }
    Zip::parse(data).map_err(|e| Error::Malformed(format!("ZIP: {e}")))
}

/// A member's bytes: stored range when uncompressed, else inflated (at most `max`).
pub(crate) fn member(data: &[u8], zip: &Zip, name: &str, max: u64) -> Result<Option<Bytes>, Error> {
    let Some(e) = zip.find(name) else {
        return Ok(None);
    };
    if e.is_stored()
        && let Ok(Some(r)) = zip.stored_range(data, e)
    {
        return Ok(Some(Bytes::Range(r)));
    }
    let b = zip
        .read(data, e, max)
        .map_err(|err| Error::Malformed(format!("{name}: {err}")))?;
    Ok(Some(Bytes::Owned(b.into_owned())))
}

/// A ZIP whose directory names `document.json`, `meta.json` and a `pages/` member.
pub fn probe_sketch_file(data: &[u8]) -> bool {
    if !data.starts_with(b"PK") {
        return false;
    }
    let Ok(z) = Zip::parse(data) else {
        return false;
    };
    z.find("document.json").is_some()
        && z.find("meta.json").is_some()
        && z.entries.iter().any(|e| e.name.starts_with("pages/"))
}

/// The embedded preview (`previews/preview.png`): a render of the page last viewed.
pub fn sketch_preview(data: &[u8]) -> Result<Bytes, Error> {
    let zip = zip_of(data, Error::NotSketch)?;
    member(data, &zip, "previews/preview.png", MAX_IMAGE_MEMBER)?
        .ok_or_else(|| Error::Unsupported("Sketch document without an embedded preview".into()))
}

/// A bare Figma canvas: the "fig-kiwi" header.
pub fn probe_figma(head: &[u8]) -> bool {
    head.starts_with(b"fig-kiwi")
}

/// A Figma local copy: a ZIP with `canvas.fig` and a thumbnail or `meta.json`.
pub fn probe_figma_file(data: &[u8]) -> bool {
    if probe_figma(data) {
        return true;
    }
    if !data.starts_with(b"PK") {
        return false;
    }
    let Ok(z) = Zip::parse(data) else {
        return false;
    };
    z.find("canvas.fig").is_some()
        && (z.find("thumbnail.png").is_some() || z.find("meta.json").is_some())
}

const FIGMA_UNDOCUMENTED: &str =
    "Figma file without a thumbnail: Figma's own format is undocumented and is not drawn";

/// The thumbnail Figma stores in a local copy (`thumbnail.png`).
pub fn figma_thumbnail(data: &[u8]) -> Result<Bytes, Error> {
    if probe_figma(data) {
        return Err(Error::Unsupported(FIGMA_UNDOCUMENTED.into()));
    }
    let zip = zip_of(data, Error::NotFigma)?;
    if zip.find("canvas.fig").is_none() {
        return Err(Error::NotFigma);
    }
    member(data, &zip, "thumbnail.png", MAX_IMAGE_MEMBER)?
        .ok_or_else(|| Error::Unsupported(FIGMA_UNDOCUMENTED.into()))
}

/// What a Figma local copy's `meta.json` says.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FigmaFacts {
    pub file_name: Option<String>,
    pub exported_at: Option<String>,
    /// The canvas background, straight RGBA8.
    pub background: Option<[u8; 4]>,
    /// The thumbnail's size as recorded.
    pub thumbnail_size: Option<(u32, u32)>,
}

/// The facts of a Figma local copy's `meta.json` (empty when absent or unreadable).
pub fn figma_facts(data: &[u8]) -> FigmaFacts {
    let mut f = FigmaFacts::default();
    let Ok(zip) = zip_of(data, Error::NotFigma) else {
        return f;
    };
    let Ok(Some(b)) = member(data, &zip, "meta.json", 1 << 20) else {
        return f;
    };
    let Ok(v) = serde_json::from_slice::<serde_json::Value>(b.get(data)) else {
        return f;
    };
    let s = |k: &str| v.get(k).and_then(|x| x.as_str()).map(str::to_string);
    f.file_name = s("file_name");
    f.exported_at = s("exported_at");
    if let Some(cm) = v.get("client_meta") {
        if let Some(c) = cm.get("background_color") {
            let ch = |k: &str| {
                let x = c.get(k).and_then(|x| x.as_f64()).unwrap_or(1.0);
                (x.clamp(0.0, 1.0) * 255.0).round() as u8
            };
            f.background = Some([ch("r"), ch("g"), ch("b"), ch("a")]);
        }
        if let Some(t) = cm.get("thumbnail_size") {
            let d = |k: &str| t.get(k).and_then(|x| x.as_u64()).map(|x| x as u32);
            if let (Some(w), Some(h)) = (d("width"), d("height")) {
                f.thumbnail_size = Some((w, h));
            }
        }
    }
    f
}

#[cfg(test)]
mod tests;
