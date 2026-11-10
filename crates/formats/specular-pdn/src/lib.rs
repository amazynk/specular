//! Paint.NET `.pdn` reader for Specular.
//!
//! A `.pdn` file is `PDN3`, a 24-bit little-endian length and an XML header (canvas size,
//! layer count, the version that saved it and a PNG thumbnail in base64), then the document
//! as a .NET `BinaryFormatter` stream, either as is (`00 01`) or gzip-compressed (`1F 8B`).
//! Paint.NET 2.x files are the bare stream, gzip-compressed, without the header. The layer
//! pixels follow the stream: each `MemoryBlock` defers its bytes there, in the order the
//! blocks were serialised, as a format byte (0 = gzip chunks, 1 = stored), a big-endian chunk
//! size, and the chunks (number, size, data), in any order.
//!
//! The stream is read as data by [`nrbf`]: the record grammar into an object table. Nothing
//! is instantiated; class names only select what to read (`PaintDotNet.Document`,
//! `LayerList`, `BitmapLayer`, `Surface`, `MemoryBlock`). Layers are BGRA with straight alpha,
//! bottom layer first, each with a name, visibility, opacity and blend mode (the 14 Paint.NET
//! modes; the `blendMode` enum of 4.x+ wins over the 3.x `blendOp` class).
//!
//! [`Pdn::render`] composites the visible layers as Paint.NET flattens them: 8-bit integer
//! arithmetic per channel, the layer's opacity scaling its alpha, and the blend function mixed
//! in by the overlap of the two alphas. It runs per chunk of the layer data in parallel, so
//! memory stays near one canvas plus a chunk per thread.
//!
//! Robustness: no input panics; the canvas is capped by [`MAX_PIXELS`] and reserved from the
//! memory budget; missing or damaged chunks leave their pixels transparent and set
//! [`Image::truncated`].

mod nrbf;

use nrbf::{Obj, Stream, Val};
use rayon::prelude::*;
use std::ops::Range;

/// Largest canvas pixel count accepted (Paint.NET's own limit is 65535 × 65535; 512 MP here).
pub const MAX_PIXELS: u64 = 1 << 29;
/// Largest decompressed document stream accepted (the object graph, not the pixels).
const MAX_STREAM: usize = 256 << 20;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    NotPdn,
    Unsupported(String),
    Truncated(String),
    Malformed(String),
    TooLarge,
    NeedsMemory(specular_core::NeedsMemory),
    Cancelled,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NotPdn => write!(f, "not a Paint.NET document"),
            Error::Unsupported(s) => write!(f, "unsupported: {s}"),
            Error::Truncated(s) => write!(f, "truncated: {s}"),
            Error::Malformed(s) => write!(f, "malformed: {s}"),
            Error::TooLarge => write!(f, "image too large"),
            Error::NeedsMemory(n) => write!(f, "{n}"),
            Error::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl std::error::Error for Error {}

impl From<specular_core::NeedsMemory> for Error {
    fn from(n: specular_core::NeedsMemory) -> Self {
        Error::NeedsMemory(n)
    }
}

const MAGIC: &[u8] = b"PDN3";

/// Whether `head` starts like a Paint.NET 3.x–5.x document.
pub fn probe(head: &[u8]) -> bool {
    head.starts_with(MAGIC) && head.get(7) == Some(&b'<')
}

/// The XML header's facts, read without touching the document stream.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Header {
    pub width: u32,
    pub height: u32,
    pub layers: u32,
    /// "4.21.6589.7045", as stored (`None` for Paint.NET 2.x files, which have no header).
    pub saved_with: Option<String>,
    /// The thumbnail Paint.NET stores for file browsers (a PNG, at most 256 pixels a side).
    pub thumbnail_png: Option<Vec<u8>>,
    /// Where the document stream starts.
    body: usize,
}

fn attr<'a>(xml: &'a str, name: &str) -> Option<&'a str> {
    let key = format!(" {name}=\"");
    let at = xml.find(&key)? + key.len();
    let end = xml[at..].find('"')?;
    Some(&xml[at..at + end])
}

