//! FictionBook 2 (`.fb2`, and `.fb2.zip`: the book zipped alone): XML with the book's
//! description, one or more bodies of nested sections, and its pictures as base64
//! `<binary>` elements. Each top-level section of the main body becomes a chapter page;
//! other bodies (notes, comments) follow it. Written from the FictionBook 2.0/2.1 schema
//! (gribuser.ru, public).

use std::collections::HashMap;

use super::{Builder, Meta, Toc, esc, image_type, xhtml};
use crate::Error;
use crate::xml::{Doc, Kind, NodeId, local_of};

/// The book's own small style sheet: FB2 has none.
const CSS: &str = "h1,h2,h3,h4,h5,h6{text-align:center}\n\
p.subtitle{text-align:center;font-weight:bold}\n\
blockquote.epigraph{font-style:italic;margin-left:30%}\n\
p.author{text-align:right;font-style:italic}\n\
div.poem{margin-left:2em}\n\
div.stanza{margin-bottom:1em}\n\
div.stanza p{margin:0;text-indent:0}\n\
div.image{text-align:center}\n";

/// Whether `head` starts an FB2 document: XML whose first element is `FictionBook`.
pub fn is_fb2(head: &[u8]) -> bool {
    let h = &head[..head.len().min(4096)];
    let s = String::from_utf8_lossy(h);
    let s = s.trim_start_matches('\u{FEFF}').trim_start();
    if !s.starts_with('<') {
        return false;
    }
    // The first element after the declaration, comments and processing instructions.
    let mut rest = s;
    loop {
        rest = rest.trim_start();
        if let Some(r) = rest.strip_prefix("<?") {
            rest = r.split_once("?>").map_or("", |x| x.1);
        } else if let Some(r) = rest.strip_prefix("<!--") {
            rest = r.split_once("-->").map_or("", |x| x.1);
        } else if rest.starts_with("<!") {
            rest = rest.split_once('>').map_or("", |x| x.1);
        } else {
            break;
        }
    }
    let name: String = rest
        .strip_prefix('<')
        .unwrap_or("")
        .chars()
        .take_while(|c| !c.is_whitespace() && *c != '>' && *c != '/')
        .collect();
    local_of(&name) == "FictionBook"
}

/// Base64 (RFC 4648), white space skipped; `None` on a stray character.
fn base64(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut acc, mut n) = (0u32, 0);
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' => break,
            c if c.is_ascii_whitespace() => continue,
            _ => return None,
        };
        acc = (acc << 6) | u32::from(v);
        n += 6;
        if n >= 8 {
            n -= 8;
            out.push((acc >> n) as u8);
        }
    }
    Some(out)
}

/// An `l:href` (or `xlink:href`, `href`) that points into the book: its id.
fn local_ref(doc: &Doc, n: NodeId) -> Option<String> {
    let h = doc.attr(n, "href")?;
    h.strip_prefix('#').map(str::to_owned)
}

struct Writer<'a> {
    doc: &'a Doc,
    /// Picture id → package path.
    images: &'a HashMap<String, String>,
    /// Element id → the page it is on (for links).
    targets: &'a HashMap<String, String>,
    toc: Vec<Toc>,
    page: String,
    sections: usize,
}

