use super::*;
use specular_drawpage::{Op, Paint, RenderRequest};
use std::sync::Arc;

fn crc32(d: &[u8]) -> u32 {
    let mut c = 0xffff_ffffu32;
    for &b in d {
        c ^= u32::from(b);
        for _ in 0..8 {
            c = if c & 1 != 0 {
                0xedb8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
        }
    }
    !c
}

/// A ZIP of stored members.
fn zip(members: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in members {
        let off = out.len() as u32;
        let crc = crc32(data);
        let mut h = Vec::new();
        h.extend_from_slice(b"PK\x03\x04");
        h.extend_from_slice(&[20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        h.extend_from_slice(&crc.to_le_bytes());
        h.extend_from_slice(&(data.len() as u32).to_le_bytes());
        h.extend_from_slice(&(data.len() as u32).to_le_bytes());
        h.extend_from_slice(&(name.len() as u16).to_le_bytes());
        h.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&h);
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(data);
        central.extend_from_slice(b"PK\x01\x02");
        central.extend_from_slice(&[20, 0, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&(data.len() as u32).to_le_bytes());
        central.extend_from_slice(&(data.len() as u32).to_le_bytes());
        central.extend_from_slice(&(name.len() as u16).to_le_bytes());
        central.extend_from_slice(&[0; 12]);
        central.extend_from_slice(&off.to_le_bytes());
        central.extend_from_slice(name.as_bytes());
    }
    let cd_off = out.len() as u32;
    out.extend_from_slice(&central);
    out.extend_from_slice(b"PK\x05\x06\0\0\0\0");
    let n = members.len() as u16;
    out.extend_from_slice(&n.to_le_bytes());
    out.extend_from_slice(&n.to_le_bytes());
    out.extend_from_slice(&(central.len() as u32).to_le_bytes());
    out.extend_from_slice(&cd_off.to_le_bytes());
    out.extend_from_slice(&[0, 0]);
    out
}

const NS: &str = r#"xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" xmlns:draw="urn:oasis:names:tc:opendocument:xmlns:drawing:1.0" xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0" xmlns:svg="urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0" xmlns:xlink="http://www.w3.org/1999/xlink" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:meta="urn:oasis:names:tc:opendocument:xmlns:meta:1.0""#;

fn styles_xml() -> String {
    format!(
        r##"<?xml version="1.0" encoding="UTF-8"?><office:document-styles {NS}><office:styles>
<draw:gradient draw:name="g" draw:style="linear" draw:start-color="#000000" draw:end-color="#ffffff" draw:angle="90deg" draw:border="0%"/>
<style:default-style style:family="graphic"><style:graphic-properties draw:fill-color="#00ff00" svg:stroke-color="#000000" draw:stroke="none"/><style:text-properties fo:font-size="12pt"/></style:default-style>
</office:styles><office:automatic-styles>
<style:page-layout style:name="PM"><style:page-layout-properties fo:page-width="2in" fo:page-height="1in"/></style:page-layout>
<style:style style:name="Mdp" style:family="drawing-page"><style:drawing-page-properties draw:fill="solid" draw:fill-color="#0000ff"/></style:style>
</office:automatic-styles><office:master-styles><style:master-page style:name="M" style:page-layout-name="PM" draw:style-name="Mdp"/></office:master-styles></office:document-styles>"##
    )
}

fn content_xml(body: &str) -> String {
    format!(
        r##"<?xml version="1.0" encoding="UTF-8"?><office:document-content {NS}><office:automatic-styles>
<style:style style:name="red" style:family="graphic"><style:graphic-properties draw:fill="solid" draw:fill-color="#ff0000"/></style:style>
<style:style style:name="grad" style:family="graphic"><style:graphic-properties draw:fill="gradient" draw:fill-gradient-name="g"/></style:style>
<style:style style:name="half" style:family="graphic" style:parent-style-name="red"><style:graphic-properties draw:opacity="50%"/></style:style>
</office:automatic-styles><office:body><office:drawing>{body}</office:drawing></office:body></office:document-content>"##
    )
}

fn meta_xml() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><office:document-meta {NS}><office:meta><meta:generator>TestGen/1.0</meta:generator><dc:title>Shapes</dc:title><meta:keyword>a</meta:keyword><meta:keyword>b</meta:keyword><dc:creator>Someone</dc:creator></office:meta></office:document-meta>"#
    )
}

fn package(body: &str, extra: &[(&str, &[u8])]) -> Vec<u8> {
    let s = styles_xml();
    let c = content_xml(body);
    let m = meta_xml();
    let mut members: Vec<(&str, &[u8])> = vec![
        ("mimetype", b"application/vnd.oasis.opendocument.graphics"),
        ("styles.xml", s.as_bytes()),
        ("content.xml", c.as_bytes()),
        ("meta.xml", m.as_bytes()),
    ];
    members.extend_from_slice(extra);
    zip(&members)
}

fn render(doc: &Document, page: usize) -> (u32, u32, Vec<u8>) {
    let p = &doc.pages[page];
    let (w, h) = (p.size.0.round() as u32, p.size.1.round() as u32);
    let b = specular_drawpage::render(
        p,
        RenderRequest {
            scale_x: 1.0,
            scale_y: 1.0,
            x: 0,
            y: 0,
            width: w,
            height: h,
        },
        &|| false,
    )
    .unwrap();
    (w, h, b)
}

fn px(b: &[u8], w: u32, x: u32, y: u32) -> [u8; 3] {
    let i = ((y * w + x) * 4) as usize;
    [b[i], b[i + 1], b[i + 2]]
}

const PAGE: &str = r#"<draw:page draw:name="One" draw:master-page-name="M">"#;

#[test]
fn shapes_styles_and_page() {
    let body = format!(
        r#"{PAGE}<draw:rect draw:style-name="red" svg:x="0in" svg:y="0in" svg:width="0.5in" svg:height="0.5in"/>
<draw:ellipse svg:x="0.5in" svg:y="0in" svg:width="0.5in" svg:height="0.5in"/>
<draw:rect draw:style-name="grad" svg:x="1in" svg:y="0in" svg:width="1in" svg:height="0.5in"/>
<draw:rect draw:style-name="half" svg:x="0in" svg:y="0.5in" svg:width="0.5in" svg:height="0.5in"/>
<dr3d:scene xmlns:dr3d="urn:oasis:names:tc:opendocument:xmlns:dr3d:1.0"/>
</draw:page><draw:page draw:name="Two" draw:master-page-name="M"/>"#
    );
    let d = package(&body, &[]);
    assert_eq!(probe_file(&d), Some(Kind::Graphics));
    let doc = open_graphics(&d, None).unwrap();
    assert_eq!(doc.pages.len(), 2);
    assert_eq!(doc.pages[0].size, (144.0, 72.0));
    assert_eq!(doc.facts.page_names, vec!["One", "Two"]);
    assert_eq!(doc.facts.generator.as_deref(), Some("TestGen/1.0"));
    assert_eq!(doc.facts.title.as_deref(), Some("Shapes"));
    assert_eq!(doc.facts.keywords.as_deref(), Some("a, b"));
    assert_eq!(doc.facts.author.as_deref(), Some("Someone"));
    assert_eq!(doc.facts.not_drawn, vec![("3D scene".to_string(), 1)]);
    assert_eq!(
        doc.describe(0),
        "OpenDocument drawing, page \"One\", 1 object not drawn"
    );
    let (w, _, b) = render(&doc, 0);
    // Red rect, default green ellipse centre, blue page background at the ellipse corner.
    assert_eq!(px(&b, w, 18, 18), [255, 0, 0]);
    assert_eq!(px(&b, w, 54, 18), [0, 255, 0]);
    assert_eq!(px(&b, w, 37, 1), [0, 0, 255]);
    // Gradient left (dark) to right (light) at 90°.
    let (l, r) = (px(&b, w, 75, 18), px(&b, w, 140, 18));
    assert!(l[0] < 40 && r[0] > 215, "{l:?} {r:?}");
    // 50 % red over blue.
    let h = px(&b, w, 18, 54);
    assert!(
        (120..136).contains(&h[0]) && (120..136).contains(&h[2]),
        "{h:?}"
    );
    // The empty second page is the background.
    let (w2, _, b2) = render(&doc, 1);
    assert_eq!(px(&b2, w2, 70, 30), [0, 0, 255]);
}

#[test]
fn custom_shape_and_text() {
    let body = format!(
        r#"{PAGE}<draw:custom-shape draw:style-name="red" svg:x="0in" svg:y="0in" svg:width="1in" svg:height="1in"><text:p>Hi <text:span>there</text:span><text:s text:c="2"/>x</text:p>
<draw:enhanced-geometry svg:viewBox="0 0 21600 21600" draw:modifiers="10800" draw:enhanced-path="M ?f0 0 L 21600 21600 0 21600 Z N" draw:text-areas="0 ?f1 21600 21600"><draw:equation draw:name="f0" draw:formula="$0"/><draw:equation draw:name="f1" draw:formula="?f0 "/></draw:enhanced-geometry></draw:custom-shape>
<draw:custom-shape draw:style-name="red" svg:x="1in" svg:y="0in" svg:width="1in" svg:height="1in"><draw:enhanced-geometry draw:type="ellipse"/></draw:custom-shape>
<draw:frame svg:x="1in" svg:y="0in" svg:width="1in" svg:height="1in"><draw:text-box><text:p>Box</text:p><text:list><text:list-item><text:p>item</text:p></text:list-item></text:list></draw:text-box></draw:frame>
</draw:page>"#
    );
    let doc = open_graphics(&package(&body, &[]), None).unwrap();
    let ops = &doc.pages[0].ops;
    let texts: Vec<&specular_drawpage::TextBlock> = ops
        .iter()
        .filter_map(|o| match o {
            Op::Text(t) => Some(&**t),
            _ => None,
        })
        .collect();
    assert_eq!(texts.len(), 2);
    let s: String = texts[0].paragraphs[0]
        .runs
        .iter()
        .map(|r| r.text.as_str())
        .collect();
    assert_eq!(s, "Hi there\u{a0}\u{a0}x");
    assert_eq!(texts[0].paragraphs[0].runs[0].size, 12.0);
    assert_eq!(texts[0].valign, specular_drawpage::VAlign::Middle);
    assert_eq!(texts[1].valign, specular_drawpage::VAlign::Top);
    assert_eq!(texts[1].paragraphs.len(), 2);
    // The list label leads the item's runs, its tail spaced out to the label width.
    let item = &texts[1].paragraphs[1];
    assert_eq!(item.runs[0].text, "\u{2022}");
    assert!(item.indent_first < 0.0 && item.indent_left > 0.0);
    let (w, _, b) = render(&doc, 0);
    // Triangle apex at the top centre: left top corner is background, bottom is red.
    assert_eq!(px(&b, w, 3, 3), [0, 0, 255]);
    assert_eq!(px(&b, w, 3, 69), [255, 0, 0]);
    assert_eq!(px(&b, w, 108, 36), [255, 0, 0]);
    assert_eq!(px(&b, w, 140, 2), [0, 0, 255]);
}

fn png_1x1(rgb: [u8; 3]) -> Vec<u8> {
    // Built by hand: a 1×1 RGB PNG with a stored zlib stream.
    fn chunk(out: &mut Vec<u8>, ty: &[u8], data: &[u8]) {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let mut c = ty.to_vec();
        c.extend_from_slice(data);
        out.extend_from_slice(&c);
        out.extend_from_slice(&crc32(&c).to_be_bytes());
    }
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&1u32.to_be_bytes());
    ihdr.extend_from_slice(&1u32.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &ihdr);
    let raw = [0, rgb[0], rgb[1], rgb[2]];
    let mut z = vec![0x78, 0x01, 1, 4, 0, 0xfb, 0xff];
    z.extend_from_slice(&raw);
    let (mut a, mut b) = (1u32, 0u32);
    for &x in &raw {
        a = (a + u32::from(x)) % 65521;
        b = (b + a) % 65521;
    }
    z.extend_from_slice(&((b << 16) | a).to_be_bytes());
    chunk(&mut out, b"IDAT", &z);
    chunk(&mut out, b"IEND", &[]);
    out
}

