//! ERDAS IMAGINE (`.img`, HFA: Hierarchical File Architecture).
//!
//! The file starts with `EHFA_HEADER_TAG` and a pointer to the file record, which points
//! to a tree of entries and a data dictionary: a text listing of record types, each a
//! sequence of typed fields (`1:lwidth,`, `0:pcproName,`, `1:e2:false,true,logvalid,`,
//! `1:*oEprj_Coordinate,upperLeftCenter,`). Every entry names its record type, so the
//! records here are read by interpreting the file's own dictionary.
//!
//! Each band is an `Eimg_Layer` entry with a `RasterDMS` child listing its tiles
//! (uncompressed, or ERDAS's run-length coding), a `Map_Info` with the upper-left pixel
//! centre and pixel size, a `Projection` with GCTP-style parameters and its `Datum`, and
//! optionally an `Eimg_NonInitializedValue` (nodata) and `Statistics`. Bands whose tiles
//! live in an external `.ige` spill file are not read.

use std::collections::{HashMap, HashSet};

use rayon::prelude::*;

use crate::crs::{self, Crs};
use crate::envi::{datum_ellipsoid, datum_epsg, unit_named, utm_crs};
use crate::geotiff::Affine2;
use crate::proj::{Ellipsoid, Method};

const MAGIC: &[u8] = b"EHFA_HEADER_TAG";
const MAX_NODES: usize = 200_000;
/// The largest tile, in samples: GDAL writes at most 2048 × 2048, and each decoding thread
/// holds one tile of f64s (32 MB at this cap).
const MAX_BLOCK_SAMPLES: u64 = 1 << 22;
/// The most tiles a band may have (64 × 64 tiles of a 131072² image).
const MAX_TILES: u64 = 1 << 22;

/// Whether the bytes start an ERDAS IMAGINE file.
pub fn sniff(head: &[u8]) -> bool {
    head.starts_with(MAGIC)
}

/// A value read through the dictionary.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Int(i64),
    Float(f64),
    Text(String),
    /// An enumeration: index and name.
    Enum(u16, String),
    Array(Vec<Value>),
    Object(Record),
    /// Base data: rows × columns of values of an IMAGINE pixel type.
    Base {
        rows: u32,
        cols: u32,
        values: Vec<f64>,
    },
}

/// A record's fields in order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Record(pub Vec<(String, Value)>);

impl Record {
    /// A field (a one-element array unwrapped).
    pub fn get(&self, name: &str) -> Option<&Value> {
        let v = self.0.iter().find(|(n, _)| n == name).map(|(_, v)| v)?;
        match v {
            Value::Array(a) if a.len() == 1 => a.first(),
            v => Some(v),
        }
    }
    pub fn num(&self, name: &str) -> Option<f64> {
        self.get(name)?.num()
    }
    pub fn text(&self, name: &str) -> Option<&str> {
        match self.get(name)? {
            Value::Text(s) => Some(s.as_str()).filter(|s| !s.is_empty()),
            Value::Enum(_, s) => Some(s.as_str()),
            _ => None,
        }
    }
    pub fn object(&self, name: &str) -> Option<&Record> {
        match self.get(name)? {
            Value::Object(r) => Some(r),
            _ => None,
        }
    }
}

impl Value {
    pub fn num(&self) -> Option<f64> {
        match self {
            Value::Int(i) => Some(*i as f64),
            Value::Float(f) => Some(*f),
            Value::Enum(i, _) => Some(f64::from(*i)),
            Value::Array(a) if a.len() == 1 => a[0].num(),
            Value::Base { values, .. } => values.first().copied(),
            _ => None,
        }
    }
    /// The numbers of an array (or of base data).
    pub fn numbers(&self) -> Vec<f64> {
        match self {
            Value::Array(a) => a.iter().filter_map(Value::num).collect(),
            Value::Base { values, .. } => values.clone(),
            v => v.num().into_iter().collect(),
        }
    }
}

#[derive(Clone, Debug)]
struct Field {
    count: u32,
    pointer: bool,
    kind: u8,
    enums: Vec<String>,
    object: Option<String>,
    name: String,
}

#[derive(Clone, Debug, Default)]
struct TypeDef {
    fields: Vec<Field>,
}

struct Cursor<'a> {
    s: &'a [u8],
    i: usize,
}

impl Cursor<'_> {
    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }
    fn skip_ws(&mut self) {
        while self.peek().is_some_and(|b| b.is_ascii_whitespace()) {
            self.i += 1;
        }
    }
    fn eat(&mut self, b: u8) -> Option<()> {
        self.skip_ws();
        (self.peek()? == b).then(|| self.i += 1)
    }
    fn number(&mut self) -> Option<u32> {
        self.skip_ws();
        let start = self.i;
        while self.peek().is_some_and(|b| b.is_ascii_digit()) {
            self.i += 1;
        }
        std::str::from_utf8(&self.s[start..self.i])
            .ok()?
            .parse()
            .ok()
    }
    /// Text up to the next comma, which is consumed.
    fn until_comma(&mut self) -> Option<String> {
        let start = self.i;
        while self.peek()? != b',' {
            self.i += 1;
        }
        let t = String::from_utf8_lossy(&self.s[start..self.i])
            .trim()
            .to_string();
        self.i += 1;
        Some(t)
    }
}

/// Parses `{fields}Name,`; inline types (`x{...}Name,`) are added to `dict` too.
fn parse_type(
    c: &mut Cursor,
    dict: &mut HashMap<String, TypeDef>,
    depth: u32,
) -> Option<(String, TypeDef)> {
    if depth > 16 {
        return None;
    }
    c.eat(b'{')?;
    let mut t = TypeDef::default();
    loop {
        c.skip_ws();
        if c.peek()? == b'}' {
            c.i += 1;
            break;
        }
        let count = c.number()?;
        c.eat(b':')?;
        let mut pointer = false;
        if matches!(c.peek()?, b'p' | b'*') {
            pointer = true;
            c.i += 1;
        }
        let kind = c.peek()?;
        c.i += 1;
        let mut enums = Vec::new();
        let mut object = None;
        match kind {
            b'e' => {
                let n = c.number()?;
                c.eat(b':')?;
                for _ in 0..n.min(4096) {
                    enums.push(c.until_comma()?);
                }
            }
            b'o' => object = Some(c.until_comma()?),
            b'x' => {
                c.skip_ws();
                if c.peek()? == b'{' {
                    let (name, inner) = parse_type(c, dict, depth + 1)?;
                    dict.insert(name.clone(), inner);
                    object = Some(name);
                } else {
                    object = Some(c.until_comma()?);
                }
            }
            _ => {}
        }
        let name = c.until_comma()?;
        t.fields.push(Field {
            count,
            pointer,
            kind,
            enums,
            object,
            name,
        });
        if t.fields.len() > 4096 {
            return None;
        }
    }
    let name = c.until_comma()?;
    Some((name, t))
}

