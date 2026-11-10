//! PDF output: one page the figure's size at a chosen resolution, the pictures as image
//! XObjects (16 bits per sample when the figure is deep, Flate), every item a path in the
//! page's content in figure px (the page's matrix maps px to points), alpha through ExtGState.
//!
//! Each item is marked content: text as `/Span` with `/ActualText` (so it can be searched and
//! copied although it is drawn as outlines), the others tagged with their role and facts
//! (`/scale-bar <</length (50) /unit (µm) /length-px (204.8)>> BDC`). The provenance is in the
//! document information dictionary (`/Specular-<key>`) and in an XMP packet.

use std::fmt::Write as _;

use crate::{El, Figure, Item, Role, xml_escape};

/// How the page is written.
#[derive(Clone, Debug)]
pub struct Options<'a> {
    /// Figure px per inch (the page is `width / dpi` inches wide).
    pub dpi: f64,
    /// An ICC profile for the pictures' colour space (sRGB); `DeviceRGB` without.
    pub icc: Option<&'a [u8]>,
    /// The program, e.g. `Specular 0.1.0`.
    pub producer: String,
    /// A PDF date (`D:20261024120000Z`), when known.
    pub created: Option<String>,
}

impl Default for Options<'_> {
    fn default() -> Self {
        Options {
            dpi: 300.0,
            icc: None,
            producer: "Specular".into(),
            created: None,
        }
    }
}

/// A number without trailing zeros, to 1/1000.
fn n(v: f32) -> String {
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.').to_owned();
    if s == "-0" { "0".into() } else { s }
}

/// A PDF text string: plain when printable ASCII, else UTF-16BE hex with a byte-order mark.
pub fn string(s: &str) -> String {
    if s.bytes().all(|b| (0x20..0x7f).contains(&b)) {
        let mut o = String::from("(");
        for c in s.chars() {
            if matches!(c, '(' | ')' | '\\') {
                o.push('\\');
            }
            o.push(c);
        }
        o.push(')');
        o
    } else {
        let mut o = String::from("<FEFF");
        for u in s.encode_utf16() {
            let _ = write!(o, "{u:04X}");
        }
        o.push('>');
        o
    }
}

/// Decodes what [`string`] wrote (for readers of the file and tests).
pub fn parse_string(s: &str) -> Option<String> {
    let s = s.trim();
    if let Some(h) = s.strip_prefix("<FEFF").and_then(|h| h.strip_suffix('>')) {
        let units: Option<Vec<u16>> = (0..h.len() / 4)
            .map(|i| u16::from_str_radix(&h[i * 4..i * 4 + 4], 16).ok())
            .collect();
        return String::from_utf16(&units?).ok();
    }
    let inner = s.strip_prefix('(')?.strip_suffix(')')?;
    let mut o = String::new();
    let mut esc = false;
    for c in inner.chars() {
        if esc {
            o.push(c);
            esc = false;
        } else if c == '\\' {
            esc = true;
        } else {
            o.push(c);
        }
    }
    Some(o)
}

