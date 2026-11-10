//! XMP for converted files. The packet is edited as text, keeping everything it doesn't
//! understand byte for byte: properties that describe the source's pixels (size, storage,
//! gain maps and the container items they live in, the extended-XMP pointer) are removed,
//! `tiff:Orientation` becomes 1, and with location removal every GPS property and the IPTC /
//! Photoshop place fields go as well. Properties are matched by their usual prefixes (XMP
//! writers use the standard ones); both attribute and element forms are handled.

/// `xmp` (an output's packet, or None) with the keywords of `source`'s XMP sidecar when it
/// names any (`IMG_1.CR3.xmp`, else `IMG_1.xmp`): its `dc:subject`, which leaves private
/// keywords out, and the hierarchy of those ([`specular_metadata::keywords::for_export`]).
pub fn with_sidecar_keywords(source: &std::path::Path, xmp: Option<Vec<u8>>) -> Option<Vec<u8>> {
    use specular_metadata::keywords as kw;
    let name = source.file_name().map(|n| n.to_string_lossy().into_owned());
    let stem = source.file_stem().map(|n| n.to_string_lossy().into_owned());
    let dir = source.parent().unwrap_or(std::path::Path::new(""));
    let side = [name, stem]
        .into_iter()
        .flatten()
        .flat_map(|n| [format!("{n}.xmp"), format!("{n}.XMP")])
        .map(|n| dir.join(n))
        .find(|p| p.is_file() && p.as_path() != source);
    let Some(text) = side.and_then(|p| std::fs::read_to_string(p).ok()) else {
        return xmp;
    };
    let Some((subject, hier)) = kw::for_export(&text) else {
        return xmp;
    };
    let existing = xmp.as_deref().and_then(|x| std::str::from_utf8(x).ok());
    Some(kw::write_lists(existing, &subject, &hier).into_bytes())
}

/// What [`filter`] keeps besides the pixel-independent properties.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Filter {
    /// GPS properties and place names.
    pub keep_location: bool,
    /// The people shown and face or person regions.
    pub keep_people: bool,
    /// Only these properties (exact names, or prefixes ending in `*`), when set. Structure
    /// (`rdf:`, `x:`, `xml:` and namespace declarations) always stays.
    pub only: Option<&'static [&'static str]>,
}

impl Filter {
    /// Everything descriptive, with or without location.
    pub fn all(keep_location: bool) -> Filter {
        Filter {
            keep_location,
            keep_people: true,
            only: None,
        }
    }
}

/// Copyright and usage terms.
pub const COPYRIGHT: &[&str] = &[
    "dc:rights",
    "xmpRights:Marked",
    "xmpRights:WebStatement",
    "xmpRights:UsageTerms",
    "xmpRights:Owner",
    "xmpRights:Certificate",
    "photoshop:Copyright",
    "plus:CopyrightOwner",
    "plus:Licensor",
    "plus:CopyrightOwner*",
    "plus:Licensor*",
];

/// Copyright and the creator's name and contact details.
pub const COPYRIGHT_CONTACT: &[&str] = &[
    "dc:rights",
    "xmpRights:Marked",
    "xmpRights:WebStatement",
    "xmpRights:UsageTerms",
    "xmpRights:Owner",
    "xmpRights:Certificate",
    "photoshop:Copyright",
    "plus:CopyrightOwner",
    "plus:Licensor",
    "plus:CopyrightOwner*",
    "plus:Licensor*",
    "dc:creator",
    "photoshop:AuthorsPosition",
    "photoshop:Credit",
    "Iptc4xmpCore:CreatorContactInfo",
    "Iptc4xmpCore:Ci*",
];

/// Removes the properties [`drop`] names and resets the orientation. `None` when the packet
/// isn't text or nothing descriptive is left (no `rdf:Description` survives).
pub fn sanitize(xmp: &[u8], keep_location: bool) -> Option<Vec<u8>> {
    filter(xmp, &Filter::all(keep_location))
}

