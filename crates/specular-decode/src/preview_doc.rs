//! Documents shown by their embedded preview only: Procreate (`.procreate`), Affinity
//! (`.afphoto`, `.afdesign`, `.afpub`), Blender (`.blend`) and SketchUp (`.skp`).
//!
//! Neither format's layers are decoded. What is shown is the PNG preview the application
//! stores for file browsers, through the PNG decoder, and the coding says "embedded preview
//! only" (the info panel then leaves out the preview's bit depth, as for a camera RAW shown by
//! its preview).
//!
//! - **Procreate**: a ZIP holding `Document.archive` (a binary property list) and
//!   `QuickLook/Thumbnail.png`, the canvas flattened at reduced size. Recognised by those two
//!   members in the ZIP directory.
//! - **Affinity**: a proprietary container starting `00 FF 4B 41`, whose preview is a PNG
//!   stored as is somewhere inside. The PNG streams in the file are found by their signature
//!   and walked chunk by chunk to `IEND` (CRCs are the PNG decoder's business); the largest
//!   well-formed one is the preview. The scan stops after [`MAX_CANDIDATES`] signatures.
//! - **Blender**: the `TEST` block near the start of the file (after its 12-byte header,
//!   blocks of 20 or 24 bytes before Blender 5, 32 after), an RGBA image stored bottom-up,
//!   written by Blender 2.5 and later when thumbnails are on. Gzip- and Zstandard-compressed
//!   files are inflated up to the block. Re-encoded as PNG for the PNG decoder.
//! - **SketchUp**: starts with the UTF-16 text "SketchUp Model"; the preview is the largest
//!   PNG stored in it, found as for Affinity.

use crate::png_dec::PngDecoder;
use crate::{Alpha, DecodeError, DecodeRequest, Decoder, Format, ImageInfo, Result, Session};
use specular_core::{CancelToken, Raster, Source};
use std::sync::Arc;

const PNG_SIG: &[u8] = b"\x89PNG\r\n\x1a\n";
const AFFINITY_MAGIC: &[u8] = b"\x00\xffKA";
const PROCREATE_THUMB: &str = "QuickLook/Thumbnail.png";
const PROCREATE_DOC: &str = "Document.archive";
/// PNG signatures looked at in an Affinity file at most.
const MAX_CANDIDATES: usize = 64;
/// Largest preview member inflated (a thumbnail is a few hundred kilobytes).
const MAX_PREVIEW: u64 = 256 << 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Procreate,
    Affinity,
    Blend,
    Sketchup,
}

const SKETCHUP_MAGIC: &[u8] = b"\xff\xfe\xff\x0eS\0k\0e\0t\0c\0h\0U\0p\0 \0M\0o\0d\0e\0l\0";

/// Bytes of a compressed `.blend` inflated at most to find its thumbnail (it comes within the
/// first blocks).
const BLEND_PREFIX: usize = 16 << 20;

