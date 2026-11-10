//! Netpbm: PBM (P1/P4), PGM (P2/P5), PPM (P3/P6) and PAM (P7), 1–16-bit samples.
//!
//! Binary rasters have fixed-size rows, so any region is decoded straight from the file in
//! parallel. ASCII ("plain") rasters are parsed once into a full-resolution buffer.
//!
//! Netpbm lets a file hold several images back to back (`pnmsplit` takes them apart):
//! [`image_offsets`] finds where each starts by reading headers only, and [`Pnm::parse_at`]
//! opens one.
//!
//! Samples deeper than 8 bits (maxval above 255) are rounded to 8 bits by [`Decode::decode`]
//! and kept at 16 bits (scaled to 0..=65535) by [`Decode::decode_wide`]: grey as
//! [`PixelFormat::Gray16`], grey + alpha as [`PixelFormat::GrayAlpha16`], RGB(A) as
//! [`PixelFormat::Rgba16`].

use crate::px::rgb_to_rgba;
use crate::scale::{Plan, Rows, Rows16, fill_grey, fill_grey16, render, render16};
use crate::{Cancel, Decode, Error, Image, Info, Options, PixelFormat, Result};

pub fn probe(head: &[u8]) -> bool {
    head.len() >= 3
        && head[0] == b'P'
        && (b'1'..=b'7').contains(&head[1])
        && head[2].is_ascii_whitespace()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    /// 1 bit per pixel (1 = black), packed MSB first (P4) or one ASCII digit per pixel (P1).
    Bits,
    /// `depth` samples per pixel of `bytes` (1 or 2, big-endian) each.
    Samples { depth: u32, maxval: u32 },
}

#[derive(Clone, Debug)]
pub struct Pnm {
    info: Info,
    kind: Kind,
    ascii: bool,
    /// Offset of the raster.
    start: usize,
}

struct Header<'a> {
    d: &'a [u8],
    p: usize,
}

impl Header<'_> {
    fn skip_ws(&mut self) {
        while let Some(&c) = self.d.get(self.p) {
            if c == b'#' {
                while self
                    .d
                    .get(self.p)
                    .is_some_and(|&c| c != b'\n' && c != b'\r')
                {
                    self.p += 1;
                }
            } else if c.is_ascii_whitespace() {
                self.p += 1;
            } else {
                break;
            }
        }
    }
    fn num(&mut self) -> Result<u32> {
        self.skip_ws();
        let s = self.p;
        let mut v: u64 = 0;
        while let Some(&c) = self.d.get(self.p) {
            if !c.is_ascii_digit() {
                break;
            }
            v = (v * 10 + (c - b'0') as u64).min(u32::MAX as u64);
            self.p += 1;
        }
        if self.p == s {
            return Err(if self.p >= self.d.len() {
                Error::Truncated("PNM header".into())
            } else {
                Error::Malformed("PNM header: number expected".into())
            });
        }
        Ok(v as u32)
    }
    /// Next whitespace-separated token on the current line (PAM).
    fn line(&mut self) -> Option<&[u8]> {
        // Skip blank lines and comment lines.
        loop {
            while self.d.get(self.p).is_some_and(|c| c.is_ascii_whitespace()) {
                self.p += 1;
            }
            if self.d.get(self.p) == Some(&b'#') {
                while self.d.get(self.p).is_some_and(|&c| c != b'\n') {
                    self.p += 1;
                }
                continue;
            }
            break;
        }
        let s = self.p;
        while self.d.get(self.p).is_some_and(|&c| c != b'\n') {
            self.p += 1;
        }
        let l = &self.d[s..self.p];
        if self.p < self.d.len() {
            self.p += 1;
        }
        (!l.is_empty()).then_some(l)
    }
}

fn parse_num(s: &[u8]) -> Result<u32> {
    let s = std::str::from_utf8(s).map_err(|_| Error::Malformed("PAM header".into()))?;
    s.trim()
        .parse()
        .map_err(|_| Error::Malformed(format!("PAM header value {s:?}")))
}

/// Images of a multi-image file at most.
pub const MAX_IMAGES: usize = 4096;