fn base64(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() / 4 * 3);
    let (mut acc, mut bits) = (0u32, 0u32);
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' | b'\r' | b'\n' | b' ' => continue,
            _ => return None,
        };
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

/// Reads the XML header (cheap: no decompression).
pub fn read_header(data: &[u8]) -> Result<Header, Error> {
    if !data.starts_with(MAGIC) {
        if data.starts_with(&[0x1f, 0x8b]) {
            // Paint.NET 2.x: the gzip stream alone; sizes come from the document.
            return Ok(Header::default());
        }
        return Err(Error::NotPdn);
    }
    let len = data
        .get(4..7)
        .map(|b| usize::from(b[0]) | usize::from(b[1]) << 8 | usize::from(b[2]) << 16)
        .ok_or_else(|| Error::Truncated("the header ends early".into()))?;
    let xml = data
        .get(7..7 + len)
        .ok_or_else(|| Error::Truncated("the header ends early".into()))?;
    let xml = std::str::from_utf8(xml).map_err(|_| Error::Malformed("header text".into()))?;
    if !xml.contains("<pdnImage") {
        return Err(Error::NotPdn);
    }
    let num = |n| attr(xml, n).and_then(|v| v.trim().parse::<u32>().ok());
    Ok(Header {
        width: num("width").unwrap_or(0),
        height: num("height").unwrap_or(0),
        layers: num("layers").unwrap_or(0),
        saved_with: attr(xml, "savedWithVersion").map(str::to_string),
        thumbnail_png: xml
            .find("<thumb ")
            .and_then(|i| attr(&xml[i..], "png"))
            .and_then(base64)
            .filter(|p| p.starts_with(b"\x89PNG")),
        body: 7 + len,
    })
}

/// A Paint.NET layer blend mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BlendMode {
    Normal,
    Multiply,
    Additive,
    ColorBurn,
    ColorDodge,
    Reflect,
    Glow,
    Overlay,
    Difference,
    Negation,
    Lighten,
    Darken,
    Screen,
    Xor,
}

impl BlendMode {
    /// The `LayerBlendMode` enum value of Paint.NET 4.x and later.
    fn from_value(v: i64) -> Option<BlendMode> {
        use BlendMode::*;
        Some(match v {
            0 => Normal,
            1 => Multiply,
            2 => Additive,
            3 => ColorBurn,
            4 => ColorDodge,
            5 => Reflect,
            6 => Glow,
            7 => Overlay,
            8 => Difference,
            9 => Negation,
            10 => Lighten,
            11 => Darken,
            12 => Screen,
            13 => Xor,
            _ => return None,
        })
    }

    /// The 3.x `UserBlendOps+<Name>BlendOp` class.
    fn from_class(c: &str) -> Option<BlendMode> {
        use BlendMode::*;
        let n = c.rsplit('+').next()?.strip_suffix("BlendOp")?;
        Some(match n {
            "Normal" => Normal,
            "Multiply" => Multiply,
            "Additive" => Additive,
            "ColorBurn" => ColorBurn,
            "ColorDodge" => ColorDodge,
            "Reflect" => Reflect,
            "Glow" => Glow,
            "Overlay" => Overlay,
            "Difference" => Difference,
            "Negation" => Negation,
            "Lighten" => Lighten,
            "Darken" => Darken,
            "Screen" => Screen,
            "Xor" => Xor,
            _ => return None,
        })
    }

    /// The name Paint.NET's Layer Properties dialog shows.
    pub fn name(self) -> &'static str {
        use BlendMode::*;
        match self {
            Normal => "Normal",
            Multiply => "Multiply",
            Additive => "Additive",
            ColorBurn => "Color Burn",
            ColorDodge => "Color Dodge",
            Reflect => "Reflect",
            Glow => "Glow",
            Overlay => "Overlay",
            Difference => "Difference",
            Negation => "Negation",
            Lighten => "Lighten",
            Darken => "Darken",
            Screen => "Screen",
            Xor => "Xor",
        }
    }
}

