//! Motion photo and Live Photo detection.
//!
//! Embedded videos (the still's file holds a whole MP4 after the image) are found from what the
//! open already has in memory: the XMP packet and the file's bytes. Nothing here reads a file.
//!
//! - Google Motion Photo (format 1.0): XMP `GCamera:MotionPhoto="1"` and a `Container:Directory`
//!   whose `Item:Semantic="MotionPhoto"` item is located by the lengths and paddings of the
//!   items, counted back from the end of the file. Also used by Samsung, Xiaomi, OnePlus… and in
//!   HEIC stills (where the video may sit in an `mpvd` box).
//! - Google MicroVideo (the earlier "MVIMG" form): `GCamera:MicroVideo="1"` and
//!   `GCamera:MicroVideoOffset`, the video's length counted back from the end.
//! - Samsung Motion Photo: the SEF trailer (`…SEFH` directory, `SEFT` at the very end) with a
//!   `MotionPhoto_Data` entry.
//! - HEIC with a top-level `mpvd` box and no (usable) XMP.
//!
//! Every candidate range must lie inside the file and start with an `ftyp` box; a malformed
//! offset is not a motion photo. Apple Live Photos are a still plus a sibling `.MOV` of the same
//! stem ([`live_photo_sibling`]), paired by Apple's content identifier when both carry one.

use std::ops::Range;
use std::path::{Path, PathBuf};

/// The kind of moving photo.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    /// Google Motion Photo (XMP container directory), from any vendor.
    MotionPhoto,
    /// Google MicroVideo ("MVIMG"): the earlier XMP form.
    MicroVideo,
    /// Samsung Motion Photo (SEF trailer).
    Samsung,
    /// Apple Live Photo: a still and a paired QuickTime movie.
    LivePhoto,
}

impl Kind {
    /// Stable key (catalogue selectors, logs).
    pub fn key(self) -> &'static str {
        match self {
            Kind::MotionPhoto => "motion-photo",
            Kind::MicroVideo => "micro-video",
            Kind::Samsung => "samsung",
            Kind::LivePhoto => "live-photo",
        }
    }
}

/// A video inside the still's file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Embedded {
    pub kind: Kind,
    /// The movie's bytes in the file (starting at its `ftyp`).
    pub range: Range<usize>,
    /// The frame of the video the still was taken from, when stated (µs).
    pub presentation_us: Option<i64>,
}

/// Finds an embedded video. `xmp`: the still's XMP packet (standard, plus extended when the
/// reader merged it), if any.
pub fn detect_embedded(file: &[u8], xmp: Option<&str>) -> Option<Embedded> {
    if let Some(x) = xmp
        && let Some(e) = from_xmp(file, x)
    {
        return Some(e);
    }
    if let Some(range) = samsung_trailer(file) {
        return Some(Embedded {
            kind: Kind::Samsung,
            range,
            presentation_us: None,
        });
    }
    heif_mpvd(file).map(|range| Embedded {
        kind: Kind::MotionPhoto,
        range,
        presentation_us: None,
    })
}

/// Whether `range` of `file` starts a movie: an `ftyp` box of a plausible size. An `mpvd`
/// wrapper (HEIC motion photos) is unwrapped, at most twice.
fn movie_at(file: &[u8], mut range: Range<usize>) -> Option<Range<usize>> {
    for _ in 0..3 {
        let d = file.get(range.clone())?;
        if d.len() >= 16 && &d[4..8] == b"mpvd" {
            range = range.start + 8..range.end;
            continue;
        }
        let size = u32::from_be_bytes(d.get(..4)?.try_into().ok()?) as usize;
        return (d.get(4..8)? == b"ftyp" && (16..=1024).contains(&size) && size < d.len())
            .then_some(range);
    }
    None
}

/// The value of an XMP property in attribute (`name="v"`) or simple-element (`<name>v<`) form.
pub(crate) fn xmp_value<'a>(x: &'a str, name: &str) -> Option<&'a str> {
    let mut from = 0;
    while let Some(i) = x.get(from..)?.find(name) {
        let at = from + i;
        let rest = &x[at + name.len()..];
        // A property is preceded by a separator, never a name character.
        let prev = x[..at].chars().next_back();
        let standalone = !prev.is_some_and(|c| c.is_alphanumeric() || c == ':' || c == '_');
        if standalone {
            let v = if let Some(r) = rest.strip_prefix("=\"") {
                r.find('"').map(|e| &r[..e])
            } else if let Some(r) = rest.strip_prefix("='") {
                r.find('\'').map(|e| &r[..e])
            } else if let Some(r) = rest.strip_prefix('>') {
                r.find('<').map(|e| &r[..e])
            } else {
                None
            };
            if let Some(v) = v {
                return Some(v.trim());
            }
        }
        from = at + name.len();
    }
    None
}

