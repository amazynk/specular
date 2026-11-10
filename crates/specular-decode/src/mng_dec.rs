//! MNG (Multiple-image Network Graphics) and JNG (JPEG Network Graphics), in-tree: the chunk
//! stream is walked here, embedded PNG images go through `specular-png` and JNG's JPEG data
//! through `specular-jpeg`.
//!
//! MNG coverage is the common subset that animations in the wild use: `MHDR`, embedded PNG
//! (`IHDR`…`IEND`, with the global `PLTE`/`tRNS` for an empty `PLTE`) and JNG (`JHDR`…`IEND`)
//! layers placed by `DEFI`, `BACK`, `FRAM` framing modes 1–4 and interframe delays, `LOOP` /
//! `ENDL` loops and `TERM` repetition. Delta images (`DHDR`), `BASI`, `PAST`, `CLON`, `MAGN` and
//! object reuse are not interpreted (their layers are skipped). Each frame is the whole
//! composited canvas.
//!
//! JNG: 8-bit JPEG colour or grey (the 8-bit part of a 12-bit `JSEP` file), alpha from PNG
//! `IDAT` (1–16 bits) or JPEG `JDAA`.
//!
//! Robustness: hostile chunk tables, loop counts and sizes are bounded ([`MAX_FRAMES`],
//! [`MAX_STEPS`], [`MAX_PIXELS`]); nothing panics.

use crate::whole::{self, Levels};
use crate::{
    Alpha, AnimationInfo, DecodeError, DecodeRequest, Decoder, Format, Frame, Frames, ImageInfo,
    Result, Session,
};
use specular_core::{CancelToken, Orientation, PixelLayout, Raster, Source};
use std::ops::Range;
use std::sync::Arc;
use std::time::Duration;

