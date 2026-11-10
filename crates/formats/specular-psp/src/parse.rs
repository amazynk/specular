//! The block structure: headers, the general image attributes, creator and extended data, the
//! palette, the composite image bank (or PSP 5's thumbnail block) and the layer bank.

use crate::{
    BlendMode, Channel, Compression, Error, Info, Layer, LayerKind, LayerRec, MAX_DIMENSION,
    MAX_PIXELS, Node, Psp, Stored, StoredBody,
};
use std::ops::Range;

const BLOCK_SIG: &[u8; 4] = b"~BK\0";
const FIELD_SIG: &[u8; 4] = b"~FL\0";

// Block identifiers (PSPBlockID).
const IMAGE: u16 = 0;
const CREATOR: u16 = 1;
const COLOR: u16 = 2;
const LAYER_START: u16 = 3;
const LAYER: u16 = 4;
const CHANNEL: u16 = 5;
/// PSP 6+: a composite image; PSP 5: the thumbnail block.
const COMPOSITE_OR_THUMB: u16 = 9;
const TUBE: u16 = 11;
const COMPOSITE_BANK: u16 = 16;
const COMPOSITE_ATTR: u16 = 17;
const JPEG: u16 = 18;
const GROUP_EXT: u16 = 25;
const VECTOR_EXT: u16 = 13;
/// Most layers a file may hold (writers stop at 500; generous for later releases).
const MAX_LAYERS: usize = 10_000;
/// Most blocks walked inside one parent (bounds hostile files).
const MAX_BLOCKS: usize = 100_000;

/// A block: its id and body range.
#[derive(Clone, Debug)]
pub(crate) struct Block {
    pub(crate) id: u16,
    /// The "initial data chunk length" of version-3 headers.
    pub(crate) init: u32,
    pub(crate) body: Range<usize>,
    /// Whether the body ran past its parent or the file (cut short).
    pub(crate) cut: bool,
}

/// Reads little-endian values with bounds checks.
pub(crate) struct Rd<'a>(pub(crate) &'a [u8]);

impl Rd<'_> {
    pub(crate) fn u8(&self, at: usize) -> Option<u8> {
        self.0.get(at).copied()
    }
    pub(crate) fn u16(&self, at: usize) -> Option<u16> {
        Some(u16::from_le_bytes(self.0.get(at..at + 2)?.try_into().ok()?))
    }
    pub(crate) fn u32(&self, at: usize) -> Option<u32> {
        Some(u32::from_le_bytes(self.0.get(at..at + 4)?.try_into().ok()?))
    }
    pub(crate) fn i32(&self, at: usize) -> Option<i32> {
        self.u32(at).map(|v| v as i32)
    }
    pub(crate) fn f64(&self, at: usize) -> Option<f64> {
        Some(f64::from_le_bytes(self.0.get(at..at + 8)?.try_into().ok()?))
    }
}

struct Parser<'a> {
    d: &'a [u8],
    r: Rd<'a>,
    major: u16,
}

fn trunc(what: &str) -> Error {
    Error::Truncated(what.into())
}

impl<'a> Parser<'a> {
    fn header_len(&self) -> usize {
        if self.major <= 3 { 14 } else { 10 }
    }

    /// The block whose header starts at `at`, inside `end`.
    fn block(&self, at: usize, end: usize) -> Option<Block> {
        if self.d.get(at..at + 4)? != BLOCK_SIG {
            return None;
        }
        let id = self.r.u16(at + 4)?;
        let (init, total) = if self.major <= 3 {
            (self.r.u32(at + 6)?, self.r.u32(at + 10)?)
        } else {
            (0, self.r.u32(at + 6)?)
        };
        let start = at + self.header_len();
        if start > end {
            return None;
        }
        let want = start.checked_add(total as usize)?;
        Some(Block {
            id,
            init,
            body: start..want.min(end),
            cut: want > end,
        })
    }

