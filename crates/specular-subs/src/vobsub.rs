//! DVD subtitles (VobSub): an `.idx` text file (picture size, 16-colour palette, a
//! timestamp and file position per subtitle) beside a `.sub` MPEG program stream of
//! sub-picture units (SPUs); and Matroska `S_VOBSUB`, whose codec data is the `.idx` header
//! and whose blocks are whole SPUs.
//!
//! An SPU is 2-bit run-length coded interlaced fields and a list of control sequences:
//! start and stop times (in 1024/90 000 s units after the packet's time), the four colours
//! (picked from the palette) and their contrast (alpha), the display area and where each
//! field's data starts.

use std::sync::Arc;

use crate::{Bitmap, Body, Cue, Error, Result, Track};

/// The `.idx` facts that decoding needs.
#[derive(Clone, Debug)]
pub struct Header {
    pub size: (u32, u32),
    pub palette: [[u8; 3]; 16],
}

impl Default for Header {
    fn default() -> Self {
        // The usual DVD greys when an `.idx` has no palette.
        let mut palette = [[0u8; 3]; 16];
        for (i, p) in palette.iter_mut().enumerate() {
            let v = (i * 17) as u8;
            *p = [v, v, v];
        }
        Header {
            size: (720, 480),
            palette,
        }
    }
}

impl Header {
    /// Reads `size:` and `palette:` lines.
    pub fn parse(idx: &str) -> Header {
        let mut h = Header::default();
        for line in idx.lines() {
            let l = line.trim();
            if let Some(v) = l.strip_prefix("size:") {
                if let Some((w, hh)) = v.trim().split_once('x')
                    && let (Ok(w), Ok(hh)) = (w.trim().parse(), hh.trim().parse())
                {
                    h.size = (w, hh);
                }
            } else if let Some(v) = l.strip_prefix("palette:") {
                for (i, c) in v.split(',').take(16).enumerate() {
                    if let Ok(x) = u32::from_str_radix(c.trim(), 16) {
                        h.palette[i] = [(x >> 16) as u8, (x >> 8) as u8, x as u8];
                    }
                }
            }
        }
        h
    }
}

/// `timestamp: 00:00:01:101, filepos: 000000000` lines of the first stream (`id:` … `index:
/// 0`), in microseconds and byte offsets.
fn entries(idx: &str) -> Vec<(i64, usize)> {
    let mut out = Vec::new();
    let mut streams = 0;
    let mut delay_us = 0i64;
    for line in idx.lines() {
        let l = line.trim();
        if l.starts_with("id:") {
            streams += 1;
            if streams > 1 {
                break;
            }
            continue;
        }
        if let Some(v) = l.strip_prefix("delay:") {
            delay_us += time(v.trim()).unwrap_or(0);
            continue;
        }
        let Some(v) = l.strip_prefix("timestamp:") else {
            continue;
        };
        let Some((t, pos)) = v.split_once(',') else {
            continue;
        };
        let Some(t) = time(t.trim()) else { continue };
        let pos = pos.trim().trim_start_matches("filepos:").trim();
        if let Ok(p) = usize::from_str_radix(pos, 16) {
            out.push((t + delay_us, p));
        }
    }
    out
}

/// `hh:mm:ss:mmm` (also with a leading `-` for delays).
fn time(s: &str) -> Option<i64> {
    let (neg, s) = match s.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, s),
    };
    let p: Vec<i64> = s
        .split(':')
        .map(|x| x.trim().parse().ok())
        .collect::<Option<_>>()?;
    let [h, m, sec, ms] = p.as_slice() else {
        return None;
    };
    let v = ((h * 60 + m) * 60 + sec) * 1_000_000 + ms * 1000;
    Some(if neg { -v } else { v })
}

