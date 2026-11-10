//! Documents another reader wrote as XHTML (Markdown, RTF, text previews): their sections,
//! style sheet and pictures packaged as EPUB, so they are laid out into pages like a book.

use super::{Builder, Meta, Toc, esc, image_type, xhtml};
use crate::Error;

/// One section of a document: its body markup (XHTML, already escaped) and the contents
/// entries pointing into it (`(title, fragment id, depth)`).
#[derive(Clone, Debug, Default)]
pub struct HtmlSection {
    pub body: String,
    pub headings: Vec<(String, String, u32)>,
}

/// A document as XHTML. Pictures are referenced from the bodies as `images/<name>`.
#[derive(Clone, Debug, Default)]
pub struct HtmlDoc {
    pub title: String,
    pub authors: Vec<String>,
    pub language: Option<String>,
    /// A style sheet for every section.
    pub css: String,
    pub sections: Vec<HtmlSection>,
    /// (name under `images/`, encoded picture: PNG, JPEG, GIF, BMP, WebP or SVG).
    pub pictures: Vec<(String, Vec<u8>)>,
}

/// `s` with XML's special characters escaped (for writers of [`HtmlDoc`] bodies).
pub fn escape(s: &str) -> String {
    esc(s)
}

/// The document as stored EPUB bytes.
pub fn to_epub(doc: &HtmlDoc) -> Result<Vec<u8>, Error> {
    if doc.sections.is_empty() {
        return Err(Error::NoPages);
    }
    let mut b = Builder {
        meta: Meta {
            title: doc.title.clone(),
            authors: doc.authors.clone(),
            language: doc.language.clone(),
            ..Meta::default()
        },
        ..Builder::default()
    };
    for (name, bytes) in &doc.pictures {
        let Some((mt, _)) = image_type(bytes) else {
            continue;
        };
        b.add(&format!("images/{name}"), mt, bytes.clone(), false)?;
    }
    b.add("style.css", "text/css", doc.css.as_bytes().to_vec(), false)?;
    for (i, s) in doc.sections.iter().enumerate() {
        let path = format!("s{i}.xhtml");
        for (title, id, depth) in &s.headings {
            b.toc.push(Toc {
                title: title.clone(),
                href: format!("{path}#{id}"),
                depth: *depth,
            });
        }
        b.add(
            &path,
            "application/xhtml+xml",
            xhtml(&doc.title, Some("style.css"), &s.body),
            true,
        )?;
    }
    Ok(b.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packages_and_opens() {
        let doc = HtmlDoc {
            title: "Notes & more".into(),
            css: "p{margin:0}".into(),
            sections: vec![HtmlSection {
                body: "<h1 id=\"h0\">Title</h1><p>Hello <b>world</b></p>".into(),
                headings: vec![("Title".into(), "h0".into(), 0)],
            }],
            ..HtmlDoc::default()
        };
        let book = crate::Book::open_html(&doc, &Default::default()).unwrap();
        assert_eq!(book.metadata().title.as_deref(), Some("Notes & more"));
        assert_eq!(book.spine().len(), 1);
        assert_eq!(book.toc().len(), 1);
        assert!(to_epub(&HtmlDoc::default()).is_err());
    }
}