fn xmp_int(x: &str, name: &str) -> Option<i64> {
    xmp_value(x, name)?.parse().ok()
}

/// One `Container:Item` of a motion photo's directory.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Item {
    semantic: String,
    length: Option<u64>,
    padding: u64,
}

/// The items of the XMP `Container:Directory`, in order.
fn directory(x: &str) -> Vec<Item> {
    let Some(start) = x.find("Container:Directory") else {
        return Vec::new();
    };
    let rest = &x[start..];
    let body = &rest[..rest.find("</Container:Directory>").unwrap_or(rest.len())];
    body.split("<rdf:li")
        .skip(1)
        .take(64)
        .map(|li| Item {
            semantic: xmp_value(li, "Item:Semantic")
                .unwrap_or_default()
                .to_owned(),
            length: xmp_value(li, "Item:Length").and_then(|v| v.parse().ok()),
            padding: xmp_value(li, "Item:Padding")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0),
        })
        .collect()
}

fn from_xmp(file: &[u8], x: &str) -> Option<Embedded> {
    let len = file.len() as u64;
    let ts = |n: &str| xmp_int(x, n).filter(|&t| t >= 0);
    if xmp_int(x, "GCamera:MotionPhoto") == Some(1) {
        let presentation_us = ts("GCamera:MotionPhotoPresentationTimestampUs");
        let items = directory(x);
        if let Some(i) = items.iter().position(|it| it.semantic == "MotionPhoto") {
            let after: u64 = items[i + 1..]
                .iter()
                .map(|it| it.length.unwrap_or(0).saturating_add(it.padding))
                .fold(0u64, u64::saturating_add);
            let vlen = items[i].length.unwrap_or(0);
            let end = len
                .checked_sub(after)
                .and_then(|e| e.checked_sub(items[i].padding));
            if let Some(end) = end
                && let Some(start) = end.checked_sub(vlen)
                && vlen > 0
                && let Some(range) = movie_at(file, start as usize..end as usize)
            {
                return Some(Embedded {
                    kind: Kind::MotionPhoto,
                    range,
                    presentation_us,
                });
            }
        }
        // A flag without a usable directory: a HEIC's `mpvd`, else a Samsung trailer.
        if let Some(range) = heif_mpvd(file).or_else(|| samsung_trailer(file)) {
            return Some(Embedded {
                kind: Kind::MotionPhoto,
                range,
                presentation_us,
            });
        }
    }
    if xmp_int(x, "GCamera:MicroVideo") == Some(1)
        && let Some(off) = xmp_int(x, "GCamera:MicroVideoOffset")
        && off > 0
        && let Some(start) = len.checked_sub(off as u64)
        && let Some(range) = movie_at(file, start as usize..file.len())
    {
        return Some(Embedded {
            kind: Kind::MicroVideo,
            range,
            presentation_us: ts("GCamera:MicroVideoPresentationTimestampUs"),
        });
    }
    None
}

fn le32(b: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        b.get(o..o.checked_add(4)?)?.try_into().ok()?,
    ))
}

/// Samsung's SEF trailer: `[u32 LE directory length]["SEFT"]` at the very end; the directory
/// (`SEFH`, version, count, 12-byte entries) before it. Each entry's data block lies `offset`
/// bytes before the directory and starts with its type, name length and name.
fn samsung_trailer(file: &[u8]) -> Option<Range<usize>> {
    let n = file.len();
    if n < 8 || &file[n - 4..] != b"SEFT" {
        return None;
    }
    let dir_len = le32(file, n - 8)? as usize;
    let dir_start = (n - 8).checked_sub(dir_len)?;
    let dir = &file[dir_start..n - 8];
    if !dir.starts_with(b"SEFH") {
        return None;
    }
    let count = le32(dir, 8)? as usize;
    for k in 0..count.min(256) {
        let e = 12 + 12 * k;
        let (off, size) = (le32(dir, e + 4)? as usize, le32(dir, e + 8)? as usize);
        let start = dir_start.checked_sub(off)?;
        let end = start.checked_add(size)?.min(dir_start);
        let block = file.get(start..end)?;
        let name_len = le32(block, 4)? as usize;
        let name = block.get(8..8usize.checked_add(name_len)?)?;
        if name == b"MotionPhoto_Data" {
            let v = start + 8 + name_len..end;
            if let Some(r) = movie_at(file, v) {
                return Some(r);
            }
        }
    }
    None
}

