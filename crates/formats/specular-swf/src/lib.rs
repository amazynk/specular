//! Flash movies (`.swf`), from Adobe's SWF File Format Specification (version 19): the
//! first frame drawn as a vector page, and the movie's facts. No ActionScript is ever run.
//!
//! Read: the header (uncompressed `FWS`, zlib `CWS`, LZMA `ZWS`), the frame size and rate,
//! and the tags up to the first ShowFrame: the background colour, shapes (DefineShape 1–4:
//! solid, linear, radial and focal gradient and bitmap fills, line styles), morph shapes
//! (at their start), bitmaps (DefineBits with JPEGTables, DefineBitsJPEG2/3/4 with alpha,
//! DefineBitsLossless 1/2), fonts (DefineFont 1–3) with static text (DefineText 1/2),
//! edit text drawn in a system font, sprites (their first frame), and the display list
//! (PlaceObject 1–3 with matrices, colour transforms and clip layers).
//!
//! Counted but not drawn: buttons, video streams (named by codec), sound, filters, blend
//! modes and ActionScript.

mod bits;
mod shape;

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use bits::{Bytes, Cxform};
use shape::{Fill, Part, Version};
use specular_drawpage::{
    Affine, BezPath, HAlign, Image, ImageDecoder, Op, Page, Paint, Paragraph, Point, Rect, Run,
    Shape, TextBlock, VAlign,
};

/// Why a movie was not read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Malformed(String),
    Unsupported(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Malformed(s) => write!(f, "damaged Flash movie: {s}"),
            Error::Unsupported(s) => f.write_str(s),
        }
    }
}

impl std::error::Error for Error {}

fn bad(s: &str) -> Error {
    Error::Malformed(s.into())
}

/// Largest movie body unpacked.
const MAX_BODY: usize = 512 << 20;
/// Deepest sprite nesting drawn.
const MAX_DEPTH: usize = 16;

/// Whether `d` starts a Flash movie.
pub fn is_swf(d: &[u8]) -> bool {
    d.len() >= 8 && matches!(&d[..3], b"FWS" | b"CWS" | b"ZWS") && d[3] > 0 && d[3] < 60
}

/// What a movie is and holds.
#[derive(Clone, Debug, Default)]
pub struct Facts {
    pub version: u8,
    pub compression: &'static str,
    pub frame_rate: f64,
    pub frame_count: u16,
    pub shapes: usize,
    pub bitmaps: usize,
    pub sprites: usize,
    pub fonts: usize,
    pub texts: usize,
    pub buttons: usize,
    /// Video streams by codec name.
    pub videos: Vec<String>,
    /// Sound by format name (streams and event sounds).
    pub sounds: Vec<String>,
    pub actionscript: bool,
    /// Pictures that did not decode.
    pub missing_pictures: usize,
}

/// A read movie: the first frame and the facts.
pub struct Movie {
    pub page: Page,
    pub facts: Facts,
}

/// A character of the dictionary.
enum Character {
    Shape(Vec<Part>),
    Bitmap(Arc<Image>),
    Sprite(Vec<Tag>),
    Font(Font),
    Text(Text),
    EditText(EditText),
}

struct Font {
    glyphs: Vec<Vec<Part>>,
    /// Glyph units an em (1024; DefineFont3: 20480).
    em: f64,
}

struct TextRecord {
    font: u16,
    colour: [u8; 4],
    x: f64,
    y: f64,
    height: f64,
    glyphs: Vec<(usize, f64)>,
}

struct Text {
    matrix: Affine,
    records: Vec<TextRecord>,
}

struct EditText {
    bounds: Rect,
    height: f64,
    colour: [u8; 4],
    align: HAlign,
    text: String,
    border: bool,
}

/// A display-list tag of a timeline (the movie's or a sprite's).
#[derive(Clone)]
enum Tag {
    Place {
        depth: u16,
        id: Option<u16>,
        matrix: Option<Affine>,
        cx: Option<Cxform>,
        clip: Option<u16>,
        move_only: bool,
    },
    Remove(u16),
    ShowFrame,
}

#[derive(Clone)]
struct Placed {
    id: u16,
    matrix: Affine,
    cx: Cxform,
    clip: Option<u16>,
}