/// Where the images of a (possibly multi-image) file start: 0, then just past each binary
/// raster where another header follows (whitespace before it is skipped). Only headers are
/// read. The scan stops after a plain (ASCII) image, whose end is known only by parsing its
/// samples, at bytes that are not a header that parses, at a raster the file does not hold
/// whole, and at [`MAX_IMAGES`].
pub fn image_offsets(data: &[u8]) -> Vec<usize> {
    let mut out = vec![0];
    let mut off = 0;
    while out.len() < MAX_IMAGES {
        let Ok(p) = Pnm::parse_at(data, off) else {
            break;
        };
        let Some(end) = p.end() else { break };
        let Some(next) = data
            .get(end..)
            .and_then(|rest| rest.iter().position(|c| !c.is_ascii_whitespace()))
            .map(|n| end + n)
        else {
            break;
        };
        if !probe(&data[next..]) || Pnm::parse_at(data, next).is_err() {
            break;
        }
        out.push(next);
        off = next;
    }
    out
}

impl Pnm {
    /// The image whose header starts at byte `off` of `data` (see [`image_offsets`]); it
    /// decodes from the whole of `data`.
    pub fn parse_at(data: &[u8], off: usize) -> Result<Pnm> {
        let rest = data
            .get(off..)
            .ok_or_else(|| Error::Truncated("PNM header".into()))?;
        let mut p = Pnm::parse(rest)?;
        p.start += off;
        Ok(p)
    }

    /// Just past the raster, for a binary image the file holds whole.
    fn end(&self) -> Option<usize> {
        if self.ascii {
            return None;
        }
        let n = self.row_bytes().checked_mul(self.info.height as usize)?;
        self.start.checked_add(n)
    }

    pub fn parse(data: &[u8]) -> Result<Pnm> {
        if data.len() < 3 {
            return Err(Error::Truncated("PNM header".into()));
        }
        if !probe(data) {
            return Err(Error::Malformed("not a PNM file".into()));
        }
        let m = data[1];
        let mut h = Header { d: data, p: 2 };
        let (w, ht, kind, tupl) = if m == b'7' {
            let (mut w, mut ht, mut depth, mut maxval) = (None, None, None, None);
            let mut tupl = String::new();
            loop {
                let Some(l) = h.line() else {
                    return Err(Error::Truncated("PAM header".into()));
                };
                let l = l.trim_ascii();
                if l == b"ENDHDR" {
                    break;
                }
                let (k, v) = match l.iter().position(|c| c.is_ascii_whitespace()) {
                    Some(i) => (&l[..i], l[i..].trim_ascii()),
                    None => (l, &b""[..]),
                };
                match k {
                    b"WIDTH" => w = Some(parse_num(v)?),
                    b"HEIGHT" => ht = Some(parse_num(v)?),
                    b"DEPTH" => depth = Some(parse_num(v)?),
                    b"MAXVAL" => maxval = Some(parse_num(v)?),
                    b"TUPLTYPE" => {
                        if !tupl.is_empty() {
                            tupl.push(' ');
                        }
                        tupl.push_str(&String::from_utf8_lossy(v));
                    }
                    _ => {
                        return Err(Error::Malformed(format!(
                            "PAM header line {:?}",
                            String::from_utf8_lossy(l)
                        )));
                    }
                }
            }
            let (Some(w), Some(ht), Some(depth), Some(maxval)) = (w, ht, depth, maxval) else {
                return Err(Error::Malformed("PAM header incomplete".into()));
            };
            if !(1..=4).contains(&depth) {
                return Err(Error::Unsupported(format!("PAM depth {depth}")));
            }
            if !(1..=65535).contains(&maxval) {
                return Err(Error::Malformed(format!("PAM maxval {maxval}")));
            }
            (w, ht, Kind::Samples { depth, maxval }, tupl)
        } else {
            let w = h.num()?;
            let ht = h.num()?;
            let kind = match m {
                b'1' | b'4' => Kind::Bits,
                _ => {
                    let maxval = h.num()?;
                    if !(1..=65535).contains(&maxval) {
                        return Err(Error::Malformed(format!("PNM maxval {maxval}")));
                    }
                    let depth = if matches!(m, b'3' | b'6') { 3 } else { 1 };
                    Kind::Samples { depth, maxval }
                }
            };
            // Exactly one whitespace byte separates the header from a binary raster.
            if h.p >= data.len() {
                if matches!(m, b'4' | b'5' | b'6') {
                    return Err(Error::Truncated("PNM header".into()));
                }
            } else {
                h.p += 1;
            }
            (w, ht, kind, String::new())
        };
        crate::check_dims(w, ht)?;
        // PAM BLACKANDWHITE with maxval 1 is a bitmap stored one byte per sample; treat it as
        // grey samples with maxval 1 (1 = white), which is what Samples does.
        let ascii = matches!(m, b'1' | b'2' | b'3');
        let (has_alpha, gray, bits) = match kind {
            Kind::Bits => (false, true, 1),
            Kind::Samples { depth, maxval } => (
                depth == 2 || depth == 4,
                depth == 1,
                if maxval > 255 {
                    16
                } else {
                    (32 - maxval.leading_zeros()) as u8
                },
            ),
        };
        let name = match m {
            b'1' | b'4' => "PBM",
            b'2' | b'5' => "PGM",
            b'3' | b'6' => "PPM",
            _ => "PAM",
        };
        let coding = format!(
            "{name} {}{}, {bits}-bit{}",
            if ascii { "plain" } else { "binary" },
            if tupl.is_empty() {
                String::new()
            } else {
                format!(" {tupl}")
            },
            match kind {
                Kind::Samples { maxval, .. } if maxval != 255 && maxval != 65535 && maxval != 1 =>
                    format!(" (maxval {maxval})"),
                _ => String::new(),
            }
        );
        Ok(Pnm {
            info: Info {
                width: w,
                height: ht,
                has_alpha,
                alpha: crate::AlphaKind::Straight,
                gray,
                model: match kind {
                    Kind::Samples { depth: 3 | 4, .. } => crate::Model::Rgb,
                    _ => crate::Model::Grey,
                },
                bit_depth: bits,
                coding,
                random_access: !ascii,
            },
            kind,
            ascii,
            start: h.p,
        })
    }