    /// The blocks one after another from `at` to `end`; stops at anything that is not a
    /// block header.
    fn blocks(&self, mut at: usize, end: usize) -> Vec<Block> {
        let mut out = Vec::new();
        while out.len() < MAX_BLOCKS {
            let Some(b) = self.block(at, end) else { break };
            at = b.body.end;
            let cut = b.cut;
            out.push(b);
            if cut {
                break;
            }
        }
        out
    }

    /// The first chunk of a block body (v4+: a size-prefixed chunk, size included; v3: the
    /// header's initial chunk length) and where the rest of the body starts.
    fn first_chunk(&self, b: &Block) -> (Range<usize>, usize) {
        let s = b.body.start;
        if self.major <= 3 {
            let e = (s + b.init as usize).min(b.body.end);
            (s..e, e)
        } else {
            let n = self.r.u32(s).unwrap_or(0) as usize;
            let e = s.saturating_add(n.max(4)).min(b.body.end);
            ((s + 4).min(e)..e, e)
        }
    }

    /// A size-prefixed chunk at `at` (v4+): its content and where it ends.
    fn chunk_at(&self, at: usize, end: usize) -> (Range<usize>, usize) {
        let n = self.r.u32(at).unwrap_or(0) as usize;
        let e = at.saturating_add(n.max(4)).min(end);
        ((at + 4).min(e)..e, e)
    }

    /// `~FL\0` fields: (keyword, value range).
    fn fields(&self, body: &Range<usize>) -> Vec<(u16, Range<usize>)> {
        let mut out = Vec::new();
        let mut at = body.start;
        while out.len() < 4096 && at + 10 <= body.end {
            if &self.d[at..at + 4] != FIELD_SIG {
                // v3 extended data blocks may start with an undefined chunk: look ahead.
                match self.d[at..body.end].windows(4).position(|w| w == FIELD_SIG) {
                    Some(p) => {
                        at += p;
                        continue;
                    }
                    None => break,
                }
            }
            let key = self.r.u16(at + 4).unwrap_or(0);
            let len = self.r.u32(at + 6).unwrap_or(0) as usize;
            let s = at + 10;
            let e = s.saturating_add(len).min(body.end);
            out.push((key, s..e));
            at = e;
        }
        out
    }

    fn palette(&self, b: &Block) -> Vec<[u8; 3]> {
        let (info, rest) = self.first_chunk(b);
        let count = self.r.u32(info.start).unwrap_or(0).min(65_536) as usize;
        let mut out = Vec::with_capacity(count.min(256));
        for i in 0..count {
            let at = rest + i * 4;
            if at + 4 > b.body.end {
                break;
            }
            // RGBQUAD as Windows stores it: blue, green, red, reserved.
            out.push([self.d[at + 2], self.d[at + 1], self.d[at]]);
        }
        out
    }

    fn channel(&self, b: &Block) -> Option<Channel> {
        let (info, rest) = self.first_chunk(b);
        let clen = self.r.u32(info.start)? as usize;
        let dib = self.r.u16(info.start + 8)?;
        let kind = self.r.u16(info.start + 10)?;
        let e = rest.saturating_add(clen).min(b.body.end);
        Some(Channel {
            data: rest.min(e)..e,
            dib,
            kind,
        })
    }

    /// Channel and palette sub-blocks from `at` to the end of `b`.
    fn channels(&self, at: usize, end: usize) -> (Option<Vec<[u8; 3]>>, Vec<Channel>) {
        let mut pal = None;
        let mut chans = Vec::new();
        for sb in self.blocks(at, end) {
            match sb.id {
                COLOR => pal = Some(self.palette(&sb)),
                CHANNEL => chans.extend(self.channel(&sb)),
                _ => {}
            }
        }
        (pal, chans)
    }