/// The movie body after the 8-byte header.
fn body(d: &[u8]) -> Result<(Vec<u8>, &'static str), Error> {
    let size = u32::from_le_bytes([d[4], d[5], d[6], d[7]]) as usize;
    let want = size.saturating_sub(8).min(MAX_BODY);
    match &d[..3] {
        b"FWS" => Ok((d[8..].to_vec(), "none")),
        b"CWS" => {
            // A cut-off zlib stream still gives what it holds.
            let v = specular_png::inflate_zlib(&d[8..], want).map_err(|_| bad("zlib body"))?;
            Ok((v, "zlib"))
        }
        _ => {
            // ZWS: compressed length, 5 LZMA properties bytes, then the raw stream.
            let p = d.get(12..17).ok_or_else(|| bad("LZMA header"))?;
            let dict = u32::from_le_bytes([p[1], p[2], p[3], p[4]]).clamp(4096, 1 << 28);
            let mut rd =
                lzma_rust2::LzmaReader::new_with_props(&d[17..], want as u64, p[0], dict, None)
                    .map_err(|_| bad("LZMA properties"))?;
            let mut out = Vec::with_capacity(want.min(64 << 20));
            use std::io::Read;
            let _ = rd.by_ref().take(want as u64).read_to_end(&mut out);
            if out.is_empty() {
                return Err(bad("LZMA body"));
            }
            Ok((out, "LZMA"))
        }
    }
}

struct Reader<'a> {
    images: Option<&'a ImageDecoder>,
    dict: HashMap<u16, Character>,
    jpeg_tables: Vec<u8>,
    facts: Facts,
    background: Option<[u8; 4]>,
}

fn video_codec(c: u8) -> &'static str {
    match c {
        2 => "Sorenson H.263",
        3 => "Screen Video",
        4 => "On2 VP6",
        5 => "On2 VP6 with alpha",
        6 => "Screen Video 2",
        7 => "H.264",
        _ => "unknown codec",
    }
}

fn sound_format(f: u8) -> &'static str {
    match f {
        0 | 3 => "PCM",
        1 => "ADPCM",
        2 => "MP3",
        4..=6 => "Nellymoser",
        11 => "Speex",
        _ => "unknown sound",
    }
}