/// A layer's properties, bottom layer first in [`Pdn::layers`].
#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    pub name: String,
    pub visible: bool,
    pub opacity: u8,
    pub blend: BlendMode,
    pub background: bool,
    /// A blend mode this reader does not know (drawn as Normal), by its stored name.
    pub unknown_blend: Option<String>,
    surface: Surface,
}

#[derive(Clone, Debug, PartialEq)]
struct Surface {
    width: u32,
    height: u32,
    stride: usize,
    /// Index into [`Pdn::blocks`].
    block: Option<usize>,
}

/// A memory block's deferred data: where its chunks are.
#[derive(Clone, Debug, PartialEq)]
struct Block {
    length: u64,
    compressed: bool,
    chunk_size: usize,
    /// Per chunk number, its bytes in the stream (None: missing).
    chunks: Vec<Option<Range<usize>>>,
    /// Inline bytes (a block saved inside the object graph), in the stream.
    inline: Option<Range<usize>>,
}

/// A parsed document: layer properties and where their pixels are. `Send + Sync`.
pub struct Pdn {
    header: Header,
    width: u32,
    height: u32,
    layers: Vec<Layer>,
    blocks: Vec<Block>,
    /// The decompressed stream when the document was gzip-compressed as a whole.
    unpacked: Option<Vec<u8>>,
    body: usize,
    truncated: bool,
}

/// Interleaved straight-alpha RGBA8.
#[derive(Clone, Debug)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
    pub truncated: bool,
}

#[derive(Clone, Copy, Default)]
pub struct Options<'a> {
    pub parallel: bool,
    pub cancel: Option<&'a (dyn Fn() -> bool + Sync)>,
}

impl Options<'_> {
    fn cancelled(&self) -> bool {
        self.cancel.is_some_and(|c| c())
    }
}

fn bad(what: &str) -> Error {
    Error::Malformed(format!("the document's {what} is damaged"))
}

/// Reads the document: the header, the object graph and the index of the layer chunks
/// (nothing is decompressed but the object graph).
pub fn parse(data: &[u8]) -> Result<Pdn, Error> {
    let header = read_header(data)?;
    let body = data.get(header.body..).unwrap_or_default();
    let (unpacked, start) = if body.starts_with(&[0x1f, 0x8b]) {
        let s = specular_png::inflate_gzip(body, MAX_STREAM)
            .map_err(|e| Error::Malformed(format!("the document's compression ({e:?})")))?;
        (Some(s), 0)
    } else if body.starts_with(&[0x00, 0x01]) {
        (None, header.body + 2)
    } else if body.is_empty() {
        return Err(Error::Truncated("the document has no content".into()));
    } else {
        return Err(Error::Unsupported(
            "a Paint.NET document stored in an unknown way".into(),
        ));
    };
    let stream = unpacked
        .as_deref()
        .unwrap_or(&data[start.min(data.len())..]);
    let s = nrbf::parse(stream)?;
    let mut pdn = document(&s, stream, header)?;
    pdn.body = start;
    pdn.unpacked = unpacked;
    Ok(pdn)
}

fn class<'s>(s: &'s Stream, v: Option<Val>, suffix: &str) -> Option<&'s Obj> {
    let o = s.get(v?)?;
    o.class_name()
        .is_some_and(|n| n.ends_with(suffix))
        .then_some(o)
}

