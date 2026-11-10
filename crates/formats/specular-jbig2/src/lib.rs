//! JBIG2 (ITU-T T.88 | ISO/IEC 14492) decoding of standalone files (`.jb2`, `.jbig2`),
//! written from the Recommendation.
//!
//! Covered: sequential and random-access file organisation, known and unknown page counts;
//! every segment header form; page information with striping and unknown height (end of
//! stripe), end of page and end of file; generic regions (templates 0–3, TPGDON, adaptive
//! pixels, MMR, unknown data length), generic refinement (templates 0/1, TPGRON), symbol
//! dictionaries (arithmetic and Huffman, refinement and aggregation, context reuse, collective
//! bitmaps uncompressed or MMR), text regions (arithmetic and Huffman, refinement, transposed,
//! every reference corner, run-coded symbol ID tables), pattern dictionaries and halftone
//! regions (MMR, skip, Gray-coded grey-scale planes); intermediate, immediate and lossless
//! regions with all combination operators; standard tables B.1–B.15 and table segments;
//! comment extensions. Profiles and colour palette segments are skipped (pages are shown in
//! black and white).
//!
//! Refused: the 12-pixel extended generic template (T.88 Amendment 2). No encoder writes it,
//! no file using it is known, and no decoder to check against reads it (jbig2dec and pdfium
//! refuse it), so it stays refused by name.
//!
//! Each page is a page of the file. Output is 8-bit grey: black 0, white 255.

mod bitmap;
mod generic;
mod halftone;
mod huff;
mod mmr;
mod mq;
mod symbol;
mod text;

use std::collections::HashMap;
use std::rc::Rc;

use bitmap::{Bitmap, Budget, Op};
use huff::{Bits, Table};

/// The file signature.
pub const MAGIC: [u8; 8] = [0x97, 0x4A, 0x42, 0x32, 0x0D, 0x0A, 0x1A, 0x0A];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Not a JBIG2 file.
    NotRecognised,
    /// A valid file using a feature this decoder does not implement.
    Unsupported(String),
    Truncated(String),
    Malformed(String),
    /// Over the size limits.
    TooLarge,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NotRecognised => write!(f, "not a JBIG2 file"),
            Error::Unsupported(s) => write!(f, "unsupported: {s}"),
            Error::Truncated(s) => write!(f, "truncated: {s}"),
            Error::Malformed(s) => write!(f, "malformed: {s}"),
            Error::TooLarge => write!(f, "image too large"),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

/// How the stored samples describe colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Model {
    Grey,
    Rgb,
    Palette,
    YCbCr,
    Bilevel,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Info {
    pub width: u32,
    pub height: u32,
    pub pages: u32,
    pub bit_depth: u8,
    pub model: Model,
    pub has_alpha: bool,
    pub orientation: u8,
    pub coding: String,
    pub summary: Vec<(&'static str, String)>,
    pub fields: Vec<(String, String)>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Pixels {
    Gray8(Vec<u8>),
    Rgba8(Vec<u8>),
    Gray16(Vec<u16>),
    Rgba16(Vec<u16>),
    RgbaF32(Vec<f32>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub pixels: Pixels,
    /// The data ended early or was damaged; what could not be decoded is the page's
    /// background.
    pub truncated: bool,
}

pub(crate) fn be16(d: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_be_bytes(d.get(o..o + 2)?.try_into().ok()?))
}

pub(crate) fn be32(d: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_be_bytes(d.get(o..o + 4)?.try_into().ok()?))
}

/// Whether `head` starts with the JBIG2 file signature.
pub fn probe(head: &[u8]) -> bool {
    head.starts_with(&MAGIC)
}

/// A segment header and where its data is.
#[derive(Clone, Debug)]
struct Seg {
    number: u32,
    kind: u8,
    page: u32,
    refs: Vec<u32>,
    start: usize,
    end: usize,
    /// The data runs past the end of the file.
    cut: bool,
}

struct File {
    sequential: bool,
    declared_pages: Option<u32>,
    segs: Vec<Seg>,
    /// The file ends inside a segment header or the data of one.
    truncated: bool,
}

/// A segment header at `p`: the segment (data range not set), its header length and data
/// length (`None`: unknown).
fn segment_header(d: &[u8], p: usize) -> Result<(Seg, usize, Option<u32>)> {
    let short = || Error::Truncated("segment header".into());
    let number = be32(d, p).ok_or_else(short)?;
    let flags = *d.get(p + 4).ok_or_else(short)?;
    let kind = flags & 63;
    let mut q = p + 5;
    let rb = *d.get(q).ok_or_else(short)?;
    let mut count = (rb >> 5) as u32;
    if count == 7 {
        count = be32(d, q).ok_or_else(short)? & 0x1FFF_FFFF;
        q += 4 + (count as usize + 1).div_ceil(8);
    } else if count > 4 {
        return Err(Error::Malformed("bad referred-to segment count".into()));
    } else {
        q += 1;
    }
    if count > 1 << 16 {
        return Err(Error::Malformed("too many referred-to segments".into()));
    }
    let size = if number <= 256 {
        1
    } else if number <= 65536 {
        2
    } else {
        4
    };
    let mut refs = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let v = match size {
            1 => *d.get(q).ok_or_else(short)? as u32,
            2 => be16(d, q).ok_or_else(short)? as u32,
            _ => be32(d, q).ok_or_else(short)?,
        };
        refs.push(v);
        q += size;
    }
    let page = if flags & 64 != 0 {
        let v = be32(d, q).ok_or_else(short)?;
        q += 4;
        v
    } else {
        let v = *d.get(q).ok_or_else(short)? as u32;
        q += 1;
        v
    };
    let len = be32(d, q).ok_or_else(short)?;
    q += 4;
    let seg = Seg {
        number,
        kind,
        page,
        refs,
        start: 0,
        end: 0,
        cut: false,
    };
    Ok((seg, q - p, (len != u32::MAX).then_some(len)))
}