impl Reader<'_> {
    /// A JPEG, PNG or GIF bitmap, with DefineBitsJPEG3's alpha plane (zlib, a byte a
    /// pixel) when given.
    fn decode_image(&mut self, bytes: &[u8], alpha: Option<&[u8]>) -> Option<Image> {
        let dec = self.images?;
        let (w, h, mut px) = dec(bytes)?;
        if let Some(a) = alpha.filter(|a| !a.is_empty())
            && let Ok(a) = specular_png::inflate_zlib(a, (w * h) as usize)
            && a.len() == (w * h) as usize
        {
            for (p, &v) in px.chunks_exact_mut(4).zip(&a) {
                p[3] = v;
            }
        }
        Image::from_rgba(w, h, px)
    }

    /// JPEG data as a decoder reads it: the erroneous FF D9 FF D8 some writers put first
    /// dropped, the movie's JPEGTables joined in front when given.
    fn jpeg(&self, data: &[u8], tables: bool) -> Vec<u8> {
        let mut d = data;
        if d.starts_with(&[0xFF, 0xD9, 0xFF, 0xD8]) {
            d = &d[4..];
        }
        if tables && !self.jpeg_tables.is_empty() {
            let mut t = self.jpeg_tables.as_slice();
            if t.starts_with(&[0xFF, 0xD9, 0xFF, 0xD8]) {
                t = &t[4..];
            }
            let t = t.strip_suffix(&[0xFF, 0xD9]).unwrap_or(t);
            let body = d.strip_prefix(&[0xFF, 0xD8]).unwrap_or(d);
            let mut out = t.to_vec();
            out.extend_from_slice(body);
            return out;
        }
        d.to_vec()
    }

    /// Reads tags into the dictionary; gives the timeline's display tags up to the first
    /// ShowFrame (all frames' are not needed).
    fn tags(&mut self, d: &[u8], depth: usize) -> Vec<Tag> {
        let mut r = Bytes::new(d);
        let mut timeline = Vec::new();
        let mut shown = false;
        while r.left() >= 2 {
            let Some(head) = r.u16() else { break };
            let code = head >> 6;
            let mut len = usize::from(head & 0x3F);
            if len == 0x3F {
                let Some(l) = r.u32() else { break };
                len = l as usize;
            }
            let Some(t) = r.bytes(len.min(r.left())) else {
                break;
            };
            if code == 0 {
                break;
            }
            let mut b = Bytes::new(t);
            match code {
                1 => {
                    if !shown {
                        timeline.push(Tag::ShowFrame);
                    }
                    shown = true;
                }
                // PlaceObject, PlaceObject2, PlaceObject3.
                4 | 26 | 70 if !shown => {
                    if let Some(p) = place(&mut b, code) {
                        timeline.push(p);
                    }
                }
                5 | 28 if !shown => {
                    if code == 5 {
                        b.u16();
                    }
                    if let Some(depth) = b.u16() {
                        timeline.push(Tag::Remove(depth));
                    }
                }
                9 if depth == 0 => {
                    if let Some(c) = b.rgb() {
                        self.background = Some(c);
                    }
                }
                2 | 22 | 32 | 83 => {
                    let v = Version(match code {
                        2 => 1,
                        22 => 2,
                        32 => 3,
                        _ => 4,
                    });
                    let Some(id) = b.u16() else { continue };
                    b.rect();
                    if v.0 == 4 {
                        b.rect();
                        b.u8();
                    }
                    if let Some(parts) = shape::shape_with_style(&mut b, v) {
                        self.facts.shapes += 1;
                        self.dict.insert(id, Character::Shape(parts));
                    }
                }
                46 | 84 => {
                    if let Some((id, parts)) = morph(&mut b, code == 84) {
                        self.facts.shapes += 1;
                        self.dict.insert(id, Character::Shape(parts));
                    }
                }
                8 => self.jpeg_tables = t.to_vec(),
                6 | 21 | 35 | 90 => {
                    let Some(id) = b.u16() else { continue };
                    let (data, alpha) = match code {
                        6 | 21 => (b.rest(), None),
                        _ => {
                            let Some(n) = b.u32() else { continue };
                            if code == 90 {
                                b.u16();
                            }
                            let Some(img) = b.bytes((n as usize).min(b.left())) else {
                                continue;
                            };
                            (img, Some(b.rest()))
                        }
                    };
                    let bytes = self.jpeg(data, code == 6);
                    let img = self.decode_image(&bytes, alpha);
                    self.facts.bitmaps += 1;
                    match img {
                        Some(i) => {
                            self.dict.insert(id, Character::Bitmap(Arc::new(i)));
                        }
                        None => self.facts.missing_pictures += 1,
                    }
                }
                20 | 36 => {
                    self.facts.bitmaps += 1;
                    match lossless(&mut b, code == 36) {
                        Some((id, img)) => {
                            self.dict.insert(id, Character::Bitmap(Arc::new(img)));
                        }
                        None => self.facts.missing_pictures += 1,
                    }
                }
                39 if depth < MAX_DEPTH => {
                    let Some(id) = b.u16() else { continue };
                    b.u16();
                    let inner = self.tags(b.rest(), depth + 1);
                    self.facts.sprites += 1;
                    self.dict.insert(id, Character::Sprite(inner));
                }
                10 | 48 | 75 => {
                    if let Some((id, f)) = font(&mut b, code) {
                        self.facts.fonts += 1;
                        self.dict.insert(id, Character::Font(f));
                    }
                }
                11 | 33 => {
                    if let Some((id, t)) = text(&mut b, code == 33) {
                        self.facts.texts += 1;
                        self.dict.insert(id, Character::Text(t));
                    }
                }
                37 => {
                    if let Some((id, t)) = edit_text(&mut b) {
                        self.facts.texts += 1;
                        self.dict.insert(id, Character::EditText(t));
                    }
                }
                7 | 34 => self.facts.buttons += 1,
                60 => {
                    // DefineVideoStream: id, frames, width, height, flags, codec.
                    if let Some(c) = t.get(9) {
                        self.facts.videos.push(video_codec(*c).into());
                    }
                }
                18 | 45 => {
                    if let Some(f) = t.get(1) {
                        let s = sound_format(f >> 4);
                        if !self.facts.sounds.iter().any(|x| x == s) {
                            self.facts.sounds.push(s.into());
                        }
                    }
                }
                14 => {
                    if let Some(f) = t.get(2) {
                        let s = sound_format(f >> 4);
                        if !self.facts.sounds.iter().any(|x| x == s) {
                            self.facts.sounds.push(s.into());
                        }
                    }
                }
                12 | 59 | 72 | 82 => self.facts.actionscript = true,
                _ => {}
            }
        }
        timeline
    }
}

