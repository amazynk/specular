//! Finding the Exif (a TIFF structure) of a file without decoding it: the APP1 segment of a
//! JPEG, the eXIf chunk of a PNG (or the legacy "Raw profile type exif" text chunk
//! ImageMagick writes), the EXIF chunk of a WebP, the `Exif` item of a HEIF/AVIF file (found
//! through `iinf`/`iloc`; also an `application/exif` `mime` item, and either decompressed
//! when it declares a `content_encoding`), the `Exif` box of a JPEG XL container, the Exif
//! image resource of a PSD/PSB, the CMT1–CMT4 blocks of a Canon CR3 (merged into one TIFF
//! structure; see [`crate::container_locate`]), and IFD0 with the Exif, Interop and GPS IFDs of
//! a TIFF-based file (TIFF, DNG, NEF, CR2, ARW, ORF, RW2…), rebuilt into a compact structure
//! without the image layout (see [`crate::tiff_exif`]). Only box and chunk headers and the
//! metadata itself are read, through [`At`], so one walk serves a mapped file (borrowing) and
//! a reader (a few small reads).
//!
//! A JPEG XL `Exif` box may be Brotli-compressed (a `brob` box with inner type `Exif`); it is
//! then decompressed (specular-brotli, capped at [`MAX_EXIF`]). [`jxl_xmp`] finds the XMP
//! packet of a JPEG XL container the same way.

use std::borrow::Cow;
use std::cell::RefCell;
use std::io::{Read, Seek, SeekFrom};

use crate::container_locate::{PSD_EXIF1, PSD_EXIF3, cr3_exif, is_cr3, is_psd, psd_resource};

/// Largest Exif accepted (HEIC maker notes run to tens of KiB; JPEG's is under 64 KiB).
pub(crate) const MAX_EXIF: u64 = 4 << 20;
/// Largest HEIF `meta` box read (grid images list many tiles; still well under this).
const MAX_META: u64 = 16 << 20;
/// Boxes, chunks or segments walked before giving up.
pub(crate) const MAX_STEPS: usize = 1 << 16;

/// Random access to a file's bytes.
pub(crate) trait At<'a> {
    fn size(&self) -> u64;
    /// Exactly `len` bytes at `off`, or None past the end.
    fn at(&self, off: u64, len: u64) -> Option<Cow<'a, [u8]>>;
}

impl<'a> At<'a> for &'a [u8] {
    fn size(&self) -> u64 {
        self.len() as u64
    }
    fn at(&self, off: u64, len: u64) -> Option<Cow<'a, [u8]>> {
        let s = usize::try_from(off).ok()?;
        let e = s.checked_add(usize::try_from(len).ok()?)?;
        self.get(s..e).map(Cow::Borrowed)
    }
}

struct Reader<R> {
    r: RefCell<(R, u64)>,
    size: u64,
}

impl<R: Read + Seek> At<'static> for Reader<R> {
    fn size(&self) -> u64 {
        self.size
    }
    fn at(&self, off: u64, len: u64) -> Option<Cow<'static, [u8]>> {
        if off.checked_add(len)? > self.size {
            return None;
        }
        let mut g = self.r.borrow_mut();
        let (r, pos) = &mut *g;
        if *pos != off {
            r.seek(SeekFrom::Start(off)).ok()?;
        }
        let mut v = vec![0; usize::try_from(len).ok()?];
        let ok = r.read_exact(&mut v).is_ok();
        *pos = if ok { off + len } else { u64::MAX };
        ok.then_some(Cow::Owned(v))
    }
}

/// The Exif TIFF structure of a whole file in memory (JPEG, PNG, WebP, HEIC, AVIF, JPEG XL,
/// PSD/PSB, CR3, TIFF-based files). Borrows when the payload is one contiguous range.
pub fn exif_tiff(bytes: &[u8]) -> Option<Cow<'_, [u8]>> {
    locate(&bytes)
}

/// [`exif_tiff`] of a file read through `r`, reading only headers and the Exif itself (for a
/// TIFF-based file, its header, IFDs and their values).
pub fn read_exif_tiff<R: Read + Seek>(mut r: R) -> Option<Vec<u8>> {
    let size = r.seek(SeekFrom::End(0)).ok()?;
    r.seek(SeekFrom::Start(0)).ok()?;
    let src = Reader {
        r: RefCell::new((r, 0)),
        size,
    };
    locate(&src).map(Cow::into_owned)
}

fn locate<'a>(s: &impl At<'a>) -> Option<Cow<'a, [u8]>> {
    let head = s.at(0, s.size().min(16))?;
    let h: &[u8] = &head;
    if h.starts_with(&[0xFF, 0xD8]) {
        jpeg(s)
    } else if h.starts_with(b"\x89PNG\r\n\x1a\n") {
        png(s)
    } else if h.len() >= 12 && &h[..4] == b"RIFF" && &h[8..12] == b"WEBP" {
        webp(s)
    } else if h.starts_with(b"\0\0\0\x0cJXL \r\n\x87\n") {
        jxl(s)
    } else if is_psd(h) {
        [PSD_EXIF1, PSD_EXIF3]
            .into_iter()
            .find_map(|id| psd_resource(s, id, MAX_EXIF).and_then(tiff_after_marker))
    } else if is_cr3(h) {
        cr3_exif(s)
    } else if h.len() >= 8 && &h[4..8] == b"ftyp" {
        isobmff(s)
    } else if is_tiff(h) || is_big_tiff(h) || is_raw_tiff(h) || is_jxr(h) {
        crate::tiff_exif::tiff_exif(s).map(Cow::Owned)
    } else if h.starts_with(RAF_MAGIC) {
        raf(s)
    } else if h.starts_with(b"\0MRM") {
        mrw(s)
    } else {
        None
    }
}

/// A Fujifilm RAF file's signature.
const RAF_MAGIC: &[u8] = b"FUJIFILMCCD-RAW ";

/// Bytes `off..off + len` of another source, as a source of their own.
struct Sub<'s, S> {
    s: &'s S,
    off: u64,
    len: u64,
}

