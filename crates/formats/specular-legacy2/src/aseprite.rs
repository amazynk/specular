//! Aseprite sprites (`.aseprite`, `.ase`), from Aseprite's published file-format document.
//!
//! Little-endian. A 128-byte header: the file size, the magic 0xA5E0, the frame count,
//! width, height, colour depth (32 RGBA, 16 grey and alpha, 8 indexed), flags (1: layer
//! opacity is valid, 2: group opacity and blend mode are valid), the transparent index of
//! indexed sprites, the colour count and the pixel ratio. Then the frames, each a header
//! (size, magic 0xF1FA, chunk counts, duration in ms) and chunks:
//!
//! * layers (0x2004): flags (1 visible, 8 background, 64 reference), type (image, group,
//!   tilemap), child level, blend mode, opacity and name, in order from the bottom;
//! * cels (0x2005): the layer, position, opacity, type (raw, linked to another frame's
//!   cel, zlib-compressed image, compressed tilemap) and a z-index;
//! * palettes (0x2019, and the old 0x0004 / 0x0011), tags (0x2018), the colour profile
//!   (0x2007) and tilesets (0x2023, tiles stored in the file).
//!
//! Each frame is drawn from the bottom layer up: hidden and reference layers are skipped,
//! groups of visible layers composited into their own canvas when the header says group
//! opacity and blend modes are valid (else straight onto their parent), cels blended with
//! their layer's mode (the W3C/Photoshop definitions, [`specular_blend`]) at the layer's
//! opacity times the cel's. Indexed pixels of the transparent index are transparent except
//! on a background layer. Tilemap cels draw their tiles with the flips their tile words ask
//! for. Tilesets kept in an external file are drawn empty, with a note.

use crate::{Error, Image, Info, Model, Pixels, Result, check_dims};
use specular_blend::{Mode, PorterDuff};

const MAGIC: u16 = 0xA5E0;
const FRAME_MAGIC: u16 = 0xF1FA;
/// Most frames read.
const MAX_FRAMES: usize = 1 << 16;
/// Largest inflated cel or tileset.
const MAX_CEL: usize = 1 << 28;

fn u16le(d: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_le_bytes(d.get(o..o + 2)?.try_into().ok()?))
}

fn i16le(d: &[u8], o: usize) -> Option<i16> {
    u16le(d, o).map(|v| v as i16)
}

fn u32le(d: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(d.get(o..o + 4)?.try_into().ok()?))
}