fn parse_dictionary(s: &[u8]) -> HashMap<String, TypeDef> {
    let mut dict = HashMap::new();
    let mut c = Cursor { s, i: 0 };
    loop {
        c.skip_ws();
        if c.peek() != Some(b'{') {
            break;
        }
        match parse_type(&mut c, &mut dict, 0) {
            Some((name, t)) => {
                dict.insert(name, t);
            }
            None => break,
        }
    }
    dict
}

fn u16_at(d: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(d.get(at..at + 2)?.try_into().ok()?))
}
fn u32_at(d: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(d.get(at..at + 4)?.try_into().ok()?))
}

/// An IMAGINE pixel type (the `pixelType` enumeration's order).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelType {
    U1,
    U2,
    U4,
    U8,
    S8,
    U16,
    S16,
    U32,
    S32,
    F32,
    F64,
    C64,
    C128,
}

impl PixelType {
    pub fn from_index(i: u16) -> Option<PixelType> {
        use PixelType::*;
        Some(match i {
            0 => U1,
            1 => U2,
            2 => U4,
            3 => U8,
            4 => S8,
            5 => U16,
            6 => S16,
            7 => U32,
            8 => S32,
            9 => F32,
            10 => F64,
            11 => C64,
            12 => C128,
            _ => return None,
        })
    }
    /// Bits per sample.
    pub fn bits(self) -> u32 {
        use PixelType::*;
        match self {
            U1 => 1,
            U2 => 2,
            U4 => 4,
            U8 | S8 => 8,
            U16 | S16 => 16,
            U32 | S32 | F32 => 32,
            F64 | C64 => 64,
            C128 => 128,
        }
    }
    pub fn name(self) -> &'static str {
        use PixelType::*;
        match self {
            U1 => "1-bit unsigned",
            U2 => "2-bit unsigned",
            U4 => "4-bit unsigned",
            U8 => "8-bit unsigned",
            S8 => "8-bit signed",
            U16 => "16-bit unsigned",
            S16 => "16-bit signed",
            U32 => "32-bit unsigned",
            S32 => "32-bit signed",
            F32 => "32-bit float",
            F64 => "64-bit float",
            C64 => "complex 32-bit float",
            C128 => "complex 64-bit float",
        }
    }
    pub fn is_float(self) -> bool {
        matches!(
            self,
            PixelType::F32 | PixelType::F64 | PixelType::C64 | PixelType::C128
        )
    }
    /// Sample `i` of little-endian packed data (complex: magnitude).
    fn read(self, d: &[u8], i: usize) -> Option<f64> {
        use PixelType::*;
        let n = |len: usize| d.get(i * len..i * len + len);
        Some(match self {
            U1 => f64::from((d.get(i / 8)? >> (i % 8)) & 1),
            U2 => f64::from((d.get(i / 4)? >> ((i % 4) * 2)) & 3),
            U4 => f64::from((d.get(i / 2)? >> ((i % 2) * 4)) & 15),
            U8 => f64::from(*d.get(i)?),
            S8 => f64::from(*d.get(i)? as i8),
            U16 => f64::from(u16::from_le_bytes(n(2)?.try_into().ok()?)),
            S16 => f64::from(i16::from_le_bytes(n(2)?.try_into().ok()?)),
            U32 => f64::from(u32::from_le_bytes(n(4)?.try_into().ok()?)),
            S32 => f64::from(i32::from_le_bytes(n(4)?.try_into().ok()?)),
            F32 => f64::from(f32::from_le_bytes(n(4)?.try_into().ok()?)),
            F64 => f64::from_le_bytes(n(8)?.try_into().ok()?),
            C64 => {
                let b = n(8)?;
                let re = f32::from_le_bytes(b[..4].try_into().ok()?);
                let im = f32::from_le_bytes(b[4..].try_into().ok()?);
                f64::from(re).hypot(f64::from(im))
            }
            C128 => {
                let b = n(16)?;
                let re = f64::from_le_bytes(b[..8].try_into().ok()?);
                let im = f64::from_le_bytes(b[8..].try_into().ok()?);
                re.hypot(im)
            }
        })
    }
}

#[derive(Clone, Debug)]
struct Node {
    name: String,
    ty: String,
    data: usize,
    size: usize,
    children: Vec<usize>,
}

/// One tile's place.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Block {
    pub offset: u64,
    pub size: u32,
    pub valid: bool,
    pub compressed: bool,
    /// In an external spill file.
    pub external: bool,
}

/// A band.
#[derive(Clone, Debug)]
pub struct Band {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub pixel: PixelType,
    pub block_width: u32,
    pub block_height: u32,
    pub blocks: Vec<Block>,
    pub nodata: Option<f64>,
    /// "thematic" or "athematic".
    pub layer_type: Option<String>,
    /// Statistics stored with the band (minimum, maximum, mean, median, mode, stddev).
    pub statistics: Vec<(String, f64)>,
    /// Why the band cannot be read, when it cannot.
    pub unreadable: Option<String>,
    /// Where the tiles are when they live in an external spill file (`.ige`).
    pub spill: Option<Spill>,
}

/// A layer kept in an external spill file: its tiles all uncompressed, in block order,
/// the layers of a stack interleaved tile by tile, with a validity bitmap per layer.
#[derive(Clone, Debug, PartialEq)]
pub struct Spill {
    /// The spill file's name, beside the `.img`.
    pub file: String,
    pub flags_offset: u64,
    pub data_offset: u64,
    pub stack_count: u32,
    pub stack_index: u32,
}