impl<'a, S: At<'a>> At<'a> for Sub<'_, S> {
    fn size(&self) -> u64 {
        self.len
    }
    fn at(&self, off: u64, len: u64) -> Option<Cow<'a, [u8]>> {
        if off.checked_add(len)? > self.len {
            return None;
        }
        self.s.at(self.off.checked_add(off)?, len)
    }
}

/// A Fujifilm RAF's Exif: in the APP1 segment of the JPEG preview it embeds, whose offset and
/// length (big-endian) sit at bytes 84 and 88 of the header.
fn raf<'a>(s: &impl At<'a>) -> Option<Cow<'a, [u8]>> {
    let h = s.at(84, 8)?;
    let off = u64::from(u32::from_be_bytes([h[0], h[1], h[2], h[3]]));
    let len = u64::from(u32::from_be_bytes([h[4], h[5], h[6], h[7]]));
    if len < 4 || off.checked_add(len)? > s.size() {
        return None;
    }
    let j = Sub { s, off, len };
    if j.at(0, 2)?.as_ref() != [0xFF, 0xD8] {
        return None;
    }
    jpeg(&j)
}

/// A Minolta MRW's Exif: the TIFF structure of its `\0TTW` block. The file is a `\0MRM`
/// block (big-endian length) holding blocks of a 4-byte name and a big-endian length.
fn mrw<'a>(s: &impl At<'a>) -> Option<Cow<'a, [u8]>> {
    let h = s.at(4, 4)?;
    let end = 8 + u64::from(u32::from_be_bytes([h[0], h[1], h[2], h[3]]));
    let mut i = 8u64;
    for _ in 0..64 {
        if i + 8 > end {
            break;
        }
        let b = s.at(i, 8)?;
        let len = u64::from(u32::from_be_bytes([b[4], b[5], b[6], b[7]]));
        if &b[..4] == b"\0TTW" {
            let t = Sub {
                s,
                off: i + 8,
                len: len.min(s.size().saturating_sub(i + 8)),
            };
            return crate::tiff_exif::tiff_exif(&t).map(Cow::Owned);
        }
        i = i.checked_add(8 + len)?;
    }
    None
}

pub(crate) fn is_tiff(b: &[u8]) -> bool {
    b.starts_with(b"II*\0") || b.starts_with(b"MM\0*")
}

pub(crate) fn is_big_tiff(b: &[u8]) -> bool {
    b.starts_with(b"II+\0") || b.starts_with(b"MM\0+")
}

/// A JPEG XR container: a little-endian TIFF-like header with magic 0x01BC (version 1, or 0 in
/// early files) whose IFD0 carries the Exif and GPS IFD pointers, XMP and ICC under the TIFF tags.
pub(crate) fn is_jxr(b: &[u8]) -> bool {
    b.starts_with(b"II\xBC\x01") || b.starts_with(b"II\xBC\x00")
}

/// A RAW file's TIFF header with a magic number of its own: Panasonic RW2 (and Leica RWL),
/// Olympus ORF.
pub(crate) fn is_raw_tiff(b: &[u8]) -> bool {
    [b"IIU\0", b"IIRO", b"IIRS", b"MMOR"]
        .iter()
        .any(|m| b.starts_with(*m))
}

/// `c` from byte `from` on.
fn tail(c: Cow<'_, [u8]>, from: usize) -> Option<Cow<'_, [u8]>> {
    match c {
        Cow::Borrowed(b) => b.get(from..).map(Cow::Borrowed),
        Cow::Owned(mut v) => {
            if from > v.len() {
                return None;
            }
            v.drain(..from);
            Some(Cow::Owned(v))
        }
    }
}

/// A TIFF structure, after an optional `Exif\0\0` (some writers put one in PNG and WebP).
fn tiff_after_marker(c: Cow<'_, [u8]>) -> Option<Cow<'_, [u8]>> {
    let c = if c.starts_with(b"Exif\0\0") {
        tail(c, 6)?
    } else {
        c
    };
    is_tiff(&c).then_some(c)
}

/// The body of an ISOBMFF `Exif` item or JPEG XL `Exif` box: a big-endian offset to the TIFF
/// header, then the data. Lenient like other readers: a wrong offset falls back to the first
/// TIFF header in the body.
fn tiff_after_offset(c: Cow<'_, [u8]>) -> Option<Cow<'_, [u8]>> {
    let off = c
        .get(..4)
        .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as usize)?;
    let at = 4usize
        .checked_add(off)
        .filter(|&a| c.get(a..).is_some_and(is_tiff))
        .or_else(|| (4..c.len().saturating_sub(4)).find(|&i| is_tiff(&c[i..])))?;
    tail(c, at)
}

pub(crate) fn jpeg<'a>(s: &impl At<'a>) -> Option<Cow<'a, [u8]>> {
    let mut i = 2u64;
    for _ in 0..MAX_STEPS {
        let h = s.at(i, 4)?;
        if h[0] != 0xFF {
            return None;
        }
        let m = h[1];
        if m == 0xFF {
            i += 1;
            continue;
        }
        if m == 0xDA || m == 0xD9 {
            return None;
        }
        if m == 0x01 || (0xD0..=0xD7).contains(&m) {
            i += 2;
            continue;
        }
        let len = u64::from(u16::from_be_bytes([h[2], h[3]]));
        if len < 2 {
            return None;
        }
        if m == 0xE1 && len >= 8 + 8 && s.at(i + 4, 6)?.as_ref() == b"Exif\0\0" {
            return tiff_after_marker(s.at(i + 10, len - 8)?);
        }
        i += 2 + len;
    }
    None
}

