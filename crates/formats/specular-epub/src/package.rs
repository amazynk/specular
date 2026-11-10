//! The OCF container and the package: `META-INF/container.xml`, the OPF (metadata,
//! manifest, spine, guide), the table of contents (EPUB 3 `nav.xhtml` or the EPUB 2 NCX),
//! `encryption.xml` (font obfuscation undone, anything else refused) and the cover.

use std::collections::HashMap;

use sha1::{Digest, Sha1};

use crate::xml::{Doc, NodeId, collapse_ws};
use crate::{Creator, Direction, DrmScheme, Error, Layout, Metadata, TocEntry};

pub const IDPF_OBFUSCATION: &str = "http://www.idpf.org/2008/embedding";
pub const ADOBE_OBFUSCATION: &str = "http://ns.adobe.com/pdf/enc#RC";

#[derive(Debug, Clone)]
pub struct ManifestItem {
    /// Path inside the archive.
    pub path: String,
    pub media_type: String,
    pub properties: String,
}

#[derive(Debug, Clone)]
pub struct SpineItem {
    pub manifest: usize,
    pub properties: String,
}

/// How a font file was obfuscated.
#[derive(Debug, Clone)]
pub enum Obfuscation {
    /// XOR of the first 1040 bytes with the SHA-1 of the unique identifier.
    Idpf([u8; 20]),
    /// XOR of the first 1024 bytes with the 16 bytes of the `urn:uuid:` identifier.
    Adobe([u8; 16]),
}

impl Obfuscation {
    pub fn undo(&self, data: &mut [u8]) {
        let (key, n): (&[u8], usize) = match self {
            Obfuscation::Idpf(k) => (k, 1040),
            Obfuscation::Adobe(k) => (k, 1024),
        };
        for (i, b) in data.iter_mut().take(n).enumerate() {
            *b ^= key[i % key.len()];
        }
    }
}

/// What the package says, before any page is read.
#[derive(Debug, Clone, Default)]
pub struct Package {
    pub opf_path: String,
    pub metadata: Metadata,
    pub manifest: Vec<ManifestItem>,
    /// Linear items in spine order, then the non-linear ones.
    pub spine: Vec<SpineItem>,
    pub toc: Vec<TocEntry>,
    pub obfuscated: HashMap<String, Obfuscation>,
    pub cover_image: Option<usize>,
    /// The cover page the guide names (a spine document), when there is one.
    pub cover_page: Option<String>,
    /// Viewport size of the package (`rendition:viewport` / `<meta name="original-resolution">`).
    pub viewport: Option<(f32, f32)>,
}

impl Package {
    pub fn item_by_path(&self, path: &str) -> Option<usize> {
        self.manifest.iter().position(|m| m.path == path)
    }

    pub fn spine_of_path(&self, path: &str) -> Option<usize> {
        self.spine
            .iter()
            .position(|s| self.manifest[s.manifest].path == path)
    }
}

/// The directory part of an archive path, with its trailing slash ("" at the root).
pub fn dir_of(path: &str) -> &str {
    path.rfind('/').map_or("", |i| &path[..=i])
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let Some(v) = std::str::from_utf8(&b[i + 1..i + 3])
                .ok()
                .and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// `href` relative to the directory `base` (ending in `/` or empty): the archive path and
/// the fragment. `None` for links out of the book (`http:`, `mailto:` …).
pub fn resolve(base: &str, href: &str) -> Option<(String, Option<String>)> {
    let href = href.trim();
    if href.is_empty() {
        return None;
    }
    let lower = href.to_ascii_lowercase();
    if let Some(colon) = lower.find(':')
        && lower[..colon].chars().all(|c| c.is_ascii_alphabetic())
        && colon > 1
    {
        return None;
    }
    let (p, frag) = match href.split_once('#') {
        Some((p, f)) => (p, Some(percent_decode(f)).filter(|f| !f.is_empty())),
        None => (href, None),
    };
    let p = percent_decode(p.split('?').next().unwrap_or(""));
    let joined = if p.is_empty() {
        String::new()
    } else if let Some(abs) = p.strip_prefix('/') {
        abs.to_owned()
    } else {
        format!("{base}{p}")
    };
    let mut parts: Vec<&str> = Vec::new();
    for seg in joined.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            s => parts.push(s),
        }
    }
    Some((parts.join("/"), frag))
}

