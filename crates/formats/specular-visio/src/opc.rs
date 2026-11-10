//! The OPC (ZIP) package of a VSDX: members by name, relationships, part paths.

use std::collections::HashMap;

use specular_drawpage::xml;
use specular_ora::zip::Zip;

use crate::Error;

/// Largest media part read.
const MAX_MEDIA: u64 = 512 << 20;

pub(crate) struct Package<'d> {
    data: &'d [u8],
    zip: Zip,
}

/// One relationship: id → (type, absolute target part).
pub(crate) type Rels = HashMap<String, (String, String)>;

impl<'d> Package<'d> {
    pub fn open(data: &'d [u8]) -> Result<Package<'d>, Error> {
        let zip = Zip::parse(data).map_err(|e| Error::Malformed(format!("Visio package: {e}")))?;
        Ok(Package { data, zip })
    }

    pub fn has(&self, name: &str) -> bool {
        self.find(name).is_some()
    }

    fn find(&self, name: &str) -> Option<&specular_ora::zip::Entry> {
        let name = name.trim_start_matches('/');
        self.zip.find(name).or_else(|| {
            self.zip
                .entries
                .iter()
                .find(|e| e.name.eq_ignore_ascii_case(name))
        })
    }

    pub fn read(&self, name: &str, max: u64) -> Option<Vec<u8>> {
        let e = self.find(name)?;
        self.zip
            .read(self.data, e, max)
            .ok()
            .map(|c| c.into_owned())
    }

    pub fn text(&self, name: &str) -> Option<String> {
        // Largest XML part read: the drawing stack's cap.
        xml::text_of(&self.read(name, xml::MAX_XML_PART as u64)?)
    }

    pub fn media(&self, name: &str) -> Option<Vec<u8>> {
        self.read(name, MAX_MEDIA)
    }

    /// The relationships of `part` (from `<dir>/_rels/<file>.rels`), targets made absolute.
    pub fn rels(&self, part: &str) -> Rels {
        let (dir, file) = split(part);
        let rels_name = if dir.is_empty() {
            format!("_rels/{file}.rels")
        } else {
            format!("{dir}/_rels/{file}.rels")
        };
        let mut out = HashMap::new();
        let Some(t) = self.text(&rels_name) else {
            return out;
        };
        let Ok(doc) = xml::parse(&t) else {
            return out;
        };
        for r in doc
            .descendants()
            .filter(|n| n.is_element() && n.tag_name().name() == "Relationship")
        {
            let (Some(id), Some(target)) = (r.attribute("Id"), r.attribute("Target")) else {
                continue;
            };
            if r.attribute("TargetMode") == Some("External") {
                continue;
            }
            let ty = r.attribute("Type").unwrap_or("").to_string();
            out.insert(id.to_string(), (ty, resolve(dir, target)));
        }
        out
    }
}

/// `dir/file` → (`dir`, `file`).
fn split(part: &str) -> (&str, &str) {
    let part = part.trim_start_matches('/');
    match part.rfind('/') {
        Some(i) => (&part[..i], &part[i + 1..]),
        None => ("", part),
    }
}

/// A relationship target relative to `dir` (or absolute) as a package part name.
pub(crate) fn resolve(dir: &str, target: &str) -> String {
    let mut parts: Vec<&str> = if target.starts_with('/') {
        Vec::new()
    } else {
        dir.split('/').filter(|s| !s.is_empty()).collect()
    };
    for seg in target.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            s => parts.push(s),
        }
    }
    parts.join("/")
}

/// The type suffix of a relationship type URI ("…/relationships/page" → "page").
pub(crate) fn rel_kind(ty: &str) -> &str {
    ty.rsplit('/').next().unwrap_or(ty)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn targets() {
        assert_eq!(resolve("visio/pages", "page1.xml"), "visio/pages/page1.xml");
        assert_eq!(
            resolve("visio/pages", "../media/a.png"),
            "visio/media/a.png"
        );
        assert_eq!(resolve("visio", "/docProps/x"), "docProps/x");
        assert_eq!(resolve("", "visio/document.xml"), "visio/document.xml");
        assert_eq!(resolve("a", "../../../x"), "x");
    }
}
