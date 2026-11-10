//! APV (Advanced Professional Video, `apv1`), the intra-only professional codec of the IETF
//! draft-lim-apv (Samsung, now the Academy Software Foundation's OpenAPV): an own decoder
//! written from the draft.
//!
//! An access unit is the signature `aPv1` and primitive bitstream units (PBUs), each a
//! 32-bit size, a type, a group id and its body. The primary frame (type 1) is decoded;
//! non-primary, preview, depth and alpha frames are skipped (counted in [`AccessUnit`]).
//!
//! A frame header gives the profile, level and band, the size, the chroma format (4:0:0,
//! 4:2:2, 4:4:4, 4:4:4:4), the bit depth (10 to 16), the colour description, optional
//! quantisation matrices and the tile grid. Each tile holds, per component, 16×16
//! macroblocks of 8×8 blocks: a DC difference from the previous block's DC and the AC
//! levels as zero runs and levels in zig-zag order, all in an adaptive Golomb-Rice-like
//! code whose parameter follows the previous values. Blocks are dequantised
//! (`level × matrix × levelScale[qP % 6] << qP / 6`) and inverse transformed with the
//! HEVC-style 8-point integer DCT, columns then rows. Tiles decode in parallel.
//!
//! The raw `.apv` file is a 32-bit access-unit size before each access unit.

use rayon::prelude::*;

/// Largest frame accepted (luma samples).
pub const MAX_SAMPLES: u64 = 1 << 28;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Truncated(&'static str),
    Malformed(String),
    Unsupported(String),
    TooLarge,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Truncated(s) => write!(f, "truncated: {s}"),
            Error::Malformed(s) => write!(f, "malformed: {s}"),
            Error::Unsupported(s) => write!(f, "unsupported: {s}"),
            Error::TooLarge => f.write_str("frame too large"),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

/// The access-unit signature.
pub const SIGNATURE: &[u8; 4] = b"aPv1";

/// Whether `d` starts an access unit.
pub fn is_access_unit(d: &[u8]) -> bool {
    d.starts_with(SIGNATURE)
}

/// Whether `d` is a raw `.apv` file: an access-unit size, then the signature.
pub fn is_raw_file(d: &[u8]) -> bool {
    d.len() >= 8 && d[4..8] == *SIGNATURE && u32::from_be_bytes([d[0], d[1], d[2], d[3]]) >= 8
}

/// The access units of a raw `.apv` file: (offset, size) of each, after its size word.
pub fn raw_units(d: &[u8]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut at = 0;
    while at + 8 <= d.len() {
        let n = u32::from_be_bytes([d[at], d[at + 1], d[at + 2], d[at + 3]]) as usize;
        if n < 4 || d[at + 4..at + 8] != *SIGNATURE {
            break;
        }
        let end = (at + 4).saturating_add(n).min(d.len());
        out.push((at + 4, end - at - 4));
        at = at + 4 + n;
    }
    out
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Chroma {
    /// 4:0:0 (luma only).
    Mono,
    Yuv422,
    Yuv444,
    /// 4:4:4 with a fourth (alpha or other) component.
    Yuv4444,
}

impl Chroma {
    pub fn components(self) -> usize {
        match self {
            Chroma::Mono => 1,
            Chroma::Yuv422 | Chroma::Yuv444 => 3,
            Chroma::Yuv4444 => 4,
        }
    }
    /// Horizontal chroma subsampling.
    pub fn sub_w(self) -> usize {
        if self == Chroma::Yuv422 { 2 } else { 1 }
    }
    pub fn name(self) -> &'static str {
        match self {
            Chroma::Mono => "4:0:0",
            Chroma::Yuv422 => "4:2:2",
            Chroma::Yuv444 => "4:4:4",
            Chroma::Yuv4444 => "4:4:4:4",
        }
    }
}

/// `frame_info()`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameInfo {
    pub profile: u8,
    pub level: u8,
    pub band: u8,
    pub width: u32,
    pub height: u32,
    pub chroma: Chroma,
    pub bit_depth: u8,
    pub capture_time_distance: u8,
}