impl Writer<'_> {
    /// The markup of `n`'s children.
    fn children(&mut self, n: NodeId, depth: u32, out: &mut String) {
        for &c in self.doc.children(n) {
            self.node(c, depth, out);
        }
    }

    fn id_attr(&self, n: NodeId) -> String {
        self.doc
            .attr_exact(n, "id")
            .map(|i| format!(" id=\"{}\"", esc(i)))
            .unwrap_or_default()
    }

    fn image(&self, n: NodeId) -> Option<String> {
        let id = local_ref(self.doc, n)?;
        let path = self.images.get(&id)?;
        let alt = self.doc.attr(n, "alt").unwrap_or("");
        Some(format!("<img src=\"{}\" alt=\"{}\"/>", esc(path), esc(alt)))
    }

    fn node(&mut self, n: NodeId, depth: u32, out: &mut String) {
        let doc = self.doc;
        let el = match &doc.node(n).kind {
            Kind::Text(t) => {
                out.push_str(&esc(t));
                return;
            }
            Kind::Element { name, .. } => local_of(name).to_owned(),
            Kind::Root => return,
        };
        let id = self.id_attr(n);
        let wrap = |w: &mut Self, tag: &str, attrs: &str, out: &mut String| {
            out.push_str(&format!("<{tag}{attrs}>"));
            w.children(n, depth, out);
            out.push_str(&format!("</{tag}>"));
        };
        match el.as_str() {
            "section" => {
                self.sections += 1;
                let anchor = doc
                    .attr_exact(n, "id")
                    .map_or_else(|| format!("s{}", self.sections), str::to_owned);
                out.push_str(&format!("<div id=\"{}\">", esc(&anchor)));
                if let Some(t) = doc.first(n, "title") {
                    self.toc.push(Toc {
                        title: doc.text(t),
                        href: format!("{}#{anchor}", self.page),
                        depth,
                    });
                }
                for &c in doc.children(n) {
                    self.node(c, depth + 1, out);
                }
                out.push_str("</div>");
            }
            "title" => {
                // Sections nest one level each: a book's title is h1, its parts' h1 too.
                let h = depth.clamp(1, 6);
                out.push_str(&format!("<h{h}{id}>"));
                // A title's paragraphs are its lines.
                let mut first = true;
                for &c in doc.children(n) {
                    if doc.is(c, "p") {
                        if !first {
                            out.push_str("<br/>");
                        }
                        first = false;
                        self.children(c, depth, out);
                    } else if doc.is(c, "empty-line") {
                        out.push_str("<br/>");
                    } else if !matches!(doc.node(c).kind, Kind::Text(_)) {
                        self.node(c, depth, out);
                    }
                }
                out.push_str(&format!("</h{h}>"));
            }
            "p" => wrap(self, "p", &id, out),
            "subtitle" => wrap(self, "p", &format!(" class=\"subtitle\"{id}"), out),
            "text-author" => wrap(self, "p", " class=\"author\"", out),
            "empty-line" => out.push_str("<p>\u{a0}</p>"),
            "emphasis" => wrap(self, "em", "", out),
            "strong" => wrap(self, "strong", "", out),
            "strikethrough" => wrap(self, "del", "", out),
            "sub" => wrap(self, "sub", "", out),
            "sup" => wrap(self, "sup", "", out),
            "code" => wrap(self, "code", "", out),
            "style" => wrap(self, "span", "", out),
            "epigraph" => wrap(self, "blockquote", " class=\"epigraph\"", out),
            "cite" => wrap(self, "blockquote", &id, out),
            "annotation" => wrap(self, "div", &id, out),
            "poem" => wrap(self, "div", " class=\"poem\"", out),
            "stanza" => wrap(self, "div", " class=\"stanza\"", out),
            "v" => wrap(self, "p", "", out),
            "date" => wrap(self, "p", " class=\"author\"", out),
            "table" => wrap(self, "table", "", out),
            "tr" => wrap(self, "tr", "", out),
            "td" => wrap(self, "td", "", out),
            "th" => wrap(self, "th", "", out),
            "a" => {
                let target = local_ref(doc, n).and_then(|t| {
                    let page = self.targets.get(&t)?;
                    Some(if *page == self.page {
                        format!("#{t}")
                    } else {
                        format!("{page}#{t}")
                    })
                });
                let note = doc.attr(n, "type") == Some("note");
                if note {
                    out.push_str("<sup>");
                }
                match target {
                    Some(h) => wrap(self, "a", &format!(" href=\"{}\"", esc(&h)), out),
                    None => self.children(n, depth, out),
                }
                if note {
                    out.push_str("</sup>");
                }
            }
            "image" => {
                if let Some(img) = self.image(n) {
                    // An image among paragraphs is a block; inside one, inline.
                    let parent = doc.node(n).parent;
                    let block = doc.is(parent, "section")
                        || doc.is(parent, "body")
                        || doc.is(parent, "coverpage");
                    if block {
                        out.push_str(&format!("<div class=\"image\"{id}>{img}</div>"));
                    } else {
                        out.push_str(&img);
                    }
                }
            }
            _ => self.children(n, depth, out),
        }
    }
}

/// The text of the first `local` element below `n`.
fn text_of(doc: &Doc, n: NodeId, local: &str) -> Option<String> {
    let t = doc.text(doc.first(n, local)?);
    (!t.is_empty()).then_some(t)
}