/// As [`sanitize`], keeping what `f` says.
pub fn filter(xmp: &[u8], f: &Filter) -> Option<Vec<u8>> {
    let s = std::str::from_utf8(xmp).ok()?;
    let s = s.trim_end_matches('\0');
    let mut out = String::with_capacity(s.len());
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] != b'<' {
            let next = s[i..].find('<').map_or(b.len(), |n| i + n);
            out.push_str(&s[i..next]);
            i = next;
            continue;
        }
        // Comments, processing instructions, CDATA and end tags pass through.
        if s[i..].starts_with("<!--") {
            let end = s[i..].find("-->").map_or(b.len(), |n| i + n + 3);
            out.push_str(&s[i..end]);
            i = end;
            continue;
        }
        if s[i..].starts_with("<?") || s[i..].starts_with("<!") || s[i..].starts_with("</") {
            let end = tag_end(s, i);
            out.push_str(&s[i..end]);
            i = end;
            continue;
        }
        let end = tag_end(s, i);
        let tag = &s[i..end];
        let name = tag_name(tag);
        if drop(name, f) {
            i = skip_element(s, end, name);
            continue;
        }
        if name == "tiff:Orientation" && !tag.ends_with("/>") {
            // Element form: replace the content up to the end tag.
            let close = format!("</{name}>");
            if let Some(n) = s[end..].find(&close) {
                out.push_str(tag);
                out.push('1');
                out.push_str(&close);
                i = end + n + close.len();
                continue;
            }
        }
        out.push_str(&rewrite_attributes(tag, f));
        i = end;
    }
    // A description left without content is harmless; a packet without any isn't worth
    // writing.
    out.contains("rdf:Description").then(|| out.into_bytes())
}

/// Whether a property is removed.
fn drop(name: &str, f: &Filter) -> bool {
    const ALWAYS: &[&str] = &[
        "tiff:ImageWidth",
        "tiff:ImageLength",
        "tiff:BitsPerSample",
        "tiff:Compression",
        "tiff:PhotometricInterpretation",
        "tiff:SamplesPerPixel",
        "tiff:PlanarConfiguration",
        "tiff:YCbCrSubSampling",
        "tiff:YCbCrPositioning",
        "exif:PixelXDimension",
        "exif:PixelYDimension",
        "xmpNote:HasExtendedXMP",
    ];
    const ALWAYS_PREFIX: &[&str] = &["hdrgm:", "GContainer:", "Container:", "Item:"];
    const PLACE: &[&str] = &[
        "photoshop:City",
        "photoshop:State",
        "photoshop:Country",
        "Iptc4xmpCore:Location",
        "Iptc4xmpCore:CountryCode",
        "Iptc4xmpExt:LocationCreated",
        "Iptc4xmpExt:LocationShown",
    ];
    const PEOPLE: &[&str] = &[
        "Iptc4xmpExt:PersonInImage",
        "Iptc4xmpExt:PersonInImageWDetails",
        "Iptc4xmpExt:ImageRegion",
        "mwg-rs:Regions",
        "MP:RegionInfo",
        "MPRI:Regions",
    ];
    if ALWAYS.contains(&name) || ALWAYS_PREFIX.iter().any(|p| name.starts_with(p)) {
        return true;
    }
    if !f.keep_people && PEOPLE.contains(&name) {
        return true;
    }
    if let Some(only) = f.only {
        let structural = !name.contains(':')
            || ["rdf:", "x:", "xml:", "xmlns"]
                .iter()
                .any(|p| name.starts_with(p));
        let listed = only.iter().any(|o| match o.strip_suffix('*') {
            Some(prefix) => name.starts_with(prefix),
            None => *o == name,
        });
        if !structural && !listed {
            return true;
        }
    }
    if f.keep_location {
        return false;
    }
    let local = name.rsplit(':').next().unwrap_or(name);
    (name.contains(':') && local.starts_with("GPS")) || PLACE.contains(&name)
}

/// End (exclusive) of the tag starting at `at`, honouring quoted attribute values.
fn tag_end(s: &str, at: usize) -> usize {
    let mut quote = None;
    for (n, c) in s[at..].char_indices() {
        match (quote, c) {
            (None, '"' | '\'') => quote = Some(c),
            (Some(q), c) if c == q => quote = None,
            (None, '>') => return at + n + 1,
            _ => {}
        }
    }
    s.len()
}