#[test]
fn images_and_thumbnail() {
    let png = png_1x1([10, 200, 30]);
    let body = format!(
        r#"{PAGE}<draw:frame svg:x="0in" svg:y="0in" svg:width="1in" svg:height="1in"><draw:image xlink:href="Pictures/a.png"/></draw:frame>
<draw:frame svg:x="1in" svg:y="0in" svg:width="1in" svg:height="1in"><draw:image xlink:href="Pictures/missing.png"/></draw:frame>
<draw:frame svg:x="1in" svg:y="0in" svg:width="1in" svg:height="1in"><draw:object xlink:href="./Object 1"/></draw:frame></draw:page>"#
    );
    let d = package(
        &body,
        &[("Pictures/a.png", &png), ("Thumbnails/thumbnail.png", &png)],
    );
    let decode: ImageDecoder = Arc::new(|b: &[u8]| {
        let img = specular_png::decode(b, &Default::default()).ok()?;
        Some((img.width, img.height, img.to_rgba8()))
    });
    let doc = open_graphics(&d, Some(decode)).unwrap();
    assert_eq!(doc.facts.images, 1);
    assert_eq!(
        doc.facts.not_drawn,
        vec![("embedded object".into(), 1), ("image".into(), 1)]
    );
    let (w, _, b) = render(&doc, 0);
    assert_eq!(px(&b, w, 36, 36), [10, 200, 30]);
    match thumbnail(&d).unwrap() {
        Thumbnail::Range(r) => assert_eq!(&d[r], &png[..]),
        Thumbnail::Owned(_) => panic!("stored member"),
    }
    let none = package("<draw:page/>", &[]);
    assert_eq!(
        thumbnail(&none).unwrap_err().to_string(),
        "OpenDocument file without a thumbnail (saved with thumbnails off)"
    );
}