/// The thumbnail of a `.blend` file as straight RGBA rows top-down: (width, height, pixels).
/// `Ok(None)`: a Blender file without one.
pub(crate) fn blend_thumbnail(
    data: &[u8],
) -> std::result::Result<Option<(u32, u32, Vec<u8>)>, String> {
    let owned;
    let b: &[u8] = if data.starts_with(&[0x1f, 0x8b]) {
        owned = specular_png::inflate_gzip_prefix(data, BLEND_PREFIX).map_err(|e| e.to_string())?;
        &owned
    } else if data.starts_with(&[0x28, 0xb5, 0x2f, 0xfd]) {
        use std::io::Read;
        let mut out = Vec::new();
        let mut dec = ruzstd::decoding::StreamingDecoder::new(data).map_err(|e| e.to_string())?;
        // A short read is fine: only the start is needed.
        let _ = dec.by_ref().take(BLEND_PREFIX as u64).read_to_end(&mut out);
        owned = out;
        &owned
    } else {
        data
    };
    if !b.starts_with(b"BLENDER") || b.len() < 12 {
        return Err("not a Blender file".into());
    }
    // Before Blender 5: "BLENDER" pointer-size ('_' 4, '-' 8) endianness ('v', 'V') version.
    // From 5: "BLENDER" header size (2 digits) '-' format (2 digits) endianness version.
    let (mut at, ptr, le, wide) = if b[7].is_ascii_digit() {
        let size = std::str::from_utf8(&b[7..9])
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .ok_or("bad Blender header")?;
        let e = *b.get(12).ok_or("bad Blender header")?;
        (size, 8, e == b'v', true)
    } else {
        (12, if b[7] == b'-' { 8 } else { 4 }, b[8] == b'v', false)
    };
    let i32_at = |o: usize| -> Option<i64> {
        let s: [u8; 4] = b.get(o..o + 4)?.try_into().ok()?;
        Some(i64::from(if le {
            i32::from_le_bytes(s)
        } else {
            i32::from_be_bytes(s)
        }))
    };
    let i64_at = |o: usize| -> Option<i64> {
        let s: [u8; 8] = b.get(o..o + 8)?.try_into().ok()?;
        Some(if le {
            i64::from_le_bytes(s)
        } else {
            i64::from_be_bytes(s)
        })
    };
    for _ in 0..10_000 {
        let Some(code) = b.get(at..at + 4) else { break };
        let (len, head) = if wide {
            (i64_at(at + 16), 32)
        } else {
            (i32_at(at + 4), 16 + ptr)
        };
        let Some(len) = len.and_then(|l| usize::try_from(l).ok()) else {
            break;
        };
        let body = at + head;
        if code == b"TEST" {
            let (Some(w), Some(h)) = (i32_at(body), i32_at(body + 4)) else {
                return Err("thumbnail cut short".into());
            };
            let (w, h) = (
                usize::try_from(w).unwrap_or(0),
                usize::try_from(h).unwrap_or(0),
            );
            if w == 0 || h == 0 || w > 4096 || h > 4096 {
                return Err("bad thumbnail size".into());
            }
            let px = b
                .get(body + 8..body + 8 + w * h * 4)
                .ok_or("thumbnail cut short")?;
            let mut out = Vec::with_capacity(w * h * 4);
            for row in px.chunks_exact(w * 4).rev() {
                out.extend_from_slice(row);
            }
            return Ok(Some((w as u32, h as u32, out)));
        }
        if code == b"ENDB" || code == b"DNA1" {
            break;
        }
        at = body.checked_add(len).ok_or("bad block size")?;
    }
    Ok(None)
}

pub(crate) struct PreviewDocDecoder(pub(crate) Kind);

fn is_procreate(data: &[u8]) -> bool {
    data.starts_with(b"PK\x03\x04")
        && specular_ora::zip::Zip::parse(data)
            .is_ok_and(|z| z.find(PROCREATE_THUMB).is_some() && z.find(PROCREATE_DOC).is_some())
}

/// The end of a PNG stream starting at `at` (just past `IEND`), with its pixel count, when the
/// chunks from the signature to `IEND` are well formed and start with an `IHDR`.
fn png_extent(data: &[u8], at: usize) -> Option<(usize, u64)> {
    let mut pos = at + PNG_SIG.len();
    let mut pixels = None;
    loop {
        let len = u32::from_be_bytes(data.get(pos..pos + 4)?.try_into().ok()?) as usize;
        let typ = data.get(pos + 4..pos + 8)?;
        let end = pos.checked_add(12)?.checked_add(len)?;
        if end > data.len() || len > (1 << 31) {
            return None;
        }
        if pixels.is_none() {
            if typ != b"IHDR" || len != 13 {
                return None;
            }
            let w = u32::from_be_bytes(data[pos + 8..pos + 12].try_into().ok()?);
            let h = u32::from_be_bytes(data[pos + 12..pos + 16].try_into().ok()?);
            pixels = Some(u64::from(w) * u64::from(h));
        }
        if !typ.iter().all(u8::is_ascii_alphabetic) {
            return None;
        }
        pos = end;
        if typ == b"IEND" {
            return Some((pos, pixels?));
        }
    }
}

/// The largest well-formed PNG stream embedded in `data`.
fn largest_png(data: &[u8]) -> Option<std::ops::Range<usize>> {
    let mut best: Option<(u64, std::ops::Range<usize>)> = None;
    let mut from = 0;
    for _ in 0..MAX_CANDIDATES {
        let Some(i) = data[from..]
            .windows(PNG_SIG.len())
            .position(|w| w == PNG_SIG)
        else {
            break;
        };
        let at = from + i;
        match png_extent(data, at) {
            Some((end, px)) => {
                if best.as_ref().is_none_or(|(b, _)| px > *b) {
                    best = Some((px, at..end));
                }
                from = end;
            }
            None => from = at + 1,
        }
    }
    best.map(|(_, r)| r)
}

