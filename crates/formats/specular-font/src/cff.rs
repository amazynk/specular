//! CFF and CFF2 (`CFF ` and `CFF2` tables): INDEXes, DICTs, the charset, CID-keyed fonts
//! (FDArray and FDSelect), and Type 2 charstrings run into path commands. CFF2 outlines are
//! the default instance: `blend` keeps the default values.

use crate::names_std::CFF_STANDARD;
use crate::outline::{Builder, Outline};
use crate::read::{Reader, u16_at, u32_at};

/// Deepest subroutine nesting (the Type 2 limit).
const MAX_CALL_DEPTH: u32 = 10;
/// Operators run for one glyph at most (seac's two glyphs included).
const MAX_OPS: u32 = 1 << 20;
/// Most fonts in an FDArray.
const MAX_FDS: usize = 256;

/// An INDEX: its items' bytes.
#[derive(Clone, Copy, Default)]
pub(crate) struct Index<'a> {
    data: &'a [u8],
    offsets: &'a [u8],
    off_size: u8,
    count: u32,
}

impl<'a> Index<'a> {
    /// The INDEX at `r` (CFF2 counts are 32-bit); `r` moves past it.
    fn read(r: &mut Reader<'a>, cff2: bool) -> Option<Index<'a>> {
        let count = if cff2 { r.u32()? } else { r.u16()? as u32 };
        if count == 0 {
            return Some(Index::default());
        }
        let off_size = r.u8()?;
        if !(1..=4).contains(&off_size) {
            return None;
        }
        let offsets = r.bytes((count as usize + 1).checked_mul(off_size as usize)?)?;
        let ix = Index {
            data: &[],
            offsets,
            off_size,
            count,
        };
        let last = ix.offset(count)?;
        let data = r.bytes(last.checked_sub(1)?)?;
        Some(Index { data, ..ix })
    }

    fn offset(&self, i: u32) -> Option<usize> {
        let at = i as usize * self.off_size as usize;
        let b = self.offsets.get(at..at + self.off_size as usize)?;
        Some(b.iter().fold(0usize, |v, &x| (v << 8) | x as usize))
    }

    pub fn len(&self) -> u32 {
        self.count
    }

    pub fn get(&self, i: u32) -> Option<&'a [u8]> {
        if i >= self.count {
            return None;
        }
        let s = self.offset(i)?.checked_sub(1)?;
        let e = self.offset(i + 1)?.checked_sub(1)?;
        self.data.get(s..e)
    }
}

/// A parsed DICT: (operator, operands) in order; two-byte operators are `1200 + b`.
struct Dict {
    entries: Vec<(u16, Vec<f64>)>,
}

impl Dict {
    /// `regions` gives the region count of a vsindex for CFF2 `blend`.
    fn parse(d: &[u8], regions: &dyn Fn(u16) -> u16) -> Option<Dict> {
        let mut r = Reader::new(d);
        let mut stack: Vec<f64> = Vec::new();
        let mut entries = Vec::new();
        let mut vsindex = 0u16;
        while let Some(b) = r.u8() {
            match b {
                0..=21 | 22 | 23 | 24 | 25 | 26 | 27 | 31 => {
                    let op = if b == 12 {
                        1200 + r.u8()? as u16
                    } else {
                        b as u16
                    };
                    match op {
                        22 => {
                            vsindex = stack.last().copied().unwrap_or(0.0) as u16;
                            entries.push((op, std::mem::take(&mut stack)));
                        }
                        23 => {
                            let n = stack.pop()? as usize;
                            let k = regions(vsindex) as usize;
                            let total = n.checked_mul(k + 1)?;
                            let base = stack.len().checked_sub(total)?;
                            stack.truncate(base + n);
                        }
                        _ => entries.push((op, std::mem::take(&mut stack))),
                    }
                }
                28 => stack.push(r.i16()? as f64),
                29 => stack.push(r.i32()? as f64),
                30 => stack.push(real(&mut r)?),
                32..=246 => stack.push(b as f64 - 139.0),
                247..=250 => stack.push(((b as f64 - 247.0) * 256.0) + r.u8()? as f64 + 108.0),
                251..=254 => stack.push(-((b as f64 - 251.0) * 256.0) - r.u8()? as f64 - 108.0),
                _ => return None,
            }
            if stack.len() > 513 {
                return None;
            }
        }
        Some(Dict { entries })
    }

