//! One font face: its tables and what they say (names, metrics, embedding, axes, colour),
//! its character map, advances, glyph names and outlines.

use std::sync::Arc;

use crate::Error;
use crate::cff::Cff;
use crate::glyf::Glyf;
use crate::names_std::{MAC_GLYPHS, MAC_ROMAN_HIGH};
use crate::outline::Outline;
use crate::read::{Reader, i16_at, slice, u16_at, u32_at};
use crate::sfnt::{self, TableRecord};

/// The kind of outlines a face has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutlineKind {
    /// Quadratic outlines (`glyf`).
    TrueType,
    /// PostScript outlines (`CFF `).
    Cff,
    /// PostScript outlines with variations (`CFF2`).
    Cff2,
    /// Bitmaps only (`sbix`, `CBDT`, `EBDT`).
    BitmapOnly,
    /// Neither outlines nor bitmaps.
    None,
}

/// What the font's licence lets a document that embeds it do (OS/2 `fsType`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Permission {
    /// May be embedded and installed for good.
    Installable,
    /// May not be embedded.
    Restricted,
    /// May be embedded to view and print the document only.
    PreviewPrint,
    /// May be embedded, and the document edited.
    Editable,
}

/// The OS/2 `fsType` embedding permissions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Embedding {
    pub permission: Permission,
    /// The font may not be subset before embedding.
    pub no_subsetting: bool,
    /// Only the font's bitmaps may be embedded.
    pub bitmap_only: bool,
    /// The raw `fsType` value.
    pub raw: u16,
}

impl Embedding {
    pub(crate) fn from_fs_type(v: u16) -> Embedding {
        let permission = if v & 0x0008 != 0 {
            Permission::Editable
        } else if v & 0x0004 != 0 {
            Permission::PreviewPrint
        } else if v & 0x0002 != 0 {
            Permission::Restricted
        } else {
            Permission::Installable
        };
        Embedding {
            permission,
            no_subsetting: v & 0x0100 != 0,
            bitmap_only: v & 0x0200 != 0,
            raw: v,
        }
    }
}

/// The name table's entries the viewer shows (empty when the font has none).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Names {
    pub copyright: String,
    pub family: String,
    pub subfamily: String,
    pub unique_id: String,
    pub full_name: String,
    pub version: String,
    pub postscript: String,
    pub trademark: String,
    pub manufacturer: String,
    pub designer: String,
    pub description: String,
    pub vendor_url: String,
    pub designer_url: String,
    pub licence: String,
    pub licence_url: String,
    pub typographic_family: String,
    pub typographic_subfamily: String,
    pub sample_text: String,
}

/// A variation axis (`fvar`).
#[derive(Clone, Debug, PartialEq)]
pub struct Axis {
    pub tag: [u8; 4],
    /// From the name table; the tag when it has none.
    pub name: String,
    pub min: f32,
    pub default: f32,
    pub max: f32,
    /// The font asks for the axis not to be shown.
    pub hidden: bool,
}

impl Axis {
    pub fn tag_str(&self) -> String {
        String::from_utf8_lossy(&self.tag).into_owned()
    }
}

/// A named instance (`fvar`): a style of a variable font.
#[derive(Clone, Debug, PartialEq)]
pub struct Instance {
    pub name: String,
    pub coords: Vec<f32>,
    pub postscript: Option<String>,
}

/// The colour tables of a face.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Colour {
    /// `COLR` version, and its colour glyphs (version 0 records plus version 1 list).
    pub colr: Option<(u16, u32)>,
    /// `CPAL` palettes, each a list of RGBA colours.
    pub palettes: Vec<Vec<[u8; 4]>>,
    /// `sbix` strikes' pixels per em.
    pub sbix: Vec<u16>,
    /// `CBDT`/`CBLC` strikes' pixels per em (empty without them).
    pub cbdt: Vec<u8>,
    /// An `SVG ` table.
    pub svg: bool,
}

