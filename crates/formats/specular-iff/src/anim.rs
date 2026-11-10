//! IFF `ANIM`: a `FORM ANIM` of `FORM ILBM` frames. The first is a whole picture; each later
//! one has an `ANHD` header and a `DLTA` chunk that changes the bitplanes of an earlier frame
//! (and may carry a new `CMAP`).
//!
//! `ANHD`: operation, mask, width, height, x, y, absolute and relative time (in 1/60 s),
//! interleave (0 means 2: frames are double-buffered, a delta changes the frame two back),
//! padding and flags (bit 0: long data for operations 7 and 8, bit 1: XOR). Operations:
//!
//! - 0: a whole ILBM `BODY`;
//! - 5 (byte vertical delta): per plane a pointer, per byte column a count of ops: an op
//!   `0` repeats the next byte the count after it times down the column, an op under 128
//!   skips that many rows, a higher one copies `op & 127` bytes down the column;
//! - 7 (short / long vertical delta): as 5 on 16- or 32-bit columns, the op bytes and the
//!   data words in separate lists (eight pointers each);
//! - 8 (word / long vertical delta): as 5 with ops, counts and data all 16- or 32-bit.
//!
//! Other operations (1 XOR ILBM, 2 long delta, 3 short delta, 4 general vertical, `J` and
//! `l`) are refused by name.

use crate::{
    ByteRun1, Compression, Error, Form, Image, Masking, Parsed, Result, be16, be32, form_bounds,
    parse, render,
};

/// A frame of an animation and how long it shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnimFrame {
    pub image: Image,
    pub delay_ms: u32,
}

/// Where the inner `FORM ILBM` frames of an `ANIM` lie.
fn frames(data: &[u8]) -> Result<Vec<std::ops::Range<usize>>> {
    let (_, end) = form_bounds(data)?;
    let mut out = Vec::new();
    let mut pos = 12usize;
    while pos.saturating_add(12) <= end && out.len() < crate::MAX_CHUNKS {
        let len = be32(data, pos + 4) as usize;
        let stop = pos.saturating_add(8).saturating_add(len).min(end);
        if &data[pos..pos + 4] == b"FORM" && &data[pos + 8..pos + 12] == b"ILBM" {
            out.push(pos..stop);
        }
        pos = pos
            .saturating_add(8)
            .saturating_add(len)
            .saturating_add(len & 1);
    }
    Ok(out)
}

/// The first frame's `FORM` and the number of frames.
pub(crate) fn first_frame(data: &[u8]) -> Result<(&[u8], usize)> {
    let f = frames(data)?;
    let first = f.first().ok_or(Error::Malformed("ANIM without frames"))?;
    Ok((&data[first.clone()], f.len()))
}

/// Frames in an `ANIM` (0 for a still picture).
pub fn anim_frames(data: &[u8]) -> Result<usize> {
    let (kind, _) = form_bounds(data)?;
    if kind != b"ANIM" {
        return Ok(0);
    }
    Ok(frames(data)?.len())
}

/// How long frame `k` shows, in milliseconds: until the next frame's relative time (`ANHD`,
/// in 1/60 s), the last frame for its own; at least one tick, at most a minute.
fn delay_ms(data: &[u8], frames: &[std::ops::Range<usize>], k: usize) -> u32 {
    let jiffies = |i: usize| {
        let f = data.get(frames.get(i)?.clone())?;
        chunk(f, b"ANHD")
            .filter(|a| a.len() >= 18)
            .map(|a| be32(a, 14))
    };
    let j = jiffies(k + 1)
        .or_else(|| jiffies(k))
        .unwrap_or(1)
        .clamp(1, 60 * 60);
    j * 1000 / 60
}

/// The display time of every frame of an `ANIM`, in milliseconds (empty for a still).
pub fn anim_delays(data: &[u8]) -> Result<Vec<u32>> {
    let (kind, _) = form_bounds(data)?;
    if kind != b"ANIM" {
        return Ok(Vec::new());
    }
    let f = frames(data)?;
    Ok((0..f.len()).map(|k| delay_ms(data, &f, k)).collect())
}

