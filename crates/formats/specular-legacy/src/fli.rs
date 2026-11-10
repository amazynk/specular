//! Autodesk Animator FLI (0xAF11) and Animator Pro FLC (0xAF12).
//!
//! 128-byte little-endian header, then frame chunks (0xF1FA) holding sub-chunks:
//! COLOR_256 (4), COLOR_64 (11), DELTA_FLC / SS2 (7), DELTA_FLI / LC (12), BLACK (13),
//! BYTE_RUN / BRUN (15), COPY (16); PSTAMP (18) and unknown chunks are skipped, as are prefix
//! chunks (0xF100). The file stores `frames + 1` frames: the last ("ring") frame turns the last
//! image back into the first for seamless looping. [`FliFrames`] does not emit it; rewinding
//! restarts from the first frame instead, which gives the same pictures.
//!
//! [`FliFrames`] maps 1:1 onto `specular_decode::Frames`: a sequential iterator that owns the
//! canvas and yields every frame as the full composited RGBA canvas with its stored delay.

use std::time::Duration;

use crate::*;

const HEADER: usize = 128;
const FRAME_TYPE: u16 = 0xF1FA;
const PREFIX_TYPE: u16 = 0xF100;

pub fn probe(head: &[u8]) -> bool {
    head.len() >= 16
        && matches!(u16le(head, 4), Some(0xAF11 | 0xAF12))
        && matches!(u16le(head, 12), Some(0 | 8))
}

/// Header facts of an animation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FliInfo {
    pub width: u32,
    pub height: u32,
    /// Frames to show (the ring frame excluded).
    pub frame_count: u32,
    /// Default delay per frame (a frame chunk may override it).
    pub delay: Duration,
    /// FLC (Animator Pro) rather than FLI.
    pub flc: bool,
    /// Times to play; 0 = forever (FLI animations loop; the ring frame exists for that).
    pub loop_count: u32,
}

fn header(data: &[u8]) -> Result<(FliInfo, usize)> {
    if !probe(data) {
        return Err(Error::NotRecognised);
    }
    if data.len() < HEADER {
        return Err(Error::Truncated("header".into()));
    }
    let flc = u16le(data, 4) == Some(0xAF12);
    let frames = u16le(data, 6).unwrap_or(0) as u32;
    let mut w = u16le(data, 8).unwrap_or(0) as u32;
    let mut h = u16le(data, 10).unwrap_or(0) as u32;
    if !flc && w == 0 && h == 0 {
        (w, h) = (320, 200);
    }
    check_dims(w, h)?;
    let delay = if flc {
        Duration::from_millis(u32le(data, 16).unwrap_or(0) as u64)
    } else {
        // FLI: 1/70 s "jiffies".
        Duration::from_micros(u16le(data, 16).unwrap_or(0) as u64 * 1_000_000 / 70)
    };
    let first = if flc {
        match u32le(data, 80) {
            Some(o) if o as usize >= HEADER && (o as usize) < data.len() => o as usize,
            _ => HEADER,
        }
    } else {
        HEADER
    };
    Ok((
        FliInfo {
            width: w,
            height: h,
            frame_count: frames,
            delay,
            flc,
            loop_count: 0,
        },
        first,
    ))
}

pub fn fli_info(data: &[u8]) -> Result<FliInfo> {
    Ok(header(data)?.0)
}

pub fn info(data: &[u8]) -> Result<Info> {
    let (i, _) = header(data)?;
    Ok(Info {
        format: Format::Fli,
        width: i.width,
        height: i.height,
        pixel_format: PixelFormat::Rgba8,
        has_alpha: false,
        model: crate::Model::Palette,
        bit_depth: 8,
        coding: format!(
            "{} animation, {} frames",
            if i.flc { "FLC" } else { "FLI" },
            i.frame_count
        ),
    })
}

/// The first frame only.
pub fn decode_first(data: &[u8]) -> Result<Image> {
    let mut f = FliFrames::new(data)?;
    match f.next_frame() {
        Some(r) => Ok(r?.image),
        None => Err(Error::Truncated("no frames".into())),
    }
}

/// One composited frame.
#[derive(Clone, Debug)]
pub struct FliFrame {
    pub index: u32,
    /// The whole canvas, RGBA8 (opaque).
    pub image: Image,
    /// As stored (frame override or header speed).
    pub delay: Duration,
}

/// Sequential frame decoder owning its canvas and palette.
pub struct FliFrames<D: AsRef<[u8]>> {
    data: D,
    info: FliInfo,
    first: usize,
    pos: usize,
    index: u32,
    canvas: Vec<u8>,
    palette: [[u8; 3]; 256],
    truncated: bool,
}