/// A parsed file.
#[derive(Clone, Debug)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub bands: Vec<Band>,
    /// Pixel corner to map.
    pub transform: Option<Affine2>,
    pub crs: Option<Crs>,
    pub crs_note: Option<String>,
    /// Map units as the file names them.
    pub units: Option<String>,
    /// Facts for the info panel.
    pub facts: Vec<(String, String)>,
}

struct Hfa<'a> {
    data: &'a [u8],
    dict: HashMap<String, TypeDef>,
    nodes: Vec<Node>,
}

impl<'a> Hfa<'a> {
    fn parse(data: &'a [u8]) -> Result<Hfa<'a>, String> {
        if !sniff(data) {
            return Err("not an ERDAS IMAGINE file".into());
        }
        let file = u32_at(data, 16).ok_or("truncated header")? as usize;
        let root = u32_at(data, file + 8).ok_or("truncated file record")? as usize;
        let dict_at = u32_at(data, file + 14).ok_or("truncated file record")? as usize;
        let dict_end = data
            .get(dict_at..)
            .ok_or("dictionary out of range")?
            .iter()
            .position(|&b| b == 0)
            .map_or(data.len(), |p| dict_at + p);
        let dict = parse_dictionary(&data[dict_at..dict_end]);
        if dict.is_empty() {
            return Err("empty data dictionary".into());
        }
        let mut h = Hfa {
            data,
            dict,
            nodes: Vec::new(),
        };
        let mut seen = HashSet::new();
        h.read_node(root, &mut seen, 0).ok_or("bad entry tree")?;
        Ok(h)
    }

    /// Reads the entry at `at` and its subtree; its index.
    fn read_node(&mut self, at: usize, seen: &mut HashSet<usize>, depth: u32) -> Option<usize> {
        if at == 0 || depth > 64 || !seen.insert(at) || self.nodes.len() >= MAX_NODES {
            return None;
        }
        let d = self.data;
        let child = u32_at(d, at + 12)? as usize;
        let data = u32_at(d, at + 16)? as usize;
        let size = u32_at(d, at + 20)? as usize;
        let text = |o: usize, len: usize| -> Option<String> {
            let b = d.get(at + o..at + o + len)?;
            let end = b.iter().position(|&c| c == 0).unwrap_or(len);
            Some(String::from_utf8_lossy(&b[..end]).into_owned())
        };
        let name = text(24, 64)?;
        let ty = text(88, 32)?;
        let idx = self.nodes.len();
        self.nodes.push(Node {
            name,
            ty,
            data,
            size,
            children: Vec::new(),
        });
        let mut next = child;
        let mut children = Vec::new();
        while next != 0 {
            let following = u32_at(d, next).unwrap_or(0) as usize;
            match self.read_node(next, seen, depth + 1) {
                Some(c) => children.push(c),
                None => break,
            }
            next = following;
        }
        self.nodes[idx].children = children;
        Some(idx)
    }

    fn child(&self, node: usize, name: &str) -> Option<usize> {
        self.nodes[node]
            .children
            .iter()
            .copied()
            .find(|&c| self.nodes[c].name == name)
    }

    fn child_of_type(&self, node: usize, ty: &str) -> Option<usize> {
        self.nodes[node]
            .children
            .iter()
            .copied()
            .find(|&c| self.nodes[c].ty == ty)
    }

    /// The entry's record read through the dictionary.
    fn record(&self, node: usize) -> Option<Record> {
        let n = &self.nodes[node];
        let t = self.dict.get(&n.ty)?;
        let d = self.data.get(n.data..n.data.checked_add(n.size)?)?;
        let mut pos = 0;
        self.read_record(t, d, &mut pos, 0)
    }

    fn read_record(&self, t: &TypeDef, d: &[u8], pos: &mut usize, depth: u32) -> Option<Record> {
        if depth > 16 {
            return None;
        }
        let mut r = Record::default();
        for f in &t.fields {
            match self.read_field(f, d, pos, depth) {
                Some(v) => r.0.push((f.name.clone(), v)),
                // A record cut short keeps the fields read so far.
                None => break,
            }
        }
        Some(r)
    }

    fn read_field(&self, f: &Field, d: &[u8], pos: &mut usize, depth: u32) -> Option<Value> {
        let count = if f.pointer {
            let c = u32_at(d, *pos)?;
            *pos += 8;
            c
        } else {
            f.count
        };
        if count as usize > d.len().saturating_sub(*pos) + 1 {
            return None;
        }
        match f.kind {
            b'c' | b'C' => {
                let b = d.get(*pos..*pos + count as usize)?;
                *pos += count as usize;
                let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
                Some(Value::Text(String::from_utf8_lossy(&b[..end]).into_owned()))
            }
            b'b' => {
                if count == 0 {
                    return Some(Value::Base {
                        rows: 0,
                        cols: 0,
                        values: Vec::new(),
                    });
                }
                let rows = u32_at(d, *pos)?;
                let cols = u32_at(d, *pos + 4)?;
                let ty = PixelType::from_index(u16_at(d, *pos + 8)?)?;
                *pos += 12;
                // Bounded by the record's bytes before anything is allocated.
                let n = (rows as usize).checked_mul(cols as usize)?;
                let bytes = n.checked_mul(ty.bits() as usize)?.div_ceil(8);
                let b = d.get(*pos..pos.checked_add(bytes)?)?;
                *pos += bytes;
                let values = (0..n).filter_map(|i| ty.read(b, i)).collect();
                Some(Value::Base { rows, cols, values })
            }
            _ => {
                let mut items = Vec::with_capacity(count.min(1 << 16) as usize);
                for _ in 0..count {
                    // Every item must use some of the record's bytes: a nested type that
                    // reads none (`{N:oA}A`) would repeat N items at each of 16 levels.
                    let before = *pos;
                    items.push(self.read_item(f, d, pos, depth)?);
                    if *pos == before {
                        return None;
                    }
                }
                Some(if count == 1 {
                    items.pop()?
                } else {
                    Value::Array(items)
                })
            }
        }
    }

    fn read_item(&self, f: &Field, d: &[u8], pos: &mut usize, depth: u32) -> Option<Value> {
        let take = |pos: &mut usize, n: usize| -> Option<&[u8]> {
            let b = d.get(*pos..*pos + n)?;
            *pos += n;
            Some(b)
        };
        Some(match f.kind {
            b'1' | b'2' | b'4' => Value::Int(i64::from(take(pos, 1)?[0])),
            b'e' => {
                let i = u16::from_le_bytes(take(pos, 2)?.try_into().ok()?);
                Value::Enum(i, f.enums.get(i as usize).cloned().unwrap_or_default())
            }
            b's' => Value::Int(i64::from(u16::from_le_bytes(
                take(pos, 2)?.try_into().ok()?,
            ))),
            b'S' => Value::Int(i64::from(i16::from_le_bytes(
                take(pos, 2)?.try_into().ok()?,
            ))),
            b't' | b'l' => Value::Int(i64::from(u32::from_le_bytes(
                take(pos, 4)?.try_into().ok()?,
            ))),
            b'L' => Value::Int(i64::from(i32::from_le_bytes(
                take(pos, 4)?.try_into().ok()?,
            ))),
            b'f' => Value::Float(f64::from(f32::from_le_bytes(
                take(pos, 4)?.try_into().ok()?,
            ))),
            b'd' => Value::Float(f64::from_le_bytes(take(pos, 8)?.try_into().ok()?)),
            b'm' => {
                let b = take(pos, 8)?;
                Value::Float(PixelType::C64.read(b, 0)?)
            }
            b'M' => Value::Float(PixelType::C128.read(take(pos, 16)?, 0)?),
            b'o' | b'x' => {
                let t = self.dict.get(f.object.as_ref()?)?;
                Value::Object(self.read_record(t, d, pos, depth + 1)?)
            }
            _ => return None,
        })
    }
}

/// Parses the file's structure: bands, tiles, georeference.
pub fn open(data: &[u8]) -> Result<Image, String> {
    let h = Hfa::parse(data)?;
    let layers: Vec<usize> = h.nodes[0]
        .children
        .iter()
        .copied()
        .filter(|&c| h.nodes[c].ty == "Eimg_Layer")
        .collect();
    if layers.is_empty() {
        return Err("no image layers".into());
    }
    let mut bands = Vec::new();
    for &l in &layers {
        bands.push(band(&h, l)?);
    }
    let (width, height) = (bands[0].width, bands[0].height);
    bands.retain(|b| b.width == width && b.height == height);
    let first = layers[0];
    let mut facts = Vec::new();
    let map = h.child(first, "Map_Info").and_then(|m| h.record(m));
    let units = map
        .as_ref()
        .and_then(|m| m.text("units"))
        .map(str::to_string);
    let transform = map.as_ref().and_then(|m| {
        let ul = m.object("upperLeftCenter")?;
        let px = m.object("pixelSize")?;
        let (x, y) = (ul.num("x")?, ul.num("y")?);
        let (w, hgt) = (px.num("width")?, px.num("height")?);
        Some(Affine2 {
            a: w,
            b: 0.0,
            c: x - 0.5 * w,
            d: 0.0,
            e: -hgt,
            f: y + 0.5 * hgt,
        })
    });
    let map_proj = map
        .as_ref()
        .and_then(|m| m.text("proName"))
        .map(str::to_string);
    let (crs, crs_note) = projection(&h, first, map_proj.as_deref(), units.as_deref());
    if h.child(first, "MapToPixelXForm").is_some() {
        facts.push((
            "Georeference".to_string(),
            "a polynomial transform (not applied)".to_string(),
        ));
    }
    if let Some(b) = bands.first() {
        facts.push(("Pixel type".into(), b.pixel.name().into()));
        facts.push((
            "Tiles".into(),
            format!("{} × {}", b.block_width, b.block_height),
        ));
        if b.blocks.iter().any(|k| k.compressed) {
            facts.push(("Compression".into(), "run-length".into()));
        }
        if let Some(t) = &b.layer_type {
            facts.push(("Layer type".into(), t.clone()));
        }
    }
    if let Some(p) = &map_proj {
        facts.push(("Projection".into(), p.clone()));
    }
    Ok(Image {
        width,
        height,
        bands,
        transform,
        crs,
        crs_note,
        units,
        facts,
    })
}

fn band(h: &Hfa, layer: usize) -> Result<Band, String> {
    let r = h.record(layer).ok_or("unreadable layer record")?;
    let dim = |k: &str| {
        r.num(k)
            .filter(|v| *v >= 1.0 && *v <= f64::from(1u32 << 24))
            .map(|v| v as u32)
            .ok_or_else(|| format!("bad {k}"))
    };
    let width = dim("width")?;
    let height = dim("height")?;
    let block_width = dim("blockWidth")?;
    let block_height = dim("blockHeight")?;
    if u64::from(block_width) * u64::from(block_height) > MAX_BLOCK_SAMPLES {
        return Err(format!("tiles too large ({block_width} × {block_height})"));
    }
    if u64::from(width.div_ceil(block_width)) * u64::from(height.div_ceil(block_height)) > MAX_TILES
    {
        return Err("too many tiles".into());
    }
    let pixel = match r.get("pixelType") {
        Some(Value::Enum(i, _)) => PixelType::from_index(*i),
        _ => None,
    }
    .ok_or("unknown pixel type")?;
    let mut unreadable = None;
    let mut spill = None;
    let mut blocks = Vec::new();
    if let Some(dms) = h
        .child_of_type(layer, "Edms_State")
        .or_else(|| h.child(layer, "RasterDMS"))
    {
        let s = h.record(dms).ok_or("unreadable tile directory")?;
        let infos = match s.0.iter().find(|(n, _)| n == "blockinfo").map(|(_, v)| v) {
            Some(Value::Array(a)) => a.clone(),
            Some(v @ Value::Object(_)) => vec![v.clone()],
            _ => Vec::new(),
        };
        for v in infos {
            let Value::Object(b) = v else { continue };
            blocks.push(Block {
                offset: b.num("offset").unwrap_or(0.0) as u32 as u64,
                size: b.num("size").unwrap_or(0.0) as u32,
                valid: b.num("logvalid").unwrap_or(0.0) != 0.0,
                compressed: b.num("compressionType").unwrap_or(0.0) != 0.0,
                external: b.num("fileCode").unwrap_or(0.0) != 0.0,
            });
        }
        let need = width.div_ceil(block_width) as usize * height.div_ceil(block_height) as usize;
        if blocks.len() < need {
            unreadable = Some(format!("{} of {need} tiles listed", blocks.len()));
        }
    } else if let Some(n) = h.child(layer, "ExternalRasterDMS") {
        let s = h.record(n).ok_or("unreadable spill file record")?;
        // 64-bit offsets are two 32-bit words, low first.
        let big = |k: &str| -> Option<u64> {
            match s.0.iter().find(|(n, _)| n == k).map(|(_, v)| v)? {
                Value::Array(a) if a.len() == 2 => {
                    let w = |i: usize| a[i].num().map(|v| v as i64 as u32 as u64);
                    Some(w(0)? | (w(1)? << 32))
                }
                v => v.num().map(|v| v as u64),
            }
        };
        spill = (|| {
            Some(Spill {
                file: s.object("fileName")?.text("string")?.to_string(),
                flags_offset: big("layerStackValidFlagsOffset")?,
                data_offset: big("layerStackDataOffset")?,
                stack_count: s.num("layerStackCount").filter(|v| *v >= 1.0)? as u32,
                stack_index: s.num("layerStackIndex").filter(|v| *v >= 0.0)? as u32,
            })
        })();
        if spill.is_none() {
            unreadable = Some("unreadable spill file record".into());
        }
    } else {
        unreadable = Some("no tile directory".into());
    }
    if blocks.iter().any(|b| b.external) {
        unreadable = Some("tiles are in an external spill file (.ige)".into());
    }
    let nodata = h
        .child_of_type(layer, "Eimg_NonInitializedValue")
        .and_then(|n| h.record(n))
        .and_then(|r| r.get("valueBD").and_then(Value::num));
    let mut statistics = Vec::new();
    if let Some(s) = h
        .child_of_type(layer, "Esta_Statistics")
        .and_then(|n| h.record(n))
    {
        for k in ["minimum", "maximum", "mean", "median", "mode", "stddev"] {
            if let Some(v) = s.num(k) {
                statistics.push((k.to_string(), v));
            }
        }
    }
    Ok(Band {
        name: h.nodes[layer].name.clone(),
        width,
        height,
        pixel,
        block_width,
        block_height,
        blocks,
        nodata,
        layer_type: r.text("layerType").map(str::to_string),
        statistics,
        unreadable,
        spill,
    })
}

/// GCTP projection numbers.
const PRO_GEOGRAPHIC: i64 = 0;
const PRO_UTM: i64 = 1;
const PRO_ALBERS: i64 = 3;
const PRO_LCC: i64 = 4;
const PRO_MERCATOR: i64 = 5;
const PRO_TM: i64 = 9;

fn projection(
    h: &Hfa,
    layer: usize,
    map_proj: Option<&str>,
    units: Option<&str>,
) -> (Option<Crs>, Option<String>) {
    // An ESRI PE string, when the file carries one.
    let pe = h
        .child(layer, "ProjectionX")
        .and_then(|n| {
            let n = &h.nodes[n];
            h.data.get(n.data..n.data.checked_add(n.size)?)
        })
        .and_then(find_wkt);
    let pe_crs = pe.as_deref().and_then(crate::wkt::crs_from_wkt);
    let Some(pn) = h.child(layer, "Projection") else {
        if let Some(c) = pe_crs {
            return (Some(c), None);
        }
        let geo = map_proj.is_some_and(|p| p.to_ascii_lowercase().contains("geographic"));
        return if geo {
            (crs::from_epsg(4326), None)
        } else {
            (None, map_proj.filter(|p| !p.is_empty()).map(str::to_string))
        };
    };
    let Some(p) = h.record(pn) else {
        return (pe_crs, None);
    };
    let datum = h
        .child(pn, "Datum")
        .and_then(|d| h.record(d))
        .and_then(|d| d.text("datumname").map(str::to_string))
        .or_else(|| {
            p.object("proSpheroid")
                .and_then(|s| s.text("sphereName"))
                .map(str::to_string)
        })
        .unwrap_or_else(|| "WGS 84".into());
    let number = p.num("proNumber").map_or(-1, |v| v as i64);
    let params: Vec<f64> =
        p.0.iter()
            .find(|(n, _)| n == "proParams")
            .map(|(_, v)| v.numbers())
            .unwrap_or_default();
    let par = |i: usize| params.get(i).copied().unwrap_or(0.0);
    let deg = |i: usize| par(i).to_degrees();
    let ell = p
        .object("proSpheroid")
        .and_then(|s| {
            let (a, b) = (s.num("a")?, s.num("b")?);
            (a > 0.0).then(|| Ellipsoid::inv(a, if b > 0.0 && a != b { a / (a - b) } else { 0.0 }))
        })
        .unwrap_or_else(|| datum_ellipsoid(&datum));
    let unit = unit_named(units);
    let name = p.text("proName").unwrap_or("").to_string();
    let direct = match number {
        PRO_GEOGRAPHIC => crs::from_epsg(datum_epsg(&datum).unwrap_or(4326)),
        PRO_UTM => p
            .num("proZone")
            .and_then(|z| utm_crs(z as u32, par(3) >= 0.0, &datum, &unit)),
        _ => None,
    };
    let method = match number {
        PRO_TM => Some(Method::TransverseMercator {
            lat0: deg(5),
            lon0: deg(4),
            k0: if par(2) > 0.0 { par(2) } else { 1.0 },
            fe: par(6),
            fn_: par(7),
        }),
        PRO_LCC => Some(Method::LambertConic {
            lat0: deg(5),
            lon0: deg(4),
            lat1: deg(2),
            lat2: deg(3),
            k0: 1.0,
            fe: par(6),
            fn_: par(7),
        }),
        PRO_ALBERS => Some(Method::Albers {
            lat0: deg(5),
            lon0: deg(4),
            lat1: deg(2),
            lat2: deg(3),
            fe: par(6),
            fn_: par(7),
        }),
        PRO_MERCATOR => Some(if par(5) == 0.0 {
            Method::Mercator {
                lon0: deg(4),
                k0: 1.0,
                fe: par(6),
                fn_: par(7),
            }
        } else {
            Method::MercatorB {
                lon0: deg(4),
                lat1: deg(5),
                fe: par(6),
                fn_: par(7),
            }
        }),
        _ => None,
    };
    let pro_crs = direct.or_else(|| {
        method.map(|m| {
            let label = if name.is_empty() {
                m.name().to_string()
            } else {
                name.clone()
            };
            let mut c = Crs::custom_projected(label, ell, m, unit.clone());
            c.datum = datum_epsg(&datum)
                .and_then(crs::from_epsg)
                .and_then(|b| b.datum);
            c
        })
    });
    let pro_crs = pro_crs.map(|c| crs::identify(&c).unwrap_or(c));
    match (pro_crs, pe_crs) {
        (Some(c), _) if c.epsg.is_some() => (Some(c), None),
        (_, Some(pe)) => (Some(pe), None),
        (Some(c), None) => (Some(c), None),
        (None, None) => (None, (!name.is_empty()).then_some(name)),
    }
}

/// The WKT text inside an entry's data. The PE string sits in a MIF object (a record
/// serialised inside a byte field), so the bytes are searched rather than the record.
fn find_wkt(data: &[u8]) -> Option<String> {
    let start = ["PROJCS[", "GEOGCS[", "PROJCRS[", "GEOGCRS["]
        .iter()
        .filter_map(|p| data.windows(p.len()).position(|w| w == p.as_bytes()))
        .min()?;
    let rest = &data[start..];
    let end = rest.iter().position(|&c| c == 0).unwrap_or(rest.len());
    std::str::from_utf8(&rest[..end]).ok().map(str::to_string)
}

impl Image {
    /// Band `index` as values, row 0 first, nodata as NaN; whether tiles were missing or
    /// damaged.
    pub fn band_values(&self, data: &[u8], index: usize) -> (Vec<f32>, bool) {
        self.band_values_with(data, None, index)
    }

