//! The inspection report ([`crate::report::Report`]) as a Word document (Office Open XML
//! WordprocessingML, ECMA-376), written by hand: paragraphs, tables and PNG pictures, nothing
//! else.
//!
//! - The title, the line under it and the notes, then per image: its name as a heading, the
//!   image with the measurements drawn into it (numbered as in the table), the visible photo
//!   when there is one, the facts as a two-column table and the measurements table.
//! - A4 portrait with 2 cm margins. Right-to-left languages set paragraphs and tables right to
//!   left (`w:bidi`, `w:bidiVisual`, `w:rtl`).
//! - The marks are burned into the pictures (a Word picture has no vector overlay): yellow
//!   lines with a dark edge, and the measurement's number in a dark box.

use std::fmt::Write as _;
use std::io::{Cursor, Write};

use crate::report::{Entry, Image, Mark, Report};

/// EMU per millimetre.
const EMU_MM: f64 = 36_000.0;
/// Widest and tallest a picture is placed (mm).
const PICTURE_W: f64 = 165.0;
const PICTURE_H: f64 = 115.0;

fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            // XML 1.0 has no other control characters.
            c if (c as u32) < 0x20 && !matches!(c, '\t' | '\n' | '\r') => {}
            c => o.push(c),
        }
    }
    o
}

/// The document's body, as it is built.
struct Body {
    xml: String,
    rtl: bool,
    /// PNG pictures, in order (`media/image{n}.png`, relationship `rIdImg{n}`).
    pictures: Vec<Vec<u8>>,
}

impl Body {
    /// A paragraph of runs (lines split at `\n` with breaks), in `style`.
    fn para(&mut self, style: Option<&str>, text: &str, bold: bool) {
        self.xml.push_str("<w:p><w:pPr>");
        if let Some(s) = style {
            let _ = write!(self.xml, "<w:pStyle w:val=\"{s}\"/>");
        }
        if self.rtl {
            self.xml.push_str("<w:bidi/>");
        }
        self.xml.push_str("</w:pPr>");
        for (i, line) in text.split('\n').enumerate() {
            self.xml.push_str("<w:r>");
            self.rpr(bold);
            if i > 0 {
                self.xml.push_str("<w:br/>");
            }
            let _ = write!(
                self.xml,
                "<w:t xml:space=\"preserve\">{}</w:t></w:r>",
                esc(line)
            );
        }
        self.xml.push_str("</w:p>");
    }

    fn rpr(&mut self, bold: bool) {
        if bold || self.rtl {
            self.xml.push_str("<w:rPr>");
            if bold {
                self.xml.push_str("<w:b/><w:bCs/>");
            }
            if self.rtl {
                self.xml.push_str("<w:rtl/>");
            }
            self.xml.push_str("</w:rPr>");
        }
    }

    /// A picture in a paragraph of its own, at most `max_w` × `max_h` mm.
    fn picture(&mut self, img: &Image, name: &str, (max_w, max_h): (f64, f64)) {
        let Ok(png) = specular_png::encode(
            img.width,
            img.height,
            specular_png::ColorType::Rgb,
            &img.rgb,
        ) else {
            return;
        };
        self.pictures.push(png);
        let n = self.pictures.len();
        let (w, h) = (f64::from(img.width.max(1)), f64::from(img.height.max(1)));
        let s = (max_w / w).min(max_h / h);
        let (cx, cy) = ((w * s * EMU_MM) as u64, (h * s * EMU_MM) as u64);
        self.xml.push_str("<w:p><w:pPr>");
        if self.rtl {
            self.xml.push_str("<w:bidi/>");
        }
        self.xml
            .push_str("<w:jc w:val=\"center\"/></w:pPr><w:r><w:drawing>");
        let _ = write!(
            self.xml,
            "<wp:inline distT=\"0\" distB=\"0\" distL=\"0\" distR=\"0\">\
             <wp:extent cx=\"{cx}\" cy=\"{cy}\"/>\
             <wp:docPr id=\"{n}\" name=\"{name}\"/>\
             <a:graphic><a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/picture\">\
             <pic:pic><pic:nvPicPr><pic:cNvPr id=\"{n}\" name=\"image{n}.png\"/><pic:cNvPicPr/></pic:nvPicPr>\
             <pic:blipFill><a:blip r:embed=\"rIdImg{n}\"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill>\
             <pic:spPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"{cx}\" cy=\"{cy}\"/></a:xfrm>\
             <a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></pic:spPr></pic:pic>\
             </a:graphicData></a:graphic></wp:inline>",
            name = esc(name)
        );
        self.xml.push_str("</w:drawing></w:r></w:p>");
    }

