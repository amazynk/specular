//! The start of a text file or table as XHTML pages: what thumbnails, the page view and
//! printing show (the viewer shows the whole file itself).

use specular_epub::{HtmlDoc, HtmlSection, html_escape as esc};

use crate::{TableFile, TextFile};

/// Lines of text in the preview.
pub const TEXT_LINES: usize = 2000;
/// Bytes of text in the preview at most.
pub const TEXT_BYTES: usize = 256 << 10;
/// Rows and columns of a table in the preview.
pub const TABLE_ROWS: usize = 400;
pub const TABLE_COLUMNS: usize = 40;
/// Characters of a table cell in the preview.
const CELL_CHARS: usize = 120;

const TEXT_CSS: &str =
    "body{margin:0}\npre{font-size:9pt;line-height:1.25;white-space:pre-wrap;margin:0}\n";
const DOS_CSS: &str =
    "body{margin:0}\npre{font-size:8pt;line-height:1.0;white-space:pre;margin:0}\n";
const TABLE_CSS: &str = "body{margin:0}\ntable{border-collapse:collapse;font-size:8pt}\ntd,th{border:1px solid #bbb;padding:1px 4px}\nth{background:#eee}\n";

/// Tabs to spaces at stops of 8 (as terminals show logs).
fn expand_tabs(s: &str) -> String {
    if !s.contains('\t') {
        return s.to_owned();
    }
    let mut o = String::with_capacity(s.len() + 16);
    let mut col = 0;
    for c in s.chars() {
        if c == '\t' {
            let n = 8 - col % 8;
            o.extend(std::iter::repeat_n(' ', n));
            col += n;
        } else {
            o.push(c);
            col += 1;
        }
    }
    o
}

/// The first lines of a text file as one page of preformatted text; `dos` for DOS art (no
/// wrapping, tight lines).
pub fn text(file: &TextFile, title: &str, dos: bool) -> HtmlDoc {
    let mut body = String::from("<pre>");
    let mut bytes = 0;
    for l in file.lines(0, TEXT_LINES) {
        bytes += l.len() + 1;
        // Escape characters (terminal colours) and other controls are not shown.
        let l: String = expand_tabs(&l)
            .chars()
            .map(|c| if c.is_control() { ' ' } else { c })
            .collect();
        body.push_str(&esc(&l));
        body.push('\n');
        if bytes > TEXT_BYTES {
            break;
        }
    }
    body.push_str("</pre>");
    HtmlDoc {
        title: title.to_owned(),
        css: if dos { DOS_CSS } else { TEXT_CSS }.to_owned(),
        sections: vec![HtmlSection {
            body,
            headings: Vec::new(),
        }],
        ..HtmlDoc::default()
    }
}

fn cell(s: &str) -> String {
    let s: String = s
        .chars()
        .take(CELL_CHARS)
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    esc(&s)
}

/// The first rows of a table as a page of a grid (the header row in bold).
pub fn table(file: &TableFile, title: &str) -> HtmlDoc {
    let mut body = String::from("<table>\n");
    let cols = file.table.columns.clamp(1, TABLE_COLUMNS);
    for (i, row) in file.rows(0, TABLE_ROWS).iter().enumerate() {
        let tag = if i == 0 && file.table.header {
            "th"
        } else {
            "td"
        };
        body.push_str("<tr>");
        for k in 0..cols {
            body.push_str(&format!(
                "<{tag}>{}</{tag}>",
                row.get(k).map(|s| cell(s)).unwrap_or_default()
            ));
        }
        body.push_str("</tr>\n");
    }
    body.push_str("</table>");
    HtmlDoc {
        title: title.to_owned(),
        css: TABLE_CSS.to_owned(),
        sections: vec![HtmlSection {
            body,
            headings: Vec::new(),
        }],
        ..HtmlDoc::default()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    #[test]
    fn previews() {
        let t = TextFile::open(
            Arc::new(b"a\tb <x>\n\x1b[31mred\n".to_vec()),
            false,
            &|| false,
        )
        .unwrap();
        let d = text(&t, "log", false);
        assert!(d.sections[0].body.contains("a       b &lt;x&gt;\n [31mred"));
        let t = TableFile::open(Arc::new(b"h1,h2\n1,2\n".to_vec()), None, &|| false).unwrap();
        let d = table(&t, "t");
        assert!(
            d.sections[0]
                .body
                .contains("<tr><th>h1</th><th>h2</th></tr>")
        );
        assert!(d.sections[0].body.contains("<tr><td>1</td><td>2</td></tr>"));
        let book = specular_epub::Book::open_html(&d, &Default::default()).unwrap();
        assert_eq!(book.spine().len(), 1);
    }
}
