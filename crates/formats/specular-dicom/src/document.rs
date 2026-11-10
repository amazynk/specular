//! Encapsulated documents (PS3.3 A.45, C.24.2): a PDF, a CDA document, or an STL / OBJ / MTL
//! model carried whole in Encapsulated Document (0042,0011) instead of pixel data.
//!
//! [`Dicom::document`] finds the document and what the info panel shows about it (title and
//! MIME type; patient and study attributes are not read here). [`cda_body`] turns an HL7 CDA
//! document into lines of text: the body only (section titles, paragraphs, lists, tables) —
//! the header, with its patient, author and custodian, is skipped — or the PDF / plain text of
//! an unstructured (`nonXMLBody`) document.

use std::ops::Range;

use crate::Dicom;
use crate::charset::Charset;
use crate::summary::Reader;

/// What kind of document is encapsulated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Pdf,
    Cda,
    Stl,
    Obj,
    Mtl,
    /// An encapsulated document of another MIME type.
    Other,
}

impl Kind {
    /// The kind's name for the info panel ("PDF", "CDA", "STL", ...).
    pub fn name(self) -> &'static str {
        match self {
            Kind::Pdf => "PDF",
            Kind::Cda => "CDA",
            Kind::Stl => "STL",
            Kind::Obj => "OBJ",
            Kind::Mtl => "MTL",
            Kind::Other => "document",
        }
    }
}

/// An encapsulated document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Document {
    pub kind: Kind,
    /// The document's bytes in [`Dicom::buf`] (pad byte removed when the length is given).
    pub range: Range<usize>,
    /// Document Title (0042,0010), else the Concept Name's meaning.
    pub title: Option<String>,
    /// MIME Type of Encapsulated Document (0042,0012).
    pub mime: Option<String>,
}

const ENCAPSULATED_DOCUMENT: u32 = 0x0042_0011;
const DOCUMENT_TITLE: u32 = 0x0042_0010;
const MIME_TYPE: u32 = 0x0042_0012;
const DOCUMENT_LENGTH: u32 = 0x0042_0015;
const CONCEPT_NAME: u32 = 0x0040_A043;
const CODE_MEANING: u32 = 0x0008_0104;
const SOP_CLASS: u32 = 0x0008_0016;
const MEDIA_SOP_CLASS: u32 = 0x0002_0002;

const CLASSES: &[(&str, Kind)] = &[
    ("1.2.840.10008.5.1.4.1.1.104.1", Kind::Pdf),
    ("1.2.840.10008.5.1.4.1.1.104.2", Kind::Cda),
    ("1.2.840.10008.5.1.4.1.1.104.3", Kind::Stl),
    ("1.2.840.10008.5.1.4.1.1.104.4", Kind::Obj),
    ("1.2.840.10008.5.1.4.1.1.104.5", Kind::Mtl),
];

fn kind_of_mime(m: &str) -> Kind {
    let m = m.to_ascii_lowercase();
    let m = m.split(';').next().unwrap_or("").trim();
    match m {
        "application/pdf" => Kind::Pdf,
        "text/xml" | "application/xml" | "application/hl7-cda+xml" => Kind::Cda,
        "model/stl" | "model/x.stl-binary" | "model/x.stl-ascii" | "application/sla" => Kind::Stl,
        "model/obj" => Kind::Obj,
        "model/mtl" => Kind::Mtl,
        _ => Kind::Other,
    }
}