impl<D: AsRef<[u8]>> FliFrames<D> {
    pub fn new(data: D) -> Result<Self> {
        let (info, first) = header(data.as_ref())?;
        let n = info.width as usize * info.height as usize;
        // Every frame chunk needs 16 bytes; an animation whose canvas dwarfs the file is
        // corrupt (a COPY chunk would be needed to fill it, but BRUN can compress 127:1).
        check_payload(data.as_ref().len(), n as u64, 4096)?;
        Ok(FliFrames {
            data,
            info,
            first,
            pos: first,
            index: 0,
            canvas: vec![0; n],
            palette: [[0; 3]; 256],
            truncated: false,
        })
    }

    pub fn info(&self) -> &FliInfo {
        &self.info
    }

    /// Restart at frame 0 (never fails; `Result` to match `specular_decode::Frames::rewind`).
    pub fn rewind(&mut self) -> Result<()> {
        self.pos = self.first;
        self.index = 0;
        self.canvas.fill(0);
        self.palette = [[0; 3]; 256];
        self.truncated = false;
        Ok(())
    }

    /// The next frame, or `None` after the last (the ring frame is not shown).
    pub fn next_frame(&mut self) -> Option<Result<FliFrame>> {
        if self.index >= self.info.frame_count {
            return None;
        }
        let data = self.data.as_ref();
        // Find the next frame chunk, skipping prefix and unknown chunks.
        let (start, size, nchunks, delay_override) = loop {
            let p = self.pos;
            let (Some(size), Some(kind)) = (u32le(data, p), u16le(data, p + 4)) else {
                // Out of data: stop (frames already shown stay valid).
                return if self.index == 0 {
                    Some(Err(Error::Truncated("no frame data".into())))
                } else {
                    None
                };
            };
            let size = size as usize;
            if size < 6 {
                return Some(Err(Error::Malformed(format!("chunk size {size}"))));
            }
            self.pos = p.saturating_add(size);
            if kind == FRAME_TYPE {
                break (
                    p,
                    size,
                    u16le(data, p + 6).unwrap_or(0),
                    u16le(data, p + 8).unwrap_or(0),
                );
            }
            if kind != PREFIX_TYPE {
                // Unknown top-level chunk: skip it.
                continue;
            }
        };
        let end = start.saturating_add(size).min(data.len());
        if start + size > data.len() {
            self.truncated = true;
        }
        let mut p = start + 16;
        for _ in 0..nchunks {
            let (Some(csize), Some(ctype)) = (u32le(data, p), u16le(data, p + 4)) else {
                self.truncated = true;
                break;
            };
            let csize = csize as usize;
            if csize < 6 {
                self.truncated = true;
                break;
            }
            let cend = p.saturating_add(csize).min(end);
            let body = data.get(p + 6..cend).unwrap_or(&[]);
            if p.saturating_add(csize) > end {
                self.truncated = true;
            }
            let (w, h) = (self.info.width as usize, self.info.height as usize);
            let ok = apply(ctype, body, &mut self.canvas, &mut self.palette, w, h);
            if !ok {
                self.truncated = true;
            }
            p = p.saturating_add(csize);
            if p >= end {
                break;
            }
        }
        let delay = if delay_override != 0 && self.info.flc {
            Duration::from_millis(delay_override as u64)
        } else {
            self.info.delay
        };
        let mut image = Image::new(self.info.width, self.info.height, PixelFormat::Rgba8);
        expand_palette(&self.canvas, &self.palette, &mut image.data);
        image.truncated = self.truncated;
        let index = self.index;
        self.index += 1;
        Some(Ok(FliFrame {
            index,
            image,
            delay,
        }))
    }
}

/// Apply one sub-chunk; false when it was cut short or malformed.
fn apply(
    kind: u16,
    b: &[u8],
    canvas: &mut [u8],
    palette: &mut [[u8; 3]; 256],
    w: usize,
    h: usize,
) -> bool {
    match kind {
        4 | 11 => color(b, palette, kind == 11),
        7 => ss2(b, canvas, w, h),
        12 => lc(b, canvas, w, h),
        13 => {
            canvas.fill(0);
            true
        }
        15 => brun(b, canvas, w, h),
        16 => {
            let n = b.len().min(canvas.len());
            canvas[..n].copy_from_slice(&b[..n]);
            n == canvas.len()
        }
        _ => true, // PSTAMP (18) and unknown chunks
    }
}

/// COLOR_256 / COLOR_64 packets: (skip, count; 0 = 256) then count RGB triples.
fn color(b: &[u8], palette: &mut [[u8; 3]; 256], six_bit: bool) -> bool {
    let Some(packets) = u16le(b, 0) else {
        return false;
    };
    let s = |v: u8| {
        if six_bit {
            let v = v & 63;
            (v << 2) | (v >> 4)
        } else {
            v
        }
    };
    let mut i = 2;
    let mut idx = 0usize;
    for _ in 0..packets {
        let (Some(&skip), Some(&count)) = (b.get(i), b.get(i + 1)) else {
            return false;
        };
        i += 2;
        idx += skip as usize;
        let n = if count == 0 { 256 } else { count as usize };
        for _ in 0..n {
            let Some(c) = b.get(i..i + 3) else {
                return false;
            };
            i += 3;
            if idx < 256 {
                palette[idx] = [s(c[0]), s(c[1]), s(c[2])];
            }
            idx += 1;
        }
    }
    true
}