/// A name from a key: letters, digits and `-` kept, the rest `-`.
fn name(k: &str) -> String {
    k.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

fn deflate(data: &[u8]) -> Vec<u8> {
    miniz_oxide::deflate::compress_to_vec_zlib(data, 6)
}

fn path_ops(o: &mut String, p: &[El]) {
    let mut cur = [0.0f32; 2];
    for e in p {
        let _ = match *e {
            El::Move(a) => {
                cur = a;
                writeln!(o, "{} {} m", n(a[0]), n(a[1]))
            }
            El::Line(a) => {
                cur = a;
                writeln!(o, "{} {} l", n(a[0]), n(a[1]))
            }
            El::Quad(c, a) => {
                // As a cubic: control points two thirds of the way to the quadratic's.
                let c1 = [
                    cur[0] + (c[0] - cur[0]) * 2.0 / 3.0,
                    cur[1] + (c[1] - cur[1]) * 2.0 / 3.0,
                ];
                let c2 = [
                    a[0] + (c[0] - a[0]) * 2.0 / 3.0,
                    a[1] + (c[1] - a[1]) * 2.0 / 3.0,
                ];
                cur = a;
                writeln!(
                    o,
                    "{} {} {} {} {} {} c",
                    n(c1[0]),
                    n(c1[1]),
                    n(c2[0]),
                    n(c2[1]),
                    n(a[0]),
                    n(a[1])
                )
            }
            El::Cubic(c1, c2, a) => {
                cur = a;
                writeln!(
                    o,
                    "{} {} {} {} {} {} c",
                    n(c1[0]),
                    n(c1[1]),
                    n(c2[0]),
                    n(c2[1]),
                    n(a[0]),
                    n(a[1])
                )
            }
            El::Close => writeln!(o, "h"),
        };
    }
}

fn rgb(c: crate::Rgba) -> String {
    format!(
        "{} {} {}",
        n(f32::from(c.r) / 255.0),
        n(f32::from(c.g) / 255.0),
        n(f32::from(c.b) / 255.0)
    )
}

/// The marked-content opening for an item.
fn mark(it: &Item) -> String {
    match (&it.text, it.role) {
        (Some(t), _) => format!("/Span <</ActualText {}>> BDC\n", string(t)),
        (None, Role::Plain) => String::new(),
        (None, r) => {
            let mut d = String::new();
            for (k, v) in &it.data {
                let _ = write!(d, "/{} {} ", name(k), string(v));
            }
            format!("/{} <<{}>> BDC\n", r.id(), d.trim_end())
        }
    }
}

/// The XMP packet: title, producer and the provenance in [`crate::svg::NS`].
pub fn xmp(f: &Figure, producer: &str) -> String {
    let mut o = String::new();
    o.push_str("<?xpacket begin=\"\u{FEFF}\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>\n");
    o.push_str("<x:xmpmeta xmlns:x=\"adobe:ns:meta/\">\n");
    o.push_str("<rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n");
    let _ = writeln!(
        o,
        "<rdf:Description rdf:about=\"\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\" \
         xmlns:pdf=\"http://ns.adobe.com/pdf/1.3/\" xmlns:specular=\"{}\">",
        crate::svg::NS
    );
    let _ = writeln!(
        o,
        "<dc:title><rdf:Alt><rdf:li xml:lang=\"x-default\">{}</rdf:li></rdf:Alt></dc:title>",
        xml_escape(&f.title)
    );
    let _ = writeln!(o, "<pdf:Producer>{}</pdf:Producer>", xml_escape(producer));
    for (k, v) in &f.meta {
        let k = name(k);
        let k = if k.starts_with(|c: char| c.is_ascii_digit()) {
            format!("k{k}")
        } else {
            k
        };
        let _ = writeln!(o, "<specular:{k}>{}</specular:{k}>", xml_escape(v));
    }
    o.push_str("</rdf:Description>\n</rdf:RDF>\n</x:xmpmeta>\n<?xpacket end=\"w\"?>");
    o
}

/// The figure as a one-page PDF.
pub fn write(f: &Figure, opts: &Options<'_>) -> Result<Vec<u8>, String> {
    let k = 72.0 / opts.dpi.max(1.0);
    let (wpt, hpt) = (f64::from(f.width) * k, f64::from(f.height) * k);
    // Objects: 1 catalog, 2 pages, 3 page, 4 content, 5 metadata, 6 info, 7 ICC (when
    // given), then the images.
    let mut objs: Vec<Vec<u8>> = Vec::new();
    let has_icc = opts.icc.is_some();
    let first_image = if has_icc { 8 } else { 7 };
    let cs = if has_icc {
        "[/ICCBased 7 0 R]".to_owned()
    } else {
        "/DeviceRGB".to_owned()
    };

    // Content.
    let mut c = String::new();
    let _ = writeln!(
        c,
        "q\n{} 0 0 {} 0 {} cm",
        n(k as f32),
        n(-k as f32),
        n(hpt as f32)
    );
    let _ = writeln!(
        c,
        "{} rg\n0 0 {} {} re f",
        rgb(f.background),
        f.width,
        f.height
    );
    let mut images = Vec::new();
    for (i, p) in f.pictures.iter().enumerate() {
        let _ = writeln!(
            c,
            "/picture <</Name {}>> BDC\nq {} 0 0 {} {} {} cm /Im{i} Do Q\nEMC",
            string(&p.name),
            n(p.w),
            n(-p.h),
            n(p.x),
            n(p.y + p.h)
        );
        let bits = if f.deep { 16 } else { 8 };
        let raw = if f.deep {
            p.px.to_be_bytes()
        } else {
            p.px.to_rgb8()
        };
        let data = deflate(&raw);
        let mut o = format!(
            "<</Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace {cs} \
             /BitsPerComponent {bits} /Filter /FlateDecode /Length {}{}>>\nstream\n",
            p.px.width,
            p.px.height,
            data.len(),
            if p.smooth { " /Interpolate true" } else { "" }
        )
        .into_bytes();
        o.extend_from_slice(&data);
        o.extend_from_slice(b"\nendstream");
        images.push(o);
    }
    let mut states: Vec<(String, String)> = Vec::new();
    let mut state = |fill: f32, stroke: f32| -> String {
        let key = format!("/ca {} /CA {}", n(fill), n(stroke));
        if let Some(i) = states.iter().position(|(k, _)| *k == key) {
            return states[i].1.clone();
        }
        let id = format!("G{}", states.len());
        states.push((key, id.clone()));
        id
    };
    for it in &f.items {
        if it.path.is_empty() || (it.fill.is_none() && it.stroke.is_none()) {
            continue;
        }
        c.push_str(&mark(it));
        c.push_str("q\n");
        let fa = it.fill.map_or(1.0, |c| c.a);
        let sa = it.stroke.map_or(1.0, |s| s.colour.a);
        if fa < 1.0 || sa < 1.0 {
            let _ = writeln!(c, "/{} gs", state(fa, sa));
        }
        if let Some(fc) = it.fill {
            let _ = writeln!(c, "{} rg", rgb(fc));
        }
        if let Some(s) = it.stroke {
            let _ = writeln!(c, "{} RG\n{} w\n1 j 1 J", rgb(s.colour), n(s.width));
        }
        path_ops(&mut c, &it.path);
        c.push_str(match (it.fill.is_some(), it.stroke.is_some()) {
            (true, true) => "B\n",
            (true, false) => "f\n",
            _ => "S\n",
        });
        c.push_str("Q\n");
        if it.text.is_some() || it.role != Role::Plain {
            c.push_str("EMC\n");
        }
    }
    c.push_str("Q\n");

    // 1 catalog, 2 pages, 3 page.
    objs.push(b"<</Type /Catalog /Pages 2 0 R /Metadata 5 0 R>>".to_vec());
    objs.push(b"<</Type /Pages /Kids [3 0 R] /Count 1>>".to_vec());
    let mut res = String::from("/XObject <<");
    for i in 0..images.len() {
        let _ = write!(res, " /Im{i} {} 0 R", first_image + i);
    }
    res.push_str(" >>");
    if !states.is_empty() {
        res.push_str(" /ExtGState <<");
        for (k, id) in &states {
            let _ = write!(res, " /{id} <<{k}>>");
        }
        res.push_str(" >>");
    }
    objs.push(
        format!(
            "<</Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Resources <<{res}>> \
             /Contents 4 0 R>>",
            n(wpt as f32),
            n(hpt as f32)
        )
        .into_bytes(),
    );
    // 4 content.
    let z = deflate(c.as_bytes());
    let mut o = format!("<</Filter /FlateDecode /Length {}>>\nstream\n", z.len()).into_bytes();
    o.extend_from_slice(&z);
    o.extend_from_slice(b"\nendstream");
    objs.push(o);
    // 5 metadata.
    let x = xmp(f, &opts.producer);
    let mut o = format!(
        "<</Type /Metadata /Subtype /XML /Length {}>>\nstream\n",
        x.len()
    )
    .into_bytes();
    o.extend_from_slice(x.as_bytes());
    o.extend_from_slice(b"\nendstream");
    objs.push(o);
    // 6 info.
    let mut info = format!(
        "<</Title {} /Producer {} /Creator (Specular)",
        string(&f.title),
        string(&opts.producer)
    );
    if let Some(d) = &opts.created {
        let _ = write!(info, " /CreationDate {}", string(d));
    }
    for (k, v) in &f.meta {
        let _ = write!(info, " /Specular-{} {}", name(k), string(v));
    }
    info.push_str(">>");
    objs.push(info.into_bytes());
    // 7 ICC.
    if let Some(icc) = opts.icc {
        let z = deflate(icc);
        let mut o = format!(
            "<</N 3 /Filter /FlateDecode /Length {}>>\nstream\n",
            z.len()
        )
        .into_bytes();
        o.extend_from_slice(&z);
        o.extend_from_slice(b"\nendstream");
        objs.push(o);
    }
    objs.extend(images);

    let mut out = b"%PDF-1.7\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets = Vec::with_capacity(objs.len());
    for (i, ob) in objs.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
        out.extend_from_slice(ob);
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref = out.len();
    let _ = (|| -> std::fmt::Result {
        let mut t = String::new();
        writeln!(t, "xref\n0 {}", objs.len() + 1)?;
        t.push_str("0000000000 65535 f \n");
        for off in &offsets {
            writeln!(t, "{off:010} 00000 n ")?;
        }
        writeln!(
            t,
            "trailer\n<</Size {} /Root 1 0 R /Info 6 0 R>>\nstartxref\n{xref}\n%%EOF",
            objs.len() + 1
        )?;
        out.extend_from_slice(t.as_bytes());
        Ok(())
    })();
    Ok(out)
}

/// The decompressed content stream of a PDF [`write`] made (for tests and readers that want
/// the marked content).
pub fn content(pdf: &[u8]) -> Option<String> {
    let at = find(pdf, b"4 0 obj\n")?;
    let rest = &pdf[at..];
    let s = find(rest, b"stream\n")? + 7;
    let e = find(rest, b"\nendstream")?;
    let raw = miniz_oxide::inflate::decompress_to_vec_zlib(&rest[s..e]).ok()?;
    String::from_utf8(raw).ok()
}

fn find(h: &[u8], n: &[u8]) -> Option<usize> {
    h.windows(n.len()).position(|w| w == n)
}
