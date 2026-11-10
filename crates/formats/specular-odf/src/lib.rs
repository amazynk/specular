//! OpenDocument packages: Draw drawings (`.odg`, `.otg`, flat `.fodg`) and Impress
//! presentations (`.odp`) rendered as pages from their shapes, and the
//! `Thumbnails/thumbnail.png` every package carries (the preview for text documents and
//! spreadsheets).
//!
//! [`open_graphics`] reads `styles.xml`, `content.xml` and `meta.xml` once into a display list
//! per page ([`specular_drawpage::Page`]): the page size from its master page's page layout,
//! the drawing-page background, the master page's shapes under the page's own, then the
//! shapes in document order.
//!
//! Drawn: rectangles (rounded), ellipses and circles (full, section, cut, arc), lines,
//! polylines, polygons, regular polygons and stars, paths, connectors (their stored route),
//! measure lines (the line), captions (the box), custom shapes from their enhanced geometry
//! (equations, modifiers, every enhanced-path command, mirroring, text areas; the common
//! presets when no path is stored), frames with images (embedded PNG, JPEG, …, through the
//! caller's decoder) and text boxes, groups, `draw:transform`. Styles with parent chains and
//! defaults: solid, gradient (linear, axial, radial, ellipsoid, square, rectangular, with
//! border, angle, centre, intensity and multi-colour stops), bitmap (tiled or stretched) and
//! hatch fills, opacity, solid and dashed strokes with caps and joins, arrow markers,
//! shadows. Text: paragraphs, spans, spaces, tabs, line breaks, lists (bulleted), with
//! fonts, sizes, weight, slant, colour, underline, strike-through, alignment, line height,
//! margins, padding, vertical anchoring and wrapping. Hidden layers are left out.
//!
//! Not drawn, counted in [`Facts::not_drawn`]: 3D scenes, form controls, embedded objects
//! (charts, formulas, OLE) without a decodable replacement image, page thumbnails, text on
//! lines and connectors.
//!
//! Bounds: [`draw::MAX_SHAPES`] shapes, [`specular_drawpage::MAX_OPS`] display-list entries
//! and [`specular_drawpage::MAX_IMAGE_PIXELS`] image pixels per document; parts of at most
//! [`MAX_PART`] bytes; XML nesting and node counts bounded by `specular_drawpage::xml`.

mod draw;
mod geometry;
mod measure;
mod style;
mod units;

use std::ops::Range;

use specular_drawpage::kurbo::Affine;
use specular_drawpage::xml::roxmltree::{Document as Xml, Node};
use specular_drawpage::{ImageDecoder, Page};
use specular_ora::zip::Zip;

pub use draw::MAX_SHAPES;

/// Largest XML part or image member read.
pub const MAX_PART: u64 = 256 << 20;
/// Most pages read.
pub const MAX_PAGES: usize = 10_000;
/// A4 portrait, points: the page when no layout gives one.
const A4: (f64, f64) = (595.276, 841.89);

const MIME_PREFIX: &str = "application/vnd.oasis.opendocument.";

/// What an OpenDocument package holds, by its `mimetype`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    Graphics,
    Presentation,
    Spreadsheet,
    Text,
    /// Another OpenDocument type (chart, formula, image, database…): the subtype.
    Other(String),
}

impl Kind {
    fn from_mime(m: &str) -> Option<Kind> {
        let sub = m.trim().strip_prefix(MIME_PREFIX)?;
        let base = sub.strip_suffix("-template").unwrap_or(sub);
        Some(match base {
            "graphics" | "graphics-flat-xml" => Kind::Graphics,
            "presentation" | "presentation-flat-xml" => Kind::Presentation,
            "spreadsheet" | "spreadsheet-flat-xml" => Kind::Spreadsheet,
            "text" | "text-master" | "text-web" | "text-flat-xml" => Kind::Text,
            other => Kind::Other(other.to_string()),
        })
    }