/// BYTE_RUN: every line, a (ignored) packet count, then i8 counts: positive = repeat the next
/// byte, negative = copy literal bytes; until the line is full.
fn brun(b: &[u8], canvas: &mut [u8], w: usize, h: usize) -> bool {
    let mut i = 0;
    for y in 0..h {
        let line = &mut canvas[y * w..(y + 1) * w];
        if b.get(i).is_none() {
            return false;
        }
        i += 1;
        let mut x = 0;
        while x < w {
            let Some(&c) = b.get(i) else { return false };
            i += 1;
            let c = c as i8;
            if c >= 0 {
                let Some(&v) = b.get(i) else { return false };
                i += 1;
                let n = (c as usize).min(w - x);
                line[x..x + n].fill(v);
                x += n;
                if c == 0 {
                    // A zero count would never advance; treat as corrupt.
                    return false;
                }
            } else {
                let n = (-(c as i32)) as usize;
                let take = n.min(w - x);
                let Some(src) = b.get(i..i + take) else {
                    return false;
                };
                line[x..x + take].copy_from_slice(src);
                i += n;
                x += take;
            }
        }
    }
    true
}

/// DELTA_FLI (LC): skip lines, then per line byte packets (skip, count: positive = literal,
/// negative = run).
fn lc(b: &[u8], canvas: &mut [u8], w: usize, h: usize) -> bool {
    let (Some(skip), Some(lines)) = (u16le(b, 0), u16le(b, 2)) else {
        return false;
    };
    let mut i = 4;
    for y in skip as usize..skip as usize + lines as usize {
        if y >= h {
            return false;
        }
        let line = &mut canvas[y * w..(y + 1) * w];
        let Some(&packets) = b.get(i) else {
            return false;
        };
        i += 1;
        let mut x = 0usize;
        for _ in 0..packets {
            let (Some(&sk), Some(&c)) = (b.get(i), b.get(i + 1)) else {
                return false;
            };
            i += 2;
            x += sk as usize;
            let c = c as i8;
            if c >= 0 {
                let n = c as usize;
                let Some(src) = b.get(i..i + n) else {
                    return false;
                };
                i += n;
                put(line, x, src);
                x += n;
            } else {
                let n = (-(c as i32)) as usize;
                let Some(&v) = b.get(i) else { return false };
                i += 1;
                let s = x.min(w);
                let e = (x + n).min(w);
                line[s..e].fill(v);
                x += n;
            }
        }
    }
    true
}

/// Copy `src` into `line` at `x`, clipped.
fn put(line: &mut [u8], x: usize, src: &[u8]) {
    if x < line.len() {
        let n = src.len().min(line.len() - x);
        line[x..x + n].copy_from_slice(&src[..n]);
    }
}

/// DELTA_FLC (SS2): word-oriented line deltas with line-skip and last-byte opcodes.
fn ss2(b: &[u8], canvas: &mut [u8], w: usize, h: usize) -> bool {
    let Some(lines) = u16le(b, 0) else {
        return false;
    };
    let mut i = 2;
    let mut y = 0usize;
    for _ in 0..lines {
        // Opcodes until the packet count.
        let mut last: Option<u8> = None;
        let packets = loop {
            let Some(op) = u16le(b, i) else { return false };
            i += 2;
            match op & 0xC000 {
                0x0000 => break op as usize,
                0xC000 => y += (-(op as i16) as i32) as usize,
                0x8000 => last = Some(op as u8),
                _ => return false, // 0x4000 is undefined
            }
        };
        if y >= h {
            return false;
        }
        let line = &mut canvas[y * w..(y + 1) * w];
        let mut x = 0usize;
        for _ in 0..packets {
            let (Some(&sk), Some(&c)) = (b.get(i), b.get(i + 1)) else {
                return false;
            };
            i += 2;
            x += sk as usize;
            let c = c as i8;
            if c >= 0 {
                let n = c as usize * 2;
                let Some(src) = b.get(i..i + n) else {
                    return false;
                };
                i += n;
                put(line, x, src);
                x += n;
            } else {
                let Some(pair) = b.get(i..i + 2) else {
                    return false;
                };
                i += 2;
                for _ in 0..(-(c as i32)) {
                    put(line, x, pair);
                    x += 2;
                }
            }
        }
        if let Some(v) = last {
            line[w - 1] = v;
        }
        y += 1;
    }
    true
}