impl FrameInfo {
    /// The profile's name (draft section 9.3).
    pub fn profile_name(&self) -> String {
        match self.profile {
            33 => "422-10".into(),
            44 => "422-12".into(),
            55 => "444-10".into(),
            66 => "444-12".into(),
            77 => "4444-10".into(),
            88 => "4444-12".into(),
            99 => "400-10".into(),
            p => format!("profile {p}"),
        }
    }
    /// The level as written (level_idc is 30 × the level).
    pub fn level_name(&self) -> String {
        let l = f64::from(self.level) / 30.0;
        format!("{l}")
    }
}

/// The colour description, when the frame header has one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Colour {
    pub primaries: u8,
    pub transfer: u8,
    pub matrix: u8,
    pub full_range: bool,
}

/// A decoded frame: each component's samples, row by row (chroma planes at their size).
#[derive(Clone, Debug, PartialEq)]
pub struct Picture {
    pub info: FrameInfo,
    pub colour: Option<Colour>,
    pub planes: Vec<Vec<u16>>,
    /// Tiles that could not be decoded (left mid-grey).
    pub damaged_tiles: usize,
}

impl Picture {
    /// The width and height of component `c`.
    pub fn plane_size(&self, c: usize) -> (usize, usize) {
        let (w, h) = (self.info.width as usize, self.info.height as usize);
        if c == 0 || c == 3 {
            (w, h)
        } else {
            (w.div_ceil(self.info.chroma.sub_w()), h)
        }
    }
}

/// What an access unit holds.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AccessUnit {
    /// The primary frame's PBU body (from its frame header).
    pub primary: Option<(usize, usize)>,
    /// Other frames (non-primary, preview, depth, alpha) by PBU type.
    pub others: Vec<u8>,
    pub metadata: usize,
}

struct Bits<'a> {
    d: &'a [u8],
    p: usize,
}

impl<'a> Bits<'a> {
    fn new(d: &'a [u8]) -> Self {
        Bits { d, p: 0 }
    }
    #[inline]
    fn bit(&mut self) -> Option<u32> {
        let b = *self.d.get(self.p >> 3)?;
        let v = (b >> (7 - (self.p & 7))) & 1;
        self.p += 1;
        Some(u32::from(v))
    }
    #[inline]
    fn u(&mut self, n: u32) -> Option<u32> {
        let mut v = 0u32;
        for _ in 0..n {
            v = (v << 1) | self.bit()?;
        }
        Some(v)
    }
    fn align(&mut self) {
        self.p = self.p.div_ceil(8) * 8;
    }
    fn byte_pos(&self) -> usize {
        self.p / 8
    }
    /// The variable length code of draft section 7.1.4 with parameter `k`.
    #[inline]
    fn vlc(&mut self, k: u32) -> Option<u32> {
        let mut k = k;
        let mut v = 0u32;
        let golomb = if self.bit()? == 1 {
            false
        } else if self.bit()? == 0 {
            v += 1 << k;
            false
        } else {
            v += 2 << k;
            true
        };
        if golomb {
            loop {
                if self.bit()? == 1 {
                    break;
                }
                v = v.checked_add(1 << k)?;
                k += 1;
                if k > 24 {
                    return None;
                }
            }
        }
        if k > 0 {
            v = v.checked_add(self.u(k)?)?;
        }
        Some(v)
    }
}

/// Lists an access unit's PBUs (`au` starts with the signature).
pub fn access_unit(au: &[u8]) -> Result<AccessUnit> {
    if !is_access_unit(au) {
        return Err(Error::Malformed("no aPv1 signature".into()));
    }
    let mut out = AccessUnit::default();
    let mut at = 4;
    while at + 8 <= au.len() {
        let size = u32::from_be_bytes([au[at], au[at + 1], au[at + 2], au[at + 3]]) as usize;
        if size < 4 {
            return Err(Error::Malformed(format!("PBU size {size}")));
        }
        let body = at + 4;
        let end = body.saturating_add(size).min(au.len());
        let kind = au[body];
        match kind {
            1 if out.primary.is_none() && au[body + 3] == 0 => {
                out.primary = Some((body + 4, end - body - 4));
            }
            1 | 2 | 25..=27 => out.others.push(kind),
            66 => out.metadata += 1,
            _ => {}
        }
        at = body.saturating_add(size);
    }
    Ok(out)
}

