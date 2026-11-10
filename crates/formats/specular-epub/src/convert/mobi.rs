//! Mobipocket and Kindle books (`.mobi`, `.prc`, `.azw`, `.azw3`) and PalmDOC (`TEXtREAd`)
//! as EPUB. Written from the formats' public descriptions (the MobileRead wiki's PDB, MOBI,
//! EXTH, PalmDOC, HUFF/CDIC, KF8 and indexing pages):
//!
//! - a Palm database of records; record 0 holds the PalmDOC header, the MOBI header and the
//!   EXTH metadata; the text records follow, compressed with PalmDOC's LZ77 or HUFF/CDIC,
//!   each with trailing entries the header's flags describe;
//! - MOBI 6 text is HTML with `<mbp:pagebreak/>`, `recindex` pictures and `filepos` links:
//!   split at the page breaks into pages, links turned into anchors;
//! - KF8 (AZW3, and the second half of a joint MOBI) keeps its XHTML files as skeletons and
//!   fragments in the text's first flow, its CSS in the other flows, all found through the
//!   FDST record and the skeleton and fragment indexes; `kindle:` references become package
//!   paths;
//! - the contents come from the NCX index.
//!
//! Locked books (Mobipocket or Kindle DRM) are refused; KFX (Kindle's newer container,
//! undocumented) is refused by name.

use std::collections::BTreeMap;

use super::{Builder, Meta, Toc, esc, image_type, xhtml};
use crate::{DrmScheme, Error};

fn u16be(d: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes(d.get(at..at + 2)?.try_into().ok()?))
}

fn u32be(d: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(d.get(at..at + 4)?.try_into().ok()?))
}

fn bad(m: &str) -> Error {
    Error::Damaged(m.into())
}

/// The kind of Palm database `head` is: a Mobipocket/Kindle book or a PalmDOC.
pub fn is_mobi(head: &[u8]) -> bool {
    matches!(head.get(60..68), Some(b"BOOKMOBI" | b"TEXtREAd"))
}

/// Kindle's KFX container (or its DRM wrapper), refused by name.
pub fn is_kfx(head: &[u8]) -> bool {
    head.starts_with(b"CONT\x02\x00") || head.starts_with(b"\xeaDRMION\xee")
}

/// The records of a Palm database.
struct Pdb<'a> {
    data: &'a [u8],
    offsets: Vec<usize>,
}

impl<'a> Pdb<'a> {
    fn parse(data: &'a [u8]) -> Result<Pdb<'a>, Error> {
        let n = usize::from(u16be(data, 76).ok_or_else(|| bad("Palm database header cut short"))?);
        let mut offsets = Vec::with_capacity(n + 1);
        for i in 0..n {
            let o = u32be(data, 78 + 8 * i).ok_or_else(|| bad("record list cut short"))? as usize;
            offsets.push(o.min(data.len()));
        }
        offsets.push(data.len());
        // Offsets go up; a record past the next one is cut there.
        for i in (0..n).rev() {
            if offsets[i] > offsets[i + 1] {
                offsets[i] = offsets[i + 1];
            }
        }
        Ok(Pdb { data, offsets })
    }

    fn len(&self) -> usize {
        self.offsets.len() - 1
    }

    fn record(&self, i: usize) -> &'a [u8] {
        match (self.offsets.get(i), self.offsets.get(i + 1)) {
            (Some(&a), Some(&b)) => &self.data[a..b],
            _ => &[],
        }
    }
}

/// PalmDOC LZ77 (compression 2).
fn palmdoc(d: &[u8], out: &mut Vec<u8>) {
    let mut i = 0;
    while i < d.len() {
        let c = d[i];
        i += 1;
        match c {
            1..=8 => {
                let n = usize::from(c).min(d.len() - i);
                out.extend_from_slice(&d[i..i + n]);
                i += n;
            }
            0x80..=0xBF => {
                let Some(&n) = d.get(i) else { break };
                i += 1;
                let v = (usize::from(c) << 8) | usize::from(n);
                let dist = (v >> 3) & 0x7FF;
                let len = (v & 7) + 3;
                if dist == 0 || dist > out.len() {
                    continue;
                }
                for _ in 0..len {
                    out.push(out[out.len() - dist]);
                }
            }
            0xC0..=0xFF => {
                out.push(b' ');
                out.push(c ^ 0x80);
            }
            c => out.push(c),
        }
    }
}

/// The HUFF/CDIC decompressor (compression 17480): Huffman codes into a dictionary whose
/// entries may themselves be compressed.
struct Huff {
    /// By the code's top byte: (code length, terminal, max code).
    dict1: [(u32, bool, u64); 256],
    mincode: [u64; 33],
    maxcode: [u64; 33],
    /// Entries: bytes and whether they are already plain.
    dict: Vec<(Vec<u8>, bool)>,
}