/// Reads an archive entry by path.
pub trait Reader {
    fn read(&self, path: &str) -> Option<crate::Part>;
    fn exists(&self, path: &str) -> bool;
}

pub fn read_package(r: &dyn Reader) -> Result<Package, Error> {
    let container = r.read("META-INF/container.xml").ok_or(Error::NotEpub)?;
    let cdoc = Doc::parse_xml(&container);
    let opf_path = cdoc
        .descendants(0, "rootfile")
        .into_iter()
        .filter_map(|n| {
            let mt = cdoc.attr(n, "media-type").unwrap_or("");
            let p = cdoc.attr(n, "full-path")?;
            Some((
                mt == "application/oebps-package+xml" || mt.is_empty(),
                p.to_owned(),
            ))
        })
        .max_by_key(|(good, _)| *good)
        .map(|(_, p)| p)
        .and_then(|p| resolve("", &p).map(|r| r.0))
        .ok_or_else(|| Error::Damaged("the container names no package file".into()))?;
    // Locked books first: their package may be readable, their pages are not.
    check_drm(r)?;
    let opf = r
        .read(&opf_path)
        .ok_or_else(|| Error::Damaged(format!("the package file {opf_path} is missing")))?;
    let doc = Doc::parse_xml(&opf);
    let pkg_el = doc
        .find(0, "package")
        .ok_or_else(|| Error::Damaged("the package file has no package element".into()))?;
    let base = dir_of(&opf_path).to_owned();
    let mut p = Package {
        opf_path: opf_path.clone(),
        ..Default::default()
    };
    p.metadata.version = doc
        .attr(pkg_el, "version")
        .unwrap_or("2.0")
        .trim()
        .to_owned();
    let uid_ref = doc.attr(pkg_el, "unique-identifier").map(str::to_owned);

    // Manifest.
    let mut by_id: HashMap<String, usize> = HashMap::new();
    if let Some(man) = doc.first(pkg_el, "manifest") {
        for it in doc.elements(man, "item") {
            let (Some(id), Some(href)) = (doc.attr(it, "id"), doc.attr(it, "href")) else {
                continue;
            };
            let Some((path, _)) = resolve(&base, href) else {
                continue;
            };
            by_id.insert(id.to_owned(), p.manifest.len());
            p.manifest.push(ManifestItem {
                path,
                media_type: doc
                    .attr(it, "media-type")
                    .unwrap_or("")
                    .trim()
                    .to_ascii_lowercase(),
                properties: doc.attr(it, "properties").unwrap_or("").to_owned(),
            });
        }
    }

    // Metadata.
    if let Some(md) = doc.first(pkg_el, "metadata") {
        read_metadata(&doc, md, uid_ref.as_deref(), &mut p);
        // EPUB 2 cover: <meta name="cover" content="id">.
        for m in doc.descendants(md, "meta") {
            if doc.attr(m, "name") == Some("cover")
                && let Some(i) = doc.attr(m, "content").and_then(|c| by_id.get(c.trim()))
                && p.manifest[*i].media_type.starts_with("image/")
                && p.cover_image.is_none()
            {
                p.cover_image = Some(*i);
            }
        }
    }
    if let Some(i) = p
        .manifest
        .iter()
        .position(|m| m.properties.split_whitespace().any(|x| x == "cover-image"))
    {
        p.cover_image = Some(i);
    }

    // Spine.
    let mut ncx_id = None;
    if let Some(spine) = doc.first(pkg_el, "spine") {
        ncx_id = doc.attr(spine, "toc").map(str::to_owned);
        p.metadata.direction = match doc.attr(spine, "page-progression-direction") {
            Some("rtl") => Direction::Rtl,
            Some("ltr") => Direction::Ltr,
            _ => Direction::Default,
        };
        let mut linear = Vec::new();
        let mut aux = Vec::new();
        for r in doc.elements(spine, "itemref") {
            let Some(&m) = doc.attr(r, "idref").and_then(|i| by_id.get(i.trim())) else {
                continue;
            };
            let is_linear = doc.attr(r, "linear").is_none_or(|l| l.trim() != "no");
            let item = SpineItem {
                manifest: m,
                properties: doc.attr(r, "properties").unwrap_or("").to_owned(),
            };
            if is_linear {
                linear.push(item)
            } else {
                aux.push(item)
            }
        }
        p.spine = linear;
        p.spine.extend(aux);
    }

    // Guide (EPUB 2): the cover page.
    if let Some(g) = doc.first(pkg_el, "guide") {
        for r in doc.elements(g, "reference") {
            if doc
                .attr(r, "type")
                .is_some_and(|t| t.eq_ignore_ascii_case("cover"))
                && let Some((path, _)) = doc.attr(r, "href").and_then(|h| resolve(&base, h))
            {
                p.cover_page = Some(path);
                break;
            }
        }
    }

    // Encryption: obfuscated fonts (anything else was refused above).
    read_obfuscation(r, &p.metadata, &mut p.obfuscated);

    // Table of contents: nav first, else the NCX.
    let nav = p
        .manifest
        .iter()
        .find(|m| m.properties.split_whitespace().any(|x| x == "nav"))
        .map(|m| m.path.clone());
    if let Some(nav) = nav
        && let Some(bytes) = r.read(&nav)
    {
        p.toc = read_nav(&Doc::parse(&bytes), &nav, &p);
    }
    if p.toc.is_empty() {
        let ncx = ncx_id
            .and_then(|i| by_id.get(i.trim()).copied())
            .or_else(|| {
                p.manifest
                    .iter()
                    .position(|m| m.media_type == "application/x-dtbncx+xml")
            })
            .map(|i| p.manifest[i].path.clone());
        if let Some(ncx) = ncx
            && let Some(bytes) = r.read(&ncx)
        {
            p.toc = read_ncx(&Doc::parse_xml(&bytes), &ncx, &p);
        }
    }
    Ok(p)
}