fn frame_info(b: &mut Bits) -> Result<FrameInfo> {
    let short = || Error::Truncated("APV frame info");
    let mut u = |n| b.u(n).ok_or_else(short);
    let (profile, level, band) = (u(8)? as u8, u(8)? as u8, u(3)? as u8);
    u(5)?;
    let (width, height) = (u(24)?, u(24)?);
    let chroma = match u(4)? {
        0 => Chroma::Mono,
        2 => Chroma::Yuv422,
        3 => Chroma::Yuv444,
        4 => Chroma::Yuv4444,
        c => {
            return Err(Error::Unsupported(format!(
                "APV chroma format {c} is not supported (4:0:0, 4:2:2, 4:4:4 and 4:4:4:4 are)"
            )));
        }
    };
    let bit_depth = u(4)? as u8 + 8;
    let capture_time_distance = u(8)? as u8;
    u(8)?;
    Ok(FrameInfo {
        profile,
        level,
        band,
        width,
        height,
        chroma,
        bit_depth,
        capture_time_distance,
    })
}

/// The frame header, parsed.
struct Header {
    info: FrameInfo,
    colour: Option<Colour>,
    /// Per component, the matrix in raster order (y × 8 + x).
    qmatrix: Vec<[u8; 64]>,
    col_starts: Vec<usize>,
    row_starts: Vec<usize>,
    /// Bytes of the frame header.
    len: usize,
}

fn header(d: &[u8]) -> Result<Header> {
    let mut b = Bits::new(d);
    let short = || Error::Truncated("APV frame header");
    let info = frame_info(&mut b)?;
    if !(10..=16).contains(&info.bit_depth) {
        return Err(Error::Unsupported(format!(
            "{}-bit APV is not supported (10 to 16 bits are)",
            info.bit_depth
        )));
    }
    if info.width == 0 || info.height == 0 {
        return Err(Error::Malformed("empty frame".into()));
    }
    if u64::from(info.width) * u64::from(info.height) > MAX_SAMPLES {
        return Err(Error::TooLarge);
    }
    b.u(8).ok_or_else(short)?;
    let colour = if b.u(1).ok_or_else(short)? == 1 {
        Some(Colour {
            primaries: b.u(8).ok_or_else(short)? as u8,
            transfer: b.u(8).ok_or_else(short)? as u8,
            matrix: b.u(8).ok_or_else(short)? as u8,
            full_range: b.u(1).ok_or_else(short)? == 1,
        })
    } else {
        None
    };
    let n = info.chroma.components();
    let mut qmatrix = vec![[16u8; 64]; n];
    if b.u(1).ok_or_else(short)? == 1 {
        for m in &mut qmatrix {
            // q_matrix[i][x][y], x fastest within each row y.
            for y in 0..8 {
                for x in 0..8 {
                    let v = b.u(8).ok_or_else(short)? as u8;
                    if v == 0 {
                        return Err(Error::Malformed("zero in a quantisation matrix".into()));
                    }
                    m[y * 8 + x] = v;
                }
            }
        }
    }
    let tw = b.u(20).ok_or_else(short)? as usize;
    let th = b.u(20).ok_or_else(short)? as usize;
    if tw == 0 || th == 0 {
        return Err(Error::Malformed("empty tiles".into()));
    }
    let (wmb, hmb) = (
        (info.width as usize).div_ceil(16),
        (info.height as usize).div_ceil(16),
    );
    let starts = |n: usize, step: usize| {
        let mut v: Vec<usize> = (0..n).step_by(step).map(|m| m * 16).collect();
        v.push(n * 16);
        v
    };
    let (col_starts, row_starts) = (starts(wmb, tw), starts(hmb, th));
    let tiles = (col_starts.len() - 1) * (row_starts.len() - 1);
    if tiles > 1 << 16 {
        return Err(Error::Malformed(format!("{tiles} tiles")));
    }
    if b.u(1).ok_or_else(short)? == 1 {
        for _ in 0..tiles {
            b.u(32).ok_or_else(short)?;
        }
    }
    b.u(8).ok_or_else(short)?;
    b.align();
    Ok(Header {
        info,
        colour,
        qmatrix,
        col_starts,
        row_starts,
        len: b.byte_pos(),
    })
}