/// The chunk `id` of a frame `FORM`.
fn chunk<'a>(form: &'a [u8], id: &[u8; 4]) -> Option<&'a [u8]> {
    let mut pos = 12usize;
    while pos + 8 <= form.len() {
        let len = be32(form, pos + 4) as usize;
        let start = pos + 8;
        let stop = start.saturating_add(len).min(form.len());
        if &form[pos..pos + 4] == id {
            return Some(&form[start..stop]);
        }
        pos = stop.saturating_add(len & 1);
    }
    None
}

/// Plays an `ANIM` frame by frame. It keeps no borrow of the file: each call is handed the
/// same bytes [`Player::new`] was.
pub struct Player {
    frames: Vec<std::ops::Range<usize>>,
    first: crate::Info,
    cmap: Vec<u8>,
    /// Double-buffered planes (an ACBM body each).
    bufs: [Vec<u8>; 2],
    next: usize,
    /// Plane row bytes, rows, planes.
    row: usize,
    height: usize,
    planes: usize,
}

impl Player {
    pub fn new(data: &[u8]) -> Result<Player> {
        let frames = frames(data)?;
        let at = frames
            .first()
            .ok_or(Error::Malformed("ANIM without frames"))?;
        let first = parse(&data[at.clone()])?;
        if first.info.form != Form::Ilbm {
            return Err(Error::Unsupported("ANIM of non-ILBM frames".into()));
        }
        let row = (first.info.width as usize).div_ceil(16) * 2;
        let planes = usize::from(first.info.planes);
        let height = first.info.height as usize;
        Ok(Player {
            cmap: first.cmap.to_vec(),
            frames,
            first: first.info,
            bufs: [Vec::new(), Vec::new()],
            next: 0,
            row,
            height,
            planes,
        })
    }

    /// Frames in the animation.
    pub fn len(&self) -> usize {
        self.frames.len()
    }

    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    /// Starts again from the first frame.
    pub fn rewind(&mut self) {
        self.next = 0;
    }

    /// The next frame of `data` (the file given to [`Player::new`]), `None` after the last.
    pub fn next_frame(&mut self, data: &[u8]) -> Option<Result<AnimFrame>> {
        let k = self.next;
        let at = self.frames.get(k)?.clone();
        self.next += 1;
        let Some(form) = data.get(at) else {
            return Some(Err(Error::Truncated("ANIM frame")));
        };
        Some(self.frame(k, form, data))
    }

    fn frame(&mut self, k: usize, form: &[u8], data: &[u8]) -> Result<AnimFrame> {
        let plane_bytes = self.row * self.height;
        let size = plane_bytes * self.planes;
        let anhd = chunk(form, b"ANHD");
        if k == 0 {
            let p = parse(form)?;
            let planes = body_planes(&p, self.row, self.height, self.planes)?;
            self.bufs = [planes.clone(), planes];
            self.cmap = p.cmap.to_vec();
        } else {
            let a = anhd.ok_or(Error::Malformed("ANIM frame without ANHD"))?;
            if a.len() < 24 {
                return Err(Error::Truncated("ANHD"));
            }
            let op = a[0];
            let interleave = match a[18] {
                0 => 2,
                n => usize::from(n),
            };
            let bits = be32(a, 20);
            let target = k % 2;
            if interleave == 1 {
                let prev = self.bufs[(k + 1) % 2].clone();
                self.bufs[target] = prev;
            }
            if let Some(c) = chunk(form, b"CMAP") {
                self.cmap = c.to_vec();
            }
            let buf = &mut self.bufs[target];
            buf.resize(size, 0);
            match op {
                0 => {
                    let p = parse(form)?;
                    *buf = body_planes(&p, self.row, self.height, self.planes)?;
                }
                3 | 5 | 7 | 8 | 74 | 108 => {
                    let d =
                        chunk(form, b"DLTA").ok_or(Error::Malformed("ANIM frame without DLTA"))?;
                    let g = Geometry {
                        row: self.row,
                        height: self.height,
                        planes: self.planes,
                        xor: bits & 2 != 0,
                    };
                    let n = if bits & 1 != 0 { 4 } else { 2 };
                    let width = self.first.width as usize;
                    match op {
                        3 => short_delta(d, buf, &g),
                        5 => byte_vertical(d, buf, &g),
                        7 => split_vertical(d, buf, &g, n),
                        8 => word_vertical(d, buf, &g, n),
                        74 => delta_j(d, buf, &g, width),
                        // The flags of these files hold junk: no XOR here.
                        _ => delta_l(d, buf, &Geometry { xor: false, ..g }, width, n == 2),
                    }
                }
                1 => return Err(Error::Unsupported("ANIM operation 1 (XOR ILBM)".into())),
                n => return Err(Error::Unsupported(format!("ANIM operation {n}"))),
            }
        }
        let delay = delay_ms(data, &self.frames, k);
        let mut info = self.first.clone();
        info.form = Form::Acbm;
        info.compression = Compression::None;
        info.compressed = false;
        info.masking = match info.masking {
            Masking::Mask => Masking::None,
            m => m,
        };
        info.anim_frames = 0;
        let p = Parsed {
            info,
            cmap: &[],
            body: &self.bufs[k % 2],
            ranges: Vec::new(),
            deep: None,
            lines: None,
        };
        let image = render(p, Some(&self.cmap))?;
        Ok(AnimFrame {
            image,
            delay_ms: delay,
        })
    }
}