fn read_metadata(doc: &Doc, md: NodeId, uid_ref: Option<&str>, p: &mut Package) {
    // EPUB 3 refinements: (id, property) -> value.
    let mut refines: HashMap<(String, String), String> = HashMap::new();
    for m in doc.descendants(md, "meta") {
        if let (Some(r), Some(prop)) = (doc.attr(m, "refines"), doc.attr(m, "property")) {
            refines.insert(
                (r.trim_start_matches('#').to_owned(), prop.to_owned()),
                doc.text(m),
            );
        }
    }
    let refined = |n: NodeId, prop: &str| {
        doc.attr(n, "id")
            .and_then(|id| refines.get(&(id.to_owned(), prop.to_owned())))
            .cloned()
    };
    let m = &mut p.metadata;
    let mut titles: Vec<(bool, String)> = Vec::new();
    for c in doc.children(md).to_vec() {
        let Some(name) = doc.name(c) else { continue };
        let text = doc.text(c);
        match name {
            "dc:title" | "title" if !text.is_empty() => {
                let main = refined(c, "title-type").is_none_or(|t| t == "main");
                titles.push((main, text));
            }
            "dc:creator" | "creator" if !text.is_empty() => m.creators.push(Creator {
                name: text,
                role: doc
                    .attr(c, "opf:role")
                    .map(str::to_owned)
                    .or_else(|| refined(c, "role")),
                file_as: doc
                    .attr(c, "opf:file-as")
                    .map(str::to_owned)
                    .or_else(|| refined(c, "file-as")),
            }),
            "dc:publisher" | "publisher" if !text.is_empty() => {
                m.publisher.get_or_insert(text);
            }
            "dc:language" | "language" if !text.is_empty() => m.languages.push(text),
            "dc:date" | "date" if !text.is_empty() => {
                m.date.get_or_insert(text);
            }
            "dc:identifier" | "identifier" if !text.is_empty() => {
                let unique = uid_ref.is_some() && doc.attr(c, "id") == uid_ref;
                if unique || m.identifier.is_none() {
                    m.identifier = Some(text.clone());
                }
                m.identifiers.push(text);
            }
            "dc:description" | "description" if !text.is_empty() => {
                // Descriptions are often escaped HTML.
                let inner = Doc::parse(text.as_bytes());
                m.description.get_or_insert(inner.text(0));
            }
            "dc:subject" | "subject" if !text.is_empty() => m.subjects.push(text),
            "dc:rights" | "rights" if !text.is_empty() => {
                m.rights.get_or_insert(text);
            }
            _ => {}
        }
        if doc.local(c) == Some("meta") {
            let text = doc.text(c);
            match (doc.attr(c, "property"), doc.attr(c, "refines")) {
                (Some("dcterms:modified"), None) => m.modified = Some(text.clone()),
                (Some("rendition:layout"), None) if text == "pre-paginated" => {
                    m.layout = Layout::PrePaginated;
                }
                (Some("belongs-to-collection"), None) if m.series.is_none() => {
                    m.series = Some(text.clone());
                }
                (Some("rendition:viewport"), None) => {
                    p.viewport = parse_viewport(&text);
                }
                _ => {}
            }
            match doc.attr(c, "name") {
                Some("calibre:series") if m.series.is_none() => {
                    m.series = doc.attr(c, "content").map(str::to_owned);
                }
                Some("original-resolution") => {
                    let v = doc.attr(c, "content").unwrap_or("");
                    if let Some((w, h)) = v.split_once('x')
                        && let (Ok(w), Ok(h)) = (w.trim().parse::<f32>(), h.trim().parse::<f32>())
                        && w > 0.0
                        && h > 0.0
                    {
                        p.viewport.get_or_insert((w, h));
                    }
                }
                _ => {}
            }
        }
    }
    m.title = titles
        .iter()
        .find(|t| t.0)
        .or(titles.first())
        .map(|t| t.1.clone());
}