#[test]
fn kinds_and_refusals() {
    let text = zip(&[(
        "mimetype",
        b"application/vnd.oasis.opendocument.text-template",
    )]);
    assert_eq!(probe_file(&text), Some(Kind::Text));
    assert!(matches!(
        open_graphics(&text, None),
        Err(Error::Unsupported(_))
    ));
    let calc = zip(&[(
        "mimetype",
        b"application/vnd.oasis.opendocument.spreadsheet",
    )]);
    assert_eq!(probe_file(&calc), Some(Kind::Spreadsheet));
    let chart = zip(&[("mimetype", b"application/vnd.oasis.opendocument.chart")]);
    assert_eq!(probe_file(&chart), Some(Kind::Other("chart".into())));
    assert_eq!(probe_file(b"PK\x03\x04garbage"), None);
    assert_eq!(probe_file(b"hello"), None);
    let enc = zip(&[
        ("mimetype", b"application/vnd.oasis.opendocument.graphics"),
        (
            "META-INF/manifest.xml",
            b"<manifest:manifest><manifest:file-entry><manifest:encryption-data/></manifest:file-entry></manifest:manifest>",
        ),
    ]);
    assert_eq!(open_graphics(&enc, None).err(), Some(Error::Encrypted));
    assert_eq!(
        Error::Encrypted.to_string(),
        "password-protected OpenDocument file: not supported"
    );
    let nocontent = zip(&[("mimetype", b"application/vnd.oasis.opendocument.graphics")]);
    assert!(matches!(
        open_graphics(&nocontent, None),
        Err(Error::Malformed(_))
    ));
    // Garbage XML.
    let bad = zip(&[
        ("mimetype", b"application/vnd.oasis.opendocument.graphics"),
        ("content.xml", b"<office:document-content><unclosed"),
    ]);
    assert!(matches!(
        open_graphics(&bad, None),
        Err(Error::Malformed(_))
    ));
}