fn png<'a>(s: &impl At<'a>) -> Option<Cow<'a, [u8]>> {
    const RAW_EXIF: &[u8] = b"Raw profile type exif\0";
    const RAW_APP1: &[u8] = b"Raw profile type APP1\0";
    let mut i = 8u64;
    // ImageMagick's legacy hex text form; eXIf wins when both are present.
    let mut legacy = None;
    for _ in 0..MAX_STEPS {
        let Some(h) = s.at(i, 8) else { break };
        let len = u64::from(u32::from_be_bytes([h[0], h[1], h[2], h[3]]));
        let kind = [h[4], h[5], h[6], h[7]];
        let data = i + 8;
        match &kind {
            b"IEND" => break,
            b"eXIf" if len <= MAX_EXIF => return tiff_after_marker(s.at(data, len)?),
            b"tEXt" | b"zTXt" if legacy.is_none() && len <= 3 * MAX_EXIF => {
                let kw = s.at(data, len.min(RAW_EXIF.len() as u64))?;
                if kw.as_ref() == RAW_EXIF || kw.as_ref() == RAW_APP1 {
                    legacy = Some((
                        data + RAW_EXIF.len() as u64,
                        len - RAW_EXIF.len() as u64,
                        kind,
                    ));
                }
            }
            _ => {}
        }
        i = data + len + 4;
    }
    let (off, len, kind) = legacy?;
    let body = s.at(off, len)?;
    let text = if &kind == b"zTXt" {
        // Compression method 0 (zlib).
        if body.first() != Some(&0) {
            return None;
        }
        Cow::Owned(specular_png::inflate_zlib(&body[1..], 3 * MAX_EXIF as usize).ok()?)
    } else {
        body
    };
    raw_profile(&text).and_then(|b| tiff_after_marker(Cow::Owned(b)))
}

/// The bytes of an ImageMagick raw profile: "\n<name>\n<spaces><length>\n<hex lines>".
fn raw_profile(t: &[u8]) -> Option<Vec<u8>> {
    let mut w = t
        .split(|b| b.is_ascii_whitespace())
        .filter(|w| !w.is_empty());
    let _name = w.next()?;
    let len: usize = std::str::from_utf8(w.next()?).ok()?.parse().ok()?;
    if len as u64 > MAX_EXIF {
        return None;
    }
    let hex = |c: u8| (c as char).to_digit(16).map(|d| d as u8);
    let mut out = Vec::with_capacity(len);
    let mut hi = None;
    for c in w.flatten() {
        let d = hex(*c)?;
        match hi.take() {
            None => hi = Some(d),
            Some(h) => {
                out.push(h << 4 | d);
                if out.len() == len {
                    return Some(out);
                }
            }
        }
    }
    None
}

fn webp<'a>(s: &impl At<'a>) -> Option<Cow<'a, [u8]>> {
    let mut i = 12u64;
    for _ in 0..MAX_STEPS {
        let h = s.at(i, 8)?;
        let len = u64::from(u32::from_le_bytes([h[4], h[5], h[6], h[7]]));
        if &h[..4] == b"EXIF" {
            return (len <= MAX_EXIF)
                .then(|| s.at(i + 8, len))
                .flatten()
                .and_then(tiff_after_marker);
        }
        i += 8 + len + (len & 1);
    }
    None
}

/// The box at `i` of an ISOBMFF-style file: type, body offset and body length.
fn box_at<'a>(s: &impl At<'a>, i: u64) -> Option<([u8; 4], u64, u64)> {
    let h = s.at(i, 8)?;
    let kind = [h[4], h[5], h[6], h[7]];
    let (size, head) = match u32::from_be_bytes([h[0], h[1], h[2], h[3]]) {
        0 => (s.size() - i, 8),
        1 => {
            let l = s.at(i + 8, 8)?;
            (u64::from_be_bytes(l.as_ref().try_into().ok()?), 16)
        }
        n => (u64::from(n), 8),
    };
    Some((kind, i + head, size.checked_sub(head)?))
}

fn jxl<'a>(s: &impl At<'a>) -> Option<Cow<'a, [u8]>> {
    tiff_after_offset(jxl_box(s, *b"Exif", MAX_EXIF)?)
}

/// The body of the first top-level JPEG XL container box of type `kind`, plain or
/// Brotli-compressed (`brob`), if it is at most `max` bytes (compressed and decompressed).
pub(crate) fn jxl_box<'a>(s: &impl At<'a>, kind: [u8; 4], max: u64) -> Option<Cow<'a, [u8]>> {
    let mut i = 0u64;
    for _ in 0..MAX_STEPS {
        let (k, body, len) = box_at(s, i)?;
        if k == kind {
            return if len <= max { s.at(body, len) } else { None };
        }
        if &k == b"brob" && len >= 4 && s.at(body, 4)?.as_ref() == kind {
            if len > max + 4 {
                return None;
            }
            let packed = s.at(body + 4, len - 4)?;
            let cap = usize::try_from(max).ok()?;
            return specular_brotli::decompress(&packed, cap)
                .ok()
                .map(Cow::Owned);
        }
        i = body.checked_add(len)?;
    }
    None
}

/// Largest XMP packet accepted.
pub(crate) const MAX_XMP: u64 = 4 << 20;

/// The XMP packet of a JPEG XL container: its `xml ` box, plain or Brotli-compressed (`brob`).
/// None for a bare codestream or another format.
pub fn jxl_xmp(bytes: &[u8]) -> Option<Cow<'_, [u8]>> {
    let s = &bytes;
    if !bytes.starts_with(b"\0\0\0\x0cJXL \r\n\x87\n") {
        return None;
    }
    jxl_box(s, *b"xml ", MAX_XMP)
}

/// The body of the top-level `meta` box of an ISOBMFF file (HEIF, AVIF), after its version and
/// flags.
pub(crate) fn heif_meta<'a>(s: &impl At<'a>) -> Option<Cow<'a, [u8]>> {
    let mut i = 0u64;
    for _ in 0..1024 {
        let (kind, body, len) = box_at(s, i)?;
        if &kind == b"meta" {
            if !(4..=MAX_META).contains(&len) {
                return None;
            }
            return s.at(body + 4, len - 4);
        }
        i = body.checked_add(len)?;
    }
    None
}