pub const MNG_SIGNATURE: [u8; 8] = [0x8A, b'M', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
pub const JNG_SIGNATURE: [u8; 8] = [0x8B, b'J', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

/// Most frames played (loops beyond are cut).
pub const MAX_FRAMES: u32 = 100_000;
/// Most chunk steps taken while counting frames (bounds `LOOP` nests with huge counts).
pub const MAX_STEPS: u64 = 4_000_000;
/// Largest canvas or layer.
pub const MAX_PIXELS: u64 = 1 << 28;
const MAX_SIDE: u32 = 1 << 16;
/// Deepest `LOOP` nest followed.
const MAX_NEST: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Mng,
    Jng,
}

pub struct MngDecoder(pub Kind);

#[derive(Clone, Debug)]
struct Chunk {
    typ: [u8; 4],
    /// Data.
    data: Range<usize>,
    /// The whole chunk (length, type, data, CRC), clamped to the file.
    whole: Range<usize>,
}

/// The chunks after the signature; a cut-short chunk ends the list (its data clamped).
fn chunks(d: &[u8]) -> Vec<Chunk> {
    let mut out = Vec::new();
    let mut at = 8usize;
    while at + 8 <= d.len() {
        let len = u32::from_be_bytes(d[at..at + 4].try_into().unwrap_or_default()) as usize;
        let typ: [u8; 4] = d[at + 4..at + 8].try_into().unwrap_or_default();
        let start = at + 8;
        let end = start.saturating_add(len);
        let whole_end = end.saturating_add(4).min(d.len());
        out.push(Chunk {
            typ,
            data: start..end.min(d.len()),
            whole: at..whole_end,
        });
        if end > d.len() || &typ == b"MEND" {
            break;
        }
        at = end + 4;
    }
    out
}

fn be32(d: &[u8], p: usize) -> Option<u32> {
    Some(u32::from_be_bytes(d.get(p..p + 4)?.try_into().ok()?))
}

fn be16(d: &[u8], p: usize) -> Option<u16> {
    Some(u16::from_be_bytes(d.get(p..p + 2)?.try_into().ok()?))
}

fn size_ok(w: u32, h: u32) -> bool {
    w > 0 && h > 0 && w <= MAX_SIDE && h <= MAX_SIDE && u64::from(w) * u64::from(h) <= MAX_PIXELS
}

/// An RGBA8 image (straight alpha).
pub(crate) struct Rgba {
    pub(crate) w: u32,
    pub(crate) h: u32,
    pub(crate) px: Vec<u8>,
}

fn png_err(e: specular_png::PngError) -> DecodeError {
    match e {
        specular_png::PngError::Cancelled => DecodeError::Cancelled,
        e => DecodeError::Malformed(format!("embedded PNG: {e}")),
    }
}

fn jpeg_err(e: specular_jpeg::JpegError) -> DecodeError {
    match e {
        specular_jpeg::JpegError::Cancelled => DecodeError::Cancelled,
        e => DecodeError::Malformed(format!("JNG JPEG data: {e}")),
    }
}

pub(crate) fn decode_png(png: &[u8], cancel: &CancelToken) -> Result<Rgba> {
    let img = specular_png::decode(
        png,
        &specular_png::DecodeOptions {
            output: specular_png::Output::Rgba8,
            cancel: Some(cancel.flag()),
            ..Default::default()
        },
    )
    .map_err(png_err)?;
    let p = &img.planes[0];
    let row = img.width as usize * 4;
    let px = if p.stride == row {
        p.data.clone()
    } else {
        p.data
            .chunks(p.stride)
            .take(img.height as usize)
            .flat_map(|r| &r[..row])
            .copied()
            .collect()
    };
    Ok(Rgba {
        w: img.width,
        h: img.height,
        px,
    })
}

pub(crate) fn decode_jpeg(jpeg: &[u8], cancel: &CancelToken) -> Result<Rgba> {
    let planes = specular_jpeg::decode(
        jpeg,
        &specular_jpeg::DecodeOptions {
            cancel: Some(cancel.flag()),
            ..Default::default()
        },
    )
    .map_err(jpeg_err)?;
    Ok(Rgba {
        w: planes.width,
        h: planes.height,
        px: specular_jpeg::planes_to_rgba(&planes),
    })
}

/// Rebuilds a stand-alone PNG from an embedded image's chunks (`IHDR`…`IEND`), giving an
/// empty `PLTE` the global palette (and its `tRNS` when the image has none).
fn embedded_png(
    d: &[u8],
    cs: &[Chunk],
    global_plte: Option<&Chunk>,
    global_trns: Option<&Chunk>,
) -> Vec<u8> {
    let mut png = PNG_SIGNATURE.to_vec();
    let has_trns = cs.iter().any(|c| &c.typ == b"tRNS");
    for c in cs {
        if &c.typ == b"PLTE" && c.data.is_empty() {
            if let Some(g) = global_plte {
                png.extend_from_slice(&d[g.whole.clone()]);
                if let (false, Some(t)) = (has_trns, global_trns) {
                    png.extend_from_slice(&d[t.whole.clone()]);
                }
            }
            continue;
        }
        png.extend_from_slice(&d[c.whole.clone()]);
    }
    png
}

fn chunk(typ: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut c = (data.len() as u32).to_be_bytes().to_vec();
    c.extend_from_slice(typ);
    c.extend_from_slice(data);
    c.extend_from_slice(&crc32(&c[4..]).to_be_bytes());
    c
}

/// The PNG chunk CRC (bitwise; only for the few small chunks written here).
fn crc32(d: &[u8]) -> u32 {
    let mut c = !0u32;
    for &b in d {
        c ^= u32::from(b);
        for _ in 0..8 {
            c = if c & 1 != 0 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
        }
    }
    !c
}

/// JNG header facts.
#[derive(Clone, Copy, Debug)]
struct Jhdr {
    w: u32,
    h: u32,
    colour: u8,
    depth: u8,
    alpha_depth: u8,
    alpha_jpeg: bool,
    alpha_interlace: u8,
}

fn jhdr(d: &[u8]) -> Result<Jhdr> {
    if d.len() < 16 {
        return Err(DecodeError::Truncated("JHDR".into()));
    }
    let j = Jhdr {
        w: be32(d, 0).unwrap_or(0),
        h: be32(d, 4).unwrap_or(0),
        colour: d[8],
        depth: d[9],
        alpha_depth: d[12],
        alpha_jpeg: d[13] == 8,
        alpha_interlace: d[15],
    };
    if !size_ok(j.w, j.h) {
        return Err(DecodeError::TooLarge);
    }
    if !matches!(j.colour, 8 | 10 | 12 | 14) || !matches!(j.depth, 8 | 12 | 20) {
        return Err(DecodeError::Malformed("JHDR colour type or depth".into()));
    }
    if j.depth == 12 {
        return Err(DecodeError::Unsupported("12-bit JNG".into()));
    }
    Ok(j)
}

/// Decodes a JNG image from its chunks (`JHDR`…`IEND`).
fn decode_jng(d: &[u8], cs: &[Chunk], cancel: &CancelToken) -> Result<Rgba> {
    let head = cs
        .first()
        .filter(|c| &c.typ == b"JHDR")
        .ok_or_else(|| DecodeError::Malformed("no JHDR".into()))?;
    let j = jhdr(&d[head.data.clone()])?;
    let mut jpeg = Vec::new();
    let mut alpha_jpeg = Vec::new();
    let mut idat = Vec::new();
    let mut sep = false;
    for c in cs {
        match &c.typ {
            b"JDAT" if !sep => jpeg.extend_from_slice(&d[c.data.clone()]),
            b"JSEP" => sep = true,
            b"JDAA" => alpha_jpeg.extend_from_slice(&d[c.data.clone()]),
            b"IDAT" => idat.push(c.whole.clone()),
            _ => {}
        }
    }
    let mut img = decode_jpeg(&jpeg, cancel)?;
    if (img.w, img.h) != (j.w, j.h) {
        return Err(DecodeError::Malformed(
            "JNG JPEG size differs from JHDR".into(),
        ));
    }
    let has_alpha = matches!(j.colour, 12 | 14) && j.alpha_depth > 0;
    if !has_alpha {
        return Ok(img);
    }
    let alpha = if j.alpha_jpeg {
        decode_jpeg(&alpha_jpeg, cancel).ok()
    } else if !idat.is_empty() && matches!(j.alpha_depth, 1 | 2 | 4 | 8 | 16) {
        let mut ihdr = Vec::with_capacity(13);
        ihdr.extend_from_slice(&j.w.to_be_bytes());
        ihdr.extend_from_slice(&j.h.to_be_bytes());
        ihdr.extend_from_slice(&[j.alpha_depth, 0, 0, 0, u8::from(j.alpha_interlace == 1)]);
        let mut png = PNG_SIGNATURE.to_vec();
        png.extend_from_slice(&chunk(b"IHDR", &ihdr));
        for r in idat {
            png.extend_from_slice(&d[r]);
        }
        png.extend_from_slice(&chunk(b"IEND", &[]));
        decode_png(&png, cancel).ok()
    } else {
        None
    };
    // A missing or broken alpha channel leaves the image opaque.
    if let Some(a) = alpha.filter(|a| (a.w, a.h) == (img.w, img.h)) {
        for (p, a) in img.px.chunks_exact_mut(4).zip(a.px.chunks_exact(4)) {
            p[3] = a[0];
        }
    }
    Ok(img)
}

/// What the stream does next.
#[derive(Debug)]
enum Step {
    /// A layer: the chunks of an embedded PNG or JNG image, drawn at `pos` unless hidden.
    Layer {
        chunks: Range<usize>,
        jng: bool,
        pos: (i32, i32),
        show: bool,
    },
    /// A `FRAM` chunk: new framing mode (0 = unchanged) and interframe delay in ticks.
    Fram {
        mode: u8,
        delay: Option<u32>,
    },
    End,
}

/// Walks the chunk list: `LOOP`/`ENDL` repetition, `DEFI` positions, layer boundaries.
#[derive(Clone)]
struct Walker {
    pc: usize,
    /// `(chunk after LOOP, nest level, iterations left)`.
    loops: Vec<(usize, u8, u32)>,
    pos: (i32, i32),
    show: bool,
    steps: u64,
}

impl Walker {
    fn new() -> Walker {
        Walker {
            pc: 0,
            loops: Vec::new(),
            pos: (0, 0),
            show: true,
            steps: 0,
        }
    }

    fn next(&mut self, d: &[u8], cs: &[Chunk]) -> Step {
        loop {
            self.steps += 1;
            if self.steps > MAX_STEPS {
                return Step::End;
            }
            let Some(c) = cs.get(self.pc) else {
                return Step::End;
            };
            let data = &d[c.data.clone()];
            self.pc += 1;
            match &c.typ {
                b"MEND" => return Step::End,
                b"IHDR" | b"JHDR" => {
                    let start = self.pc - 1;
                    while self.pc < cs.len() && &cs[self.pc].typ != b"IEND" {
                        self.pc += 1;
                    }
                    self.pc = (self.pc + 1).min(cs.len());
                    return Step::Layer {
                        chunks: start..self.pc,
                        jng: &c.typ == b"JHDR",
                        pos: self.pos,
                        show: self.show,
                    };
                }
                // Delta images and basis objects: skip to their end.
                b"DHDR" | b"BASI" => {
                    while self.pc < cs.len() && &cs[self.pc].typ != b"IEND" {
                        self.pc += 1;
                    }
                    self.pc = (self.pc + 1).min(cs.len());
                }
                b"DEFI" => {
                    self.show = data.get(2).is_none_or(|&v| v != 1);
                    self.pos = match (be32(data, 4), be32(data, 8)) {
                        (Some(x), Some(y)) => (x as i32, y as i32),
                        _ => (0, 0),
                    };
                }
                b"FRAM" => {
                    let mode = data.first().copied().unwrap_or(0);
                    // The subframe name ends at a NUL; the change flags follow.
                    let delay = data.get(1..).and_then(|rest| {
                        let name = rest.iter().position(|&b| b == 0)?;
                        let flags = rest.get(name + 1..name + 5)?;
                        if flags[0] == 0 {
                            return None;
                        }
                        be32(rest, name + 5)
                    });
                    return Step::Fram { mode, delay };
                }
                b"LOOP" => {
                    let level = data.first().copied().unwrap_or(0);
                    let count = be32(data, 1).unwrap_or(1);
                    if count == 0 {
                        // Skip to the matching ENDL.
                        while let Some(e) = cs.get(self.pc) {
                            self.pc += 1;
                            if &e.typ == b"ENDL" && d.get(e.data.start) == Some(&level) {
                                break;
                            }
                        }
                    } else if self.loops.len() < MAX_NEST {
                        self.loops.push((self.pc, level, count));
                    }
                }
                b"ENDL" => {
                    let level = data.first().copied().unwrap_or(0);
                    // Unwind to the loop of this level (ignore an ENDL without one).
                    if let Some(i) = self.loops.iter().rposition(|l| l.1 == level) {
                        self.loops.truncate(i + 1);
                        let l = &mut self.loops[i];
                        if l.2 > 1 {
                            l.2 -= 1;
                            self.pc = l.0;
                        } else {
                            self.loops.pop();
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

/// The parsed MNG or JNG stream.
struct Stream {
    kind: Kind,
    width: u32,
    height: u32,
    ticks: u32,
    chunks: Vec<Chunk>,
    plte: Option<usize>,
    trns: Option<usize>,
    /// Opaque background from `BACK`; transparent otherwise.
    background: Option<[u8; 3]>,
    /// `TERM` repetition (0 = forever), 1 without.
    loop_count: u32,
    /// Frames counted by a dry run (capped by [`MAX_FRAMES`]); `None` when the cap or the
    /// step budget was reached.
    frames: Option<u32>,
    layers: u32,
    any_jng: bool,
    skipped: bool,
}

fn parse(kind: Kind, d: &[u8]) -> Result<Stream> {
    let sig = match kind {
        Kind::Mng => MNG_SIGNATURE,
        Kind::Jng => JNG_SIGNATURE,
    };
    if d.len() < 8 || d[..8] != sig {
        return Err(DecodeError::UnsupportedFormat);
    }
    let chunks = chunks(d);
    let first = chunks
        .first()
        .ok_or_else(|| DecodeError::Truncated("header chunk".into()))?;
    let mut s = Stream {
        kind,
        width: 0,
        height: 0,
        ticks: 1000,
        plte: None,
        trns: None,
        background: None,
        loop_count: 1,
        frames: Some(1),
        layers: 1,
        any_jng: kind == Kind::Jng,
        skipped: false,
        chunks: Vec::new(),
    };
    match kind {
        Kind::Jng => {
            let j = jhdr(&d[first.data.clone()])?;
            (s.width, s.height) = (j.w, j.h);
            s.chunks = chunks;
            return Ok(s);
        }
        Kind::Mng => {
            let h = &d[first.data.clone()];
            if &first.typ != b"MHDR" || h.len() < 12 {
                return Err(DecodeError::Truncated("MHDR".into()));
            }
            s.width = be32(h, 0).unwrap_or(0);
            s.height = be32(h, 4).unwrap_or(0);
            s.ticks = be32(h, 8).unwrap_or(0);
        }
    }
    for (i, c) in chunks.iter().enumerate() {
        let data = &d[c.data.clone()];
        match &c.typ {
            b"PLTE" if s.plte.is_none() && !data.is_empty() => s.plte = Some(i),
            b"tRNS" if s.trns.is_none() => s.trns = Some(i),
            // The first top-level `BACK` (they are not inside images).
            b"BACK" if s.background.is_none() && data.len() >= 6 => {
                let c = |k| (be16(data, k).unwrap_or(0) >> 8) as u8;
                s.background = Some([c(0), c(2), c(4)]);
            }
            b"TERM" if data.len() >= 10 && data[0] == 3 => {
                let max = be32(data, 6).unwrap_or(0);
                s.loop_count = if max >= 0x7fff_ffff { 0 } else { max.max(1) };
            }
            b"DHDR" | b"BASI" | b"PAST" | b"CLON" | b"MAGN" => s.skipped = true,
            b"JHDR" => s.any_jng = true,
            _ => {}
        }
    }
    // Global `PLTE`/`tRNS` are the ones outside images.
    let mut inside = false;
    let (mut plte, mut trns) = (None, None);
    for (i, c) in chunks.iter().enumerate() {
        match &c.typ {
            b"IHDR" | b"JHDR" | b"DHDR" | b"BASI" => inside = true,
            b"IEND" => inside = false,
            b"PLTE" if !inside && plte.is_none() => plte = Some(i),
            b"tRNS" if !inside && trns.is_none() => trns = Some(i),
            _ => {}
        }
    }
    (s.plte, s.trns) = (plte, trns);
    s.chunks = chunks;
    // The first layer gives the size when MHDR does not.
    let mut w = Walker::new();
    let (mut frames, mut layers, mut pending, mut mode) = (0u32, 0u32, false, 1u8);
    let capped = loop {
        match w.next(d, &s.chunks) {
            Step::Layer { chunks, show, .. } => {
                layers = layers.saturating_add(1);
                if s.width == 0 || s.height == 0 {
                    let h = &d[s.chunks[chunks.start].data.clone()];
                    s.width = be32(h, 0).unwrap_or(0);
                    s.height = be32(h, 4).unwrap_or(0);
                }
                if !show {
                    continue;
                }
                if matches!(mode, 1 | 3) {
                    frames += 1;
                } else {
                    pending = true;
                }
            }
            Step::Fram { mode: m, .. } => {
                if matches!(mode, 2 | 4) && pending {
                    frames += 1;
                    pending = false;
                }
                if (1..=4).contains(&m) {
                    mode = m;
                }
            }
            Step::End => {
                frames += u32::from(pending);
                break w.steps > MAX_STEPS;
            }
        }
        if frames >= MAX_FRAMES {
            break true;
        }
    };
    if !size_ok(s.width, s.height) {
        return Err(DecodeError::TooLarge);
    }
    if frames == 0 && !capped {
        return Err(DecodeError::Malformed("no images".into()));
    }
    s.frames = (!capped).then_some(frames);
    s.layers = layers;
    Ok(s)
}

/// Plays a [`Stream`] on a canvas.
struct Player {
    source: Arc<Source>,
    stream: Arc<Stream>,
    walker: Walker,
    canvas: Vec<u8>,
    mode: u8,
    /// Default and next-subframe delays in ticks.
    delay: u32,
    started: bool,
    index: u32,
    pending: bool,
    done: bool,
}

impl Player {
    fn new(source: Arc<Source>, stream: Arc<Stream>) -> Player {
        let n = stream.width as usize * stream.height as usize;
        let mut p = Player {
            source,
            stream,
            walker: Walker::new(),
            canvas: vec![0; n * 4],
            mode: 1,
            delay: 1,
            started: false,
            index: 0,
            pending: false,
            done: false,
        };
        p.clear();
        p
    }

    fn clear(&mut self) {
        let px = match self.stream.background {
            Some([r, g, b]) => [r, g, b, 255],
            None => [0; 4],
        };
        for p in self.canvas.chunks_exact_mut(4) {
            p.copy_from_slice(&px);
        }
    }

    fn draw(&mut self, img: &Rgba, (x0, y0): (i32, i32)) {
        let (cw, ch) = (self.stream.width as i64, self.stream.height as i64);
        for y in 0..img.h as i64 {
            let cy = y + i64::from(y0);
            if !(0..ch).contains(&cy) {
                continue;
            }
            for x in 0..img.w as i64 {
                let cx = x + i64::from(x0);
                if !(0..cw).contains(&cx) {
                    continue;
                }
                let s = &img.px[(y * img.w as i64 + x) as usize * 4..][..4];
                let o = (cy * cw + cx) as usize * 4;
                let dst = &mut self.canvas[o..o + 4];
                match s[3] {
                    255 => dst.copy_from_slice(s),
                    0 => {}
                    a => {
                        // Straight-alpha "over".
                        let (sa, da) = (u32::from(a), u32::from(dst[3]));
                        let oa = sa * 255 + da * (255 - sa);
                        for k in 0..3 {
                            let v = (u32::from(s[k]) * sa * 255
                                + u32::from(dst[k]) * da * (255 - sa))
                                / oa.max(1);
                            dst[k] = v.min(255) as u8;
                        }
                        dst[3] = (oa / 255) as u8;
                    }
                }
            }
        }
    }

    fn layer(&self, chunks: Range<usize>, jng: bool, cancel: &CancelToken) -> Result<Rgba> {
        let d = self.source.bytes();
        let s = &self.stream;
        let cs = &s.chunks[chunks];
        if jng {
            return decode_jng(d, cs, cancel);
        }
        let head = &d[cs[0].data.clone()];
        let (w, h) = (be32(head, 0).unwrap_or(0), be32(head, 4).unwrap_or(0));
        if !size_ok(w, h) {
            return Err(DecodeError::TooLarge);
        }
        let png = embedded_png(
            d,
            cs,
            s.plte.map(|i| &s.chunks[i]),
            s.trns.map(|i| &s.chunks[i]),
        );
        decode_png(&png, cancel)
    }

    fn frame(&mut self, scale: u32) -> Frame {
        let ticks = self.stream.ticks;
        let delay = if ticks == 0 {
            Duration::ZERO
        } else {
            Duration::from_secs_f64(f64::from(self.delay) / f64::from(ticks))
        };
        let (w, h) = (self.stream.width, self.stream.height);
        let mut raster = whole::interleaved(w, h, PixelLayout::Rgba, self.canvas.clone(), false);
        if scale > 1 {
            raster = whole::downscale(&raster, w, h, scale);
        }
        let f = Frame {
            index: self.index,
            raster,
            delay,
        };
        self.index += 1;
        f
    }

    fn next_frame(&mut self, scale: u32, cancel: &CancelToken) -> Option<Result<Frame>> {
        if self.done || self.index >= MAX_FRAMES {
            return None;
        }
        loop {
            if cancel.is_cancelled() {
                return Some(Err(DecodeError::Cancelled));
            }
            let step = self.walker.next(self.source.bytes(), &self.stream.chunks);
            match step {
                Step::Layer {
                    chunks,
                    jng,
                    pos,
                    show,
                } => {
                    if !show {
                        continue;
                    }
                    if self.mode == 3 || (self.mode == 4 && !self.pending) {
                        self.clear();
                    }
                    match self.layer(chunks, jng, cancel) {
                        Ok(img) => self.draw(&img, pos),
                        Err(DecodeError::Cancelled) => return Some(Err(DecodeError::Cancelled)),
                        // A broken layer is skipped, the animation goes on.
                        Err(e) => {
                            specular_log::debug!(target: specular_log::DECODE, "mng: layer: {e}");
                        }
                    }
                    self.started = true;
                    if matches!(self.mode, 1 | 3) {
                        return Some(Ok(self.frame(scale)));
                    }
                    self.pending = true;
                }
                Step::Fram { mode, delay } => {
                    let emit = matches!(self.mode, 2 | 4) && self.pending;
                    let f = emit.then(|| self.frame(scale));
                    self.pending &= !emit;
                    if (1..=4).contains(&mode) {
                        self.mode = mode;
                    }
                    if let Some(d) = delay {
                        self.delay = d;
                    }
                    if let Some(f) = f {
                        return Some(Ok(f));
                    }
                }
                Step::End => {
                    self.done = true;
                    if self.pending {
                        self.pending = false;
                        return Some(Ok(self.frame(scale)));
                    }
                    return None;
                }
            }
        }
    }
}

impl Decoder for MngDecoder {
    fn format(&self) -> Format {
        match self.0 {
            Kind::Mng => Format::Mng,
            Kind::Jng => Format::Jng,
        }
    }
    fn probe(&self, head: &[u8]) -> bool {
        let sig = match self.0 {
            Kind::Mng => MNG_SIGNATURE,
            Kind::Jng => JNG_SIGNATURE,
        };
        head.len() >= 8 && head[..8] == sig
    }
    fn extensions(&self) -> &'static [&'static str] {
        match self.0 {
            Kind::Mng => &["mng"],
            Kind::Jng => &["jng"],
        }
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let s = parse(self.0, source.bytes())?;
        let has_alpha = match s.kind {
            Kind::Jng => {
                let j = jhdr(&source.bytes()[s.chunks[0].data.clone()])?;
                matches!(j.colour, 12 | 14) && j.alpha_depth > 0
            }
            Kind::Mng => s.background.is_none(),
        };
        let coding = match s.kind {
            Kind::Jng => {
                let j = jhdr(&source.bytes()[s.chunks[0].data.clone()])?;
                let colour = if matches!(j.colour, 8 | 12) {
                    "grey"
                } else {
                    "colour"
                };
                match (has_alpha, j.alpha_jpeg) {
                    (false, _) => format!("JPEG {colour}"),
                    (true, true) => format!("JPEG {colour} + JPEG alpha"),
                    (true, false) => format!("JPEG {colour} + {}-bit alpha", j.alpha_depth),
                }
            }
            Kind::Mng => {
                let mut c = match s.frames {
                    Some(1) => "1 frame".to_string(),
                    Some(n) => format!("{n} frames"),
                    None => format!("over {MAX_FRAMES} frames"),
                };
                if s.layers > 1 {
                    c += &format!(", {} layers", s.layers);
                }
                if s.any_jng {
                    c += ", JNG layers";
                }
                if s.skipped {
                    c += ", delta images not shown";
                }
                c
            }
        };
        let animation = (s.kind == Kind::Mng && s.frames != Some(1)).then_some(AnimationInfo {
            frame_count: s.frames,
            loop_count: s.loop_count,
        });
        let info = ImageInfo {
            format: self.format(),
            width: s.width,
            height: s.height,
            orientation: Orientation::NORMAL,
            icc_profile: None,
            has_alpha,
            bit_depth: 8,
            progressive: false,
            coding,
            embedded_preview: None,
            fast_regions: false,
            scales: &[1, 2, 4, 8],
        };
        Ok(Arc::new(MngSession {
            source,
            info,
            stream: Arc::new(s),
            animation,
            levels: Levels::new(),
        }))
    }
}

struct MngSession {
    source: Arc<Source>,
    info: ImageInfo,
    stream: Arc<Stream>,
    animation: Option<AnimationInfo>,
    levels: Levels,
}

impl MngSession {
    /// The first frame (MNG) or the image (JNG), full size.
    fn first(&self, cancel: &CancelToken) -> Result<Raster> {
        let d = self.source.bytes();
        if self.stream.kind == Kind::Jng {
            let img = decode_jng(d, &self.stream.chunks, cancel)?;
            return Ok(whole::interleaved(
                img.w,
                img.h,
                PixelLayout::Rgba,
                img.px,
                false,
            ));
        }
        let mut p = Player::new(self.source.clone(), self.stream.clone());
        match p.next_frame(1, cancel) {
            Some(r) => r.map(|f| f.raster),
            None => Err(DecodeError::Malformed("no frames".into())),
        }
    }
}

impl Session for MngSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn colour_model(&self) -> Option<&'static str> {
        Some("RGB")
    }
    fn alpha(&self) -> Option<Alpha> {
        self.info.has_alpha.then_some(Alpha::Straight)
    }
    fn source(&self) -> &Arc<Source> {
        &self.source
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        None
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        self.levels.decode(
            req,
            (self.info.width, self.info.height),
            |_| 1,
            |_| self.first(cancel),
            cancel,
        )
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        let (w, h) = (self.info.width, self.info.height);
        let s = req.scale.max(1);
        let mut m = whole::level_bytes(w, h, s, 4);
        if !self.levels.has(1) {
            // The canvas, the full level and one decoded layer.
            m += 3 * whole::level_bytes(w, h, 1, 4);
        }
        m
    }
    fn memory_bytes(&self) -> usize {
        self.levels.memory_bytes()
    }
    fn trim_memory(&self) -> usize {
        self.levels.clear()
    }
    fn animation(&self) -> Option<&AnimationInfo> {
        self.animation.as_ref()
    }
    fn frames(&self, scale: u32) -> Result<Box<dyn Frames>> {
        if self.animation.is_none() {
            return Err(DecodeError::Unsupported("not animated".into()));
        }
        Ok(Box::new(MngFrames {
            player: Player::new(self.source.clone(), self.stream.clone()),
            scale: scale.clamp(1, 8),
        }))
    }
}

struct MngFrames {
    player: Player,
    scale: u32,
}

impl Frames for MngFrames {
    fn next_frame(&mut self, cancel: &CancelToken) -> Option<Result<Frame>> {
        self.player.next_frame(self.scale, cancel)
    }
    fn rewind(&mut self) -> Result<()> {
        self.player = Player::new(self.player.source.clone(), self.player.stream.clone());
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn png(w: u32, h: u32, rgba: [u8; 4]) -> Vec<u8> {
        let px: Vec<u8> = (0..w * h).flat_map(|_| rgba).collect();
        specular_png::encode(w, h, specular_png::ColorType::Rgba, &px).unwrap()
    }

    /// The chunks of a PNG file (after its signature).
    fn png_chunks(png: &[u8]) -> &[u8] {
        &png[8..]
    }

    /// An MNG: `w`×`h`, 100 ticks a second, mode-1 frames of `layers` placed at `pos`.
    pub(crate) fn mng(w: u32, h: u32, layers: &[(Vec<u8>, (i32, i32))], extra: &[u8]) -> Vec<u8> {
        let mut d = MNG_SIGNATURE.to_vec();
        let mut mhdr = Vec::new();
        for v in [w, h, 100, 0, 0, 0, 0] {
            mhdr.extend_from_slice(&v.to_be_bytes());
        }
        d.extend_from_slice(&chunk(b"MHDR", &mhdr));
        d.extend_from_slice(extra);
        // FRAM mode 1 with an interframe delay of 5 ticks.
        d.extend_from_slice(&chunk(b"FRAM", &[1, 0, 2, 0, 0, 0, 0, 0, 0, 5]));
        for (p, (x, y)) in layers {
            let mut defi = vec![0, 0, 0, 0];
            defi.extend_from_slice(&x.to_be_bytes());
            defi.extend_from_slice(&y.to_be_bytes());
            d.extend_from_slice(&chunk(b"DEFI", &defi));
            d.extend_from_slice(png_chunks(p));
        }
        d.extend_from_slice(&chunk(b"MEND", &[]));
        d
    }

    fn open(kind: Kind, bytes: Vec<u8>) -> Result<Arc<dyn Session>> {
        MngDecoder(kind).open(Arc::new(Source::from_vec("t", bytes)))
    }

    fn all_frames(s: &Arc<dyn Session>) -> Vec<Frame> {
        let mut f = s.frames(1).unwrap();
        std::iter::from_fn(|| f.next_frame(&CancelToken::new()))
            .map(|r| r.unwrap())
            .collect()
    }

    #[test]
    fn mng_frames_layers_and_loops() {
        let back = chunk(b"BACK", &[0, 0, 0, 0, 0xff, 0xff]);
        let d = mng(
            8,
            4,
            &[
                (png(8, 4, [255, 0, 0, 255]), (0, 0)),
                (png(2, 2, [0, 255, 0, 255]), (6, 2)),
                (png(4, 4, [0, 0, 0, 0]), (-2, -2)),
            ],
            &back,
        );
        let s = open(Kind::Mng, d.clone()).unwrap();
        assert_eq!((s.info().width, s.info().height), (8, 4));
        assert_eq!(s.animation().unwrap().frame_count, Some(3));
        assert_eq!(s.info().coding, "3 frames, 3 layers");
        assert!(!s.info().has_alpha);
        let fs = all_frames(&s);
        assert_eq!(fs.len(), 3);
        assert_eq!(fs[0].delay, Duration::from_millis(50));
        let px = |f: &Frame, x: usize, y: usize| {
            f.raster.planes[0].data[(y * 8 + x) * 4..][..4].to_vec()
        };
        assert_eq!(px(&fs[0], 7, 3), [255, 0, 0, 255]);
        assert_eq!(px(&fs[1], 7, 3), [0, 255, 0, 255]);
        assert_eq!(px(&fs[1], 0, 0), [255, 0, 0, 255]);
        // The still is the first frame.
        let still = s
            .decode(&DecodeRequest::full(1), &CancelToken::new())
            .unwrap();
        assert_eq!(still.planes[0].data, fs[0].raster.planes[0].data);
        let half = s
            .decode(&DecodeRequest::full(2), &CancelToken::new())
            .unwrap();
        assert_eq!((half.width, half.height), (4, 2));

        // LOOP 3 × (one layer) and TERM 3 forever.
        let mut d = MNG_SIGNATURE.to_vec();
        let mut mhdr = Vec::new();
        for v in [4u32, 4, 10, 0, 0, 0, 0] {
            mhdr.extend_from_slice(&v.to_be_bytes());
        }
        d.extend_from_slice(&chunk(b"MHDR", &mhdr));
        d.extend_from_slice(&chunk(b"TERM", &[3, 0, 0, 0, 0, 0, 0x7f, 0xff, 0xff, 0xff]));
        d.extend_from_slice(&chunk(b"LOOP", &[0, 0, 0, 0, 3]));
        d.extend_from_slice(png_chunks(&png(4, 4, [9, 9, 9, 255])));
        d.extend_from_slice(&chunk(b"ENDL", &[0]));
        d.extend_from_slice(&chunk(b"MEND", &[]));
        let s = open(Kind::Mng, d).unwrap();
        let a = s.animation().unwrap();
        assert_eq!((a.frame_count, a.loop_count), (Some(3), 0));
        let fs = all_frames(&s);
        assert_eq!(fs.len(), 3);
        assert_eq!(fs[2].delay, Duration::from_millis(100));
        assert!(s.info().has_alpha);
    }

    #[test]
    fn jng_with_png_alpha() {
        let (w, h) = (16u32, 8u32);
        let rgb: Vec<u8> = (0..w * h).flat_map(|_| [200u8, 40, 40]).collect();
        let jpeg = specular_jpeg::encode::encode(
            w,
            h,
            specular_jpeg::encode::Samples::Rgb(&rgb),
            &Default::default(),
        )
        .unwrap();
        let alpha = specular_png::encode(
            w,
            h,
            specular_png::ColorType::Gray,
            &vec![77; (w * h) as usize],
        )
        .unwrap();
        let mut d = JNG_SIGNATURE.to_vec();
        let mut jh = Vec::new();
        jh.extend_from_slice(&w.to_be_bytes());
        jh.extend_from_slice(&h.to_be_bytes());
        jh.extend_from_slice(&[14, 8, 8, 0, 8, 0, 0, 0]);
        d.extend_from_slice(&chunk(b"JHDR", &jh));
        for part in jpeg.chunks(100) {
            d.extend_from_slice(&chunk(b"JDAT", part));
        }
        // The alpha PNG's IDAT chunks.
        for c in chunks(&alpha).iter().filter(|c| &c.typ == b"IDAT") {
            d.extend_from_slice(&alpha[c.whole.clone()]);
        }
        d.extend_from_slice(&chunk(b"IEND", &[]));
        let s = open(Kind::Jng, d.clone()).unwrap();
        assert_eq!((s.info().width, s.info().height), (16, 8));
        assert_eq!(s.info().coding, "JPEG colour + 8-bit alpha");
        assert!(s.animation().is_none());
        let r = s
            .decode(&DecodeRequest::full(1), &CancelToken::new())
            .unwrap();
        for p in r.planes[0].data.chunks(4) {
            assert!(p[0].abs_diff(200) < 6 && p[1].abs_diff(40) < 6, "{p:?}");
            assert_eq!(p[3], 77);
        }
        // A JNG layer inside an MNG.
        let mut m = MNG_SIGNATURE.to_vec();
        let mut mhdr = Vec::new();
        for v in [w, h, 1, 0, 0, 0, 0] {
            mhdr.extend_from_slice(&v.to_be_bytes());
        }
        m.extend_from_slice(&chunk(b"MHDR", &mhdr));
        m.extend_from_slice(&d[8..]);
        m.extend_from_slice(&chunk(b"MEND", &[]));
        let s = open(Kind::Mng, m).unwrap();
        assert!(s.info().coding.contains("JNG layers"));
        let r2 = s
            .decode(&DecodeRequest::full(1), &CancelToken::new())
            .unwrap();
        assert_eq!(r2.planes[0].data[3], 77);
    }

    #[test]
    fn malformed_streams_fail_cleanly() {
        let d = mng(
            8,
            4,
            &[
                (png(8, 4, [255, 0, 0, 255]), (0, 0)),
                (png(2, 2, [0, 255, 0, 128]), (100, -100)),
            ],
            &[],
        );
        for n in 0..d.len() {
            if let Ok(s) = open(Kind::Mng, d[..n].to_vec()) {
                let _ = s.decode(&DecodeRequest::full(1), &CancelToken::new());
                if let Ok(mut f) = s.frames(1) {
                    while let Some(r) = f.next_frame(&CancelToken::new()) {
                        let _ = r;
                    }
                }
            }
        }
        // A huge canvas is refused before anything is allocated.
        let mut big = d.clone();
        big[16..20].copy_from_slice(&0x0010_0000u32.to_be_bytes());
        assert!(open(Kind::Mng, big).is_err());
        // Nested loops with enormous counts stop at the step budget, quickly.
        let mut l = MNG_SIGNATURE.to_vec();
        let mut mhdr = Vec::new();
        for v in [4u32, 4, 10, 0, 0, 0, 0] {
            mhdr.extend_from_slice(&v.to_be_bytes());
        }
        l.extend_from_slice(&chunk(b"MHDR", &mhdr));
        for k in 0..10u8 {
            l.extend_from_slice(&chunk(b"LOOP", &[k, 0x7f, 0xff, 0xff, 0xff]));
        }
        l.extend_from_slice(&chunk(b"FRAM", &[2]));
        for k in (0..10u8).rev() {
            l.extend_from_slice(&chunk(b"ENDL", &[k]));
        }
        l.extend_from_slice(png_chunks(&png(4, 4, [1, 2, 3, 255])));
        l.extend_from_slice(&chunk(b"MEND", &[]));
        let t = std::time::Instant::now();
        let s = open(Kind::Mng, l).unwrap();
        // The image lies past the step budget: no frame, an error, not a hang.
        assert!(
            s.decode(&DecodeRequest::full(1), &CancelToken::new())
                .is_err()
        );
        assert!(t.elapsed() < Duration::from_secs(5));
        // Garbage after a JNG signature.
        for n in 0..64u8 {
            let mut j = JNG_SIGNATURE.to_vec();
            j.extend((0..300).map(|i: u32| (i as u8).wrapping_mul(n)));
            if let Ok(s) = open(Kind::Jng, j) {
                let _ = s.decode(&DecodeRequest::full(1), &CancelToken::new());
            }
        }
    }
}