    /// "drawing", "presentation", "spreadsheet", "text document", "document".
    pub fn noun(&self) -> &'static str {
        match self {
            Kind::Graphics => "drawing",
            Kind::Presentation => "presentation",
            Kind::Spreadsheet => "spreadsheet",
            Kind::Text => "text document",
            Kind::Other(_) => "document",
        }
    }
}

/// Why a package cannot be read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("not an OpenDocument file")]
    NotOdf,
    #[error("password-protected OpenDocument file: not supported")]
    Encrypted,
    #[error("{0}")]
    Unsupported(String),
    #[error("damaged OpenDocument file: {0}")]
    Malformed(String),
}

fn zip_err(e: specular_ora::Error) -> Error {
    Error::Malformed(e.to_string())
}

/// The OpenDocument type of `data`: a ZIP package whose `mimetype` member names one, or a
/// flat XML document (`office:document` with `office:mimetype`).
pub fn probe_file(data: &[u8]) -> Option<Kind> {
    if data.starts_with(b"PK\x03\x04") {
        // The first member is normally `mimetype`, stored: read it from the local header.
        let name_len = usize::from(u16::from_le_bytes([*data.get(26)?, *data.get(27)?]));
        let extra = usize::from(u16::from_le_bytes([*data.get(28)?, *data.get(29)?]));
        let size = u32::from_le_bytes(data.get(18..22)?.try_into().ok()?) as usize;
        if data.get(30..30 + name_len) == Some(b"mimetype") && data.get(8..10) == Some(&[0, 0]) {
            let at = 30 + name_len + extra;
            let m = data.get(at..at + size.min(200))?;
            return Kind::from_mime(std::str::from_utf8(m).ok()?);
        }
        let zip = Zip::parse(data).ok()?;
        let e = zip.find("mimetype")?;
        let m = zip.read(data, e, 256).ok()?;
        return Kind::from_mime(std::str::from_utf8(&m).ok()?);
    }
    let head = &data[..data.len().min(8192)];
    let head = head.strip_prefix(b"\xef\xbb\xbf").unwrap_or(head);
    if !head.starts_with(b"<?xml") && !head.starts_with(b"<office:document") {
        return None;
    }
    let text = String::from_utf8_lossy(head);
    if !text.contains("<office:document") {
        return None;
    }
    let i = text.find("office:mimetype=")? + "office:mimetype=".len();
    let rest = &text[i + 1..];
    let end = rest.find(['"', '\''])?;
    Kind::from_mime(&rest[..end])
}

/// Where a thumbnail's PNG bytes are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Thumbnail {
    /// Stored as is in the file.
    Range(Range<usize>),
    /// Inflated.
    Owned(Vec<u8>),
}

/// The package's `Thumbnails/thumbnail.png`.
pub fn thumbnail(data: &[u8]) -> Result<Thumbnail, Error> {
    if !data.starts_with(b"PK") {
        return if probe_file(data).is_some() {
            Err(Error::Unsupported(
                "flat OpenDocument XML file: it has no thumbnail".into(),
            ))
        } else {
            Err(Error::NotOdf)
        };
    }
    let zip = Zip::parse(data).map_err(zip_err)?;
    let e = zip.find("Thumbnails/thumbnail.png").ok_or_else(|| {
        Error::Unsupported(
            "OpenDocument file without a thumbnail (saved with thumbnails off)".into(),
        )
    })?;
    match zip.stored_range(data, e).map_err(zip_err)? {
        Some(r) => Ok(Thumbnail::Range(r)),
        None => Ok(Thumbnail::Owned(
            zip.read(data, e, 64 << 20).map_err(zip_err)?.into_owned(),
        )),
    }
}

/// What a document says about itself, for the info panel.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Facts {
    pub kind: Option<Kind>,
    /// `meta:generator`, e.g. "LibreOffice/7.6.4.1$Linux_X86_64 …".
    pub generator: Option<String>,
    pub title: Option<String>,
    pub subject: Option<String>,
    pub keywords: Option<String>,
    /// `dc:creator`, else `meta:initial-creator`.
    pub author: Option<String>,
    /// `draw:name` of each page ("page1"…, "Slide 1"…).
    pub page_names: Vec<String>,
    /// Shapes walked.
    pub shapes: u64,
    /// Shapes or parts not drawn, by kind, with counts.
    pub not_drawn: Vec<(String, u64)>,
    /// Images drawn.
    pub images: u32,
    /// Flat XML (`.fodg`/`.fodp`) rather than a package.
    pub flat: bool,
    /// A limit was hit (shapes, display list, image pixels, pages): some content is left
    /// out.
    pub limited: bool,
}