/// The magic at byte 4 and a known colour depth.
pub fn probe(head: &[u8]) -> bool {
    head.len() >= 14
        && u16le(head, 4) == Some(MAGIC)
        && matches!(u16le(head, 12), Some(8 | 16 | 32))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayerKind {
    Image,
    Group,
    Tilemap,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    pub name: String,
    pub kind: LayerKind,
    pub flags: u16,
    pub level: u16,
    pub blend: u16,
    pub opacity: u8,
    pub tileset: u32,
}

impl Layer {
    pub fn visible(&self) -> bool {
        self.flags & 1 != 0
    }
    pub fn background(&self) -> bool {
        self.flags & 8 != 0
    }
    pub fn reference(&self) -> bool {
        self.flags & 64 != 0
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Content {
    /// Pixels (`w × h` at the sprite's depth), inflated.
    Image { w: u32, h: u32, px: Vec<u8> },
    /// The same layer's cel in another frame.
    Linked(u16),
    /// Tile words (`w × h`), with the masks of the id and the flips.
    Tiles {
        w: u32,
        h: u32,
        words: Vec<u32>,
        id: u32,
        flips: [u32; 3],
    },
}

#[derive(Clone, Debug, PartialEq)]
struct Cel {
    layer: u16,
    x: i32,
    y: i32,
    opacity: u8,
    z: i16,
    content: Content,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Tag {
    pub name: String,
    pub from: u16,
    pub to: u16,
    /// 0 forward, 1 reverse, 2 ping-pong, 3 ping-pong reverse.
    pub direction: u8,
}

#[derive(Clone, Debug, PartialEq)]
struct Tileset {
    id: u32,
    tile_w: u32,
    tile_h: u32,
    count: u32,
    /// The tiles stacked vertically, at the sprite's depth; empty when external.
    px: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    /// Duration in milliseconds.
    pub duration: u16,
    cels: Vec<Cel>,
}

/// A parsed sprite.
#[derive(Clone, Debug, PartialEq)]
pub struct Sprite {
    pub width: u32,
    pub height: u32,
    /// Bits per pixel: 32, 16 or 8.
    pub depth: u16,
    pub flags: u32,
    pub transparent: u8,
    pub pixel_ratio: (u8, u8),
    pub layers: Vec<Layer>,
    pub frames: Vec<Frame>,
    pub tags: Vec<Tag>,
    pub palette: Vec<[u8; 4]>,
    pub icc: bool,
    pub external_tiles: bool,
    tilesets: Vec<Tileset>,
    /// The file stopped before its last frame.
    pub truncated: bool,
}

fn string(d: &[u8], at: usize) -> Option<(String, usize)> {
    let n = u16le(d, at)? as usize;
    let s = d.get(at + 2..at + 2 + n)?;
    Some((
        crate::swatch::short(&String::from_utf8_lossy(s)),
        at + 2 + n,
    ))
}

fn inflate(src: &[u8], need: usize) -> Vec<u8> {
    let mut out = vec![0u8; need.min(MAX_CEL)];
    let n = crate::inflate_zlib(src, &mut out).unwrap_or(0);
    out.truncate(n);
    out
}

fn bpp(depth: u16) -> usize {
    (depth / 8) as usize
}

pub fn parse(d: &[u8]) -> Result<Sprite> {
    if !probe(d) {
        return Err(Error::NotRecognised);
    }
    if d.len() < 128 {
        return Err(Error::Truncated("header".into()));
    }
    let g16 = |o| u16le(d, o).unwrap_or(0);
    let frames_n = g16(6) as usize;
    let (width, height, depth) = (u32::from(g16(8)), u32::from(g16(10)), g16(12));
    check_dims(width.into(), height.into())?;
    let flags = u32le(d, 14).unwrap_or(0);
    let transparent = d[28];
    let pixel_ratio = (d[34], d[35]);
    let mut s = Sprite {
        width,
        height,
        depth,
        flags,
        transparent,
        pixel_ratio,
        layers: Vec::new(),
        frames: Vec::new(),
        tags: Vec::new(),
        palette: Vec::new(),
        icc: false,
        external_tiles: false,
        tilesets: Vec::new(),
        truncated: false,
    };
    let b = bpp(depth);
    let mut at = 128;
    let mut new_palette = false;
    for _ in 0..frames_n.clamp(1, MAX_FRAMES) {
        let (Some(len), Some(magic)) = (u32le(d, at), u16le(d, at + 4)) else {
            s.truncated = true;
            break;
        };
        if magic != FRAME_MAGIC || (len as usize) < 16 {
            return Err(Error::Malformed(format!(
                "frame {} header",
                s.frames.len() + 1
            )));
        }
        let end = at.saturating_add(len as usize).min(d.len());
        s.truncated |= at + len as usize > d.len();
        let old = g16(at + 6) as usize;
        let duration = g16(at + 8);
        let new = u32le(d, at + 12).unwrap_or(0) as usize;
        let chunks = if new == 0 { old } else { new };
        let mut frame = Frame {
            duration,
            cels: Vec::new(),
        };
        let mut p = at + 16;
        for _ in 0..chunks {
            // Chunks lie within their frame: past its end the loop only spun, up to 2^32
            // times for a damaged chunk count.
            if p.saturating_add(6) > end {
                break;
            }
            let (Some(clen), Some(kind)) = (u32le(d, p), u16le(d, p + 4)) else {
                break;
            };
            if clen < 6 {
                break;
            }
            let cend = p.saturating_add(clen as usize).min(end);
            let c = &d[(p + 6).min(cend)..cend];
            match kind {
                0x2004 => {
                    let k = u16le(c, 2).unwrap_or(0);
                    let (name, q) = string(c, 16).unwrap_or_default();
                    s.layers.push(Layer {
                        name,
                        kind: match k {
                            1 => LayerKind::Group,
                            2 => LayerKind::Tilemap,
                            _ => LayerKind::Image,
                        },
                        flags: u16le(c, 0).unwrap_or(1),
                        level: u16le(c, 4).unwrap_or(0),
                        blend: u16le(c, 10).unwrap_or(0),
                        opacity: c.get(12).copied().unwrap_or(255),
                        tileset: if k == 2 { u32le(c, q).unwrap_or(0) } else { 0 },
                    });
                }
                0x2005 if c.len() >= 16 => {
                    let ty = u16le(c, 7).unwrap_or(0);
                    let content = match ty {
                        0 | 2 => {
                            let (w, h) = (u32::from(g(c, 16)), u32::from(g(c, 18)));
                            let need = (w as usize) * (h as usize) * b;
                            let body = c.get(20..).unwrap_or(&[]);
                            let px = if ty == 0 {
                                body[..body.len().min(need)].to_vec()
                            } else {
                                inflate(body, need)
                            };
                            Content::Image { w, h, px }
                        }
                        1 => Content::Linked(g(c, 16)),
                        3 => {
                            let (w, h, bits) = (u32::from(g(c, 16)), u32::from(g(c, 18)), g(c, 20));
                            let m = |o| u32le(c, o).unwrap_or(0);
                            let tb = (bits / 8).max(1) as usize;
                            let raw =
                                inflate(c.get(48..).unwrap_or(&[]), w as usize * h as usize * tb);
                            let words = raw
                                .chunks_exact(tb)
                                .map(|t| match tb {
                                    1 => u32::from(t[0]),
                                    2 => u32::from(u16::from_le_bytes([t[0], t[1]])),
                                    _ => u32::from_le_bytes([t[0], t[1], t[2], t[3]]),
                                })
                                .collect();
                            Content::Tiles {
                                w,
                                h,
                                words,
                                id: m(22),
                                flips: [m(26), m(30), m(34)],
                            }
                        }
                        _ => {
                            p = cend;
                            continue;
                        }
                    };
                    frame.cels.push(Cel {
                        layer: g(c, 0),
                        x: i32::from(i16le(c, 2).unwrap_or(0)),
                        y: i32::from(i16le(c, 4).unwrap_or(0)),
                        opacity: c[6],
                        z: i16le(c, 9).unwrap_or(0),
                        content,
                    });
                }
                0x2019 => {
                    new_palette = true;
                    let size = u32le(c, 0).unwrap_or(0).min(1 << 16) as usize;
                    let (first, last) = (
                        u32le(c, 4).unwrap_or(0) as usize,
                        u32le(c, 8).unwrap_or(0) as usize,
                    );
                    if s.palette.len() < size {
                        s.palette.resize(size, [0, 0, 0, 255]);
                    }
                    let mut q = 20;
                    for i in first..=last.min(first + (1 << 16)) {
                        let Some(e) = c.get(q..q + 6) else { break };
                        let f = u16::from_le_bytes([e[0], e[1]]);
                        if i < s.palette.len() {
                            s.palette[i] = [e[2], e[3], e[4], e[5]];
                        }
                        q += 6;
                        if f & 1 != 0 {
                            q = string(c, q).map_or(c.len(), |(_, n)| n);
                        }
                    }
                }
                // Old palettes count only in files without the new chunk.
                0x0004 | 0x0011 if !new_palette => {
                    // Old palettes: packets of (skip, count) and RGB triples (0x0011 in 6 bits).
                    let packets = g(c, 0);
                    let mut q = 2;
                    let mut i = 0usize;
                    for _ in 0..packets {
                        let (Some(&skip), Some(&n)) = (c.get(q), c.get(q + 1)) else {
                            break;
                        };
                        q += 2;
                        i += skip as usize;
                        let n = if n == 0 { 256 } else { n as usize };
                        for _ in 0..n {
                            let Some(e) = c.get(q..q + 3) else { break };
                            let v = |x: u8| {
                                if kind == 0x0011 {
                                    (x << 2) | (x >> 4)
                                } else {
                                    x
                                }
                            };
                            if s.palette.len() <= i {
                                s.palette.resize(i + 1, [0, 0, 0, 255]);
                            }
                            s.palette[i] = [v(e[0]), v(e[1]), v(e[2]), 255];
                            i += 1;
                            q += 3;
                        }
                    }
                }
                0x2018 => {
                    let n = g(c, 0);
                    let mut q = 10;
                    for _ in 0..n {
                        let (Some(from), Some(to), Some(&dir)) =
                            (u16le(c, q), u16le(c, q + 2), c.get(q + 4))
                        else {
                            break;
                        };
                        let Some((name, next)) = string(c, q + 17) else {
                            break;
                        };
                        s.tags.push(Tag {
                            name,
                            from,
                            to,
                            direction: dir,
                        });
                        q = next;
                    }
                }
                0x2007 => s.icc |= g(c, 0) == 2,
                0x2023 => {
                    let (id, tflags, count) = (
                        u32le(c, 0).unwrap_or(0),
                        u32le(c, 4).unwrap_or(0),
                        u32le(c, 8).unwrap_or(0),
                    );
                    let (tw, th) = (u32::from(g(c, 12)), u32::from(g(c, 14)));
                    let Some((_, mut q)) = string(c, 32) else {
                        p = cend;
                        continue;
                    };
                    if tflags & 1 != 0 {
                        q += 8;
                    }
                    let mut px = Vec::new();
                    if tflags & 2 != 0 {
                        let len = u32le(c, q).unwrap_or(0) as usize;
                        let z = c
                            .get(q + 4..(q + 4).saturating_add(len).min(c.len()))
                            .unwrap_or(&[]);
                        // Saturating: a damaged count overflows the product (inflate caps it).
                        let need = (tw as usize * th as usize)
                            .saturating_mul(count as usize)
                            .saturating_mul(b);
                        px = inflate(z, need);
                    } else {
                        s.external_tiles = true;
                    }
                    s.tilesets.push(Tileset {
                        id,
                        tile_w: tw,
                        tile_h: th,
                        count,
                        px,
                    });
                }
                _ => {}
            }
            p = cend;
        }
        s.frames.push(frame);
        at = at.saturating_add(len as usize);
        if at >= d.len() {
            break;
        }
    }
    if s.frames.is_empty() {
        return Err(Error::Truncated("no frames".into()));
    }
    s.truncated |= s.frames.len() < frames_n;
    Ok(s)
}

fn g(c: &[u8], o: usize) -> u16 {
    u16le(c, o).unwrap_or(0)
}

/// Aseprite's blend modes, by their number.
fn mode(n: u16) -> Mode {
    match n {
        1 => Mode::Multiply,
        2 => Mode::Screen,
        3 => Mode::Overlay,
        4 => Mode::Darken,
        5 => Mode::Lighten,
        6 => Mode::ColorDodge,
        7 => Mode::ColorBurn,
        8 => Mode::HardLight,
        9 => Mode::SoftLight,
        10 => Mode::Difference,
        11 => Mode::Exclusion,
        12 => Mode::Hue,
        13 => Mode::Saturation,
        14 => Mode::Color,
        15 => Mode::Luminosity,
        16 => Mode::LinearDodge,
        17 => Mode::Subtract,
        18 => Mode::Divide,
        _ => Mode::Normal,
    }
}

const MODE_NAMES: [&str; 19] = [
    "normal",
    "multiply",
    "screen",
    "overlay",
    "darken",
    "lighten",
    "color dodge",
    "color burn",
    "hard light",
    "soft light",
    "difference",
    "exclusion",
    "hue",
    "saturation",
    "color",
    "luminosity",
    "addition",
    "subtract",
    "divide",
];

/// A straight-alpha float canvas.
struct Canvas {
    w: usize,
    h: usize,
    px: Vec<[f32; 4]>,
}

impl Canvas {
    fn new(w: u32, h: u32) -> Canvas {
        Canvas {
            w: w as usize,
            h: h as usize,
            px: vec![[0.0; 4]; w as usize * h as usize],
        }
    }

    fn put(&mut self, x: i64, y: i64, c: [f32; 4], m: Mode) {
        if x < 0 || y < 0 || x as usize >= self.w || y as usize >= self.h || c[3] <= 0.0 {
            return;
        }
        let d = &mut self.px[y as usize * self.w + x as usize];
        let (rgb, a) = specular_blend::composite(
            m,
            PorterDuff::SourceOver,
            [d[0], d[1], d[2]],
            d[3],
            [c[0], c[1], c[2]],
            c[3],
        );
        *d = [rgb[0], rgb[1], rgb[2], a];
    }

    fn over(&mut self, src: &Canvas, m: Mode, k: f32) {
        for y in 0..self.h as i64 {
            for x in 0..self.w as i64 {
                let mut c = src.px[y as usize * src.w + x as usize];
                c[3] *= k;
                self.put(x, y, c, m);
            }
        }
    }
}

impl Sprite {
    /// The colour of pixel bytes `p` (one pixel at the sprite's depth).
    fn colour(&self, p: &[u8], background: bool) -> [f32; 4] {
        let f = |v: u8| f32::from(v) / 255.0;
        match self.depth {
            32 => [f(p[0]), f(p[1]), f(p[2]), f(p[3])],
            16 => [f(p[0]), f(p[0]), f(p[0]), f(p[1])],
            _ => {
                let i = p[0];
                if i == self.transparent && !background {
                    return [0.0; 4];
                }
                let c = self
                    .palette
                    .get(i as usize)
                    .copied()
                    .unwrap_or([0, 0, 0, 255]);
                [f(c[0]), f(c[1]), f(c[2]), f(c[3])]
            }
        }
    }

    fn cel(&self, frame: usize, layer: u16) -> Option<&Cel> {
        let mut f = frame;
        for _ in 0..8 {
            let c = self.frames.get(f)?.cels.iter().find(|c| c.layer == layer)?;
            match c.content {
                Content::Linked(to) => f = to as usize,
                _ => return Some(c),
            }
        }
        None
    }

    fn draw_cel(&self, out: &mut Canvas, cel: &Cel, l: &Layer) {
        let k = f32::from(cel.opacity) / 255.0
            * if self.flags & 1 != 0 {
                f32::from(l.opacity) / 255.0
            } else {
                1.0
            };
        let m = mode(l.blend);
        let b = bpp(self.depth);
        match &cel.content {
            Content::Image { w, h, px } => {
                for (i, p) in px
                    .chunks_exact(b)
                    .take(*w as usize * *h as usize)
                    .enumerate()
                {
                    let (x, y) = ((i % *w as usize) as i64, (i / *w as usize) as i64);
                    let mut c = self.colour(p, l.background());
                    c[3] *= k;
                    out.put(i64::from(cel.x) + x, i64::from(cel.y) + y, c, m);
                }
            }
            Content::Tiles {
                w,
                h,
                words,
                id,
                flips,
            } => {
                let Some(ts) = self.tilesets.iter().find(|t| t.id == l.tileset) else {
                    return;
                };
                let (tw, th) = (ts.tile_w as usize, ts.tile_h as usize);
                let tile_bytes = tw * th * b;
                for (i, &word) in words.iter().enumerate().take(*w as usize * *h as usize) {
                    let t = (word & id) as usize;
                    if t >= ts.count as usize || ts.px.len() < (t + 1) * tile_bytes {
                        continue;
                    }
                    let (fx, fy, fd) = (
                        word & flips[0] != 0,
                        word & flips[1] != 0,
                        word & flips[2] != 0,
                    );
                    let tile = &ts.px[t * tile_bytes..(t + 1) * tile_bytes];
                    let (ox, oy) = ((i % *w as usize) * tw, (i / *w as usize) * th);
                    // Only the part on the canvas: a large tilemap of large tiles off the
                    // canvas cost billions of skipped pixels (a hang on a small file).
                    let (cx, cy) = (i64::from(cel.x) + ox as i64, i64::from(cel.y) + oy as i64);
                    let span = |c: i64, n: usize, max: usize| {
                        let from = (-c).clamp(0, n as i64) as usize;
                        let to = (max as i64 - c).clamp(0, n as i64) as usize;
                        from..to.max(from)
                    };
                    let (xs, ys) = (span(cx, tw, out.w), span(cy, th, out.h));
                    for ty in ys {
                        for tx in xs.clone() {
                            let (mut sx, mut sy) = (tx, ty);
                            if fd && tw == th {
                                std::mem::swap(&mut sx, &mut sy);
                            }
                            if fx {
                                sx = tw - 1 - sx;
                            }
                            if fy {
                                sy = th - 1 - sy;
                            }
                            let p = &tile[(sy * tw + sx) * b..(sy * tw + sx + 1) * b];
                            let mut c = self.colour(p, false);
                            c[3] *= k;
                            out.put(
                                i64::from(cel.x) + (ox + tx) as i64,
                                i64::from(cel.y) + (oy + ty) as i64,
                                c,
                                m,
                            );
                        }
                    }
                }
            }
            Content::Linked(_) => {}
        }
    }

    /// Draws layers `range` (siblings at `level` and their children) of `frame` onto `out`.
    fn draw_layers(&self, out: &mut Canvas, frame: usize, start: usize, end: usize, level: u16) {
        // The layers at this level, each with the end of its children.
        let mut items = Vec::new();
        let mut i = start;
        while i < end {
            let mut j = i + 1;
            while j < end && self.layers[j].level > level {
                j += 1;
            }
            items.push((i, j));
            i = j;
        }
        // Cels' z-index reorders siblings: by (index + z, z).
        let z = |li: usize| self.cel(frame, li as u16).map_or(0, |c| i64::from(c.z));
        let mut order: Vec<usize> = (0..items.len()).collect();
        order.sort_by_key(|&k| {
            let zz = z(items[k].0);
            (k as i64 + zz, zz)
        });
        for k in order {
            let (li, children_end) = items[k];
            let l = &self.layers[li];
            if !l.visible() || l.reference() {
                continue;
            }
            match l.kind {
                LayerKind::Group => {
                    if self.flags & 2 != 0 {
                        let mut g = Canvas::new(self.width, self.height);
                        self.draw_layers(&mut g, frame, li + 1, children_end, l.level + 1);
                        out.over(&g, mode(l.blend), f32::from(l.opacity) / 255.0);
                    } else {
                        self.draw_layers(out, frame, li + 1, children_end, l.level + 1);
                    }
                }
                _ => {
                    if let Some(c) = self.cel(frame, li as u16) {
                        self.draw_cel(out, c, l);
                    }
                }
            }
        }
    }

    /// Frame `frame` composited, RGBA8 (straight alpha).
    pub fn render(&self, frame: usize) -> Vec<u8> {
        let mut c = Canvas::new(self.width, self.height);
        let level = self.layers.iter().map(|l| l.level).min().unwrap_or(0);
        self.draw_layers(
            &mut c,
            frame.min(self.frames.len() - 1),
            0,
            self.layers.len(),
            level,
        );
        c.px.iter()
            .flat_map(|p| {
                let a = (p[3].clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
                if a == 0 {
                    [0; 4]
                } else {
                    let e = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
                    [e(p[0]), e(p[1]), e(p[2]), a]
                }
            })
            .collect()
    }

    pub fn info(&self) -> Info {
        let model = match self.depth {
            32 => Model::Rgb,
            16 => Model::Grey,
            _ => Model::Palette,
        };
        let mut fields = vec![
            ("Frames".to_string(), self.frames.len().to_string()),
            ("Layers".into(), self.layers.len().to_string()),
            (
                "Colour mode".into(),
                match self.depth {
                    32 => "RGBA",
                    16 => "grayscale",
                    _ => "indexed",
                }
                .into(),
            ),
        ];
        if self.depth == 8 {
            fields.push(("Palette colours".into(), self.palette.len().to_string()));
            fields.push(("Transparent index".into(), self.transparent.to_string()));
        }
        for t in self.tags.iter().take(64) {
            fields.push((
                format!("Tag {}", t.name),
                format!(
                    "frames {}–{}, {}",
                    t.from + 1,
                    t.to + 1,
                    ["forward", "reverse", "ping-pong", "ping-pong reverse"]
                        .get(t.direction as usize)
                        .unwrap_or(&"forward")
                ),
            ));
        }
        for l in self.layers.iter().take(256) {
            let mut v = match l.kind {
                LayerKind::Image => "image".to_string(),
                LayerKind::Group => "group".to_string(),
                LayerKind::Tilemap => "tilemap".to_string(),
            };
            v += &format!(
                ", {}",
                MODE_NAMES.get(l.blend as usize).unwrap_or(&"normal")
            );
            if l.opacity < 255 && self.flags & 1 != 0 {
                v += &format!(", {} %", (u32::from(l.opacity) * 100 + 127) / 255);
            }
            if !l.visible() {
                v += ", hidden";
            }
            if l.reference() {
                v += ", reference (not drawn)";
            }
            fields.push((
                format!("Layer {}{}", "  ".repeat(l.level.min(8) as usize), l.name),
                v,
            ));
        }
        if self.pixel_ratio.0 > 0
            && self.pixel_ratio.1 > 0
            && self.pixel_ratio.0 != self.pixel_ratio.1
        {
            fields.push((
                "Pixel ratio".into(),
                format!("{}:{}", self.pixel_ratio.0, self.pixel_ratio.1),
            ));
        }
        if self.icc {
            fields.push(("Colour profile".into(), "ICC (not applied)".into()));
        }
        if self.external_tiles {
            fields.push((
                "Tilesets".into(),
                "kept in another file: drawn empty".into(),
            ));
        }
        Info {
            width: self.width,
            height: self.height,
            pages: 1,
            bit_depth: 8,
            model,
            has_alpha: true,
            orientation: 1,
            coding: format!(
                "Aseprite {}, {} frame{}",
                match self.depth {
                    32 => "RGBA",
                    16 => "grayscale",
                    _ => "indexed",
                },
                self.frames.len(),
                if self.frames.len() == 1 { "" } else { "s" }
            ),
            summary: Vec::new(),
            fields,
        }
    }
}

pub fn info(data: &[u8], _page: u32) -> Result<Info> {
    Ok(parse(data)?.info())
}

pub fn decode(data: &[u8], _page: u32) -> Result<Image> {
    let s = parse(data)?;
    Ok(Image {
        width: s.width,
        height: s.height,
        pixels: Pixels::Rgba8(s.render(0)),
        truncated: s.truncated,
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    fn chunk(kind: u16, body: &[u8]) -> Vec<u8> {
        let mut c = ((body.len() + 6) as u32).to_le_bytes().to_vec();
        c.extend_from_slice(&kind.to_le_bytes());
        c.extend_from_slice(body);
        c
    }

    fn layer(name: &str, flags: u16, kind: u16, level: u16, blend: u16, opacity: u8) -> Vec<u8> {
        let mut b = Vec::new();
        for v in [flags, kind, level, 0, 0, blend] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        b.extend_from_slice(&[opacity, 0, 0, 0]);
        b.extend_from_slice(&(name.len() as u16).to_le_bytes());
        b.extend_from_slice(name.as_bytes());
        chunk(0x2004, &b)
    }

    fn cel(layer: u16, x: i16, y: i16, w: u16, h: u16, px: &[u8]) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(&layer.to_le_bytes());
        b.extend_from_slice(&x.to_le_bytes());
        b.extend_from_slice(&y.to_le_bytes());
        b.push(255);
        b.extend_from_slice(&0u16.to_le_bytes());
        b.extend_from_slice(&[0; 7]);
        b.extend_from_slice(&w.to_le_bytes());
        b.extend_from_slice(&h.to_le_bytes());
        b.extend_from_slice(px);
        chunk(0x2005, &b)
    }

    fn linked(layer: u16, to: u16) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(&layer.to_le_bytes());
        b.extend_from_slice(&[0; 4]);
        b.push(255);
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&[0; 7]);
        b.extend_from_slice(&to.to_le_bytes());
        chunk(0x2005, &b)
    }

    /// A sprite of `frames` (chunk lists), RGBA, `w × h`.
    pub(crate) fn sprite(w: u16, h: u16, depth: u16, frames: &[Vec<Vec<u8>>]) -> Vec<u8> {
        let mut d = vec![0u8; 128];
        d[4..6].copy_from_slice(&MAGIC.to_le_bytes());
        d[6..8].copy_from_slice(&(frames.len() as u16).to_le_bytes());
        d[8..10].copy_from_slice(&w.to_le_bytes());
        d[10..12].copy_from_slice(&h.to_le_bytes());
        d[12..14].copy_from_slice(&depth.to_le_bytes());
        d[14..18].copy_from_slice(&3u32.to_le_bytes());
        for f in frames {
            let body: Vec<u8> = f.concat();
            let mut fh = ((body.len() + 16) as u32).to_le_bytes().to_vec();
            fh.extend_from_slice(&FRAME_MAGIC.to_le_bytes());
            fh.extend_from_slice(&(f.len() as u16).to_le_bytes());
            fh.extend_from_slice(&100u16.to_le_bytes());
            fh.extend_from_slice(&[0, 0]);
            fh.extend_from_slice(&(f.len() as u32).to_le_bytes());
            d.extend(fh);
            d.extend(body);
        }
        let n = d.len() as u32;
        d[..4].copy_from_slice(&n.to_le_bytes());
        d
    }

    #[test]
    fn layers_cels_links_and_blends() {
        let red = [255, 0, 0, 255];
        let half_blue = [0, 0, 255, 128];
        let f0 = vec![
            layer("bg", 1 | 8, 0, 0, 0, 255),
            layer("top", 1, 0, 0, 1, 255),
            layer("hidden", 0, 0, 0, 0, 255),
            cel(0, 0, 0, 2, 1, &[red, red].concat()),
            cel(1, 1, 0, 1, 1, &half_blue),
            cel(2, 0, 0, 1, 1, &[0, 255, 0, 255]),
        ];
        let f1 = vec![linked(0, 0)];
        let d = sprite(2, 1, 32, &[f0, f1]);
        assert!(probe(&d));
        let s = parse(&d).unwrap();
        assert_eq!((s.frames.len(), s.layers.len()), (2, 3));
        let px = s.render(0);
        assert_eq!(&px[..4], &red);
        // Multiply of half-opaque blue over red: darker red.
        assert_eq!(px[7], 255);
        assert!(px[4] > 100 && px[4] < 140 && px[6] == 0, "{:?}", &px[4..8]);
        // Frame 1 links the background's cel; the top layer has none there.
        assert_eq!(s.render(1), [red, red].concat());
        let i = s.info();
        assert!(
            i.fields
                .iter()
                .any(|(k, v)| k == "Layer hidden" && v.contains("hidden"))
        );
    }

    #[test]
    fn indexed_and_groups() {
        let mut pal = Vec::new();
        for v in [3u32, 0, 2] {
            pal.extend_from_slice(&v.to_le_bytes());
        }
        pal.extend_from_slice(&[0; 8]);
        for c in [[0u8, 0, 0, 255], [10, 20, 30, 255], [40, 50, 60, 255]] {
            pal.extend_from_slice(&[0, 0]);
            pal.extend_from_slice(&c);
        }
        let f0 = vec![
            chunk(0x2019, &pal),
            layer("group", 1, 1, 0, 0, 255),
            layer("child", 1, 0, 1, 0, 255),
            cel(1, 0, 0, 2, 1, &[0, 2]),
        ];
        let d = sprite(2, 1, 8, &[f0]);
        let s = parse(&d).unwrap();
        assert_eq!(s.render(0), [0, 0, 0, 0, 40, 50, 60, 255]);
    }
}