/// "Affinity Photo" and so on, from the file's extension.
fn affinity_app(source: &Source) -> &'static str {
    let ext = source
        .path()
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    match ext.as_deref() {
        Some("afdesign") => "Affinity Designer",
        Some("afpub") => "Affinity Publisher",
        _ => "Affinity Photo",
    }
}

impl Decoder for PreviewDocDecoder {
    fn format(&self) -> Format {
        match self.0 {
            Kind::Procreate => Format::Procreate,
            Kind::Affinity => Format::Affinity,
            Kind::Blend => Format::Blend,
            Kind::Sketchup => Format::Sketchup,
        }
    }
    fn probe(&self, head: &[u8]) -> bool {
        match self.0 {
            // A ZIP like any other from its first bytes: the directory decides.
            Kind::Procreate => false,
            Kind::Affinity => head.starts_with(AFFINITY_MAGIC),
            // Compressed `.blend` files go by extension.
            Kind::Blend => head.starts_with(b"BLENDER"),
            Kind::Sketchup => head.starts_with(SKETCHUP_MAGIC),
        }
    }
    fn probe_file(&self, data: &[u8]) -> bool {
        match self.0 {
            Kind::Procreate => is_procreate(data),
            _ => self.probe(&data[..data.len().min(crate::PROBE_HEAD)]),
        }
    }
    fn extensions(&self) -> &'static [&'static str] {
        match self.0 {
            Kind::Procreate => &["procreate"],
            Kind::Affinity => &["afphoto", "afdesign", "afpub"],
            Kind::Blend => &["blend"],
            Kind::Sketchup => &["skp"],
        }
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let data = source.bytes();
        let (png, what) = match self.0 {
            Kind::Procreate => {
                let zip = specular_ora::zip::Zip::parse(data)
                    .map_err(|e| DecodeError::Malformed(format!("Procreate: {e}")))?;
                let e = zip.find(PROCREATE_THUMB).ok_or_else(|| {
                    DecodeError::Unsupported("Procreate file without a QuickLook thumbnail".into())
                })?;
                let sub = match zip.stored_range(data, e) {
                    Ok(Some(r)) => Source::from_slice(source.path(), source.clone(), &data[r]),
                    _ => {
                        let bytes = zip
                            .read(data, e, MAX_PREVIEW)
                            .map_err(|e| DecodeError::Malformed(format!("Procreate: {e}")))?;
                        Some(Source::from_vec(source.path(), bytes.into_owned()))
                    }
                };
                (sub, "Procreate document")
            }
            Kind::Affinity => {
                if !data.starts_with(AFFINITY_MAGIC) {
                    return Err(DecodeError::UnsupportedFormat);
                }
                let r = largest_png(data).ok_or_else(|| {
                    DecodeError::Unsupported("Affinity file without an embedded preview".into())
                })?;
                (
                    Source::from_slice(source.path(), source.clone(), &data[r]),
                    affinity_app(&source),
                )
            }
            Kind::Sketchup => {
                if !data.starts_with(SKETCHUP_MAGIC) {
                    return Err(DecodeError::Malformed("not a SketchUp model".into()));
                }
                let r = largest_png(data).ok_or_else(|| {
                    DecodeError::Unsupported(
                        "SketchUp model without an embedded preview (the model itself is not read)"
                            .into(),
                    )
                })?;
                (
                    Source::from_slice(source.path(), source.clone(), &data[r]),
                    "SketchUp model",
                )
            }
            Kind::Blend => {
                let thumb = blend_thumbnail(data)
                    .map_err(|e| DecodeError::Malformed(format!("Blender: {e}")))?
                    .ok_or_else(|| {
                        DecodeError::Unsupported(
                            "Blender file without an embedded thumbnail (saved before Blender \
                             2.5 or with thumbnails off; the scene itself is not read)"
                                .into(),
                        )
                    })?;
                let (w, h, rgba) = thumb;
                let png = specular_png::encode(w, h, specular_png::ColorType::Rgba, &rgba)
                    .map_err(|e| DecodeError::Malformed(format!("Blender thumbnail: {e}")))?;
                (Some(Source::from_vec(source.path(), png)), "Blender file")
            }
        };
        let png = png.ok_or(DecodeError::Malformed("preview outside the file".into()))?;
        let inner = PngDecoder.open(Arc::new(png))?;
        let mut info = inner.info().clone();
        info.format = self.format();
        info.coding = format!("{what}, embedded preview only (PNG {})", info.coding);
        info.embedded_preview = None;
        Ok(Arc::new(PreviewDocSession {
            source,
            info,
            inner,
        }))
    }
}