    /// A table with a bold, shaded first row when `head`.
    fn table(&mut self, rows: &[Vec<String>], head: bool) {
        let cols = rows.iter().map(Vec::len).max().unwrap_or(0);
        if cols == 0 {
            return;
        }
        // The text width (twentieths of a point): A4 less 2 cm margins.
        let total = 9638u32;
        let w = total / cols as u32;
        self.xml
            .push_str("<w:tbl><w:tblPr><w:tblStyle w:val=\"TableGrid\"/>");
        if self.rtl {
            self.xml.push_str("<w:bidiVisual/>");
        }
        let _ = write!(self.xml, "<w:tblW w:w=\"{total}\" w:type=\"dxa\"/>");
        self.xml.push_str("<w:tblBorders>");
        for side in ["top", "left", "bottom", "right", "insideH", "insideV"] {
            let _ = write!(
                self.xml,
                "<w:{side} w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"C8CAD0\"/>"
            );
        }
        self.xml.push_str("</w:tblBorders></w:tblPr><w:tblGrid>");
        for _ in 0..cols {
            let _ = write!(self.xml, "<w:gridCol w:w=\"{w}\"/>");
        }
        self.xml.push_str("</w:tblGrid>");
        for (i, row) in rows.iter().enumerate() {
            let bold = head && i == 0;
            self.xml.push_str("<w:tr>");
            if bold {
                self.xml.push_str("<w:trPr><w:tblHeader/></w:trPr>");
            }
            for c in 0..cols {
                let _ = write!(
                    self.xml,
                    "<w:tc><w:tcPr><w:tcW w:w=\"{w}\" w:type=\"dxa\"/>"
                );
                if bold {
                    self.xml
                        .push_str("<w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"ECEDF0\"/>");
                }
                self.xml.push_str("</w:tcPr>");
                let cell = row.get(c).map_or("", String::as_str);
                self.para(None, cell, bold);
                self.xml.push_str("</w:tc>");
            }
            self.xml.push_str("</w:tr>");
        }
        self.xml.push_str("</w:tbl>");
        // Word needs a paragraph between two tables.
        self.para(None, "", false);
    }

    fn page_break(&mut self) {
        self.xml
            .push_str("<w:p><w:r><w:br w:type=\"page\"/></w:r></w:p>");
    }
}

// ----- Burning the marks in ----------------------------------------------------------------