/// The planes of an ILBM frame's `BODY` (raw, ByteRun1 or VDAT), laid out as an ACBM body
/// (the mask plane dropped).
fn body_planes(p: &Parsed, row: usize, height: usize, planes: usize) -> Result<Vec<u8>> {
    let mask = usize::from(p.info.masking == Masking::Mask);
    let plane_bytes = row * height;
    let mut out = vec![0u8; plane_bytes * planes];
    match p.info.compression {
        Compression::Vdat => {
            let v = crate::vdat::decode(p.body, p.info.width as usize, height, planes + mask);
            out.copy_from_slice(&v[..plane_bytes * planes]);
        }
        Compression::None | Compression::ByteRun1 => {
            let mut line = vec![0u8; row * (planes + mask)];
            let mut rle = ByteRun1::new(p.body);
            for y in 0..height {
                let ok = if p.info.compression == Compression::ByteRun1 {
                    rle.fill(&mut line)
                } else {
                    match p.body.get(y * line.len()..(y + 1) * line.len()) {
                        Some(s) => {
                            line.copy_from_slice(s);
                            true
                        }
                        None => false,
                    }
                };
                if !ok {
                    break;
                }
                for pl in 0..planes {
                    let at = pl * plane_bytes + y * row;
                    out[at..at + row].copy_from_slice(&line[pl * row..(pl + 1) * row]);
                }
            }
        }
        c => return Err(Error::Unsupported(format!("ANIM frame with {}", c.name()))),
    }
    Ok(out)
}

#[derive(Clone, Copy)]
struct Geometry {
    row: usize,
    height: usize,
    planes: usize,
    xor: bool,
}

impl Geometry {
    /// Stores `v` (`n` bytes) at column byte `col` of row `y` of plane `p`.
    fn put(&self, buf: &mut [u8], p: usize, y: usize, col: usize, v: &[u8]) {
        if y >= self.height || col + v.len() > self.row {
            return;
        }
        let at = (p * self.height + y) * self.row + col;
        for (d, &s) in buf[at..at + v.len()].iter_mut().zip(v) {
            *d = if self.xor { *d ^ s } else { s };
        }
    }
}

/// The eight plane pointers at `base` of a `DLTA` (offsets from its start; 0: unchanged).
fn pointers(d: &[u8], base: usize) -> [usize; 8] {
    let mut p = [0usize; 8];
    for (i, v) in p.iter_mut().enumerate() {
        if let Some(b) = d.get(base + 4 * i..base + 4 * i + 4) {
            *v = u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as usize;
        }
    }
    p
}

/// Operation 5.
fn byte_vertical(d: &[u8], buf: &mut [u8], g: &Geometry) {
    let at = |i: usize| d.get(i).copied();
    for (p, &ptr) in pointers(d, 0).iter().enumerate().take(g.planes) {
        if ptr == 0 {
            continue;
        }
        let mut i = ptr;
        for col in 0..g.row {
            let Some(ops) = at(i) else { return };
            i += 1;
            let mut y = 0usize;
            for _ in 0..ops {
                let Some(op) = at(i) else { return };
                i += 1;
                if op == 0 {
                    let (Some(n), Some(v)) = (at(i), at(i + 1)) else {
                        return;
                    };
                    i += 2;
                    for _ in 0..n {
                        g.put(buf, p, y, col, &[v]);
                        y += 1;
                    }
                } else if op < 0x80 {
                    y += usize::from(op);
                } else {
                    for _ in 0..op & 0x7f {
                        let Some(v) = at(i) else { return };
                        i += 1;
                        g.put(buf, p, y, col, &[v]);
                        y += 1;
                    }
                }
            }
        }
    }
}

