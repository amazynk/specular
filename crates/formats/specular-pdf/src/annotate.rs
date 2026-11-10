//! Adds annotations to a page of an existing PDF as an incremental update (ISO 32000-1
//! §7.5.6): the original bytes are kept as they are and the new objects, the page with its
//! new `/Annots`, a cross-reference section and a trailer are appended. The page's content is
//! untouched, so it stays vector at its own size and scale.
//!
//! Each [`Annot`] brings its dictionary entries and its normal appearance (a form XObject
//! whose content is drawn in user space; `/BBox` is the annotation's `/Rect`, the matrix is
//! the identity), so every reader, hayro included, draws it the same way. Appearances may use
//! Helvetica as `/Helv` and Helvetica-Bold as `/HeBo` (standard fonts, WinAnsi).
//!
//! The update's cross-reference section has the original's kind: a table after a table, a
//! stream after a stream. Encrypted files are refused (new strings would have to be
//! encrypted), as are files whose last cross-reference section cannot be found (a damaged
//! file only a repairing reader opens).

use std::fmt::Write as _;

use hayro::hayro_syntax::Pdf;
use hayro::hayro_syntax::object::{Array, Dict};

/// One annotation to add.
#[derive(Clone, Debug, PartialEq)]
pub struct Annot {
    /// Dictionary entries other than `/Type`, `/Rect`, `/P` and `/AP`
    /// (e.g. `/Subtype /Square /C [1 0 0]`).
    pub entries: String,
    /// `/Rect` in user space: x0, y0, x1, y1.
    pub rect: [f64; 4],
    /// The normal appearance's content stream (user space).
    pub appearance: Vec<u8>,
}

/// Why annotations could not be added.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AnnotateError {
    #[error("the PDF is encrypted")]
    Encrypted,
    #[error("the PDF's structure is damaged")]
    Damaged,
    #[error("the PDF has no page {0}")]
    NoPage(usize),
}