#[test]
fn flat_xml() {
    let doc = format!(
        r##"<?xml version="1.0" encoding="UTF-8"?><office:document {NS} office:mimetype="application/vnd.oasis.opendocument.graphics"><office:meta><dc:title>Flat</dc:title></office:meta><office:automatic-styles><style:style style:name="red" style:family="graphic"><style:graphic-properties draw:fill="solid" draw:fill-color="#ff0000"/></style:style></office:automatic-styles><office:body><office:drawing><draw:page draw:name="P"><draw:rect draw:style-name="red" svg:x="0cm" svg:y="0cm" svg:width="2cm" svg:height="2cm"/></draw:page></office:drawing></office:body></office:document>"##
    );
    let d = doc.as_bytes();
    assert_eq!(probe_file(d), Some(Kind::Graphics));
    let doc = open_graphics(d, None).unwrap();
    assert!(doc.facts.flat);
    assert_eq!(doc.facts.title.as_deref(), Some("Flat"));
    assert_eq!(doc.pages[0].size, A4);
    assert!(doc.pages[0].ops.iter().any(|o| matches!(
        o,
        Op::Fill {
            paint: Paint::Solid([255, 0, 0, 255]),
            ..
        }
    )));
    assert!(thumbnail(d).is_err());
}

#[test]
fn hostile_nesting_and_counts() {
    // Deep groups stop at the depth limit; many shapes stop at the shape limit.
    let mut body = String::from(PAGE);
    for _ in 0..200 {
        body.push_str("<draw:g>");
    }
    body.push_str(r#"<draw:rect svg:width="1cm" svg:height="1cm"/>"#);
    for _ in 0..200 {
        body.push_str("</draw:g>");
    }
    body.push_str("</draw:page>");
    let doc = open_graphics(&package(&body, &[]), None).unwrap();
    assert!(doc.facts.limited);
    // A custom shape with a pathological path and equations.
    let body = format!(
        r#"{PAGE}<draw:custom-shape svg:width="1cm" svg:height="1cm"><draw:enhanced-geometry draw:enhanced-path="M ?f0 ?f0 L 1e308 -1e308 C ?f0 ?f0 A B W 1 2 3 4 5 6 7 8 T 0 0 0 0 0 1e9 Q"><draw:equation draw:name="f0" draw:formula="?f0 *(((((((((("/></draw:enhanced-geometry></draw:custom-shape></draw:page>"#
    );
    let doc = open_graphics(&package(&body, &[]), None).unwrap();
    let _ = render(&doc, 0);
}

#[test]
fn tables_lists_and_fit() {
    let body = format!(
        r##"{PAGE}<draw:frame svg:x="0in" svg:y="0in" svg:width="1in" svg:height="1in"><table:table xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0"><table:table-column/><table:table-column/>
<table:table-row><table:table-cell table:style-name="cr"><text:p>a</text:p></table:table-cell><table:covered-table-cell/></table:table-row>
<table:table-row><table:table-cell table:number-columns-spanned="2"><text:p>b</text:p></table:table-cell></table:table-row></table:table></draw:frame>
<draw:frame draw:style-name="fit" svg:x="1in" svg:y="0in" svg:width="1in" svg:height="0.25in"><draw:text-box><text:p>stretched to fit</text:p></draw:text-box></draw:frame>
<draw:frame svg:x="1in" svg:y="0.5in" svg:width="1in" svg:height="0.5in"><draw:text-box><text:list text:style-name="num"><text:list-item><text:p>one</text:p></text:list-item><text:list-item><text:p>two</text:p></text:list-item></text:list></draw:text-box></draw:frame>
</draw:page>"##
    );
    let extra = r##"<style:style style:name="cr" style:family="table-cell"><style:graphic-properties draw:fill="solid" draw:fill-color="#ff0000"/><style:paragraph-properties fo:border="1pt solid #000000"/></style:style>
<style:style style:name="fit" style:family="graphic"><style:graphic-properties draw:fit-to-size="true" draw:fill="none"/></style:style>
<text:list-style style:name="num"><text:list-level-style-number text:level="1" style:num-format="i" style:num-suffix="."><style:list-level-properties text:min-label-width="0.5in"/></text:list-level-style-number></text:list-style>"##;
    let c = content_xml(&body).replace(
        "</office:automatic-styles>",
        &format!("{extra}</office:automatic-styles>"),
    );
    let s = styles_xml();
    let m = meta_xml();
    let d = zip(&[
        ("mimetype", b"application/vnd.oasis.opendocument.graphics"),
        ("styles.xml", s.as_bytes()),
        ("content.xml", c.as_bytes()),
        ("meta.xml", m.as_bytes()),
    ]);
    let doc = open_graphics(&d, None).unwrap();
    let ops = &doc.pages[0].ops;
    // The red cell (top half of the table) with its border; text boxes for the cells, the
    // fitted frame and the list.
    let (w, _, b) = render(&doc, 0);
    assert_eq!(px(&b, w, 18, 9), [255, 0, 0]);
    assert!(
        ops.iter()
            .any(|o| matches!(o, Op::Stroke { stroke, .. } if (stroke.width - 1.0).abs() < 1e-9))
    );
    let texts: Vec<&specular_drawpage::TextBlock> = ops
        .iter()
        .filter_map(|o| match o {
            Op::Text(t) => Some(&**t),
            _ => None,
        })
        .collect();
    assert_eq!(texts.len(), 4);
    // Fit to frame: unwrapped and scaled to the box.
    let fit = texts[2];
    assert!(!fit.wrap);
    let scaled = fit
        .transform
        .transform_rect_bbox(specular_drawpage::Rect::new(
            0.0, 0.0, fit.width, fit.height,
        ));
    assert!(
        (scaled.width() - 72.0).abs() < 0.5 && (scaled.height() - 18.0).abs() < 0.5,
        "{scaled:?}"
    );
    // Roman numbered list: label "ii" then "." spaced to the label width.
    let list = texts[3];
    assert_eq!(list.paragraphs[1].runs[0].text, "ii");
    assert_eq!(list.paragraphs[1].runs[1].text, ".");
    assert!((list.paragraphs[1].indent_left - 36.0).abs() < 1e-9);
}