/// A session showing `inner`, a document's embedded preview decoded by another decoder, as
/// a file of `format`: the coding says what the document is and why only its preview
/// shows.
pub(crate) fn preview_session(
    source: Arc<Source>,
    inner: Arc<dyn Session>,
    format: Format,
    what: &str,
) -> Arc<dyn Session> {
    let mut info = inner.info().clone();
    info.format = format;
    info.coding = format!("{what}, embedded preview only ({})", info.coding);
    info.embedded_preview = None;
    Arc::new(PreviewDocSession {
        source,
        info,
        inner,
    })
}

/// The preview's PNG session under the document's name.
struct PreviewDocSession {
    source: Arc<Source>,
    info: ImageInfo,
    inner: Arc<dyn Session>,
}

impl Session for PreviewDocSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn source(&self) -> &Arc<Source> {
        &self.source
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        self.inner.decode_preview()
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        self.inner.decode(req, cancel)
    }
    fn prepare_regions(&self, cancel: &CancelToken) -> Result<()> {
        self.inner.prepare_regions(cancel)
    }
    fn regions_ready(&self) -> bool {
        self.inner.regions_ready()
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        self.inner.decode_memory(req)
    }
    fn open_memory(&self, levels: &[u32]) -> u64 {
        self.inner.open_memory(levels)
    }
    fn memory_bytes(&self) -> usize {
        self.inner.memory_bytes()
    }
    fn trim_memory(&self) -> usize {
        self.inner.trim_memory()
    }
    fn scaled_decode_cheap(&self) -> bool {
        self.inner.scaled_decode_cheap()
    }
    fn want_levels(&self, scales: &[u32]) {
        self.inner.want_levels(scales)
    }
    fn embedded_icc(&self) -> Option<&[u8]> {
        self.inner.embedded_icc()
    }
    fn colour_model(&self) -> Option<&'static str> {
        self.inner.colour_model()
    }
    fn alpha(&self) -> Option<Alpha> {
        self.inner.alpha()
    }
    fn colour_space(&self) -> Option<&'static str> {
        self.inner.colour_space()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(w: u32, h: u32, rgba: [u8; 4]) -> Vec<u8> {
        let px: Vec<u8> = (0..w * h).flat_map(|_| rgba).collect();
        specular_png::encode(w, h, specular_png::ColorType::Rgba, &px).unwrap()
    }

    fn crc32(d: &[u8]) -> u32 {
        let mut c = !0u32;
        for &b in d {
            c ^= u32::from(b);
            for _ in 0..8 {
                c = if c & 1 != 0 {
                    0xedb8_8320 ^ (c >> 1)
                } else {
                    c >> 1
                };
            }
        }
        !c
    }

    /// A stored-only ZIP of `members`.
    fn zip(members: &[(&str, &[u8])]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut central = Vec::new();
        for (name, body) in members {
            let off = out.len() as u32;
            let crc = crc32(body);
            let fixed = |sig: &[u8], central: bool| {
                let mut h = sig.to_vec();
                if central {
                    h.extend_from_slice(&20u16.to_le_bytes());
                }
                h.extend_from_slice(&20u16.to_le_bytes());
                h.extend_from_slice(&[0; 8]); // flags, method, time, date
                h.extend_from_slice(&crc.to_le_bytes());
                h.extend_from_slice(&(body.len() as u32).to_le_bytes());
                h.extend_from_slice(&(body.len() as u32).to_le_bytes());
                h.extend_from_slice(&(name.len() as u16).to_le_bytes());
                h.extend_from_slice(&0u16.to_le_bytes());
                h
            };
            out.extend_from_slice(&fixed(b"PK\x03\x04", false));
            out.extend_from_slice(name.as_bytes());
            out.extend_from_slice(body);
            central.extend_from_slice(&fixed(b"PK\x01\x02", true));
            central.extend_from_slice(&[0; 10]); // comment len, disk, attributes
            central.extend_from_slice(&off.to_le_bytes());
            central.extend_from_slice(name.as_bytes());
        }
        let cd = out.len() as u32;
        out.extend_from_slice(&central);
        out.extend_from_slice(b"PK\x05\x06\0\0\0\0");
        let n = (members.len() as u16).to_le_bytes();
        out.extend_from_slice(&n);
        out.extend_from_slice(&n);
        out.extend_from_slice(&(central.len() as u32).to_le_bytes());
        out.extend_from_slice(&cd.to_le_bytes());
        out.extend_from_slice(&[0, 0]);
        out
    }

    fn procreate() -> Vec<u8> {
        zip(&[
            (PROCREATE_DOC, b"bplist00"),
            (PROCREATE_THUMB, &png(6, 4, [10, 20, 30, 255])),
        ])
    }

    fn affinity() -> Vec<u8> {
        let mut d = AFFINITY_MAGIC.to_vec();
        d.extend_from_slice(b"ninAprsn and some data \x89PNG\r\n\x1a\n broken");
        d.extend_from_slice(&png(2, 2, [255, 0, 0, 255]));
        d.extend_from_slice(&[7; 33]);
        d.extend_from_slice(&png(5, 3, [0, 0, 255, 128]));
        d.extend_from_slice(&[9; 20]);
        d
    }

    /// A `.blend` (64-bit pointers, little-endian) with a REND block and a 2×2 thumbnail
    /// whose bottom row is red (stored first) and top row green.
    fn blend() -> Vec<u8> {
        let mut d = b"BLENDER-v279".to_vec();
        let block = |d: &mut Vec<u8>, code: &[u8; 4], body: &[u8]| {
            d.extend_from_slice(code);
            d.extend_from_slice(&(body.len() as i32).to_le_bytes());
            d.extend_from_slice(&[0; 8]); // old pointer
            d.extend_from_slice(&[0; 8]); // SDNA index, count
            d.extend_from_slice(body);
        };
        block(&mut d, b"REND", &[1; 72]);
        let mut t = 2i32.to_le_bytes().to_vec();
        t.extend_from_slice(&2i32.to_le_bytes());
        for _ in 0..2 {
            t.extend_from_slice(&[255, 0, 0, 255]);
        }
        for _ in 0..2 {
            t.extend_from_slice(&[0, 255, 0, 255]);
        }
        block(&mut d, b"TEST", &t);
        block(&mut d, b"ENDB", &[]);
        d
    }

    fn sketchup() -> Vec<u8> {
        let mut d = SKETCHUP_MAGIC.to_vec();
        d.extend_from_slice(&[0xff, 0xfe, 0xff, 0x0c, b'{', 0, b'1', 0, b'8', 0]);
        d.extend_from_slice(&png(3, 2, [1, 2, 3, 255]));
        d.extend_from_slice(&[0; 40]);
        d
    }

    #[test]
    fn blender_and_sketchup_previews() {
        let s = open(Kind::Blend, "a.blend", blend()).unwrap();
        assert_eq!((s.info().width, s.info().height), (2, 2));
        assert_eq!(s.info().format, Format::Blend);
        assert!(
            s.info()
                .coding
                .starts_with("Blender file, embedded preview only")
        );
        let r = s
            .decode(&DecodeRequest::full(1), &CancelToken::new())
            .unwrap();
        assert_eq!(&r.planes[0].data[..4], &[0, 255, 0, 255], "top row first");
        // Gzip-compressed (a stored deflate stream).
        let raw = blend();
        let mut gz = vec![0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 0, 255, 1];
        gz.extend_from_slice(&(raw.len() as u16).to_le_bytes());
        gz.extend_from_slice(&(!(raw.len() as u16)).to_le_bytes());
        gz.extend_from_slice(&raw);
        gz.extend_from_slice(&crc32(&raw).to_le_bytes());
        gz.extend_from_slice(&(raw.len() as u32).to_le_bytes());
        assert!(open(Kind::Blend, "a.blend", gz).is_ok());
        // Without a thumbnail: refused with the reason.
        let mut none = b"BLENDER_v248".to_vec();
        none.extend_from_slice(b"ENDB");
        none.extend_from_slice(&[0; 16]);
        let e = open(Kind::Blend, "a.blend", none)
            .err()
            .unwrap()
            .to_string();
        assert!(e.contains("thumbnail"), "{e}");
        let s = open(Kind::Sketchup, "a.skp", sketchup()).unwrap();
        assert_eq!((s.info().width, s.info().height), (3, 2));
        assert!(
            s.info()
                .coding
                .starts_with("SketchUp model, embedded preview only")
        );
        assert!(PreviewDocDecoder(Kind::Sketchup).probe(&sketchup()));
        let mut bare = SKETCHUP_MAGIC.to_vec();
        bare.extend_from_slice(&[0; 64]);
        let e = open(Kind::Sketchup, "a.skp", bare)
            .err()
            .unwrap()
            .to_string();
        assert!(e.contains("preview"), "{e}");
        for good in [blend(), sketchup()] {
            for n in 0..good.len() {
                let _ = open(Kind::Blend, "x.blend", good[..n].to_vec());
                let _ = open(Kind::Sketchup, "x.skp", good[..n].to_vec());
            }
        }
    }

    /// The corpus's Blender and SketchUp files (tools/corpus/fetch-models.sh), when present.
    #[test]
    fn corpus_previews() {
        let root =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/corpus/models");
        let mut files = Vec::new();
        for d in ["assimp/models/BLEND", "assimp/models-nonbsd/BLEND", "skp"] {
            if let Ok(rd) = std::fs::read_dir(root.join(d)) {
                files.extend(rd.flatten().map(|e| e.path()));
            }
        }
        let (mut shown, mut refused) = (0, 0);
        for p in files {
            let kind = match p.extension().and_then(|e| e.to_str()) {
                Some("blend") => Kind::Blend,
                Some("skp") => Kind::Sketchup,
                _ => continue,
            };
            match open(kind, "x", std::fs::read(&p).unwrap()) {
                Ok(s) => {
                    s.decode(&DecodeRequest::full(1), &CancelToken::new())
                        .unwrap();
                    shown += 1;
                }
                Err(e) => {
                    assert!(
                        matches!(e, DecodeError::Unsupported(_)),
                        "{}: {e}",
                        p.display()
                    );
                    refused += 1;
                }
            }
        }
        eprintln!("previews: {shown} shown, {refused} without one");
    }

    fn open(kind: Kind, name: &str, bytes: Vec<u8>) -> Result<Arc<dyn Session>> {
        PreviewDocDecoder(kind).open(Arc::new(Source::from_vec(name, bytes)))
    }

    #[test]
    fn procreate_thumbnail() {
        let d = procreate();
        assert!(PreviewDocDecoder(Kind::Procreate).probe_file(&d));
        assert!(!PreviewDocDecoder(Kind::Procreate).probe_file(&zip(&[("a.png", b"x")])));
        let s = open(Kind::Procreate, "a.procreate", d).unwrap();
        assert_eq!((s.info().width, s.info().height), (6, 4));
        assert_eq!(s.info().format, Format::Procreate);
        assert!(
            s.info()
                .coding
                .starts_with("Procreate document, embedded preview only (PNG"),
            "{}",
            s.info().coding
        );
        let r = s
            .decode(&DecodeRequest::full(1), &CancelToken::new())
            .unwrap();
        assert_eq!(&r.planes[0].data[..4], &[10, 20, 30, 255]);
    }

    #[test]
    fn affinity_largest_preview() {
        let d = affinity();
        assert!(PreviewDocDecoder(Kind::Affinity).probe(&d));
        let s = open(Kind::Affinity, "a.afdesign", d).unwrap();
        assert_eq!((s.info().width, s.info().height), (5, 3));
        assert!(
            s.info()
                .coding
                .starts_with("Affinity Designer, embedded preview only"),
            "{}",
            s.info().coding
        );
        let r = s
            .decode(&DecodeRequest::full(1), &CancelToken::new())
            .unwrap();
        assert_eq!(&r.planes[0].data[..4], &[0, 0, 255, 128]);
    }

    #[test]
    fn malformed_preview_documents_fail_cleanly() {
        let mut seed = 0x1234_5678u32;
        for (kind, good) in [(Kind::Procreate, procreate()), (Kind::Affinity, affinity())] {
            let try_one = |b: Vec<u8>| {
                if let Ok(s) = open(kind, "x", b) {
                    let _ = s.decode(&DecodeRequest::full(1), &CancelToken::new());
                }
            };
            for n in 0..good.len() {
                try_one(good[..n].to_vec());
            }
            for _ in 0..300 {
                let mut b = good.clone();
                for _ in 0..3 {
                    seed ^= seed << 13;
                    seed ^= seed >> 17;
                    seed ^= seed << 5;
                    let at = seed as usize % b.len();
                    b[at] = (seed >> 8) as u8;
                }
                try_one(b);
            }
        }
        // Many signatures, none well formed: bounded.
        let mut d = AFFINITY_MAGIC.to_vec();
        for _ in 0..10_000 {
            d.extend_from_slice(PNG_SIG);
        }
        let t = std::time::Instant::now();
        assert!(open(Kind::Affinity, "x.afphoto", d).is_err());
        assert!(t.elapsed() < std::time::Duration::from_secs(2));
        assert!(open(Kind::Procreate, "x.procreate", b"PK\x03\x04junk".to_vec()).is_err());
    }
}