impl Colour {
    pub fn any(&self) -> bool {
        self.colr.is_some() || !self.sbix.is_empty() || !self.cbdt.is_empty() || self.svg
    }
}

/// Size and vertical metrics in font units.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Metrics {
    pub units_per_em: u16,
    pub ascender: i16,
    pub descender: i16,
    pub line_gap: i16,
    pub cap_height: Option<i16>,
    pub x_height: Option<i16>,
}

/// One face of a font file.
#[derive(Debug)]
pub struct Face {
    data: Arc<Vec<u8>>,
    index: u32,
    pub tables: Vec<TableRecord>,
    pub names: Names,
    pub glyph_count: u16,
    pub metrics: Metrics,
    pub weight: u16,
    pub width: u16,
    pub italic: bool,
    pub monospaced: bool,
    pub outlines: OutlineKind,
    pub embedding: Option<Embedding>,
    pub axes: Vec<Axis>,
    pub instances: Vec<Instance>,
    pub avar: bool,
    pub colour: Colour,
    chars: Vec<(u32, u16)>,
    num_hmetrics: u16,
}

impl Face {
    /// The face at `index` of `data` (an SFNT, or a collection whose font `index` starts at
    /// `offset`).
    pub(crate) fn parse(data: Arc<Vec<u8>>, index: u32, offset: usize) -> Result<Face, Error> {
        let (version, tables) = sfnt::directory(&data, offset)?;
        if version == sfnt::TYP1 {
            return Err(Error::Type1);
        }
        let d: &[u8] = &data;
        let table = |tag: &[u8; 4]| -> Option<&[u8]> {
            let t = tables.iter().find(|t| &t.tag == tag)?;
            slice(d, t.offset as usize, t.length as usize)
        };
        let head = table(b"head").ok_or(Error::MissingTable("head"))?;
        let units_per_em = u16_at(head, 18).ok_or(Error::Truncated)?;
        let mac_style = u16_at(head, 44).unwrap_or(0);
        let maxp = table(b"maxp").ok_or(Error::MissingTable("maxp"))?;
        let mut glyph_count = u16_at(maxp, 4).ok_or(Error::Truncated)?;
        if glyph_count == 0 {
            return Err(Error::NoGlyphs);
        }
        let name = table(b"name");
        let names = name.map(read_names).unwrap_or_default();
        let hhea = table(b"hhea");
        let os2 = table(b"OS/2");
        let post = table(b"post");
        let mut metrics = Metrics {
            units_per_em,
            ..Default::default()
        };
        if let Some(h) = hhea {
            metrics.ascender = i16_at(h, 4).unwrap_or(0);
            metrics.descender = i16_at(h, 6).unwrap_or(0);
            metrics.line_gap = i16_at(h, 8).unwrap_or(0);
        }
        let (mut weight, mut width, mut italic) = (400, 5, mac_style & 2 != 0);
        let mut embedding = None;
        if let Some(o) = os2 {
            let ver = u16_at(o, 0).unwrap_or(0);
            weight = u16_at(o, 4).unwrap_or(400);
            width = u16_at(o, 6).unwrap_or(5);
            embedding = u16_at(o, 8).map(Embedding::from_fs_type);
            if let Some(sel) = u16_at(o, 62) {
                italic = sel & 1 != 0;
            }
            if ver >= 2 {
                metrics.x_height = i16_at(o, 86);
                metrics.cap_height = i16_at(o, 88);
            }
        }
        let monospaced = post.and_then(|p| u32_at(p, 12)).is_some_and(|v| v != 0);
        let has = |t: &[u8; 4]| tables.iter().any(|r| &r.tag == t);
        let outlines = if has(b"glyf") && has(b"loca") {
            OutlineKind::TrueType
        } else if has(b"CFF ") {
            OutlineKind::Cff
        } else if has(b"CFF2") {
            OutlineKind::Cff2
        } else if has(b"sbix") || has(b"CBDT") || has(b"EBDT") {
            OutlineKind::BitmapOnly
        } else {
            OutlineKind::None
        };
        if matches!(outlines, OutlineKind::Cff | OutlineKind::Cff2) {
            let cff2 = outlines == OutlineKind::Cff2;
            let t = table(if cff2 { b"CFF2" } else { b"CFF " });
            if let Some(c) = t.and_then(|t| Cff::parse(t, cff2)) {
                glyph_count = glyph_count.min(c.glyph_count().min(u16::MAX as u32) as u16);
            }
        }
        let name_of = |id: u16| name.and_then(|n| name_by_id(n, id));
        let (axes, instances) = table(b"fvar")
            .map(|f| read_fvar(f, &name_of))
            .unwrap_or_default();
        let colour = read_colour(&table);
        let chars = table(b"cmap")
            .map(|c| crate::cmap::parse(c, glyph_count))
            .unwrap_or_default();
        let num_hmetrics = hhea.and_then(|h| u16_at(h, 34)).unwrap_or(0);
        let avar = has(b"avar");
        Ok(Face {
            data: data.clone(),
            index,
            tables,
            names,
            glyph_count,
            metrics,
            weight,
            width,
            italic,
            monospaced,
            outlines,
            embedding,
            axes,
            instances,
            avar,
            colour,
            chars,
            num_hmetrics,
        })
    }