/// A HEIF file's top-level `mpvd` box (the motion photo video of a HEIC still).
fn heif_mpvd(file: &[u8]) -> Option<Range<usize>> {
    if file.get(4..8)? != b"ftyp" {
        return None;
    }
    let mut pos = 0usize;
    for _ in 0..64 {
        let h = file.get(pos..pos + 8)?;
        let size32 = u32::from_be_bytes(h[..4].try_into().ok()?) as u64;
        let (size, head) = match size32 {
            0 => ((file.len() - pos) as u64, 8),
            1 => (
                u64::from_be_bytes(file.get(pos + 8..pos + 16)?.try_into().ok()?),
                16,
            ),
            s => (s, 8),
        };
        if size < head || size > (file.len() - pos) as u64 {
            return None;
        }
        if &h[4..8] == b"mpvd" {
            return movie_at(file, pos + head as usize..pos + size as usize);
        }
        pos += size as usize;
    }
    None
}

/// The sibling movie of a Live Photo still: the same stem with a `.MOV` / `.mov` extension.
/// One or two `stat`s: callers run it off the open's hot path.
pub fn live_photo_sibling(still: &Path) -> Option<PathBuf> {
    let ext = still.extension()?.to_str()?.to_ascii_lowercase();
    if !matches!(ext.as_str(), "heic" | "heif" | "jpg" | "jpeg") {
        return None;
    }
    ["MOV", "mov", "Mov"]
        .into_iter()
        .map(|e| still.with_extension(e))
        .find(|p| p.is_file())
}

/// Apple's content identifier from a still's Exif (TIFF structure): MakerNote ("Apple iOS")
/// tag 0x0011.
pub fn apple_content_id(tiff: &[u8]) -> Option<String> {
    let t = Tiff::new(tiff)?;
    let ifd0 = t.u32(4)? as usize;
    let exif = t.u32(t.find(ifd0, 0x8769)? + 8)? as usize;
    let mn = t.find(exif, 0x927C)?;
    let (len, off) = (t.u32(mn + 4)? as usize, t.u32(mn + 8)? as usize);
    let note = tiff.get(off..off.checked_add(len)?)?;
    if !note.starts_with(b"Apple iOS\0") {
        return None;
    }
    // A version word, the byte order, then the IFD; offsets count from the note's start.
    let m = Tiff {
        b: note,
        le: note.get(12..14)? == b"II",
    };
    let e = m.find(14, 0x0011)?;
    if m.u16(e + 2)? != 2 {
        return None;
    }
    let n = m.u32(e + 4)? as usize;
    let at = if n <= 4 {
        e + 8
    } else {
        m.u32(e + 8)? as usize
    };
    let s = m.b.get(at..at.checked_add(n)?)?;
    let s = String::from_utf8_lossy(s.split(|&c| c == 0).next()?)
        .trim()
        .to_owned();
    (!s.is_empty()).then_some(s)
}

struct Tiff<'a> {
    b: &'a [u8],
    le: bool,
}