fn place(b: &mut Bytes, code: u16) -> Option<Tag> {
    if code == 4 {
        let id = b.u16()?;
        let depth = b.u16()?;
        let matrix = b.matrix();
        let cx = if b.left() > 0 { b.cxform(false) } else { None };
        return Some(Tag::Place {
            depth,
            id: Some(id),
            matrix,
            cx,
            clip: None,
            move_only: false,
        });
    }
    let flags = b.u8()?;
    let flags2 = if code == 70 { b.u8()? } else { 0 };
    let depth = b.u16()?;
    if code == 70 && (flags2 & 0x08 != 0 || (flags2 & 0x10 != 0 && flags & 0x02 != 0)) {
        // Class name string.
        b.string()?;
    }
    let id = if flags & 0x02 != 0 {
        Some(b.u16()?)
    } else {
        None
    };
    let matrix = if flags & 0x04 != 0 {
        Some(b.matrix()?)
    } else {
        None
    };
    let cx = if flags & 0x08 != 0 {
        Some(b.cxform(true)?)
    } else {
        None
    };
    // The morph ratio: morph shapes are drawn at their start.
    if flags & 0x10 != 0 {
        b.u16()?;
    }
    if flags & 0x20 != 0 {
        b.string()?;
    }
    let clip = if flags & 0x40 != 0 {
        Some(b.u16()?)
    } else {
        None
    };
    Some(Tag::Place {
        depth,
        id,
        matrix,
        cx,
        clip,
        move_only: flags & 0x01 != 0,
    })
}

/// DefineMorphShape (1 or 2) at its start: start styles, start edges.
fn morph(b: &mut Bytes, v2: bool) -> Option<(u16, Vec<Part>)> {
    let id = b.u16()?;
    b.rect()?;
    b.rect()?;
    if v2 {
        b.rect()?;
        b.rect()?;
        b.u8()?;
    }
    b.u32()?;
    let mut n = usize::from(b.u8()?);
    if n == 0xFF {
        n = usize::from(b.u16()?);
    }
    let mut fills = Vec::with_capacity(n);
    for _ in 0..n {
        let t = b.u8()?;
        fills.push(match t {
            0x00 => {
                let c = b.rgba()?;
                b.rgba()?;
                Fill::Solid(c)
            }
            0x10 | 0x12 | 0x13 => {
                let m = b.matrix()?;
                b.matrix()?;
                let k = usize::from(b.u8()? & 15);
                let mut stops = Vec::new();
                for _ in 0..k {
                    let r = b.u8()?;
                    let c = b.rgba()?;
                    b.u8()?;
                    b.rgba()?;
                    stops.push((r, c));
                }
                if t == 0x13 {
                    b.u16()?;
                    b.u16()?;
                }
                Fill::Gradient {
                    radial: t != 0x10,
                    matrix: m,
                    stops,
                }
            }
            0x40..=0x43 => {
                let id = b.u16()?;
                let m = b.matrix()?;
                b.matrix()?;
                Fill::Bitmap {
                    id,
                    matrix: m,
                    repeat: t == 0x40 || t == 0x42,
                }
            }
            _ => return None,
        });
    }
    let mut n = usize::from(b.u8()?);
    if n == 0xFF {
        n = usize::from(b.u16()?);
    }
    let mut lines = Vec::with_capacity(n);
    for _ in 0..n {
        let w = f64::from(b.u16()?);
        b.u16()?;
        let (fill, cap, join) = if v2 {
            let (f1, _f2) = (b.u8()?, b.u8()?);
            if (f1 >> 4) & 3 == 2 {
                b.u16()?;
            }
            if f1 & 0x08 != 0 {
                return None;
            }
            let c = b.rgba()?;
            b.rgba()?;
            (
                Fill::Solid(c),
                specular_drawpage::Cap::Round,
                specular_drawpage::Join::Round,
            )
        } else {
            let c = b.rgba()?;
            b.rgba()?;
            (
                Fill::Solid(c),
                specular_drawpage::Cap::Round,
                specular_drawpage::Join::Round,
            )
        };
        lines.push(shape::Line {
            width: w,
            fill,
            cap,
            join,
        });
    }
    let parts = shape::shape_records(b, Version(3), fills, lines)?;
    Some((id, parts))
}