/// Operation 7: op bytes and `n`-byte data in separate lists.
fn split_vertical(d: &[u8], buf: &mut [u8], g: &Geometry, n: usize) {
    let ops_at = pointers(d, 0);
    let data_at = pointers(d, 32);
    let cols = g.row / n;
    for p in 0..g.planes.min(8) {
        if ops_at[p] == 0 {
            continue;
        }
        let (mut i, mut j) = (ops_at[p], data_at[p]);
        let mut take = || -> Option<&[u8]> {
            let v = d.get(j..j + n)?;
            j += n;
            Some(v)
        };
        for col in 0..cols {
            let Some(&count) = d.get(i) else { break };
            i += 1;
            let mut y = 0usize;
            for _ in 0..count {
                let Some(&op) = d.get(i) else { return };
                i += 1;
                if op == 0 {
                    let Some(&rep) = d.get(i) else { return };
                    i += 1;
                    let Some(v) = take() else { return };
                    for _ in 0..rep {
                        g.put(buf, p, y, col * n, v);
                        y += 1;
                    }
                } else if op < 0x80 {
                    y += usize::from(op);
                } else {
                    for _ in 0..op & 0x7f {
                        let Some(v) = take() else { return };
                        g.put(buf, p, y, col * n, v);
                        y += 1;
                    }
                }
            }
        }
    }
}

/// Operation 8: ops, counts and data all `n` bytes.
fn word_vertical(d: &[u8], buf: &mut [u8], g: &Geometry, n: usize) {
    let top = 1u32 << (8 * n - 1);
    let cols = g.row / n;
    for (p, &ptr) in pointers(d, 0).iter().enumerate().take(g.planes) {
        if ptr == 0 {
            continue;
        }
        let mut i = ptr;
        let mut num = || -> Option<u32> {
            let v = d.get(i..i + n)?;
            i += n;
            Some(if n == 2 {
                u32::from(be16(v, 0))
            } else {
                be32(v, 0)
            })
        };
        for col in 0..cols {
            let Some(ops) = num() else { return };
            let mut y = 0usize;
            for _ in 0..ops.min(1 << 16) {
                let Some(op) = num() else { return };
                if op == 0 {
                    let (Some(c), Some(v)) = (num(), num()) else {
                        return;
                    };
                    let v = v.to_be_bytes();
                    // Rows past the bottom are dropped: repeat at most that far.
                    for _ in 0..c.min(g.height as u32) {
                        g.put(buf, p, y, col * n, &v[4 - n..]);
                        y += 1;
                    }
                } else if op & top == 0 {
                    y += op as usize;
                } else {
                    for _ in 0..(op & !top).min(1 << 16) {
                        let Some(v) = num() else { return };
                        g.put(buf, p, y, col * n, &v.to_be_bytes()[4 - n..]);
                        y += 1;
                    }
                }
            }
        }
    }
}

/// Operation 3 (short delta): per plane a pointer to (offset, data) words, ending at
/// `0xFFFF`. A non-negative offset moves that many words on and stores the data word; a
/// negative one moves `-offset - 2` words on, then a count of words follows, each stored one
/// word further on. Positions run through the plane in memory order.
fn short_delta(d: &[u8], buf: &mut [u8], g: &Geometry) {
    let word = |i: usize| d.get(i..i + 2).map(|w| [w[0], w[1]]);
    for (p, &ptr) in pointers(d, 0).iter().enumerate().take(g.planes) {
        if ptr == 0 {
            continue;
        }
        let (mut i, mut pos) = (ptr, 0usize);
        let mut store = |pos: usize, w: [u8; 2]| g.put(buf, p, pos / g.row, pos % g.row, &w);
        while let Some(o) = word(i) {
            if o == [0xff, 0xff] {
                break;
            }
            i += 2;
            let off = i16::from_be_bytes(o);
            if off >= 0 {
                let Some(v) = word(i) else { break };
                i += 2;
                pos += off as usize * 2;
                store(pos, v);
            } else {
                let Some(n) = word(i) else { break };
                i += 2;
                pos += 2 * (-(i32::from(off) + 2)).max(0) as usize;
                for _ in 0..u16::from_be_bytes(n) {
                    let Some(v) = word(i) else { return };
                    i += 2;
                    pos += 2;
                    store(pos, v);
                }
            }
            if pos > g.row * g.height {
                break;
            }
        }
    }
}