/// The zig-zag scan of an 8×8 block: scan position to raster position.
fn zigzag() -> [u8; 64] {
    let mut z = [0u8; 64];
    let (w, h) = (8i32, 8i32);
    let mut pos = 1;
    for line in 1..(w + h - 1) {
        if line % 2 == 1 {
            let (mut x, mut y) = (line.min(w - 1), 0.max(line - (w - 1)));
            while x >= 0 && y < h {
                z[pos] = (y * w + x) as u8;
                pos += 1;
                x -= 1;
                y += 1;
            }
        } else {
            let (mut y, mut x) = (line.min(h - 1), 0.max(line - (h - 1)));
            while y >= 0 && x < w {
                z[pos] = (y * w + x) as u8;
                pos += 1;
                x += 1;
                y -= 1;
            }
        }
    }
    z
}

const LEVEL_SCALE: [i64; 6] = [40, 45, 51, 57, 64, 71];

const DCT8: [[i32; 8]; 8] = [
    [64, 64, 64, 64, 64, 64, 64, 64],
    [89, 75, 50, 18, -18, -50, -75, -89],
    [84, 35, -35, -84, -84, -35, 35, 84],
    [75, -18, -89, -50, 50, 89, 18, -75],
    [64, -64, -64, 64, 64, -64, -64, 64],
    [50, -89, 18, 75, -75, -18, 89, -50],
    [35, -84, 84, -35, -35, 84, -84, 35],
    [18, -50, 75, -89, 89, -75, 50, -18],
];

/// One 8-point inverse transform: `y[i] = Σ basis[j][i] · x[j]`.
#[inline]
fn idct8(x: &[i32; 8]) -> [i64; 8] {
    let mut y = [0i64; 8];
    for (j, &xj) in x.iter().enumerate() {
        if xj == 0 {
            continue;
        }
        for (i, yi) in y.iter_mut().enumerate() {
            *yi += i64::from(DCT8[j][i]) * i64::from(xj);
        }
    }
    y
}

/// One component of one tile: its samples, `w × h`.
struct TilePlane {
    w: usize,
    h: usize,
    px: Vec<u16>,
}

