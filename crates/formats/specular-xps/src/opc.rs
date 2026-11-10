//! The Open Packaging Conventions layer: part names, interleaved pieces, content types and
//! relationships over the ZIP reader.

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

use crate::zip::{Zip, ZipError};

/// Largest part read (inflated), bytes.
pub const MAX_PART: u64 = 256 << 20;
/// Largest XML part parsed, bytes.
pub const MAX_XML: u64 = 64 << 20;

pub type Bytes = Arc<dyn AsRef<[u8]> + Send + Sync>;

/// One relationship of a part.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rel {
    pub kind: String,
    /// The target, resolved to a part name (lower case, no leading slash).
    pub target: String,
}

pub struct Package {
    data: Bytes,
    zip: Zip,
    /// Part name (normalised, lower case) → ZIP entries holding it, in piece order.
    parts: HashMap<String, Vec<usize>>,
    defaults: HashMap<String, String>,
    overrides: HashMap<String, String>,
}

impl std::fmt::Debug for Package {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Package")
            .field("parts", &self.parts.len())
            .finish()
    }
}

/// Percent-decodes a URI path (invalid escapes are kept as written).
fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2]))
        {
            out.push(h << 4 | l);
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

/// Normalises a part name: no leading slash, `.` and `..` resolved, backslashes as slashes,
/// lower case (part names compare case-insensitively).
pub fn normalise(name: &str) -> String {
    let mut segs: Vec<&str> = Vec::new();
    let name = name.replace('\\', "/");
    for s in name.split('/') {
        match s {
            "" | "." => {}
            ".." => {
                segs.pop();
            }
            s => segs.push(s),
        }
    }
    segs.join("/").to_lowercase()
}

/// Resolves `uri` (relative to the part `base`, or absolute) to a part name; `None` for
/// external URIs.
pub fn resolve(base: &str, uri: &str) -> Option<String> {
    let uri = uri.trim();
    let uri = uri.split(['#', '?']).next().unwrap_or("");
    if uri.is_empty() || uri.contains("://") || uri.starts_with("mailto:") {
        return None;
    }
    let uri = percent_decode(uri);
    if let Some(abs) = uri.strip_prefix('/') {
        return Some(normalise(abs));
    }
    let dir = match base.rfind('/') {
        Some(i) => &base[..i],
        None => "",
    };
    Some(normalise(&format!("{dir}/{uri}")))
}

/// Splits a ZIP entry name into its part name and piece number, with whether it is the last
/// piece: `a/b.fpage/[3].last.piece` → (`a/b.fpage`, 3, true).
fn piece(name: &str) -> Option<(&str, u32, bool)> {
    let (part, last) = name.rsplit_once('/')?;
    let rest = last.strip_prefix('[')?;
    let (n, tail) = rest.split_once(']')?;
    let n: u32 = n.parse().ok()?;
    let tail = tail.to_ascii_lowercase();
    match tail.as_str() {
        ".piece" => Some((part, n, false)),
        ".last.piece" => Some((part, n, true)),
        _ => None,
    }
}

impl Package {
    pub fn open(data: Bytes) -> Result<Package, ZipError> {
        let zip = Zip::parse((*data).as_ref())?;
        let mut parts: HashMap<String, Vec<usize>> = HashMap::new();
        let mut pieces: HashMap<String, Vec<(u32, usize)>> = HashMap::new();
        for (i, e) in zip.entries.iter().enumerate() {
            if e.name.ends_with('/') {
                continue;
            }
            if let Some((part, n, _)) = piece(&e.name) {
                pieces.entry(normalise(part)).or_default().push((n, i));
            } else {
                parts.insert(normalise(&e.name), vec![i]);
            }
        }
        for (name, mut list) in pieces {
            list.sort_unstable();
            // Only complete runs [0], [1], … count.
            if list.iter().enumerate().all(|(k, (n, _))| *n as usize == k) {
                parts
                    .entry(name)
                    .or_insert_with(|| list.into_iter().map(|(_, i)| i).collect());
            }
        }
        let mut pkg = Package {
            data,
            zip,
            parts,
            defaults: HashMap::new(),
            overrides: HashMap::new(),
        };
        pkg.read_content_types();
        Ok(pkg)
    }

    pub fn has(&self, name: &str) -> bool {
        self.parts.contains_key(name)
    }

    /// Part names, in no particular order.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.parts.keys().map(String::as_str)
    }

    /// Whether any member is encrypted (traditional ZIP encryption or AES).
    pub fn any_encrypted(&self) -> bool {
        self.zip.entries.iter().any(|e| e.encrypted())
    }

    /// The part's bytes (pieces joined), at most `max`.
    pub fn read(&self, name: &str, max: u64) -> Result<Cow<'_, [u8]>, ZipError> {
        let list = self.parts.get(name).ok_or(ZipError::Corrupt)?;
        let data = (*self.data).as_ref();
        if let [one] = list.as_slice() {
            return Zip::read(data, &self.zip.entries[*one], max.min(MAX_PART));
        }
        let mut out = Vec::new();
        for &i in list {
            let left = max.min(MAX_PART).saturating_sub(out.len() as u64);
            let b = Zip::read(data, &self.zip.entries[i], left)?;
            out.extend_from_slice(&b);
        }
        Ok(Cow::Owned(out))
    }

    fn read_content_types(&mut self) {
        if let Some((defaults, overrides)) = self.content_types() {
            self.defaults = defaults;
            self.overrides = overrides;
        }
    }

    #[allow(clippy::type_complexity)]
    fn content_types(&self) -> Option<(HashMap<String, String>, HashMap<String, String>)> {
        let b = self.read("[content_types].xml", MAX_XML).ok()?;
        let text = xml_text(&b)?;
        let doc = crate::xml::parse(&text).ok()?;
        let mut defaults = HashMap::new();
        let mut overrides = HashMap::new();
        for n in doc.root_element().children().filter(|n| n.is_element()) {
            let ct = n
                .attribute("ContentType")
                .unwrap_or("")
                .to_ascii_lowercase();
            match n.tag_name().name() {
                "Default" => {
                    if let Some(ext) = n.attribute("Extension") {
                        defaults.insert(ext.to_ascii_lowercase(), ct);
                    }
                }
                "Override" => {
                    if let Some(p) = n.attribute("PartName") {
                        overrides.insert(normalise(&percent_decode(p)), ct);
                    }
                }
                _ => {}
            }
        }
        Some((defaults, overrides))
    }

    /// The part's content type (lower case), when the package says.
    pub fn content_type(&self, name: &str) -> Option<&str> {
        if let Some(t) = self.overrides.get(name) {
            return Some(t);
        }
        let ext = name.rsplit_once('.')?.1;
        self.defaults.get(ext).map(String::as_str)
    }

    /// The relationships of `part` (`""` for the package's own).
    pub fn rels(&self, part: &str) -> Vec<Rel> {
        let rels = match part.rsplit_once('/') {
            Some((dir, file)) => format!("{dir}/_rels/{file}.rels"),
            None if part.is_empty() => "_rels/.rels".to_owned(),
            None => format!("_rels/{part}.rels"),
        };
        let Ok(b) = self.read(&rels, MAX_XML) else {
            return Vec::new();
        };
        let Some(text) = xml_text(&b) else {
            return Vec::new();
        };
        let Ok(doc) = crate::xml::parse(&text) else {
            return Vec::new();
        };
        doc.root_element()
            .children()
            .filter(|n| n.is_element() && n.tag_name().name() == "Relationship")
            .filter(|n| n.attribute("TargetMode") != Some("External"))
            .filter_map(|n| {
                Some(Rel {
                    kind: n.attribute("Type")?.to_owned(),
                    target: resolve(part, n.attribute("Target")?)?,
                })
            })
            .collect()
    }
}