impl Huff {
    fn new(huff: &[u8], cdics: &[&[u8]]) -> Result<Huff, Error> {
        if !huff.starts_with(b"HUFF") {
            return Err(bad("no HUFF record"));
        }
        let t1 = u32be(huff, 8).ok_or_else(|| bad("HUFF cut short"))? as usize;
        let t2 = u32be(huff, 12).ok_or_else(|| bad("HUFF cut short"))? as usize;
        let mut dict1 = [(0u32, false, 0u64); 256];
        for (i, d) in dict1.iter_mut().enumerate() {
            let v = u32be(huff, t1 + 4 * i).ok_or_else(|| bad("HUFF table cut short"))?;
            *d = (v & 0x1F, v & 0x80 != 0, u64::from(v >> 8));
        }
        let mut mincode = [0u64; 33];
        let mut maxcode = [0u64; 33];
        for len in 1..=32usize {
            let lo = u64::from(
                u32be(huff, t2 + 8 * (len - 1)).ok_or_else(|| bad("HUFF table cut short"))?,
            );
            let hi = u64::from(
                u32be(huff, t2 + 8 * (len - 1) + 4).ok_or_else(|| bad("HUFF table cut short"))?,
            );
            mincode[len] = lo << (32 - len);
            maxcode[len] = ((hi + 1) << (32 - len)).wrapping_sub(1);
        }
        let mut dict = Vec::new();
        for c in cdics {
            if !c.starts_with(b"CDIC") {
                return Err(bad("no CDIC record"));
            }
            let hl = u32be(c, 4).unwrap_or(16) as usize;
            let n = u32be(c, 8).unwrap_or(0) as usize;
            let bits = u32be(c, 12).unwrap_or(0).min(16);
            let here = (n - dict.len().min(n)).min(1 << bits);
            for k in 0..here {
                let Some(off) = u16be(c, hl + 2 * k) else {
                    break;
                };
                let at = hl + usize::from(off);
                let Some(len) = u16be(c, at) else { break };
                let plain = len & 0x8000 != 0;
                let len = usize::from(len & 0x7FFF);
                let bytes = c.get(at + 2..at + 2 + len).unwrap_or_default().to_vec();
                dict.push((bytes, plain));
            }
        }
        Ok(Huff {
            dict1,
            mincode,
            maxcode,
            dict,
        })
    }

    fn unpack(&mut self, d: &[u8], out: &mut Vec<u8>, depth: u32) {
        if depth > 32 {
            return;
        }
        let mut padded = d.to_vec();
        padded.extend_from_slice(&[0; 8]);
        let word = |p: usize| u64::from_be_bytes(padded[p..p + 8].try_into().unwrap_or([0; 8]));
        let mut bitsleft = d.len() as i64 * 8;
        let mut pos = 0;
        let mut x = word(0);
        let mut n: i64 = 32;
        loop {
            if n <= 0 {
                pos += 4;
                if pos + 8 > padded.len() {
                    break;
                }
                x = word(pos);
                n += 32;
            }
            let code = (x >> n) & 0xFFFF_FFFF;
            let (mut len, term, mut max) = self.dict1[(code >> 24) as usize];
            if !term {
                while len < 32 && code < self.mincode[len as usize] {
                    len += 1;
                }
                max = self.maxcode[len as usize];
            }
            if len == 0 {
                break;
            }
            n -= i64::from(len);
            bitsleft -= i64::from(len);
            if bitsleft < 0 {
                break;
            }
            let r = (max.wrapping_sub(code) >> (32 - len)) as usize;
            let Some((bytes, plain)) = self.dict.get(r).cloned() else {
                break;
            };
            if plain {
                out.extend_from_slice(&bytes);
            } else {
                // Unpacked once, kept plain (and empty meanwhile: a loop ends there).
                self.dict[r] = (Vec::new(), true);
                let mut sub = Vec::new();
                self.unpack(&bytes, &mut sub, depth + 1);
                out.extend_from_slice(&sub);
                self.dict[r] = (sub, true);
            }
        }
    }
}

/// A trailing entry's size, written backwards at the end of `d`.
fn trailing_size(d: &[u8]) -> usize {
    let mut v = 0usize;
    let mut shift = 0;
    for &b in d.iter().rev().take(4) {
        v |= usize::from(b & 0x7F) << shift;
        shift += 7;
        if b & 0x80 != 0 {
            break;
        }
    }
    v
}

/// A text record without its trailing entries.
fn strip_trailing(mut d: &[u8], flags: u16) -> &[u8] {
    let mut f = flags >> 1;
    while f != 0 {
        if f & 1 != 0 {
            let n = trailing_size(d).min(d.len());
            d = &d[..d.len() - n];
        }
        f >>= 1;
    }
    if flags & 1 != 0
        && let Some(&last) = d.last()
    {
        let n = (usize::from(last & 3) + 1).min(d.len());
        d = &d[..d.len() - n];
    }
    d
}

/// A forward variable-width integer (the last byte has its high bit set): value and length.
fn varint(d: &[u8]) -> (u64, usize) {
    let mut v = 0u64;
    for (i, &b) in d.iter().enumerate().take(9) {
        v = (v << 7) | u64::from(b & 0x7F);
        if b & 0x80 != 0 {
            return (v, i + 1);
        }
    }
    (v, d.len().min(9))
}