    /// The font data (an SFNT, or the whole collection) for the text stack.
    pub fn sfnt(&self) -> Arc<Vec<u8>> {
        self.data.clone()
    }

    /// The face's index in [`Face::sfnt`] (0 unless it is a collection).
    pub fn index(&self) -> u32 {
        self.index
    }

    /// The bytes of table `tag`.
    pub fn table(&self, tag: &[u8; 4]) -> Option<&[u8]> {
        let t = self.tables.iter().find(|t| &t.tag == tag)?;
        slice(&self.data, t.offset as usize, t.length as usize)
    }

    /// Mapped characters, sorted by code point.
    pub fn chars(&self) -> &[(u32, u16)] {
        &self.chars
    }

    pub fn glyph_index(&self, c: char) -> Option<u16> {
        let c = c as u32;
        self.chars
            .binary_search_by_key(&c, |p| p.0)
            .ok()
            .map(|i| self.chars[i].1)
    }

    /// The advance width of glyph `gid` in font units.
    pub fn advance(&self, gid: u16) -> Option<u16> {
        if gid >= self.glyph_count || self.num_hmetrics == 0 {
            return None;
        }
        let hmtx = self.table(b"hmtx")?;
        let i = gid.min(self.num_hmetrics - 1) as usize;
        u16_at(hmtx, i * 4)
    }

    /// The glyph's name (`post` version 2, or the CFF charset).
    pub fn glyph_name(&self, gid: u16) -> Option<String> {
        if let Some(p) = self.table(b"post")
            && u32_at(p, 0) == Some(0x0002_0000)
        {
            let n = u16_at(p, 32)?;
            if gid >= n {
                return None;
            }
            let i = u16_at(p, 34 + gid as usize * 2)? as usize;
            if i < 258 {
                return Some(MAC_GLYPHS[i].to_owned());
            }
            let mut r = Reader::at(p, 34 + n as usize * 2);
            for _ in 0..i - 258 {
                let l = r.u8()? as usize;
                r.skip(l)?;
            }
            let l = r.u8()? as usize;
            return Some(String::from_utf8_lossy(r.bytes(l)?).into_owned());
        }
        if self.outlines == OutlineKind::Cff {
            return Cff::parse(self.table(b"CFF ")?, false)?.glyph_name(gid);
        }
        None
    }