/// The Exif of a HEIF/AVIF file: an `Exif` item (a 4-byte offset, then the TIFF structure),
/// else a `mime` item of type `application/exif`. A `content_encoding` (deflate, zlib, gzip or
/// Brotli) is undone: `mime` items declare one after their content type, and an `Exif` item
/// written the same way (fields after its name) is read alike.
fn isobmff<'a>(s: &impl At<'a>) -> Option<Cow<'a, [u8]>> {
    let meta = heif_meta(s)?;
    let m = HeifMeta::parse(&meta);
    // (is an `Exif` item, encoding) of the matching items, in the order `items` lists them.
    let mut found = Vec::new();
    let ids = m.items(|kind, rest| {
        // item_name, then content_type and content_encoding.
        let mut f = rest.split(|&b| b == 0);
        let (_name, ctype, enc) = (f.next(), f.next(), f.next().unwrap_or_default());
        let hit = match kind {
            b"Exif" => Some((true, Encoding::of(enc).unwrap_or(Encoding::Identity))),
            b"mime" if ctype.is_some_and(|t| t.eq_ignore_ascii_case(b"application/exif")) => {
                Encoding::of(enc).map(|e| (false, e))
            }
            _ => None,
        };
        found.extend(hit);
        hit.is_some()
    });
    // An `Exif` item before a `mime` one; of several, the one describing the primary item.
    let exif_type = found.iter().any(|f| f.0);
    let pick: Vec<u32> = ids
        .iter()
        .zip(&found)
        .filter(|(_, f)| f.0 == exif_type)
        .map(|(&id, _)| id)
        .collect();
    let id = m.prefer_primary(&pick)?;
    let at = ids.iter().position(|&i| i == id)?;
    let data = m.item_data(s, id, MAX_EXIF)?;
    let data = found.get(at)?.1.decode(data, MAX_EXIF)?;
    if is_tiff(&data) || data.starts_with(b"Exif\0\0") {
        tiff_after_marker(data)
    } else {
        tiff_after_offset(data)
    }
}

/// The `content_encoding` of a HEIF item (HTTP content codings).
#[derive(Debug, Clone, Copy)]
pub(crate) enum Encoding {
    Identity,
    /// "deflate": zlib per RFC 9110, though some writers store raw DEFLATE; both are read.
    Deflate,
    /// "gzip" (RFC 1952).
    Gzip,
    /// "br".
    Brotli,
}

impl Encoding {
    pub(crate) fn of(name: &[u8]) -> Option<Encoding> {
        match name {
            b"" | b"identity" => Some(Encoding::Identity),
            b"deflate" | b"zlib" | b"compress_zlib" => Some(Encoding::Deflate),
            b"gzip" | b"x-gzip" => Some(Encoding::Gzip),
            b"br" => Some(Encoding::Brotli),
            _ => None,
        }
    }

    /// `data` decoded, if at most `max` bytes.
    pub(crate) fn decode(self, data: Cow<'_, [u8]>, max: u64) -> Option<Cow<'_, [u8]>> {
        let cap = usize::try_from(max).ok()?;
        let out = match self {
            Encoding::Identity => return Some(data),
            Encoding::Deflate => specular_png::inflate_zlib(&data, cap)
                .or_else(|_| specular_png::inflate_raw(&data, cap))
                .ok()?,
            Encoding::Gzip => specular_png::inflate_gzip(&data, cap).ok()?,
            Encoding::Brotli => specular_brotli::decompress(&data, cap).ok()?,
        };
        (out.len() <= cap).then_some(Cow::Owned(out))
    }
}

/// Big-endian reads over an in-memory box.
struct Cur<'b> {
    b: &'b [u8],
    p: usize,
}

impl<'b> Cur<'b> {
    fn take(&mut self, n: usize) -> Option<&'b [u8]> {
        let s = self.b.get(self.p..self.p.checked_add(n)?)?;
        self.p += n;
        Some(s)
    }
    fn uint(&mut self, n: usize) -> Option<u64> {
        Some(
            self.take(n)?
                .iter()
                .fold(0u64, |a, &b| a << 8 | u64::from(b)),
        )
    }
    fn u16(&mut self) -> Option<u32> {
        self.uint(2).map(|v| v as u32)
    }
    fn u32(&mut self) -> Option<u32> {
        self.uint(4).map(|v| v as u32)
    }
    /// A 16-bit id when `small`, else a 32-bit one.
    fn id(&mut self, small: bool) -> Option<u32> {
        if small { self.u16() } else { self.u32() }
    }
    fn rest(&self) -> &'b [u8] {
        self.b.get(self.p..).unwrap_or_default()
    }
}

/// The child boxes of an in-memory box body: (type, body).
fn children(mut b: &[u8]) -> impl Iterator<Item = ([u8; 4], &[u8])> {
    std::iter::from_fn(move || {
        let mut c = Cur { b, p: 0 };
        let size = c.u32()? as u64;
        let kind: [u8; 4] = c.take(4)?.try_into().ok()?;
        let (size, head) = match size {
            0 => (b.len() as u64, 8),
            1 => (c.uint(8)?, 16),
            n => (n, 8),
        };
        let size = usize::try_from(size).ok().filter(|&n| n >= head)?;
        let body = b.get(head..size)?;
        b = &b[size..];
        Some((kind, body))
    })
}

/// The boxes of a HEIF `meta` box body (after its version and flags) that locate items and
/// their properties.
pub(crate) struct HeifMeta<'m> {
    /// The primary item (`pitm`).
    primary: Option<u32>,
    iinf: Option<&'m [u8]>,
    iloc: Option<&'m [u8]>,
    iref: Option<&'m [u8]>,
    idat: Option<&'m [u8]>,
    iprp: Option<&'m [u8]>,
}

impl<'m> HeifMeta<'m> {
    pub(crate) fn parse(meta: &'m [u8]) -> Self {
        let mut m = HeifMeta {
            primary: None,
            iinf: None,
            iloc: None,
            iref: None,
            idat: None,
            iprp: None,
        };
        for (kind, body) in children(meta) {
            match &kind {
                b"pitm" => {
                    let mut c = Cur { b: body, p: 0 };
                    m.primary = c.uint(4).and_then(|v| c.id(v >> 24 == 0));
                }
                b"iinf" => m.iinf = Some(body),
                b"iloc" => m.iloc = Some(body),
                b"iref" => m.iref = Some(body),
                b"idat" => m.idat = Some(body),
                b"iprp" => m.iprp = Some(body),
                _ => {}
            }
        }
        m
    }