/// `width=1200, height=1800` (also with `;`).
pub fn parse_viewport(s: &str) -> Option<(f32, f32)> {
    let mut w = None;
    let mut h = None;
    for part in s.split([',', ';']) {
        let Some((k, v)) = part.split_once('=') else {
            continue;
        };
        let v = v.trim().trim_end_matches("px").parse::<f32>().ok();
        match k.trim().to_ascii_lowercase().as_str() {
            "width" => w = v,
            "height" => h = v,
            _ => {}
        }
    }
    match (w, h) {
        (Some(w), Some(h)) if w >= 1.0 && h >= 1.0 && w < 100_000.0 && h < 100_000.0 => {
            Some((w, h))
        }
        _ => None,
    }
}

fn check_drm(r: &dyn Reader) -> Result<(), Error> {
    let marker = if r.exists("META-INF/sinf.xml") {
        Some(DrmScheme::FairPlay)
    } else if r.exists("META-INF/license.lcpl") || r.exists("license.lcpl") {
        Some(DrmScheme::Lcp)
    } else if r.exists("META-INF/rights.xml") {
        Some(DrmScheme::Adobe)
    } else {
        None
    };
    let Some(enc) = r.read("META-INF/encryption.xml") else {
        // FairPlay and LCP books always list their encrypted parts; a marker alone with no
        // encrypted part leaves the book readable.
        return Ok(());
    };
    let doc = Doc::parse_xml(&enc);
    for ed in doc.descendants(0, "encrypteddata") {
        let alg = doc
            .find(ed, "encryptionmethod")
            .and_then(|m| doc.attr(m, "algorithm"))
            .unwrap_or("")
            .trim();
        if alg == IDPF_OBFUSCATION || alg == ADOBE_OBFUSCATION {
            continue;
        }
        let key_info = doc.find(ed, "keyinfo").map(|k| {
            let mut s = doc.text(k);
            for rm in doc.descendants(k, "retrievalmethod") {
                s.push(' ');
                s.push_str(doc.attr(rm, "uri").unwrap_or(""));
            }
            s.to_ascii_lowercase()
        });
        let key_info = key_info.unwrap_or_default();
        let scheme = if key_info.contains("license.lcpl") {
            DrmScheme::Lcp
        } else if key_info.contains("adept") || key_info.contains("adobe") {
            DrmScheme::Adobe
        } else if let Some(m) = marker {
            m
        } else if r.exists("rights.xml") || key_info.contains("kobo") {
            DrmScheme::Kobo
        } else {
            DrmScheme::Unknown
        };
        return Err(Error::Drm(scheme));
    }
    Ok(())
}

