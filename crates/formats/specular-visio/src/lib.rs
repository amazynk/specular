//! Microsoft Visio drawings.
//!
//! **VSDX** (and the template, stencil and macro-enabled variants: VSTX, VSSX, VSDM, VSTM,
//! VSSM) is an OPC package of ShapeSheet XML. [`open_vsdx`] reads it once into display lists
//! ([`specular_drawpage::Page`]), one per foreground page (a stencil, which has no pages, gets
//! one per master). Shapes resolve each cell through the instance, its master shape and the
//! style sheets (line, fill, text), and "Themed" cells through the quick-style cells and the
//! drawing's theme (colours, variant colours and styles, fill/line/font style matrices,
//! connector scheme, theme fonts).
//!
//! Drawn: group nesting with pins, local pins, angles and flips; geometry sections (MoveTo,
//! LineTo, ArcTo, EllipticalArcTo, Ellipse, the relative rows, PolylineTo, NURBSTo, splines as
//! polylines; NoFill, NoLine, NoShow); fills (solid, patterns as the mix of their colours,
//! the legacy and Visio 2013 gradients, transparency); lines (weight, colour, transparency,
//! dash patterns, caps) and arrowheads; images (foreign shapes, through the caller's decoder,
//! cropped to the shape); text (character, paragraph and text-block sections: fonts, sizes,
//! colours, bold/italic/underline/strike, alignment, indents, spacing, bullets, margins,
//! vertical alignment, text background, the text transform); background pages under their
//! foreground pages; hidden layers left out.
//!
//! Not drawn: shadows, effects (glow, soft edges, 3-D), rounding of corners, fill patterns
//! as patterns, metafile images (counted in [`Facts::images_not_drawn`]), infinite lines
//! (counted), formulas (the stored values are used, as Visio writes them).
//!
//! **VSD** (binary, Visio 5 to 2016) is shown by the preview it stores for file browsers:
//! [`vsd_preview`] returns that WMF, EMF or DIB for the caller to draw.
//!
//! Bounds: [`sheet::MAX_SHAPES`] shapes per part, nesting to [`sheet::MAX_DEPTH`],
//! [`specular_drawpage::MAX_OPS`] operations per document and
//! [`specular_drawpage::MAX_IMAGE_PIXELS`] image pixels; hostile files draw less, never
//! panic or loop (master and background-page cycles are cut).

mod draw;
mod opc;
mod sheet;
mod theme;
mod vsd;

use std::collections::{HashMap, HashSet};

use specular_drawpage::xml::{self, roxmltree::Node};
use specular_drawpage::{Affine, ImageDecoder, Page};

use draw::{Ctx, Master, PageEnv};
use sheet::Sheet;

/// Why a Visio file cannot be shown.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("not a Visio drawing")]
    NotVisio,
    #[error("{0}")]
    Unsupported(String),
    #[error("damaged Visio drawing: {0}")]
    Malformed(String),
}

/// What a VSDX package says about itself, for the info panel.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Facts {
    /// "drawing", "template" or "stencil".
    pub kind: &'static str,
    /// "VSDX", "VSTX", "VSSX", "VSDM", "VSTM" or "VSSM".
    pub extension: &'static str,
    /// Names of the pages shown, in order.
    pub page_names: Vec<String>,
    /// Background pages (drawn under the pages that use them, not shown on their own).
    pub background_pages: usize,
    pub masters: usize,
    /// Shapes drawn (master members included).
    pub shapes: u64,
    /// Images that could not be decoded (metafiles, unknown formats).
    pub images_not_drawn: u64,
    /// Geometry rows not drawn (infinite lines).
    pub geometry_not_drawn: u64,
    /// A document limit was hit: some content is left out.
    pub limited: bool,
    /// The stencil's masters are shown as pages.
    pub masters_as_pages: bool,
    pub application: Option<String>,
    pub app_version: Option<String>,
    pub title: Option<String>,
    pub author: Option<String>,
    pub subject: Option<String>,
    pub keywords: Option<String>,
    pub description: Option<String>,
}

/// A VSDX package read into pages.
#[derive(Debug, Clone)]
pub struct Document {
    pub pages: Vec<Page>,
    pub facts: Facts,
}