    /// The ids of the items whose `infe` (version 2 or 3) passes `want(item_type, rest)`, where
    /// `rest` is the entry after the type (the item name, then for `mime` items the content
    /// type and encoding).
    pub(crate) fn items(&self, mut want: impl FnMut(&[u8], &'m [u8]) -> bool) -> Vec<u32> {
        let mut out = Vec::new();
        let Some(iinf) = self.iinf else { return out };
        let mut c = Cur { b: iinf, p: 0 };
        let Some(v) = c.uint(4).map(|v| v >> 24) else {
            return out;
        };
        if c.id(v == 0).is_none() {
            return out;
        }
        for (kind, body) in children(c.rest()) {
            if &kind != b"infe" {
                continue;
            }
            let mut e = Cur { b: body, p: 0 };
            let Some(v) = e.uint(4).map(|v| v >> 24) else {
                continue;
            };
            if v < 2 {
                continue;
            }
            let Some(id) = e.id(v == 2) else { continue };
            if e.u16().is_none() {
                continue;
            }
            if let Some(t) = e.take(4)
                && want(t, e.rest())
            {
                out.push(id);
            }
        }
        out
    }

    /// Of `ids`, the one that describes (`cdsc`) the primary item, else the first.
    pub(crate) fn prefer_primary(&self, ids: &[u32]) -> Option<u32> {
        let first = *ids.first()?;
        let (Some(iref), Some(primary)) = (self.iref, self.primary) else {
            return Some(first);
        };
        let mut c = Cur { b: iref, p: 0 };
        let small = c.uint(4).map(|v| v >> 24) == Some(0);
        for (kind, body) in children(c.rest()) {
            let mut r = Cur { b: body, p: 0 };
            let Some(from) = r.id(small) else { continue };
            let n = r.u16().unwrap_or(0);
            if &kind != b"cdsc" || !ids.contains(&from) {
                continue;
            }
            for _ in 0..n {
                match r.id(small) {
                    Some(to) if to == primary => return Some(from),
                    Some(_) => {}
                    // Cut short: the rest of the count holds nothing.
                    None => break,
                }
            }
        }
        Some(first)
    }

    /// The data of item `want` (at most `max` bytes), from the file or from `idat`.
    pub(crate) fn item_data<'a>(
        &self,
        s: &impl At<'a>,
        want: u32,
        max: u64,
    ) -> Option<Cow<'a, [u8]>> {
        let mut c = Cur {
            b: self.iloc?,
            p: 0,
        };
        let v = (c.uint(4)? >> 24) as u8;
        let sizes = c.u16()?;
        let (off_size, len_size, base_size) = (
            (sizes >> 12) as usize,
            (sizes >> 8 & 15) as usize,
            (sizes >> 4 & 15) as usize,
        );
        let idx_size = if v == 1 || v == 2 {
            (sizes & 15) as usize
        } else {
            0
        };
        let count = if v < 2 { c.u16()? } else { c.u32()? };
        for _ in 0..count {
            let id = c.id(v < 2)?;
            let method = if v == 1 || v == 2 { c.u16()? & 15 } else { 0 };
            c.u16()?; // data_reference_index
            let base = c.uint(base_size)?;
            let mut n = c.u16()?;
            if idx_size + off_size + len_size == 0 {
                // Extents of no bytes in the box are all (base, 0): one stands for them, so
                // a count of them cannot spin without consuming anything.
                n = n.min(1);
            }
            let mut extents = Vec::with_capacity(n.min(64) as usize);
            for _ in 0..n {
                c.uint(idx_size)?;
                let off = c.uint(off_size)?;
                let len = c.uint(len_size)?;
                extents.push((base.checked_add(off)?, len));
            }
            if id != want {
                continue;
            }
            return match method {
                0 => read_extents(s, &extents, max),
                1 => {
                    let idat = self.idat?;
                    let mut v = Vec::new();
                    for &(off, len) in &extents {
                        let off = usize::try_from(off).ok()?;
                        let end = if len == 0 {
                            idat.len()
                        } else {
                            off.checked_add(usize::try_from(len).ok()?)?
                        };
                        v.extend_from_slice(idat.get(off..end)?);
                        if v.len() as u64 > max {
                            return None;
                        }
                    }
                    Some(Cow::Owned(v))
                }
                _ => None,
            };
        }
        None
    }

    /// The ICC profile of a `colr` property (`prof` or `rICC`) of the primary item (`ipma`),
    /// else of the first such property.
    pub(crate) fn icc(&self) -> Option<&'m [u8]> {
        let mut ipco = None;
        let mut ipma = Vec::new();
        for (kind, body) in children(self.iprp?) {
            match &kind {
                b"ipco" if ipco.is_none() => ipco = Some(body),
                b"ipma" => ipma.push(body),
                _ => {}
            }
        }
        // Property indices are 15 bits at most.
        let props: Vec<([u8; 4], &[u8])> = children(ipco?).take(1 << 15).collect();
        let icc_of = |(kind, body): &([u8; 4], &'m [u8])| -> Option<&'m [u8]> {
            (kind == b"colr" && body.len() > 4 && matches!(&body[..4], b"prof" | b"rICC"))
                .then(|| &body[4..])
        };
        if let Some(primary) = self.primary {
            for ma in ipma {
                let mut c = Cur { b: ma, p: 0 };
                let Some(vf) = c.u32() else { continue };
                let (v, wide) = (vf >> 24, vf & 1 != 0);
                let Some(n) = c.u32() else { continue };
                'entries: for _ in 0..n {
                    let Some(id) = c.id(v < 1) else { break };
                    let Some(k) = c.uint(1) else { break };
                    for _ in 0..k {
                        let idx = if wide {
                            c.u16().map(|x| x & 0x7FFF)
                        } else {
                            c.uint(1).map(|x| x as u32 & 0x7F)
                        };
                        let Some(idx) = idx else { break 'entries };
                        if id == primary
                            && let Some(icc) = (idx as usize)
                                .checked_sub(1)
                                .and_then(|i| props.get(i))
                                .and_then(icc_of)
                        {
                            return Some(icc);
                        }
                    }
                }
            }
        }
        props.iter().find_map(icc_of)
    }
}