    fn compression(v: u16) -> Compression {
        match v {
            1 => Compression::Rle,
            2 => Compression::Lz77,
            3 => Compression::Jpeg,
            _ => Compression::None,
        }
    }

    /// The composite image bank (v4+).
    fn bank(&self, b: &Block) -> Vec<Stored> {
        let (info, rest) = self.first_chunk(b);
        let count = self.r.u32(info.start).unwrap_or(0).min(64) as usize;
        let subs = self.blocks(rest, b.body.end);
        let mut attrs = Vec::new();
        let mut images = Vec::new();
        for sb in subs {
            match sb.id {
                COMPOSITE_ATTR => attrs.push(sb),
                COMPOSITE_OR_THUMB | JPEG => images.push(sb),
                _ => {}
            }
        }
        let mut out = Vec::new();
        for (i, a) in attrs
            .iter()
            .take(count.max(attrs.len().min(64)))
            .enumerate()
        {
            let (ai, _) = self.first_chunk(a);
            let s = ai.start;
            let (Some(w), Some(h), Some(bd), Some(comp)) = (
                self.r.i32(s),
                self.r.i32(s + 4),
                self.r.u16(s + 8),
                self.r.u16(s + 10),
            ) else {
                continue;
            };
            let kind = self.r.u16(s + 18).unwrap_or(0);
            let body = match images.get(i) {
                Some(im) if im.id == JPEG => {
                    let (ji, jrest) = self.first_chunk(im);
                    let clen = self.r.u32(ji.start).unwrap_or(0) as usize;
                    let e = jrest.saturating_add(clen).min(im.body.end);
                    StoredBody::Jpeg(jrest.min(e)..e)
                }
                Some(im) => {
                    let (_, crest) = self.first_chunk(im);
                    let (palette, channels) = self.channels(crest, im.body.end);
                    StoredBody::Channels { palette, channels }
                }
                None => StoredBody::Missing,
            };
            if w <= 0 || h <= 0 {
                continue;
            }
            out.push(Stored {
                width: w as u32,
                height: h as u32,
                bit_depth: bd,
                compression: Self::compression(comp),
                kind,
                body,
            });
        }
        out
    }

    /// PSP 5's thumbnail block.
    fn v3_thumbnail(&self, b: &Block) -> Option<Stored> {
        let (info, rest) = self.first_chunk(b);
        let s = info.start;
        let w = self.r.i32(s)?;
        let h = self.r.i32(s + 4)?;
        let bd = self.r.u16(s + 8)?;
        let comp = self.r.u16(s + 10)?;
        if w <= 0 || h <= 0 {
            return None;
        }
        let (palette, channels) = self.channels(rest, b.body.end);
        Some(Stored {
            width: w as u32,
            height: h as u32,
            bit_depth: bd,
            compression: Self::compression(comp),
            kind: 1,
            body: StoredBody::Channels { palette, channels },
        })
    }

    fn rect(&self, at: usize) -> Option<(i32, i32, i32, i32)> {
        Some((
            self.r.i32(at)?,
            self.r.i32(at + 4)?,
            self.r.i32(at + 8)?,
            self.r.i32(at + 12)?,
        ))
    }

