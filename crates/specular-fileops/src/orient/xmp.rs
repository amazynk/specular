//! `tiff:Orientation` in XMP: the sidecar a RAW file's rotation goes to (the RAW itself is
//! never written), and in-place patches of XMP packets embedded in images.

use std::path::{Path, PathBuf};

const NS_TIFF: &str = "http://ns.adobe.com/tiff/1.0/";
const NAME: &str = "tiff:Orientation";

/// Where `name` is set in `s`: attribute (`name="v"`) or element (`<name>v</name>`).
/// Returns the value's byte range.
fn find(s: &[u8], name: &str) -> Option<std::ops::Range<usize>> {
    let n = name.as_bytes();
    let mut from = 0;
    while let Some(off) = s.get(from..)?.windows(n.len()).position(|w| w == n) {
        let i = from + off;
        from = i + n.len();
        let before = i.checked_sub(1).map(|k| s[k]);
        let after = &s[i + n.len()..];
        if before == Some(b'<') {
            if after.first() == Some(&b'>') {
                let v0 = i + n.len() + 1;
                let end = s[v0..].iter().position(|&c| c == b'<')?;
                return Some(v0..v0 + end);
            }
            continue;
        }
        if !before.is_some_and(|c| c.is_ascii_whitespace()) {
            continue;
        }
        let mut k = i + n.len();
        while s.get(k).is_some_and(|c| c.is_ascii_whitespace()) {
            k += 1;
        }
        if s.get(k) != Some(&b'=') {
            continue;
        }
        k += 1;
        while s.get(k).is_some_and(|c| c.is_ascii_whitespace()) {
            k += 1;
        }
        let q = *s.get(k)?;
        if q != b'"' && q != b'\'' {
            continue;
        }
        let v0 = k + 1;
        let len = s[v0..].iter().position(|&c| c == q)?;
        return Some(v0..v0 + len);
    }
    None
}

/// The `tiff:Orientation` of an XMP packet.
pub fn read_orientation(xmp: &[u8]) -> Option<u8> {
    let r = find(xmp, NAME)?;
    let v: u8 = std::str::from_utf8(&xmp[r]).ok()?.trim().parse().ok()?;
    (1..=8).contains(&v).then_some(v)
}

/// Set an existing one-digit `tiff:Orientation` in place (the packet keeps its length, so
/// embedded packets need no re-layout). False when there is none to patch.
pub fn patch_orientation_in_place(xmp: &mut [u8], o: u8) -> bool {
    let Some(r) = find(xmp, NAME) else {
        return false;
    };
    let v = &xmp[r.clone()];
    let trimmed: Vec<usize> = (r.clone())
        .filter(|&i| !xmp[i].is_ascii_whitespace())
        .collect();
    if trimmed.len() != 1 || !v.iter().any(u8::is_ascii_digit) {
        return false;
    }
    xmp[trimmed[0]] = b'0' + o;
    true
}

/// `existing` (a sidecar's text, or none) with `tiff:Orientation` set to `o`; everything else
/// kept byte for byte.
pub fn update(existing: Option<&str>, o: u8) -> String {
    let Some(s) = existing.filter(|s| s.contains("rdf:Description")) else {
        return minimal(o);
    };
    if let Some(r) = find(s.as_bytes(), NAME) {
        return format!("{}{o}{}", &s[..r.start], &s[r.end..]);
    }
    let mut out = s.replacen(
        "<rdf:Description",
        &format!("<rdf:Description {NAME}=\"{o}\""),
        1,
    );
    if !out.contains("xmlns:tiff=") {
        out = out.replacen(
            "<rdf:Description",
            &format!("<rdf:Description xmlns:tiff=\"{NS_TIFF}\""),
            1,
        );
    }
    out
}

fn minimal(o: u8) -> String {
    format!(
        "<?xpacket begin=\"\u{feff}\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>\n\
         <x:xmpmeta xmlns:x=\"adobe:ns:meta/\" x:xmptk=\"Specular\">\n \
         <rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n  \
         <rdf:Description rdf:about=\"\"\n   \
         xmlns:tiff=\"{NS_TIFF}\"\n   \
         {NAME}=\"{o}\"/>\n \
         </rdf:RDF>\n\
         </x:xmpmeta>\n\
         <?xpacket end=\"w\"?>\n"
    )
}