/// A drawing or presentation: its pages and facts.
pub struct Document {
    pub pages: Vec<Page>,
    pub facts: Facts,
}

impl Document {
    /// Short coding for the info panel: `OpenDocument drawing, page "Page 1"`.
    pub fn describe(&self, page: usize) -> String {
        let kind = self.facts.kind.as_ref().unwrap_or(&Kind::Graphics);
        let mut s = format!("OpenDocument {}", kind.noun());
        if self.facts.flat {
            s.push_str(" (flat XML)");
        }
        let unit = if *kind == Kind::Presentation {
            "slide"
        } else {
            "page"
        };
        match self.facts.page_names.get(page).filter(|n| !n.is_empty()) {
            Some(n) => s.push_str(&format!(", {unit} \"{n}\"")),
            None => s.push_str(&format!(", {unit} {}", page + 1)),
        }
        let skipped: u64 = self.facts.not_drawn.iter().map(|(_, n)| n).sum();
        if skipped > 0 {
            let noun = if skipped == 1 { "object" } else { "objects" };
            s.push_str(&format!(", {skipped} {noun} not drawn"));
        }
        if self.facts.limited {
            s.push_str(", partly drawn (limits reached)");
        }
        s
    }
}

enum Package<'d> {
    Zip(Zip, &'d [u8]),
    Flat,
}

impl Package<'_> {
    fn member(&self, name: &str) -> Option<Vec<u8>> {
        match self {
            Package::Zip(z, data) => {
                let e = z.find(name)?;
                Some(z.read(data, e, MAX_PART).ok()?.into_owned())
            }
            Package::Flat => None,
        }
    }

    fn part(&self, name: &str) -> Result<Option<String>, Error> {
        match self {
            Package::Zip(z, data) => {
                let Some(e) = z.find(name) else {
                    return Ok(None);
                };
                let b = z
                    .read(data, e, specular_drawpage::xml::MAX_XML_PART as u64)
                    .map_err(zip_err)?;
                specular_drawpage::xml::text_of(&b)
                    .map(Some)
                    .ok_or_else(|| Error::Malformed(format!("{name} is not UTF-8 text")))
            }
            Package::Flat => Ok(None),
        }
    }
}

fn xml<'t>(text: &'t str, part: &str) -> Result<Xml<'t>, Error> {
    specular_drawpage::xml::parse(text).map_err(|e| Error::Malformed(format!("{part}: {e}")))
}