    fn row_bytes(&self) -> usize {
        let w = self.info.width as usize;
        match self.kind {
            Kind::Bits => w.div_ceil(8),
            Kind::Samples { depth, maxval } => {
                w * depth as usize * if maxval > 255 { 2 } else { 1 }
            }
        }
    }

    fn channels(&self) -> usize {
        if self.info.gray { 1 } else { 4 }
    }

    /// Samples per pixel of 16-bit output.
    fn channels16(&self) -> usize {
        match self.kind {
            Kind::Samples { depth: 2, .. } => 2,
            Kind::Samples { depth: 3 | 4, .. } => 4,
            _ => 1,
        }
    }
}

/// Sample `v` (at most `maxval`) scaled to 16 bits.
#[inline]
fn scale16(v: u32, maxval: u32) -> u16 {
    if maxval == 65535 {
        v as u16
    } else {
        ((v.min(maxval) * 65535 + maxval / 2) / maxval) as u16
    }
}

/// Write `n` pixels of `d` samples (sample `i` via `s`) as 16-bit grey (d = 1), grey + alpha
/// (d = 2) or RGBA.
#[inline(always)]
fn expand16(d: usize, n: usize, s: impl Fn(usize) -> u16, out: &mut [u16]) {
    match d {
        1 | 2 => {
            for (i, o) in out[..n * d].iter_mut().enumerate() {
                *o = s(i);
            }
        }
        3 => {
            for (i, o) in out.chunks_exact_mut(4).take(n).enumerate() {
                o.copy_from_slice(&[s(i * 3), s(i * 3 + 1), s(i * 3 + 2), 65535]);
            }
        }
        _ => {
            for (i, o) in out[..n * 4].iter_mut().enumerate() {
                *o = s(i);
            }
        }
    }
}

/// Binary samples deeper than 8 bits as 16-bit rows.
struct BinRows16<'a> {
    d: &'a [u8],
    start: usize,
    rb: usize,
    depth: usize,
    maxval: u32,
    ch: usize,
}

impl BinRows16<'_> {
    fn conv(&self, row: &[u8], x0: usize, x1: usize, out: &mut [u16]) {
        let px = &row[x0 * self.depth * 2..x1 * self.depth * 2];
        let s = |i: usize| {
            scale16(
                u16::from_be_bytes([px[2 * i], px[2 * i + 1]]) as u32,
                self.maxval,
            )
        };
        expand16(self.depth, x1 - x0, s, out);
    }
}

