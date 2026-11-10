//! Lines of text set as a PDF (A4, Courier from the standard 14 fonts), so text documents —
//! e.g. an HL7 CDA report in a DICOM file — page, zoom and render like any PDF.
//!
//! Courier is monospaced (600/1000 em), so lines are wrapped by character count at word
//! boundaries. Text is written in WinAnsiEncoding: characters outside it show as '?'.

/// How a line is set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    Title,
    Heading,
    Body,
}

const PAGE_W: f32 = 595.0;
const PAGE_H: f32 = 842.0;
const MARGIN: f32 = 56.0;
/// Most pages written; later lines are dropped (with a note on the last page).
const MAX_PAGES: usize = 2000;

impl Style {
    fn size(self) -> f32 {
        match self {
            Style::Title => 15.0,
            Style::Heading => 11.5,
            Style::Body => 10.0,
        }
    }

    fn font(self) -> &'static str {
        match self {
            Style::Body => "F1",
            _ => "F2",
        }
    }

    fn columns(self) -> usize {
        ((PAGE_W - 2.0 * MARGIN) / (self.size() * 0.6)) as usize
    }
}

/// A PDF of `lines`, wrapped to the page width; an empty line is a paragraph gap.
pub fn build(lines: &[(Style, &str)]) -> Vec<u8> {
    let mut pages: Vec<String> = Vec::new();
    let mut page = String::new();
    let mut y = PAGE_H - MARGIN;
    let mut truncated = false;
    'all: for &(style, text) in lines {
        let size = style.size();
        let lead = size * 1.3;
        let gap = if style == Style::Body {
            0.0
        } else {
            size * 0.5
        };
        let wrapped = wrap(text, style.columns());
        for (i, line) in wrapped.iter().enumerate() {
            let before = if i == 0 { gap } else { 0.0 };
            if y - before - size < MARGIN {
                pages.push(std::mem::take(&mut page));
                y = PAGE_H - MARGIN;
                if pages.len() >= MAX_PAGES {
                    truncated = true;
                    break 'all;
                }
            } else {
                y -= before;
            }
            y -= size;
            if !line.is_empty() {
                page.push_str(&format!(
                    "BT /{} {size} Tf {MARGIN} {y:.2} Td ({}) Tj ET\n",
                    style.font(),
                    escape(line)
                ));
            }
            y -= lead - size;
        }
    }
    if truncated {
        let last = pages.last_mut().expect("a page");
        last.push_str(&format!(
            "BT /F2 10 Tf {MARGIN} {} Td (\\(document truncated\\)) Tj ET\n",
            MARGIN / 2.0
        ));
    } else if !page.is_empty() || pages.is_empty() {
        pages.push(page);
    }
    write(&pages)
}

/// Splits `text` into lines of at most `cols` characters, at spaces where possible.
fn wrap(text: &str, cols: usize) -> Vec<String> {
    let cols = cols.max(8);
    let mut out = Vec::new();
    for para in text.split('\n') {
        let para = para.trim_end().replace('\t', "    ");
        let mut line = String::new();
        let mut len = 0usize;
        for word in para.split(' ') {
            let mut word: Vec<char> = word.chars().collect();
            if len > 0 && len + 1 + word.len() > cols {
                out.push(std::mem::take(&mut line));
                len = 0;
            }
            if len > 0 {
                line.push(' ');
                len += 1;
            }
            while len + word.len() > cols {
                let rest = word.split_off(cols - len);
                line.extend(word);
                out.push(std::mem::take(&mut line));
                len = 0;
                word = rest;
            }
            len += word.len();
            line.extend(word);
        }
        out.push(line);
    }
    out
}