/// A MOBI header (record 0 of a book, or of a joint file's KF8 half).
struct Header {
    /// Index of record 0 in the database.
    base: usize,
    compression: u16,
    text_records: usize,
    encryption: u16,
    utf8: bool,
    version: u32,
    first_image: usize,
    huff: (usize, usize),
    extra_flags: u16,
    ncx: Option<usize>,
    fdst: Option<usize>,
    skel: Option<usize>,
    frag: Option<usize>,
    exth: BTreeMap<u32, Vec<Vec<u8>>>,
    full_name: String,
}

const NONE: u32 = 0xFFFF_FFFF;

impl Header {
    fn parse(pdb: &Pdb, base: usize, mobi: bool) -> Result<Header, Error> {
        let r = pdb.record(base);
        let compression = u16be(r, 0).ok_or_else(|| bad("record 0 cut short"))?;
        let text_records = usize::from(u16be(r, 8).unwrap_or(0));
        let encryption = u16be(r, 12).unwrap_or(0);
        let mut h = Header {
            base,
            compression,
            text_records,
            encryption,
            utf8: false,
            version: 0,
            first_image: NONE as usize,
            huff: (0, 0),
            extra_flags: 0,
            ncx: None,
            fdst: None,
            skel: None,
            frag: None,
            exth: BTreeMap::new(),
            full_name: String::new(),
        };
        if !mobi || r.get(16..20) != Some(b"MOBI") {
            return Ok(h);
        }
        let hl = u32be(r, 20).unwrap_or(0) as usize;
        let field = |off: usize| u32be(r, off).filter(|_| off + 4 <= 16 + hl);
        let rel = |v: Option<u32>| {
            v.filter(|&v| v != NONE && v != 0)
                .map(|v| v as usize + base)
        };
        h.utf8 = field(28) == Some(65001);
        h.version = field(36).unwrap_or(0);
        h.first_image = field(108)
            .filter(|&v| v != NONE)
            .map_or(NONE as usize, |v| v as usize + base);
        h.huff = (
            field(112).map_or(0, |v| v as usize + base),
            field(116).unwrap_or(0) as usize,
        );
        if hl >= 0xE4 {
            h.extra_flags = u16be(r, 242).unwrap_or(0);
        }
        h.ncx = rel(field(244));
        if h.version >= 8 {
            h.fdst = rel(field(192));
            h.frag = rel(field(248));
            h.skel = rel(field(252));
        }
        let name_off = field(84).unwrap_or(0) as usize;
        let name_len = field(88).unwrap_or(0) as usize;
        if let Some(n) = r.get(name_off..name_off + name_len) {
            h.full_name = h.text(n);
        }
        if field(128).is_some_and(|f| f & 0x40 != 0) {
            let at = 16 + hl;
            if r.get(at..at + 4) == Some(b"EXTH") {
                let count = u32be(r, at + 8).unwrap_or(0);
                let mut p = at + 12;
                for _ in 0..count.min(4096) {
                    let (Some(t), Some(l)) = (u32be(r, p), u32be(r, p + 4)) else {
                        break;
                    };
                    let l = l as usize;
                    if l < 8 {
                        break;
                    }
                    if let Some(v) = r.get(p + 8..p + l) {
                        h.exth.entry(t).or_default().push(v.to_vec());
                    }
                    p += l;
                }
            }
        }
        Ok(h)
    }

    fn text(&self, b: &[u8]) -> String {
        if self.utf8 {
            String::from_utf8_lossy(b).into_owned()
        } else {
            b.iter().map(|&c| crate::xml::cp1252(c)).collect()
        }
    }