/// The length of an immediate generic region of unknown data length starting at `start`
/// (7.2.7): up to its end marker and the row count after it.
fn unknown_length(d: &[u8], start: usize) -> Option<usize> {
    let flags = *d.get(start + 17)?;
    let mmr = flags & 1 != 0;
    let template = (flags >> 1) & 3;
    let at = if mmr {
        0
    } else if template == 0 {
        8
    } else {
        2
    };
    let from = start + 18 + at;
    let marker: [u8; 2] = if mmr { [0, 0] } else { [0xFF, 0xAC] };
    let rel = d.get(from..)?.windows(2).position(|w| w == marker)?;
    Some(from + rel + 2 + 4 - start)
}

fn parse_file(d: &[u8]) -> Result<File> {
    if !probe(d) {
        return Err(Error::NotRecognised);
    }
    let flags = *d
        .get(8)
        .ok_or_else(|| Error::Truncated("file header".into()))?;
    let sequential = flags & 1 != 0;
    let mut p = 9;
    let declared_pages = if flags & 2 == 0 {
        let n = be32(d, 9).ok_or_else(|| Error::Truncated("file header".into()))?;
        p = 13;
        Some(n)
    } else {
        None
    };
    let mut segs = Vec::new();
    let mut truncated = false;
    if sequential {
        while p < d.len() {
            let (mut s, hl, len) = match segment_header(d, p) {
                Ok(v) => v,
                Err(Error::Truncated(_)) => {
                    truncated = true;
                    break;
                }
                Err(e) => return Err(e),
            };
            s.start = p + hl;
            let len = match len {
                Some(l) => l as usize,
                None => match unknown_length(d, s.start) {
                    Some(l) if matches!(s.kind, 38 | 39) => l,
                    _ => d.len().saturating_sub(s.start),
                },
            };
            s.end = s.start.saturating_add(len);
            if s.end > d.len() {
                s.end = d.len();
                s.cut = true;
                truncated = true;
            }
            p = s.end;
            let eof = s.kind == 51;
            segs.push(s);
            if eof || segs.len() > 1 << 20 {
                break;
            }
        }
    } else {
        let mut lens = Vec::new();
        while p < d.len() {
            let (s, hl, len) = match segment_header(d, p) {
                Ok(v) => v,
                Err(Error::Truncated(_)) => {
                    truncated = true;
                    break;
                }
                Err(e) => return Err(e),
            };
            let Some(len) = len else {
                return Err(Error::Malformed(
                    "unknown data length in a random-access file".into(),
                ));
            };
            p += hl;
            let eof = s.kind == 51;
            segs.push(s);
            lens.push(len as usize);
            if eof || segs.len() > 1 << 20 {
                break;
            }
        }
        for (s, len) in segs.iter_mut().zip(lens) {
            s.start = p.min(d.len());
            s.end = p.saturating_add(len);
            if s.end > d.len() {
                s.end = d.len();
                s.cut = true;
                truncated = true;
            }
            p = s.end;
        }
    }
    Ok(File {
        sequential,
        declared_pages,
        segs,
        truncated,
    })
}