impl Rows16 for BinRows16<'_> {
    fn row16(&self, y: u32, x0: u32, x1: u32, out: &mut [u16]) {
        let (x0, x1) = (x0 as usize, x1 as usize);
        let s = self.start + y as usize * self.rb;
        match self.d.get(s..s + self.rb) {
            Some(row) => self.conv(row, x0, x1, out),
            None => {
                // Partial last row: decode what's there over grey.
                fill_grey16(out, self.ch);
                let avail = self.d.len().saturating_sub(s);
                let n = (avail / (self.depth * 2)).min(x1);
                if n > x0 {
                    let mut tmp = vec![0u8; self.rb];
                    tmp[..avail].copy_from_slice(&self.d[s..]);
                    self.conv(&tmp, x0, n, &mut out[..(n - x0) * self.ch]);
                }
            }
        }
    }
}

/// A decoded plain (ASCII) 16-bit raster at full resolution, as a row source.
struct Full16 {
    w: usize,
    ch: usize,
    data: Vec<u16>,
}

impl Rows16 for Full16 {
    fn row16(&self, y: u32, x0: u32, x1: u32, out: &mut [u16]) {
        let s = (y as usize * self.w + x0 as usize) * self.ch;
        out.copy_from_slice(&self.data[s..s + (x1 - x0) as usize * self.ch]);
    }
}

/// The numbers of a plain raster, in order.
struct Plain<'a> {
    d: &'a [u8],
    p: usize,
}

impl Plain<'_> {
    /// The next number; `bit` reads one P1 digit (no separator needed).
    fn next(&mut self, bit: bool) -> Option<u32> {
        let d = self.d;
        // Skip whitespace and comments.
        loop {
            let c = *d.get(self.p)?;
            if c == b'#' {
                while d.get(self.p).is_some_and(|&c| c != b'\n') {
                    self.p += 1;
                }
            } else if c.is_ascii_whitespace() {
                self.p += 1;
            } else {
                break;
            }
        }
        if bit {
            let c = d[self.p];
            self.p += 1;
            return c.is_ascii_digit().then_some((c != b'0') as u32);
        }
        let mut v: u32 = 0;
        let s = self.p;
        while let Some(&c) = d.get(self.p).filter(|c| c.is_ascii_digit()) {
            v = v.saturating_mul(10).saturating_add((c - b'0') as u32);
            self.p += 1;
        }
        (self.p > s).then_some(v)
    }
}

/// Sample value → 8-bit.
fn sample_lut(maxval: u32) -> Vec<u8> {
    let n = if maxval > 255 { 65536 } else { 256 };
    (0..n as u32)
        .map(|v| ((v.min(maxval) * 255 + maxval / 2) / maxval) as u8)
        .collect()
}

struct BinRows<'a> {
    d: &'a [u8],
    p: &'a Pnm,
    rb: usize,
    lut: Vec<u8>,
}

impl Rows for BinRows<'_> {
    fn row(&self, y: u32, x0: u32, x1: u32, out: &mut [u8]) {
        let ch = self.p.channels();
        let s = self.p.start + y as usize * self.rb;
        let Some(row) = self.d.get(s..s + self.rb) else {
            // Partial last row: decode what's there over grey.
            fill_grey(out, ch);
            let mut tmp = vec![0u8; self.rb];
            let avail = self.d.len().saturating_sub(s);
            if avail == 0 {
                return;
            }
            tmp[..avail].copy_from_slice(&self.d[s..]);
            let n = self.px_in(avail);
            if n > x0 as usize {
                let x1b = (x1 as usize).min(n) as u32;
                self.conv(&tmp, x0, x1b, &mut out[..(x1b - x0) as usize * ch]);
            }
            return;
        };
        self.conv(row, x0, x1, out);
    }
}