fn document(s: &Stream, stream: &[u8], header: Header) -> Result<Pdn, Error> {
    let root = s
        .objects
        .get(&s.root)
        .filter(|o| {
            o.class_name()
                .is_some_and(|n| n.ends_with("PaintDotNet.Document"))
        })
        .ok_or(Error::NotPdn)?;
    let dim = |o: &Obj, n| nrbf::int(o.member(n)).and_then(|v| u32::try_from(v).ok());
    let width = dim(root, "width").ok_or_else(|| bad("canvas size"))?;
    let height = dim(root, "height").ok_or_else(|| bad("canvas size"))?;
    if width == 0 || height == 0 {
        return Err(bad("canvas size"));
    }
    if u64::from(width) * u64::from(height) > MAX_PIXELS {
        return Err(Error::TooLarge);
    }
    let list = class(s, root.member("layers"), "LayerList").ok_or_else(|| bad("layer list"))?;
    let size = nrbf::int(list.member("_size")).unwrap_or(0).max(0) as usize;
    let items = match s.get(list.member("_items").unwrap_or(Val::Null)) {
        Some(Obj::Array(v)) => v.as_slice(),
        None => &[],
        _ => return Err(bad("layer list")),
    };
    // MemoryBlocks in stream order: their deferred data follows the stream in this order.
    let block_ids: Vec<i32> = s
        .order
        .iter()
        .copied()
        .filter(|id| {
            s.objects[id]
                .class_name()
                .is_some_and(|n| n.ends_with("PaintDotNet.MemoryBlock"))
        })
        .collect();
    let mut blocks = Vec::with_capacity(block_ids.len());
    let mut pos = s.end;
    let mut truncated = false;
    for id in &block_ids {
        let o = &s.objects[id];
        let length = nrbf::int(o.member("length64"))
            .or_else(|| nrbf::int(o.member("length")))
            .and_then(|l| u64::try_from(l).ok())
            .ok_or_else(|| bad("layer data"))?;
        if nrbf::boolean(o.member("hasParent")) == Some(true) {
            return Err(Error::Unsupported(
                "a Paint.NET layer that shares memory with another".into(),
            ));
        }
        let inline = match o.member("pointerData").and_then(|v| s.get(v)) {
            Some(Obj::Bytes(r)) => Some(r.clone()),
            _ => None,
        };
        let deferred = nrbf::boolean(o.member("deferred")).unwrap_or(inline.is_none());
        if !deferred || inline.is_some() {
            blocks.push(Block {
                length,
                compressed: false,
                chunk_size: 0,
                chunks: Vec::new(),
                inline,
            });
            continue;
        }
        let (block, next) = chunk_index(stream, pos, length);
        truncated |= block.chunks.iter().any(Option::is_none);
        blocks.push(block);
        pos = next;
    }
    let mut layers = Vec::new();
    for v in items.iter().take(size) {
        let Some(l) = s.get(*v) else {
            continue;
        };
        if !l.class_name().is_some_and(|n| n.ends_with("BitmapLayer")) {
            return Err(Error::Unsupported(format!(
                "a Paint.NET layer of type {}",
                l.class_name().unwrap_or("unknown")
            )));
        }
        layers.push(layer(s, l, &block_ids)?);
    }
    Ok(Pdn {
        header,
        width,
        height,
        layers,
        blocks,
        unpacked: None,
        body: 0,
        truncated,
    })
}