    fn layer(&self, b: &Block) -> Option<LayerRec> {
        let (info, mut rest) = self.first_chunk(b);
        let s = info.start;
        let (name, q) = if self.major <= 3 {
            let raw = self.d.get(s..s + 256)?;
            let n = raw.iter().position(|&c| c == 0).unwrap_or(256);
            (latin1(&raw[..n]), s + 256)
        } else {
            let n = self.r.u16(s)? as usize;
            let raw = self.d.get(s + 2..(s + 2 + n).min(info.end))?;
            let raw = raw.split(|&c| c == 0).next().unwrap_or(raw);
            (latin1(raw), s + 2 + n)
        };
        let typ = self.r.u8(q)?;
        let img = self.rect(q + 1)?;
        let saved = self.rect(q + 17)?;
        let opacity = self.r.u8(q + 33)?;
        let blend = BlendMode::from_u8(self.r.u8(q + 34)?);
        let flags = self.r.u8(q + 35)?;
        let mask = self.rect(q + 38).unwrap_or((0, 0, 0, 0));
        let saved_mask = self.rect(q + 54).unwrap_or((0, 0, 0, 0));
        let mask_disabled = self.r.u8(q + 71).unwrap_or(0) != 0;
        let mask_invert = self.r.u8(q + 72).unwrap_or(0) != 0;
        let (visible, kind) = if self.major <= 3 {
            (
                flags != 0,
                match typ {
                    0 | 1 => LayerKind::Raster,
                    n => LayerKind::Other(n.into()),
                },
            )
        } else {
            (
                flags & 1 != 0,
                match typ {
                    1 | 2 => LayerKind::Raster,
                    3 => LayerKind::Vector,
                    4 => LayerKind::Adjustment,
                    5 => LayerKind::Group,
                    6 => LayerKind::Mask,
                    7 => LayerKind::ArtMedia,
                    n => LayerKind::Other(n.into()),
                },
            )
        };
        // Extension sub-blocks and channels follow (v4+ with the layer bitmap chunk between).
        let mut channels = Vec::new();
        let mut group_count = None;
        let mut vector = None;
        let mut guard = 0;
        while rest + 4 <= b.body.end && guard < MAX_BLOCKS {
            guard += 1;
            if let Some(sb) = self.block(rest, b.body.end) {
                match sb.id {
                    CHANNEL => channels.extend(self.channel(&sb)),
                    GROUP_EXT => {
                        let (gi, _) = self.first_chunk(&sb);
                        group_count = self.r.u32(gi.start);
                    }
                    VECTOR_EXT if kind == LayerKind::Vector && self.major >= 4 => {
                        vector = Some(crate::vector::read(self.d, sb.body.start, sb.body.end));
                    }
                    _ => {}
                }
                rest = sb.body.end;
                if sb.cut {
                    break;
                }
            } else if self.major <= 3 {
                break;
            } else {
                // The layer bitmap chunk (bitmap and channel counts).
                let (_, e) = self.chunk_at(rest, b.body.end);
                if e <= rest {
                    break;
                }
                rest = e;
            }
        }
        if kind == LayerKind::Group && group_count.is_none() {
            group_count = Some(0);
        }
        let (ix, iy) = (img.0, img.1);
        let sw = saved.2.saturating_sub(saved.0).max(0) as u32;
        let sh = saved.3.saturating_sub(saved.1).max(0) as u32;
        let rect = (
            ix.saturating_add(saved.0),
            iy.saturating_add(saved.1),
            sw,
            sh,
        );
        let mw = saved_mask.2.saturating_sub(saved_mask.0).max(0) as u32;
        let mh = saved_mask.3.saturating_sub(saved_mask.1).max(0) as u32;
        let mask_rect = (
            mask.0.saturating_add(saved_mask.0),
            mask.1.saturating_add(saved_mask.1),
            mw,
            mh,
        );
        let has_colour = channels.iter().any(|c| c.dib == 0);
        let has_mask = channels.iter().any(|c| c.dib == 2) && mw > 0 && mh > 0;
        let drawable = matches!(
            kind,
            LayerKind::Raster | LayerKind::ArtMedia | LayerKind::Mask
        );
        let has_pixels = drawable && has_colour && sw > 0 && sh > 0;
        let vector = vector.filter(|v: &crate::vector::Shapes| !v.list.is_empty() || v.text);
        Some(LayerRec {
            vector,
            public: Layer {
                name,
                kind,
                visible,
                opacity,
                blend,
                rect,
                has_mask,
                has_pixels,
                depth: 0,
            },
            mask_rect,
            mask_disabled,
            mask_invert,
            channels,
            group_count,
        })
    }
}