/// Converts the FB2 document in `data` (or the one FB2 inside a ZIP) to an EPUB package.
pub fn to_epub(data: &[u8]) -> Result<Vec<u8>, Error> {
    let owned;
    let data = if data.starts_with(b"PK\x03\x04") {
        owned = unzip_fb2(data)?;
        &owned[..]
    } else {
        data
    };
    if !is_fb2(data) {
        return Err(Error::NotEpub);
    }
    let doc = Doc::parse_xml(data);
    let root = doc
        .find(0, "FictionBook".to_ascii_lowercase().as_str())
        .or_else(|| doc.find(0, "fictionbook"))
        .ok_or_else(|| Error::Damaged("no FictionBook element".into()))?;
    let mut b = Builder::default();
    // Description.
    let mut meta = Meta::default();
    if let Some(d) = doc.first(root, "description") {
        if let Some(ti) = doc.first(d, "title-info") {
            meta.title = text_of(&doc, ti, "book-title").unwrap_or_default();
            for a in doc.elements(ti, "author") {
                let part = |k: &str| {
                    doc.first(a, k)
                        .map(|n| doc.text(n))
                        .filter(|t| !t.is_empty())
                };
                let name: Vec<String> = ["first-name", "middle-name", "last-name"]
                    .iter()
                    .filter_map(|k| part(k))
                    .collect();
                let name = if name.is_empty() {
                    part("nickname").unwrap_or_default()
                } else {
                    name.join(" ")
                };
                if !name.is_empty() {
                    meta.authors.push(name);
                }
            }
            meta.language = text_of(&doc, ti, "lang");
            meta.date = doc
                .first(ti, "date")
                .and_then(|n| {
                    doc.attr(n, "value")
                        .map(str::to_owned)
                        .or_else(|| Some(doc.text(n)))
                })
                .filter(|t| !t.is_empty());
            meta.description = text_of(&doc, ti, "annotation");
            meta.subjects = doc
                .elements(ti, "genre")
                .map(|g| doc.text(g))
                .filter(|t| !t.is_empty())
                .collect();
        }
        if let Some(pi) = doc.first(d, "publish-info") {
            meta.publisher = text_of(&doc, pi, "publisher");
            if meta.date.is_none() {
                meta.date = text_of(&doc, pi, "year");
            }
        }
        if let Some(di) = doc.first(d, "document-info") {
            meta.identifier = text_of(&doc, di, "id");
        }
    }
    b.meta = meta;
    // Pictures.
    let mut images = HashMap::new();
    for (k, bin) in doc.elements(root, "binary").enumerate() {
        let Some(id) = doc.attr_exact(bin, "id") else {
            continue;
        };
        let raw: String = doc
            .children(bin)
            .iter()
            .filter_map(|&c| match &doc.node(c).kind {
                Kind::Text(t) => Some(t.as_str()),
                _ => None,
            })
            .collect();
        let Some(bytes) = base64(&raw) else { continue };
        let Some((mt, ext)) = image_type(&bytes) else {
            continue;
        };
        let path = format!("images/{k}.{ext}");
        b.add(&path, mt, bytes, false)?;
        images.insert(id.to_owned(), path);
    }
    let cover = doc
        .first(root, "description")
        .and_then(|d| doc.first(d, "title-info"))
        .and_then(|t| doc.first(t, "coverpage"))
        .and_then(|c| doc.first(c, "image"))
        .and_then(|i| local_ref(&doc, i))
        .and_then(|id| images.get(&id).cloned());
    // Pages: the cover, then a page per top-level section of the first body, then the other
    // bodies (notes) a page each.
    let bodies: Vec<NodeId> = doc.elements(root, "body").collect();
    if bodies.is_empty() {
        return Err(Error::NoPages);
    }
    let mut pages: Vec<(String, Vec<NodeId>, Option<NodeId>)> = Vec::new();
    let main = bodies[0];
    let mut pending: Vec<NodeId> = Vec::new();
    for &c in doc.children(main) {
        if doc.is(c, "section") {
            if !pending.is_empty() {
                pages.push((
                    format!("text/p{}.xhtml", pages.len()),
                    std::mem::take(&mut pending),
                    None,
                ));
            }
            pages.push((format!("text/p{}.xhtml", pages.len()), vec![c], None));
        } else if !matches!(&doc.node(c).kind, Kind::Text(t) if t.trim().is_empty()) {
            pending.push(c);
        }
    }
    if !pending.is_empty() {
        pages.push((format!("text/p{}.xhtml", pages.len()), pending, None));
    }
    for &extra in &bodies[1..] {
        pages.push((
            format!("text/p{}.xhtml", pages.len()),
            doc.children(extra).to_vec(),
            Some(extra),
        ));
    }
    // Where every id is, for links (notes).
    let mut targets = HashMap::new();
    for (path, nodes, _) in &pages {
        for &n in nodes {
            let mut stack = vec![n];
            while let Some(x) = stack.pop() {
                if let Some(id) = doc.attr_exact(x, "id") {
                    let page = path.rsplit('/').next().unwrap_or(path);
                    targets.insert(id.to_owned(), page.to_owned());
                }
                stack.extend(doc.children(x).iter().copied());
            }
        }
    }
    b.add("style.css", "text/css", CSS.as_bytes().to_vec(), false)?;
    if let Some(c) = &cover {
        b.set_cover(c);
        let body = format!(
            "<div class=\"image\"><img src=\"../{}\" alt=\"\"/></div>",
            esc(c)
        );
        b.add(
            "text/cover.xhtml",
            "application/xhtml+xml",
            xhtml("Cover", Some("../style.css"), &body),
            true,
        )?;
    }
    let mut w = Writer {
        doc: &doc,
        images: &HashMap::new(),
        targets: &targets,
        toc: Vec::new(),
        page: String::new(),
        sections: 0,
    };
    // Pictures are referred to from `text/`.
    let rel: HashMap<String, String> = images
        .iter()
        .map(|(k, v)| (k.clone(), format!("../{v}")))
        .collect();
    w.images = &rel;
    for (path, nodes, body) in &pages {
        w.page = path.rsplit('/').next().unwrap_or(path).to_owned();
        let mut html = String::new();
        let first_toc = w.toc.len();
        if let Some(bn) = body {
            let title = doc.attr_exact(*bn, "name").unwrap_or("Notes");
            if doc.first(*bn, "title").is_none() {
                w.toc.push(Toc {
                    title: title.to_owned(),
                    href: w.page.clone(),
                    depth: 0,
                });
            }
        }
        for &n in nodes {
            w.node(n, 0, &mut html);
        }
        let title = w
            .toc
            .get(first_toc)
            .map(|t| t.title.clone())
            .unwrap_or_default();
        b.add(
            path,
            "application/xhtml+xml",
            xhtml(&title, Some("../style.css"), &html),
            true,
        )?;
    }
    // Table of contents entries point from the package root.
    b.toc = w
        .toc
        .into_iter()
        .map(|t| Toc {
            href: format!("text/{}", t.href),
            ..t
        })
        .collect();
    Ok(b.finish())
}

