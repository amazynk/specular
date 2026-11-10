//! The sidecar of a PDF: XFDF (ISO 19444-1), each object as the annotation a PDF tool would
//! make of it, in the page's user space (points, y up), so Acrobat and other tools can import
//! it onto the PDF.
//!
//! | Markup  | Element    | Notes |
//! |---------|------------|-------|
//! | cloud   | `polygon`  | `style="cloudy" intensity="1"`, `vertices` |
//! | text    | `freetext` | `defaultappearance` `/Helv <size> Tf …` |
//! | arrow   | `line`     | `tail="OpenArrow"` at `end`; the label in `contents` |
//! | stamp   | `stamp`    | `icon`: Approved, NotApproved, Reviewed or Custom |
//! | length  | `line`     | `intent="LineDimension"`, the value in `contents` |
//! | area    | `polygon`  | `intent="PolygonDimension"`, the value in `contents` |
//! | pin     | `text`     | a note; the photo's path in `spec:photo` |
//!
//! Every element also has `spec:kind`, `spec:size` and, for text, stamps and pins, `spec:at`
//! (the anchor in user space) in the `urn:specular:markup:1` namespace, which other tools
//! ignore. `name` is the object's id, `title` its author, `date` and `creationdate` when it
//! was made.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::PathBuf;

use specular_pdf::annotate::num;

use crate::{
    Affine, Markup, P, Shape, Stamp, apply, hex_colour, invert, iso_of_pdf_date, parse_colour,
    pdf_date,
};

const XFDF_NS: &str = "http://ns.adobe.com/xfdf/";
const SPEC_NS: &str = "urn:specular:markup:1";

/// Why a sidecar could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadError {
    Xml(String),
    NotXfdf,
}

impl std::fmt::Display for ReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReadError::Xml(e) => write!(f, "not valid XML: {e}"),
            ReadError::NotXfdf => f.write_str("not an XFDF file"),
        }
    }
}

fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\n' => o.push_str("&#10;"),
            '\r' => {}
            c if (c as u32) < 0x20 && c != '\t' => {}
            c => o.push(c),
        }
    }
    o
}

fn pt(p: P) -> String {
    format!("{},{}", num(p.x), num(p.y))
}

/// The XFDF of `pages` for the PDF named `file`; `user_to_page(n)` is page `n`'s map from user
/// space to the page as displayed (pages without one are skipped).
pub fn write(
    file: &str,
    pages: &[(u32, &[Markup])],
    user_to_page: &dyn Fn(u32) -> Option<Affine>,
    stamp_words: &dyn Fn(&Markup) -> String,
) -> String {
    let mut s = String::new();
    s.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    let _ = writeln!(
        s,
        "<xfdf xmlns=\"{XFDF_NS}\" xmlns:spec=\"{SPEC_NS}\" xml:space=\"preserve\">"
    );
    let _ = writeln!(s, "<f href=\"{}\"/>", esc(file));
    s.push_str("<annots>\n");
    for &(page, items) in pages {
        let Some(m) = user_to_page(page).and_then(|m| invert(&m)) else {
            continue;
        };
        let u = |p: P| apply(&m, p);
        for it in items {
            let b = it.bounds(&stamp_words(it));
            let corners = [
                u(P::new(b[0], b[1])),
                u(P::new(b[2], b[1])),
                u(P::new(b[0], b[3])),
                u(P::new(b[2], b[3])),
            ];
            let (x0, y0, x1, y1) = corners.iter().fold(
                (
                    f64::INFINITY,
                    f64::INFINITY,
                    f64::NEG_INFINITY,
                    f64::NEG_INFINITY,
                ),
                |a, p| (a.0.min(p.x), a.1.min(p.y), a.2.max(p.x), a.3.max(p.y)),
            );
            let scale = (m[0] * m[3] - m[1] * m[2]).abs().sqrt();
            let common = format!(
                "page=\"{page}\" name=\"{}\" title=\"{}\" date=\"{d}\" creationdate=\"{d}\" \
                 color=\"{}\" width=\"{}\" rect=\"{},{},{},{}\" flags=\"print\" spec:kind=\"{}\" \
                 spec:size=\"{}\"",
                esc(&it.id),
                esc(&it.author),
                hex_colour(it.colour),
                num(it.width * scale),
                num(x0),
                num(y0),
                num(x1),
                num(y1),
                it.shape.kind(),
                num(it.size * scale),
                d = pdf_date(&it.date),
            );
            let contents = format!("<contents>{}</contents>", esc(&it.text));
            let vertices = |pts: &[P]| {
                let v: Vec<String> = pts.iter().map(|p| pt(u(*p))).collect();
                format!("<vertices>{}</vertices>", v.join(";"))
            };
            let _ = match &it.shape {
                Shape::Cloud(pts) => writeln!(
                    s,
                    "<polygon {common} subject=\"Cloud\" style=\"cloudy\" intensity=\"1\">{contents}{}</polygon>",
                    vertices(pts)
                ),
                Shape::Area(pts) => writeln!(
                    s,
                    "<polygon {common} subject=\"Area Measurement\" intent=\"PolygonDimension\">{contents}{}</polygon>",
                    vertices(pts)
                ),
                Shape::Text(at) => {
                    let [r, g, bl] = it.colour.map(|c| f64::from(c) / 255.0);
                    writeln!(
                        s,
                        "<freetext {common} subject=\"Text\" spec:at=\"{}\">{contents}\
                         <defaultappearance>/Helv {} Tf {} {} {} rg</defaultappearance></freetext>",
                        pt(u(*at)),
                        num(it.size * scale),
                        num(r),
                        num(g),
                        num(bl)
                    )
                }
                Shape::Arrow { tail, head } => writeln!(
                    s,
                    "<line {common} subject=\"Arrow\" start=\"{}\" end=\"{}\" head=\"None\" tail=\"OpenArrow\">{contents}</line>",
                    pt(u(*tail)),
                    pt(u(*head))
                ),
                Shape::Length(a, bb) => writeln!(
                    s,
                    "<line {common} subject=\"Length Measurement\" intent=\"LineDimension\" start=\"{}\" end=\"{}\" head=\"Butt\" tail=\"Butt\">{contents}</line>",
                    pt(u(*a)),
                    pt(u(*bb))
                ),
                Shape::Stamp { at, kind } => writeln!(
                    s,
                    "<stamp {common} subject=\"Stamp\" icon=\"{}\" spec:stamp=\"{}\" spec:at=\"{}\">{contents}</stamp>",
                    kind.pdf_name(),
                    kind.id(),
                    pt(u(*at))
                ),
                Shape::Pin { at, photo } => writeln!(
                    s,
                    "<text {common} subject=\"Photo\" icon=\"Comment\" spec:at=\"{}\" spec:photo=\"{}\">{contents}</text>",
                    pt(u(*at)),
                    esc(&photo.to_string_lossy())
                ),
            };
        }
    }
    s.push_str("</annots>\n</xfdf>\n");
    s
}