/// DefineBitsLossless (1 or 2): colour-mapped, RGB555 or 32-bit pixels, zlib.
fn lossless(b: &mut Bytes, alpha: bool) -> Option<(u16, Image)> {
    let id = b.u16()?;
    let format = b.u8()?;
    let w = usize::from(b.u16()?);
    let h = usize::from(b.u16()?);
    let colours = if format == 3 {
        usize::from(b.u8()?) + 1
    } else {
        0
    };
    if w == 0 || h == 0 || w * h > 1 << 26 {
        return None;
    }
    let want = match format {
        3 => colours * if alpha { 4 } else { 3 } + w.div_ceil(4) * 4 * h,
        4 => (w * 2).div_ceil(4) * 4 * h,
        _ => w * h * 4,
    };
    let data = specular_png::inflate_zlib(b.rest(), want * 2 + 4096).ok()?;
    let mut px = vec![0u8; w * h * 4];
    match format {
        3 => {
            let cb = if alpha { 4 } else { 3 };
            let table = data.get(..colours * cb)?;
            let stride = w.div_ceil(4) * 4;
            for y in 0..h {
                for x in 0..w {
                    let i = usize::from(*data.get(colours * cb + y * stride + x)?);
                    let c = table.get(i * cb..i * cb + cb).unwrap_or(&[0, 0, 0, 0]);
                    let o = 4 * (y * w + x);
                    px[o..o + 3].copy_from_slice(&c[..3]);
                    px[o + 3] = if alpha { c[3] } else { 255 };
                }
            }
        }
        4 => {
            let stride = (w * 2).div_ceil(4) * 4;
            for y in 0..h {
                for x in 0..w {
                    let at = y * stride + 2 * x;
                    let v = u16::from_be_bytes([*data.get(at)?, *data.get(at + 1)?]);
                    let c = |s: u16| (((v >> s) & 31) * 255 / 31) as u8;
                    let o = 4 * (y * w + x);
                    px[o..o + 4].copy_from_slice(&[c(10), c(5), c(0), 255]);
                }
            }
        }
        5 => {
            for i in 0..w * h {
                let p = data.get(4 * i..4 * i + 4)?;
                let a = if alpha { p[0] } else { 255 };
                // Version 2's colours are premultiplied by alpha.
                let un = |v: u8| {
                    if alpha && a > 0 && a < 255 {
                        ((u16::from(v) * 255 + u16::from(a) / 2) / u16::from(a)).min(255) as u8
                    } else {
                        v
                    }
                };
                px[4 * i..4 * i + 4].copy_from_slice(&[un(p[1]), un(p[2]), un(p[3]), a]);
            }
        }
        _ => return None,
    }
    Some((id, Image::from_rgba(w as u32, h as u32, px)?))
}

fn font(b: &mut Bytes, code: u16) -> Option<(u16, Font)> {
    let id = b.u16()?;
    if code == 10 {
        let start = b.at;
        let first = usize::from(b.u16()?);
        let n = first / 2;
        let mut offsets = vec![first];
        for _ in 1..n {
            offsets.push(usize::from(b.u16()?));
        }
        let mut glyphs = Vec::with_capacity(n);
        for o in offsets {
            let mut g = Bytes {
                d: b.d,
                at: start + o,
            };
            glyphs.push(
                shape::shape_records(
                    &mut g,
                    Version(1),
                    vec![Fill::Solid([0, 0, 0, 255])],
                    Vec::new(),
                )
                .unwrap_or_default(),
            );
        }
        return Some((id, Font { glyphs, em: 1024.0 }));
    }
    let flags = b.u8()?;
    b.u8()?;
    let name_len = usize::from(b.u8()?);
    b.bytes(name_len)?;
    let n = usize::from(b.u16()?);
    let start = b.at;
    let wide = flags & 0x08 != 0;
    let mut offsets = Vec::with_capacity(n);
    for _ in 0..n {
        offsets.push(if wide {
            b.u32()? as usize
        } else {
            usize::from(b.u16()?)
        });
    }
    let mut glyphs = Vec::with_capacity(n);
    for o in offsets {
        let mut g = Bytes {
            d: b.d,
            at: start + o,
        };
        glyphs.push(
            shape::shape_records(
                &mut g,
                Version(1),
                vec![Fill::Solid([0, 0, 0, 255])],
                Vec::new(),
            )
            .unwrap_or_default(),
        );
    }
    Some((
        id,
        Font {
            glyphs,
            em: if code == 75 { 20480.0 } else { 1024.0 },
        },
    ))
}

fn text(b: &mut Bytes, rgba: bool) -> Option<(u16, Text)> {
    let id = b.u16()?;
    b.rect()?;
    let matrix = b.matrix()?;
    let gbits = u32::from(b.u8()?);
    let abits = u32::from(b.u8()?);
    let mut records = Vec::new();
    let mut cur = TextRecord {
        font: 0,
        colour: [0, 0, 0, 255],
        x: 0.0,
        y: 0.0,
        height: 240.0,
        glyphs: Vec::new(),
    };
    loop {
        let flags = b.u8()?;
        if flags == 0 {
            break;
        }
        if flags & 0x08 != 0 {
            cur.font = b.u16()?;
        }
        if flags & 0x04 != 0 {
            cur.colour = if rgba { b.rgba()? } else { b.rgb()? };
        }
        if flags & 0x01 != 0 {
            cur.x = f64::from(b.i16()?);
        }
        if flags & 0x02 != 0 {
            cur.y = f64::from(b.i16()?);
        }
        if flags & 0x08 != 0 {
            cur.height = f64::from(b.u16()?);
        }
        let n = b.u8()?;
        let mut bits = b.bits();
        let mut glyphs = Vec::with_capacity(usize::from(n));
        for _ in 0..n {
            let g = bits.u(gbits)? as usize;
            let adv = f64::from(bits.s(abits)?);
            glyphs.push((g, adv));
        }
        b.at = bits.done();
        let advance: f64 = glyphs.iter().map(|g| g.1).sum();
        records.push(TextRecord {
            font: cur.font,
            colour: cur.colour,
            x: cur.x,
            y: cur.y,
            height: cur.height,
            glyphs,
        });
        // The next record starts after this one's advances unless it sets its x.
        cur.x += advance;
    }
    Some((id, Text { matrix, records }))
}