/// File extents (`len` 0: to the end of the file), joined, if at most `max` bytes.
fn read_extents<'a>(s: &impl At<'a>, extents: &[(u64, u64)], max: u64) -> Option<Cow<'a, [u8]>> {
    let len_of = |&(off, len): &(u64, u64)| {
        if len == 0 {
            s.size().checked_sub(off)
        } else {
            Some(len)
        }
    };
    let total = extents
        .iter()
        .try_fold(0u64, |a, e| a.checked_add(len_of(e)?))?;
    if total > max {
        return None;
    }
    if let [e] = extents {
        return s.at(e.0, len_of(e)?);
    }
    let mut v = Vec::with_capacity(total as usize);
    for e in extents {
        v.extend_from_slice(&s.at(e.0, len_of(e)?)?);
    }
    Some(Cow::Owned(v))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// IFD0 with one entry (Make = "Cam"), little-endian.
    fn tiff() -> Vec<u8> {
        let mut t = b"II\x2a\0\x08\0\0\0\x01\0".to_vec();
        t.extend_from_slice(&[0x0F, 0x01, 2, 0, 4, 0, 0, 0, b'C', b'a', b'm', 0]);
        t.extend_from_slice(&[0; 4]);
        t
    }

    fn bx(kind: &[u8], body: &[u8]) -> Vec<u8> {
        let mut b = ((body.len() + 8) as u32).to_be_bytes().to_vec();
        b.extend_from_slice(kind);
        b.extend_from_slice(body);
        b
    }

    fn both(file: &[u8]) -> (Option<Vec<u8>>, Option<Vec<u8>>) {
        (
            exif_tiff(file).map(Cow::into_owned),
            read_exif_tiff(std::io::Cursor::new(file)),
        )
    }

    fn check(file: &[u8]) {
        let (a, b) = both(file);
        assert_eq!(a.as_deref(), Some(&tiff()[..]));
        assert_eq!(b.as_deref(), Some(&tiff()[..]));
        for n in 0..file.len() {
            let _ = both(&file[..n]);
        }
    }

    /// A box whose size is smaller than its own header (found by fuzzing) is the end of the
    /// walk, not an underflow.
    #[test]
    fn box_smaller_than_its_header() {
        for size in 1u32..8 {
            let mut f = bx(b"JXL ", b"\r\n\x87\n");
            f.extend_from_slice(&size.to_be_bytes());
            f.extend_from_slice(b"Exif");
            f.extend_from_slice(&[0; 8]);
            assert_eq!(both(&f), (None, None));
            assert!(super::jxl_xmp(&f).is_none());
            let mut h = bx(b"ftyp", b"heic\0\0\0\0mif1");
            h.extend_from_slice(&size.to_be_bytes());
            h.extend_from_slice(b"meta");
            h.extend_from_slice(&[0; 8]);
            assert_eq!(both(&h), (None, None));
        }
    }

    #[test]
    fn jpeg_app1() {
        let mut j = vec![0xFF, 0xD8, 0xFF, 0xE0, 0, 4, 0, 0, 0xFF, 0xE1];
        j.extend_from_slice(&((tiff().len() + 8) as u16).to_be_bytes());
        j.extend_from_slice(b"Exif\0\0");
        j.extend_from_slice(&tiff());
        j.extend_from_slice(&[0xFF, 0xD9]);
        check(&j);
    }

    fn png_chunk(kind: &[u8], data: &[u8]) -> Vec<u8> {
        let mut c = (data.len() as u32).to_be_bytes().to_vec();
        c.extend_from_slice(kind);
        c.extend_from_slice(data);
        c.extend_from_slice(&[0; 4]);
        c
    }

    fn png(chunks: &[Vec<u8>]) -> Vec<u8> {
        let mut f = b"\x89PNG\r\n\x1a\n".to_vec();
        f.extend(png_chunk(b"IHDR", &[0; 13]));
        f.extend(png_chunk(b"IDAT", &[0; 64]));
        for c in chunks {
            f.extend_from_slice(c);
        }
        f.extend(png_chunk(b"IEND", &[]));
        f
    }

    #[test]
    fn png_exif_and_raw_profile() {
        check(&png(&[png_chunk(b"eXIf", &tiff())]));
        // ImageMagick's hex form, plain and compressed, with the Exif marker inside.
        let mut payload = b"Exif\0\0".to_vec();
        payload.extend(tiff());
        let mut text = format!("\nexif\n{:8}\n", payload.len());
        for (i, b) in payload.iter().enumerate() {
            text.push_str(&format!("{b:02x}"));
            if i % 36 == 35 {
                text.push('\n');
            }
        }
        text.push('\n');
        let mut t = b"Raw profile type exif\0".to_vec();
        t.extend_from_slice(text.as_bytes());
        check(&png(&[png_chunk(b"tEXt", &t)]));
        let mut z = b"Raw profile type APP1\0\0".to_vec();
        z.extend(specular_png_zlib(text.as_bytes()));
        check(&png(&[png_chunk(b"zTXt", &z)]));
        // eXIf wins.
        let (a, _) = both(&png(&[png_chunk(b"tEXt", &t), png_chunk(b"eXIf", &tiff())]));
        assert_eq!(a.as_deref(), Some(&tiff()[..]));
        assert_eq!(both(&png(&[])).0, None);
    }

    /// A stored (uncompressed) zlib stream.
    fn specular_png_zlib(data: &[u8]) -> Vec<u8> {
        let mut z = vec![0x78, 0x01, 1];
        z.extend_from_slice(&(data.len() as u16).to_le_bytes());
        z.extend_from_slice(&(!(data.len() as u16)).to_le_bytes());
        z.extend_from_slice(data);
        let (mut a, mut b) = (1u32, 0u32);
        for &x in data {
            a = (a + u32::from(x)) % 65521;
            b = (b + a) % 65521;
        }
        z.extend_from_slice(&(b << 16 | a).to_be_bytes());
        z
    }

    #[test]
    fn webp_chunk() {
        let mut body = b"WEBP".to_vec();
        for (k, d) in [(&b"VP8X"[..], vec![8u8; 10]), (b"VP8L", vec![0; 5])] {
            body.extend_from_slice(k);
            body.extend_from_slice(&(d.len() as u32).to_le_bytes());
            body.extend_from_slice(&d);
            if d.len() % 2 == 1 {
                body.push(0);
            }
        }
        body.extend_from_slice(b"EXIF");
        body.extend_from_slice(&(tiff().len() as u32 + 6).to_le_bytes());
        body.extend_from_slice(b"Exif\0\0");
        body.extend(tiff());
        let mut f = b"RIFF".to_vec();
        f.extend_from_slice(&(body.len() as u32).to_le_bytes());
        f.extend(body);
        check(&f);
    }

    #[test]
    fn jxl_box_and_brob() {
        let sig = bx(b"JXL ", b"\r\n\x87\n");
        let mut exif = vec![0, 0, 0, 2, 0xAA, 0xBB];
        exif.extend(tiff());
        let mut f = sig.clone();
        f.extend(bx(b"ftyp", b"jxl \0\0\0\0jxl "));
        f.extend(bx(b"jxlc", &[0xFF, 0x0A, 0, 0]));
        f.extend(bx(b"Exif", &exif));
        check(&f);
        // Brotli-compressed Exif, here a stored (uncompressed) meta-block: WBITS 16, then a
        // meta-block of MLEN bytes (ISLAST 0, MNIBBLES 4, MLEN - 1, ISUNCOMPRESSED 1), byte
        // aligned, then the empty last meta-block.
        let brob = |data: &[u8]| {
            let m = data.len() as u32 - 1;
            let bits = (m << 4) | (1 << 20);
            let mut b = b"Exif".to_vec();
            b.extend_from_slice(&bits.to_le_bytes()[..3]);
            b.extend_from_slice(data);
            b.push(0b11);
            b
        };
        let mut f = sig.clone();
        f.extend(bx(b"ftyp", b"jxl \0\0\0\0jxl "));
        f.extend(bx(b"brob", &brob(&exif)));
        f.extend(bx(b"jxlc", &[0xFF, 0x0A, 0, 0]));
        check(&f);
        // Not valid Brotli.
        let mut f = sig;
        f.extend(bx(b"brob", b"Exif\x1b\x00"));
        assert_eq!(both(&f), (None, None));
    }

    /// A HEIF file with two Exif items, the second describing the primary item (1), in the
    /// file (`method` 0, split in two extents) or in `idat` (1).
    fn heif(method: u8) -> Vec<u8> {
        let mut payload = vec![0, 0, 0, 0];
        payload.extend(tiff());
        heif_with(method, (b"Exif", b"Exif", b"\0"), payload)
    }

    /// [`heif`] with item 2 of type `kinds.0` and item 3 (whose data is `payload`) of type
    /// `kinds.1`, its `infe` ending in `kinds.2` (the item name and what follows).
    fn heif_with(method: u8, kinds: (&[u8], &[u8], &[u8]), payload: Vec<u8>) -> Vec<u8> {
        let full = |v: u8, rest: &[u8]| {
            let mut b = vec![v, 0, 0, 0];
            b.extend_from_slice(rest);
            b
        };
        let infe = |id: u16, kind: &[u8], tail: &[u8]| {
            let mut b = id.to_be_bytes().to_vec();
            b.extend_from_slice(&[0, 0]);
            b.extend_from_slice(kind);
            b.extend_from_slice(tail);
            bx(b"infe", &full(2, &b))
        };
        let mut iinf = 3u16.to_be_bytes().to_vec();
        iinf.extend(infe(1, b"hvc1", b"\0"));
        iinf.extend(infe(2, kinds.0, b"\0"));
        iinf.extend(infe(3, kinds.1, kinds.2));
        let iref = bx(b"cdsc", &[0, 3, 0, 1, 0, 1]);
        let ftyp = bx(b"ftyp", b"heic\0\0\0\0mif1heic");
        // iloc v1: offset 4, length 4, base 0, index 0.
        let iloc = |data_at: u32| {
            let mut b = vec![0x44, 0x00, 0, 2];
            // Item 2: junk, 1 extent.
            b.extend_from_slice(&[0, 2, 0, 0, 0, 0, 0, 1]);
            b.extend_from_slice(&0u32.to_be_bytes());
            b.extend_from_slice(&4u32.to_be_bytes());
            // Item 3: the real one, two extents.
            b.extend_from_slice(&[0, 3, 0, method, 0, 0, 0, 2]);
            let half = (payload.len() / 2) as u32;
            b.extend_from_slice(&data_at.to_be_bytes());
            b.extend_from_slice(&half.to_be_bytes());
            b.extend_from_slice(&(data_at + half).to_be_bytes());
            b.extend_from_slice(&(payload.len() as u32 - half).to_be_bytes());
            bx(b"iloc", &full(1, &b))
        };
        let meta = |data_at: u32| {
            let mut m = bx(b"pitm", &full(0, &[0, 1]));
            m.extend(bx(b"iinf", &full(0, &iinf)));
            m.extend(bx(b"iref", &full(0, &iref)));
            m.extend(iloc(data_at));
            if method == 1 {
                m.extend(bx(b"idat", &payload));
            }
            bx(b"meta", &full(0, &m))
        };
        // The mdat (payload) follows the meta box, whose size does not depend on `data_at`.
        let at = (ftyp.len() + meta(0).len() + 8) as u32;
        let mut f = ftyp;
        f.extend(meta(if method == 0 { at } else { 0 }));
        f.extend(bx(b"mdat", if method == 0 { &payload } else { &[] }));
        f
    }

    #[test]
    fn heif_item() {
        check(&heif(0));
        check(&heif(1));
    }

    /// A gzip member of `data` stored (uncompressed), with a file name.
    pub(crate) fn gzip(data: &[u8]) -> Vec<u8> {
        let mut g = vec![0x1f, 0x8b, 8, 0x08, 0, 0, 0, 0, 0, 3];
        g.extend_from_slice(b"exif.bin\0");
        g.push(1);
        g.extend_from_slice(&(data.len() as u16).to_le_bytes());
        g.extend_from_slice(&(!(data.len() as u16)).to_le_bytes());
        g.extend_from_slice(data);
        let mut crc = !0u32;
        for &b in data {
            crc ^= u32::from(b);
            for _ in 0..8 {
                crc = if crc & 1 == 1 {
                    crc >> 1 ^ 0xEDB8_8320
                } else {
                    crc >> 1
                };
            }
        }
        g.extend_from_slice(&(!crc).to_le_bytes());
        g.extend_from_slice(&(data.len() as u32).to_le_bytes());
        g
    }

    /// Compressed Exif items: `mime` items of type application/exif in each encoding, with and
    /// without the offset prefix, and an `Exif` item that declares an encoding anyway.
    #[test]
    fn heif_compressed_item() {
        let mut prefixed = vec![0, 0, 0, 0];
        prefixed.extend(tiff());
        let mut marked = b"Exif\0\0".to_vec();
        marked.extend(tiff());
        let raw = |z: Vec<u8>| z[2..z.len() - 4].to_vec();
        for payload in [prefixed, marked, tiff()] {
            for (enc, data) in [
                (&b"deflate"[..], specular_png_zlib(&payload)),
                (b"deflate", raw(specular_png_zlib(&payload))),
                (b"zlib", specular_png_zlib(&payload)),
                (b"gzip", gzip(&payload)),
                (b"", payload.clone()),
            ] {
                let tail = [&b"\0application/exif\0"[..], enc, b"\0"].concat();
                for method in [0, 1] {
                    check(&heif_with(method, (b"hvc1", b"mime", &tail), data.clone()));
                    // Upper-case content type.
                    let up = [&b"\0Application/EXIF\0"[..], enc, b"\0"].concat();
                    check(&heif_with(method, (b"hvc1", b"mime", &up), data.clone()));
                    let tail = [&b"\0\0"[..], enc, b"\0"].concat();
                    check(&heif_with(method, (b"hvc1", b"Exif", &tail), data.clone()));
                }
            }
        }
        let mut prefixed = vec![0, 0, 0, 0];
        prefixed.extend(tiff());
        let z = specular_png_zlib(&prefixed);
        // An unknown encoding on a `mime` item, another content type, a corrupt stream.
        let none = |kinds: (&[u8], &[u8], &[u8]), data: Vec<u8>| {
            assert_eq!(both(&heif_with(0, kinds, data)), (None, None));
        };
        none((b"hvc1", b"mime", b"\0application/exif\0lzma\0"), z.clone());
        none(
            (b"hvc1", b"mime", b"\0application/rdf+xml\0\0"),
            prefixed.clone(),
        );
        let mut bad = gzip(&prefixed);
        let n = bad.len() - 6;
        bad[n] ^= 1;
        none((b"hvc1", b"mime", b"\0application/exif\0gzip\0"), bad);
        // An `Exif` item (item 2, junk) wins over a `mime` one even when that describes the
        // primary item.
        none(
            (b"Exif", b"mime", b"\0application/exif\0\0"),
            prefixed.clone(),
        );
        check(&heif_with(
            0,
            (b"hvc1", b"mime", b"\0application/exif\0\0"),
            prefixed,
        ));
    }

    #[test]
    fn offset_prefix_is_lenient() {
        let mut b = vec![0, 0, 0, 99, 1, 2];
        b.extend(tiff());
        assert_eq!(
            tiff_after_offset(Cow::Borrowed(&b)).as_deref(),
            Some(&tiff()[..])
        );
    }
}