/// Layer names are Windows ANSI text; Latin-1 is close enough (and never fails).
fn latin1(b: &[u8]) -> String {
    b.iter().map(|&c| c as char).collect()
}

/// Builds the group tree from the file-order list: a group layer is followed by its direct
/// children (its count), each child group by its own.
fn build_tree(recs: &mut [LayerRec]) -> Vec<Node> {
    fn take(recs: &mut [LayerRec], at: &mut usize, n: usize, depth: u32, out: &mut Vec<Node>) {
        let mut k = 0;
        while k < n && *at < recs.len() {
            let i = *at;
            *at += 1;
            recs[i].public.depth = depth;
            let mut node = Node {
                rec: i,
                children: Vec::new(),
            };
            if let Some(c) = recs[i].group_count
                && depth < 64
            {
                take(recs, at, c as usize, depth + 1, &mut node.children);
            }
            out.push(node);
            k += 1;
        }
    }
    let mut out = Vec::new();
    let mut at = 0;
    let n = recs.len();
    take(recs, &mut at, n, 0, &mut out);
    out
}

/// Top-most first: siblings reversed, each group before its own layers.
fn flatten(nodes: &[Node], out: &mut Vec<usize>) {
    for n in nodes.iter().rev() {
        out.push(n.rec);
        flatten(&n.children, out);
    }
}