fn edit_text(b: &mut Bytes) -> Option<(u16, EditText)> {
    let id = b.u16()?;
    let (x0, x1, y0, y1) = b.rect()?;
    // Two flag bytes, each from its top bit.
    let (f1, f2) = (b.u8()?, b.u8()?);
    let (has_text, has_colour, has_max, has_font) = (
        f1 & 0x80 != 0,
        f1 & 0x04 != 0,
        f1 & 0x02 != 0,
        f1 & 0x01 != 0,
    );
    let (has_class, has_layout, border, html) = (
        f2 & 0x80 != 0,
        f2 & 0x20 != 0,
        f2 & 0x08 != 0,
        f2 & 0x02 != 0,
    );
    let mut height = 240.0;
    if has_font {
        b.u16()?;
    }
    if has_class {
        b.string()?;
    }
    if has_font {
        height = f64::from(b.u16()?);
    }
    let colour = if has_colour {
        b.rgba()?
    } else {
        [0, 0, 0, 255]
    };
    if has_max {
        b.u16()?;
    }
    let mut align = HAlign::Left;
    if has_layout {
        align = match b.u8()? {
            1 => HAlign::Right,
            2 => HAlign::Centre,
            3 => HAlign::Justify,
            _ => HAlign::Left,
        };
        b.u16()?;
        b.u16()?;
        b.u16()?;
        b.u16()?;
    }
    b.string()?;
    let mut text = if has_text { b.string()? } else { String::new() };
    if html {
        text = strip_html(&text);
    }
    Some((
        id,
        EditText {
            bounds: Rect::new(f64::from(x0), f64::from(y0), f64::from(x1), f64::from(y1)),
            height,
            colour,
            align,
            text,
            border,
        },
    ))
}

fn strip_html(s: &str) -> String {
    let mut out = String::new();
    let mut tag = String::new();
    let mut in_tag = false;
    for c in s.chars() {
        match (in_tag, c) {
            (false, '<') => {
                in_tag = true;
                tag.clear();
            }
            (true, '>') => {
                in_tag = false;
                let t = tag.to_ascii_lowercase();
                if (t.starts_with("br") || t == "/p") && !out.ends_with('\n') {
                    out.push('\n');
                }
            }
            (true, c) => tag.push(c),
            (false, c) => out.push(c),
        }
    }
    out.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
        .trim_end()
        .to_owned()
}

/// Plays a timeline's first frame into a display list.
fn display(tags: &[Tag]) -> BTreeMap<u16, Placed> {
    let mut list: BTreeMap<u16, Placed> = BTreeMap::new();
    for t in tags {
        match t {
            Tag::Place {
                depth,
                id,
                matrix,
                cx,
                clip,
                move_only,
                ..
            } => {
                let prev = list.get(depth).cloned();
                let base = match (move_only, prev, id) {
                    (true, Some(p), None) => p,
                    (_, Some(p), Some(i)) if *move_only => Placed { id: *i, ..p },
                    (_, _, Some(i)) => Placed {
                        id: *i,
                        matrix: Affine::IDENTITY,
                        cx: Cxform::default(),
                        clip: None,
                    },
                    _ => continue,
                };
                list.insert(
                    *depth,
                    Placed {
                        matrix: matrix.unwrap_or(base.matrix),
                        cx: cx.unwrap_or(base.cx),
                        clip: clip.or(base.clip),
                        ..base
                    },
                );
            }
            Tag::Remove(d) => {
                list.remove(d);
            }
            Tag::ShowFrame => break,
        }
    }
    list
}

struct Drawer<'a> {
    dict: &'a HashMap<u16, Character>,
}