/// The SPU found at `pos` in a `.sub` program stream: private stream 1 packets of one
/// substream, joined until the SPU's own size is reached.
fn spu_at(sub: &[u8], pos: usize) -> Option<Vec<u8>> {
    let mut p = pos;
    let mut spu: Vec<u8> = Vec::new();
    let mut want: Option<usize> = None;
    let mut stream: Option<u8> = None;
    let mut guard = 0;
    while p + 4 <= sub.len() && guard < 256 {
        guard += 1;
        if sub[p..p + 3] != [0, 0, 1] {
            p += 1;
            continue;
        }
        let code = sub[p + 3];
        match code {
            0xBA => {
                // Pack header (MPEG-2: 14 bytes plus stuffing).
                if p + 14 > sub.len() {
                    return None;
                }
                if sub[p + 4] & 0xC0 == 0x40 {
                    p += 14 + usize::from(sub[p + 13] & 7);
                } else {
                    p += 12;
                }
            }
            0xBD => {
                let len = usize::from(u16::from_be_bytes([*sub.get(p + 4)?, *sub.get(p + 5)?]));
                let body = sub.get(p + 6..p + 6 + len)?;
                // MPEG-2 PES header: flags, header length.
                let hl = usize::from(*body.get(2)?);
                let payload = body.get(3 + hl..)?;
                let (&id, data) = payload.split_first()?;
                if (0x20..0x40).contains(&id) && stream.is_none_or(|s| s == id) {
                    stream = Some(id);
                    spu.extend_from_slice(data);
                    if want.is_none() && spu.len() >= 2 {
                        want = Some(usize::from(u16::from_be_bytes([spu[0], spu[1]])));
                    }
                    if want.is_some_and(|w| spu.len() >= w) {
                        spu.truncate(want.unwrap_or(spu.len()));
                        return Some(spu);
                    }
                }
                p += 6 + len;
            }
            0xB9 => return None,
            _ => {
                // Other streams (padding, video): skip by their length.
                let len = usize::from(u16::from_be_bytes([*sub.get(p + 4)?, *sub.get(p + 5)?]));
                p += 6 + len;
            }
        }
    }
    None
}

/// A decoded SPU: its picture and when it shows relative to its packet time.
#[derive(Debug)]
pub struct Spu {
    pub start_us: i64,
    pub stop_us: Option<i64>,
    pub bitmap: Option<Bitmap>,
}

/// Decodes one SPU with the `.idx` header.
pub fn decode_spu(spu: &[u8], h: &Header) -> Option<Spu> {
    if spu.len() < 4 {
        return None;
    }
    let ctrl = usize::from(u16::from_be_bytes([spu[2], spu[3]]));
    let (mut colours, mut alpha) = ([0u8; 4], [0u8, 15, 15, 15]);
    let mut area: Option<(u32, u32, u32, u32)> = None;
    let mut fields: Option<(usize, usize)> = None;
    let (mut start, mut stop) = (None, None);
    let mut p = ctrl;
    let mut seen = 0;
    loop {
        if p + 4 > spu.len() || seen > 32 {
            break;
        }
        seen += 1;
        let delay = i64::from(u16::from_be_bytes([spu[p], spu[p + 1]])) * 1024 * 1_000_000 / 90_000;
        let next = usize::from(u16::from_be_bytes([spu[p + 2], spu[p + 3]]));
        let mut q = p + 4;
        while q < spu.len() {
            let cmd = spu[q];
            q += 1;
            match cmd {
                0x00 | 0x01 => start = Some(start.unwrap_or(delay)),
                0x02 => stop = Some(delay),
                0x03 => {
                    let b = spu.get(q..q + 2)?;
                    colours = [b[1] & 15, b[1] >> 4, b[0] & 15, b[0] >> 4];
                    q += 2;
                }
                0x04 => {
                    let b = spu.get(q..q + 2)?;
                    alpha = [b[1] & 15, b[1] >> 4, b[0] & 15, b[0] >> 4];
                    q += 2;
                }
                0x05 => {
                    let b = spu.get(q..q + 6)?;
                    let x1 = (u32::from(b[0]) << 4) | u32::from(b[1] >> 4);
                    let x2 = (u32::from(b[1] & 15) << 8) | u32::from(b[2]);
                    let y1 = (u32::from(b[3]) << 4) | u32::from(b[4] >> 4);
                    let y2 = (u32::from(b[4] & 15) << 8) | u32::from(b[5]);
                    area = Some((x1, y1, x2, y2));
                    q += 6;
                }
                0x06 => {
                    let b = spu.get(q..q + 4)?;
                    fields = Some((
                        usize::from(u16::from_be_bytes([b[0], b[1]])),
                        usize::from(u16::from_be_bytes([b[2], b[3]])),
                    ));
                    q += 4;
                }
                0x07 => {
                    // Colour and contrast changes within the area: their own size.
                    let b = spu.get(q..q + 2)?;
                    q += usize::from(u16::from_be_bytes([b[0], b[1]]));
                }
                _ => break,
            }
        }
        if next == p || next < ctrl {
            break;
        }
        p = next;
    }
    let bitmap = match (area, fields) {
        (Some((x1, y1, x2, y2)), Some((top, bottom))) if x2 >= x1 && y2 >= y1 => {
            let (w, hh) = (x2 - x1 + 1, y2 - y1 + 1);
            if w * hh > 4_000_000 {
                return None;
            }
            let mut rgba = vec![0u8; (w * hh * 4) as usize];
            let lut: Vec<[u8; 4]> = (0..4)
                .map(|i| {
                    let c = h.palette[usize::from(colours[i])];
                    [c[0], c[1], c[2], alpha[i] * 17]
                })
                .collect();
            for (field, off) in [(0u32, top), (1, bottom)] {
                let mut r = Nibbles::new(spu, off, ctrl);
                let mut y = field;
                while y < hh {
                    let mut x = 0u32;
                    while x < w {
                        let Some(v) = r.code() else { break };
                        let (mut n, c) = (v >> 2, (v & 3) as usize);
                        if n == 0 {
                            n = w - x;
                        }
                        let n = n.min(w - x);
                        let row = (y * w + x) as usize * 4;
                        for k in 0..n as usize {
                            rgba[row + k * 4..row + k * 4 + 4].copy_from_slice(&lut[c]);
                        }
                        x += n;
                    }
                    r.align();
                    if r.done() {
                        break;
                    }
                    y += 2;
                }
            }
            (!rgba.chunks_exact(4).all(|p| p[3] == 0)).then(|| Bitmap {
                x: x1,
                y: y1,
                w,
                h: hh,
                canvas: h.size,
                rgba: Arc::from(rgba.into_boxed_slice()),
            })
        }
        _ => None,
    };
    Some(Spu {
        start_us: start.unwrap_or(0),
        stop_us: stop,
        bitmap,
    })
}