fn tag_name(tag: &str) -> &str {
    let t = tag.trim_start_matches('<');
    let end = t
        .find(|c: char| c.is_whitespace() || c == '>' || c == '/')
        .unwrap_or(t.len());
    &t[..end]
}

/// Skips the element whose start tag spans `start..end`: past `/>` or its matching end tag
/// (counting nested elements of the same name).
fn skip_element(s: &str, end: usize, name: &str) -> usize {
    if s[..end].ends_with("/>") {
        return end;
    }
    // `<name` / `</name` followed by something that ends the name (not a longer name).
    let find = |from: usize, pat: &str| -> Option<usize> {
        let mut k = from;
        loop {
            let at = k + s[k..].find(pat)?;
            let next = s[at + pat.len()..].chars().next();
            if next.is_none_or(|c| c.is_whitespace() || c == '>' || c == '/') {
                return Some(at);
            }
            k = at + pat.len();
        }
    };
    let open = format!("<{name}");
    let close = format!("</{name}");
    let mut depth = 1;
    let mut i = end;
    while depth > 0 {
        match (find(i, &open), find(i, &close)) {
            (Some(o), Some(c)) if o < c => {
                let e = tag_end(s, o);
                if !s[..e].ends_with("/>") {
                    depth += 1;
                }
                i = e;
            }
            (_, Some(c)) => {
                depth -= 1;
                i = tag_end(s, c);
            }
            _ => return s.len(),
        }
    }
    i
}

