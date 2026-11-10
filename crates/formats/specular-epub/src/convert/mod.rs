//! Other e-book formats as EPUB: Mobipocket and Kindle (MOBI, AZW, AZW3 / KF8, PRC) and
//! FictionBook 2 (FB2, FB2.ZIP) are rewritten into an EPUB 3 package in memory (stored ZIP),
//! which [`crate::Book`] then opens like any EPUB: one layout, one renderer.
//!
//! Books locked with DRM are refused with the scheme named; nothing here removes it.

pub mod fb2;
pub mod html;
pub mod mobi;

use crate::Error;

/// Largest converted book (its text and pictures).
pub(crate) const MAX_BOOK: usize = 512 << 20;

/// An entry of the table of contents: title, target (a package path with an optional
/// `#fragment`) and depth from 0.
#[derive(Clone, Debug)]
pub(crate) struct Toc {
    pub title: String,
    pub href: String,
    pub depth: u32,
}

/// What the package says about the book.
#[derive(Clone, Debug, Default)]
pub(crate) struct Meta {
    pub title: String,
    pub authors: Vec<String>,
    pub publisher: Option<String>,
    pub date: Option<String>,
    pub language: Option<String>,
    pub description: Option<String>,
    pub subjects: Vec<String>,
    pub identifier: Option<String>,
    pub rights: Option<String>,
}

/// An EPUB 3 package being written: XHTML and CSS in reading order, pictures, the cover.
#[derive(Default)]
pub(crate) struct Builder {
    pub meta: Meta,
    /// (path under `OEBPS/`, media type, bytes, in the spine).
    files: Vec<(String, String, Vec<u8>, bool)>,
    cover: Option<String>,
    pub toc: Vec<Toc>,
    size: usize,
}

/// `s` with XML's five special characters escaped.
pub(crate) fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&apos;"),
            // Characters XML 1.0 forbids.
            c if (c as u32) < 0x20 && !matches!(c, '\t' | '\n' | '\r') => {}
            c => o.push(c),
        }
    }
    o
}

/// The media type of a picture by its signature.
pub(crate) fn image_type(d: &[u8]) -> Option<(&'static str, &'static str)> {
    if d.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some(("image/jpeg", "jpg"))
    } else if d.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some(("image/png", "png"))
    } else if d.starts_with(b"GIF8") {
        Some(("image/gif", "gif"))
    } else if d.starts_with(b"BM") && d.len() > 26 {
        Some(("image/bmp", "bmp"))
    } else if d.len() > 12 && &d[..4] == b"RIFF" && &d[8..12] == b"WEBP" {
        Some(("image/webp", "webp"))
    } else if d.starts_with(b"<svg") || d.starts_with(b"<?xml") {
        Some(("image/svg+xml", "svg"))
    } else {
        None
    }
}

impl Builder {
    /// Adds a file; `spine` puts it in reading order (in the order added).
    pub fn add(
        &mut self,
        path: &str,
        media_type: &str,
        bytes: Vec<u8>,
        spine: bool,
    ) -> Result<(), Error> {
        self.size += bytes.len();
        if self.size > MAX_BOOK {
            return Err(Error::TooLarge(format!(
                "more than {} MB of text and pictures",
                MAX_BOOK >> 20
            )));
        }
        self.files
            .push((path.to_owned(), media_type.to_owned(), bytes, spine));
        Ok(())
    }

    /// Marks the picture at `path` as the cover.
    pub fn set_cover(&mut self, path: &str) {
        self.cover = Some(path.to_owned());
    }