/// Reads nibbles from `start` up to `end`.
struct Nibbles<'a> {
    d: &'a [u8],
    pos: usize,
    half: bool,
    end: usize,
}

impl<'a> Nibbles<'a> {
    fn new(d: &'a [u8], start: usize, end: usize) -> Nibbles<'a> {
        Nibbles {
            d,
            pos: start,
            half: false,
            end: end.min(d.len()),
        }
    }

    fn nibble(&mut self) -> Option<u32> {
        if self.pos >= self.end {
            return None;
        }
        let b = self.d[self.pos];
        let v = if self.half {
            self.pos += 1;
            b & 15
        } else {
            b >> 4
        };
        self.half = !self.half;
        Some(u32::from(v))
    }

    /// One run code: 1 to 4 nibbles, by its leading zeros.
    fn code(&mut self) -> Option<u32> {
        let mut v = self.nibble()?;
        if v < 0x4 {
            v = (v << 4) | self.nibble()?;
            if v < 0x10 {
                v = (v << 4) | self.nibble()?;
                if v < 0x40 {
                    v = (v << 4) | self.nibble()?;
                }
            }
        }
        Some(v)
    }

    fn align(&mut self) {
        if self.half {
            self.half = false;
            self.pos += 1;
        }
    }

    fn done(&self) -> bool {
        self.pos >= self.end
    }
}

/// An SPU at packet time `pts_us` as a cue (`next_us`: the next subtitle's time, ending one
/// without a stop).
pub fn cue(spu: &[u8], h: &Header, pts_us: i64, next_us: Option<i64>) -> Option<Cue> {
    let s = decode_spu(spu, h)?;
    let bitmap = s.bitmap?;
    let start = pts_us + s.start_us;
    let end = match s.stop_us {
        Some(st) if st > s.start_us => pts_us + st,
        _ => next_us.unwrap_or(start + 5_000_000),
    };
    Some(Cue {
        start_us: start,
        end_us: end.max(start + 1),
        body: Body::Bitmap(bitmap),
    })
}

/// An `.idx` and its `.sub`.
pub fn from_files(idx: &str, sub: &[u8]) -> Result<Track> {
    let h = Header::parse(idx);
    let list = entries(idx);
    if list.is_empty() {
        return Err(Error::Malformed("no timestamps in the .idx".into()));
    }
    let mut cues = Vec::new();
    for (i, &(t, pos)) in list.iter().enumerate() {
        let next = list.get(i + 1).map(|e| e.0);
        if let Some(spu) = spu_at(sub, pos)
            && let Some(c) = cue(&spu, &h, t, next)
        {
            cues.push(c);
        }
    }
    Ok(Track::new(cues))
}

/// Test data: an SPU drawing a `w`×`h` box of colour 1, and its `.sub`/`.idx`.
#[doc(hidden)]
pub mod fixture {
    /// An SPU at (x, y) of `w`×`h` (even height), shown from delay 0 to `stop` units.
    pub fn spu(x: u16, y: u16, w: u16, h: u16, stop: u16) -> Vec<u8> {
        // Each line: one run of colour 1 to the end of the line (n = 0 → fill), coded in
        // four nibbles: 0000 00nn nnnn nncc with n = 0, c = 1 → 0x0001.
        let line = [0x00u8, 0x01];
        let mut top = Vec::new();
        let mut bottom = Vec::new();
        for _ in 0..h / 2 {
            top.extend_from_slice(&line);
            bottom.extend_from_slice(&line);
        }
        let top_at = 4usize;
        let bottom_at = top_at + top.len();
        let ctrl = bottom_at + bottom.len();
        let mut s = vec![0, 0, (ctrl >> 8) as u8, ctrl as u8];
        s.extend_from_slice(&top);
        s.extend_from_slice(&bottom);
        let (x2, y2) = (x + w - 1, y + h - 1);
        // First control sequence: colours, contrast, area, field offsets, start.
        let second = ctrl + 4 + 1 + 3 + 3 + 7 + 5 + 1;
        s.extend_from_slice(&[0, 0, (second >> 8) as u8, second as u8]);
        s.push(0x01);
        s.extend_from_slice(&[0x03, 0x00, 0x10]); // colour 1 = palette entry 1
        s.extend_from_slice(&[0x04, 0x00, 0xF0]); // colour 1 opaque
        s.extend_from_slice(&[
            0x05,
            (x >> 4) as u8,
            (((x & 15) << 4) as u8) | (x2 >> 8) as u8,
            x2 as u8,
            (y >> 4) as u8,
            (((y & 15) << 4) as u8) | (y2 >> 8) as u8,
            y2 as u8,
        ]);
        s.extend_from_slice(&[
            0x06,
            (top_at >> 8) as u8,
            top_at as u8,
            (bottom_at >> 8) as u8,
            bottom_at as u8,
        ]);
        s.push(0xFF);
        // Second: stop.
        s.extend_from_slice(&[
            (stop >> 8) as u8,
            stop as u8,
            (second >> 8) as u8,
            second as u8,
            0x02,
            0xFF,
        ]);
        let n = s.len();
        s[0] = (n >> 8) as u8;
        s[1] = n as u8;
        s
    }