fn layer(s: &Stream, l: &Obj, block_ids: &[i32]) -> Result<Layer, Error> {
    let props = class(s, l.member("Layer+properties"), "LayerProperties")
        .ok_or_else(|| bad("layer properties"))?;
    let name = props
        .member("name")
        .and_then(|v| s.string(v))
        .unwrap_or_default()
        .to_string();
    let visible = nrbf::boolean(props.member("visible")).unwrap_or(true);
    let background = nrbf::boolean(props.member("isBackground")).unwrap_or(false);
    let opacity = nrbf::int(props.member("opacity"))
        .unwrap_or(255)
        .clamp(0, 255) as u8;
    let enum_mode = class(s, props.member("blendMode"), "LayerBlendMode")
        .and_then(|o| nrbf::int(o.member("value__")));
    let op_class = class(s, l.member("properties"), "BitmapLayerProperties")
        .and_then(|p| s.get(p.member("blendOp")?))
        .and_then(Obj::class_name);
    let (blend, unknown_blend) = match (enum_mode, op_class) {
        (Some(v), _) => match BlendMode::from_value(v) {
            Some(b) => (b, None),
            None => (BlendMode::Normal, Some(format!("blend mode {v}"))),
        },
        (None, Some(c)) => match BlendMode::from_class(c) {
            Some(b) => (b, None),
            None => (
                BlendMode::Normal,
                Some(c.rsplit('+').next().unwrap_or(c).to_string()),
            ),
        },
        (None, None) => (BlendMode::Normal, None),
    };
    let surf =
        class(s, l.member("surface"), "PaintDotNet.Surface").ok_or_else(|| bad("layer surface"))?;
    let dim = |n| nrbf::int(surf.member(n)).and_then(|v| u32::try_from(v).ok());
    let (Some(width), Some(height), Some(stride)) = (dim("width"), dim("height"), dim("stride"))
    else {
        return Err(bad("layer surface"));
    };
    let block = match surf.member("scan0") {
        Some(Val::Ref(id)) => block_ids.iter().position(|b| *b == id),
        _ => None,
    };
    Ok(Layer {
        name,
        visible,
        opacity,
        blend,
        background,
        unknown_blend,
        surface: Surface {
            width,
            height,
            stride: stride as usize,
            block,
        },
    })
}

fn be32(d: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(
        d.get(at..at.checked_add(4)?)?.try_into().ok()?,
    ))
}

/// The chunks of a deferred block of `length` bytes starting at `pos`, and where the next
/// block starts. Stops at the first chunk header that does not fit; the rest stay missing.
fn chunk_index(d: &[u8], pos: usize, length: u64) -> (Block, usize) {
    let mut b = Block {
        length,
        compressed: true,
        chunk_size: 0,
        chunks: Vec::new(),
        inline: None,
    };
    let (Some(&format), Some(size)) = (d.get(pos), be32(d, pos + 1)) else {
        return (b, d.len());
    };
    b.compressed = format == 0;
    if size == 0 || format > 1 {
        return (b, d.len());
    }
    b.chunk_size = size as usize;
    let count = length.div_ceil(u64::from(size));
    // A chunk header takes 8 bytes: more chunks than bytes left is damage.
    let count = count.min(((d.len() - pos) / 8) as u64 + 1) as usize;
    b.chunks = vec![None; count];
    let mut p = pos + 5;
    for _ in 0..count {
        let (Some(n), Some(len)) = (be32(d, p), be32(d, p + 4)) else {
            return (b, d.len());
        };
        let start = p + 8;
        let Some(end) = start.checked_add(len as usize).filter(|&e| e <= d.len()) else {
            return (b, d.len());
        };
        if let Some(slot) = b.chunks.get_mut(n as usize)
            && slot.is_none()
        {
            *slot = Some(start..end);
        }
        p = end;
    }
    (b, p)
}

/// Paint.NET's 8-bit multiply with rounding: `a * b / 255`.
#[inline]
fn mul(a: u32, b: u32) -> u32 {
    let r = a * b + 0x80;
    ((r >> 8) + r) >> 8
}

#[inline]
fn blend_channel(mode: BlendMode, a: u32, b: u32) -> u32 {
    use BlendMode::*;
    // `a`: the layer below, `b`: this layer.
    match mode {
        Normal => b,
        Multiply => mul(a, b),
        Additive => (a + b).min(255),
        ColorBurn => ((255 - a) * 255)
            .checked_div(b)
            .map_or(0, |q| 255u32.saturating_sub(q)),
        ColorDodge => {
            if b == 255 {
                255
            } else {
                (a * 255 / (255 - b)).min(255)
            }
        }
        Reflect => {
            if b == 255 {
                255
            } else {
                (a * a / (255 - b)).min(255)
            }
        }
        Glow => {
            if a == 255 {
                255
            } else {
                (b * b / (255 - a)).min(255)
            }
        }
        Overlay => {
            if a < 128 {
                mul(2 * a, b)
            } else {
                255 - mul(2 * (255 - a), 255 - b)
            }
        }
        Difference => a.abs_diff(b),
        Negation => 255 - (255i32 - a as i32 - b as i32).unsigned_abs(),
        Lighten => a.max(b),
        Darken => a.min(b),
        Screen => b + a - mul(b, a),
        Xor => a ^ b,
    }
}