/// The bytes without a UTF-8 byte order mark.
pub fn strip_bom(b: &[u8]) -> &[u8] {
    b.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(b)
}

/// XML part text: UTF-8 (with or without BOM) or UTF-16 with a BOM.
pub fn xml_text(b: &[u8]) -> Option<Cow<'_, str>> {
    if let Some(rest) = b.strip_prefix(b"\xFF\xFE") {
        let u: Vec<u16> = rest
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        return String::from_utf16(&u).ok().map(Cow::Owned);
    }
    if let Some(rest) = b.strip_prefix(b"\xFE\xFF") {
        let u: Vec<u16> = rest
            .chunks_exact(2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect();
        return String::from_utf16(&u).ok().map(Cow::Owned);
    }
    std::str::from_utf8(strip_bom(b)).ok().map(Cow::Borrowed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_resolve() {
        assert_eq!(
            resolve("Documents/1/FixedDocument.fdoc", "Pages/1.fpage").as_deref(),
            Some("documents/1/pages/1.fpage")
        );
        assert_eq!(
            resolve(
                "documents/1/pages/1.fpage",
                "../../../Resources/A%20B.odttf"
            )
            .as_deref(),
            Some("resources/a b.odttf")
        );
        assert_eq!(
            resolve("x/y", "/Abs/z.png#frag").as_deref(),
            Some("abs/z.png")
        );
        assert_eq!(resolve("x/y", "http://example.com/a"), None);
        assert_eq!(
            piece("a/b.fpage/[3].last.piece"),
            Some(("a/b.fpage", 3, true))
        );
        assert_eq!(piece("a/b.fpage/[0].piece"), Some(("a/b.fpage", 0, false)));
        assert_eq!(piece("a/b.fpage"), None);
    }
}