/// The sidecar of `image` and whether it exists. The rules of the gallery's rating sidecars:
/// `IMG_1.xmp` or `IMG_1.CR3.xmp` in either case of `.xmp`; when files share the stem
/// (RAW+JPEG) the stem form belongs to the RAW and the others use the long form; a new
/// sidecar takes the stem form when the image owns it.
pub fn sidecar_path(image: &Path) -> (PathBuf, bool) {
    let dir = image.parent().unwrap_or(Path::new("."));
    let name = image
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let stem = image
        .file_stem()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut sharing = 0;
    let mut raws = 0;
    let mut present: Vec<String> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            let n = e.file_name().to_string_lossy().into_owned();
            if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("xmp")) {
                present.push(n);
                continue;
            }
            if p.file_stem().is_some_and(|s| s.to_string_lossy() == stem) {
                sharing += 1;
                if crate::names::is_raw(&p) {
                    raws += 1;
                }
            }
        }
    }
    let own_stem = sharing <= 1 || (crate::names::is_raw(image) && raws == 1);
    let short = [format!("{stem}.xmp"), format!("{stem}.XMP")];
    let long = [format!("{name}.xmp"), format!("{name}.XMP")];
    let candidates: Vec<&String> = if own_stem {
        short.iter().chain(long.iter()).collect()
    } else {
        long.iter().collect()
    };
    for c in candidates {
        if present.contains(c) {
            return (dir.join(c), true);
        }
    }
    (dir.join(if own_stem { &short[0] } else { &long[0] }), false)
}

/// The orientation a RAW's sidecar sets, if any (read on open: a rotation saved for a RAW
/// lives there).
///
/// Called on every RAW open, so it looks the names up directly instead of listing the folder:
/// the long form first (`IMG_1.CR3.xmp` is always this image's), then the stem form.
pub fn sidecar_orientation(image: &Path) -> Option<u8> {
    let name = image.file_name()?.to_string_lossy().into_owned();
    let stem = image.file_stem()?.to_string_lossy().into_owned();
    let dir = image.parent().unwrap_or(Path::new("."));
    let p = [
        format!("{name}.xmp"),
        format!("{name}.XMP"),
        format!("{stem}.xmp"),
        format!("{stem}.XMP"),
    ]
    .into_iter()
    .map(|n| dir.join(n))
    .find(|p| p.is_file())?;
    let f = std::fs::File::open(&p).ok()?;
    if !f.metadata().ok()?.is_file() {
        return None;
    }
    let mut buf = Vec::new();
    use std::io::Read;
    f.take(4 << 20).read_to_end(&mut buf).ok()?;
    read_orientation(&buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attribute_and_element_forms() {
        let a = b"<rdf:Description tiff:Orientation=\"6\" xmp:Rating=\"3\"/>";
        assert_eq!(read_orientation(a), Some(6));
        let e = b"<rdf:Description><tiff:Orientation>8</tiff:Orientation></rdf:Description>";
        assert_eq!(read_orientation(e), Some(8));
        let mut m = e.to_vec();
        assert!(patch_orientation_in_place(&mut m, 3));
        assert_eq!(read_orientation(&m), Some(3));
        assert_eq!(m.len(), e.len());
        // Not a different property that ends the same way.
        assert_eq!(read_orientation(b"<x exif:tiff:OrientationX=\"6\"/>"), None);
    }

    #[test]
    fn update_keeps_the_rest() {
        let s = minimal(1);
        assert_eq!(read_orientation(s.as_bytes()), Some(1));
        let u = update(Some(&s), 6);
        assert_eq!(read_orientation(u.as_bytes()), Some(6));
        let rated = "<x:xmpmeta><rdf:RDF><rdf:Description rdf:about=\"\" xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\" xmp:Rating=\"4\"/></rdf:RDF></x:xmpmeta>";
        let u = update(Some(rated), 8);
        assert!(u.contains("xmp:Rating=\"4\""));
        assert!(u.contains("xmlns:tiff="));
        assert_eq!(read_orientation(u.as_bytes()), Some(8));
    }
}