impl BinRows<'_> {
    /// Complete pixels in the first `bytes` of a row.
    fn px_in(&self, bytes: usize) -> usize {
        match self.p.kind {
            Kind::Bits => bytes * 8,
            Kind::Samples { depth, maxval } => {
                bytes / (depth as usize * if maxval > 255 { 2 } else { 1 })
            }
        }
    }

    fn conv(&self, row: &[u8], x0: u32, x1: u32, out: &mut [u8]) {
        let (x0, x1) = (x0 as usize, x1 as usize);
        match self.p.kind {
            Kind::Bits => {
                for (x, o) in (x0..x1).zip(out.iter_mut()) {
                    *o = if row[x >> 3] & (0x80 >> (x & 7)) != 0 {
                        0
                    } else {
                        255
                    };
                }
            }
            Kind::Samples { depth, maxval } => {
                let d = depth as usize;
                let lut = &self.lut[..];
                if maxval > 255 {
                    let px = &row[x0 * d * 2..x1 * d * 2];
                    let s = |i: usize| lut[u16::from_be_bytes([px[i * 2], px[i * 2 + 1]]) as usize];
                    expand_samples(d, x1 - x0, s, out);
                } else if maxval == 255 && d == 1 {
                    out.copy_from_slice(&row[x0..x1]);
                } else if maxval == 255 && d == 3 {
                    rgb_to_rgba(&row[x0 * 3..x1 * 3], out, false);
                } else if maxval == 255 && d == 4 {
                    out.copy_from_slice(&row[x0 * 4..x1 * 4]);
                } else {
                    let px = &row[x0 * d..x1 * d];
                    expand_samples(d, x1 - x0, |i| lut[px[i] as usize], out);
                }
            }
        }
    }
}

/// Write `n` pixels of `d` samples (sample `i` via `s`) as grey (d = 1) or RGBA.
#[inline(always)]
fn expand_samples(d: usize, n: usize, s: impl Fn(usize) -> u8, out: &mut [u8]) {
    match d {
        1 => {
            for (i, o) in out[..n].iter_mut().enumerate() {
                *o = s(i);
            }
        }
        2 => {
            for (i, o) in out.chunks_exact_mut(4).take(n).enumerate() {
                let g = s(i * 2);
                o.copy_from_slice(&[g, g, g, s(i * 2 + 1)]);
            }
        }
        3 => {
            for (i, o) in out.chunks_exact_mut(4).take(n).enumerate() {
                o.copy_from_slice(&[s(i * 3), s(i * 3 + 1), s(i * 3 + 2), 255]);
            }
        }
        _ => {
            for (i, o) in out.chunks_exact_mut(4).take(n).enumerate() {
                o.copy_from_slice(&[s(i * 4), s(i * 4 + 1), s(i * 4 + 2), s(i * 4 + 3)]);
            }
        }
    }
}

/// A decoded plain (ASCII) raster at full resolution, as a row source.
struct Full {
    w: usize,
    ch: usize,
    data: Vec<u8>,
}

impl Rows for Full {
    fn row(&self, y: u32, x0: u32, x1: u32, out: &mut [u8]) {
        let s = (y as usize * self.w + x0 as usize) * self.ch;
        out.copy_from_slice(&self.data[s..s + (x1 - x0) as usize * self.ch]);
    }
}

impl Pnm {
    /// Parse a plain raster. Returns the pixels and whether the data ran out.
    fn decode_ascii(&self, d: &[u8], cancel: Cancel) -> Result<(Full, bool)> {
        let (w, h) = (self.info.width as usize, self.info.height as usize);
        let ch = self.channels();
        // Pixels never read (truncated data) stay grey.
        let mut data = vec![0u8; w * h * ch];
        fill_grey(&mut data, ch);
        let mut plain = Plain { d, p: self.start };
        let mut next = |bit: bool| plain.next(bit);
        let mut truncated = false;
        match self.kind {
            Kind::Bits => {
                for (i, o) in data.iter_mut().enumerate() {
                    if i % (1 << 20) == 0 && cancel() {
                        return Err(Error::Cancelled);
                    }
                    match next(true) {
                        Some(v) => *o = if v != 0 { 0 } else { 255 },
                        None => {
                            truncated = true;
                            break;
                        }
                    }
                }
            }
            Kind::Samples { depth, maxval } => {
                let d = depth as usize;
                let mut px = [0u8; 4];
                'outer: for (i, o) in data.chunks_exact_mut(ch).enumerate() {
                    if i % (1 << 18) == 0 && cancel() {
                        return Err(Error::Cancelled);
                    }
                    for s in px.iter_mut().take(d) {
                        let Some(v) = next(false) else {
                            truncated = true;
                            break 'outer;
                        };
                        *s = ((v.min(maxval) * 255 + maxval / 2) / maxval) as u8;
                    }
                    match d {
                        1 => o[0] = px[0],
                        2 => o.copy_from_slice(&[px[0], px[0], px[0], px[1]]),
                        3 => o.copy_from_slice(&[px[0], px[1], px[2], 255]),
                        _ => o.copy_from_slice(&px),
                    }
                }
            }
        }
        Ok((Full { w, ch, data }, truncated))
    }

    /// Parse a plain raster of samples deeper than 8 bits at 16 bits.
    fn decode_ascii16(
        &self,
        d: &[u8],
        depth: usize,
        maxval: u32,
        cancel: Cancel,
    ) -> Result<(Full16, bool)> {
        let (w, h) = (self.info.width as usize, self.info.height as usize);
        let ch = self.channels16();
        let mut data = vec![0u16; w * h * ch];
        fill_grey16(&mut data, ch);
        let mut plain = Plain { d, p: self.start };
        let mut truncated = false;
        let mut px = [0u16; 4];
        'outer: for (i, o) in data.chunks_exact_mut(ch).enumerate() {
            if i % (1 << 18) == 0 && cancel() {
                return Err(Error::Cancelled);
            }
            for s in px.iter_mut().take(depth) {
                let Some(v) = plain.next(false) else {
                    truncated = true;
                    break 'outer;
                };
                *s = scale16(v, maxval);
            }
            expand16(depth, 1, |i| px[i], o);
        }
        Ok((Full16 { w, ch, data }, truncated))
    }
}