    /// The SPU in a program stream pack (one PES packet of substream 0x20).
    pub fn sub(spus: &[Vec<u8>]) -> (Vec<u8>, Vec<usize>) {
        let mut out = Vec::new();
        let mut at = Vec::new();
        for s in spus {
            at.push(out.len());
            out.extend_from_slice(&[0, 0, 1, 0xBA, 0x44, 0, 4, 0, 4, 1, 0, 0, 3, 0xF8]);
            let len = 3 + 5 + 1 + s.len();
            out.extend_from_slice(&[0, 0, 1, 0xBD, (len >> 8) as u8, len as u8]);
            out.extend_from_slice(&[0x81, 0x80, 5, 0x21, 0, 1, 0, 1, 0x20]);
            out.extend_from_slice(s);
        }
        (out, at)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idx_and_sub() {
        let a = fixture::spu(100, 400, 40, 6, 176);
        let b = fixture::spu(10, 20, 8, 2, 0);
        let (sub, at) = fixture::sub(&[a, b]);
        let idx = format!(
            "# VobSub index file, v7\nsize: 720x576\npalette: 000000, ffffff, 808080, 000000, 000000, 000000, 000000, 000000, 000000, 000000, 000000, 000000, 000000, 000000, 000000, 000000\n\nid: en, index: 0\ntimestamp: 00:00:01:000, filepos: {:09x}\ntimestamp: 00:00:04:500, filepos: {:09x}\n",
            at[0], at[1]
        );
        let t = from_files(&idx, &sub).unwrap();
        assert_eq!(t.cues.len(), 2);
        let c = &t.cues[0];
        assert_eq!(c.start_us, 1_000_000);
        // 176 units of 1024/90000 s.
        assert_eq!(c.end_us, 1_000_000 + 176 * 1024 * 1_000_000 / 90_000);
        let Body::Bitmap(bm) = &c.body else { panic!() };
        assert_eq!(
            (bm.x, bm.y, bm.w, bm.h, bm.canvas),
            (100, 400, 40, 6, (720, 576))
        );
        assert!(bm.rgba.chunks_exact(4).all(|p| p == [255, 255, 255, 255]));
        // No stop: until 5 s later (the last one).
        assert_eq!(t.cues[1].end_us, 4_500_000 + 5_000_000);
    }

    #[test]
    fn header_and_times() {
        let h = Header::parse("size: 720x480\npalette: ff0000, 00ff00\n");
        assert_eq!(h.size, (720, 480));
        assert_eq!(h.palette[0], [255, 0, 0]);
        assert_eq!(h.palette[1], [0, 255, 0]);
        assert_eq!(time("01:02:03:004"), Some(3_723_004_000));
        assert_eq!(time("-00:00:01:000"), Some(-1_000_000));
    }
}