/// Decodes the macroblocks of one component of a tile into `out` (`w × h`, block-aligned).
#[allow(clippy::too_many_arguments)]
fn tile_component(
    data: &[u8],
    blk_w: usize,
    blk_h: usize,
    mbs: (usize, usize),
    qp: u32,
    qm: &[u8; 64],
    depth: u32,
    scan: &[u8; 64],
    out: &mut TilePlane,
) -> Option<()> {
    let mut b = Bits::new(data);
    let (mut prev_dc, mut prev_dc_diff, mut prev_first_ac) = (0i32, 20u32, 0u32);
    // Dequantisation: (level × qm × scale << qP/6 + round) >> shift, with shift = depth − 2.
    let shift = depth - 2;
    let scale = LEVEL_SCALE[(qp % 6) as usize] << (qp / 6);
    let mid = 1i64 << (depth - 1);
    let maxv = (1i64 << depth) - 1;
    let out_shift = 20 - depth;
    for my in 0..mbs.1 {
        for mx in 0..mbs.0 {
            for by in (0..blk_h).step_by(8) {
                for bx in (0..blk_w).step_by(8) {
                    let mut coef = [0i32; 64];
                    let k = (prev_dc_diff >> 1).min(5);
                    let diff = b.vlc(k)?;
                    // Damaged data can push the prediction past i32: it wraps (as release
                    // builds always did) rather than panic.
                    let dc = if diff != 0 && b.bit()? == 1 {
                        prev_dc.wrapping_sub(diff as i32)
                    } else {
                        prev_dc.wrapping_add(diff as i32)
                    };
                    coef[0] = dc;
                    prev_dc = dc;
                    prev_dc_diff = diff;
                    // AC levels.
                    let mut pos = 1usize;
                    let mut first = true;
                    let mut prev_level = prev_first_ac;
                    let mut prev_run = 0u32;
                    while pos < 64 {
                        let run = b.vlc((prev_run >> 2).min(2))?;
                        pos = pos.checked_add(run as usize)?;
                        prev_run = run;
                        if pos < 64 {
                            let m1 = b.vlc((prev_level >> 2).min(4))?;
                            let neg = b.bit()? == 1;
                            let level = m1.checked_add(1)? as i32;
                            coef[scan[pos] as usize] = if neg { -level } else { level };
                            pos += 1;
                            prev_level = m1 + 1;
                            if first {
                                first = false;
                                prev_first_ac = prev_level;
                            }
                        } else if pos > 64 {
                            return None;
                        }
                    }
                    // Dequantise, then columns, rows.
                    let mut d = [0i32; 64];
                    for (i, (dv, &c)) in d.iter_mut().zip(&coef).enumerate() {
                        if c != 0 {
                            let v = (i64::from(c) * i64::from(qm[i]) * scale
                                + (1i64 << (shift - 1)))
                                >> shift;
                            *dv = v.clamp(-32768, 32767) as i32;
                        }
                    }
                    let mut g = [0i32; 64];
                    for x in 0..8 {
                        let col: [i32; 8] = std::array::from_fn(|y| d[y * 8 + x]);
                        let e = idct8(&col);
                        for y in 0..8 {
                            g[y * 8 + x] = ((e[y] + 64) >> 7) as i32;
                        }
                    }
                    let ox = mx * blk_w + bx;
                    let oy = my * blk_h + by;
                    for y in 0..8 {
                        let row: [i32; 8] = std::array::from_fn(|x| g[y * 8 + x]);
                        let r = idct8(&row);
                        let line = &mut out.px[(oy + y) * out.w + ox..(oy + y) * out.w + ox + 8];
                        for (o, &v) in line.iter_mut().zip(&r) {
                            let s = ((v + (1i64 << (out_shift - 1))) >> out_shift) + mid;
                            *o = s.clamp(0, maxv) as u16;
                        }
                    }
                }
            }
        }
    }
    let _ = out.h;
    Some(())
}

/// Reads the frame info of an access unit's primary frame.
pub fn info(au: &[u8]) -> Result<(FrameInfo, Option<Colour>)> {
    let u = access_unit(au)?;
    let (at, len) = u
        .primary
        .ok_or_else(|| Error::Malformed("no primary frame".into()))?;
    let h = header(&au[at..at + len])?;
    Ok((h.info, h.colour))
}