    /// The spill files the bands name, in band order (a name once).
    pub fn spill_files(&self) -> Vec<String> {
        let mut v: Vec<String> = Vec::new();
        for b in &self.bands {
            if let Some(s) = &b.spill
                && !v.contains(&s.file)
            {
                v.push(s.file.clone());
            }
        }
        v
    }

    /// [`Image::band_values`] with the band's spill file's bytes, when it has one.
    pub fn band_values_with(
        &self,
        data: &[u8],
        spill: Option<&[u8]>,
        index: usize,
    ) -> (Vec<f32>, bool) {
        let Some(b) = self.bands.get(index) else {
            return (Vec::new(), true);
        };
        let (w, hgt) = (b.width as usize, b.height as usize);
        let (bw, bh) = (b.block_width as usize, b.block_height as usize);
        let mut out = vec![f32::NAN; w * hgt];
        let spilled;
        let (data, blocks) = match (&b.spill, spill) {
            (Some(s), Some(d)) => match spill_blocks(b, s, d) {
                Some(k) => {
                    spilled = k;
                    (d, &spilled)
                }
                None => return (out, true),
            },
            (Some(_), None) => return (out, true),
            _ => (data, &b.blocks),
        };
        if b.unreadable.is_some() && blocks.is_empty() {
            return (out, true);
        }
        let per_row = w.div_ceil(bw);
        let bad = std::sync::atomic::AtomicBool::new(false);
        let fill = if b.nodata.is_some() { f32::NAN } else { 0.0 };
        // Nodata compared at the sample's precision (a float32 band's -1e30 is not the f64 -1e30).
        let nodata = b.nodata.map(|v| match b.pixel {
            PixelType::F32 | PixelType::C64 => f64::from(v as f32),
            _ => v,
        });
        out.par_chunks_mut(bh * w)
            .enumerate()
            .for_each(|(by, strip)| {
                let rows = strip.len() / w;
                let mut tile = vec![0f64; bw * bh];
                for bx in 0..per_row {
                    let ok = blocks.get(by * per_row + bx).is_some_and(|k| {
                        if !k.valid {
                            tile.fill(f64::NAN);
                            return true;
                        }
                        !k.external && decode_block(data, k, b.pixel, &mut tile)
                    });
                    if !ok {
                        bad.store(true, std::sync::atomic::Ordering::Relaxed);
                    }
                    let cols = bw.min(w - bx * bw);
                    for y in 0..rows {
                        let dst = &mut strip[y * w + bx * bw..y * w + bx * bw + cols];
                        for (x, v) in dst.iter_mut().enumerate() {
                            *v = if !ok {
                                f32::NAN
                            } else {
                                let s = tile[y * bw + x];
                                if s.is_nan() {
                                    fill
                                } else if nodata == Some(s) {
                                    f32::NAN
                                } else {
                                    s as f32
                                }
                            };
                        }
                    }
                }
            });
        (out, bad.into_inner())
    }
}

/// The tiles of a spilled layer. The spill file starts `ERDAS_IMG_EXTERNAL_RASTER`; at
/// the flags offset each layer has 20 bytes (two words, then rows, columns, a 1-bit type
/// and an object type) and a bitmap of valid tiles, rows padded to bytes.
fn spill_blocks(b: &Band, s: &Spill, d: &[u8]) -> Option<Vec<Block>> {
    if !d.starts_with(b"ERDAS_IMG_EXTERNAL_RASTER") {
        return None;
    }
    let per_row = b.width.div_ceil(b.block_width) as usize;
    let rows = b.height.div_ceil(b.block_height) as usize;
    let row_bytes = per_row.div_ceil(8);
    let size =
        (b.block_width as usize * b.block_height as usize * b.pixel.bits() as usize).div_ceil(8);
    let map_bytes = row_bytes.checked_mul(rows)?;
    let flags = usize::try_from(s.flags_offset)
        .ok()?
        .checked_add((s.stack_index as usize).checked_mul(map_bytes.checked_add(20)?)?)?
        .checked_add(20)?;
    let bitmap = d.get(flags..flags.checked_add(map_bytes)?)?;
    let (count, index) = (u64::from(s.stack_count), u64::from(s.stack_index));
    Some(
        (0..per_row * rows)
            .map(|i| {
                let (r, c) = (i / per_row, i % per_row);
                Block {
                    // A wrapped offset only lands outside the file: that tile reads as damaged.
                    offset: s.data_offset.wrapping_add(
                        (i as u64)
                            .wrapping_mul(count)
                            .wrapping_add(index)
                            .wrapping_mul(size as u64),
                    ),
                    size: size as u32,
                    valid: bitmap[r * row_bytes + c / 8] >> (c % 8) & 1 == 1,
                    compressed: false,
                    external: false,
                }
            })
            .collect(),
    )
}

/// Decodes one tile into `out` (`block_width × block_height` samples); false when damaged.
fn decode_block(data: &[u8], k: &Block, pixel: PixelType, out: &mut [f64]) -> bool {
    let Some(src) = usize::try_from(k.offset)
        .ok()
        .and_then(|o| data.get(o..o.checked_add(k.size as usize)?))
    else {
        return false;
    };
    if k.compressed {
        return decode_rle(src, pixel, out);
    }
    for (i, v) in out.iter_mut().enumerate() {
        match pixel.read(src, i) {
            Some(s) => *v = s,
            None => return false,
        }
    }
    true
}

/// ERDAS run-length tiles: minimum (i32, or f32 bits for float tiles), run count (−1:
/// no runs, every value stored), offset of the values, bits per value; then the run
/// lengths (one to four bytes, the top two bits of the first byte giving the extra
/// bytes) and the values (1, 2 and 4 bits packed from the low bits, 8 bits, 16 and 32
/// bits big-endian), each added to the minimum.
pub fn decode_rle(src: &[u8], pixel: PixelType, out: &mut [f64]) -> bool {
    let (Some(min), Some(runs), Some(voff), Some(&nbits)) =
        (u32_at(src, 0), u32_at(src, 4), u32_at(src, 8), src.get(12))
    else {
        return false;
    };
    let runs = runs as i32;
    let min_i = min as i32;
    // Integer sums wrap to the pixel type (a signed minimum is stored as its unsigned bits
    // when the writer worked in unsigned arithmetic). Float tiles work on the bit patterns:
    // the minimum's bits plus the stored offset are the value's bits.
    let value = |raw: u32| -> f64 {
        let n = min_i.wrapping_add(raw as i32);
        match pixel {
            PixelType::F32 => f64::from(f32::from_bits(n as u32)),
            PixelType::S8 => f64::from(n as i8),
            PixelType::U8 => f64::from(n as u8),
            PixelType::S16 => f64::from(n as i16),
            PixelType::U16 => f64::from(n as u16),
            PixelType::U32 => f64::from(n as u32),
            _ => f64::from(n),
        }
    };
    let mut vals = ValueReader {
        d: src,
        at: if runs == -1 { 13 } else { voff as usize },
        bit: 0,
        bits: u32::from(nbits),
    };
    if runs == -1 {
        for v in out.iter_mut() {
            match vals.next() {
                Some(r) => *v = value(r),
                None => return false,
            }
        }
        return true;
    }
    let mut at = 13usize;
    let mut i = 0usize;
    for _ in 0..runs.max(0) {
        let Some(&b0) = src.get(at) else {
            return false;
        };
        let extra = usize::from(b0 >> 6);
        let Some(bytes) = src.get(at..at + 1 + extra) else {
            return false;
        };
        let mut n = u32::from(b0 & 0x3f);
        for &b in &bytes[1..] {
            n = (n << 8) | u32::from(b);
        }
        at += 1 + extra;
        let Some(r) = vals.next() else {
            return false;
        };
        let v = value(r);
        let end = (i + n as usize).min(out.len());
        out[i..end].fill(v);
        i = end;
    }
    i == out.len()
}

struct ValueReader<'a> {
    d: &'a [u8],
    at: usize,
    bit: u32,
    bits: u32,
}