    fn get(&self, op: u16) -> Option<&[f64]> {
        self.entries
            .iter()
            .find(|e| e.0 == op)
            .map(|e| e.1.as_slice())
    }

    fn int(&self, op: u16) -> Option<usize> {
        let v = *self.get(op)?.first()?;
        (v >= 0.0 && v < u32::MAX as f64).then_some(v as usize)
    }

    fn pair(&self, op: u16) -> Option<(usize, usize)> {
        let v = self.get(op)?;
        let (a, b) = (*v.first()?, *v.get(1)?);
        (a >= 0.0 && b >= 0.0 && a < u32::MAX as f64 && b < u32::MAX as f64)
            .then_some((a as usize, b as usize))
    }
}

fn real(r: &mut Reader<'_>) -> Option<f64> {
    let mut s = String::new();
    'outer: loop {
        let b = r.u8()?;
        for nib in [b >> 4, b & 15] {
            match nib {
                0..=9 => s.push((b'0' + nib) as char),
                0xA => s.push('.'),
                0xB => s.push('E'),
                0xC => s.push_str("E-"),
                0xE => s.push('-'),
                0xF => break 'outer,
                _ => {}
            }
        }
        if s.len() > 64 {
            return None;
        }
    }
    Some(s.parse().unwrap_or(0.0))
}

/// A font's private data: its local subroutines.
#[derive(Clone, Copy, Default)]
struct Private<'a> {
    subrs: Index<'a>,
}

/// A parsed `CFF ` or `CFF2` table.
pub(crate) struct Cff<'a> {
    cff2: bool,
    charstrings: Index<'a>,
    gsubrs: Index<'a>,
    strings: Index<'a>,
    /// One per font of the FDArray, else the Top DICT's.
    privates: Vec<Private<'a>>,
    fd_select: Option<FdSelect<'a>>,
    charset: Charset<'a>,
    /// Region counts of the variation store's data, by vsindex.
    regions: Vec<u16>,
    pub cid: bool,
}