impl Pnm {
    /// The most pixels a byte of the file can give: a bitmap packs 8, anything else (and
    /// every plain-text file) at most one.
    fn pixels_per_byte(&self) -> u64 {
        if !self.ascii && matches!(self.kind, Kind::Bits) {
            8
        } else {
            1
        }
    }
}

impl Decode for Pnm {
    fn info(&self) -> &Info {
        &self.info
    }

    fn decode(&self, data: &[u8], opts: &Options, cancel: Cancel) -> Result<Image> {
        let (w, h) = (self.info.width, self.info.height);
        let plan = Plan::new(w, h, opts)?;
        let ch = self.channels();
        let opaque = !self.info.has_alpha;
        crate::proven(data.len(), w, h, self.pixels_per_byte())?;
        let (out, truncated) = if self.ascii {
            let (full, truncated) = self.decode_ascii(data, cancel)?;
            (
                render(&full, ch, opaque, &plan, opts.parallel, cancel)?,
                truncated,
            )
        } else {
            let rb = self.row_bytes();
            let lut = match self.kind {
                Kind::Samples { maxval, .. } => sample_lut(maxval),
                _ => Vec::new(),
            };
            let rows = BinRows {
                d: data,
                p: self,
                rb,
                lut,
            };
            let need = self.start as u64 + rb as u64 * h as u64;
            (
                render(&rows, ch, opaque, &plan, opts.parallel, cancel)?,
                (data.len() as u64) < need,
            )
        };
        Ok(Image {
            width: plan.rw,
            height: plan.rh,
            format: if ch == 1 {
                PixelFormat::Gray8
            } else {
                PixelFormat::Rgba8
            },
            data: out,
            truncated,
        })
    }

    fn wide_format(&self) -> Option<PixelFormat> {
        match self.kind {
            Kind::Samples { depth, maxval } if maxval > 255 => Some(match depth {
                1 => PixelFormat::Gray16,
                2 => PixelFormat::GrayAlpha16,
                _ => PixelFormat::Rgba16,
            }),
            _ => None,
        }
    }

    fn decode_wide(&self, data: &[u8], opts: &Options, cancel: Cancel) -> Result<Image> {
        let (Some(format), Kind::Samples { depth, maxval }) = (self.wide_format(), self.kind)
        else {
            return self.decode(data, opts, cancel);
        };
        let plan = Plan::new(self.info.width, self.info.height, opts)?;
        crate::proven(
            data.len(),
            self.info.width,
            self.info.height,
            self.pixels_per_byte(),
        )?;
        let ch = self.channels16();
        let (out, truncated) = if self.ascii {
            let (full, truncated) = self.decode_ascii16(data, depth as usize, maxval, cancel)?;
            (
                render16(&full, ch, &plan, opts.parallel, cancel)?,
                truncated,
            )
        } else {
            let rb = self.row_bytes();
            let rows = BinRows16 {
                d: data,
                start: self.start,
                rb,
                depth: depth as usize,
                maxval,
                ch,
            };
            let need = self.start as u64 + rb as u64 * self.info.height as u64;
            (
                render16(&rows, ch, &plan, opts.parallel, cancel)?,
                (data.len() as u64) < need,
            )
        };
        Ok(Image {
            width: plan.rw,
            height: plan.rh,
            format,
            data: out,
            truncated,
        })
    }
}