/// The page information segments, in file order.
fn page_infos(f: &File) -> Vec<&Seg> {
    f.segs.iter().filter(|s| s.kind == 48).collect()
}

struct PageInfo {
    w: u32,
    h: u32,
    xres: u32,
    yres: u32,
    flags: u8,
    striping: u16,
}

fn page_info(d: &[u8], s: &Seg) -> Result<PageInfo> {
    let b = &d[s.start..s.end];
    let short = || Error::Truncated("page information".into());
    Ok(PageInfo {
        w: be32(b, 0).ok_or_else(short)?,
        h: be32(b, 4).ok_or_else(short)?,
        xres: be32(b, 8).ok_or_else(short)?,
        yres: be32(b, 12).ok_or_else(short)?,
        flags: *b.get(16).ok_or_else(short)?,
        striping: be16(b, 17).unwrap_or(0),
    })
}

/// Region segment information field: width, height, x, y, flags.
fn region_info(b: &[u8]) -> Result<(u32, u32, i64, i64, u8)> {
    let short = || Error::Truncated("region segment information".into());
    Ok((
        be32(b, 0).ok_or_else(short)?,
        be32(b, 4).ok_or_else(short)?,
        be32(b, 8).ok_or_else(short)? as i32 as i64,
        be32(b, 12).ok_or_else(short)? as i32 as i64,
        *b.get(16).ok_or_else(short)?,
    ))
}

/// The comments of extension segments (7.4.15): "name: value".
fn comments(b: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    let Some(t) = be32(b, 0) else { return out };
    let body = &b[4..];
    match t & 0x7FFF_FFFF {
        0x2000_0000 => {
            let mut parts = body
                .split(|&c| c == 0)
                .map(|s| String::from_utf8_lossy(s).into_owned());
            while let (Some(n), Some(v)) = (parts.next(), parts.next()) {
                if n.is_empty() {
                    break;
                }
                out.push(format!("{n}: {v}"));
            }
        }
        0x2000_0002 => {
            let units: Vec<u16> = body
                .chunks_exact(2)
                .map(|c| u16::from_be_bytes([c[0], c[1]]))
                .collect();
            let mut parts = units.split(|&c| c == 0).map(String::from_utf16_lossy);
            while let (Some(n), Some(v)) = (parts.next(), parts.next()) {
                if n.is_empty() {
                    break;
                }
                out.push(format!("{n}: {v}"));
            }
        }
        _ => {}
    }
    out
}

/// The page number (association) of the `index`th page and its page information segment.
fn page_of(f: &File, index: u32) -> Result<(u32, &Seg)> {
    let pages = page_infos(f);
    if pages.is_empty() {
        return Err(Error::Malformed("the file has no page information".into()));
    }
    let s = pages
        .get(index as usize)
        .ok_or_else(|| Error::Malformed(format!("no page {}", index + 1)))?;
    Ok((s.page, s))
}

fn op_name(op: u8) -> &'static str {
    match op & 3 {
        0 => "OR",
        1 => "AND",
        2 => "XOR",
        _ => "XNOR",
    }
}