/// Composites one BGRA source pixel onto an RGBA destination pixel.
#[inline]
fn blend_pixel(mode: BlendMode, opacity: u32, dst: &mut [u8], src: &[u8]) {
    let ra = mul(u32::from(src[3]), opacity);
    if ra == 0 {
        return;
    }
    let la = u32::from(dst[3]);
    let src_rgb = [u32::from(src[2]), u32::from(src[1]), u32::from(src[0])];
    if la == 0 {
        dst[..3].copy_from_slice(&[src[2], src[1], src[0]]);
        dst[3] = ra as u8;
        return;
    }
    let y = mul(la, 255 - ra);
    let total = y + ra;
    let x = mul(la, ra);
    let z = ra - x;
    for c in 0..3 {
        let l = u32::from(dst[c]);
        let r = src_rgb[c];
        let f = blend_channel(mode, l, r);
        dst[c] = ((l * y + r * z + f * x) / total).min(255) as u8;
    }
    dst[3] = total.min(255) as u8;
}

impl Pdn {
    pub fn header(&self) -> &Header {
        &self.header
    }
    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn height(&self) -> u32 {
        self.height
    }
    /// The layers, bottom first (as Paint.NET's layer list stores them).
    pub fn layers(&self) -> &[Layer] {
        &self.layers
    }
    /// Whether layer data is missing from the file (cut short).
    pub fn truncated(&self) -> bool {
        self.truncated
    }

    fn stream<'d>(&'d self, data: &'d [u8]) -> &'d [u8] {
        self.unpacked
            .as_deref()
            .unwrap_or_else(|| data.get(self.body..).unwrap_or_default())
    }

    fn canvas(&self) -> Result<Vec<u8>, Error> {
        let bytes = u64::from(self.width) * u64::from(self.height) * 4;
        let _r = specular_core::budget::reserve(bytes)?;
        Ok(vec![0u8; bytes as usize])
    }

    /// Composites the visible layers into straight-alpha RGBA.
    pub fn render(&self, data: &[u8], opts: &Options) -> Result<Image, Error> {
        let mut out = self.canvas()?;
        let mut truncated = false;
        for l in self.layers.iter().filter(|l| l.visible && l.opacity > 0) {
            truncated |= self.draw(data, l, l.blend, u32::from(l.opacity), &mut out, opts)?;
        }
        Ok(Image {
            width: self.width,
            height: self.height,
            data: out,
            truncated,
        })
    }

    /// Composites `layers` (indices into [`Pdn::layers`], bottom first) whatever their
    /// visibility, each with its mode and opacity.
    pub fn render_layers(
        &self,
        data: &[u8],
        layers: &[usize],
        opts: &Options,
    ) -> Result<Image, Error> {
        let mut out = self.canvas()?;
        let mut truncated = false;
        for &i in layers {
            let l = self
                .layers
                .get(i)
                .ok_or_else(|| Error::Malformed("no such layer".into()))?;
            truncated |= self.draw(data, l, l.blend, u32::from(l.opacity), &mut out, opts)?;
        }
        Ok(Image {
            width: self.width,
            height: self.height,
            data: out,
            truncated,
        })
    }

    /// Layer `i` alone on the canvas, with its opacity (its blend mode needs a layer below).
    /// Fully transparent pixels come out as transparent black.
    pub fn render_layer(&self, data: &[u8], i: usize, opts: &Options) -> Result<Image, Error> {
        let l = self
            .layers
            .get(i)
            .ok_or_else(|| Error::Malformed("no such layer".into()))?;
        let mut out = self.canvas()?;
        let truncated = self.draw(
            data,
            l,
            BlendMode::Normal,
            u32::from(l.opacity),
            &mut out,
            opts,
        )?;
        Ok(Image {
            width: self.width,
            height: self.height,
            data: out,
            truncated,
        })
    }

