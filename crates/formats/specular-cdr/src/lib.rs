//! CorelDRAW `.cdr` reader for Specular: what the file is, and its embedded previews.
//!
//! CorelDRAW drawings are vector pages; this crate does not draw them. It finds the bitmap
//! previews CorelDRAW stores for file browsers and says which version wrote the file:
//!
//! | Versions | Container | Preview |
//! |---|---|---|
//! | 1, 2 | `WL` header | none (refused) |
//! | 3 | RIFF `CDR ` | a BMP file in the page list's `imhd` chunk |
//! | 4 – X3 (13) | RIFF `CDR4` … `CDRD` | a DIB in the top-level `DISP` chunk |
//! | X4 (14) | ZIP with `content/riffData.cdr` | `metadata/thumbnails/*.bmp` |
//! | X5 (15) and later | ZIP with `content/root.dat` | `previews/*.png` |
//!
//! ZIP packages list a preview per page as well as one for the document; [`Cdr::previews`]
//! has the page previews first, in page order, then the document thumbnail. RIFF files have
//! one preview. Previews are a few hundred pixels at most.
//!
//! Robustness: no input panics; the RIFF walk is bounded in depth and chunk count, ZIP
//! members by [`MAX_PREVIEW`].

use std::ops::Range;

/// Largest preview member read (they are a few hundred kilobytes).
pub const MAX_PREVIEW: u64 = 64 << 20;
/// Most bytes of inflated previews kept for one file (each page has one).
pub const MAX_PREVIEWS_HELD: u64 = 256 << 20;
/// RIFF chunks looked at, at most, while searching for the preview.
const MAX_CHUNKS: usize = 4096;
/// The ZIP's XMP document (`META-INF/metadata.xml`, `metadata/metadata.xml`), at most.
const MAX_XMP: u64 = 4 << 20;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    NotCdr,
    Unsupported(String),
    Truncated(String),
    Malformed(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NotCdr => write!(f, "not a CorelDRAW file"),
            Error::Unsupported(s) => write!(f, "unsupported: {s}"),
            Error::Truncated(s) => write!(f, "truncated: {s}"),
            Error::Malformed(s) => write!(f, "malformed: {s}"),
        }
    }
}