/// Header facts of page `page` (0-based).
pub fn info(data: &[u8], page: u32) -> Result<Info> {
    let f = parse_file(data)?;
    let npages = page_infos(&f).len() as u32;
    let (pno, ps) = page_of(&f, page)?;
    let pi = page_info(data, ps)?;
    let mut height = pi.h;
    let mut kinds: Vec<&str> = Vec::new();
    let mut coders: Vec<&str> = Vec::new();
    let mut notes: Vec<String> = Vec::new();
    let mut counts: HashMap<&'static str, u32> = HashMap::new();
    let add = |v: &mut Vec<&'static str>, s: &'static str| {
        if !v.contains(&s) {
            v.push(s);
        }
    };
    let mut extent = 0u64;
    let mut eos = 0u64;
    for s in f.segs.iter().filter(|s| s.page == 0 || s.page == pno) {
        let b = &data[s.start..s.end];
        let name = match s.kind {
            0 => "Symbol dictionaries",
            4 | 6 | 7 => "Text regions",
            16 => "Pattern dictionaries",
            20 | 22 | 23 => "Halftone regions",
            36 | 38 | 39 => "Generic regions",
            40 | 42 | 43 => "Refinement regions",
            53 => "Huffman tables",
            62 => "Extensions",
            _ => "",
        };
        if !name.is_empty() {
            *counts.entry(name).or_default() += 1;
        }
        match s.kind {
            0 => {
                let fl = be16(b, 0).unwrap_or(0);
                add(
                    &mut coders,
                    if fl & 1 != 0 { "Huffman" } else { "arithmetic" },
                );
            }
            4 | 6 | 7 => {
                add(&mut kinds, "text");
                let fl = be16(b, 17).unwrap_or(0);
                add(
                    &mut coders,
                    if fl & 1 != 0 { "Huffman" } else { "arithmetic" },
                );
            }
            20 | 22 | 23 => {
                add(&mut kinds, "halftone");
                let fl = b.get(17).copied().unwrap_or(0);
                add(&mut coders, if fl & 1 != 0 { "MMR" } else { "arithmetic" });
            }
            36 | 38 | 39 => {
                add(&mut kinds, "generic");
                let fl = b.get(17).copied().unwrap_or(0);
                add(&mut coders, if fl & 1 != 0 { "MMR" } else { "arithmetic" });
            }
            40 | 42 | 43 => {
                add(&mut kinds, "refinement");
                add(&mut coders, "arithmetic");
            }
            50 => eos = eos.max(be32(b, 0).unwrap_or(0) as u64 + 1),
            62 => notes.extend(comments(b)),
            _ => {}
        }
        if matches!(s.kind, 6 | 7 | 22 | 23 | 38 | 39 | 42 | 43)
            && let Ok((_, h, _, y, _)) = region_info(b)
        {
            let h = if h == u32::MAX && b.len() >= 4 {
                be32(b, b.len() - 4).unwrap_or(0)
            } else {
                h
            };
            extent = extent.max((y.max(0) as u64).saturating_add(h as u64));
        }
    }
    if height == u32::MAX {
        height = eos.max(extent).min(u32::MAX as u64 - 1) as u32;
    }
    if pi.w == 0 || height == 0 {
        return Err(Error::Malformed("empty page".into()));
    }
    if pi.w > bitmap::MAX_SIDE
        || height > bitmap::MAX_SIDE
        || pi.w as u64 * height as u64 > bitmap::MAX_PIXELS
    {
        return Err(Error::TooLarge);
    }
    let coding = format!(
        "JBIG2 {}, {}",
        if kinds.is_empty() {
            "empty page".to_string()
        } else {
            kinds.join(" + ")
        },
        if coders.is_empty() {
            "no coding".to_string()
        } else {
            coders.join(" + ")
        }
    );
    let mut summary: Vec<(&'static str, String)> = Vec::new();
    if pi.xres > 0 && pi.yres > 0 {
        let dpi = |r: u32| (r as f64 * 0.0254).round() as u64;
        summary.push((
            "resolution",
            format!("{} × {} dpi", dpi(pi.xres), dpi(pi.yres)),
        ));
    }
    if !coders.is_empty() {
        summary.push(("compression", format!("JBIG2 ({})", coders.join(", "))));
    }
    if !notes.is_empty() {
        summary.push(("comment", notes.join("; ")));
    }
    let mut fields: Vec<(String, String)> = vec![
        (
            "Organisation".into(),
            if f.sequential {
                "sequential"
            } else {
                "random access"
            }
            .into(),
        ),
        ("Pages".into(), npages.to_string()),
    ];
    if let Some(n) = f.declared_pages {
        fields.push(("Declared pages".into(), n.to_string()));
    }
    fields.push(("Page".into(), format!("{} of {npages}", page + 1)));
    fields.push(("Page width".into(), pi.w.to_string()));
    fields.push((
        "Page height".into(),
        if pi.h == u32::MAX {
            format!("{height} (unknown in the header)")
        } else {
            pi.h.to_string()
        },
    ));
    if pi.xres > 0 || pi.yres > 0 {
        fields.push((
            "Resolution".into(),
            format!("{} × {} pixels per metre", pi.xres, pi.yres),
        ));
    }
    fields.push((
        "Default pixel".into(),
        if pi.flags & 4 != 0 { "black" } else { "white" }.into(),
    ));
    fields.push(("Default combination".into(), op_name(pi.flags >> 3).into()));
    if pi.flags & 1 != 0 {
        fields.push(("Eventually lossless".into(), "yes".into()));
    }
    if pi.striping & 0x8000 != 0 {
        fields.push((
            "Striped".into(),
            format!("up to {} rows per stripe", pi.striping & 0x7FFF),
        ));
    }
    let mut c: Vec<_> = counts.into_iter().collect();
    c.sort();
    for (k, v) in c {
        fields.push((k.into(), v.to_string()));
    }
    for n in &notes {
        fields.push(("Comment".into(), n.clone()));
    }
    Ok(Info {
        width: pi.w,
        height,
        pages: npages,
        bit_depth: 1,
        model: Model::Bilevel,
        has_alpha: false,
        orientation: 1,
        coding,
        summary,
        fields,
    })
}

enum Res {
    Dict(symbol::Dict),
    Patterns(Vec<Bitmap>),
    Table(Table),
    Region(Bitmap),
}

/// Decodes page `page` (0-based).
pub fn decode(data: &[u8], page: u32) -> Result<Image> {
    let page_bm = decode_page(data, page)?;
    let (bm, truncated) = page_bm;
    let px = bm.d.iter().map(|&b| if b != 0 { 0 } else { 255 }).collect();
    Ok(Image {
        width: bm.w,
        height: bm.h,
        pixels: Pixels::Gray8(px),
        truncated,
    })
}

struct Page {
    bm: Bitmap,
    /// The height is not known yet: the page grows with what is drawn on it.
    open_height: bool,
    default: u8,
}

fn decode_page(data: &[u8], page: u32) -> Result<(Bitmap, bool)> {
    let f = parse_file(data)?;
    let (pno, _) = page_of(&f, page)?;
    let budget = Budget::new();
    let mut results: HashMap<u32, Res> = HashMap::new();
    let mut pg: Option<Page> = None;
    let mut truncated = f.truncated;
    for s in &f.segs {
        if s.page != 0 && s.page != pno {
            continue;
        }
        let b = &data[s.start..s.end];
        truncated |= s.cut;
        let r = segment(s, b, &f, &mut results, &mut pg, &budget);
        match r {
            Ok(Flow::Go) => {}
            Ok(Flow::Stop) => break,
            Err(Error::TooLarge) => return Err(Error::TooLarge),
            Err(e @ Error::Unsupported(_)) => return Err(e),
            Err(e) => {
                if pg.is_none() {
                    return Err(e);
                }
                truncated = true;
                break;
            }
        }
    }
    let p = pg.ok_or_else(|| Error::Malformed("the page has no page information".into()))?;
    if p.bm.w == 0 || p.bm.h == 0 {
        return Err(Error::Malformed("empty page".into()));
    }
    Ok((p.bm, truncated))
}

enum Flow {
    Go,
    Stop,
}

/// The referred-to segments' results of one kind, in reference order.
fn referred<'r>(s: &Seg, results: &'r HashMap<u32, Res>) -> impl Iterator<Item = &'r Res> {
    s.refs.iter().filter_map(|n| results.get(n))
}