/// The start tag with removed properties' attributes dropped and `tiff:Orientation="…"`
/// set to 1.
fn rewrite_attributes(tag: &str, f: &Filter) -> String {
    let name = tag_name(tag);
    let head_end = tag.find(name).map_or(0, |p| p + name.len());
    let mut out = String::from(&tag[..head_end]);
    let b = tag.as_bytes();
    let mut i = head_end;
    loop {
        // Whitespace run.
        let ws_start = i;
        while i < b.len() && (b[i] as char).is_whitespace() {
            i += 1;
        }
        let ws = &tag[ws_start..i];
        // Attribute name.
        let n_start = i;
        while i < b.len() && !(b[i] as char).is_whitespace() && b[i] != b'=' && b[i] != b'>' {
            if b[i] == b'/' && b.get(i + 1) == Some(&b'>') {
                break;
            }
            i += 1;
        }
        let attr = &tag[n_start..i];
        if attr.is_empty() || b.get(i) != Some(&b'=') {
            out.push_str(&tag[ws_start..]);
            return out;
        }
        i += 1;
        let Some(&q) = b.get(i).filter(|&&q| q == b'"' || q == b'\'') else {
            out.push_str(&tag[ws_start..]);
            return out;
        };
        let v_end = tag[i + 1..]
            .find(q as char)
            .map_or(b.len(), |n| i + 1 + n + 1);
        if drop(attr, f) {
            // Dropped together with the whitespace before it.
        } else if attr == "tiff:Orientation" {
            out.push_str(ws);
            out.push_str(attr);
            out.push_str("=\"1\"");
        } else {
            out.push_str(&tag[ws_start..v_end.min(b.len())]);
        }
        i = v_end.min(b.len());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An export takes the photo's sidecar keywords over its own, without private ones.
    #[test]
    fn sidecar_keywords_go_into_exports() {
        use specular_metadata::keywords as kw;
        let d = std::env::temp_dir().join(format!("specular-xmp-kw-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let src = d.join("IMG_1.CR3");
        std::fs::write(&src, b"raw").unwrap();
        // No sidecar: the packet as it was.
        assert_eq!(with_sidecar_keywords(&src, None), None);
        let mut v = kw::Vocabulary::new();
        v.add("Secret");
        v.set_private("Secret", true);
        let keywords = ["Places|Lisbon".to_owned(), "Secret".to_owned()];
        std::fs::write(
            d.join("IMG_1.xmp"),
            kw::write_xmp(None, &keywords, Some(&v)),
        )
        .unwrap();
        let own = br#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description rdf:about="" xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:subject><rdf:Bag><rdf:li>camera</rdf:li></rdf:Bag></dc:subject></rdf:Description></rdf:RDF></x:xmpmeta>"#;
        let out = with_sidecar_keywords(&src, Some(own.to_vec())).unwrap();
        let x = specular_metadata::xmp::parse(&out);
        assert_eq!(x.get_all("dc:subject").unwrap(), ["Lisbon", "Places"]);
        assert_eq!(
            x.get_all("lr:hierarchicalSubject").unwrap(),
            ["Places|Lisbon"]
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    const PACKET: &str = r#"<?xpacket begin="" id="W5M0MpCehiHzreSzNTczkc9d"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/">
 <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about=""
    xmlns:exif="http://ns.adobe.com/exif/1.0/"
    xmlns:tiff="http://ns.adobe.com/tiff/1.0/"
    xmlns:photoshop="http://ns.adobe.com/photoshop/1.0/"
    xmlns:hdrgm="http://ns.adobe.com/hdr-gain-map/1.0/"
    tiff:Orientation="6"
    tiff:ImageWidth="4000"
    exif:GPSLatitude="48,51.5N"
    exif:GPSLongitude='2,21.0E'
    photoshop:City="Paris"
    hdrgm:Version="1.0"
    exif:ExposureTime="1/250">
   <dc:title><rdf:Alt><rdf:li xml:lang="x-default">Tower</rdf:li></rdf:Alt></dc:title>
   <exif:GPSAltitude>35/1</exif:GPSAltitude>
   <Iptc4xmpExt:LocationShown><rdf:Bag><rdf:li><Iptc4xmpExt:City>Paris</Iptc4xmpExt:City></rdf:li></rdf:Bag></Iptc4xmpExt:LocationShown>
   <GContainer:Directory><rdf:Seq><rdf:li/></rdf:Seq></GContainer:Directory>
   <exif:PixelXDimension>4000</exif:PixelXDimension>
  </rdf:Description>
  <rdf:Description rdf:about=""><tiff:Orientation>8</tiff:Orientation></rdf:Description>
 </rdf:RDF>
</x:xmpmeta>
<?xpacket end="w"?>"#;

    #[test]
    fn strips_location_and_stale_properties() {
        let out = String::from_utf8(sanitize(PACKET.as_bytes(), false).unwrap()).unwrap();
        for gone in [
            "GPS",
            "Paris",
            "photoshop:City",
            "hdrgm:",
            "GContainer",
            "ImageWidth",
            "PixelXDimension",
            "LocationShown",
        ] {
            assert!(!out.contains(gone), "{gone} left in:\n{out}");
        }
        for kept in [
            "exif:ExposureTime=\"1/250\"",
            "tiff:Orientation=\"1\"",
            "<tiff:Orientation>1</tiff:Orientation>",
            "<rdf:li xml:lang=\"x-default\">Tower</rdf:li>",
            "<?xpacket end=\"w\"?>",
            "xmlns:exif=",
        ] {
            assert!(out.contains(kept), "{kept} missing from:\n{out}");
        }
        assert!(out.contains("exif:ExposureTime=\"1/250\">"));
    }

    #[test]
    fn keeps_location_when_asked() {
        let out = String::from_utf8(sanitize(PACKET.as_bytes(), true).unwrap()).unwrap();
        assert!(out.contains("exif:GPSLatitude=\"48,51.5N\""));
        assert!(out.contains("exif:GPSLongitude='2,21.0E'"));
        assert!(out.contains("<exif:GPSAltitude>35/1</exif:GPSAltitude>"));
        assert!(out.contains("photoshop:City=\"Paris\""));
        assert!(!out.contains("hdrgm:"));
        assert!(!out.contains("tiff:ImageWidth"));
    }

    #[test]
    fn junk() {
        assert_eq!(sanitize(&[0xff, 0xfe], true), None);
        assert_eq!(sanitize(b"<x:xmpmeta/>", true), None);
        // Unterminated input doesn't panic.
        let _ = sanitize(b"<rdf:Description a=\"b", false);
        let _ = sanitize(b"<rdf:Description><exif:GPSLatitude>", false);
    }
}