fn fill_paint(f: &Fill, m: Affine, cx: &Cxform, dict: &HashMap<u16, Character>) -> Option<Paint> {
    Some(match f {
        Fill::Solid(c) => Paint::Solid(cx.apply(*c)),
        Fill::Gradient {
            radial,
            matrix,
            stops,
            ..
        } => {
            let s = shape::stops(stops, cx);
            if s.is_empty() {
                return None;
            }
            let g = m * *matrix;
            if *radial {
                Paint::Radial {
                    transform: g * Affine::scale(16384.0),
                    stops: s,
                }
            } else {
                Paint::Linear {
                    start: g * Point::new(-16384.0, 0.0),
                    end: g * Point::new(16384.0, 0.0),
                    stops: s,
                }
            }
        }
        Fill::Bitmap { id, matrix, repeat } => match dict.get(id) {
            Some(Character::Bitmap(img)) => Paint::Image {
                image: img.clone(),
                transform: m * *matrix,
                repeat: *repeat,
            },
            _ => Paint::Solid(cx.apply([255, 0, 0, 255])),
        },
    })
}

impl Drawer<'_> {
    fn parts(&self, parts: &[Part], m: Affine, cx: &Cxform, ops: &mut Vec<Op>) {
        for p in parts {
            match p {
                Part::Fill(path, f) => {
                    if let Some(paint) = fill_paint(f, m, cx, self.dict) {
                        ops.push(Op::Fill {
                            path: m * path.clone(),
                            even_odd: false,
                            paint,
                        });
                    }
                }
                Part::Line(path, l) => {
                    if let Some(paint) = fill_paint(&l.fill, m, cx, self.dict) {
                        let mut s = shape::stroke(l);
                        s.width *= m.determinant().abs().sqrt();
                        ops.push(Op::Stroke {
                            path: m * path.clone(),
                            stroke: s,
                            paint,
                        });
                    }
                }
            }
        }
    }

    /// The outline of a character (for clip layers).
    fn outline(&self, id: u16, m: Affine, out: &mut BezPath, depth: usize) {
        match self.dict.get(&id) {
            Some(Character::Shape(parts)) => {
                for p in parts {
                    if let Part::Fill(path, _) = p {
                        out.extend((m * path.clone()).elements().iter().copied());
                    }
                }
            }
            Some(Character::Sprite(tags)) if depth < MAX_DEPTH => {
                for p in display(tags).values() {
                    self.outline(p.id, m * p.matrix, out, depth + 1);
                }
            }
            Some(Character::Bitmap(img)) => {
                let r = Rect::new(
                    0.0,
                    0.0,
                    f64::from(img.width) * 20.0,
                    f64::from(img.height) * 20.0,
                );
                out.extend((m * r.to_path(0.1)).elements().iter().copied());
            }
            _ => {}
        }
    }

    fn character(&self, id: u16, m: Affine, cx: &Cxform, ops: &mut Vec<Op>, depth: usize) {
        if depth > MAX_DEPTH {
            return;
        }
        match self.dict.get(&id) {
            Some(Character::Shape(parts)) => self.parts(parts, m, cx, ops),
            Some(Character::Bitmap(img)) => {
                // A bitmap placed directly: one pixel a twip at scale 1/20.
                let alpha = (cx.mult[3] as f32 / 256.0).clamp(0.0, 1.0);
                ops.push(Op::Image {
                    image: img.clone(),
                    transform: m * Affine::scale(20.0),
                    opacity: alpha,
                });
            }
            Some(Character::Sprite(tags)) => self.list(&display(tags), m, cx, ops, depth + 1),
            Some(Character::Text(t)) => {
                for r in &t.records {
                    let Some(Character::Font(f)) = self.dict.get(&r.font) else {
                        continue;
                    };
                    let k = r.height / f.em;
                    let mut x = r.x;
                    for &(g, adv) in &r.glyphs {
                        if let Some(parts) = f.glyphs.get(g) {
                            let gm = m * t.matrix * Affine::translate((x, r.y)) * Affine::scale(k);
                            let tinted: Vec<Part> = parts
                                .iter()
                                .map(|p| match p {
                                    Part::Fill(path, _) => {
                                        Part::Fill(path.clone(), Fill::Solid(r.colour))
                                    }
                                    p => p.clone(),
                                })
                                .collect();
                            self.parts(&tinted, gm, cx, ops);
                        }
                        x += adv;
                    }
                }
            }
            Some(Character::EditText(e)) => {
                let r = e.bounds;
                if e.border {
                    ops.push(Op::Stroke {
                        path: m * r.to_path(0.1),
                        stroke: specular_drawpage::Stroke::new(20.0 * m.determinant().abs().sqrt()),
                        paint: Paint::Solid(cx.apply([0, 0, 0, 255])),
                    });
                }
                if !e.text.trim().is_empty() {
                    let mut b = TextBlock::new(
                        m * Affine::translate((r.x0 + 40.0, r.y0 + 40.0)),
                        (r.width() - 80.0).max(20.0),
                        (r.height() - 40.0).max(20.0),
                    );
                    b.valign = VAlign::Top;
                    for line in e.text.split(['\n', '\r']) {
                        let mut run = Run::new(line, "Times New Roman", e.height);
                        run.color = cx.apply(e.colour);
                        b.paragraphs.push(Paragraph {
                            runs: vec![run],
                            align: e.align,
                            ..Paragraph::default()
                        });
                    }
                    ops.push(Op::Text(Box::new(b)));
                }
            }
            _ => {}
        }
    }

    fn list(
        &self,
        list: &BTreeMap<u16, Placed>,
        m: Affine,
        cx: &Cxform,
        ops: &mut Vec<Op>,
        depth: usize,
    ) {
        // Clip layers: a placement with a clip depth masks the layers above it up to that
        // depth.
        let mut open: Vec<u16> = Vec::new();
        for (&d, p) in list {
            while open.last().is_some_and(|&c| d > c) {
                open.pop();
                ops.push(Op::Pop);
            }
            let pm = m * p.matrix;
            let pc = cx.then(&p.cx);
            if let Some(c) = p.clip {
                let mut path = BezPath::new();
                self.outline(p.id, pm, &mut path, depth);
                ops.push(Op::PushClip {
                    path,
                    even_odd: false,
                });
                open.push(c);
                continue;
            }
            let fade = pc.mult[3] < 256 && pc.add[3] == 0;
            if fade {
                ops.push(Op::PushOpacity((pc.mult[3].max(0) as f32 / 256.0).min(1.0)));
            }
            let inner = if fade {
                Cxform {
                    mult: [pc.mult[0], pc.mult[1], pc.mult[2], 256],
                    add: pc.add,
                }
            } else {
                pc
            };
            self.character(p.id, pm, &inner, ops, depth);
            if fade {
                ops.push(Op::Pop);
            }
        }
        for _ in open {
            ops.push(Op::Pop);
        }
    }
}