fn place(
    pg: &mut Option<Page>,
    bm: &Bitmap,
    x: i64,
    y: i64,
    flags: u8,
    budget: &Budget,
) -> Result<()> {
    let p = pg
        .as_mut()
        .ok_or_else(|| Error::Malformed("a region comes before the page information".into()))?;
    if p.open_height {
        let bottom = (y.max(0) as u64 + bm.h as u64).min(bitmap::MAX_SIDE as u64 + 1) as u32;
        p.bm.grow(bottom, p.default, budget)?;
    }
    p.bm.compose(bm, x, y, Op::from_bits(flags & 7));
    Ok(())
}

/// Refuses a region far larger than its page (a damaged size field), before decoding it.
/// Intermediate regions are checked against the page too: they end up on it.
fn plausible(pg: &Option<Page>, w: u32, h: u32) -> Result<()> {
    let Some(p) = pg else { return Ok(()) };
    let too_wide = w as u64 > 2 * p.bm.w as u64 + 256;
    let too_high = !p.open_height && h as u64 > 2 * p.bm.h as u64 + 256;
    if too_wide || too_high {
        return Err(Error::Malformed(format!(
            "a {w}×{h} region does not fit a {}×{} page",
            p.bm.w, p.bm.h
        )));
    }
    Ok(())
}