impl ValueReader<'_> {
    fn next(&mut self) -> Option<u32> {
        let d = self.d;
        Some(match self.bits {
            0 => 0,
            1 | 2 | 4 => {
                let v = (u32::from(*d.get(self.at)?) >> self.bit) & ((1 << self.bits) - 1);
                self.bit += self.bits;
                if self.bit == 8 {
                    self.bit = 0;
                    self.at += 1;
                }
                v
            }
            8 => {
                self.at += 1;
                u32::from(*d.get(self.at - 1)?)
            }
            16 => {
                self.at += 2;
                u32::from(u16::from_be_bytes(
                    d.get(self.at - 2..self.at)?.try_into().ok()?,
                ))
            }
            32 => {
                self.at += 4;
                u32::from_be_bytes(d.get(self.at - 4..self.at)?.try_into().ok()?)
            }
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A type holding 1000 of itself, reading no bytes: 1000^16 items without the rule that
    /// every item uses some bytes.
    #[test]
    fn self_nested_type_bounded() {
        let dict = parse_dictionary(b"{1000:oA,a,}A,.");
        let h = Hfa {
            data: &[],
            dict,
            nodes: Vec::new(),
        };
        let d = vec![0u8; 4000];
        let mut pos = 0;
        let t = &h.dict["A"];
        let r = h.read_record(t, &d, &mut pos, 0).unwrap();
        assert!(r.0.is_empty());
    }

    #[test]
    fn dictionary() {
        let d = parse_dictionary(
            b"{1:lwidth,1:e3:a,b,c,kind,0:pcname,1:*oPt,at,1:x{1:dv,}Inner,inner,}Rec,{1:dx,1:dy,}Pt,.",
        );
        let r = &d["Rec"];
        assert_eq!(r.fields.len(), 5);
        assert_eq!(r.fields[1].enums, ["a", "b", "c"]);
        assert!(r.fields[2].pointer);
        assert_eq!(r.fields[3].object.as_deref(), Some("Pt"));
        assert_eq!(r.fields[4].object.as_deref(), Some("Inner"));
        assert!(d.contains_key("Inner"));
        // Read a record through it.
        let h = Hfa {
            data: &[],
            dict: d,
            nodes: Vec::new(),
        };
        let mut b = Vec::new();
        b.extend(7u32.to_le_bytes());
        b.extend(2u16.to_le_bytes());
        b.extend(3u32.to_le_bytes());
        b.extend(0u32.to_le_bytes());
        b.extend(b"ab\0");
        b.extend(1u32.to_le_bytes());
        b.extend(0u32.to_le_bytes());
        b.extend(1.5f64.to_le_bytes());
        b.extend(2.5f64.to_le_bytes());
        b.extend(9.0f64.to_le_bytes());
        let mut pos = 0;
        let r = h.read_record(&h.dict["Rec"], &b, &mut pos, 0).unwrap();
        assert_eq!(pos, b.len());
        assert_eq!(r.num("width"), Some(7.0));
        assert_eq!(r.text("kind"), Some("c"));
        assert_eq!(r.text("name"), Some("ab"));
        assert_eq!(r.object("at").unwrap().num("y"), Some(2.5));
        assert_eq!(r.object("inner").unwrap().num("v"), Some(9.0));
    }

    #[test]
    fn rle() {
        // Runs 3 × (min + 0), 2 × (min + 300) with 16-bit values.
        let mut b = Vec::new();
        b.extend(100i32.to_le_bytes());
        b.extend(2i32.to_le_bytes());
        b.extend(15u32.to_le_bytes());
        b.push(16);
        b.extend([3, 2]);
        b.extend(0u16.to_be_bytes());
        b.extend(300u16.to_be_bytes());
        let mut out = [0.0; 5];
        assert!(decode_rle(&b, PixelType::U16, &mut out));
        assert_eq!(out, [100.0, 100.0, 100.0, 400.0, 400.0]);
        // No runs: every value stored, 4 bits each.
        let mut b = Vec::new();
        b.extend((-1i32).to_le_bytes());
        b.extend((-1i32).to_le_bytes());
        b.extend(0u32.to_le_bytes());
        b.push(4);
        b.extend([0x21, 0x03]);
        let mut out = [0.0; 3];
        assert!(decode_rle(&b, PixelType::S8, &mut out));
        assert_eq!(out, [0.0, 1.0, 2.0]);
        // A long run with a two-byte length.
        let mut b = Vec::new();
        b.extend(0i32.to_le_bytes());
        b.extend(1i32.to_le_bytes());
        b.extend(15u32.to_le_bytes());
        b.push(8);
        b.extend([0x41, 0x2c]);
        b.push(9);
        let mut out = vec![0.0; 300];
        assert!(decode_rle(&b, PixelType::U8, &mut out));
        assert!(out.iter().all(|&v| v == 9.0));
    }

    #[test]
    fn rejects() {
        assert!(open(b"EHFA_HEADER_TAG\0").is_err());
        assert!(open(b"not imagine").is_err());
    }

    /// A file with one `Eimg_Layer` of the given size and tile size, and no tile directory.
    fn one_layer(w: u32, h: u32, bw: u32, bh: u32) -> Vec<u8> {
        let dict = b"{1:lwidth,1:lheight,1:e3:u1,u2,u4,pixelType,1:lblockWidth,1:lblockHeight,}Eimg_Layer,{1:lx,}Eimg_Root,.\0";
        let mut d = vec![0u8; 512];
        d[..15].copy_from_slice(MAGIC);
        let put =
            |d: &mut Vec<u8>, at: usize, v: u32| d[at..at + 4].copy_from_slice(&v.to_le_bytes());
        // File record at 32: root entry at 64, dictionary at 400.
        put(&mut d, 16, 32);
        put(&mut d, 40, 64);
        d[46..50].copy_from_slice(&400u32.to_le_bytes());
        let node =
            |d: &mut Vec<u8>, at: usize, child: u32, data: u32, size: u32, name: &str, ty: &str| {
                put(d, at + 12, child);
                put(d, at + 16, data);
                put(d, at + 20, size);
                d[at + 24..at + 24 + name.len()].copy_from_slice(name.as_bytes());
                d[at + 88..at + 88 + ty.len()].copy_from_slice(ty.as_bytes());
            };
        node(&mut d, 64, 184, 0, 0, "root", "Eimg_Root");
        node(&mut d, 184, 0, 304, 22, "Layer_1", "Eimg_Layer");
        for (i, v) in [w, h].into_iter().enumerate() {
            put(&mut d, 304 + 4 * i, v);
        }
        d[312] = 0; // pixel type u1
        put(&mut d, 314, bw);
        put(&mut d, 318, bh);
        d.truncate(400);
        d.extend_from_slice(dict);
        d
    }

    #[test]
    fn hostile_tile_size_is_refused() {
        // A 2^24 × 2^24 tile would be a 2 PB f64 buffer per decoding thread.
        let f = one_layer(4, 4, 1 << 24, 1 << 24);
        assert!(open(&f).unwrap_err().contains("tiles too large"));
        // Single-pixel tiles of a 2^24² band: 2^48 tile entries.
        let f = one_layer(1 << 24, 1 << 24, 1, 1);
        assert_eq!(open(&f).unwrap_err(), "too many tiles");
        // An ordinary layout still opens (unreadable: no tile directory).
        let img = open(&one_layer(4, 4, 64, 64)).unwrap();
        assert_eq!(img.bands[0].block_width, 64);
    }

    #[test]
    fn hostile_base_data_size() {
        // A base-data field claiming 2^32 × 2^32 samples: the product overflows.
        let h = Hfa {
            data: &[],
            dict: parse_dictionary(b"{1:bdata,}Rec,."),
            nodes: Vec::new(),
        };
        let mut b = Vec::new();
        b.extend(u32::MAX.to_le_bytes());
        b.extend(u32::MAX.to_le_bytes());
        b.extend(u32::MAX.to_le_bytes());
        b.extend(10u16.to_le_bytes());
        let mut pos = 0;
        let r = h.read_record(&h.dict["Rec"], &b, &mut pos, 0).unwrap();
        assert!(r.get("data").is_none());
    }

    #[test]
    fn hostile_spill_offsets() {
        let band = Band {
            name: String::new(),
            width: 64,
            height: 64,
            pixel: PixelType::U8,
            block_width: 64,
            block_height: 64,
            blocks: Vec::new(),
            nodata: None,
            layer_type: None,
            statistics: Vec::new(),
            unreadable: None,
            spill: None,
        };
        let mut d = b"ERDAS_IMG_EXTERNAL_RASTER".to_vec();
        d.resize(64, 1);
        let s = Spill {
            file: String::new(),
            flags_offset: u64::MAX,
            data_offset: u64::MAX,
            stack_count: u32::MAX,
            stack_index: u32::MAX,
        };
        assert!(spill_blocks(&band, &s, &d).is_none());
        let s = Spill {
            flags_offset: 0,
            stack_index: 0,
            ..s
        };
        let k = spill_blocks(&band, &s, &d).unwrap();
        assert_eq!(k.len(), 1);
        let mut tile = vec![0.0; 64 * 64];
        assert!(!decode_block(&d, &k[0], PixelType::U8, &mut tile));
    }
}