impl Dicom {
    /// The encapsulated document, when the data set has one.
    pub fn document(&self, file: &[u8]) -> Option<Document> {
        let e = self.dataset.get(ENCAPSULATED_DOCUMENT)?;
        let v = self.values(file);
        let r = Reader {
            ds: &self.dataset,
            v,
            cs: Charset::of(&self.dataset, &v),
        };
        let class = |ds: &crate::DataSet, tag| {
            ds.get(tag)
                .and_then(|e| std::str::from_utf8(v.text_bytes(e)).ok())
                .and_then(|u| CLASSES.iter().find(|c| c.0 == u))
                .map(|c| c.1)
        };
        let mime = r.text(MIME_TYPE);
        let kind = class(&self.dataset, SOP_CLASS)
            .or_else(|| class(&self.meta, MEDIA_SOP_CLASS))
            .unwrap_or_else(|| mime.as_deref().map_or(Kind::Other, kind_of_mime));
        let mut range = e.value.clone();
        // The value is padded to even length; Encapsulated Document Length gives the real one.
        if let Some(n) = self
            .dataset
            .get(DOCUMENT_LENGTH)
            .and_then(|l| v.int(l))
            .and_then(|n| usize::try_from(n).ok())
            && n <= range.len()
            && n + 1 >= range.len()
        {
            range.end = range.start + n;
        }
        let title = r.text(DOCUMENT_TITLE).or_else(|| {
            let item = self.dataset.item(CONCEPT_NAME)?;
            r.text_in(item, CODE_MEANING)
        });
        Some(Document {
            kind,
            range,
            title,
            mime,
        })
    }
}

/// The body of a CDA document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CdaBody {
    /// Lines of a structured body, with the document title first.
    Text(Vec<Line>),
    /// An unstructured body that is a PDF.
    Pdf(Vec<u8>),
}

/// One line of document text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    pub style: Style,
    pub text: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    Title,
    Heading,
    Body,
}

/// Why a CDA document cannot be shown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CdaError(pub String);

/// Largest decoded unstructured body.
const MAX_BODY: usize = 256 << 20;

/// Reads the body of the HL7 CDA document `xml`.
pub fn cda_body(xml: &[u8]) -> Result<CdaBody, CdaError> {
    let text = std::str::from_utf8(xml.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(xml))
        .map_err(|_| CdaError("CDA document is not UTF-8".into()))?;
    let text = text.trim_end_matches(['\0', ' ', '\n', '\r', '\t']);
    // roxmltree recurses once per element level while parsing.
    if !specular_limits::xml_within_depth(text.as_bytes(), specular_limits::MAX_XML_DEPTH) {
        return Err(CdaError("CDA document nested too deeply".into()));
    }
    let opts = roxmltree::ParsingOptions {
        allow_dtd: false,
        ..Default::default()
    };
    let doc = roxmltree::Document::parse_with_options(text, opts)
        .map_err(|e| CdaError(format!("CDA document: {e}")))?;
    let root = doc.root_element();
    if root.tag_name().name() != "ClinicalDocument" {
        return Err(CdaError("not an HL7 CDA document".into()));
    }
    let mut lines = Vec::new();
    if let Some(t) = child(root, "title") {
        push(&mut lines, Style::Title, &flat(t));
    }
    let body =
        child(root, "component").ok_or_else(|| CdaError("CDA document has no body".into()))?;
    if let Some(nx) = child(body, "nonXMLBody") {
        let t = child(nx, "text").ok_or_else(|| CdaError("CDA body has no text".into()))?;
        let media = t
            .attribute("mediaType")
            .unwrap_or("text/plain")
            .to_ascii_lowercase();
        let raw: String = t
            .descendants()
            .filter(|n| n.is_text())
            .filter_map(|n| n.text())
            .collect();
        let bytes = if t.attribute("representation") == Some("B64") {
            base64(&raw).ok_or_else(|| CdaError("CDA body: bad base64".into()))?
        } else {
            raw.into_bytes()
        };
        if media.starts_with("application/pdf") {
            return Ok(CdaBody::Pdf(bytes));
        }
        if media.starts_with("text/") {
            let s = String::from_utf8_lossy(&bytes);
            for l in s.lines() {
                push(&mut lines, Style::Body, l);
            }
            return Ok(CdaBody::Text(lines));
        }
        return Err(CdaError(format!("CDA body of type {media}")));
    }
    let sb =
        child(body, "structuredBody").ok_or_else(|| CdaError("CDA document has no body".into()))?;
    for c in children(sb, "component") {
        if let Some(s) = child(c, "section") {
            section(s, &mut lines, 0);
        }
    }
    Ok(CdaBody::Text(lines))
}