impl std::error::Error for Error {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Container {
    /// CorelDRAW 1 and 2 (`WL`).
    Legacy,
    Riff,
    Zip,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageKind {
    Png,
    /// A complete BMP file (a DIB from `DISP` is given its file header).
    Bmp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreviewKind {
    /// The whole document's thumbnail.
    Thumbnail,
    /// Page `n` (from 1).
    Page(u32),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Bytes {
    /// Stored as is in the file.
    Range(Range<usize>),
    /// Inflated from a ZIP member, or a DIB given a BMP file header.
    Owned(Vec<u8>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Preview {
    pub kind: PreviewKind,
    pub image: ImageKind,
    pub bytes: Bytes,
}

impl Preview {
    pub fn data<'a>(&'a self, file: &'a [u8]) -> &'a [u8] {
        match &self.bytes {
            Bytes::Range(r) => file.get(r.clone()).unwrap_or_default(),
            Bytes::Owned(v) => v,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cdr {
    pub container: Container,
    /// The version as CorelDRAW numbers it times 100 (300, 1300, 1700…), when known.
    pub version: Option<u32>,
    pub previews: Vec<Preview>,
    /// The ZIP's XMP metadata document.
    pub xmp: Option<Vec<u8>>,
}

/// Whether `head` starts like a RIFF CorelDRAW file or a CorelDRAW 1/2 file. ZIP packages
/// need the directory: see [`probe_file`].
pub fn probe(head: &[u8]) -> bool {
    riff_version(head).is_some() || is_legacy(head)
}

/// [`probe`], or a ZIP with a CorelDRAW content member.
pub fn probe_file(data: &[u8]) -> bool {
    probe(data)
        || (data.starts_with(b"PK\x03\x04")
            && specular_ora::zip::Zip::parse(data).is_ok_and(|z| {
                z.find("content/riffData.cdr").is_some() || z.find("content/root.dat").is_some()
            }))
}

fn is_legacy(head: &[u8]) -> bool {
    head.len() >= 4 && head[0] == b'W' && head[1] == b'L' && head[3] == 0
}

/// The version from a RIFF type `CDR?` (`CDR ` is 3, `CDR4`… `CDR9`, then `CDRA` = 10 …).
fn riff_version(head: &[u8]) -> Option<u32> {
    if head.get(0..4)? != b"RIFF" {
        return None;
    }
    let t = head.get(8..12)?;
    if !t[..3].eq_ignore_ascii_case(b"CDR") {
        return None;
    }
    Some(match t[3].to_ascii_uppercase() {
        b' ' => 3,
        c @ b'0'..=b'9' => u32::from(c - b'0'),
        c @ b'A'..=b'Z' => u32::from(c - b'A') + 10,
        _ => return None,
    })
}

/// "CorelDRAW 9", "CorelDRAW X7", "CorelDRAW 2023" for a version times 100.
pub fn version_name(v: u32) -> String {
    let major = v / 100;
    match major {
        13..=18 => format!("CorelDRAW X{}", major - 10),
        19 => "CorelDRAW 2017".into(),
        20..=40 => format!("CorelDRAW {}", 1998 + major),
        _ => format!("CorelDRAW {major}"),
    }
}

fn le32(d: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        d.get(at..at.checked_add(4)?)?.try_into().ok()?,
    ))
}

/// Reads what the file is and where its previews are.
pub fn parse(data: &[u8]) -> Result<Cdr, Error> {
    if is_legacy(data) {
        return Err(Error::Unsupported(
            "a CorelDRAW 1 or 2 drawing: it holds no preview, and its vector pages are not drawn"
                .into(),
        ));
    }
    if let Some(v) = riff_version(data) {
        return parse_riff(data, v);
    }
    if data.starts_with(b"PK\x03\x04") {
        return parse_zip(data);
    }
    Err(Error::NotCdr)
}

fn no_preview() -> Error {
    Error::Unsupported(
        "this CorelDRAW file holds no preview, and its vector pages are not drawn".into(),
    )
}

fn parse_riff(data: &[u8], major: u32) -> Result<Cdr, Error> {
    let mut version = Some(major * 100);
    let mut preview = None;
    let mut seen = 0usize;
    // An iterative walk: (start, end, depth) of chunk lists.
    let end = (le32(data, 4).unwrap_or(0) as usize)
        .saturating_add(8)
        .min(data.len());
    let mut stack = vec![(12usize, end, 0u32)];
    while let Some((mut off, end, depth)) = stack.pop() {
        while off + 8 <= end && seen < MAX_CHUNKS {
            seen += 1;
            let id = &data[off..off + 4];
            let n = le32(data, off + 4).unwrap_or(0) as usize;
            let body = off + 8;
            let stop = body.saturating_add(n).min(end);
            match id {
                b"vrsn" if stop >= body + 2 => {
                    let v = u32::from(u16::from_le_bytes([data[body], data[body + 1]]));
                    if (100..=9900).contains(&v) {
                        version = Some(v);
                    }
                }
                b"DISP" if preview.is_none() => preview = dib_preview(&data[body..stop]),
                b"imhd" if preview.is_none() && data[body..stop].starts_with(b"BM") => {
                    preview = Some(Preview {
                        kind: PreviewKind::Thumbnail,
                        image: ImageKind::Bmp,
                        bytes: Bytes::Range(body..stop),
                    });
                }
                // The preview sits in the first page list (version 3) at most two deep.
                b"LIST" if depth < 2 && n >= 4 => stack.push((body + 4, stop, depth + 1)),
                _ => {}
            }
            if preview.is_some() && version.is_some() && depth == 0 && id == b"DISP" {
                break;
            }
            off = body.saturating_add(n).saturating_add(n & 1);
        }
    }
    let preview = preview.ok_or_else(no_preview)?;
    Ok(Cdr {
        container: Container::Riff,
        version,
        previews: vec![preview],
        xmp: None,
    })
}

/// A `DISP` chunk: a clipboard format (8, CF_DIB) and a DIB, given a BMP file header.
fn dib_preview(chunk: &[u8]) -> Option<Preview> {
    let dib = chunk.get(4..)?;
    let hsize = le32(dib, 0)? as usize;
    if !(12..=124).contains(&hsize) || dib.len() < hsize {
        return None;
    }
    let (bpp, colours, compression) = if hsize == 12 {
        (u32::from(u16::from_le_bytes([dib[10], dib[11]])), 0, 0)
    } else {
        (
            u32::from(u16::from_le_bytes([dib[14], dib[15]])),
            le32(dib, 32)?,
            le32(dib, 16)?,
        )
    };
    let entry = if hsize == 12 { 3 } else { 4 };
    let palette = if bpp <= 8 {
        let n = if colours == 0 {
            1u32 << bpp
        } else {
            colours.min(256)
        };
        n as usize * entry
    } else {
        colours.min(256) as usize * entry
    };
    // BI_BITFIELDS with a 40-byte header keeps its masks after it.
    let masks = if hsize == 40 && compression == 3 {
        12
    } else {
        0
    };
    let offset = 14 + hsize + masks + palette;
    let mut bmp = Vec::with_capacity(14 + dib.len());
    bmp.extend_from_slice(b"BM");
    bmp.extend_from_slice(&((14 + dib.len()) as u32).to_le_bytes());
    bmp.extend_from_slice(&[0; 4]);
    bmp.extend_from_slice(&(offset as u32).to_le_bytes());
    bmp.extend_from_slice(dib);
    Some(Preview {
        kind: PreviewKind::Thumbnail,
        image: ImageKind::Bmp,
        bytes: Bytes::Owned(bmp),
    })
}

fn zip_err(e: specular_ora::Error) -> Error {
    Error::Malformed(format!("CorelDRAW package: {e}"))
}

fn parse_zip(data: &[u8]) -> Result<Cdr, Error> {
    let zip = specular_ora::zip::Zip::parse(data).map_err(zip_err)?;
    let main = ["content/root.dat", "content/riffData.cdr"]
        .iter()
        .find_map(|n| zip.find(n))
        .ok_or(Error::NotCdr)?;
    // The RIFF type of the main member names the version.
    let version = zip
        .read_prefix(data, main, 12)
        .ok()
        .and_then(|h| riff_version(&h))
        .map(|v| v * 100);
    let mut pages: Vec<(u32, Preview)> = Vec::new();
    let mut held = 0u64;
    let mut thumb = None;
    for e in &zip.entries {
        let name = e.name.as_str();
        let (stem, image) = if let Some(s) = name.strip_prefix("previews/") {
            (s.strip_suffix(".png"), ImageKind::Png)
        } else if let Some(s) = name.strip_prefix("metadata/thumbnails/") {
            (s.strip_suffix(".bmp"), ImageKind::Bmp)
        } else {
            continue;
        };
        let Some(stem) = stem else {
            continue;
        };
        let kind = if stem == "thumbnail" {
            PreviewKind::Thumbnail
        } else if let Some(n) = stem
            .strip_prefix("page")
            .and_then(|n| n.parse::<u32>().ok())
        {
            PreviewKind::Page(n)
        } else {
            continue;
        };
        if e.size > MAX_PREVIEW {
            continue;
        }
        let bytes = match zip.stored_range(data, e) {
            Ok(Some(r)) => Bytes::Range(r),
            _ if held.saturating_add(e.size) > MAX_PREVIEWS_HELD => continue,
            _ => match zip.read(data, e, MAX_PREVIEW) {
                Ok(b) => {
                    held += b.len() as u64;
                    Bytes::Owned(b.into_owned())
                }
                Err(_) => continue,
            },
        };
        let p = Preview { kind, image, bytes };
        match kind {
            PreviewKind::Page(n) => pages.push((n, p)),
            PreviewKind::Thumbnail => thumb = Some(p),
        }
    }
    pages.sort_by_key(|(n, _)| *n);
    pages.dedup_by_key(|(n, _)| *n);
    let mut previews: Vec<Preview> = pages.into_iter().map(|(_, p)| p).collect();
    previews.extend(thumb);
    if previews.is_empty() {
        return Err(no_preview());
    }
    let xmp = ["META-INF/metadata.xml", "metadata/metadata.xml"]
        .iter()
        .find_map(|n| zip.find(n))
        .and_then(|e| zip.read(data, e, MAX_XMP).ok())
        .map(|b| b.into_owned())
        .filter(|x| x.windows(9).any(|w| w == b"x:xmpmeta"));
    Ok(Cdr {
        container: Container::Zip,
        version,
        previews,
        xmp,
    })
}

#[cfg(test)]
mod tests;