fn segment(
    s: &Seg,
    b: &[u8],
    f: &File,
    results: &mut HashMap<u32, Res>,
    pg: &mut Option<Page>,
    budget: &Budget,
) -> Result<Flow> {
    let _ = f;
    match s.kind {
        48 => {
            if pg.is_some() {
                return Ok(Flow::Go);
            }
            let short = || Error::Truncated("page information".into());
            let w = be32(b, 0).ok_or_else(short)?;
            let h = be32(b, 4).ok_or_else(short)?;
            let flags = *b.get(16).ok_or_else(short)?;
            let default = (flags >> 2) & 1;
            let open = h == u32::MAX;
            if w > bitmap::MAX_SIDE || (!open && h > bitmap::MAX_SIDE) {
                return Err(Error::TooLarge);
            }
            let bm = Bitmap::filled(w, if open { 0 } else { h }, default, budget)?;
            *pg = Some(Page {
                bm,
                open_height: open,
                default,
            });
        }
        49 => {
            if s.page != 0 {
                return Ok(Flow::Stop);
            }
        }
        50 => {
            if let (Some(p), Some(y)) = (pg.as_mut(), be32(b, 0))
                && p.open_height
            {
                let h = (y as u64 + 1).min(bitmap::MAX_SIDE as u64 + 1) as u32;
                p.bm.grow(h, p.default, budget)?;
            }
        }
        51 => return Ok(Flow::Stop),
        53 => {
            results.insert(s.number, Res::Table(Table::parse(b)?));
        }
        0 => {
            let inputs: Vec<Rc<Bitmap>> = referred(s, results)
                .filter_map(|r| match r {
                    Res::Dict(d) => Some(d.exported.iter().cloned()),
                    _ => None,
                })
                .flatten()
                .collect();
            let reuse = s
                .refs
                .iter()
                .rev()
                .filter_map(|n| match results.get(n) {
                    Some(Res::Dict(d)) => Some(d),
                    _ => None,
                })
                .next();
            let d = {
                let mut tables = referred(s, results).filter_map(|r| match r {
                    Res::Table(t) => Some(t),
                    _ => None,
                });
                symbol::decode(b, inputs, &mut tables, reuse, budget)?
            };
            results.insert(s.number, Res::Dict(d));
        }
        16 => {
            results.insert(s.number, Res::Patterns(halftone::patterns(b, budget)?));
        }
        4 | 6 | 7 => {
            let (w, h, x, y, rflags) = region_info(b)?;
            plausible(pg, w, h)?;
            let bm = text_region(s, b, w, h, results, budget)?;
            if s.kind == 4 {
                results.insert(s.number, Res::Region(bm));
            } else {
                place(pg, &bm, x, y, rflags, budget)?;
            }
        }
        20 | 22 | 23 => {
            let (w, h, x, y, rflags) = region_info(b)?;
            plausible(pg, w, h)?;
            let pats = referred(s, results)
                .find_map(|r| match r {
                    Res::Patterns(p) => Some(p.as_slice()),
                    _ => None,
                })
                .ok_or_else(|| {
                    Error::Malformed("halftone region without a pattern dictionary".into())
                })?;
            let bm = halftone::region(&b[17..], w, h, pats, budget)?;
            if s.kind == 20 {
                results.insert(s.number, Res::Region(bm));
            } else {
                place(pg, &bm, x, y, rflags, budget)?;
            }
        }
        36 | 38 | 39 => {
            let (w, mut h, x, y, rflags) = region_info(b)?;
            let fl = *b
                .get(17)
                .ok_or_else(|| Error::Truncated("generic region".into()))?;
            if h == u32::MAX && b.len() >= 4 {
                h = be32(b, b.len() - 4).unwrap_or(0);
            }
            plausible(pg, w, h)?;
            let (bm, damaged) = generic_region(b, w, h, fl, budget)?;
            if s.kind == 36 {
                results.insert(s.number, Res::Region(bm));
            } else {
                place(pg, &bm, x, y, rflags, budget)?;
            }
            if damaged {
                return Err(Error::Truncated("generic region".into()));
            }
        }
        40 | 42 | 43 => {
            let (w, h, x, y, rflags) = region_info(b)?;
            plausible(pg, w, h)?;
            let fl = *b
                .get(17)
                .ok_or_else(|| Error::Truncated("refinement region".into()))?;
            let template = fl & 1;
            let tpgron = fl & 2 != 0;
            let mut at = [(-1i8, -1i8), (-1, -1)];
            let mut o = 18;
            if template == 0 {
                let a = b
                    .get(18..22)
                    .ok_or_else(|| Error::Truncated("refinement region".into()))?;
                at = [(a[0] as i8, a[1] as i8), (a[2] as i8, a[3] as i8)];
                o = 22;
            }
            let intermediate = s.refs.iter().find_map(|n| match results.get(n) {
                Some(Res::Region(bm)) => Some(bm.clone()),
                _ => None,
            });
            let reference = match intermediate {
                Some(r) => r,
                None => {
                    let p = pg.as_ref().ok_or_else(|| {
                        Error::Malformed("a refinement comes before the page information".into())
                    })?;
                    p.bm.crop(x, y, w, h, budget)?
                }
            };
            let mut cx = vec![0u8; generic::refine_contexts(template)];
            let p = generic::Refine {
                w,
                h,
                template,
                reference: &reference,
                dx: 0,
                dy: 0,
                tpgron,
                at,
            };
            let bm = generic::refine(&mut mq::Mq::new(&b[o.min(b.len())..]), &mut cx, &p, budget)?;
            if s.kind == 40 {
                results.insert(s.number, Res::Region(bm));
            } else {
                place(pg, &bm, x, y, rflags, budget)?;
            }
        }
        _ => {}
    }
    Ok(Flow::Go)
}

