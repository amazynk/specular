//! A small, forgiving XMP (RDF/XML) reader that flattens properties into
//! `prefix:Name` paths, e.g. `dc:title`, `xmp:Rating`, `exif:Flash/exif:Fired`.
//!
//! Namespace prefixes are canonicalised by URI, so a file that binds the Dublin
//! Core namespace to `foo:` still yields `dc:title`.

use std::collections::HashMap;

use quick_xml::NsReader;
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::ResolveResult;

const RDF_NS: &[u8] = b"http://www.w3.org/1999/02/22-rdf-syntax-ns#";

/// Canonical prefixes for well-known namespace URIs.
fn canonical_prefix(uri: &[u8]) -> Option<&'static str> {
    Some(match uri {
        b"http://purl.org/dc/elements/1.1/" => "dc",
        b"http://ns.adobe.com/xap/1.0/" => "xmp",
        b"http://ns.adobe.com/xap/1.0/rights/" => "xmpRights",
        b"http://ns.adobe.com/xap/1.0/mm/" => "xmpMM",
        b"http://ns.adobe.com/xmp/1.0/DynamicMedia/" => "xmpDM",
        b"http://ns.adobe.com/tiff/1.0/" => "tiff",
        b"http://ns.adobe.com/exif/1.0/" => "exif",
        b"http://cipa.jp/exif/1.0/" => "exifEX",
        b"http://ns.adobe.com/exif/1.0/aux/" => "aux",
        b"http://ns.adobe.com/photoshop/1.0/" => "photoshop",
        b"http://ns.adobe.com/camera-raw-settings/1.0/" => "crs",
        b"http://ns.adobe.com/lightroom/1.0/" => "lr",
        b"http://iptc.org/std/Iptc4xmpCore/1.0/xmlns/" => "Iptc4xmpCore",
        b"http://iptc.org/std/Iptc4xmpExt/2008-02-29/" => "Iptc4xmpExt",
        b"http://ns.useplus.org/ldf/xmp/1.0/" => "plus",
        b"http://ns.google.com/photos/1.0/panorama/" => "GPano",
        b"http://ns.google.com/photos/1.0/camera/" => "GCamera",
        b"http://ns.adobe.com/hdr-gain-map/1.0/" => "hdrgm",
        b"http://www.metadataworkinggroup.com/schemas/regions/" => "mwg-rs",
        b"http://ns.adobe.com/pdf/1.3/" => "pdf",
        b"http://ns.adobe.com/xap/1.0/sType/ResourceEvent#" => "stEvt",
        b"http://ns.adobe.com/xap/1.0/sType/ResourceRef#" => "stRef",
        // DJI drones: gimbal and flight attitude, altitudes, which camera took the image.
        b"http://www.dji.com/drone-dji/1.0/" => "drone-dji",
        // Other photo apps' sidecars (`crate::sidecars`).
        b"http://darktable.sf.net/" => "darktable",
        b"http://www.digikam.org/ns/1.0/" => "digiKam",
        _ => return None,
    })
}

#[derive(Debug, Clone, PartialEq)]
pub struct XmpProperty {
    /// Canonical prefix of the top-level property, e.g. "dc".
    pub prefix: String,
    /// Full path, e.g. "dc:title" or "exif:Flash/exif:Fired".
    pub name: String,
    /// Values in document order (several for Bag/Seq/Alt).
    pub values: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct Xmp {
    pub properties: Vec<XmpProperty>,
    index: HashMap<String, usize>,
    pub error: Option<String>,
}

impl Xmp {
    pub fn get(&self, name: &str) -> Option<&str> {
        self.index
            .get(name)
            .and_then(|&i| self.properties[i].values.first())
            .map(String::as_str)
    }

    pub fn get_all(&self, name: &str) -> Option<&[String]> {
        self.index
            .get(name)
            .map(|&i| self.properties[i].values.as_slice())
    }

    /// Adds a value of the property `name` ("dc:title"); its prefix is the name's.
    pub(crate) fn add(&mut self, name: &str, value: &str) {
        let prefix = name.split_once(':').map_or(name, |(p, _)| p);
        self.push(prefix, name.to_owned(), value.to_owned());
    }