/// The WinAnsiEncoding byte of `c`.
fn win_ansi(c: char) -> u8 {
    const HIGH: [(char, u8); 27] = [
        ('€', 0x80),
        ('‚', 0x82),
        ('ƒ', 0x83),
        ('„', 0x84),
        ('…', 0x85),
        ('†', 0x86),
        ('‡', 0x87),
        ('ˆ', 0x88),
        ('‰', 0x89),
        ('Š', 0x8A),
        ('‹', 0x8B),
        ('Œ', 0x8C),
        ('Ž', 0x8E),
        ('‘', 0x91),
        ('’', 0x92),
        ('“', 0x93),
        ('”', 0x94),
        ('•', 0x95),
        ('–', 0x96),
        ('—', 0x97),
        ('˜', 0x98),
        ('™', 0x99),
        ('š', 0x9A),
        ('›', 0x9B),
        ('œ', 0x9C),
        ('ž', 0x9E),
        ('Ÿ', 0x9F),
    ];
    match c as u32 {
        0x20..=0x7E | 0xA0..=0xFF => c as u8,
        _ => HIGH.iter().find(|h| h.0 == c).map_or(b'?', |h| h.1),
    }
}

/// A PDF literal string body for `s`.
fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match win_ansi(c) {
            b'(' => out.push_str("\\("),
            b')' => out.push_str("\\)"),
            b'\\' => out.push_str("\\\\"),
            b @ 0x20..=0x7E => out.push(b as char),
            b => out.push_str(&format!("\\{b:03o}")),
        }
    }
    out
}

fn write(pages: &[String]) -> Vec<u8> {
    // 1 catalog, 2 page tree, 3 and 4 fonts, then (page, content) pairs.
    let mut objects: Vec<String> = vec![
        String::new(),
        String::new(),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Courier /Encoding /WinAnsiEncoding >>".into(),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Courier-Bold /Encoding /WinAnsiEncoding >>"
            .into(),
    ];
    objects[0] = "<< /Type /Catalog /Pages 2 0 R >>".into();
    let kids: Vec<String> = (0..pages.len())
        .map(|i| format!("{} 0 R", 5 + 2 * i))
        .collect();
    objects[1] = format!(
        "<< /Type /Pages /Kids [{}] /Count {} >>",
        kids.join(" "),
        pages.len()
    );
    for (i, content) in pages.iter().enumerate() {
        objects.push(format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {PAGE_W} {PAGE_H}] /Contents {} 0 R \
             /Resources << /Font << /F1 3 0 R /F2 4 0 R >> >> >>",
            6 + 2 * i
        ));
        objects.push(format!(
            "<< /Length {} >>\nstream\n{content}\nendstream",
            content.len() + 1
        ));
    }
    let mut out = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets = Vec::with_capacity(objects.len());
    for (i, o) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{o}\nendobj\n", i + 1).as_bytes());
    }
    let xref = out.len();
    out.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
    );
    for off in offsets {
        out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Document, RenderRequest};
    use std::sync::Arc;

    #[test]
    fn wraps_at_spaces_and_splits_long_words() {
        assert_eq!(wrap("aaa bbb ccc", 8), ["aaa bbb", "ccc"]);
        assert_eq!(wrap("abcdefghijkl", 8), ["abcdefgh", "ijkl"]);
        assert_eq!(wrap("a\n\nb", 8), ["a", "", "b"]);
        assert_eq!(escape("(a\\b) é•ж"), "\\(a\\\\b\\) \\351\\225?");
    }

    #[test]
    fn text_pdf_opens_pages_and_renders() {
        let long = "word ".repeat(400);
        let mut lines = vec![
            (Style::Title, "Report (draft)"),
            (Style::Heading, "Findings"),
        ];
        for _ in 0..30 {
            lines.push((Style::Body, &long));
        }
        let pdf = build(&lines);
        let doc = Document::open(Arc::new(pdf)).unwrap();
        assert!(doc.page_count() > 1);
        assert_eq!(doc.page_size(0), Some((595.0, 842.0)));
        let req = RenderRequest {
            page: 0,
            scale_x: 0.5,
            scale_y: 0.5,
            x: 0,
            y: 0,
            width: 297,
            height: 421,
        };
        let px = doc.render(req, &|| false).unwrap();
        // Some dark text on the white page.
        assert!(px.chunks(4).any(|p| p[0] < 100));
        // Empty input still gives one page.
        let doc = Document::open(Arc::new(build(&[]))).unwrap();
        assert_eq!(doc.page_count(), 1);
    }
}