pub(crate) fn parse(d: &[u8]) -> Result<Psp, Error> {
    if !crate::probe(d) {
        return Err(Error::NotPsp);
    }
    let r = Rd(d);
    let major = r.u16(32).ok_or_else(|| trunc("file header"))?;
    let minor = r.u16(34).ok_or_else(|| trunc("file header"))?;
    if major < 3 {
        return Err(Error::Unsupported(format!(
            "PaintShop Pro file version {major} (before Paint Shop Pro 5)"
        )));
    }
    let p = Parser { d, r, major };
    let top = p.blocks(36, d.len());
    let first = top
        .iter()
        .find(|b| b.id == IMAGE)
        .ok_or_else(|| trunc("image attributes block"))?;
    let (gi, _) = p.first_chunk(first);
    let s = gi.start;
    let w = p.r.i32(s).ok_or_else(|| trunc("image attributes"))?;
    let h = p.r.i32(s + 4).ok_or_else(|| trunc("image attributes"))?;
    let res = p.r.f64(s + 8).unwrap_or(0.0);
    let metric = p.r.u8(s + 16).unwrap_or(0);
    let comp = p.r.u16(s + 17).ok_or_else(|| trunc("image attributes"))?;
    let bd = p.r.u16(s + 19).ok_or_else(|| trunc("image attributes"))?;
    let grey = p.r.u8(s + 27).unwrap_or(0) != 0;
    if w <= 0 || h <= 0 {
        return Err(Error::Malformed(format!("image size {w}×{h}")));
    }
    let (w, h) = (w as u32, h as u32);
    if w > MAX_DIMENSION || h > MAX_DIMENSION || u64::from(w) * u64::from(h) > MAX_PIXELS {
        return Err(Error::TooLarge);
    }
    let compression = match comp {
        0 => Compression::None,
        1 => Compression::Rle,
        2 => Compression::Lz77,
        n => {
            return Err(Error::Unsupported(format!(
                "PaintShop Pro layer compression {n}"
            )));
        }
    };
    if !matches!(bd, 1 | 4 | 8 | 16 | 24 | 48) {
        return Err(Error::Unsupported(format!(
            "PaintShop Pro image with {bd} bits per pixel"
        )));
    }
    let dpi = match metric {
        1 if res > 0.0 && res.is_finite() => Some(res),
        2 if res > 0.0 && res.is_finite() => Some(res * 2.54),
        _ => None,
    };

    let mut palette = None;
    let mut stored = Vec::new();
    let mut recs = Vec::new();
    let mut creator = None;
    let mut tube = false;
    for b in &top {
        match b.id {
            COLOR => palette = Some(p.palette(b)),
            CREATOR => {
                let f = p.fields(&b.body);
                let app = f
                    .iter()
                    .find(|(k, _)| *k == 6)
                    .and_then(|(_, v)| p.r.u32(v.start));
                let ver = f
                    .iter()
                    .find(|(k, _)| *k == 7)
                    .and_then(|(_, v)| p.r.u32(v.start));
                if app == Some(1) {
                    creator = Some(match ver {
                        Some(v) if v >> 24 != 0 => {
                            format!("Paint Shop Pro {}.{:02}", v >> 24, (v >> 16) & 0xff)
                        }
                        _ => "Paint Shop Pro".to_string(),
                    });
                }
            }
            TUBE => tube = true,
            COMPOSITE_BANK if major >= 4 => stored.extend(p.bank(b)),
            COMPOSITE_OR_THUMB if major <= 3 => stored.extend(p.v3_thumbnail(b)),
            LAYER_START => {
                let (_, rest) = if major <= 3 {
                    (0..0, b.body.start + b.init as usize)
                } else {
                    // The layer bank has no chunk of its own in practice, but a writer may put
                    // one: skip anything before the first block.
                    let at = d[b.body.clone()]
                        .windows(4)
                        .position(|x| x == BLOCK_SIG)
                        .map_or(b.body.end, |o| b.body.start + o);
                    (0..0, at)
                };
                for lb in p.blocks(rest.min(b.body.end), b.body.end) {
                    if lb.id != LAYER {
                        continue;
                    }
                    if recs.len() >= MAX_LAYERS {
                        return Err(Error::Unsupported(format!(
                            "PaintShop Pro file with more than {MAX_LAYERS} layers"
                        )));
                    }
                    if let Some(l) = p.layer(&lb) {
                        recs.push(l);
                    }
                }
            }
            _ => {}
        }
    }

    let tree = build_tree(&mut recs);
    let mut order = Vec::with_capacity(recs.len());
    flatten(&tree, &mut order);
    let layers: Vec<Layer> = order.iter().map(|&i| recs[i].public.clone()).collect();

    let mut skipped = Vec::new();
    for rec in order.iter().map(|&i| &recs[i]) {
        let l = &rec.public;
        if !l.visible {
            continue;
        }
        let what = match l.kind {
            LayerKind::Vector if rec.vector.as_ref().is_some_and(|v| v.text) => "vector text",
            LayerKind::Vector if rec.vector.is_some() => continue,
            LayerKind::Vector => "vector",
            LayerKind::Adjustment => "adjustment",
            LayerKind::ArtMedia if !l.has_pixels => "art media",
            LayerKind::Other(_) => "unknown",
            _ => continue,
        };
        if !skipped.contains(&what) {
            skipped.push(what);
        }
    }
    let not_drawn = (!skipped.is_empty()).then(|| {
        let list = match skipped.len() {
            1 => skipped[0].to_string(),
            n => format!("{} and {}", skipped[..n - 1].join(", "), skipped[n - 1]),
        };
        format!("{list} layers are not drawn")
    });
    let has_composite = stored.iter().any(|s| {
        s.kind == 0 && s.width == w && s.height == h && !matches!(s.body, StoredBody::Missing)
    });
    let has_thumbnail = stored
        .iter()
        .any(|s| s.kind == 1 && !matches!(s.body, StoredBody::Missing));
    let info = Info {
        width: w,
        height: h,
        major,
        minor,
        bit_depth: bd,
        greyscale: grey,
        palette: bd <= 8 && !grey,
        compression,
        dpi,
        layer_count: recs.len(),
        has_composite,
        has_thumbnail,
        not_drawn,
        creator,
        tube,
    };
    Ok(Psp {
        info,
        recs,
        tree,
        order,
        layers,
        palette,
        stored,
    })
}