impl<'a> Tiff<'a> {
    fn new(b: &'a [u8]) -> Option<Self> {
        let le = match b.get(..2)? {
            b"II" => true,
            b"MM" => false,
            _ => return None,
        };
        let t = Tiff { b, le };
        (t.u16(2)? == 42).then_some(t)
    }
    fn u16(&self, o: usize) -> Option<u16> {
        let x: [u8; 2] = self.b.get(o..o.checked_add(2)?)?.try_into().ok()?;
        Some(if self.le {
            u16::from_le_bytes(x)
        } else {
            u16::from_be_bytes(x)
        })
    }
    fn u32(&self, o: usize) -> Option<u32> {
        let x: [u8; 4] = self.b.get(o..o.checked_add(4)?)?.try_into().ok()?;
        Some(if self.le {
            u32::from_le_bytes(x)
        } else {
            u32::from_be_bytes(x)
        })
    }
    /// The 12-byte entry of `tag` in the IFD at `ifd`.
    fn find(&self, ifd: usize, tag: u16) -> Option<usize> {
        let n = self.u16(ifd)? as usize;
        (0..n.min(1024))
            .map(|i| ifd + 2 + 12 * i)
            .find(|&e| self.u16(e) == Some(tag))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::mp4::build::{Spec, movie};

    pub fn jpeg_body() -> Vec<u8> {
        let mut j = vec![0xff, 0xd8, 0xff, 0xe0, 0, 4, 0, 0];
        j.extend(std::iter::repeat_n(0x55, 300));
        j.extend([0xff, 0xd9]);
        j
    }

    pub fn container_xmp(video_len: usize, padding: usize, gain_map: Option<usize>) -> String {
        let gm = gain_map.map_or(String::new(), |l| {
            format!(
                r#"<rdf:li rdf:parseType="Resource"><Container:Item Item:Mime="image/jpeg" Item:Semantic="GainMap" Item:Length="{l}"/></rdf:li>"#
            )
        });
        format!(
            r#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF><rdf:Description GCamera:MotionPhoto="1" GCamera:MotionPhotoVersion="1" GCamera:MotionPhotoPresentationTimestampUs="733000"><Container:Directory><rdf:Seq><rdf:li rdf:parseType="Resource"><Container:Item Item:Mime="image/jpeg" Item:Semantic="Primary" Item:Length="0" Item:Padding="{padding}"/></rdf:li>{gm}<rdf:li rdf:parseType="Resource"><Container:Item Item:Mime="video/mp4" Item:Semantic="MotionPhoto" Item:Length="{video_len}"/></rdf:li></rdf:Seq></Container:Directory></rdf:Description></rdf:RDF></x:xmpmeta>"#
        )
    }

    /// A million nested `mpvd` wrappers: refused after two, without recursing.
    #[test]
    fn nested_mpvd_wrappers() {
        let mut f = Vec::new();
        for _ in 0..1_000_000 {
            f.extend(16u32.to_be_bytes());
            f.extend(b"mpvd");
        }
        f.extend(movie(&Spec::default()));
        let r = specular_thread::on_stack(256 * 1024, || movie_at(&f, 0..f.len()));
        assert_eq!(r, None);
        // One wrapper is unwrapped.
        assert!(movie_at(&f, f.len() - movie(&Spec::default()).len() - 8..f.len()).is_some());
    }

    #[test]
    fn google_container_directory() {
        let mp4 = movie(&Spec::default());
        let mut f = jpeg_body();
        let at = f.len();
        f.extend(&mp4);
        let x = container_xmp(mp4.len(), 0, None);
        let e = detect_embedded(&f, Some(&x)).unwrap();
        assert_eq!(e.kind, Kind::MotionPhoto);
        assert_eq!(e.range, at..f.len());
        assert_eq!(e.presentation_us, Some(733_000));
    }

    #[test]
    fn google_container_with_element_form_and_a_gain_map_before_the_video() {
        // Ultra HDR + motion: primary, gain map, then the video, written as elements.
        let mp4 = movie(&Spec::default());
        let gm = vec![0xAB; 77];
        let mut f = jpeg_body();
        f.extend(&gm);
        let at = f.len();
        f.extend(&mp4);
        let x = format!(
            "<rdf:Description><GCamera:MotionPhoto>1</GCamera:MotionPhoto><Container:Directory><rdf:Seq>\
             <rdf:li><Container:Item><Item:Semantic>Primary</Item:Semantic><Item:Mime>image/jpeg</Item:Mime></Container:Item></rdf:li>\
             <rdf:li><Container:Item><Item:Semantic>GainMap</Item:Semantic><Item:Length>77</Item:Length></Container:Item></rdf:li>\
             <rdf:li><Container:Item><Item:Semantic>MotionPhoto</Item:Semantic><Item:Length>{}</Item:Length></Container:Item></rdf:li>\
             </rdf:Seq></Container:Directory></rdf:Description>",
            mp4.len()
        );
        let e = detect_embedded(&f, Some(&x)).unwrap();
        assert_eq!(e.range, at..f.len());
        assert_eq!(e.presentation_us, None);
    }

    #[test]
    fn items_after_the_video_shift_it_back() {
        let mp4 = movie(&Spec::default());
        let mut f = jpeg_body();
        let at = f.len();
        f.extend(&mp4);
        f.extend([0u8; 10]); // the video's padding
        f.extend([7u8; 40]); // a trailing item
        let x = format!(
            r#"GCamera:MotionPhoto="1"><Container:Directory><rdf:Seq>
            <rdf:li><Container:Item Item:Semantic="Primary" Item:Length="0"/></rdf:li>
            <rdf:li><Container:Item Item:Semantic="MotionPhoto" Item:Length="{}" Item:Padding="10"/></rdf:li>
            <rdf:li><Container:Item Item:Semantic="Depth" Item:Length="40"/></rdf:li>
            </rdf:Seq></Container:Directory>"#,
            mp4.len()
        );
        let e = detect_embedded(&f, Some(&x)).unwrap();
        assert_eq!(e.range, at..at + mp4.len());
    }

    #[test]
    fn micro_video() {
        let mp4 = movie(&Spec::default());
        let mut f = jpeg_body();
        let at = f.len();
        f.extend(&mp4);
        let x = format!(
            r#"GCamera:MicroVideoVersion="1" GCamera:MicroVideo="1" GCamera:MicroVideoOffset="{}" GCamera:MicroVideoPresentationTimestampUs="1500000""#,
            mp4.len()
        );
        let e = detect_embedded(&f, Some(&x)).unwrap();
        assert_eq!(e.kind, Kind::MicroVideo);
        assert_eq!(e.range, at..f.len());
        assert_eq!(e.presentation_us, Some(1_500_000));
    }

    pub fn samsung_file(mp4: &[u8]) -> (Vec<u8>, usize) {
        let mut f = jpeg_body();
        let block_start = f.len();
        let name = b"MotionPhoto_Data";
        f.extend([0, 0, 0x30, 0x0a]);
        f.extend((name.len() as u32).to_le_bytes());
        f.extend(name);
        let at = f.len();
        f.extend(mp4);
        let block_len = f.len() - block_start;
        let dir_start = f.len();
        let mut dir = b"SEFH".to_vec();
        dir.extend(106u32.to_le_bytes());
        dir.extend(1u32.to_le_bytes());
        dir.extend([0, 0, 0x30, 0x0a]);
        dir.extend(((dir_start - block_start) as u32).to_le_bytes());
        dir.extend((block_len as u32).to_le_bytes());
        f.extend(&dir);
        f.extend((dir.len() as u32).to_le_bytes());
        f.extend(b"SEFT");
        (f, at)
    }

    #[test]
    fn samsung_sef_trailer() {
        let mp4 = movie(&Spec::default());
        let (f, at) = samsung_file(&mp4);
        let e = detect_embedded(&f, None).unwrap();
        assert_eq!(e.kind, Kind::Samsung);
        assert_eq!(e.range, at..at + mp4.len());
    }

    #[test]
    fn heic_mpvd_box() {
        let mp4 = movie(&Spec::default());
        let mut f = crate::mp4::build::bx(b"ftyp", b"heic\0\0\0\0mif1heic");
        f.extend(crate::mp4::build::bx(b"meta", &[0u8; 20]));
        f.extend(crate::mp4::build::bx(b"mdat", &[1u8; 50]));
        let at = f.len() + 8;
        f.extend(crate::mp4::build::bx(b"mpvd", &mp4));
        let e = detect_embedded(&f, None).unwrap();
        assert_eq!(e.range, at..f.len());
        // The same with the XMP, whose directory length includes the mpvd header.
        let x = container_xmp(mp4.len() + 8, 0, None);
        let e = detect_embedded(&f, Some(&x)).unwrap();
        assert_eq!(e.range, at..f.len());
    }

    #[test]
    fn malformed_offsets_are_not_motion_photos() {
        let mp4 = movie(&Spec::default());
        let mut f = jpeg_body();
        f.extend(&mp4);
        for len in [
            0,
            1,
            mp4.len() - 1,
            mp4.len() + 1,
            f.len(),
            f.len() + 1,
            usize::MAX / 2,
        ] {
            let x = container_xmp(len, 0, None);
            assert_eq!(detect_embedded(&f, Some(&x)), None, "length {len}");
        }
        // An item before the video does not move it, whatever its length…
        let x = container_xmp(mp4.len(), 0, Some(u32::MAX as usize));
        assert_eq!(detect_embedded(&f, Some(&x)).unwrap().range.end, f.len());
        // …one after it does, and a wrong length there misplaces the video.
        for trail in [1, f.len(), u32::MAX as usize, usize::MAX] {
            let x = container_xmp(mp4.len(), 0, None).replace(
                "</rdf:Seq>",
                &format!(r#"<rdf:li><Container:Item Item:Semantic="Depth" Item:Length="{trail}"/></rdf:li></rdf:Seq>"#),
            );
            assert_eq!(detect_embedded(&f, Some(&x)), None, "trailing item {trail}");
        }
        for off in ["-5", "0", "99999999999", "abc", "18446744073709551615"] {
            let x = format!(r#"GCamera:MicroVideo="1" GCamera:MicroVideoOffset="{off}""#);
            assert_eq!(detect_embedded(&f, Some(&x)), None, "offset {off}");
        }
        // No flag: no motion photo even with a directory.
        let x = container_xmp(mp4.len(), 0, None).replace("MotionPhoto=\"1\"", "MotionPhoto=\"0\"");
        assert_eq!(detect_embedded(&f, Some(&x)), None);
        // A plain JPEG.
        assert_eq!(detect_embedded(&jpeg_body(), None), None);
        assert_eq!(detect_embedded(b"", Some("")), None);
    }

    #[test]
    fn corrupt_samsung_trailers_never_panic() {
        let mp4 = movie(&Spec::default());
        let (f, _) = samsung_file(&mp4);
        let tail = f.len() - 40;
        for i in tail..f.len() {
            for v in [0u8, 0xff, 0x80, 0x10] {
                let mut d = f.clone();
                d[i] = v;
                if let Some(e) = detect_embedded(&d, None) {
                    assert!(e.range.end <= d.len());
                    assert_eq!(&d[e.range.start + 4..e.range.start + 8], b"ftyp");
                }
            }
        }
        for n in 0..f.len().min(64) {
            let _ = detect_embedded(&f[f.len() - n..], None);
        }
    }

    #[test]
    fn xmp_values_match_whole_names() {
        let x = r#"GCamera:MotionPhotoVersion="2" GCamera:MotionPhoto="1""#;
        assert_eq!(xmp_value(x, "GCamera:MotionPhoto"), Some("1"));
        let x = r#"XGCamera:MotionPhoto="1""#;
        assert_eq!(xmp_value(x, "GCamera:MotionPhoto"), None);
    }

    fn apple_tiff(id: &str) -> Vec<u8> {
        // MM TIFF: IFD0 with ExifIFD pointer → Exif IFD with MakerNote → Apple note.
        let mut note = b"Apple iOS\0\0\x01MM".to_vec();
        let ifd_at = note.len();
        let s = format!("{id}\0");
        note.extend(1u16.to_be_bytes());
        note.extend(0x0011u16.to_be_bytes());
        note.extend(2u16.to_be_bytes());
        note.extend((s.len() as u32).to_be_bytes());
        note.extend(((ifd_at + 2 + 12 + 4) as u32).to_be_bytes());
        note.extend(0u32.to_be_bytes());
        note.extend(s.as_bytes());
        let mut t = b"MM\0\x2a\0\0\0\x08".to_vec();
        t.extend(1u16.to_be_bytes());
        t.extend([0x87, 0x69, 0, 4, 0, 0, 0, 1]);
        t.extend(26u32.to_be_bytes());
        t.extend(0u32.to_be_bytes());
        t.extend(1u16.to_be_bytes());
        t.extend([0x92, 0x7c, 0, 7]);
        t.extend((note.len() as u32).to_be_bytes());
        t.extend(44u32.to_be_bytes());
        t.extend(0u32.to_be_bytes());
        assert_eq!(t.len(), 44);
        t.extend(note);
        t
    }

    #[test]
    fn apple_content_identifier() {
        let id = "7A0B3C5D-1E2F-4A6B-8C9D-0E1F2A3B4C5D";
        let t = apple_tiff(id);
        assert_eq!(apple_content_id(&t).as_deref(), Some(id));
        for n in 0..t.len() {
            let _ = apple_content_id(&t[..n]);
        }
        assert_eq!(apple_content_id(b"II*\0\x08\0\0\0\0\0"), None);
    }
}