/// Synthetic hostile inputs: none may panic, hang or blow up memory.
#[cfg(test)]
mod hostile {
    use super::*;

    fn bx(kind: &[u8], body: &[u8]) -> Vec<u8> {
        let mut b = ((body.len() + 8) as u32).to_be_bytes().to_vec();
        b.extend_from_slice(kind);
        b.extend_from_slice(body);
        b
    }

    /// An `iloc` of zero-sized fields whose 200 000 items each claim 65 535 extents: none of
    /// them consumes a byte, so they must not be counted out one by one.
    #[test]
    fn iloc_extents_of_no_bytes_end() {
        let items = 200_000u32;
        let mut iloc = vec![2, 0, 0, 0, 0x00, 0x00];
        iloc.extend_from_slice(&items.to_be_bytes());
        for id in 0..items {
            iloc.extend_from_slice(&(id + 10).to_be_bytes());
            iloc.extend_from_slice(&[0, 0, 0, 0, 0xFF, 0xFF]);
        }
        let meta = bx(b"iloc", &iloc);
        let m = HeifMeta::parse(&meta);
        let file: &[u8] = &[0; 64];
        let t = std::time::Instant::now();
        assert!(m.item_data(&file, 1, MAX_EXIF).is_none());
        assert!(
            m.item_data(&file, items + 9, MAX_EXIF)
                .is_some_and(|d| d.len() <= 64)
        );
        assert!(t.elapsed().as_secs() < 20);
    }

    /// An `iref` of 200 000 empty `cdsc` boxes each claiming 65 535 references.
    #[test]
    fn iref_counts_past_the_box_end() {
        let mut iref = vec![0, 0, 0, 0];
        for _ in 0..200_000 {
            iref.extend(bx(b"cdsc", &[0, 1, 0xFF, 0xFF]));
        }
        let mut meta = bx(b"pitm", &[0, 0, 0, 0, 0, 7]);
        meta.extend(bx(b"iref", &iref));
        let m = HeifMeta::parse(&meta);
        let t = std::time::Instant::now();
        assert_eq!(m.prefer_primary(&[1]), Some(1));
        assert!(t.elapsed().as_secs() < 20);
    }
}