impl Document {
    /// A short coding for the info panel: `Visio drawing (VSDX), page "Page-1"`.
    pub fn describe(&self, page: usize) -> String {
        let f = &self.facts;
        let mut s = format!("Visio {} ({})", f.kind, f.extension);
        if let Some(name) = f.page_names.get(page).filter(|n| !n.is_empty()) {
            if f.masters_as_pages {
                s.push_str(&format!(", master \"{name}\""));
            } else {
                s.push_str(&format!(", page \"{name}\""));
            }
        }
        if f.images_not_drawn > 0 {
            let n = f.images_not_drawn;
            s.push_str(&format!(
                ", {n} image{} not drawn",
                if n == 1 { "" } else { "s" }
            ));
        }
        if f.limited {
            s.push_str(", too large to draw in full");
        }
        s
    }
}

/// The preview a binary VSD stores.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preview {
    pub kind: PreviewKind,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewKind {
    /// A standalone WMF with a placeable header.
    Wmf,
    Emf,
    /// A packed DIB (BITMAPINFOHEADER, palette, pixels), without a BMP file header.
    Dib,
    Png,
    Jpeg,
}

fn find(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

/// A ZIP whose directory names `visio/document.xml`.
pub fn probe_vsdx(data: &[u8]) -> bool {
    if !data.starts_with(b"PK\x03\x04") {
        return false;
    }
    let tail = &data[data.len().saturating_sub(1 << 20)..];
    find(tail, b"visio/document.xml")
}

/// A compound file (a binary VSD has no other signature; [`probe_vsd_file`] decides).
pub fn probe_vsd(head: &[u8]) -> bool {
    specular_cfb::sniff(head)
}

/// A compound file with a `VisioDocument` stream.
pub fn probe_vsd_file(data: &[u8]) -> bool {
    specular_cfb::Cfb::open(data).is_ok_and(|c| c.exists("VisioDocument"))
}

/// The preview of a binary Visio file.
pub fn vsd_preview(data: &[u8]) -> Result<Preview, Error> {
    vsd::preview(data)
}

fn attr_u32(n: Node, name: &str) -> Option<u32> {
    n.attribute(name).and_then(|v| v.trim().parse().ok())
}

fn element<'a, 'i>(doc: &'a xml::roxmltree::Document<'i>, name: &str) -> Option<Node<'a, 'i>> {
    doc.descendants()
        .find(|n| n.is_element() && n.tag_name().name() == name)
}

fn rel_of(n: Node) -> Option<String> {
    n.children()
        .find(|c| c.is_element() && c.tag_name().name() == "Rel")
        .and_then(|r| r.attributes().find(|a| a.name() == "id"))
        .map(|a| a.value().to_string())
}

fn simple_rels(rels: &opc::Rels) -> HashMap<String, String> {
    rels.iter()
        .map(|(k, (_, t))| (k.clone(), t.clone()))
        .collect()
}

/// A part's shapes (`PageContents` / `MasterContents`).
fn read_shapes(pkg: &opc::Package, part: &str) -> Result<Vec<Sheet>, Error> {
    let Some(t) = pkg.text(part) else {
        return Ok(Vec::new());
    };
    let doc = xml::parse(&t).map_err(|e| Error::Malformed(format!("{part}: {e}")))?;
    let mut count = 0;
    Ok(sheet::parse_shapes(doc.root_element(), &mut count))
}

struct PageInfo {
    id: u32,
    name: String,
    background: bool,
    back_page: Option<u32>,
    sheet: Sheet,
    shapes: Vec<Sheet>,
    rels: HashMap<String, String>,
}

/// Reads a VSDX/VSTX/VSSX package into pages.
pub fn open_vsdx(data: &[u8], decode: Option<ImageDecoder>) -> Result<Document, Error> {
    if specular_cfb::sniff(data) {
        let cfb = specular_cfb::Cfb::open(data).map_err(|_| Error::NotVisio)?;
        if cfb.exists("EncryptedPackage") {
            return Err(Error::Unsupported(
                "password-protected Visio drawing: opening it needs the password, which is not \
                 supported"
                    .into(),
            ));
        }
        return Err(Error::NotVisio);
    }
    if data.starts_with(b"<?xml") || data.starts_with(b"\xef\xbb\xbf<?xml") {
        let head = String::from_utf8_lossy(&data[..data.len().min(4096)]);
        if head.contains("VisioDocument") {
            return Err(Error::Unsupported(
                "Visio XML drawing (VDX, Visio 2003–2010): not supported; save it as VSDX".into(),
            ));
        }
        return Err(Error::NotVisio);
    }
    let pkg = opc::Package::open(data)?;
    // The document part: the package's office document relationship, else the usual name.
    let doc_part = pkg
        .rels("")
        .values()
        .find(|(ty, _)| opc::rel_kind(ty) == "document")
        .map(|(_, t)| t.clone())
        .filter(|t| pkg.has(t))
        .unwrap_or_else(|| "visio/document.xml".into());
    if !pkg.has(&doc_part) {
        return Err(Error::NotVisio);
    }
    let mut facts = Facts {
        kind: "drawing",
        extension: "VSDX",
        ..Facts::default()
    };
    if let Some(ct) = pkg.text("[Content_Types].xml") {
        let (kind, ext) = if ct.contains("visio.stencil.macroEnabled") {
            ("stencil", "VSSM")
        } else if ct.contains("visio.stencil") {
            ("stencil", "VSSX")
        } else if ct.contains("visio.template.macroEnabled") {
            ("template", "VSTM")
        } else if ct.contains("visio.template") {
            ("template", "VSTX")
        } else if ct.contains("visio.drawing.macroEnabled") {
            ("drawing", "VSDM")
        } else {
            ("drawing", "VSDX")
        };
        facts.kind = kind;
        facts.extension = ext;
    }
    read_props(&pkg, &mut facts);

    // document.xml: fonts, colours, style sheets.
    let doc_text = pkg
        .text(&doc_part)
        .ok_or_else(|| Error::Malformed("the document part cannot be read".into()))?;
    let doc = xml::parse(&doc_text).map_err(|e| Error::Malformed(format!("document: {e}")))?;
    let mut faces = HashMap::new();
    if let Some(f) = element(&doc, "FaceNames") {
        for (i, n) in f.children().filter(Node::is_element).enumerate() {
            let name = n
                .attribute("NameU")
                .or_else(|| n.attribute("Name"))
                .unwrap_or("")
                .to_string();
            faces.insert(attr_u32(n, "ID").unwrap_or(i as u32), name);
        }
    }
    let mut palette: Vec<[u8; 3]> = Vec::new();
    if let Some(c) = element(&doc, "Colors") {
        for n in c.children().filter(Node::is_element) {
            let ix = attr_u32(n, "IX").unwrap_or(palette.len() as u32) as usize;
            if let Some(rgb) = n.attribute("RGB").and_then(specular_drawpage::hex_color)
                && ix < 4096
            {
                if palette.len() <= ix {
                    palette.resize(ix + 1, [0, 0, 0]);
                }
                palette[ix] = [rgb[0], rgb[1], rgb[2]];
            }
        }
    }
    let mut styles = HashMap::new();
    if let Some(ss) = element(&doc, "StyleSheets") {
        let mut count = 0;
        for n in ss
            .children()
            .filter(|c| c.is_element() && c.tag_name().name() == "StyleSheet")
        {
            let s = sheet::parse_sheet(n, &mut count, 1);
            styles.insert(s.id, s);
        }
    }
    drop(doc);

    let doc_rels = pkg.rels(&doc_part);
    let rel_target = |kind: &str| {
        doc_rels
            .values()
            .find(|(ty, _)| opc::rel_kind(ty) == kind)
            .map(|(_, t)| t.clone())
    };
    let theme = rel_target("theme")
        .and_then(|p| pkg.text(&p))
        .and_then(|t| xml::parse(&t).ok().map(|d| theme::Theme::parse(&d)))
        .unwrap_or_default();

    // Masters.
    let mut masters: HashMap<u32, Master> = HashMap::new();
    let mut master_order: Vec<(u32, String, Sheet)> = Vec::new();
    if let Some(mp) = rel_target("masters")
        && let Some(t) = pkg.text(&mp)
    {
        let mdoc = xml::parse(&t).map_err(|e| Error::Malformed(format!("masters: {e}")))?;
        let mrels = pkg.rels(&mp);
        for m in mdoc
            .root_element()
            .children()
            .filter(|c| c.is_element() && c.tag_name().name() == "Master")
        {
            let Some(id) = attr_u32(m, "ID") else {
                continue;
            };
            let Some(part) = rel_of(m).and_then(|r| mrels.get(&r)).map(|r| r.1.clone()) else {
                continue;
            };
            let shapes = read_shapes(&pkg, &part)?;
            let rels = simple_rels(&pkg.rels(&part));
            let name = m
                .attribute("NameU")
                .or_else(|| m.attribute("Name"))
                .unwrap_or("")
                .to_string();
            let mut count = 0;
            let ps = m
                .children()
                .find(|c| c.is_element() && c.tag_name().name() == "PageSheet")
                .map(|p| sheet::parse_sheet(p, &mut count, 1))
                .unwrap_or_default();
            if m.attribute("Hidden") != Some("1") {
                master_order.push((id, name, ps));
            }
            masters.insert(id, Master { shapes, rels });
        }
    }
    facts.masters = masters.len();

    // Pages.
    let mut pages_info: Vec<PageInfo> = Vec::new();
    if let Some(pp) = rel_target("pages")
        && let Some(t) = pkg.text(&pp)
    {
        let pdoc = xml::parse(&t).map_err(|e| Error::Malformed(format!("pages: {e}")))?;
        let prels = pkg.rels(&pp);
        for p in pdoc
            .root_element()
            .children()
            .filter(|c| c.is_element() && c.tag_name().name() == "Page")
        {
            let part = rel_of(p).and_then(|r| prels.get(&r)).map(|r| r.1.clone());
            let mut count = 0;
            let ps = p
                .children()
                .find(|c| c.is_element() && c.tag_name().name() == "PageSheet")
                .map(|x| sheet::parse_sheet(x, &mut count, 1))
                .unwrap_or_default();
            let (shapes, rels) = match &part {
                Some(part) => (read_shapes(&pkg, part)?, simple_rels(&pkg.rels(part))),
                None => (Vec::new(), HashMap::new()),
            };
            pages_info.push(PageInfo {
                id: attr_u32(p, "ID").unwrap_or(pages_info.len() as u32),
                name: p
                    .attribute("Name")
                    .or_else(|| p.attribute("NameU"))
                    .unwrap_or("")
                    .to_string(),
                background: p.attribute("Background") == Some("1"),
                back_page: attr_u32(p, "BackPage"),
                sheet: ps,
                shapes,
                rels,
            });
        }
    }

    let media = |part: &str| pkg.media(part);
    let mut ctx = Ctx {
        styles: &styles,
        masters: &masters,
        theme: &theme,
        faces: &faces,
        palette: &palette,
        media: &media,
        decode: decode.as_ref(),
        images: HashMap::new(),
        image_pixels: 0,
        skipped: draw::Skipped::default(),
        shapes: 0,
        ops: 0,
    };
    let mut pages = Vec::new();
    let foreground: Vec<&PageInfo> = pages_info.iter().filter(|p| !p.background).collect();
    facts.background_pages = pages_info.len() - foreground.len();
    let shown: Vec<&PageInfo> = if foreground.is_empty() {
        pages_info.iter().collect()
    } else {
        foreground
    };
    for p in shown {
        let (w, h) = page_size(&p.sheet, &styles);
        let mut page = Page::new(p.name.clone(), (w * 72.0, h * 72.0));
        let to_page = Affine::new([72.0, 0.0, 0.0, -72.0, 0.0, h * 72.0]);
        // Background pages, deepest first.
        let mut backs = Vec::new();
        let mut seen = HashSet::from([p.id]);
        let mut cur = p.back_page;
        while let Some(b) = cur {
            if !seen.insert(b) || backs.len() > 16 {
                break;
            }
            let Some(bp) = pages_info.iter().find(|x| x.id == b) else {
                break;
            };
            backs.push(bp);
            cur = bp.back_page;
        }
        for bp in backs.iter().rev().chain(std::iter::once(&p)) {
            let env = env_of(&bp.sheet, &bp.rels);
            draw::draw_shapes(&mut ctx, &env, &bp.shapes, to_page, &mut page.ops);
        }
        facts.page_names.push(p.name.clone());
        pages.push(page);
    }
    // A stencil: its masters as pages.
    if pages.is_empty() {
        for (id, name, ps) in &master_order {
            let Some(m) = masters.get(id) else { continue };
            let (mut w, mut h) = page_size(ps, &styles);
            if !(w > 0.0 && h > 0.0) {
                (w, h) = (2.0, 2.0);
            }
            let mut page = Page::new(name.clone(), (w * 72.0, h * 72.0));
            let to_page = Affine::new([72.0, 0.0, 0.0, -72.0, 0.0, h * 72.0]);
            let env = env_of(ps, &m.rels);
            draw::draw_shapes(&mut ctx, &env, &m.shapes, to_page, &mut page.ops);
            facts.page_names.push(name.clone());
            pages.push(page);
            if pages.len() >= 1000 {
                break;
            }
        }
        facts.masters_as_pages = !pages.is_empty();
    }
    if pages.is_empty() {
        return Err(Error::Malformed("Visio drawing without pages".into()));
    }
    facts.shapes = ctx.shapes;
    facts.images_not_drawn = ctx.skipped.images;
    facts.geometry_not_drawn = ctx.skipped.geometry;
    facts.limited = ctx.skipped.limited;
    Ok(Document { pages, facts })
}

/// The page size in inches (PageWidth/PageHeight, through the page's styles); Letter when
/// unset; at most 200 inches a side.
fn page_size(ps: &Sheet, styles: &HashMap<u32, Sheet>) -> (f64, f64) {
    let get = |n: &str| {
        ps.num(n).or_else(|| {
            ps.fill_style
                .or(ps.line_style)
                .and_then(|i| styles.get(&i))
                .and_then(|s| s.num(n))
        })
    };
    let ok = |v: Option<f64>, d: f64| v.filter(|x| *x > 1e-6 && x.is_finite()).unwrap_or(d);
    (
        ok(get("PageWidth"), 8.5).min(200.0),
        ok(get("PageHeight"), 11.0).min(200.0),
    )
}

fn env_of<'a>(ps: &Sheet, rels: &'a HashMap<String, String>) -> PageEnv<'a> {
    let idx = |n: &str| {
        ps.num(n)
            .map(|v| v as i64)
            .filter(|v| (0..64).contains(v))
            .unwrap_or(0) as usize
    };
    let mut layers = HashMap::new();
    if let Some(sec) = ps.section("Layer", None) {
        for r in &sec.rows {
            if let Ok(i) = r.key.parse::<u32>() {
                layers.insert(i, r.num("Visible").unwrap_or(1.0) != 0.0);
            }
        }
    }
    PageEnv {
        variant: idx("VariationColorIndex"),
        var_style: idx("VariationStyleIndex"),
        layers,
        rels,
    }
}

fn read_props(pkg: &opc::Package, facts: &mut Facts) {
    let core = pkg
        .rels("")
        .values()
        .find(|(ty, _)| opc::rel_kind(ty) == "core-properties")
        .map(|(_, t)| t.clone())
        .unwrap_or_else(|| "docProps/core.xml".into());
    let text_of = |doc: &xml::roxmltree::Document, name: &str| {
        element(doc, name)
            .map(xml::all_text)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    if let Some(t) = pkg.text(&core)
        && let Ok(d) = xml::parse(&t)
    {
        facts.title = text_of(&d, "title");
        facts.author = text_of(&d, "creator");
        facts.subject = text_of(&d, "subject");
        facts.keywords = text_of(&d, "keywords");
        facts.description = text_of(&d, "description");
    }
    if let Some(t) = pkg.text("docProps/app.xml")
        && let Ok(d) = xml::parse(&t)
    {
        facts.application = text_of(&d, "Application");
        facts.app_version = text_of(&d, "AppVersion");
    }
}

#[cfg(test)]
mod tests;