fn generic_region(b: &[u8], w: u32, h: u32, fl: u8, budget: &Budget) -> Result<(Bitmap, bool)> {
    let mmr_coded = fl & 1 != 0;
    let template = (fl >> 1) & 3;
    let tpgdon = fl & 8 != 0;
    if fl & 16 != 0 {
        return Err(Error::Unsupported(
            "JBIG2 generic regions with the extended 12-pixel template are not supported".into(),
        ));
    }
    if mmr_coded {
        let (bm, _, damaged) = mmr::decode(b.get(18..).unwrap_or(&[]), w, h, budget)?;
        return Ok((bm, damaged));
    }
    let n = if template == 0 { 4 } else { 1 };
    let mut at = generic::default_at(template);
    let a = b
        .get(18..18 + 2 * n)
        .ok_or_else(|| Error::Truncated("generic region".into()))?;
    for (i, v) in at.iter_mut().take(n).enumerate() {
        *v = (a[2 * i] as i8 as i32, a[2 * i + 1] as i8 as i32);
    }
    let mut cx = vec![0u8; generic::contexts(template)];
    let p = generic::Generic {
        w,
        h,
        template,
        tpgdon,
        at,
        skip: None,
    };
    let mut m = mq::Mq::new(&b[18 + 2 * n..]);
    match generic::decode(&mut m, &mut cx, &p, budget) {
        Ok(bm) => Ok((bm, false)),
        Err(Error::Truncated(_)) => {
            // Keep what there is: decode again is not needed, the region is shown as far as
            // it goes by the caller treating the page as cut.
            Err(Error::Truncated("generic region data ends early".into()))
        }
        Err(e) => Err(e),
    }
}