/// `data` with `annots` added to page `page` (zero-based).
pub fn add_annotations(
    data: &[u8],
    page: usize,
    annots: &[Annot],
) -> Result<Vec<u8>, AnnotateError> {
    let trailer = Trailer::find(data).ok_or(AnnotateError::Damaged)?;
    if trailer.encrypt {
        return Err(AnnotateError::Encrypted);
    }
    let pdf = Pdf::new(data.to_vec()).map_err(|_| AnnotateError::Damaged)?;
    let pg = pdf.pages().get(page).ok_or(AnnotateError::NoPage(page))?;
    let dict = pg.raw();
    let id = dict.obj_id().ok_or(AnnotateError::Damaged)?;
    let (page_num, page_gen) = (id.obj_number, id.gen_number);
    if page_num <= 0 || page_gen < 0 {
        return Err(AnnotateError::Damaged);
    }
    let page_num = page_num as u32;
    let entries = dict_entries(dict.data()).ok_or(AnnotateError::Damaged)?;
    // The annotations already there: a direct array's items, or the array a reference
    // points to.
    let mut old = String::new();
    if let Some(a) = dict.get::<Array<'_>>(b"Annots".as_slice()) {
        let raw = a.data();
        let inner = raw
            .strip_prefix(b"[")
            .and_then(|r| r.strip_suffix(b"]"))
            .unwrap_or(raw);
        old = String::from_utf8_lossy(inner).trim().to_owned();
    }

    let mut out = data.to_vec();
    if !out.ends_with(b"\n") {
        out.push(b'\n');
    }
    let first = trailer.size.max(page_num + 1);
    let mut next = first;
    let mut objects: Vec<(u32, u16, usize)> = Vec::new();
    let mut put = |out: &mut Vec<u8>, num: u32, generation: u16, body: &[u8]| {
        objects.push((num, generation, out.len()));
        out.extend_from_slice(format!("{num} {generation} obj\n").as_bytes());
        out.extend_from_slice(body);
        out.extend_from_slice(b"\nendobj\n");
    };
    let helv = next;
    let bold = next + 1;
    next += 2;
    put(
        &mut out,
        helv,
        0,
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>",
    );
    put(
        &mut out,
        bold,
        0,
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>",
    );
    let mut refs = Vec::new();
    for a in annots {
        let [x0, y0, x1, y1] = a.rect;
        let rect = format!("[{} {} {} {}]", num(x0), num(y0), num(x1), num(y1));
        let ap = next;
        let an = next + 1;
        next += 2;
        let mut body = format!(
            "<< /Type /XObject /Subtype /Form /BBox {rect} /Resources << /Font << /Helv {helv} 0 R \
             /HeBo {bold} 0 R >> >> /Length {} >>\nstream\n",
            a.appearance.len()
        )
        .into_bytes();
        body.extend_from_slice(&a.appearance);
        body.extend_from_slice(b"\nendstream");
        put(&mut out, ap, 0, &body);
        let body = format!(
            "<< /Type /Annot {} /Rect {rect} /P {page_num} {page_gen} R /AP << /N {ap} 0 R >> >>",
            a.entries
        );
        put(&mut out, an, 0, body.as_bytes());
        refs.push(format!("{an} 0 R"));
    }
    // The page again, with the annotations added.
    let mut page_body = String::from("<<");
    for (k, v) in &entries {
        if k == "/Annots" {
            continue;
        }
        let _ = write!(page_body, " {k} {v}");
    }
    let mut all = old;
    for r in &refs {
        if !all.is_empty() {
            all.push(' ');
        }
        all.push_str(r);
    }
    let _ = write!(page_body, " /Annots [{all}] >>");
    put(
        &mut out,
        page_num,
        u16::try_from(page_gen).map_err(|_| AnnotateError::Damaged)?,
        page_body.as_bytes(),
    );

    let mut extra = String::new();
    for (k, v) in [
        ("/Root", &trailer.root),
        ("/Info", &trailer.info),
        ("/ID", &trailer.id),
    ] {
        if let Some(v) = v {
            let _ = write!(extra, " {k} {v}");
        }
    }
    if trailer.stream {
        // A cross-reference stream: one more object, itself listed.
        let xref_num = next;
        let size = next + 1;
        objects.push((xref_num, 0, out.len()));
        objects.sort_by_key(|o| o.0);
        let mut rows = Vec::new();
        let mut index = String::new();
        for run in runs(&objects) {
            let _ = write!(index, " {} {}", run[0].0, run.len());
            for &(_, generation, off) in run {
                rows.push(1u8);
                rows.extend_from_slice(&(off as u32).to_be_bytes());
                rows.extend_from_slice(&generation.to_be_bytes());
            }
        }
        if out.len() > u32::MAX as usize {
            return Err(AnnotateError::Damaged);
        }
        let xref_at = out.len();
        let mut body = format!(
            "{xref_num} 0 obj\n<< /Type /XRef /Size {size} /Index [{}] /W [1 4 2] /Prev {}{extra} \
             /Length {} >>\nstream\n",
            index.trim(),
            trailer.xref,
            rows.len()
        )
        .into_bytes();
        body.extend_from_slice(&rows);
        body.extend_from_slice(b"\nendstream\nendobj\n");
        out.extend_from_slice(&body);
        out.extend_from_slice(format!("startxref\n{xref_at}\n%%EOF\n").as_bytes());
    } else {
        objects.sort_by_key(|o| o.0);
        let xref_at = out.len();
        out.extend_from_slice(b"xref\n");
        for run in runs(&objects) {
            out.extend_from_slice(format!("{} {}\n", run[0].0, run.len()).as_bytes());
            for &(_, generation, off) in run {
                out.extend_from_slice(format!("{off:010} {generation:05} n \n").as_bytes());
            }
        }
        out.extend_from_slice(
            format!(
                "trailer\n<< /Size {next}{extra} /Prev {} >>\nstartxref\n{xref_at}\n%%EOF\n",
                trailer.xref
            )
            .as_bytes(),
        );
    }
    Ok(out)
}