/// Digits 0–9, 5 × 7, a row a byte (bit 4 is the left column).
const DIGITS: [[u8; 7]; 10] = [
    [0x0E, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0E],
    [0x04, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x0E],
    [0x0E, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1F],
    [0x1F, 0x02, 0x04, 0x02, 0x01, 0x11, 0x0E],
    [0x02, 0x06, 0x0A, 0x12, 0x1F, 0x02, 0x02],
    [0x1F, 0x10, 0x1E, 0x01, 0x01, 0x11, 0x0E],
    [0x06, 0x08, 0x10, 0x1E, 0x11, 0x11, 0x0E],
    [0x1F, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
    [0x0E, 0x11, 0x11, 0x0E, 0x11, 0x11, 0x0E],
    [0x0E, 0x11, 0x11, 0x0F, 0x01, 0x02, 0x0C],
];

const MARK: [u8; 3] = [255, 214, 0];
const EDGE: [u8; 3] = [20, 20, 24];

fn put(img: &mut Image, x: i64, y: i64, c: [u8; 3]) {
    if x >= 0 && y >= 0 && (x as u32) < img.width && (y as u32) < img.height {
        let i = (y as usize * img.width as usize + x as usize) * 3;
        img.rgb[i..i + 3].copy_from_slice(&c);
    }
}

fn disc(img: &mut Image, x: f64, y: f64, r: f64, c: [u8; 3]) {
    let ri = r.ceil() as i64;
    for dy in -ri..=ri {
        for dx in -ri..=ri {
            if (dx * dx + dy * dy) as f64 <= r * r {
                put(img, x.round() as i64 + dx, y.round() as i64 + dy, c);
            }
        }
    }
}

fn segment(img: &mut Image, a: (f64, f64), b: (f64, f64), r: f64, c: [u8; 3]) {
    let n = ((b.0 - a.0).hypot(b.1 - a.1).ceil() as usize).max(1);
    for i in 0..=n {
        let t = i as f64 / n as f64;
        disc(img, a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t, r, c);
    }
}

/// The digits of `label` at `(x, y)` (top left), `k` pixels a font pixel, on a dark box.
fn label(img: &mut Image, s: &str, x: f64, y: f64, k: i64) {
    let digits: Vec<usize> = s
        .chars()
        .filter_map(|c| c.to_digit(10))
        .map(|d| d as usize)
        .collect();
    if digits.is_empty() {
        return;
    }
    let (x, y) = (x.round() as i64, y.round() as i64);
    let w = (digits.len() as i64 * 6 + 1) * k;
    for dy in 0..9 * k {
        for dx in 0..w {
            put(img, x + dx, y + dy, EDGE);
        }
    }
    for (n, d) in digits.iter().enumerate() {
        for (row, bits) in DIGITS[*d].iter().enumerate() {
            for col in 0..5 {
                if bits & (0x10 >> col) != 0 {
                    for sy in 0..k {
                        for sx in 0..k {
                            put(
                                img,
                                x + (1 + n as i64 * 6 + col) * k + sx,
                                y + (1 + row as i64) * k + sy,
                                [255, 255, 255],
                            );
                        }
                    }
                }
            }
        }
    }
}

/// `img` with `marks` (in its pixels) drawn into it.
pub fn burn(img: &Image, marks: &[Mark]) -> Image {
    let mut out = img.clone();
    if out.rgb.len() < out.width as usize * out.height as usize * 3 {
        return out;
    }
    let side = f64::from(out.width.max(out.height));
    let r = (side / 500.0).max(1.0);
    let k = ((side / 400.0).round() as i64).max(1);
    for m in marks {
        let pts = &m.points;
        let mut segs: Vec<((f64, f64), (f64, f64))> =
            pts.windows(2).map(|w| (w[0], w[1])).collect();
        if m.closed && pts.len() > 2 {
            segs.push((pts[pts.len() - 1], pts[0]));
        }
        for (rr, c) in [(r + 1.0, EDGE), (r, MARK)] {
            if let [p] = pts[..] {
                disc(&mut out, p.0, p.1, rr * 3.0, c);
            }
            for (a, b) in &segs {
                segment(&mut out, *a, *b, rr, c);
            }
        }
        if let Some(p) = pts.first() {
            label(&mut out, &m.label, p.0 + 3.0 * r, p.1 - 12.0 * k as f64, k);
        }
    }
    out
}

// ----- The package -------------------------------------------------------------------------

const STYLES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
<w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii="Calibri" w:hAnsi="Calibri" w:cs="Arial"/><w:sz w:val="20"/><w:szCs w:val="20"/></w:rPr></w:rPrDefault>
<w:pPrDefault><w:pPr><w:spacing w:after="80"/></w:pPr></w:pPrDefault></w:docDefaults>
<w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/></w:style>
<w:style w:type="paragraph" w:styleId="Title"><w:name w:val="Title"/><w:basedOn w:val="Normal"/><w:pPr><w:spacing w:after="120"/></w:pPr><w:rPr><w:b/><w:bCs/><w:sz w:val="40"/><w:szCs w:val="40"/></w:rPr></w:style>
<w:style w:type="paragraph" w:styleId="Subtitle"><w:name w:val="Subtitle"/><w:basedOn w:val="Normal"/><w:rPr><w:color w:val="60626A"/></w:rPr></w:style>
<w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:basedOn w:val="Normal"/><w:pPr><w:keepNext/><w:spacing w:before="240" w:after="120"/><w:outlineLvl w:val="0"/></w:pPr><w:rPr><w:b/><w:bCs/><w:sz w:val="30"/><w:szCs w:val="30"/></w:rPr></w:style>
<w:style w:type="paragraph" w:styleId="Heading2"><w:name w:val="heading 2"/><w:basedOn w:val="Normal"/><w:pPr><w:keepNext/><w:spacing w:before="160" w:after="80"/><w:outlineLvl w:val="1"/></w:pPr><w:rPr><w:b/><w:bCs/><w:sz w:val="24"/><w:szCs w:val="24"/></w:rPr></w:style>
<w:style w:type="paragraph" w:styleId="Caption"><w:name w:val="caption"/><w:basedOn w:val="Normal"/><w:pPr><w:jc w:val="center"/></w:pPr><w:rPr><w:color w:val="60626A"/><w:sz w:val="18"/><w:szCs w:val="18"/></w:rPr></w:style>
<w:style w:type="table" w:styleId="TableGrid"><w:name w:val="Table Grid"/><w:tblPr><w:tblCellMar><w:left w:w="80" w:type="dxa"/><w:right w:w="80" w:type="dxa"/></w:tblCellMar></w:tblPr></w:style>
</w:styles>
"#;

fn entry(b: &mut Body, e: &Entry, table_heading: &str) {
    b.para(Some("Heading1"), &e.title, false);
    let main = burn(&e.thermal, &e.marks);
    b.picture(&main, &e.title, (PICTURE_W, PICTURE_H));
    if !e.thermal_caption.is_empty() {
        b.para(Some("Caption"), &e.thermal_caption, false);
    }
    if let Some(v) = &e.visible {
        b.picture(v, &e.title, (PICTURE_W / 2.0, PICTURE_H / 2.0));
        if !e.visible_caption.is_empty() {
            b.para(Some("Caption"), &e.visible_caption, false);
        }
    }
    if let Some(s) = &e.scale {
        b.para(None, &format!("{} – {}", s.low, s.high), false);
    }
    if !e.facts.is_empty() {
        let rows: Vec<Vec<String>> = e
            .facts
            .iter()
            .map(|(k, v)| vec![k.clone(), v.clone()])
            .collect();
        b.table(&rows, false);
    }
    b.para(Some("Heading2"), table_heading, false);
    if e.table.len() > 1 {
        b.table(&e.table, true);
    } else {
        b.para(None, &e.no_table, false);
    }
}

/// The report as a `.docx` file.
pub fn write(r: &Report) -> Vec<u8> {
    let mut b = Body {
        xml: String::new(),
        rtl: r.rtl,
        pictures: Vec::new(),
    };
    b.para(Some("Title"), &r.title, false);
    if !r.subtitle.is_empty() {
        b.para(Some("Subtitle"), &r.subtitle, false);
    }
    if !r.notes.trim().is_empty() {
        b.para(Some("Heading2"), &r.notes_heading, false);
        b.para(None, r.notes.trim(), false);
    }
    for (i, e) in r.entries.iter().enumerate() {
        if i > 0 {
            b.page_break();
        }
        entry(&mut b, e, &r.table_heading);
    }
    let mut doc = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\" \
         xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" \
         xmlns:wp=\"http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing\" \
         xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" \
         xmlns:pic=\"http://schemas.openxmlformats.org/drawingml/2006/picture\"><w:body>",
    );
    doc.push_str(&b.xml);
    doc.push_str(
        "<w:sectPr><w:pgSz w:w=\"11906\" w:h=\"16838\"/>\
         <w:pgMar w:top=\"1134\" w:right=\"1134\" w:bottom=\"1134\" w:left=\"1134\" \
         w:header=\"567\" w:footer=\"567\" w:gutter=\"0\"/></w:sectPr></w:body></w:document>\n",
    );
    let mut rels = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
         <Relationship Id=\"rIdStyles\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles\" Target=\"styles.xml\"/>",
    );
    for n in 1..=b.pictures.len() {
        let _ = write!(
            rels,
            "<Relationship Id=\"rIdImg{n}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/image\" Target=\"media/image{n}.png\"/>"
        );
    }
    rels.push_str("</Relationships>\n");
    let core = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <cp:coreProperties xmlns:cp=\"http://schemas.openxmlformats.org/package/2006/metadata/core-properties\" \
         xmlns:dc=\"http://purl.org/dc/elements/1.1/\"><dc:title>{}</dc:title><dc:creator>{}</dc:creator></cp:coreProperties>\n",
        esc(&r.title),
        esc(&r.producer)
    );
    let parts: [(&str, &[u8]); 6] = [
        (
            "[Content_Types].xml",
            b"<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\
<Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\
<Default Extension=\"xml\" ContentType=\"application/xml\"/>\
<Default Extension=\"png\" ContentType=\"image/png\"/>\
<Override PartName=\"/word/document.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml\"/>\
<Override PartName=\"/word/styles.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml\"/>\
<Override PartName=\"/docProps/core.xml\" ContentType=\"application/vnd.openxmlformats-package.core-properties+xml\"/>\
</Types>\n",
        ),
        (
            "_rels/.rels",
            b"<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
<Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"word/document.xml\"/>\
<Relationship Id=\"rId2\" Type=\"http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties\" Target=\"docProps/core.xml\"/>\
</Relationships>\n",
        ),
        ("docProps/core.xml", core.as_bytes()),
        ("word/document.xml", doc.as_bytes()),
        ("word/styles.xml", STYLES.as_bytes()),
        ("word/_rels/document.xml.rels", rels.as_bytes()),
    ];
    let mut z = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let deflate = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let stored =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let mut add = |name: &str, data: &[u8], o| -> zip::result::ZipResult<()> {
        z.start_file(name, o)?;
        z.write_all(data)?;
        Ok(())
    };
    let mut ok = parts.iter().all(|(n, d)| add(n, d, deflate).is_ok());
    for (i, p) in b.pictures.iter().enumerate() {
        ok &= add(&format!("word/media/image{}.png", i + 1), p, stored).is_ok();
    }
    if !ok {
        return Vec::new();
    }
    z.finish().map(Cursor::into_inner).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::io::Read;

    use super::*;
    use crate::report::Scale;

    fn image(w: u32, h: u32) -> Image {
        let rgb = (0..h)
            .flat_map(|y| (0..w).flat_map(move |x| [(x * 255 / w) as u8, (y * 255 / h) as u8, 90]))
            .collect();
        Image {
            rgb,
            width: w,
            height: h,
        }
    }

    /// Two images: the first with a photo, facts and two measurements; the second with no
    /// measurements. Hebrew text when `rtl`.
    pub(crate) fn sample(rtl: bool) -> Report {
        let head = vec!["#".into(), "Measurement".into(), "Value".into()];
        let first = Entry {
            title: "DJI_0001.JPG".into(),
            thermal: image(320, 240),
            marks: vec![
                Mark {
                    points: vec![(40.0, 40.0), (200.0, 120.0)],
                    closed: false,
                    label: "1".into(),
                },
                Mark {
                    points: vec![(60.0, 150.0), (160.0, 150.0), (160.0, 220.0), (60.0, 220.0)],
                    closed: true,
                    label: "12".into(),
                },
            ],
            scale: Some(Scale {
                colours: vec![[0, 0, 0], [255, 255, 255]],
                low: "12.4 °C".into(),
                high: "48.1 °C".into(),
            }),
            visible: Some(image(160, 120)),
            thermal_caption: "Image with measurements".into(),
            visible_caption: "Visible photo".into(),
            facts: vec![
                ("Camera".into(), "FC6520 & <test>".into()),
                ("Place".into(), "47.37010, 8.54600".into()),
            ],
            table: vec![
                head.clone(),
                vec![
                    "1".into(),
                    "Crack width".into(),
                    "Mean width 0.61 mm".into(),
                ],
                vec![
                    "12".into(),
                    if rtl {
                        "\u{5de}\u{5dc}\u{5d1}\u{5df}".into()
                    } else {
                        "Rectangle".into()
                    },
                    "4.2 cm²".into(),
                ],
            ],
            no_table: "No measurements".into(),
        };
        let second = Entry {
            title: "DJI_0002.JPG".into(),
            thermal: image(200, 300),
            marks: Vec::new(),
            scale: None,
            visible: None,
            thermal_caption: String::new(),
            visible_caption: String::new(),
            facts: vec![("Place".into(), "47.37020, 8.54610".into())],
            table: vec![head],
            no_table: "No measurements".into(),
        };
        Report {
            title: if rtl {
                "\u{5d3}\u{5d5}\u{5d7}".into()
            } else {
                "Inspection report".into()
            },
            subtitle: "2026-10-25 14:03".into(),
            notes: "Bridge pier 3, north face.\nSecond line.".into(),
            notes_heading: "Notes".into(),
            table_heading: "Measurements".into(),
            page_labels: vec!["Page 1 of 2".into(), "Page 2 of 2".into()],
            entries: vec![first, second],
            rtl,
            created: None,
            producer: "Specular test".into(),
        }
    }

    #[test]
    fn docx_is_a_package_of_well_formed_parts() {
        for rtl in [false, true] {
            let bytes = write(&sample(rtl));
            let mut z = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
            let mut names: Vec<String> = (0..z.len())
                .map(|i| z.by_index(i).unwrap().name().unwrap().into_owned())
                .collect();
            names.sort();
            assert_eq!(
                names,
                [
                    "[Content_Types].xml",
                    "_rels/.rels",
                    "docProps/core.xml",
                    "word/_rels/document.xml.rels",
                    "word/document.xml",
                    "word/media/image1.png",
                    "word/media/image2.png",
                    "word/media/image3.png",
                    "word/styles.xml",
                ]
            );
            for n in names.iter().filter(|n| !n.ends_with(".png")) {
                let mut s = String::new();
                z.by_name(n).unwrap().read_to_string(&mut s).unwrap();
                let doc = roxmltree::Document::parse(&s).unwrap_or_else(|e| panic!("{n}: {e}"));
                if n == "word/document.xml" {
                    let count =
                        |tag: &str| doc.descendants().filter(|d| d.has_tag_name(tag)).count();
                    // Facts and measurements of the first; facts of the second.
                    assert_eq!(count("tbl"), 3);
                    assert_eq!(count("drawing"), 3);
                    assert_eq!(count("bidi") > 0, rtl);
                    assert!(s.contains("FC6520 &amp; &lt;test&gt;"));
                }
            }
        }
    }

    #[test]
    fn marks_are_burned_in() {
        let img = image(100, 80);
        let m = Mark {
            points: vec![(10.0, 40.0), (90.0, 40.0)],
            closed: false,
            label: "3".into(),
        };
        let out = burn(&img, &[m]);
        let at = |im: &Image, x: usize, y: usize| {
            let i = (y * 100 + x) * 3;
            [im.rgb[i], im.rgb[i + 1], im.rgb[i + 2]]
        };
        assert_eq!(at(&out, 50, 40), MARK);
        assert_eq!(at(&out, 50, 70), at(&img, 50, 70));
    }

    /// Writes the samples for the python-docx oracle (`tools/quality/docx_oracle.py`).
    #[test]
    #[ignore = "run by tools/quality/docx_oracle.py"]
    fn docx_dump() {
        let Some(dir) = std::env::var_os("DOCX_ORACLE_DIR").map(std::path::PathBuf::from) else {
            return;
        };
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("report-ltr.docx"), write(&sample(false))).unwrap();
        std::fs::write(dir.join("report-rtl.docx"), write(&sample(true))).unwrap();
    }
}