#[derive(Clone, Copy)]
enum FdSelect<'a> {
    F0(&'a [u8]),
    F3(&'a [u8], u32),
    F4(&'a [u8], u32),
}

#[derive(Clone, Copy)]
enum Charset<'a> {
    IsoAdobe,
    Other,
    F0(&'a [u8]),
    F1(&'a [u8], bool),
}

fn private<'a>(
    table: &'a [u8],
    (size, off): (usize, usize),
    cff2: bool,
    regions: &dyn Fn(u16) -> u16,
) -> Option<Private<'a>> {
    let d = table.get(off..off.checked_add(size)?)?;
    let dict = Dict::parse(d, regions)?;
    let subrs = match dict.int(19) {
        Some(o) => {
            let mut r = Reader::at(table, off.checked_add(o)?);
            Index::read(&mut r, cff2).unwrap_or_default()
        }
        None => Index::default(),
    };
    Some(Private { subrs })
}

fn vstore_regions(table: &[u8], at: usize) -> Vec<u16> {
    // A u16 length, then an ItemVariationStore.
    let base = at + 2;
    let Some(n) = u16_at(table, base + 6) else {
        return Vec::new();
    };
    (0..n as usize)
        .map(|i| {
            u32_at(table, base + 8 + i * 4)
                .and_then(|o| u16_at(table, base + o as usize + 4))
                .unwrap_or(0)
        })
        .collect()
}

impl<'a> Cff<'a> {
    pub fn parse(table: &'a [u8], cff2: bool) -> Option<Cff<'a>> {
        let mut r = Reader::new(table);
        let major = r.u8()?;
        let _minor = r.u8()?;
        let hdr = r.u8()? as usize;
        let (top, strings, gsubrs) = if cff2 {
            if major != 2 {
                return None;
            }
            let top_len = r.u16()? as usize;
            let top = table.get(hdr..hdr.checked_add(top_len)?)?;
            let mut g = Reader::at(table, hdr + top_len);
            let gsubrs = Index::read(&mut g, true)?;
            (top, Index::default(), gsubrs)
        } else {
            if major != 1 {
                return None;
            }
            let mut r = Reader::at(table, hdr);
            let _names = Index::read(&mut r, false)?;
            let tops = Index::read(&mut r, false)?;
            let strings = Index::read(&mut r, false)?;
            let gsubrs = Index::read(&mut r, false)?;
            (tops.get(0)?, strings, gsubrs)
        };
        // The variation store first: blends in the DICTs need its region counts.
        let pre = Dict::parse(top, &|_| 0);
        let regions = if cff2 {
            pre.as_ref()
                .and_then(|d| d.int(24))
                .map(|o| vstore_regions(table, o))
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        let rc = |i: u16| regions.get(i as usize).copied().unwrap_or(0);
        let top = Dict::parse(top, &rc)?;
        if !cff2 && top.int(1206).is_some_and(|t| t != 2) {
            return None;
        }
        let cs_off = top.int(17)?;
        let mut cr = Reader::at(table, cs_off);
        let charstrings = Index::read(&mut cr, cff2)?;
        let cid = !cff2 && top.get(1230).is_some();
        let mut privates = Vec::new();
        let mut fd_select = None;
        if let Some(fa) = top.int(1236) {
            let mut fr = Reader::at(table, fa);
            let fds = Index::read(&mut fr, cff2)?;
            for i in 0..fds.len().min(MAX_FDS as u32) {
                let fd = Dict::parse(fds.get(i)?, &rc)?;
                let p = fd
                    .pair(18)
                    .and_then(|p| private(table, p, cff2, &rc))
                    .unwrap_or_default();
                privates.push(p);
            }
            if let Some(fs) = top.int(1237) {
                let fmt = *table.get(fs)?;
                let d = table.get(fs + 1..)?;
                fd_select = match fmt {
                    0 => Some(FdSelect::F0(d)),
                    3 => Some(FdSelect::F3(d, u16_at(d, 0)? as u32)),
                    4 => Some(FdSelect::F4(d, u32_at(d, 0)?)),
                    _ => None,
                };
            }
        } else {
            let p = top
                .pair(18)
                .and_then(|p| private(table, p, cff2, &rc))
                .unwrap_or_default();
            privates.push(p);
        }
        let charset = if cff2 {
            Charset::Other
        } else {
            match top.int(15).unwrap_or(0) {
                0 => Charset::IsoAdobe,
                1 | 2 => Charset::Other,
                o => match table.get(o) {
                    Some(0) => Charset::F0(&table[o + 1..]),
                    Some(1) => Charset::F1(&table[o + 1..], false),
                    Some(2) => Charset::F1(&table[o + 1..], true),
                    _ => Charset::Other,
                },
            }
        };
        Some(Cff {
            cff2,
            charstrings,
            gsubrs,
            strings,
            privates,
            fd_select,
            charset,
            regions,
            cid,
        })
    }

    pub fn glyph_count(&self) -> u32 {
        self.charstrings.len()
    }

    fn fd_of(&self, gid: u16) -> usize {
        let g = gid as u32;
        let fd = match self.fd_select {
            None => 0,
            Some(FdSelect::F0(d)) => d.get(gid as usize).copied().unwrap_or(0) as usize,
            Some(FdSelect::F3(d, n)) => {
                let mut fd = 0;
                for i in 0..n as usize {
                    let first = u16_at(d, 2 + i * 3).unwrap_or(u16::MAX) as u32;
                    let next = u16_at(d, 2 + (i + 1) * 3).unwrap_or(u16::MAX) as u32;
                    if g >= first && g < next {
                        fd = d.get(2 + i * 3 + 2).copied().unwrap_or(0) as usize;
                        break;
                    }
                }
                fd
            }
            Some(FdSelect::F4(d, n)) => {
                let mut fd = 0;
                for i in 0..n as usize {
                    let first = u32_at(d, 4 + i * 6).unwrap_or(u32::MAX);
                    let next = u32_at(d, 4 + (i + 1) * 6).unwrap_or(u32::MAX);
                    if g >= first && g < next {
                        fd = u16_at(d, 4 + i * 6 + 4).unwrap_or(0) as usize;
                        break;
                    }
                }
                fd
            }
        };
        fd.min(self.privates.len().saturating_sub(1))
    }

    /// The SID (or CID) of glyph `gid` from the charset.
    fn sid(&self, gid: u16) -> Option<u16> {
        if gid == 0 {
            return Some(0);
        }
        match self.charset {
            Charset::IsoAdobe => (gid <= 228).then_some(gid),
            Charset::Other => None,
            Charset::F0(d) => u16_at(d, (gid as usize - 1) * 2),
            Charset::F1(d, wide) => {
                let mut g = 1u32;
                let mut at = 0usize;
                let step = if wide { 4 } else { 3 };
                while g < self.glyph_count() {
                    let first = u16_at(d, at)? as u32;
                    let left = if wide {
                        u16_at(d, at + 2)? as u32
                    } else {
                        *d.get(at + 2)? as u32
                    };
                    if (gid as u32) < g + left + 1 {
                        return u16::try_from(first + (gid as u32 - g)).ok();
                    }
                    g += left + 1;
                    at += step;
                }
                None
            }
        }
    }

    fn gid_of_sid(&self, sid: u16) -> Option<u16> {
        (0..self.glyph_count().min(u16::MAX as u32) as u16).find(|&g| self.sid(g) == Some(sid))
    }

    pub fn glyph_name(&self, gid: u16) -> Option<String> {
        if self.cid || self.cff2 {
            return None;
        }
        let sid = self.sid(gid)? as usize;
        if let Some(s) = CFF_STANDARD.get(sid) {
            return Some((*s).to_owned());
        }
        let s = self.strings.get((sid - CFF_STANDARD.len()) as u32)?;
        Some(String::from_utf8_lossy(s).into_owned())
    }

    pub fn outline(&self, gid: u16) -> Option<Outline> {
        let cs = self.charstrings.get(gid as u32)?;
        let mut st = State {
            cff: self,
            b: Builder::default(),
            stack: Vec::new(),
            x: 0.0,
            y: 0.0,
            stems: 0,
            width_done: self.cff2,
            ops: 0,
            vsindex: 0,
            seac: None,
            ended: false,
        };
        let p = self
            .privates
            .get(self.fd_of(gid))
            .copied()
            .unwrap_or_default();
        st.run(cs, &p, 0)?;
        if self.cff2 || !st.ended {
            if !self.cff2 {
                return None;
            }
            st.b.close();
        }
        if let Some((adx, ady, base, accent)) = st.seac.take() {
            let bg = self.gid_of_sid(standard_sid(base)?)?;
            let ag = self.gid_of_sid(standard_sid(accent)?)?;
            let mut path = std::mem::take(&mut st.b.path);
            for (g, dx, dy) in [(bg, 0.0, 0.0), (ag, adx, ady)] {
                let cs = self.charstrings.get(g as u32)?;
                let p = self
                    .privates
                    .get(self.fd_of(g))
                    .copied()
                    .unwrap_or_default();
                let mut s2 = State {
                    cff: self,
                    b: Builder::default(),
                    stack: Vec::new(),
                    x: dx,
                    y: dy,
                    stems: 0,
                    width_done: false,
                    ops: st.ops,
                    vsindex: 0,
                    seac: None,
                    ended: false,
                };
                s2.run(cs, &p, 0)?;
                st.ops = s2.ops;
                path.extend(s2.b.path);
            }
            return Outline::finish(path);
        }
        Outline::finish(st.b.path)
    }
}

/// The SID of standard-encoding code `c` (for seac).
fn standard_sid(c: f32) -> Option<u16> {
    let c = c as i32;
    Some(match c {
        32..=126 => (c - 31) as u16,
        161..=175 => (c - 65) as u16,
        177..=180 => (c - 66) as u16,
        182..=189 => (c - 67) as u16,
        191 => 123,
        193..=200 => (c - 69) as u16,
        202..=203 => (c - 70) as u16,
        205..=208 => (c - 71) as u16,
        225 => 138,
        227 => 139,
        232..=235 => (c - 92) as u16,
        241 => 144,
        245 => 145,
        248..=251 => (c - 102) as u16,
        _ => return None,
    })
}

fn bias(n: u32) -> i32 {
    if n < 1240 {
        107
    } else if n < 33900 {
        1131
    } else {
        32768
    }
}

struct State<'c, 'a> {
    cff: &'c Cff<'a>,
    b: Builder,
    stack: Vec<f32>,
    x: f32,
    y: f32,
    stems: u32,
    width_done: bool,
    ops: u32,
    vsindex: u16,
    seac: Option<(f32, f32, f32, f32)>,
    ended: bool,
}

impl State<'_, '_> {
    fn max_stack(&self) -> usize {
        if self.cff.cff2 { 513 } else { 48 }
    }

    /// Drops the width in front of the arguments of the first stack-clearing operator.
    fn width(&mut self, odd_means_width: bool) {
        if !self.width_done {
            self.width_done = true;
            if odd_means_width && !self.stack.is_empty() {
                self.stack.remove(0);
            }
        }
    }

    fn line(&mut self, dx: f32, dy: f32) {
        self.x += dx;
        self.y += dy;
        self.b.line_to(self.x, self.y);
    }

    fn curve(&mut self, a: f32, b: f32, c: f32, d: f32, e: f32, f: f32) {
        let (x1, y1) = (self.x + a, self.y + b);
        let (x2, y2) = (x1 + c, y1 + d);
        self.x = x2 + e;
        self.y = y2 + f;
        self.b.cubic_to(x1, y1, x2, y2, self.x, self.y);
    }

    fn run(&mut self, cs: &[u8], p: &Private<'_>, depth: u32) -> Option<()> {
        if depth > MAX_CALL_DEPTH {
            return None;
        }
        let mut r = Reader::new(cs);
        while let Some(b) = r.u8() {
            self.ops += 1;
            if self.ops > MAX_OPS {
                return None;
            }
            if self.stack.len() > self.max_stack() {
                return None;
            }
            match b {
                28 => self.stack.push(r.i16()? as f32),
                32..=246 => self.stack.push(b as f32 - 139.0),
                247..=250 => self
                    .stack
                    .push((b as f32 - 247.0) * 256.0 + r.u8()? as f32 + 108.0),
                251..=254 => self
                    .stack
                    .push(-(b as f32 - 251.0) * 256.0 - r.u8()? as f32 - 108.0),
                255 => self.stack.push(r.i32()? as f32 / 65536.0),
                1 | 3 | 18 | 23 => {
                    self.width(self.stack.len() % 2 == 1);
                    self.stems += self.stack.len() as u32 / 2;
                    self.stack.clear();
                }
                19 | 20 => {
                    self.width(self.stack.len() % 2 == 1);
                    self.stems += self.stack.len() as u32 / 2;
                    self.stack.clear();
                    r.skip(self.stems.div_ceil(8) as usize)?;
                }
                21 => {
                    self.width(self.stack.len() > 2);
                    let dy = self.stack.pop()?;
                    let dx = self.stack.pop()?;
                    self.x += dx;
                    self.y += dy;
                    self.b.move_to(self.x, self.y);
                    self.stack.clear();
                }
                22 => {
                    self.width(self.stack.len() > 1);
                    self.x += self.stack.pop()?;
                    self.b.move_to(self.x, self.y);
                    self.stack.clear();
                }
                4 => {
                    self.width(self.stack.len() > 1);
                    self.y += self.stack.pop()?;
                    self.b.move_to(self.x, self.y);
                    self.stack.clear();
                }
                5 => {
                    let s = std::mem::take(&mut self.stack);
                    for c in s.chunks_exact(2) {
                        self.line(c[0], c[1]);
                    }
                }
                6 | 7 => {
                    let s = std::mem::take(&mut self.stack);
                    let mut horiz = b == 6;
                    for &v in &s {
                        if horiz {
                            self.line(v, 0.0);
                        } else {
                            self.line(0.0, v);
                        }
                        horiz = !horiz;
                    }
                }
                8 => {
                    let s = std::mem::take(&mut self.stack);
                    for c in s.chunks_exact(6) {
                        self.curve(c[0], c[1], c[2], c[3], c[4], c[5]);
                    }
                }
                24 => {
                    let s = std::mem::take(&mut self.stack);
                    if s.len() < 8 {
                        return None;
                    }
                    let n = (s.len() - 2) / 6 * 6;
                    for c in s[..n].chunks_exact(6) {
                        self.curve(c[0], c[1], c[2], c[3], c[4], c[5]);
                    }
                    self.line(s[n], s[n + 1]);
                }
                25 => {
                    let s = std::mem::take(&mut self.stack);
                    if s.len() < 8 {
                        return None;
                    }
                    let n = s.len() - 6;
                    for c in s[..n].chunks_exact(2) {
                        self.line(c[0], c[1]);
                    }
                    let c = &s[n..];
                    self.curve(c[0], c[1], c[2], c[3], c[4], c[5]);
                }
                26 => {
                    let mut s = &std::mem::take(&mut self.stack)[..];
                    let mut dx1 = 0.0;
                    if s.len() % 2 == 1 {
                        dx1 = s[0];
                        s = &s[1..];
                    }
                    for c in s.chunks_exact(4) {
                        self.curve(dx1, c[0], c[1], c[2], 0.0, c[3]);
                        dx1 = 0.0;
                    }
                }
                27 => {
                    let mut s = &std::mem::take(&mut self.stack)[..];
                    let mut dy1 = 0.0;
                    if s.len() % 2 == 1 {
                        dy1 = s[0];
                        s = &s[1..];
                    }
                    for c in s.chunks_exact(4) {
                        self.curve(c[0], dy1, c[1], c[2], c[3], 0.0);
                        dy1 = 0.0;
                    }
                }
                30 | 31 => {
                    let s = std::mem::take(&mut self.stack);
                    let mut horiz = b == 31;
                    let mut i = 0;
                    while i + 4 <= s.len() {
                        let last = i + 4 == s.len() - 1;
                        let f = if last { s[i + 4] } else { 0.0 };
                        if horiz {
                            self.curve(s[i], 0.0, s[i + 1], s[i + 2], f, s[i + 3]);
                        } else {
                            self.curve(0.0, s[i], s[i + 1], s[i + 2], s[i + 3], f);
                        }
                        horiz = !horiz;
                        i += 4;
                        if last {
                            break;
                        }
                    }
                }
                10 | 29 => {
                    let subrs = if b == 10 { p.subrs } else { self.cff.gsubrs };
                    let i = self.stack.pop()? as i32 + bias(subrs.len());
                    let sub = subrs.get(u32::try_from(i).ok()?)?;
                    self.run(sub, p, depth + 1)?;
                    if self.ended {
                        return Some(());
                    }
                }
                11 => {
                    if self.cff.cff2 {
                        return None;
                    }
                    return Some(());
                }
                14 => {
                    if self.cff.cff2 {
                        return None;
                    }
                    self.width(self.stack.len() == 1 || self.stack.len() == 5);
                    if self.stack.len() == 4 {
                        let s = &self.stack;
                        self.seac = Some((s[0], s[1], s[2], s[3]));
                    }
                    self.b.close();
                    self.stack.clear();
                    self.ended = true;
                    return Some(());
                }
                15 if self.cff.cff2 => {
                    self.vsindex = self.stack.pop()? as u16;
                    self.stack.clear();
                }
                16 if self.cff.cff2 => {
                    let n = self.stack.pop()? as usize;
                    let k = self.cff.regions.get(self.vsindex as usize).copied()? as usize;
                    let total = n.checked_mul(k + 1)?;
                    let base = self.stack.len().checked_sub(total)?;
                    self.stack.truncate(base + n);
                }
                12 => {
                    let op = r.u8()?;
                    let s = std::mem::take(&mut self.stack);
                    match op {
                        35 if s.len() >= 12 => {
                            self.curve(s[0], s[1], s[2], s[3], s[4], s[5]);
                            self.curve(s[6], s[7], s[8], s[9], s[10], s[11]);
                        }
                        34 if s.len() >= 7 => {
                            let y0 = self.y;
                            self.curve(s[0], 0.0, s[1], s[2], s[3], 0.0);
                            let dy = y0 - self.y;
                            self.curve(s[4], 0.0, s[5], dy, s[6], 0.0);
                        }
                        36 if s.len() >= 9 => {
                            let y0 = self.y;
                            self.curve(s[0], s[1], s[2], s[3], s[4], 0.0);
                            let y1 = self.y;
                            let (x2, y2) = (self.x + s[5], y1);
                            let (x3, y3) = (x2 + s[6], y2 + s[7]);
                            let x4 = x3 + s[8];
                            self.b.cubic_to(x2, y2, x3, y3, x4, y0);
                            self.x = x4;
                            self.y = y0;
                        }
                        37 if s.len() >= 11 => {
                            let (x0, y0) = (self.x, self.y);
                            let mut dx = 0.0;
                            let mut dy = 0.0;
                            for i in 0..5 {
                                dx += s[i * 2];
                                dy += s[i * 2 + 1];
                            }
                            self.curve(s[0], s[1], s[2], s[3], s[4], s[5]);
                            let (x1, y1) = (self.x + s[6], self.y + s[7]);
                            let (x2, y2) = (x1 + s[8], y1 + s[9]);
                            let (x3, y3) = if dx.abs() > dy.abs() {
                                (x2 + s[10], y0)
                            } else {
                                (x0, y2 + s[10])
                            };
                            self.b.cubic_to(x1, y1, x2, y2, x3, y3);
                            self.x = x3;
                            self.y = y3;
                        }
                        _ => return None,
                    }
                }
                _ => return None,
            }
        }
        Some(())
    }
}