/// Where operations J and l place a byte offset: the row and byte column in rows of
/// `(width + 7) / 8` bytes, or, for J on pictures narrower than 320 pixels, in 40-byte rows
/// with the picture centred.
fn place(off: usize, width: usize, centred: bool) -> (usize, Option<usize>) {
    if centred && width < 320 {
        let margin = (320 - width) / 16;
        (off / 40, (off % 40).checked_sub(margin))
    } else {
        let pb = width.div_ceil(8).max(1);
        (off / pb, Some(off % pb))
    }
}

/// Operation J (74): blocks until type 0. Type 1: XOR flag, rows, groups; each group an
/// offset, then for each row a byte for each plane. Type 2: XOR flag, rows, bytes per row,
/// groups; each group an offset, then for each row and plane that many bytes. Groups end on
/// a 16-bit boundary.
fn delta_j(d: &[u8], buf: &mut [u8], g: &Geometry, width: usize) {
    let word = |i: usize| {
        d.get(i..i + 2)
            .map(|w| usize::from(u16::from_be_bytes([w[0], w[1]])))
    };
    let mut i = 0usize;
    let mut blocks = 0;
    while let Some(kind) = word(i) {
        i += 2;
        blocks += 1;
        if kind == 0 || blocks > 1 << 16 {
            return;
        }
        let (Some(flag), Some(rows)) = (word(i), word(i + 2)) else {
            return;
        };
        let geo = Geometry {
            xor: flag != 0,
            ..*g
        };
        let (bytes, groups) = match kind {
            1 => (1, word(i + 4)),
            2 => (word(i + 4).unwrap_or(0), word(i + 6)),
            _ => return,
        };
        i += if kind == 1 { 6 } else { 8 };
        let Some(groups) = groups else { return };
        for _ in 0..groups {
            let Some(off) = word(i) else { return };
            i += 2;
            let n = rows * bytes * g.planes;
            let Some(data) = d.get(i..i + n) else { return };
            i += n + (n & 1);
            let (y, col) = place(off, width, true);
            let Some(col) = col else { continue };
            for (k, v) in data.chunks_exact(bytes.max(1)).enumerate() {
                let (r, p) = (k / g.planes, k % g.planes);
                geo.put(buf, p, y + r, col, v);
            }
        }
    }
}

/// Operation l (108): per plane a data pointer and an offset-list pointer (in words). The
/// list holds (offset in words, count) pairs up to `0xFFFF`: a negative count repeats the
/// next data word `-count` times, a positive one copies that many data words. Short data
/// goes down the rows, long data along the row.
fn delta_l(d: &[u8], buf: &mut [u8], g: &Geometry, width: usize, short: bool) {
    let word = |i: usize| d.get(i..i + 2).map(|w| u16::from_be_bytes([w[0], w[1]]));
    let data_at = pointers(d, 0);
    let list_at = pointers(d, 32);
    for p in 0..g.planes.min(8) {
        if data_at[p] == 0 {
            continue;
        }
        let (mut di, mut li) = (2 * data_at[p], 2 * list_at[p]);
        let mut written = 0usize;
        while let (Some(off), Some(cnt)) = (word(li), word(li + 2)) {
            if off == 0xffff {
                break;
            }
            li += 4;
            let (mut y, col) = place(2 * usize::from(off), width, false);
            let Some(mut col) = col else { continue };
            let cnt = cnt as i16;
            let n = usize::from(cnt.unsigned_abs());
            written += n;
            if written > g.row * g.height {
                return;
            }
            let rep = if cnt < 0 {
                let Some(v) = word(di) else { return };
                di += 2;
                Some(v)
            } else {
                None
            };
            for _ in 0..n {
                let v = match rep {
                    Some(v) => v,
                    None => {
                        let Some(v) = word(di) else { return };
                        di += 2;
                        v
                    }
                };
                g.put(buf, p, y, col, &v.to_be_bytes());
                if short {
                    y += 1;
                } else {
                    col += 2;
                }
            }
        }
    }
}