    /// Reads outlines of many glyphs with the outline tables parsed once.
    pub fn outliner(&self) -> Outliner<'_> {
        let inner = match self.outlines {
            OutlineKind::TrueType => (|| {
                let head = self.table(b"head")?;
                Some(Inner::Glyf(Glyf {
                    glyf: self.table(b"glyf")?,
                    loca: self.table(b"loca")?,
                    long: i16_at(head, 50)? != 0,
                    num_glyphs: self.glyph_count,
                }))
            })(),
            OutlineKind::Cff => self
                .table(b"CFF ")
                .and_then(|t| Cff::parse(t, false))
                .map(Inner::Cff),
            OutlineKind::Cff2 => self
                .table(b"CFF2")
                .and_then(|t| Cff::parse(t, true))
                .map(Inner::Cff),
            _ => None,
        };
        Outliner { inner }
    }

    /// The outline of glyph `gid` (`None` for a glyph without one).
    pub fn outline(&self, gid: u16) -> Option<Outline> {
        self.outliner().outline(gid)
    }
}

enum Inner<'a> {
    Glyf(Glyf<'a>),
    Cff(Cff<'a>),
}

/// The outline tables of a face, parsed ([`Face::outliner`]).
pub struct Outliner<'a> {
    inner: Option<Inner<'a>>,
}

impl Outliner<'_> {
    pub fn outline(&self, gid: u16) -> Option<Outline> {
        match self.inner.as_ref()? {
            Inner::Glyf(g) => g.outline(gid),
            Inner::Cff(c) => c.outline(gid),
        }
    }
}

fn utf16(b: &[u8]) -> String {
    let u: Vec<u16> = b
        .chunks_exact(2)
        .map(|c| u16::from_be_bytes([c[0], c[1]]))
        .collect();
    String::from_utf16_lossy(&u)
}

fn mac_roman(b: &[u8]) -> String {
    b.iter()
        .map(|&c| {
            if c < 0x80 {
                c as char
            } else {
                char::from_u32(MAC_ROMAN_HIGH[c as usize - 0x80] as u32).unwrap_or('\u{FFFD}')
            }
        })
        .collect()
}

/// How much a name record is preferred (lower is better); `None` when it cannot be decoded.
fn name_rank(platform: u16, encoding: u16, lang: u16) -> Option<u8> {
    Some(match (platform, encoding) {
        (3, 0 | 1 | 10) if lang == 0x0409 => 0,
        (3, 0 | 1 | 10) if lang & 0xFF == 0x09 => 1,
        (3, 0 | 1 | 10) => 2,
        (0, _) => 3,
        (1, 0) if lang == 0 => 4,
        (1, 0) => 5,
        _ => return None,
    })
}

/// The best record of name `id`.
pub(crate) fn name_by_id(n: &[u8], id: u16) -> Option<String> {
    let count = u16_at(n, 2)? as usize;
    let storage = u16_at(n, 4)? as usize;
    let mut best: Option<(u8, String)> = None;
    for i in 0..count {
        let mut r = Reader::at(n, 6 + i * 12);
        let (p, e, l, nid, len, off) = (r.u16()?, r.u16()?, r.u16()?, r.u16()?, r.u16()?, r.u16()?);
        if nid != id {
            continue;
        }
        let Some(k) = name_rank(p, e, l) else {
            continue;
        };
        if best.as_ref().is_some_and(|b| b.0 <= k) {
            continue;
        }
        let Some(b) = slice(n, storage + off as usize, len as usize) else {
            continue;
        };
        let s = if p == 1 { mac_roman(b) } else { utf16(b) };
        best = Some((k, s));
    }
    best.map(|b| b.1)
}

fn read_names(n: &[u8]) -> Names {
    let g = |id| name_by_id(n, id).unwrap_or_default();
    Names {
        copyright: g(0),
        family: g(1),
        subfamily: g(2),
        unique_id: g(3),
        full_name: g(4),
        version: g(5),
        postscript: g(6),
        trademark: g(7),
        manufacturer: g(8),
        designer: g(9),
        description: g(10),
        vendor_url: g(11),
        designer_url: g(12),
        licence: g(13),
        licence_url: g(14),
        typographic_family: g(16),
        typographic_subfamily: g(17),
        sample_text: g(19),
    }
}

/// Most axes and instances read.
const MAX_AXES: usize = 64;
const MAX_INSTANCES: usize = 1024;