    fn push(&mut self, prefix: &str, name: String, value: String) {
        let value = value.trim().to_owned();
        if value.is_empty() {
            return;
        }
        match self.index.get(&name) {
            Some(&i) => self.properties[i].values.push(value),
            None => {
                self.index.insert(name.clone(), self.properties.len());
                self.properties.push(XmpProperty {
                    prefix: prefix.to_owned(),
                    name,
                    values: vec![value],
                });
            }
        }
    }
}

enum Frame {
    /// Outside rdf:RDF (x:xmpmeta etc.).
    Outside,
    /// rdf:RDF, rdf:Description, rdf:Bag/Seq/Alt.
    Container,
    /// rdf:li — its text is a value of the enclosing property.
    Item,
    /// A property element; `qname` is "prefix:Local".
    Property { prefix: String, qname: String },
}

pub fn parse(bytes: &[u8]) -> Xmp {
    let mut xmp = Xmp::default();
    // Some writers pad with NULs or include a BOM.
    let start = bytes.iter().position(|&b| b == b'<').unwrap_or(bytes.len());
    let end = bytes
        .iter()
        .rposition(|&b| b == b'>')
        .map_or(start, |p| p + 1);
    let mut reader = NsReader::from_reader(&bytes[start..end.max(start)]);
    reader.config_mut().check_end_names = false;

    let mut stack: Vec<Frame> = Vec::new();
    let mut in_rdf = 0usize;
    let mut text = String::new();

    loop {
        let event = match reader.read_resolved_event() {
            Ok((ns, ev)) => (ns_owned(&ns), ev),
            Err(e) => {
                xmp.error = Some(e.to_string());
                break;
            }
        };
        match event {
            (ns, Event::Start(e)) => {
                text.clear();
                let frame = open(&reader, &mut xmp, &stack, ns, &e, &mut in_rdf);
                stack.push(frame);
            }
            (ns, Event::Empty(e)) => {
                text.clear();
                let frame = open(&reader, &mut xmp, &stack, ns, &e, &mut in_rdf);
                if matches!(frame, Frame::Container) && is_rdf_root(&e) {
                    in_rdf = in_rdf.saturating_sub(1);
                }
            }
            (_, Event::Text(t)) => {
                if let Ok(s) = t.decode() {
                    text.push_str(&s);
                }
            }
            (_, Event::CData(t)) => {
                if let Ok(s) = t.decode() {
                    text.push_str(&s);
                }
            }
            (_, Event::GeneralRef(r)) => {
                if let Ok(Some(c)) = r.resolve_char_ref() {
                    text.push(c);
                } else if let Ok(name) = r.decode()
                    && let Some(v) = quick_xml::escape::resolve_predefined_entity(&name)
                {
                    text.push_str(v);
                }
            }
            (_, Event::End(e)) => {
                let frame = stack.pop();
                match frame {
                    Some(Frame::Item) | Some(Frame::Property { .. }) => {
                        if let Some((prefix, path)) = current_path(&stack, frame.as_ref()) {
                            xmp.push(&prefix, path, std::mem::take(&mut text));
                        }
                        text.clear();
                    }
                    Some(Frame::Container) if e.local_name().as_ref() == b"RDF" => {
                        in_rdf = in_rdf.saturating_sub(1);
                    }
                    _ => text.clear(),
                }
            }
            (_, Event::Eof) => break,
            _ => {}
        }
    }
    xmp
}

#[derive(Clone)]
enum Ns {
    Rdf,
    Other(Vec<u8>),
    None,
}

fn ns_owned(r: &ResolveResult) -> Ns {
    match r {
        ResolveResult::Bound(ns) if ns.as_ref() == RDF_NS => Ns::Rdf,
        ResolveResult::Bound(ns) => Ns::Other(ns.as_ref().to_vec()),
        _ => Ns::None,
    }
}

fn is_rdf_root(e: &BytesStart) -> bool {
    e.local_name().as_ref() == b"RDF"
}

fn qualified(uri: &[u8], literal_prefix: Option<&[u8]>, local: &[u8]) -> (String, String) {
    let prefix = canonical_prefix(uri).map(str::to_owned).unwrap_or_else(|| {
        literal_prefix
            .map(|p| String::from_utf8_lossy(p).into_owned())
            .unwrap_or_else(|| "ns".to_owned())
    });
    let qname = format!("{prefix}:{}", String::from_utf8_lossy(local));
    (prefix, qname)
}

/// The path of the innermost property for `frame` (which has just been popped from `stack`).
fn current_path(stack: &[Frame], frame: Option<&Frame>) -> Option<(String, String)> {
    let mut parts: Vec<&str> = Vec::new();
    let mut top_prefix: Option<&str> = None;
    for f in stack.iter().chain(frame) {
        if let Frame::Property { prefix, qname } = f {
            if top_prefix.is_none() {
                top_prefix = Some(prefix);
            }
            parts.push(qname);
        }
    }
    Some((top_prefix?.to_owned(), parts.join("/")))
}

fn open(
    reader: &NsReader<&[u8]>,
    xmp: &mut Xmp,
    stack: &[Frame],
    ns: Ns,
    e: &BytesStart,
    in_rdf: &mut usize,
) -> Frame {
    let local = e.local_name();
    let frame = match ns {
        Ns::Rdf => match local.as_ref() {
            b"RDF" => {
                *in_rdf += 1;
                Frame::Container
            }
            b"li" => Frame::Item,
            _ => Frame::Container,
        },
        Ns::Other(uri) if *in_rdf > 0 => {
            let (prefix, qname) = qualified(
                &uri,
                e.name().prefix().map(|p| p.into_inner()),
                local.as_ref(),
            );
            Frame::Property { prefix, qname }
        }
        _ if *in_rdf > 0 => Frame::Container,
        _ => Frame::Outside,
    };
    if *in_rdf == 0 || matches!(frame, Frame::Outside) {
        return frame;
    }
    // Attributes: properties in shorthand form, or rdf:resource values.
    for attr in e.attributes().with_checks(false).flatten() {
        let key = attr.key;
        if key.as_ref().starts_with(b"xmlns") || key.prefix().is_some_and(|p| p.as_ref() == b"xml")
        {
            continue;
        }
        let (rns, alocal) = reader.resolver().resolve_attribute(key);
        let value = attr
            .normalized_value(quick_xml::XmlVersion::Implicit1_0)
            .map(|v| v.into_owned())
            .unwrap_or_default();
        match rns {
            ResolveResult::Bound(n) if n.as_ref() == RDF_NS => {
                if alocal.as_ref() == b"resource"
                    && let Some((prefix, path)) = current_path(stack, Some(&frame))
                {
                    xmp.push(&prefix, path, value);
                }
            }
            ResolveResult::Bound(n) => {
                let (aprefix, aq) = qualified(
                    n.as_ref(),
                    key.prefix().map(|p| p.into_inner()),
                    alocal.as_ref(),
                );
                match current_path(stack, Some(&frame)) {
                    Some((prefix, path)) => xmp.push(&prefix, format!("{path}/{aq}"), value),
                    None => xmp.push(&aprefix, aq, value),
                }
            }
            _ => {}
        }
    }
    frame
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<?xpacket begin="" id="W5M0MpCehiHzreSzNTczkc9d"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
<rdf:Description rdf:about="" xmlns:xmp="http://ns.adobe.com/xap/1.0/" xmlns:d="http://purl.org/dc/elements/1.1/"
  xmlns:exif="http://ns.adobe.com/exif/1.0/" xmp:Rating="4" xmp:Label="Red">
 <d:title><rdf:Alt><rdf:li xml:lang="x-default">Tram &amp; river</rdf:li></rdf:Alt></d:title>
 <d:subject><rdf:Bag><rdf:li>tram</rdf:li><rdf:li>lisbon</rdf:li></rdf:Bag></d:subject>
 <exif:Flash rdf:parseType="Resource"><exif:Fired>False</exif:Fired></exif:Flash>
 <exif:Other exif:Mode="2"/>
</rdf:Description></rdf:RDF></x:xmpmeta>
<?xpacket end="w"?>"#;

    #[test]
    fn flattens() {
        let x = parse(SAMPLE.as_bytes());
        assert_eq!(x.error, None);
        assert_eq!(x.get("xmp:Rating"), Some("4"));
        assert_eq!(x.get("xmp:Label"), Some("Red"));
        assert_eq!(x.get("dc:title"), Some("Tram & river"));
        assert_eq!(x.get_all("dc:subject").unwrap(), ["tram", "lisbon"]);
        assert_eq!(x.get("exif:Flash/exif:Fired"), Some("False"));
        assert_eq!(x.get("exif:Other/exif:Mode"), Some("2"));
    }
}