/// The FB2 inside a `.fb2.zip`.
fn unzip_fb2(data: &[u8]) -> Result<Vec<u8>, Error> {
    let a = specular_archive::Archive::from_bytes(data.to_vec(), Default::default())
        .map_err(|e| Error::Damaged(e.to_string()))?;
    let i = a
        .entries()
        .iter()
        .position(|e| e.name.to_ascii_lowercase().ends_with(".fb2"))
        .ok_or(Error::NotEpub)?;
    a.read(i).map_err(|e| Error::Damaged(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognised() {
        assert!(is_fb2(
            b"<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<FictionBook xmlns=\"x\">"
        ));
        assert!(is_fb2(b"\xEF\xBB\xBF<FictionBook>"));
        assert!(!is_fb2(b"<?xml version=\"1.0\"?><html>"));
        assert!(!is_fb2(b"FictionBook"));
    }

    #[test]
    fn converts_and_opens() {
        let png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";
        let b64: String = {
            let t = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
            png.chunks(3)
                .flat_map(|c| {
                    let v = (u32::from(c[0]) << 16)
                        | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
                        | u32::from(*c.get(2).unwrap_or(&0));
                    let n = c.len() + 1;
                    (0..4).map(move |i| {
                        if i < n {
                            t[(v >> (18 - 6 * i) & 63) as usize] as char
                        } else {
                            '='
                        }
                    })
                })
                .collect()
        };
        let fb2 = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<FictionBook xmlns=\"http://www.gribuser.ru/xml/fictionbook/2.0\" xmlns:l=\"http://www.w3.org/1999/xlink\">\
             <description><title-info><genre>sf</genre><author><first-name>Ann</first-name><last-name>Lee</last-name></author>\
             <book-title>Stars &amp; Sea</book-title><lang>en</lang><coverpage><image l:href=\"#c.png\"/></coverpage></title-info></description>\
             <body><section><title><p>One</p></title><p>Hello <emphasis>world</emphasis><a l:href=\"#n1\" type=\"note\">1</a></p>\
             <section><title><p>One A</p></title><p>Deeper</p></section></section>\
             <section><title><p>Two</p></title><empty-line/><p>Bye</p></section></body>\
             <body name=\"notes\"><section id=\"n1\"><p>A note</p></section></body>\
             <binary id=\"c.png\" content-type=\"image/png\">{b64}</binary></FictionBook>"
        );
        let epub = to_epub(fb2.as_bytes()).unwrap();
        let book = crate::Book::open(epub, &Default::default()).unwrap();
        let m = book.metadata();
        assert_eq!(m.title.as_deref(), Some("Stars & Sea"));
        assert_eq!(m.creators[0].name, "Ann Lee");
        assert_eq!(m.languages, ["en"]);
        let toc: Vec<(&str, u32)> = book
            .toc()
            .iter()
            .map(|t| (t.title.as_str(), t.depth))
            .collect();
        assert_eq!(toc, [("One", 0), ("One A", 1), ("Two", 0), ("notes", 0)]);
        assert_eq!(book.spine().len(), 4);
        let (cover, mt) = book.cover().unwrap();
        assert_eq!(mt, "image/png");
        assert_eq!(&cover[..8], &png[..8]);
        let p0 = String::from_utf8(book.read("OEBPS/text/p0.xhtml").unwrap().into_vec()).unwrap();
        assert!(p0.contains("<em>world</em>"), "{p0}");
        assert!(p0.contains("href=\"p2.xhtml#n1\""), "{p0}");
    }
}