fn read_fvar(f: &[u8], name_of: &dyn Fn(u16) -> Option<String>) -> (Vec<Axis>, Vec<Instance>) {
    let (Some(off), Some(n), Some(size), Some(ni), Some(isize)) = (
        u16_at(f, 4),
        u16_at(f, 8),
        u16_at(f, 10),
        u16_at(f, 12),
        u16_at(f, 14),
    ) else {
        return Default::default();
    };
    let n = (n as usize).min(MAX_AXES);
    let mut axes = Vec::with_capacity(n);
    for i in 0..n {
        let mut r = Reader::at(f, off as usize + i * size as usize);
        let (Some(tag), Some(min), Some(def), Some(max), Some(flags), Some(nid)) =
            (r.tag(), r.fixed(), r.fixed(), r.fixed(), r.u16(), r.u16())
        else {
            break;
        };
        axes.push(Axis {
            tag,
            name: name_of(nid).unwrap_or_else(|| String::from_utf8_lossy(&tag).into_owned()),
            min,
            default: def,
            max,
            hidden: flags & 1 != 0,
        });
    }
    let mut instances = Vec::new();
    let base = off as usize + n * size as usize;
    let has_ps = isize as usize >= axes.len() * 4 + 6;
    for i in 0..(ni as usize).min(MAX_INSTANCES) {
        let mut r = Reader::at(f, base + i * isize as usize);
        let (Some(nid), Some(_)) = (r.u16(), r.u16()) else {
            break;
        };
        let coords: Option<Vec<f32>> = axes.iter().map(|_| r.fixed()).collect();
        let Some(coords) = coords else { break };
        let postscript = if has_ps {
            r.u16().filter(|&id| id != 0xFFFF).and_then(name_of)
        } else {
            None
        };
        instances.push(Instance {
            name: name_of(nid).unwrap_or_default(),
            coords,
            postscript,
        });
    }
    (axes, instances)
}

/// Most palette entries read.
const MAX_COLOURS: usize = 1 << 16;

fn read_colour<'a>(table: &dyn Fn(&[u8; 4]) -> Option<&'a [u8]>) -> Colour {
    let mut c = Colour::default();
    if let Some(t) = table(b"COLR")
        && let Some(v) = u16_at(t, 0)
    {
        let mut n = u16_at(t, 2).unwrap_or(0) as u32;
        if v >= 1
            && let Some(o) = u32_at(t, 14).filter(|&o| o != 0)
        {
            n += u32_at(t, o as usize).unwrap_or(0);
        }
        c.colr = Some((v, n));
    }
    if let Some(t) = table(b"CPAL") {
        let entries = u16_at(t, 2).unwrap_or(0) as usize;
        let n = u16_at(t, 4).unwrap_or(0) as usize;
        let recs = u32_at(t, 8).unwrap_or(0) as usize;
        let mut total = 0usize;
        for p in 0..n {
            let Some(first) = u16_at(t, 12 + p * 2) else {
                break;
            };
            total += entries;
            if total > MAX_COLOURS {
                break;
            }
            let pal: Vec<[u8; 4]> = (0..entries)
                .filter_map(|i| {
                    let b = slice(t, recs + (first as usize + i) * 4, 4)?;
                    Some([b[2], b[1], b[0], b[3]])
                })
                .collect();
            c.palettes.push(pal);
        }
    }
    if let Some(t) = table(b"sbix") {
        let n = u32_at(t, 4).unwrap_or(0).min(256) as usize;
        for i in 0..n {
            if let Some(ppem) = u32_at(t, 8 + i * 4).and_then(|o| u16_at(t, o as usize)) {
                c.sbix.push(ppem);
            }
        }
    }
    if table(b"CBDT").is_some()
        && let Some(t) = table(b"CBLC")
    {
        let n = u32_at(t, 4).unwrap_or(0).min(256) as usize;
        for i in 0..n {
            if let Some(&p) = t.get(8 + i * 48 + 44) {
                c.cbdt.push(p);
            }
        }
    }
    c.svg = table(b"SVG ").is_some();
    c
}