fn child<'a, 'i>(n: roxmltree::Node<'a, 'i>, name: &str) -> Option<roxmltree::Node<'a, 'i>> {
    n.children()
        .find(|c| c.is_element() && c.tag_name().name() == name)
}

fn children<'a, 'i: 'a>(
    n: roxmltree::Node<'a, 'i>,
    name: &'a str,
) -> impl Iterator<Item = roxmltree::Node<'a, 'i>> + 'a {
    n.children()
        .filter(move |c| c.is_element() && c.tag_name().name() == name)
}

/// Adds `text` (whitespace collapsed) as a line, unless it is empty.
fn push(lines: &mut Vec<Line>, style: Style, text: &str) {
    let t = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if !t.is_empty() {
        lines.push(Line { style, text: t });
    }
}

/// All the text under `n`, whitespace collapsed.
fn flat(n: roxmltree::Node) -> String {
    let mut s = String::new();
    for d in n.descendants() {
        if d.is_text() {
            s.push_str(d.text().unwrap_or(""));
        } else if d.is_element() && d.tag_name().name() == "br" {
            s.push(' ');
        }
    }
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

const MAX_DEPTH: usize = 32;

fn section(s: roxmltree::Node, lines: &mut Vec<Line>, depth: usize) {
    if depth > MAX_DEPTH {
        return;
    }
    if let Some(t) = child(s, "title") {
        if !lines.is_empty() {
            lines.push(Line {
                style: Style::Body,
                text: String::new(),
            });
        }
        push(lines, Style::Heading, &flat(t));
    }
    if let Some(t) = child(s, "text") {
        narrative(t, lines, 0);
    }
    for c in children(s, "component") {
        if let Some(sub) = child(c, "section") {
            section(sub, lines, depth + 1);
        }
    }
}

/// A narrative block (CDA R2 section text): paragraphs, lists, tables and inline content.
fn narrative(n: roxmltree::Node, lines: &mut Vec<Line>, depth: usize) {
    if depth > MAX_DEPTH {
        return;
    }
    let mut run = String::new();
    let flush = |run: &mut String, lines: &mut Vec<Line>| {
        push(lines, Style::Body, run);
        run.clear();
    };
    for c in n.children() {
        if c.is_text() {
            run.push_str(c.text().unwrap_or(""));
            continue;
        }
        if !c.is_element() {
            continue;
        }
        match c.tag_name().name() {
            "br" => flush(&mut run, lines),
            "paragraph" => {
                flush(&mut run, lines);
                if let Some(cap) = child(c, "caption") {
                    push(lines, Style::Heading, &flat(cap));
                }
                narrative(c, lines, depth + 1);
            }
            "caption" => {}
            "list" => {
                flush(&mut run, lines);
                if let Some(cap) = child(c, "caption") {
                    push(lines, Style::Heading, &flat(cap));
                }
                let ordered = c.attribute("listType") == Some("ordered");
                for (i, item) in children(c, "item").enumerate() {
                    let mark = if ordered {
                        format!("{}.", i + 1)
                    } else {
                        "\u{2022}".to_owned()
                    };
                    let mut sub = Vec::new();
                    narrative(item, &mut sub, depth + 1);
                    for (j, l) in sub.into_iter().enumerate() {
                        let text = if j == 0 {
                            format!("{mark} {}", l.text)
                        } else {
                            format!("   {}", l.text)
                        };
                        lines.push(Line {
                            style: l.style,
                            text,
                        });
                    }
                }
            }
            "table" => {
                flush(&mut run, lines);
                if let Some(cap) = child(c, "caption") {
                    push(lines, Style::Heading, &flat(cap));
                }
                for tr in c
                    .descendants()
                    .filter(|d| d.is_element() && d.tag_name().name() == "tr")
                {
                    let cells: Vec<String> = tr
                        .children()
                        .filter(|d| d.is_element() && matches!(d.tag_name().name(), "td" | "th"))
                        .map(flat)
                        .collect();
                    let head = tr
                        .children()
                        .any(|d| d.is_element() && d.tag_name().name() == "th");
                    push(
                        lines,
                        if head { Style::Heading } else { Style::Body },
                        &cells.join(" | "),
                    );
                }
            }
            // Pictures and links to other content cannot be shown as text.
            "renderMultiMedia" | "linkHtml" if c.children().all(|d| !d.is_text()) => {}
            _ => {
                // Inline content (content, sub, sup, footnote, linkHtml): its text joins the
                // run, unless it holds block content itself.
                let block = c.descendants().any(|d| {
                    d.is_element() && matches!(d.tag_name().name(), "paragraph" | "list" | "table")
                });
                if block {
                    flush(&mut run, lines);
                    narrative(c, lines, depth + 1);
                } else {
                    run.push_str(&flat(c));
                }
            }
        }
    }
    flush(&mut run, lines);
}

/// Decodes standard base64 (RFC 4648), ignoring whitespace; `None` on another character.
pub fn base64(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() / 4 * 3);
    let (mut acc, mut bits) = (0u32, 0u32);
    for &c in s.as_bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' => break,
            b' ' | b'\n' | b'\r' | b'\t' => continue,
            _ => return None,
        };
        acc = (acc << 6) | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
        if out.len() > MAX_BODY {
            return None;
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_decodes() {
        assert_eq!(base64("aGVsbG8=").as_deref(), Some(&b"hello"[..]));
        assert_eq!(
            base64("aGVs\nbG8gd29y bGQ").as_deref(),
            Some(&b"hello world"[..])
        );
        assert_eq!(base64("").as_deref(), Some(&b""[..]));
        assert!(base64("a*b").is_none());
    }

    const CDA: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ClinicalDocument xmlns="urn:hl7-org:v3">
  <title>Discharge Summary</title>
  <recordTarget><patientRole><patient><name><given>Secret</given><family>Patient</family></name></patient></patientRole></recordTarget>
  <component><structuredBody>
    <component><section>
      <title>Findings</title>
      <text><paragraph>Lungs are <content styleCode="Bold">clear</content>.</paragraph>
        <list><item>No effusion</item><item>Heart size normal</item></list>
        <table><thead><tr><th>Test</th><th>Value</th></tr></thead>
          <tbody><tr><td>HR</td><td>72</td></tr></tbody></table>
      </text>
      <component><section><title>Impression</title><text>Normal study.</text></section></component>
    </section></component>
  </structuredBody></component>
</ClinicalDocument>"#;

    #[test]
    fn cda_structured_body_as_lines_without_the_patient() {
        let CdaBody::Text(lines) = cda_body(CDA.as_bytes()).unwrap() else {
            panic!("not text");
        };
        let texts: Vec<&str> = lines.iter().map(|l| l.text.as_str()).collect();
        assert_eq!(
            texts,
            [
                "Discharge Summary",
                "",
                "Findings",
                "Lungs are clear.",
                "\u{2022} No effusion",
                "\u{2022} Heart size normal",
                "Test | Value",
                "HR | 72",
                "",
                "Impression",
                "Normal study.",
            ]
        );
        assert_eq!(lines[0].style, Style::Title);
        assert_eq!(lines[2].style, Style::Heading);
        assert!(!texts.iter().any(|t| t.contains("Secret")));
    }

    #[test]
    fn cda_unstructured_pdf_and_text() {
        let pdf = r#"<ClinicalDocument xmlns="urn:hl7-org:v3"><component><nonXMLBody>
            <text mediaType="application/pdf" representation="B64">JVBERi0xLjQK</text>
            </nonXMLBody></component></ClinicalDocument>"#;
        assert_eq!(
            cda_body(pdf.as_bytes()).unwrap(),
            CdaBody::Pdf(b"%PDF-1.4\n".to_vec())
        );
        let txt = r#"<ClinicalDocument xmlns="urn:hl7-org:v3"><title>Note</title><component>
            <nonXMLBody><text mediaType="text/plain">line one
line two</text></nonXMLBody></component></ClinicalDocument>"#;
        let CdaBody::Text(l) = cda_body(txt.as_bytes()).unwrap() else {
            panic!()
        };
        assert_eq!(l.len(), 3);
        assert_eq!(l[2].text, "line two");
    }

    #[test]
    fn cda_refuses_garbage_without_panicking() {
        assert!(cda_body(b"").is_err());
        assert!(cda_body(b"<html/>").is_err());
        assert!(cda_body(b"<ClinicalDocument>").is_err());
        assert!(cda_body(&[0xFF, 0xFE, 0x00]).is_err());
        assert!(cda_body(b"<ClinicalDocument/>").is_err());
    }

    /// Elements nested 100 000 deep: roxmltree's parser recursed into each and overflowed the
    /// stack before the body walk's own depth cap could act.
    #[test]
    fn deeply_nested_cda_is_refused_on_a_small_stack() {
        let n = 100_000;
        let mut xml = String::from("<ClinicalDocument>");
        xml.push_str(&"<a>".repeat(n));
        xml.push_str(&"</a>".repeat(n));
        xml.push_str("</ClinicalDocument>");
        let r = specular_thread::on_stack(256 * 1024, || cda_body(xml.as_bytes()).is_err());
        assert!(r);
    }

    #[test]
    fn encapsulated_documents_found_with_title_and_exact_length() {
        use crate::fixture;
        let pdf = b"%PDF-1.4\n%odd length\n";
        assert_eq!(pdf.len() % 2, 1);
        let file = fixture::document(
            fixture::ENCAPSULATED_PDF,
            "application/pdf",
            Some("Dose report"),
            pdf,
        );
        let d = Dicom::parse(&file).unwrap();
        let doc = d.document(&file).unwrap();
        assert_eq!(doc.kind, Kind::Pdf);
        assert_eq!(&d.buf(&file)[doc.range.clone()], pdf);
        assert_eq!(doc.title.as_deref(), Some("Dose report"));
        assert_eq!(doc.mime.as_deref(), Some("application/pdf"));
        assert!(matches!(
            d.check_supported(),
            Err(crate::Error::NoPixelData)
        ));
        assert_eq!(
            d.summary(&file).study_description.as_deref(),
            Some("Synthetic study")
        );

        // No title: the concept name. Class and MIME type pick the kind.
        for (class, mime, kind) in [
            (fixture::ENCAPSULATED_CDA, "text/XML", Kind::Cda),
            (fixture::ENCAPSULATED_STL, "model/stl", Kind::Stl),
            (fixture::ENCAPSULATED_OBJ, "model/obj", Kind::Obj),
            (fixture::ENCAPSULATED_MTL, "model/mtl", Kind::Mtl),
            ("1.2.3.4", "application/pdf; x=y", Kind::Pdf),
            ("1.2.3.4", "image/x-unknown", Kind::Other),
        ] {
            let file = fixture::document(class, mime, None, b"solid x\nendsolid x\n");
            let d = Dicom::parse(&file).unwrap();
            let doc = d.document(&file).unwrap();
            assert_eq!(doc.kind, kind, "{class} {mime}");
            assert_eq!(doc.title.as_deref(), Some("Diagnostic imaging report"));
            assert_eq!(doc.range.len(), 19);
        }

        // Damaged files do not panic.
        for cut in (0..file.len()).step_by(7) {
            if let Ok(d) = Dicom::parse(&file[..cut]) {
                let _ = d.document(&file[..cut]);
            }
        }
    }
}