fn meta_into(root: Node, f: &mut Facts) {
    let Some(meta) = root.descendants().find(|n| {
        n.tag_name().name() == "meta"
            && n.tag_name()
                .namespace()
                .is_some_and(|ns| ns.contains(":office:"))
    }) else {
        return;
    };
    let text = |name: &str| {
        meta.children()
            .find(|c| c.tag_name().name() == name)
            .map(specular_drawpage::xml::all_text)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    f.generator = text("generator");
    f.title = text("title");
    f.subject = text("subject").or_else(|| text("description"));
    let kw: Vec<String> = meta
        .children()
        .filter(|c| c.tag_name().name() == "keyword")
        .map(specular_drawpage::xml::all_text)
        .filter(|s| !s.trim().is_empty())
        .collect();
    f.keywords = (!kw.is_empty()).then(|| kw.join(", "));
    f.author = text("creator").or_else(|| text("initial-creator"));
}

/// Opens a drawing (or presentation) and lays out its pages. `decode` decodes embedded
/// images; without it images are not drawn.
pub fn open_graphics(data: &[u8], decode: Option<ImageDecoder>) -> Result<Document, Error> {
    let kind = probe_file(data).ok_or(Error::NotOdf)?;
    match kind {
        Kind::Graphics | Kind::Presentation => {}
        Kind::Text => {
            return Err(Error::Unsupported(
                "OpenDocument text documents are shown by their thumbnail only".into(),
            ));
        }
        Kind::Spreadsheet => {
            return Err(Error::Unsupported(
                "OpenDocument spreadsheets are shown by their thumbnail only".into(),
            ));
        }
        Kind::Other(ref k) => {
            return Err(Error::Unsupported(format!(
                "OpenDocument {k} files are not drawn, only drawings and presentations"
            )));
        }
    }
    let flat = !data.starts_with(b"PK");
    let pkg = if flat {
        Package::Flat
    } else {
        let zip = Zip::parse(data).map_err(zip_err)?;
        if let Some(e) = zip.find("META-INF/manifest.xml") {
            let m = zip.read(data, e, MAX_PART).map_err(zip_err)?;
            if memchr(&m, b"encryption-data") {
                return Err(Error::Encrypted);
            }
        }
        Package::Zip(zip, data)
    };
    let flat_text;
    let (styles_text, content_text, meta_text) = if flat {
        flat_text = specular_drawpage::xml::text_of(data)
            .ok_or_else(|| Error::Malformed("not UTF-8 text".into()))?;
        (None, Some(flat_text), None)
    } else {
        (
            pkg.part("styles.xml")?,
            pkg.part("content.xml")?,
            pkg.part("meta.xml")?,
        )
    };
    let content_text =
        content_text.ok_or_else(|| Error::Malformed("content.xml is missing".into()))?;
    let content = xml(&content_text, "content.xml")?;
    let styles_doc = match &styles_text {
        Some(t) => Some(xml(t, "styles.xml")?),
        None => None,
    };
    let mut facts = Facts {
        kind: Some(kind.clone()),
        flat,
        ..Facts::default()
    };
    if let Some(t) = &meta_text
        && let Ok(m) = xml(t, "meta.xml")
    {
        meta_into(m.root_element(), &mut facts);
    }
    if flat {
        meta_into(content.root_element(), &mut facts);
    }
    let mut styles = style::Styles::default();
    if let Some(s) = &styles_doc {
        styles.add_part(s.root_element());
    }
    styles.add_part(content.root_element());

    // Master pages (their shapes) and hidden layers.
    let master_root = styles_doc
        .as_ref()
        .map(|d| d.root_element())
        .unwrap_or(content.root_element());
    let master_styles = master_root
        .children()
        .find(|c| c.tag_name().name() == "master-styles");
    let mut hidden = std::collections::HashSet::new();
    if let Some(ms) = master_styles {
        for l in ms
            .children()
            .filter(|c| c.tag_name().name() == "layer-set")
            .flat_map(|s| s.children())
            .filter(|l| l.tag_name().name() == "layer")
        {
            if specular_drawpage::xml::attr(l, "display") == Some("none")
                && let Some(n) = specular_drawpage::xml::attr(l, "name")
            {
                hidden.insert(n.to_string());
            }
        }
    }
    let master_node = |name: &str| {
        master_styles.and_then(|ms| {
            ms.children().find(|m| {
                m.tag_name().name() == "master-page"
                    && specular_drawpage::xml::attr(*m, "name") == Some(name)
            })
        })
    };

    let body = content
        .root_element()
        .children()
        .find(|c| c.tag_name().name() == "body")
        .ok_or_else(|| Error::Malformed("content.xml has no body".into()))?;
    let holder = body
        .children()
        .find(|c| matches!(c.tag_name().name(), "drawing" | "presentation"))
        .ok_or_else(|| Error::Malformed("no drawing or presentation in the body".into()))?;
    let member = |name: &str| pkg.member(name);
    // Presentation footer, header and date declarations, by name.
    let mut decls: std::collections::HashMap<(String, String), String> = Default::default();
    for d in holder
        .children()
        .filter(|c| c.tag_name().name().ends_with("-decl"))
    {
        let kind = d.tag_name().name().trim_end_matches("-decl").to_string();
        let Some(n) = specular_drawpage::xml::attr(d, "name") else {
            continue;
        };
        let text = if specular_drawpage::xml::attr(d, "source") == Some("current-date") {
            today()
        } else {
            specular_drawpage::xml::all_text(d)
        };
        decls.insert((kind, n.to_string()), text);
    }
    let page_count = holder
        .children()
        .filter(|c| c.tag_name().name() == "page")
        .count();
    let mut pages = Vec::new();
    let mut not_drawn = std::collections::BTreeMap::new();
    for pg in holder.children().filter(|c| c.tag_name().name() == "page") {
        if pages.len() >= MAX_PAGES {
            facts.limited = true;
            break;
        }
        let master = specular_drawpage::xml::attr(pg, "master-page-name");
        let layout = master
            .and_then(|m| styles.masters.get(m))
            .and_then(|m| m.0.as_deref())
            .and_then(|l| styles.page_layouts.get(l));
        let size = layout
            .and_then(|l| {
                let w = l.layout.get("page-width").and_then(|v| units::length(v))?;
                let h = l.layout.get("page-height").and_then(|v| units::length(v))?;
                (w > 0.0 && h > 0.0 && w < 1e6 && h < 1e6).then_some((w, h))
            })
            .unwrap_or(A4);
        let name = specular_drawpage::xml::attr(pg, "name")
            .unwrap_or("")
            .to_string();
        let mut ctx = draw::Ctx::new(&styles, &member, decode.as_ref());
        ctx.hidden_layers = hidden.clone();
        let bg = styles.page_props(master, specular_drawpage::xml::attr(pg, "style-name"));
        // "border": the background fills the page inside its margins.
        let mut bg_rect = specular_drawpage::Rect::new(0.0, 0.0, size.0, size.1);
        if bg.get("background-size").is_some_and(|v| v == "border")
            && let Some(l) = layout
        {
            let m = |k: &str| {
                l.layout
                    .get(k)
                    .and_then(|v| units::length(v))
                    .unwrap_or(0.0)
                    .max(0.0)
            };
            let r = specular_drawpage::Rect::new(
                m("margin-left"),
                m("margin-top"),
                size.0 - m("margin-right"),
                size.1 - m("margin-bottom"),
            );
            if r.width() > 0.0 && r.height() > 0.0 {
                bg_rect = r;
            }
        }
        draw::background(&mut ctx, &bg, bg_rect);
        let decl = |kind: &str, attr: &str| {
            specular_drawpage::xml::attr(pg, attr)
                .and_then(|n| decls.get(&(kind.to_string(), n.to_string())))
                .cloned()
        };
        ctx.fields = draw::Fields {
            page_number: pages.len() + 1,
            page_count,
            footer: decl("footer", "use-footer-name"),
            header: decl("header", "use-header-name"),
            date_time: decl("date-time", "use-date-time-name"),
            show: ["footer", "date-time", "page-number", "header"]
                .iter()
                .filter(|k| bg.get(&format!("display-{k}")).is_some_and(|v| v == "true"))
                .map(|k| k.to_string())
                .collect(),
        };
        if let Some(m) = master.and_then(master_node) {
            ctx.walk(m, Affine::IDENTITY, 0, true);
        }
        ctx.walk(pg, Affine::IDENTITY, 0, false);
        let mut page = Page::new(name.clone(), size);
        page.ops = std::mem::take(&mut ctx.ops);
        facts.shapes += ctx.shapes;
        facts.images += ctx.images_drawn;
        facts.limited |= ctx.limited;
        for (k, v) in ctx.not_drawn {
            *not_drawn.entry(k).or_insert(0u64) += v;
        }
        facts.page_names.push(name);
        pages.push(page);
    }
    facts.not_drawn = not_drawn.into_iter().collect();
    if pages.is_empty() {
        return Err(Error::Malformed("the document has no pages".into()));
    }
    Ok(Document { pages, facts })
}

/// Today's date (UTC) as `YYYY-MM-DD`, for date fields that show the current date.
fn today() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = (secs / 86_400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

fn memchr(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

#[cfg(test)]
mod tests;