/// Runs of consecutive object numbers (cross-reference subsections).
fn runs(objects: &[(u32, u16, usize)]) -> Vec<&[(u32, u16, usize)]> {
    let mut out = Vec::new();
    let mut start = 0;
    for i in 1..=objects.len() {
        if i == objects.len() || objects[i].0 != objects[i - 1].0 + 1 {
            out.push(&objects[start..i]);
            start = i;
        }
    }
    out
}

/// A number as PDF writes it: at most four decimals, no exponent.
pub fn num(v: f64) -> String {
    if !v.is_finite() {
        return "0".into();
    }
    let s = format!("{v:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.into() }
}

/// What the update needs from the last trailer.
#[derive(Debug, Default)]
struct Trailer {
    /// Offset of the last cross-reference section.
    xref: usize,
    /// It is a stream.
    stream: bool,
    size: u32,
    root: Option<String>,
    info: Option<String>,
    id: Option<String>,
    encrypt: bool,
}

impl Trailer {
    fn find(data: &[u8]) -> Option<Trailer> {
        let tail_from = data.len().saturating_sub(4096);
        let tail = &data[tail_from..];
        let at = tail.windows(9).rposition(|w| w == b"startxref")? + tail_from + 9;
        let digits: String = data[at..]
            .iter()
            .skip_while(|b| b.is_ascii_whitespace())
            .take_while(|b| b.is_ascii_digit())
            .map(|&b| b as char)
            .collect();
        let xref: usize = digits.parse().ok()?;
        let mut i = skip_ws(data, xref);
        let stream = !data.get(i..)?.starts_with(b"xref");
        if stream {
            // `n g obj << … >>`
            for _ in 0..2 {
                i = skip_object(data, i)?;
                i = skip_ws(data, i);
            }
            if !data.get(i..)?.starts_with(b"obj") {
                return None;
            }
            i = skip_ws(data, i + 3);
        } else {
            let from = i;
            let k = data[from..].windows(7).position(|w| w == b"trailer")? + from;
            i = skip_ws(data, k + 7);
        }
        let end = skip_object(data, i)?;
        let entries = dict_entries(&data[i..end])?;
        let mut t = Trailer {
            xref,
            stream,
            ..Trailer::default()
        };
        for (k, v) in entries {
            match k.as_str() {
                "/Size" => t.size = v.trim().parse().ok()?,
                "/Root" => t.root = Some(v),
                "/Info" => t.info = Some(v),
                "/ID" => t.id = Some(v),
                "/Encrypt" => t.encrypt = true,
                _ => {}
            }
        }
        (t.size > 0 && t.root.is_some()).then_some(t)
    }
}

fn is_delim(b: u8) -> bool {
    matches!(
        b,
        b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%'
    )
}

/// Past white space and comments.
fn skip_ws(d: &[u8], mut i: usize) -> usize {
    while i < d.len() {
        if d[i].is_ascii_whitespace() || d[i] == 0 {
            i += 1;
        } else if d[i] == b'%' {
            while i < d.len() && d[i] != b'\n' && d[i] != b'\r' {
                i += 1;
            }
        } else {
            break;
        }
    }
    i
}

/// Past a regular token (a number, a keyword or a name's characters).
fn skip_regular(d: &[u8], mut i: usize) -> usize {
    while i < d.len() && !d[i].is_ascii_whitespace() && d[i] != 0 && !is_delim(d[i]) {
        i += 1;
    }
    i
}

/// Past one object starting at or after `i` (a reference `n g R` counts as one).
fn skip_object(d: &[u8], i: usize) -> Option<usize> {
    let i = skip_ws(d, i);
    let c = *d.get(i)?;
    match c {
        b'/' => Some(skip_regular(d, i + 1)),
        b'(' => {
            let (mut depth, mut j) = (0usize, i);
            while j < d.len() {
                match d[j] {
                    b'\\' => j += 1,
                    b'(' => depth += 1,
                    b')' => {
                        depth -= 1;
                        if depth == 0 {
                            return Some(j + 1);
                        }
                    }
                    _ => {}
                }
                j += 1;
            }
            None
        }
        b'<' if d.get(i + 1) == Some(&b'<') => {
            let mut j = i + 2;
            loop {
                j = skip_ws(d, j);
                if d.get(j..)?.starts_with(b">>") {
                    return Some(j + 2);
                }
                j = skip_object(d, j)?;
            }
        }
        b'<' => Some(i + d[i..].iter().position(|&b| b == b'>')? + 1),
        b'[' => {
            let mut j = i + 1;
            loop {
                j = skip_ws(d, j);
                if *d.get(j)? == b']' {
                    return Some(j + 1);
                }
                j = skip_object(d, j)?;
            }
        }
        b'0'..=b'9' | b'+' | b'-' | b'.' => {
            let end = skip_regular(d, i);
            // `n g R`
            let is_int = |s: &[u8]| !s.is_empty() && s.iter().all(u8::is_ascii_digit);
            if is_int(&d[i..end]) {
                let g0 = skip_ws(d, end);
                let g1 = skip_regular(d, g0);
                if g1 > g0 && is_int(&d[g0..g1]) {
                    let r0 = skip_ws(d, g1);
                    if d.get(r0) == Some(&b'R')
                        && d.get(r0 + 1)
                            .is_none_or(|&b| b.is_ascii_whitespace() || is_delim(b))
                    {
                        return Some(r0 + 1);
                    }
                }
            }
            Some(end)
        }
        _ if !is_delim(c) => Some(skip_regular(d, i)),
        _ => None,
    }
}

/// The entries of the dictionary `d` (`<< … >>`) as key and raw value text.
fn dict_entries(d: &[u8]) -> Option<Vec<(String, String)>> {
    let mut i = skip_ws(d, 0);
    if !d.get(i..)?.starts_with(b"<<") {
        return None;
    }
    i += 2;
    let mut out = Vec::new();
    loop {
        i = skip_ws(d, i);
        if d.get(i..)?.starts_with(b">>") {
            return Some(out);
        }
        if d.get(i) != Some(&b'/') {
            return None;
        }
        let k_end = skip_regular(d, i + 1);
        let key = String::from_utf8_lossy(&d[i..k_end]).into_owned();
        let v0 = skip_ws(d, k_end);
        let v1 = skip_object(d, v0)?;
        out.push((key, String::from_utf8_lossy(&d[v0..v1]).into_owned()));
        i = v1;
    }
}

/// Whether `dict` is the page dictionary with `n` annotations (for tests).
#[doc(hidden)]
pub fn annotation_count(data: &[u8], page: usize) -> Option<usize> {
    let pdf = Pdf::new(data.to_vec()).ok()?;
    let pg = pdf.pages().get(page)?;
    let d: &Dict<'_> = pg.raw();
    Some(
        d.get::<Array<'_>>(b"Annots".as_slice())
            .map_or(0, |a| a.raw_iter().count()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture;

    fn square() -> Annot {
        Annot {
            entries: "/Subtype /Square /C [1 0 0]".into(),
            rect: [10.0, 10.0, 60.0, 40.0],
            appearance: b"1 0 0 RG 2 w 11 11 48 28 re S".to_vec(),
        }
    }

    #[test]
    fn lexes_dictionaries() {
        let e = dict_entries(
            b"<< /Type /Page /Kids [1 0 R 2 0 R] /S (a\\)b(c)) /N 12 0 R /X <ab> /D << /A 1 >> /F 1.5 >>",
        )
        .unwrap();
        let keys: Vec<_> = e.iter().map(|(k, v)| format!("{k}={v}")).collect();
        assert_eq!(
            keys,
            [
                "/Type=/Page",
                "/Kids=[1 0 R 2 0 R]",
                "/S=(a\\)b(c))",
                "/N=12 0 R",
                "/X=<ab>",
                "/D=<< /A 1 >>",
                "/F=1.5"
            ]
        );
    }

    #[test]
    fn adds_to_a_table_file_and_keeps_old_annotations() {
        let page = fixture::Page {
            width: 200.0,
            height: 100.0,
            content: "0 0 1 rg 0 0 5 5 re f",
        };
        let src = fixture::build(&[page], "", Some("/Title (t)"), "", &[]);
        let once = add_annotations(&src, 0, &[square()]).unwrap();
        assert!(once.starts_with(&src));
        assert_eq!(annotation_count(&once, 0), Some(1));
        let twice = add_annotations(&once, 0, &[square(), square()]).unwrap();
        assert_eq!(annotation_count(&twice, 0), Some(3));
        // Still the same page, info and size.
        let pdf = Pdf::new(twice.clone()).unwrap();
        assert_eq!(pdf.pages().len(), 1);
        let doc = crate::Document::open(std::sync::Arc::new(twice)).unwrap();
        assert_eq!(doc.info().title.as_deref(), Some("t"));
    }

    #[test]
    fn adds_to_a_stream_file() {
        // The same fixture rewritten with a cross-reference stream.
        let page = fixture::Page {
            width: 200.0,
            height: 100.0,
            content: "",
        };
        let src = fixture::build(&[page], "", None, "", &[]);
        let at = src.windows(4).rposition(|w| w == b"xref").unwrap();
        let mut s = src[..at].to_vec();
        let offsets: Vec<usize> = (1..=4)
            .map(|n| {
                let needle = format!("{n} 0 obj");
                s.windows(needle.len())
                    .position(|w| w == needle.as_bytes())
                    .unwrap()
            })
            .collect();
        let mut rows = vec![0u8, 0, 0, 0, 0, 0xff, 0xff];
        for o in &offsets {
            rows.push(1);
            rows.extend_from_slice(&(*o as u32).to_be_bytes());
            rows.extend_from_slice(&[0, 0]);
        }
        rows.extend_from_slice(&[1]);
        let xref_at = s.len();
        rows.extend_from_slice(&(xref_at as u32).to_be_bytes());
        rows.extend_from_slice(&[0, 0]);
        s.extend_from_slice(
            format!(
                "5 0 obj\n<< /Type /XRef /Size 6 /W [1 4 2] /Root 1 0 R /Length {} >>\nstream\n",
                rows.len()
            )
            .as_bytes(),
        );
        s.extend_from_slice(&rows);
        s.extend_from_slice(
            format!("\nendstream\nendobj\nstartxref\n{xref_at}\n%%EOF\n").as_bytes(),
        );
        assert_eq!(annotation_count(&s, 0), Some(0));
        let out = add_annotations(&s, 0, &[square()]).unwrap();
        let tail = String::from_utf8_lossy(&out[s.len()..]).into_owned();
        assert!(tail.contains("/Type /XRef"), "{tail}");
        assert_eq!(annotation_count(&out, 0), Some(1));
    }

    #[test]
    fn refuses_encrypted_and_damaged() {
        assert_eq!(
            add_annotations(&fixture::encrypted(), 0, &[square()]),
            Err(AnnotateError::Encrypted)
        );
        assert_eq!(
            add_annotations(b"%PDF-1.4\nnothing", 0, &[square()]),
            Err(AnnotateError::Damaged)
        );
        let page = fixture::Page {
            width: 20.0,
            height: 10.0,
            content: "",
        };
        let src = fixture::build(&[page], "", None, "", &[]);
        assert_eq!(
            add_annotations(&src, 3, &[square()]),
            Err(AnnotateError::NoPage(3))
        );
    }

    #[test]
    fn numbers() {
        assert_eq!(num(1.0), "1");
        assert_eq!(num(-0.00001), "0");
        assert_eq!(num(12.34567), "12.3457");
        assert_eq!(num(0.5), "0.5");
    }
}
