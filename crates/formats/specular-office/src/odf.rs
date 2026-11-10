//! OpenDocument `meta.xml` (ODF 1.2 part 3, §4): the Dublin Core and `meta:` properties and
//! the document statistics.

use crate::xml::{self, text};
use crate::{Properties, count};

/// The properties in an OpenDocument `meta.xml` (or a flat file's `office:meta`).
pub fn odf_properties(meta: &[u8]) -> Properties {
    let e = xml::elements(meta);
    let mut p = Properties {
        title: text(&e, "title"),
        subject: text(&e, "subject"),
        description: text(&e, "description"),
        // The first author, else the last one to save it.
        author: text(&e, "initial-creator").or_else(|| text(&e, "creator")),
        last_modified_by: text(&e, "creator"),
        created: text(&e, "creation-date"),
        modified: text(&e, "date"),
        printed: text(&e, "print-date"),
        application: text(&e, "generator"),
        revision: text(&e, "editing-cycles"),
        edit_minutes: text(&e, "editing-duration").and_then(|d| duration_minutes(&d)),
        ..Properties::default()
    };
    let keywords: Vec<String> = e
        .iter()
        .filter(|k| k.name == "keyword")
        .map(|k| k.text.trim().to_owned())
        .filter(|k| !k.is_empty())
        .collect();
    p.keywords = (!keywords.is_empty()).then(|| keywords.join(", "));
    if let Some(s) = e.iter().find(|s| s.name == "document-statistic") {
        let n = |k: &str| count(s.attr(k).map(str::to_owned));
        p.pages = n("page-count");
        p.words = n("word-count");
        p.characters = n("character-count");
        p.paragraphs = n("paragraph-count");
        p.sheets = n("table-count");
    }
    p
}

/// An ISO 8601 duration (`PT1H2M3S`, `P1DT4M`) in whole minutes.
fn duration_minutes(d: &str) -> Option<u64> {
    let d = d.trim().strip_prefix('P')?;
    let mut total = 0f64;
    let mut num = String::new();
    let mut time = false;
    for c in d.chars() {
        match c {
            'T' => time = true,
            '0'..='9' | '.' | ',' => num.push(if c == ',' { '.' } else { c }),
            u => {
                let v: f64 = num.parse().ok()?;
                num.clear();
                total += v * match (u, time) {
                    ('D', false) => 1440.0,
                    ('W', false) => 10_080.0,
                    ('H', true) => 60.0,
                    ('M', true) => 1.0,
                    ('S', true) => 1.0 / 60.0,
                    _ => return None,
                };
            }
        }
    }
    Some(total.floor() as u64).filter(|&m| m > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meta_xml() {
        let m = br#"<office:document-meta><office:meta>
            <meta:generator>LibreOffice/7.6</meta:generator><dc:title>Plan</dc:title>
            <meta:initial-creator>Ann</meta:initial-creator><dc:creator>Bob</dc:creator>
            <meta:creation-date>2024-01-02T03:04:05.123</meta:creation-date>
            <dc:date>2024-02-03T04:05:06</dc:date><meta:keyword>a</meta:keyword><meta:keyword>b</meta:keyword>
            <meta:editing-duration>PT1H30M10S</meta:editing-duration>
            <meta:document-statistic meta:page-count="4" meta:word-count="321" meta:table-count="2"/>
            </office:meta></office:document-meta>"#;
        let p = odf_properties(m);
        assert_eq!(p.title.as_deref(), Some("Plan"));
        assert_eq!(p.author.as_deref(), Some("Ann"));
        assert_eq!(p.last_modified_by.as_deref(), Some("Bob"));
        assert_eq!(p.keywords.as_deref(), Some("a, b"));
        assert_eq!(p.pages, Some(4));
        assert_eq!(p.words, Some(321));
        assert_eq!(p.sheets, Some(2));
        assert_eq!(p.edit_minutes, Some(90));
        assert_eq!(p.modified.as_deref(), Some("2024-02-03T04:05:06"));
        assert_eq!(duration_minutes("P1DT4M"), Some(1444));
        assert_eq!(duration_minutes("junk"), None);
    }
}