fn read_obfuscation(r: &dyn Reader, m: &Metadata, out: &mut HashMap<String, Obfuscation>) {
    let Some(enc) = r.read("META-INF/encryption.xml") else {
        return;
    };
    let doc = Doc::parse_xml(&enc);
    for ed in doc.descendants(0, "encrypteddata") {
        let alg = doc
            .find(ed, "encryptionmethod")
            .and_then(|m| doc.attr(m, "algorithm"))
            .unwrap_or("")
            .trim();
        let Some(uri) = doc
            .find(ed, "cipherreference")
            .and_then(|c| doc.attr(c, "uri"))
            .and_then(|u| resolve("", u))
            .map(|r| r.0)
        else {
            continue;
        };
        let key = if alg == IDPF_OBFUSCATION {
            m.identifier.as_deref().map(idpf_key).map(Obfuscation::Idpf)
        } else if alg == ADOBE_OBFUSCATION {
            adobe_key(m).map(Obfuscation::Adobe)
        } else {
            None
        };
        if let Some(k) = key {
            out.insert(uri, k);
        }
    }
}

/// The IDPF key: SHA-1 of the unique identifier without white space.
pub fn idpf_key(id: &str) -> [u8; 20] {
    let clean: String = id
        .chars()
        .filter(|c| !matches!(c, ' ' | '\t' | '\n' | '\r'))
        .collect();
    let d = Sha1::digest(clean.as_bytes());
    let mut k = [0u8; 20];
    k.copy_from_slice(d.as_slice());
    k
}

/// The Adobe key: the 16 bytes of the book's `urn:uuid:` identifier (the unique one when it
/// is a UUID).
pub fn adobe_key(m: &Metadata) -> Option<[u8; 16]> {
    let ids = m.identifier.iter().chain(m.identifiers.iter());
    ids.filter_map(|id| uuid_bytes(id)).next()
}

