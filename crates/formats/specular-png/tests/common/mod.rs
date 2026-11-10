//! A small PNG / APNG encoder for tests: every colour type and depth, Adam7, chosen filters,
//! tRNS, split IDATs, iDOT, and a reference compositor on top of the `png` crate.

#![allow(dead_code)]

use miniz_oxide::deflate::core::{
    CompressorOxide, TDEFLFlush, compress, create_comp_flags_from_zip_params,
};

pub const ADAM7: [(usize, usize, usize, usize); 7] = [
    (0, 0, 8, 8),
    (4, 0, 8, 8),
    (0, 4, 4, 8),
    (2, 0, 4, 4),
    (0, 2, 2, 4),
    (1, 0, 2, 2),
    (0, 1, 1, 2),
];

pub struct Rng(pub u64);
impl Rng {
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    pub fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

pub fn channels(ct: u8) -> usize {
    match ct {
        0 | 3 => 1,
        4 => 2,
        2 => 3,
        _ => 4,
    }
}

pub fn depths(ct: u8) -> &'static [u8] {
    match ct {
        0 => &[1, 2, 4, 8, 16],
        3 => &[1, 2, 4, 8],
        _ => &[8, 16],
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Filters {
    Fixed(u8),
    /// Pseudo-random per row.
    Mixed,
}

/// Samples of a (sub-)image: `w * h * channels` values below `2^depth`.
#[derive(Clone, Debug)]
pub struct Samples {
    pub w: usize,
    pub h: usize,
    pub ct: u8,
    pub depth: u8,
    pub v: Vec<u16>,
}

/// Content kinds.
#[derive(Clone, Copy, Debug)]
pub enum Content {
    Noise,
    /// Smooth gradients with a little noise (photo-like).
    Photo,
    /// Flat areas and repeated patterns (screenshot-like).
    Screen,
}

pub fn make(w: usize, h: usize, ct: u8, depth: u8, content: Content, seed: u64) -> Samples {
    let ch = channels(ct);
    let max = if depth == 16 {
        65535u32
    } else {
        (1u32 << depth) - 1
    };
    let mut r = Rng(seed | 1);
    let mut v = Vec::with_capacity(w * h * ch);
    for y in 0..h {
        for x in 0..w {
            for c in 0..ch {
                let s = match content {
                    Content::Noise => r.next() as u32 & max,
                    Content::Photo => {
                        let base =
                            ((x * (3 + c) + y * (5 - c.min(4))) * 97 / (w + h).max(1)) as u32;
                        let n = (r.below(9) as u32).wrapping_sub(4);
                        let v8 = (base.wrapping_add(n)) & 255;
                        if depth == 16 {
                            v8 * 257 + (r.next() as u32 & 0xff)
                        } else {
                            v8 >> (8 - depth.min(8))
                        }
                    }
                    Content::Screen => {
                        let cell = ((x / 37) + (y / 23) * 3 + c) as u32;
                        let text = (x % 11 < 2 && y % 17 < 9) as u32;
                        let v8 = if text == 1 { 20 } else { (cell * 53) & 255 };
                        if depth == 16 {
                            v8 * 257
                        } else {
                            v8 >> (8 - depth.min(8))
                        }
                    }
                };
                v.push(s.min(max) as u16);
            }
        }
    }
    Samples { w, h, ct, depth, v }
}

/// Packed scanline bytes (no filter byte) for pixels `xs` of row `y`.
fn pack_row(s: &Samples, y: usize, xs: impl Iterator<Item = usize>) -> Vec<u8> {
    let ch = channels(s.ct);
    let mut out = Vec::new();
    let mut acc = 0u32;
    let mut nbits = 0;
    for x in xs {
        for c in 0..ch {
            let v = s.v[(y * s.w + x) * ch + c];
            match s.depth {
                16 => out.extend_from_slice(&v.to_be_bytes()),
                8 => out.push(v as u8),
                d => {
                    acc = (acc << d) | u32::from(v);
                    nbits += d;
                    if nbits == 8 {
                        out.push(acc as u8);
                        acc = 0;
                        nbits = 0;
                    }
                }
            }
        }
    }
    if nbits > 0 {
        out.push((acc << (8 - nbits)) as u8);
    }
    out
}

fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let p = i16::from(a) + i16::from(b) - i16::from(c);
    let (pa, pb, pc) = (
        (p - i16::from(a)).abs(),
        (p - i16::from(b)).abs(),
        (p - i16::from(c)).abs(),
    );
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

fn filter_row(ft: u8, bpp: usize, prev: &[u8], cur: &[u8], out: &mut Vec<u8>) {
    out.push(ft);
    for i in 0..cur.len() {
        let a = if i >= bpp { cur[i - bpp] } else { 0 };
        let b = prev[i];
        let c = if i >= bpp { prev[i - bpp] } else { 0 };
        let p = match ft {
            0 => 0,
            1 => a,
            2 => b,
            3 => ((u16::from(a) + u16::from(b)) / 2) as u8,
            _ => paeth(a, b, c),
        };
        out.push(cur[i].wrapping_sub(p));
    }
}

/// The filtered (uncompressed) image data.
pub fn filtered(s: &Samples, interlace: bool, filters: Filters, seed: u64) -> Vec<u8> {
    let bpp = (channels(s.ct) * s.depth as usize).div_ceil(8);
    let mut r = Rng(seed.wrapping_mul(31) | 1);
    let mut out = Vec::new();
    let mut emit = |rows: Vec<Vec<u8>>, out: &mut Vec<u8>| {
        let mut prev = vec![0u8; rows.first().map_or(0, |r| r.len())];
        for row in rows {
            let ft = match filters {
                Filters::Fixed(f) => f,
                Filters::Mixed => r.below(5) as u8,
            };
            filter_row(ft, bpp, &prev, &row, out);
            prev = row;
        }
    };
    if !interlace {
        let rows = (0..s.h).map(|y| pack_row(s, y, 0..s.w)).collect();
        emit(rows, &mut out);
    } else {
        for &(x0, y0, dx, dy) in &ADAM7 {
            if s.w <= x0 || s.h <= y0 {
                continue;
            }
            let rows = (y0..s.h)
                .step_by(dy)
                .map(|y| pack_row(s, y, (x0..s.w).step_by(dx)))
                .collect();
            emit(rows, &mut out);
        }
    }
    out
}

pub fn crc32(data: &[u8]) -> u32 {
    let mut c = !0u32;
    for &b in data {
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

pub fn chunk(out: &mut Vec<u8>, ty: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(ty);
    out.extend_from_slice(data);
    let crc = crc32(&out[start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

pub fn ihdr(w: usize, h: usize, ct: u8, depth: u8, interlace: bool) -> Vec<u8> {
    let mut d = Vec::new();
    d.extend_from_slice(&(w as u32).to_be_bytes());
    d.extend_from_slice(&(h as u32).to_be_bytes());
    d.extend_from_slice(&[depth, ct, 0, 0, interlace as u8]);
    d
}

pub const SIG: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];

#[derive(Clone, Debug, Default)]
pub struct Extra {
    pub palette: Option<Vec<[u8; 3]>>,
    pub trns: Option<Vec<u8>>,
    /// IDAT payload size limit (split into several chunks).
    pub idat_size: Option<usize>,
    pub level: Option<u8>,
}

/// A palette with entries for every index of `depth`.
pub fn palette(depth: u8, seed: u64) -> Vec<[u8; 3]> {
    let mut r = Rng(seed | 1);
    (0..1usize << depth)
        .map(|_| [r.next() as u8, r.next() as u8, r.next() as u8])
        .collect()
}

/// A tRNS chunk that matches some pixel (colour key) or gives varied palette alpha.
pub fn trns_for(s: &Samples, seed: u64) -> Option<Vec<u8>> {
    let mut r = Rng(seed | 1);
    match s.ct {
        0 => Some(s.v[s.v.len() / 2].to_be_bytes().to_vec()),
        2 => {
            let i = (s.v.len() / 6) * 3;
            let mut d = Vec::new();
            for c in 0..3 {
                d.extend_from_slice(&s.v[i + c].to_be_bytes());
            }
            Some(d)
        }
        3 => Some(
            (0..(1usize << s.depth).min(200))
                .map(|_| [0u8, 255, r.next() as u8][r.below(3) as usize])
                .collect(),
        ),
        _ => None,
    }
}

pub fn zlib(data: &[u8], level: u8) -> Vec<u8> {
    miniz_oxide::deflate::compress_to_vec_zlib(data, level)
}

pub fn encode(s: &Samples, interlace: bool, filters: Filters, extra: &Extra, seed: u64) -> Vec<u8> {
    let raw = filtered(s, interlace, filters, seed);
    let z = zlib(&raw, extra.level.unwrap_or(6));
    let mut out = SIG.to_vec();
    chunk(&mut out, b"IHDR", &ihdr(s.w, s.h, s.ct, s.depth, interlace));
    if let Some(p) = &extra.palette {
        chunk(&mut out, b"PLTE", p.as_flattened());
    }
    if let Some(t) = &extra.trns {
        chunk(&mut out, b"tRNS", t);
    }
    let n = extra.idat_size.unwrap_or(usize::MAX).max(1);
    for part in z.chunks(n) {
        chunk(&mut out, b"IDAT", part);
    }
    chunk(&mut out, b"IEND", &[]);
    out
}

/// A non-interlaced file with an Apple-style iDOT chunk: `parts` independently compressed row
/// ranges (full flushes) in separate IDAT chunks.
pub fn encode_idot(s: &Samples, parts: usize) -> Vec<u8> {
    let raw = filtered(s, false, Filters::Mixed, 7);
    let row = raw.len() / s.h;
    let mut c = CompressorOxide::new(create_comp_flags_from_zip_params(6, 15, 0));
    let mut pieces = Vec::new();
    let mut rows = Vec::new();
    let per = s.h.div_ceil(parts);
    for p in 0..parts {
        let (a, b) = (p * per, ((p + 1) * per).min(s.h));
        let mut buf = vec![0u8; (b - a) * row * 2 + 4096];
        let flush = if p + 1 == parts {
            TDEFLFlush::Finish
        } else {
            TDEFLFlush::Full
        };
        let (_, used, n) = compress(&mut c, &raw[a * row..b * row], &mut buf, flush);
        assert_eq!(used, (b - a) * row);
        buf.truncate(n);
        pieces.push(buf);
        rows.push((a, b - a));
    }
    let mut out = SIG.to_vec();
    chunk(&mut out, b"IHDR", &ihdr(s.w, s.h, s.ct, s.depth, false));
    // iDOT: count, then (first_row, rows, offset from the iDOT chunk start to the IDAT).
    let idot_pos = out.len();
    let idot_len = 12 + 4 + 12 * parts;
    let mut offs = Vec::new();
    let mut pos = idot_pos + idot_len;
    for p in &pieces {
        offs.push(pos - idot_pos);
        pos += 12 + p.len();
    }
    let mut d = (parts as u32).to_be_bytes().to_vec();
    for (i, &(a, n)) in rows.iter().enumerate() {
        d.extend_from_slice(&(a as u32).to_be_bytes());
        d.extend_from_slice(&(n as u32).to_be_bytes());
        d.extend_from_slice(&(offs[i] as u32).to_be_bytes());
    }
    chunk(&mut out, b"iDOT", &d);
    for p in &pieces {
        chunk(&mut out, b"IDAT", p);
    }
    chunk(&mut out, b"IEND", &[]);
    out
}

pub struct ApngFrame {
    pub s: Samples,
    pub x: u32,
    pub y: u32,
    pub delay: (u16, u16),
    pub dispose: u8,
    pub blend: u8,
}

/// An APNG. With `default` set, the default image is that separate (non-animated) image;
/// otherwise frame 0 is the default image.
pub fn encode_apng(
    w: usize,
    h: usize,
    frames: &[ApngFrame],
    default: Option<&Samples>,
    interlace: bool,
    plays: u32,
    extra: &Extra,
) -> Vec<u8> {
    let ct = frames[0].s.ct;
    let depth = frames[0].s.depth;
    let mut out = SIG.to_vec();
    chunk(&mut out, b"IHDR", &ihdr(w, h, ct, depth, interlace));
    let mut actl = (frames.len() as u32).to_be_bytes().to_vec();
    actl.extend_from_slice(&plays.to_be_bytes());
    chunk(&mut out, b"acTL", &actl);
    if let Some(p) = &extra.palette {
        chunk(&mut out, b"PLTE", p.as_flattened());
    }
    if let Some(t) = &extra.trns {
        chunk(&mut out, b"tRNS", t);
    }
    let mut seq = 0u32;
    let fctl = |f: &ApngFrame, seq: &mut u32, out: &mut Vec<u8>| {
        let mut d = seq.to_be_bytes().to_vec();
        *seq += 1;
        for v in [f.s.w as u32, f.s.h as u32, f.x, f.y] {
            d.extend_from_slice(&v.to_be_bytes());
        }
        d.extend_from_slice(&f.delay.0.to_be_bytes());
        d.extend_from_slice(&f.delay.1.to_be_bytes());
        d.extend_from_slice(&[f.dispose, f.blend]);
        chunk(out, b"fcTL", &d);
    };
    let mut rest = frames;
    match default {
        Some(d) => {
            let z = zlib(&filtered(d, interlace, Filters::Mixed, 1), 6);
            chunk(&mut out, b"IDAT", &z);
        }
        None => {
            fctl(&frames[0], &mut seq, &mut out);
            let z = zlib(&filtered(&frames[0].s, interlace, Filters::Mixed, 1), 6);
            chunk(&mut out, b"IDAT", &z);
            rest = &frames[1..];
        }
    }
    for (i, f) in rest.iter().enumerate() {
        fctl(f, &mut seq, &mut out);
        let z = zlib(&filtered(&f.s, interlace, Filters::Mixed, i as u64 + 2), 6);
        // Two fdAT chunks per frame where possible.
        let mid = z.len() / 2;
        for part in [&z[..mid], &z[mid..]] {
            let mut d = seq.to_be_bytes().to_vec();
            seq += 1;
            d.extend_from_slice(part);
            chunk(&mut out, b"fdAT", &d);
        }
    }
    chunk(&mut out, b"IEND", &[]);
    out
}

// ---------------------------------------------------------------------------------------------
// Oracle (png crate)

pub struct OracleFrame {
    pub w: usize,
    pub h: usize,
    /// RGBA, 16-bit scale.
    pub rgba16: Vec<u16>,
    pub fc: Option<png::FrameControl>,
}

fn to_rgba16(buf: &[u8], info: &png::OutputInfo, stride: usize) -> Vec<u16> {
    let ch = match info.color_type {
        png::ColorType::Grayscale => 1,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        png::ColorType::Indexed => panic!("indexed after EXPAND"),
    };
    let sixteen = info.bit_depth == png::BitDepth::Sixteen;
    assert!(sixteen || info.bit_depth == png::BitDepth::Eight);
    let (w, h) = (info.width as usize, info.height as usize);
    let mut out = Vec::with_capacity(w * h * 4);
    for y in 0..h {
        let row = &buf[y * stride..];
        for x in 0..w {
            let s = |c: usize| -> u16 {
                let i = x * ch + c;
                if sixteen {
                    u16::from_be_bytes([row[i * 2], row[i * 2 + 1]])
                } else {
                    u16::from(row[i]) * 257
                }
            };
            let px = match ch {
                1 => [s(0), s(0), s(0), 65535],
                2 => [s(0), s(0), s(0), s(1)],
                3 => [s(0), s(1), s(2), 65535],
                _ => [s(0), s(1), s(2), s(3)],
            };
            out.extend_from_slice(&px);
        }
    }
    out
}

/// All images in the file as the png crate decodes them (default image first).
pub fn oracle(data: &[u8]) -> Vec<OracleFrame> {
    let mut d = png::Decoder::new(std::io::Cursor::new(data));
    d.set_transformations(png::Transformations::EXPAND);
    let mut r = d.read_info().expect("oracle read_info");
    let n = r
        .info()
        .animation_control()
        .map_or(1, |a| a.num_frames as usize)
        + usize::from(r.info().animation_control().is_some() && r.info().frame_control().is_none());
    let mut out = Vec::new();
    for _ in 0..n {
        let mut buf = vec![0u8; r.output_buffer_size().expect("size")];
        let Ok(info) = r.next_frame(&mut buf) else {
            break;
        };
        out.push(OracleFrame {
            w: info.width as usize,
            h: info.height as usize,
            rgba16: {
                // png 0.18 lays interlaced APNG subframes out with the canvas row stride.
                let stride = if r.info().interlaced {
                    info.line_size / info.width as usize * r.info().width as usize
                } else {
                    info.line_size
                };
                to_rgba16(&buf, &info, stride)
            },
            fc: r.info().frame_control().copied(),
        });
    }
    out
}

pub fn r8(v: u16) -> u8 {
    ((u32::from(v) * 255 + 32895) >> 16) as u8
}

/// Reference APNG compositor (float maths) over oracle frames; returns RGBA16 canvases.
pub fn composite(w: usize, h: usize, frames: &[OracleFrame]) -> Vec<Vec<u16>> {
    let mut canvas = vec![0u16; w * h * 4];
    let mut out = Vec::new();
    let mut pending: Option<(png::FrameControl, Vec<u16>)> = None;
    for (i, f) in frames.iter().enumerate() {
        let fc = f.fc.expect("frame control");
        if let Some((p, saved)) = pending.take() {
            let (x0, y0, fw, fh) = (
                p.x_offset as usize,
                p.y_offset as usize,
                p.width as usize,
                p.height as usize,
            );
            for y in 0..fh {
                for x in 0..fw {
                    let o = ((y0 + y) * w + x0 + x) * 4;
                    match p.dispose_op {
                        png::DisposeOp::None => {}
                        png::DisposeOp::Background => canvas[o..o + 4].fill(0),
                        png::DisposeOp::Previous => {
                            let s = (y * fw + x) * 4;
                            canvas[o..o + 4].copy_from_slice(&saved[s..s + 4]);
                        }
                    }
                }
            }
        }
        let (x0, y0) = (fc.x_offset as usize, fc.y_offset as usize);
        let mut saved = Vec::new();
        let mut dispose = fc.dispose_op;
        if i == 0 && dispose == png::DisposeOp::Previous {
            dispose = png::DisposeOp::Background;
        }
        if dispose == png::DisposeOp::Previous {
            for y in 0..f.h {
                for x in 0..f.w {
                    let o = ((y0 + y) * w + x0 + x) * 4;
                    saved.extend_from_slice(&canvas[o..o + 4]);
                }
            }
        }
        for y in 0..f.h {
            for x in 0..f.w {
                let o = ((y0 + y) * w + x0 + x) * 4;
                let s = &f.rgba16[(y * f.w + x) * 4..][..4];
                if i == 0 || fc.blend_op == png::BlendOp::Source {
                    canvas[o..o + 4].copy_from_slice(s);
                } else {
                    let sa = f64::from(s[3]) / 65535.0;
                    let da = f64::from(canvas[o + 3]) / 65535.0;
                    let oa = sa + da * (1.0 - sa);
                    if oa > 0.0 {
                        for c in 0..3 {
                            let v = (f64::from(s[c]) * sa
                                + f64::from(canvas[o + c]) * da * (1.0 - sa))
                                / oa;
                            canvas[o + c] = v.round() as u16;
                        }
                    }
                    canvas[o + 3] = (oa * 65535.0).round() as u16;
                }
            }
        }
        let mut fc2 = fc;
        fc2.dispose_op = dispose;
        pending = Some((fc2, saved));
        out.push(canvas.clone());
    }
    out
}