    /// Blends layer `l` onto `out`; returns whether some of its data was missing.
    fn draw(
        &self,
        data: &[u8],
        l: &Layer,
        mode: BlendMode,
        opacity: u32,
        out: &mut [u8],
        opts: &Options,
    ) -> Result<bool, Error> {
        let stream = self.stream(data);
        let Some(block) = l.surface.block.and_then(|b| self.blocks.get(b)) else {
            return Ok(true);
        };
        let s = &l.surface;
        let row = self.width as usize * 4;
        let src_row = (s.width.min(self.width) as usize) * 4;
        let rows = s.height.min(self.height) as usize;
        if s.stride < src_row {
            return Err(bad("layer surface"));
        }
        // Blends bytes `off..off + bytes.len()` of the layer's data into `out`.
        let apply = |out: &mut [u8], base: usize, off: usize, bytes: &[u8]| {
            // `out` holds canvas rows starting at byte `base`.
            let mut o = off;
            let end = off + bytes.len();
            while o < end {
                let (y, x) = (o / s.stride, o % s.stride);
                let run = (s.stride - x).min(end - o);
                if y >= rows {
                    break;
                }
                if x < src_row {
                    let n = run.min(src_row - x) / 4 * 4;
                    let d0 = y * row + x;
                    if d0 >= base && d0 + n <= base + out.len() {
                        let dst = &mut out[d0 - base..d0 - base + n];
                        let src = &bytes[o - off..o - off + n];
                        for (d, sp) in dst.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
                            blend_pixel(mode, opacity, d, sp);
                        }
                    }
                }
                o += run;
            }
        };
        if let Some(r) = &block.inline {
            let bytes = stream.get(r.clone()).ok_or_else(|| bad("layer data"))?;
            apply(out, 0, 0, bytes);
            return Ok((bytes.len() as u64) < block.length);
        }
        let cs = block.chunk_size;
        if cs == 0 {
            return Ok(true);
        }
        let decode = |n: usize| -> Option<Vec<u8>> {
            let r = block.chunks.get(n)?.clone()?;
            let want = (block.length - (n as u64 * cs as u64)).min(cs as u64) as usize;
            let src = stream.get(r)?;
            if block.compressed {
                specular_png::inflate_gzip(src, want).ok()
            } else {
                Some(src[..src.len().min(want)].to_vec())
            }
        };
        let missing = std::sync::atomic::AtomicBool::new(false);
        let cancelled = std::sync::atomic::AtomicBool::new(false);
        let one = |n: usize, part: &mut [u8], base: usize| {
            if opts.cancelled() {
                cancelled.store(true, std::sync::atomic::Ordering::Relaxed);
                return;
            }
            match decode(n) {
                Some(bytes) => {
                    let want = (block.length - (n as u64 * cs as u64)).min(cs as u64) as usize;
                    if bytes.len() < want {
                        missing.store(true, std::sync::atomic::Ordering::Relaxed);
                    }
                    apply(part, base, n * cs, &bytes);
                }
                None => missing.store(true, std::sync::atomic::Ordering::Relaxed),
            }
        };
        let chunks = block.length.div_ceil(cs as u64) as usize;
        if s.stride == row && cs % 4 == 0 && opts.parallel {
            // Chunk `n` covers canvas bytes n*cs.. exactly: blend chunks in parallel.
            out.par_chunks_mut(cs)
                .enumerate()
                .take(chunks)
                .for_each(|(n, part)| one(n, part, n * cs));
        } else {
            for n in 0..chunks {
                one(n, out, 0);
            }
        }
        if cancelled.into_inner() {
            return Err(Error::Cancelled);
        }
        Ok(missing.into_inner())
    }
}

#[cfg(test)]
mod tests;