    fn exth_text(&self, t: u32) -> Vec<String> {
        self.exth
            .get(&t)
            .map(|v| {
                v.iter()
                    .map(|b| self.text(b).trim().to_owned())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default()
    }

    fn exth_u32(&self, t: u32) -> Option<u32> {
        self.exth.get(&t)?.first().and_then(|b| u32be(b, 0))
    }

    /// The decompressed text (all text records joined).
    fn raw_text(&self, pdb: &Pdb) -> Result<Vec<u8>, Error> {
        let mut out = Vec::new();
        let mut huff = if self.compression == 17480 {
            let cdics: Vec<&[u8]> = (1..self.huff.1)
                .map(|k| pdb.record(self.huff.0 + k))
                .collect();
            Some(Huff::new(pdb.record(self.huff.0), &cdics)?)
        } else {
            None
        };
        for k in 1..=self.text_records {
            let r = strip_trailing(pdb.record(self.base + k), self.extra_flags);
            match self.compression {
                1 => out.extend_from_slice(r),
                2 => palmdoc(r, &mut out),
                17480 => {
                    if let Some(h) = huff.as_mut() {
                        h.unpack(r, &mut out, 0);
                    }
                }
                c => return Err(Error::Damaged(format!("unknown text compression {c}"))),
            }
            if out.len() > super::MAX_BOOK {
                return Err(Error::TooLarge("the text is too long".into()));
            }
        }
        Ok(out)
    }
}

/// An index (INDX records with their TAGX table and CNCX strings): each entry's name and
/// tag values.
/// An index entry: its name and its tags' values.
type Entry = (Vec<u8>, BTreeMap<u8, Vec<u64>>);

struct Index {
    entries: Vec<Entry>,
    cncx: Vec<u8>,
    /// Where each CNCX record starts in `cncx`.
    cncx_at: Vec<usize>,
}

impl Index {
    fn read(pdb: &Pdb, first: usize) -> Option<Index> {
        let p = pdb.record(first);
        if !p.starts_with(b"INDX") {
            return None;
        }
        let hl = u32be(p, 4)? as usize;
        let count = u32be(p, 24)? as usize;
        let ncncx = u32be(p, 52).unwrap_or(0) as usize;
        // TAGX after the header: (tag, values per entry, mask, end of control byte).
        let t = p.get(hl..)?;
        if !t.starts_with(b"TAGX") {
            return None;
        }
        let tl = u32be(t, 4)? as usize;
        let control_bytes = u32be(t, 8)? as usize;
        let tags: Vec<[u8; 4]> = t
            .get(12..tl)?
            .chunks_exact(4)
            .map(|c| [c[0], c[1], c[2], c[3]])
            .collect();
        let mut entries = Vec::new();
        for k in 1..=count.min(4096) {
            let r = pdb.record(first + k);
            if !r.starts_with(b"INDX") {
                break;
            }
            let idxt = u32be(r, 20)? as usize;
            let n = u32be(r, 24)? as usize;
            if r.get(idxt..idxt + 4) != Some(b"IDXT") {
                continue;
            }
            let offs: Vec<usize> = (0..n)
                .filter_map(|i| u16be(r, idxt + 4 + 2 * i).map(usize::from))
                .collect();
            for (i, &o) in offs.iter().enumerate() {
                let end = offs.get(i + 1).copied().unwrap_or(idxt).min(r.len());
                let Some(e) = r.get(o..end) else { continue };
                let Some((&il, rest)) = e.split_first() else {
                    continue;
                };
                let il = usize::from(il).min(rest.len());
                let ident = rest[..il].to_vec();
                let rest = &rest[il..];
                let Some(ctrl) = rest.get(..control_bytes) else {
                    continue;
                };
                let mut data = &rest[control_bytes..];
                // Which tags this entry has, and how many values.
                let mut plan: Vec<(u8, Option<u64>, Option<u64>, u8)> = Vec::new();
                let mut ci = 0;
                for &[tag, nvals, mask, end] in &tags {
                    if end & 1 == 1 {
                        ci += 1;
                        continue;
                    }
                    let Some(&cb) = ctrl.get(ci) else { break };
                    let mut v = cb & mask;
                    if v == 0 {
                        continue;
                    }
                    if v == mask {
                        if mask.count_ones() > 1 {
                            let (bytes, l) = varint(data);
                            data = &data[l..];
                            plan.push((tag, None, Some(bytes), nvals));
                        } else {
                            plan.push((tag, Some(1), None, nvals));
                        }
                    } else {
                        let mut m = mask;
                        while m & 1 == 0 {
                            m >>= 1;
                            v >>= 1;
                        }
                        plan.push((tag, Some(u64::from(v)), None, nvals));
                    }
                }
                let mut map = BTreeMap::new();
                for (tag, count, bytes, nvals) in plan {
                    let mut vals = Vec::new();
                    match (count, bytes) {
                        (Some(c), _) => {
                            for _ in 0..(c * u64::from(nvals)).min(1024) {
                                let (v, l) = varint(data);
                                if l == 0 {
                                    break;
                                }
                                data = &data[l..];
                                vals.push(v);
                            }
                        }
                        (None, Some(b)) => {
                            let mut used = 0u64;
                            while used < b && !data.is_empty() {
                                let (v, l) = varint(data);
                                data = &data[l..];
                                used += l as u64;
                                vals.push(v);
                            }
                        }
                        _ => {}
                    }
                    map.insert(tag, vals);
                }
                entries.push((ident, map));
            }
        }
        let mut cncx = Vec::new();
        let mut cncx_at = Vec::new();
        for k in 0..ncncx {
            cncx_at.push(cncx.len());
            cncx.extend_from_slice(pdb.record(first + count + 1 + k));
        }
        Some(Index {
            entries,
            cncx,
            cncx_at,
        })
    }

    /// The CNCX string at `off` (record in the high 16 bits).
    fn string(&self, off: u64, h: &Header) -> String {
        let rec = (off >> 16) as usize;
        let at = self
            .cncx_at
            .get(rec)
            .copied()
            .unwrap_or(usize::MAX)
            .saturating_add((off & 0xFFFF) as usize);
        let Some(d) = self.cncx.get(at..) else {
            return String::new();
        };
        let (len, l) = varint(d);
        d.get(l..l + len as usize)
            .map(|b| h.text(b))
            .unwrap_or_default()
    }
}

/// The pictures of a book: by resource number from 1 (MOBI `recindex`, KF8 `kindle:embed`),
/// added to the package.
fn pictures(pdb: &Pdb, h: &Header, b: &mut Builder) -> Result<BTreeMap<usize, String>, Error> {
    let mut out = BTreeMap::new();
    if h.first_image >= pdb.len() {
        return Ok(out);
    }
    for (k, i) in (h.first_image..pdb.len()).enumerate() {
        let r = pdb.record(i);
        // The end-of-file record and the KF8 boundary end the resources.
        if r.starts_with(b"\xe9\x8e\r\n") || r.starts_with(b"BOUNDARY") {
            break;
        }
        if let Some((mt, ext)) = image_type(r) {
            let path = format!("images/{}.{ext}", k + 1);
            b.add(&path, mt, r.to_vec(), false)?;
            out.insert(k + 1, path);
        }
    }
    Ok(out)
}

fn meta(h: &Header) -> Meta {
    let first = |t: u32| h.exth_text(t).into_iter().next();
    Meta {
        title: first(503).unwrap_or_else(|| h.full_name.clone()),
        authors: h.exth_text(100),
        publisher: first(101),
        date: first(106),
        language: first(524),
        description: first(103),
        subjects: h.exth_text(105),
        identifier: first(104).or_else(|| first(113)),
        rights: first(109),
    }
}

/// The cover's package path: EXTH 201 (an offset from the first picture), else the first
/// picture.
fn cover(h: &Header, pics: &BTreeMap<usize, String>) -> Option<String> {
    match h.exth_u32(201) {
        Some(o) if o != NONE => pics.get(&(o as usize + 1)).cloned(),
        _ => None,
    }
}

/// Converts a Mobipocket, Kindle (KF8) or PalmDOC book to an EPUB package.
pub fn to_epub(data: &[u8]) -> Result<Vec<u8>, Error> {
    if is_kfx(data) {
        return Err(if data.starts_with(b"\xeaDRMION\xee") {
            Error::Drm(DrmScheme::Kindle)
        } else {
            Error::Unsupported(
                "Kindle KFX books can't be opened: the format is not published".into(),
            )
        });
    }
    if !is_mobi(data) {
        return Err(Error::NotEpub);
    }
    let pdb = Pdb::parse(data)?;
    let mobi = &data[60..68] == b"BOOKMOBI";
    let mut h = Header::parse(&pdb, 0, mobi)?;
    if h.encryption != 0 {
        return Err(Error::Drm(DrmScheme::Kindle));
    }
    // A joint file: the KF8 half (EXTH 121 names its first record) is the one shown. Its
    // pictures are the first half's (shared) when its own resource number finds none.
    let mut shared_images = None;
    if let Some(b) = h
        .exth_u32(121)
        .filter(|&b| b != NONE && (b as usize) < pdb.len())
    {
        let k = Header::parse(&pdb, b as usize, true)?;
        if k.version >= 8 && k.encryption == 0 {
            shared_images = Some(h.first_image);
            h = k;
        }
    }
    let mut b = Builder {
        meta: meta(&h),
        ..Builder::default()
    };
    let mut pics = pictures(&pdb, &h, &mut b)?;
    if pics.is_empty()
        && let Some(first) = shared_images
    {
        h.first_image = first;
        pics = pictures(&pdb, &h, &mut b)?;
    }
    if let Some(c) = cover(&h, &pics) {
        b.set_cover(&c);
    }
    let text = h.raw_text(&pdb)?;
    if !mobi {
        palm_text(&h, &text, &mut b)?;
    } else if h.version >= 8 && h.fdst.is_some() {
        kf8(&pdb, &h, &text, &pics, &mut b)?;
    } else {
        mobi6(&pdb, &h, &text, &pics, &mut b)?;
    }
    Ok(b.finish())
}

/// A PalmDOC: plain text, paragraphs at blank lines.
fn palm_text(h: &Header, text: &[u8], b: &mut Builder) -> Result<(), Error> {
    let t = h.text(text).replace("\r\n", "\n");
    let mut body = String::new();
    for para in t.split("\n\n").filter(|p| !p.trim().is_empty()) {
        body.push_str("<p>");
        body.push_str(&esc(para.trim()).replace('\n', "<br/>"));
        body.push_str("</p>\n");
    }
    if b.meta.title.is_empty() {
        b.meta.title = h.full_name.clone();
    }
    b.add(
        "text/p0.xhtml",
        "application/xhtml+xml",
        xhtml(&b.meta.title.clone(), None, &body),
        true,
    )
}

/// The value of attribute `name` in the tag text `tag` (quoted or not).
fn attr_value<'t>(tag: &'t str, name: &str) -> Option<(usize, usize, &'t str)> {
    let lower = tag.to_ascii_lowercase();
    let mut from = 0;
    while let Some(i) = lower[from..].find(name) {
        let at = from + i;
        from = at + name.len();
        let before_ok = at == 0 || !lower.as_bytes()[at - 1].is_ascii_alphanumeric();
        let rest = &lower[from..];
        let eq = rest.len() - rest.trim_start().len();
        if !before_ok || !rest[eq..].starts_with('=') {
            continue;
        }
        let vstart = from + eq + 1;
        let vrest = &tag[vstart..];
        let skip = vrest.len() - vrest.trim_start().len();
        let vs = vstart + skip;
        let (s, e) = match tag[vs..].chars().next() {
            Some(q @ ('"' | '\'')) => {
                let e = tag[vs + 1..].find(q).map_or(tag.len(), |x| vs + 1 + x);
                (vs + 1, e)
            }
            _ => {
                let e = tag[vs..]
                    .find(|c: char| c.is_whitespace() || c == '>' || c == '/')
                    .map_or(tag.len(), |x| vs + x);
                (vs, e)
            }
        };
        return Some((
            at,
            if e < tag.len() && tag[e..].starts_with(['"', '\'']) {
                e + 1
            } else {
                e
            },
            &tag[s..e],
        ));
    }
    None
}

/// MOBI 6: HTML split at page breaks, with `filepos` anchors and `recindex` pictures.
fn mobi6(
    pdb: &Pdb,
    h: &Header,
    raw: &[u8],
    pics: &BTreeMap<usize, String>,
    b: &mut Builder,
) -> Result<(), Error> {
    // Link targets: byte offsets into the raw text.
    let mut targets: Vec<usize> = Vec::new();
    let lower: Vec<u8> = raw.iter().map(u8::to_ascii_lowercase).collect();
    let mut at = 0;
    while let Some(i) = memfind(&lower[at..], b"filepos=") {
        let s = at + i + 8;
        let digits: Vec<u8> = lower[s..]
            .iter()
            .skip_while(|c| **c == b'"' || **c == b'\'')
            .take_while(|c| c.is_ascii_digit())
            .copied()
            .collect();
        if let Ok(v) = std::str::from_utf8(&digits).unwrap_or("").parse::<usize>() {
            targets.push(v);
        }
        at = s;
    }
    // The contents' targets too.
    let ncx = h.ncx.and_then(|i| Index::read(pdb, i));
    if let Some(ix) = &ncx {
        for (_, tags) in &ix.entries {
            if let Some(&o) = tags.get(&1).and_then(|v| v.first()) {
                targets.push(o as usize);
            }
        }
    }
    targets.sort_unstable();
    targets.dedup();
    // Anchors in the raw text, moved out of any tag they would fall in.
    let mut text = Vec::with_capacity(raw.len() + targets.len() * 24);
    let mut last = 0;
    let mut placed = Vec::new();
    for &t in &targets {
        let mut p = t.min(raw.len());
        if p < last {
            continue;
        }
        // Inside a tag: before it.
        if let Some(lt) = raw[last..p].iter().rposition(|&c| c == b'<')
            && !raw[last + lt..p].contains(&b'>')
        {
            p = last + lt;
        }
        text.extend_from_slice(&raw[last..p]);
        text.extend_from_slice(format!("<a id=\"filepos{t}\"></a>").as_bytes());
        placed.push((t, text.len()));
        last = p;
    }
    text.extend_from_slice(&raw[last..]);
    let html = h.text(&text);
    // The body only.
    let lower = html.to_ascii_lowercase();
    let start = lower
        .find("<body")
        .and_then(|i| lower[i..].find('>').map(|j| i + j + 1))
        .unwrap_or(0);
    let end = lower
        .rfind("</body>")
        .filter(|&e| e >= start)
        .unwrap_or(html.len());
    let body = &html[start..end];
    // Pages at `<mbp:pagebreak`; tags rewritten.
    let mut pages: Vec<String> = vec![String::new()];
    let mut anchors: BTreeMap<usize, usize> = BTreeMap::new();
    let mut i = 0;
    while i < body.len() {
        let page = pages.len() - 1;
        let Some(lt) = body[i..].find('<').map(|x| i + x) else {
            pages[page].push_str(&body[i..]);
            break;
        };
        pages[page].push_str(&body[i..lt]);
        let gt = body[lt..].find('>').map_or(body.len(), |x| lt + x + 1);
        let tag = &body[lt..gt];
        let tl = tag.to_ascii_lowercase();
        i = gt;
        if tl.starts_with("<mbp:pagebreak") {
            if !pages.last().is_some_and(|p| p.trim().is_empty()) {
                pages.push(String::new());
            }
            continue;
        }
        if tl.starts_with("<mbp:") || tl.starts_with("</mbp:") {
            continue;
        }
        let mut t = tag.to_owned();
        if tl.starts_with("<a id=\"filepos")
            && let Some((_, _, v)) = attr_value(tag, "id")
            && let Ok(n) = v.trim_start_matches("filepos").parse::<usize>()
        {
            anchors.insert(n, pages.len() - 1);
        }
        if tl.starts_with("<img") {
            if let Some((s, e, v)) = attr_value(tag, "recindex") {
                let path = v.trim().parse::<usize>().ok().and_then(|n| pics.get(&n));
                t = match path {
                    Some(p) => format!("{} src=\"../{}\"{}", &tag[..s], esc(p), &tag[e..]),
                    None => String::new(),
                };
            }
        } else if let Some((s, e, v)) = attr_value(tag, "filepos") {
            t = format!(
                "{} href=\"#filepos{}\"{}",
                &tag[..s],
                v.trim_start_matches('0').parse::<usize>().unwrap_or(0),
                &tag[e..]
            );
        }
        let page = pages.len() - 1;
        pages[page].push_str(&t);
    }
    // Links to another page name it.
    let names: Vec<String> = (0..pages.len()).map(|k| format!("p{k}.xhtml")).collect();
    for (k, p) in pages.iter_mut().enumerate() {
        let mut out = String::with_capacity(p.len());
        let mut rest = p.as_str();
        while let Some(i) = rest.find("href=\"#filepos") {
            out.push_str(&rest[..i]);
            let after = &rest[i + 14..];
            let n_end = after.find('"').unwrap_or(after.len());
            let n: usize = after[..n_end].parse().unwrap_or(0);
            let page = anchors.range(..=n).next_back().map_or(0, |(_, &pg)| pg);
            let page = anchors.get(&n).copied().unwrap_or(page);
            if page == k {
                out.push_str(&format!("href=\"#filepos{n}\""));
            } else {
                out.push_str(&format!("href=\"{}#filepos{n}\"", names[page]));
            }
            rest = &after[(n_end + 1).min(after.len())..];
        }
        out.push_str(rest);
        *p = out;
    }
    let title = b.meta.title.clone();
    for (k, p) in pages.iter().enumerate() {
        b.add(
            &format!("text/{}", names[k]),
            "application/xhtml+xml",
            xhtml(&title, None, p),
            true,
        )?;
    }
    // Contents.
    if let Some(ix) = &ncx {
        for (_, tags) in &ix.entries {
            let Some(&o) = tags.get(&1).and_then(|v| v.first()) else {
                continue;
            };
            let label = tags
                .get(&3)
                .and_then(|v| v.first())
                .map(|&c| ix.string(c, h))
                .unwrap_or_default();
            let depth = tags.get(&4).and_then(|v| v.first()).copied().unwrap_or(0) as u32;
            let page = anchors.get(&(o as usize)).copied().unwrap_or(0);
            b.toc.push(Toc {
                title: label,
                href: format!("text/{}#filepos{o}", names[page]),
                depth,
            });
        }
    }
    Ok(())
}

fn memfind(h: &[u8], n: &[u8]) -> Option<usize> {
    h.windows(n.len()).position(|w| w == n)
}

/// Base 32 as KF8 writes it (0–9, A–V).
fn base32(s: &str) -> Option<usize> {
    let mut v = 0usize;
    for c in s.chars() {
        let d = c.to_digit(32)?;
        v = v.checked_mul(32)?.checked_add(d as usize)?;
    }
    Some(v)
}

/// KF8: files from skeletons and fragments, flows as style sheets, `kindle:` references
/// rewritten.
fn kf8(
    pdb: &Pdb,
    h: &Header,
    raw: &[u8],
    pics: &BTreeMap<usize, String>,
    b: &mut Builder,
) -> Result<(), Error> {
    // Flows (FDST: start and end of each).
    let f = pdb.record(h.fdst.unwrap_or(0));
    let mut flows: Vec<&[u8]> = Vec::new();
    if f.starts_with(b"FDST") {
        let n = u32be(f, 8).unwrap_or(0) as usize;
        for k in 0..n.min(4096) {
            let (Some(s), Some(e)) = (u32be(f, 12 + 8 * k), u32be(f, 16 + 8 * k)) else {
                break;
            };
            let (s, e) = (s as usize, (e as usize).min(raw.len()));
            flows.push(raw.get(s..e.max(s)).unwrap_or_default());
        }
    }
    if flows.is_empty() {
        flows.push(raw);
    }
    let main = flows[0];
    let skel = h
        .skel
        .and_then(|i| Index::read(pdb, i))
        .ok_or_else(|| bad("KF8 without a skeleton index"))?;
    let frag = h
        .frag
        .and_then(|i| Index::read(pdb, i))
        .ok_or_else(|| bad("KF8 without a fragment index"))?;
    // Fragments: (insert position, file, start, length).
    let frags: Vec<(usize, usize, usize, usize)> = frag
        .entries
        .iter()
        .map(|(ident, t)| {
            let insert = std::str::from_utf8(ident)
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let file = t.get(&3).and_then(|v| v.first()).copied().unwrap_or(0) as usize;
            let pos = t
                .get(&6)
                .map(|v| {
                    (
                        v.first().copied().unwrap_or(0) as usize,
                        v.get(1).copied().unwrap_or(0) as usize,
                    )
                })
                .unwrap_or((0, 0));
            (insert, file, pos.0, pos.1)
        })
        .collect();
    let mut files: Vec<Vec<u8>> = Vec::new();
    let mut fp = 0;
    for (_, t) in &skel.entries {
        let count = t.get(&1).and_then(|v| v.first()).copied().unwrap_or(0) as usize;
        let (sp, sl) = t
            .get(&6)
            .map(|v| {
                (
                    v.first().copied().unwrap_or(0) as usize,
                    v.get(1).copied().unwrap_or(0) as usize,
                )
            })
            .unwrap_or((0, 0));
        let mut s = main
            .get(sp..(sp + sl).min(main.len()))
            .unwrap_or_default()
            .to_vec();
        let mut base = sp + sl;
        for _ in 0..count {
            let Some(&(insert, _, _, len)) = frags.get(fp) else {
                break;
            };
            fp += 1;
            let at = insert.saturating_sub(sp).min(s.len());
            let piece = main
                .get(base..(base + len).min(main.len()))
                .unwrap_or_default();
            s.splice(at..at, piece.iter().copied());
            base += len;
        }
        files.push(s);
    }
    // Style sheets and SVG flows.
    let mut flow_paths = BTreeMap::new();
    for (k, fl) in flows.iter().enumerate().skip(1) {
        let t = h.text(fl);
        let (path, mt) = if t.trim_start().starts_with("<svg") || t.contains("<svg") {
            (format!("flows/{k}.svg"), "image/svg+xml")
        } else {
            (format!("flows/{k}.css"), "text/css")
        };
        b.add(
            &path,
            mt,
            rewrite(&t, pics, &BTreeMap::new(), &frags, "../").into_bytes(),
            false,
        )?;
        flow_paths.insert(k, path);
    }
    let names: Vec<String> = (0..files.len())
        .map(|k| format!("text/part{k:04}.xhtml"))
        .collect();
    for (k, f) in files.iter().enumerate() {
        let x = rewrite(&h.text(f), pics, &flow_paths, &frags, "../");
        b.add(&names[k], "application/xhtml+xml", x.into_bytes(), true)?;
    }
    // Contents from the NCX: tag 6 (fragment, offset) names the file.
    if let Some(ix) = h.ncx.and_then(|i| Index::read(pdb, i)) {
        for (_, t) in &ix.entries {
            let label = t
                .get(&3)
                .and_then(|v| v.first())
                .map(|&c| ix.string(c, h))
                .unwrap_or_default();
            let depth = t.get(&4).and_then(|v| v.first()).copied().unwrap_or(0) as u32;
            let file = t
                .get(&6)
                .and_then(|v| v.first())
                .and_then(|&fr| frags.get(fr as usize))
                .map(|f| f.1)
                .or_else(|| {
                    let pos = *t.get(&1)?.first()? as usize;
                    frags.iter().rev().find(|f| f.0 <= pos).map(|f| f.1)
                })
                .unwrap_or(0);
            if let Some(n) = names.get(file) {
                b.toc.push(Toc {
                    title: label,
                    href: n.clone(),
                    depth,
                });
            }
        }
    }
    Ok(())
}

/// `kindle:embed:XXXX`, `kindle:flow:XXXX` and `kindle:pos:fid:XXXX:off:YYYY` references as
/// package paths (from a file one folder down: `up` is `../`).
fn rewrite(
    t: &str,
    pics: &BTreeMap<usize, String>,
    flows: &BTreeMap<usize, String>,
    frags: &[(usize, usize, usize, usize)],
    up: &str,
) -> String {
    let mut out = String::with_capacity(t.len());
    let mut rest = t;
    while let Some(i) = rest.find("kindle:") {
        out.push_str(&rest[..i]);
        let r = &rest[i + 7..];
        let end = r
            .find(|c: char| c == '"' || c == '\'' || c == ')' || c.is_whitespace())
            .unwrap_or(r.len());
        let body = &r[..end];
        let id = |s: &str| base32(s.split(['?', ':', '#']).next().unwrap_or(""));
        let target = if let Some(e) = body.strip_prefix("embed:") {
            id(e).and_then(|n| pics.get(&n)).map(|p| format!("{up}{p}"))
        } else if let Some(f) = body.strip_prefix("flow:") {
            id(f)
                .and_then(|n| flows.get(&n))
                .map(|p| format!("{up}{p}"))
        } else if let Some(p) = body.strip_prefix("pos:fid:") {
            id(p)
                .and_then(|fr| frags.get(fr))
                .map(|f| format!("part{:04}.xhtml", f.1))
        } else {
            None
        };
        out.push_str(&target.unwrap_or_default());
        rest = &r[end..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palmdoc_lz77() {
        // Literal, a run of literals, a back-reference, a space pair.
        let mut out = Vec::new();
        palmdoc(&[b'a', b'b', 2, b'c', b'd', 0x80, 0x20, 0xE1], &mut out);
        // Distance 4, length 3: "abc" again.
        assert_eq!(out, b"abcdabc a");
    }

    #[test]
    fn trailing_entries() {
        // One entry of 3 bytes (its size written backwards: 0x83), then a multibyte byte.
        let d = [b'x', b'y', 1, 0xAA, 0xBB, 0x83];
        assert_eq!(strip_trailing(&d, 0b10), &d[..3]);
        assert_eq!(strip_trailing(&d, 0b11), &d[..1]);
        assert_eq!(varint(&[0x01, 0x82]), (130, 2));
    }

    #[test]
    fn base32_digits() {
        assert_eq!(base32("0001"), Some(1));
        assert_eq!(base32("000V"), Some(31));
        assert_eq!(base32("0010"), Some(32));
        assert_eq!(base32("00W"), None);
    }

    #[test]
    fn attributes() {
        assert_eq!(
            attr_value("<img recindex=\"00003\">", "recindex").map(|a| a.2),
            Some("00003")
        );
        assert_eq!(
            attr_value("<a filepos=0001234 >", "filepos").map(|a| a.2),
            Some("0001234")
        );
        assert!(attr_value("<a xfilepos=1>", "filepos").is_none());
    }

    #[test]
    fn refusals() {
        let e = to_epub(b"\xeaDRMION\xee rest").unwrap_err();
        assert_eq!(e, Error::Drm(DrmScheme::Kindle));
        assert!(matches!(
            to_epub(b"CONT\x02\x00 rest"),
            Err(Error::Unsupported(_))
        ));
        assert_eq!(to_epub(b"not a book").unwrap_err(), Error::NotEpub);
    }
}