fn text_region(
    s: &Seg,
    b: &[u8],
    w: u32,
    h: u32,
    results: &HashMap<u32, Res>,
    budget: &Budget,
) -> Result<Bitmap> {
    let short = || Error::Truncated("text region header".into());
    let fl = be16(b, 17).ok_or_else(short)?;
    let huffman = fl & 1 != 0;
    let refine = fl & 2 != 0;
    let log_strips = ((fl >> 2) & 3) as u32;
    let corner = ((fl >> 4) & 3) as u8;
    let transposed = fl & 0x40 != 0;
    let op = Op::from_bits(((fl >> 7) & 3) as u8);
    let default_pixel = ((fl >> 9) & 1) as u8;
    let mut ds_offset = ((fl >> 10) & 0x1F) as i64;
    if ds_offset >= 16 {
        ds_offset -= 32;
    }
    let rtemplate = ((fl >> 15) & 1) as u8;
    let mut o = 19;
    let hfl = if huffman {
        let v = be16(b, o).ok_or_else(short)?;
        o += 2;
        v
    } else {
        0
    };
    let mut rat = [(-1i8, -1i8), (-1, -1)];
    if refine && rtemplate == 0 {
        let a = b.get(o..o + 4).ok_or_else(short)?;
        rat = [(a[0] as i8, a[1] as i8), (a[2] as i8, a[3] as i8)];
        o += 4;
    }
    let instances = be32(b, o).ok_or_else(short)?;
    o += 4;
    let symbols: Vec<Rc<Bitmap>> = referred(s, results)
        .filter_map(|r| match r {
            Res::Dict(d) => Some(d.exported.iter().cloned()),
            _ => None,
        })
        .flatten()
        .collect();
    let symcodelen = mq::code_len(symbols.len() as u64);
    if symcodelen > 24 {
        return Err(Error::TooLarge);
    }
    let p = text::Text {
        w,
        h,
        instances,
        log_strips,
        symbols: &symbols,
        refine,
        default_pixel,
        op,
        transposed,
        corner,
        ds_offset,
        rtemplate,
        rat,
    };
    let body = &b[o.min(b.len())..];
    let gr = vec![0u8; generic::refine_contexts(rtemplate)];
    if huffman {
        let mut tables = referred(s, results).filter_map(|r| match r {
            Res::Table(t) => Some(t),
            _ => None,
        });
        let mut pick = |sel: u16, std: &[usize]| -> Result<Table> {
            if sel == 3 {
                tables.next().cloned().ok_or_else(|| {
                    Error::Malformed("text region refers to a missing Huffman table".into())
                })
            } else {
                std.get(sel as usize)
                    .map(|&n| Table::standard(n))
                    .ok_or_else(|| {
                        Error::Malformed("text region selects a reserved Huffman table".into())
                    })
            }
        };
        let fs = pick(hfl & 3, &[6, 7])?;
        let ds = pick((hfl >> 2) & 3, &[8, 9, 10])?;
        let dt = pick((hfl >> 4) & 3, &[11, 12, 13])?;
        let rdw = pick((hfl >> 6) & 3, &[14, 15])?;
        let rdh = pick((hfl >> 8) & 3, &[14, 15])?;
        let rdx = pick((hfl >> 10) & 3, &[14, 15])?;
        let rdy = pick((hfl >> 12) & 3, &[14, 15])?;
        let rsize = if (hfl >> 14) & 1 == 1 {
            pick(3, &[])?
        } else {
            Table::standard(1)
        };
        let mut bits = Bits::new(body);
        let ids = text::symbol_id_table(&mut bits, symbols.len())?;
        let ht = text::HuffText {
            fs,
            ds,
            dt,
            rdw,
            rdh,
            rdx,
            rdy,
            rsize,
            ids: Some(ids),
            symcodelen,
        };
        let mut gr = gr;
        let mut c = text::Coder::Huff {
            bits: &mut bits,
            t: &ht,
            gr: &mut gr,
        };
        Ok(text::decode(&mut c, &p, budget)?.0)
    } else {
        let mut cx = text::TextCx::new(symcodelen, gr);
        let mut m = mq::Mq::new(body);
        let mut c = text::Coder::Arith {
            mq: &mut m,
            cx: &mut cx,
        };
        Ok(text::decode(&mut c, &p, budget)?.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_magic() {
        assert!(probe(&MAGIC));
        assert!(!probe(b"\x97JB2\r\n\x1a"));
        assert_eq!(info(b"nope", 0), Err(Error::NotRecognised));
    }
}