    /// The package as stored ZIP bytes.
    pub fn finish(self) -> Vec<u8> {
        let m = &self.meta;
        let mut opf = String::from(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<package xmlns=\"http://www.idpf.org/2007/opf\" version=\"3.0\" unique-identifier=\"id\">\n<metadata xmlns:dc=\"http://purl.org/dc/elements/1.1/\">\n",
        );
        let title = if m.title.trim().is_empty() {
            "Untitled"
        } else {
            m.title.trim()
        };
        opf.push_str(&format!(
            "<dc:identifier id=\"id\">{}</dc:identifier>\n<dc:title>{}</dc:title>\n",
            esc(m.identifier.as_deref().unwrap_or("specular-converted")),
            esc(title)
        ));
        for a in &m.authors {
            opf.push_str(&format!("<dc:creator>{}</dc:creator>\n", esc(a)));
        }
        let opt = |tag: &str, v: &Option<String>| {
            v.as_deref()
                .filter(|v| !v.trim().is_empty())
                .map(|v| format!("<dc:{tag}>{}</dc:{tag}>\n", esc(v.trim())))
                .unwrap_or_default()
        };
        opf.push_str(&opt("publisher", &m.publisher));
        opf.push_str(&opt("date", &m.date));
        opf.push_str(&opt("language", &m.language));
        opf.push_str(&opt("description", &m.description));
        opf.push_str(&opt("rights", &m.rights));
        for s in &m.subjects {
            opf.push_str(&format!("<dc:subject>{}</dc:subject>\n", esc(s)));
        }
        if let Some(c) = &self.cover
            && let Some(i) = self.files.iter().position(|f| &f.0 == c)
        {
            opf.push_str(&format!("<meta name=\"cover\" content=\"f{i}\"/>\n"));
        }
        opf.push_str("</metadata>\n<manifest>\n<item id=\"nav\" href=\"nav.xhtml\" media-type=\"application/xhtml+xml\" properties=\"nav\"/>\n");
        for (i, (p, mt, _, _)) in self.files.iter().enumerate() {
            let props = if self.cover.as_deref() == Some(p.as_str()) {
                " properties=\"cover-image\""
            } else {
                ""
            };
            opf.push_str(&format!(
                "<item id=\"f{i}\" href=\"{}\" media-type=\"{}\"{props}/>\n",
                esc(p),
                esc(mt)
            ));
        }
        opf.push_str("</manifest>\n<spine>\n");
        for (i, f) in self.files.iter().enumerate() {
            if f.3 {
                opf.push_str(&format!("<itemref idref=\"f{i}\"/>\n"));
            }
        }
        opf.push_str("</spine>\n</package>\n");
        // The navigation document: nested lists by depth.
        let mut nav = String::from(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<html xmlns=\"http://www.w3.org/1999/xhtml\" xmlns:epub=\"http://www.idpf.org/2007/ops\"><head><title>Contents</title></head><body><nav epub:type=\"toc\"><ol>\n",
        );
        let mut depth = 0u32;
        let mut open_li = false;
        for t in &self.toc {
            let d = t.depth.min(depth + 1);
            if d > depth {
                nav.push_str("<ol>");
            } else {
                if open_li {
                    nav.push_str("</li>");
                }
                for _ in d..depth {
                    nav.push_str("</ol></li>");
                }
            }
            depth = d;
            nav.push_str(&format!(
                "<li><a href=\"{}\">{}</a>",
                esc(&t.href),
                esc(if t.title.trim().is_empty() {
                    "•"
                } else {
                    t.title.trim()
                })
            ));
            open_li = true;
        }
        if open_li {
            nav.push_str("</li>");
        }
        for _ in 0..depth {
            nav.push_str("</ol></li>");
        }
        nav.push_str("\n</ol></nav></body></html>\n");
        let mut zip = Zip::default();
        zip.add("mimetype", b"application/epub+zip");
        zip.add(
            "META-INF/container.xml",
            b"<?xml version=\"1.0\"?>\n<container version=\"1.0\" xmlns=\"urn:oasis:names:tc:opendocument:xmlns:container\"><rootfiles><rootfile full-path=\"OEBPS/content.opf\" media-type=\"application/oebps-package+xml\"/></rootfiles></container>\n",
        );
        zip.add("OEBPS/content.opf", opf.as_bytes());
        zip.add("OEBPS/nav.xhtml", nav.as_bytes());
        for (p, _, b, _) in &self.files {
            zip.add(&format!("OEBPS/{p}"), b);
        }
        zip.finish()
    }
}

/// A ZIP of stored (uncompressed) entries.
#[derive(Default)]
struct Zip {
    out: Vec<u8>,
    central: Vec<u8>,
    count: u16,
}

impl Zip {
    fn add(&mut self, name: &str, data: &[u8]) {
        let crc = crc32fast::hash(data);
        let off = self.out.len() as u32;
        let n = name.as_bytes();
        let size = data.len() as u32;
        // Local file header: version 2.0, UTF-8 names (bit 11), stored, no time.
        self.out.extend_from_slice(b"PK\x03\x04");
        self.out.extend_from_slice(&20u16.to_le_bytes());
        self.out.extend_from_slice(&0x0800u16.to_le_bytes());
        self.out.extend_from_slice(&0u16.to_le_bytes());
        self.out.extend_from_slice(&[0, 0, 0x21, 0]);
        self.out.extend_from_slice(&crc.to_le_bytes());
        self.out.extend_from_slice(&size.to_le_bytes());
        self.out.extend_from_slice(&size.to_le_bytes());
        self.out.extend_from_slice(&(n.len() as u16).to_le_bytes());
        self.out.extend_from_slice(&0u16.to_le_bytes());
        self.out.extend_from_slice(n);
        self.out.extend_from_slice(data);
        // Central directory entry.
        let c = &mut self.central;
        c.extend_from_slice(b"PK\x01\x02");
        c.extend_from_slice(&20u16.to_le_bytes());
        c.extend_from_slice(&20u16.to_le_bytes());
        c.extend_from_slice(&0x0800u16.to_le_bytes());
        c.extend_from_slice(&0u16.to_le_bytes());
        c.extend_from_slice(&[0, 0, 0x21, 0]);
        c.extend_from_slice(&crc.to_le_bytes());
        c.extend_from_slice(&size.to_le_bytes());
        c.extend_from_slice(&size.to_le_bytes());
        c.extend_from_slice(&(n.len() as u16).to_le_bytes());
        c.extend_from_slice(&[0; 8]);
        c.extend_from_slice(&[0; 4]);
        c.extend_from_slice(&off.to_le_bytes());
        c.extend_from_slice(n);
        self.count += 1;
    }

    fn finish(mut self) -> Vec<u8> {
        let at = self.out.len() as u32;
        let len = self.central.len() as u32;
        self.out.extend_from_slice(&self.central);
        self.out.extend_from_slice(b"PK\x05\x06");
        self.out.extend_from_slice(&[0; 4]);
        self.out.extend_from_slice(&self.count.to_le_bytes());
        self.out.extend_from_slice(&self.count.to_le_bytes());
        self.out.extend_from_slice(&len.to_le_bytes());
        self.out.extend_from_slice(&at.to_le_bytes());
        self.out.extend_from_slice(&0u16.to_le_bytes());
        self.out
    }
}

/// An XHTML page around `body` (already markup), titled.
pub(crate) fn xhtml(title: &str, css: Option<&str>, body: &str) -> Vec<u8> {
    let link = css
        .map(|c| {
            format!(
                "<link rel=\"stylesheet\" type=\"text/css\" href=\"{}\"/>",
                esc(c)
            )
        })
        .unwrap_or_default();
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<html xmlns=\"http://www.w3.org/1999/xhtml\"><head><title>{}</title>{link}</head><body>\n{body}\n</body></html>\n",
        esc(title)
    )
    .into_bytes()
}