/// Decodes an access unit's primary frame (`au` starts with the signature).
pub fn decode(au: &[u8]) -> Result<Picture> {
    let u = access_unit(au)?;
    let (at, len) = u
        .primary
        .ok_or_else(|| Error::Malformed("no primary frame".into()))?;
    let f = &au[at..at + len];
    let h = header(f)?;
    let info = h.info;
    let comps = info.chroma.components();
    let sub_w = info.chroma.sub_w();
    let depth = u32::from(info.bit_depth);
    let qp_offset = (depth - 8) * 6;
    let (cols, rows) = (h.col_starts.len() - 1, h.row_starts.len() - 1);
    // The tiles: (index, its bytes).
    let mut tiles = Vec::with_capacity(cols * rows);
    let mut p = h.len;
    for t in 0..cols * rows {
        let Some(sz) = f.get(p..p + 4) else {
            break;
        };
        let size = u32::from_be_bytes([sz[0], sz[1], sz[2], sz[3]]) as usize;
        let start = p + 4;
        let end = start.saturating_add(size).min(f.len());
        tiles.push((t, &f[start..end]));
        p = start.saturating_add(size);
    }
    let scan = zigzag();
    let decoded: Vec<(usize, Option<Vec<TilePlane>>)> = tiles
        .par_iter()
        .map(|&(t, data)| {
            let (tx, ty) = (t % cols, t / cols);
            let mbs = (
                (h.col_starts[tx + 1] - h.col_starts[tx]) / 16,
                (h.row_starts[ty + 1] - h.row_starts[ty]) / 16,
            );
            let run = || -> Option<Vec<TilePlane>> {
                let mut b = Bits::new(data);
                let hsize = b.u(16)? as usize;
                b.u(16)?;
                let mut sizes = vec![0usize; comps];
                for s in &mut sizes {
                    *s = b.u(32)? as usize;
                }
                let mut qps = vec![0u32; comps];
                for q in &mut qps {
                    *q = b.u(8)?;
                }
                let mut at = hsize;
                let mut planes = Vec::with_capacity(comps);
                for c in 0..comps {
                    let bw = if c == 0 || c == 3 { 16 } else { 16 / sub_w };
                    let mut tp = TilePlane {
                        w: mbs.0 * bw,
                        h: mbs.1 * 16,
                        px: vec![0; mbs.0 * bw * mbs.1 * 16],
                    };
                    let src = data.get(at..at.saturating_add(sizes[c]).min(data.len()))?;
                    // tile_qp is qP itself (Qp + QpBdOffset).
                    if qps[c] > 51 + qp_offset {
                        return None;
                    }
                    tile_component(
                        src,
                        bw,
                        16,
                        mbs,
                        qps[c],
                        &h.qmatrix[c],
                        depth,
                        &scan,
                        &mut tp,
                    )?;
                    planes.push(tp);
                    at += sizes[c];
                }
                Some(planes)
            };
            (t, run())
        })
        .collect();
    let (w, hgt) = (info.width as usize, info.height as usize);
    let mid = 1u16 << (depth - 1);
    let mut planes: Vec<Vec<u16>> = (0..comps)
        .map(|c| {
            let pw = if c == 0 || c == 3 {
                w
            } else {
                w.div_ceil(sub_w)
            };
            vec![mid; pw * hgt]
        })
        .collect();
    let mut damaged = cols * rows - decoded.iter().filter(|d| d.1.is_some()).count();
    for (t, tp) in decoded {
        let Some(tp) = tp else { continue };
        let (tx, ty) = (t % cols, t / cols);
        for (c, plane) in tp.iter().enumerate() {
            let div = if c == 0 || c == 3 { 1 } else { sub_w };
            let pw = if c == 0 || c == 3 {
                w
            } else {
                w.div_ceil(sub_w)
            };
            let (x0, y0) = (h.col_starts[tx] / div, h.row_starts[ty]);
            for y in 0..plane.h.min(hgt.saturating_sub(y0)) {
                let n = plane.w.min(pw.saturating_sub(x0));
                planes[c][(y0 + y) * pw + x0..(y0 + y) * pw + x0 + n]
                    .copy_from_slice(&plane.px[y * plane.w..y * plane.w + n]);
            }
        }
    }
    if tiles.len() < cols * rows {
        damaged = damaged.max(cols * rows - tiles.len());
    }
    Ok(Picture {
        info,
        colour: h.colour,
        planes,
        damaged_tiles: damaged,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zigzag_starts_like_jpeg() {
        let z = zigzag();
        assert_eq!(&z[..10], &[0, 1, 8, 16, 9, 2, 3, 10, 17, 24]);
        assert_eq!(z[63], 63);
    }

    #[test]
    fn variable_length_codes() {
        // "1": 0; "00" + k bits: (1 << k) + bits; "01": 2 << k, then Exp-Golomb.
        let mut b = Bits::new(&[0b1000_0000]);
        assert_eq!(b.vlc(0), Some(0));
        assert_eq!(b.vlc(0), Some(1));
        let mut b = Bits::new(&[0b0110_0000]);
        assert_eq!(b.vlc(0), Some(2));
        // "01", "0" (k = 1), "1", then one bit 1: 2 + 1 + 1.
        let mut b = Bits::new(&[0b0101_1000]);
        assert_eq!(b.vlc(0), Some(4));
    }

    #[test]
    fn raw_file_units() {
        let mut d = 8u32.to_be_bytes().to_vec();
        d.extend_from_slice(b"aPv1\0\0\0\0");
        d.extend_from_slice(&4u32.to_be_bytes());
        d.extend_from_slice(b"aPv1");
        assert!(is_raw_file(&d));
        assert_eq!(raw_units(&d), vec![(4, 8), (16, 4)]);
    }
}