fn parse_pt(s: &str) -> Option<P> {
    let (x, y) = s.split_once(',')?;
    let p = P::new(x.trim().parse().ok()?, y.trim().parse().ok()?);
    (p.x.is_finite() && p.y.is_finite()).then_some(p)
}

/// The markup of an XFDF file, by page; `user_to_page(n)` as for [`write`] (annotations on
/// pages without one are dropped). Annotations Specular does not make are skipped.
pub fn read(
    text: &str,
    user_to_page: &dyn Fn(u32) -> Option<Affine>,
) -> Result<BTreeMap<u32, Vec<Markup>>, ReadError> {
    let doc = roxmltree::Document::parse(text).map_err(|e| ReadError::Xml(e.to_string()))?;
    let root = doc.root_element();
    if root.tag_name().name() != "xfdf" {
        return Err(ReadError::NotXfdf);
    }
    let mut out: BTreeMap<u32, Vec<Markup>> = BTreeMap::new();
    let Some(annots) = root
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "annots")
    else {
        return Ok(out);
    };
    for e in annots.children().filter(roxmltree::Node::is_element) {
        let Some(page) = e.attribute("page").and_then(|p| p.parse::<u32>().ok()) else {
            continue;
        };
        let Some(m) = user_to_page(page) else {
            continue;
        };
        let scale = (m[0] * m[3] - m[1] * m[2]).abs().sqrt().max(1e-9);
        let d = |p: P| apply(&m, p);
        let spec = |k: &str| e.attribute((SPEC_NS, k));
        let child = |k: &str| {
            e.children()
                .find(|n| n.is_element() && n.tag_name().name() == k)
                .map(|n| n.text().unwrap_or("").to_owned())
        };
        let vertices = || -> Option<Vec<P>> {
            let v: Option<Vec<P>> = child("vertices")?
                .split(';')
                .filter(|s| !s.trim().is_empty())
                .map(|s| parse_pt(s).map(d))
                .collect();
            v.filter(|v| v.len() >= 2)
        };
        let point = |k: &str| e.attribute(k).and_then(parse_pt).map(d);
        let at = || spec("at").and_then(parse_pt).map(d);
        let element = e.tag_name().name();
        let kind = spec("kind").unwrap_or(match element {
            "polygon" if e.attribute("intent") == Some("PolygonDimension") => "area",
            "polygon" if e.attribute("style") == Some("cloudy") => "cloud",
            "line" if e.attribute("intent") == Some("LineDimension") => "length",
            "line" => "arrow",
            "freetext" => "text",
            "stamp" => "stamp",
            _ => "",
        });
        let shape = match kind {
            "cloud" => vertices().map(Shape::Cloud),
            "area" => vertices().map(Shape::Area),
            "text" => at().map(Shape::Text),
            "arrow" => point("start")
                .zip(point("end"))
                .map(|(tail, head)| Shape::Arrow { tail, head }),
            "length" => point("start")
                .zip(point("end"))
                .map(|(a, b)| Shape::Length(a, b)),
            "stamp" => at().map(|at| Shape::Stamp {
                at,
                kind: spec("stamp")
                    .and_then(Stamp::from_id)
                    .unwrap_or(Stamp::Custom),
            }),
            "pin" => at().zip(spec("photo")).map(|(at, p)| Shape::Pin {
                at,
                photo: PathBuf::from(p),
            }),
            _ => None,
        };
        let Some(shape) = shape else { continue };
        let f = |k: &str| {
            e.attribute(k)
                .and_then(|v| v.parse::<f64>().ok())
                .filter(|v| v.is_finite() && *v >= 0.0)
        };
        out.entry(page).or_default().push(Markup {
            id: e
                .attribute("name")
                .map_or_else(crate::new_id, str::to_owned),
            shape,
            colour: e
                .attribute("color")
                .and_then(parse_colour)
                .unwrap_or([229, 57, 53]),
            width: f("width").unwrap_or(1.0) / scale,
            size: spec("size")
                .and_then(|v| v.parse::<f64>().ok())
                .filter(|v| v.is_finite() && *v > 0.0)
                .unwrap_or(12.0)
                / scale,
            text: child("contents").unwrap_or_default().replace('\r', ""),
            author: e.attribute("title").unwrap_or("").to_owned(),
            date: e
                .attribute("creationdate")
                .or(e.attribute("date"))
                .map(iso_of_pdf_date)
                .unwrap_or_default(),
        });
    }
    Ok(out)
}