fn uuid_bytes(id: &str) -> Option<[u8; 16]> {
    let s = id.trim();
    let s = s
        .strip_prefix("urn:uuid:")
        .or_else(|| s.strip_prefix("uuid:"))
        .unwrap_or(s);
    let hex: String = s.chars().filter(|c| *c != '-').collect();
    if hex.len() != 32 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let mut out = [0u8; 16];
    for (i, o) in out.iter_mut().enumerate() {
        *o = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
}

fn toc_entry(p: &Package, base: &str, title: String, href: Option<&str>, depth: u32) -> TocEntry {
    let target = href.and_then(|h| resolve(base, h));
    let (path, fragment) = match target {
        Some((path, frag)) => (Some(path), frag),
        None => (None, None),
    };
    let spine = path.as_deref().and_then(|p2| p.spine_of_path(p2));
    TocEntry {
        title,
        depth,
        spine,
        fragment,
        href: path.unwrap_or_default(),
    }
}

/// Most table-of-contents entries read.
const MAX_TOC: usize = 20_000;

fn read_nav(doc: &Doc, path: &str, p: &Package) -> Vec<TocEntry> {
    let base = dir_of(path);
    let navs = doc.descendants(0, "nav");
    let is_toc = |n: NodeId| {
        doc.attrs(n).iter().any(|(k, v)| {
            crate::xml::local_of(k) == "type"
                && k.contains(':')
                && v.split_whitespace().any(|t| t == "toc")
        })
    };
    let Some(nav) = navs
        .iter()
        .copied()
        .find(|&n| is_toc(n))
        .or(navs.first().copied())
    else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if let Some(ol) = doc.find(nav, "ol").or_else(|| doc.find(nav, "ul")) {
        nav_list(doc, ol, 0, base, p, &mut out);
    }
    out
}

fn nav_list(doc: &Doc, ol: NodeId, depth: u32, base: &str, p: &Package, out: &mut Vec<TocEntry>) {
    if depth > 32 {
        return;
    }
    for li in doc.elements(ol, "li") {
        if out.len() >= MAX_TOC {
            return;
        }
        let a = doc
            .children(li)
            .iter()
            .copied()
            .find(|&c| doc.is(c, "a") || doc.is(c, "span"));
        if let Some(a) = a {
            let title = doc.text(a);
            let href = doc.is(a, "a").then(|| doc.attr(a, "href")).flatten();
            out.push(toc_entry(p, base, title, href, depth));
        }
        for sub in doc
            .children(li)
            .iter()
            .copied()
            .filter(|&c| doc.is(c, "ol") || doc.is(c, "ul"))
        {
            nav_list(doc, sub, depth + 1, base, p, out);
        }
    }
}

fn read_ncx(doc: &Doc, path: &str, p: &Package) -> Vec<TocEntry> {
    let base = dir_of(path);
    let mut out = Vec::new();
    if let Some(map) = doc.find(0, "navmap") {
        ncx_points(doc, map, 0, base, p, &mut out);
    }
    out
}

fn ncx_points(doc: &Doc, n: NodeId, depth: u32, base: &str, p: &Package, out: &mut Vec<TocEntry>) {
    if depth > 32 {
        return;
    }
    for np in doc.elements(n, "navpoint") {
        if out.len() >= MAX_TOC {
            return;
        }
        let title = doc
            .first(np, "navlabel")
            .map(|l| doc.text(l))
            .map(|t| collapse_ws(&t))
            .unwrap_or_default();
        let href = doc.first(np, "content").and_then(|c| doc.attr(c, "src"));
        out.push(toc_entry(p, base, title, href, depth));
        ncx_points(doc, np, depth + 1, base, p, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hrefs_resolve() {
        assert_eq!(
            resolve("OEBPS/text/", "../img/a%20b.png#x"),
            Some(("OEBPS/img/a b.png".into(), Some("x".into())))
        );
        assert_eq!(resolve("OEBPS/", "/root.css").unwrap().0, "root.css");
        assert_eq!(resolve("", "http://example.com/a"), None);
        assert_eq!(
            resolve("a/", "#frag"),
            Some(("".into(), Some("frag".into())))
        );
        assert_eq!(resolve("a/", "../../../x").unwrap().0, "x");
    }

    #[test]
    fn keys() {
        let k = idpf_key(" urn:uuid:1234 \n");
        assert_eq!(k, idpf_key("urn:uuid:1234"));
        let m = Metadata {
            identifier: Some("urn:uuid:0123abcd-0000-1111-2222-333344445555".into()),
            ..Default::default()
        };
        assert_eq!(adobe_key(&m).unwrap()[..4], [0x01, 0x23, 0xab, 0xcd]);
        assert_eq!(
            parse_viewport("width=1200, height=1800"),
            Some((1200.0, 1800.0))
        );
    }
}