/// Reads a movie: its first frame and facts. `images` decodes embedded JPEG, PNG and GIF
/// bitmaps.
pub fn open(d: &[u8], images: Option<&ImageDecoder>) -> Result<Movie, Error> {
    if !is_swf(d) {
        return Err(bad("not a SWF file"));
    }
    let version = d[3];
    let (body, compression) = body(d)?;
    let mut r = Bytes::new(&body);
    let (x0, x1, y0, y1) = r.rect().ok_or_else(|| bad("frame size"))?;
    let rate = f64::from(r.u16().ok_or_else(|| bad("frame rate"))?) / 256.0;
    let frames = r.u16().unwrap_or(0);
    let (w, h) = (f64::from(x1 - x0) / 20.0, f64::from(y1 - y0) / 20.0);
    if !(w > 0.0 && h > 0.0 && w <= 32_000.0 && h <= 32_000.0) {
        return Err(bad("frame size"));
    }
    let mut rd = Reader {
        images,
        dict: HashMap::new(),
        jpeg_tables: Vec::new(),
        background: None,
        facts: Facts {
            version,
            compression,
            frame_rate: rate,
            frame_count: frames,
            ..Facts::default()
        },
    };
    let tags = rd.tags(&body[r.at..], 0);
    let list = display(&tags);
    let mut page = Page::new("frame 1", (w, h));
    page.background = rd.background.unwrap_or([255, 255, 255, 255]);
    let dr = Drawer { dict: &rd.dict };
    let m = Affine::scale(1.0 / 20.0) * Affine::translate((-f64::from(x0), -f64::from(y0)));
    dr.list(&list, m, &Cxform::default(), &mut page.ops, 0);
    // Clip everything to the stage.
    if !page.ops.is_empty() {
        page.ops.insert(
            0,
            Op::PushClip {
                path: Rect::new(0.0, 0.0, w, h).to_path(0.1),
                even_odd: false,
            },
        );
        page.ops.push(Op::Pop);
    }
    Ok(Movie {
        page,
        facts: rd.facts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signatures() {
        assert!(is_swf(b"FWS\x0a\x20\x00\x00\x00"));
        assert!(is_swf(b"CWS\x0a\x20\x00\x00\x00"));
        assert!(!is_swf(b"FWX\x0a\x20\x00\x00\x00"));
    }
}
