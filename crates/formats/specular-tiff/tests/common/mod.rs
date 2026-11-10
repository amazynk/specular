//! A small TIFF writer for fixtures: strips or tiles, chunky or planar, any compression the
//! decoder supports, classic or BigTIFF, either byte order, page chains and SubIFDs.
#![allow(dead_code)]

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Comp {
    None,
    Lzw,
    Deflate,
    PackBits,
    /// Units come precompressed in [`Spec::pre`].
    Jpeg,
    /// Old-style JPEG (6); units precompressed like [`Comp::Jpeg`].
    OldJpeg,
    /// Any other compression code; units precompressed like [`Comp::Jpeg`].
    Coded(u16),
}

#[derive(Clone, Debug)]
pub struct Spec {
    pub w: u32,
    pub h: u32,
    pub spp: u16,
    pub bits: u16,
    /// 1 uint, 2 int, 3 float.
    pub sample_format: u16,
    pub photometric: u16,
    /// Chunky rows in the file's byte order, rows byte-aligned.
    pub raw: Vec<u8>,
    pub tile: Option<(u32, u32)>,
    pub rows_per_strip: u32,
    pub comp: Comp,
    pub predictor: u16,
    pub planar: bool,
    pub subfile: u32,
    pub extra: Vec<(u16, u16, Vec<u64>)>,
    pub sub_ifds: Vec<Spec>,
    /// Precompressed units (strips or tiles, in order), written as is.
    pub pre: Option<Vec<Vec<u8>>>,
}

impl Spec {
    pub fn new(w: u32, h: u32, spp: u16, bits: u16, photometric: u16, raw: Vec<u8>) -> Spec {
        Spec {
            w,
            h,
            spp,
            bits,
            sample_format: 1,
            photometric,
            raw,
            tile: None,
            rows_per_strip: 8,
            comp: Comp::None,
            predictor: 1,
            planar: false,
            subfile: 0,
            extra: Vec::new(),
            sub_ifds: Vec::new(),
            pre: None,
        }
    }
}

pub fn row_bytes(w: u32, spp: u16, bits: u16) -> usize {
    (w as usize * spp as usize * bits as usize).div_ceil(8)
}

/// Smooth-ish pattern with some noise, `spp` 8-bit samples per pixel.
pub fn pattern8(w: u32, h: u32, spp: u16, seed: u32) -> Vec<u8> {
    let mut v = Vec::with_capacity((w * h) as usize * spp as usize);
    let mut s = seed.wrapping_mul(2654435761) | 1;
    for y in 0..h {
        for x in 0..w {
            for c in 0..spp as u32 {
                s ^= s << 13;
                s ^= s >> 17;
                s ^= s << 5;
                let base = (x * 3 + y * 2 + c * 70) & 0xff;
                v.push((base ^ (s & 7)) as u8);
            }
        }
    }
    v
}

/// 16-bit pattern (native values).
pub fn pattern16(w: u32, h: u32, spp: u16, seed: u32) -> Vec<u16> {
    let mut v = Vec::with_capacity((w * h) as usize * spp as usize);
    let mut s = seed.wrapping_mul(2654435761) | 1;
    for y in 0..h {
        for x in 0..w {
            for c in 0..spp as u32 {
                s ^= s << 13;
                s ^= s >> 17;
                s ^= s << 5;
                v.push(((x * 397 + y * 211 + c * 9000) as u16) ^ (s & 0x3ff) as u16);
            }
        }
    }
    v
}

pub fn u16_bytes(v: &[u16], le: bool) -> Vec<u8> {
    v.iter()
        .flat_map(|x| if le { x.to_le_bytes() } else { x.to_be_bytes() })
        .collect()
}

fn packbits_enc(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < data.len() {
        // run?
        let mut r = 1;
        while i + r < data.len() && r < 128 && data[i + r] == data[i] {
            r += 1;
        }
        if r >= 3 {
            out.push((1i32 - r as i32) as i8 as u8);
            out.push(data[i]);
            i += r;
            continue;
        }
        let start = i;
        let mut n = 0;
        while i < data.len() && n < 128 {
            if i + 2 < data.len() && data[i] == data[i + 1] && data[i] == data[i + 2] {
                break;
            }
            i += 1;
            n += 1;
        }
        out.push((n - 1) as u8);
        out.extend_from_slice(&data[start..start + n]);
    }
    out
}

fn compress(c: Comp, data: &[u8]) -> Vec<u8> {
    match c {
        Comp::None => data.to_vec(),
        Comp::Lzw => weezl::encode::Encoder::with_tiff_size_switch(weezl::BitOrder::Msb, 8)
            .encode(data)
            .unwrap(),
        Comp::Deflate => miniz_oxide::deflate::compress_to_vec_zlib(data, 6),
        Comp::PackBits => packbits_enc(data),
        Comp::Jpeg | Comp::OldJpeg | Comp::Coded(_) => panic!("units must be precompressed"),
    }
}

fn comp_tag(c: Comp) -> u64 {
    match c {
        Comp::None => 1,
        Comp::Lzw => 5,
        Comp::Deflate => 8,
        Comp::PackBits => 32773,
        Comp::Jpeg => 7,
        Comp::OldJpeg => 6,
        Comp::Coded(c) => c as u64,
    }
}

/// Apply the horizontal predictor to a row (8/16-bit, file byte order).
fn predict(row: &mut [u8], bits: u16, stride: usize, le: bool) {
    match bits {
        8 => {
            for i in (stride..row.len()).rev() {
                row[i] = row[i].wrapping_sub(row[i - stride]);
            }
        }
        16 => {
            let n = row.len() / 2;
            let rd = |r: &[u8], i: usize| {
                let b = [r[2 * i], r[2 * i + 1]];
                if le {
                    u16::from_le_bytes(b)
                } else {
                    u16::from_be_bytes(b)
                }
            };
            for i in (stride..n).rev() {
                let v = rd(row, i).wrapping_sub(rd(row, i - stride));
                let b = if le { v.to_le_bytes() } else { v.to_be_bytes() };
                row[2 * i..2 * i + 2].copy_from_slice(&b);
            }
        }
        // Other depths: the samples as numbers (bit-packed MSB first unless whole bytes, which
        // are in the file's byte order), each minus the one `stride` before, modulo 2^bits.
        b => {
            let n = row.len() * 8 / b as usize;
            let mask = if b >= 64 { u64::MAX } else { (1u64 << b) - 1 };
            let v: Vec<u64> = (0..n).map(|i| get_sample(row, i, b, le)).collect();
            for i in stride..n {
                put_sample(row, i, b, le, v[i].wrapping_sub(v[i - stride]) & mask);
            }
        }
    }
}

/// Sample `i` of a row of `bits`-bit samples: bit-packed MSB first, or whole bytes in the
/// file's byte order.
pub fn get_sample(row: &[u8], i: usize, bits: u16, le: bool) -> u64 {
    let b = bits as usize;
    if b.is_multiple_of(8) {
        let s = &row[i * b / 8..(i + 1) * b / 8];
        return if le {
            s.iter().rev().fold(0, |a, &x| (a << 8) | x as u64)
        } else {
            s.iter().fold(0, |a, &x| (a << 8) | x as u64)
        };
    }
    (0..b).fold(0, |a, k| {
        let p = i * b + k;
        (a << 1) | ((row[p / 8] >> (7 - p % 8)) & 1) as u64
    })
}

/// Store sample `i` (see [`get_sample`]).
pub fn put_sample(row: &mut [u8], i: usize, bits: u16, le: bool, v: u64) {
    let b = bits as usize;
    if b.is_multiple_of(8) {
        let n = b / 8;
        for k in 0..n {
            row[i * n + k] = (v >> (8 * if le { k } else { n - 1 - k })) as u8;
        }
        return;
    }
    for k in 0..b {
        let p = i * b + k;
        let bit = ((v >> (b - 1 - k)) & 1) as u8;
        row[p / 8] = (row[p / 8] & !(0x80 >> (p % 8))) | (bit << (7 - p % 8));
    }
}

/// Cut the image into units (strips or tiles, per plane) and compress them.
fn units(s: &Spec, le: bool) -> Vec<Vec<u8>> {
    if let Some(pre) = &s.pre {
        return pre.clone();
    }
    let planes: Vec<(Vec<u8>, u16)> = if s.planar && !s.bits.is_multiple_of(8) {
        // Bit-packed rows: split sample by sample.
        let rb = row_bytes(s.w, s.spp, s.bits);
        let pb = row_bytes(s.w, 1, s.bits);
        (0..s.spp as usize)
            .map(|c| {
                let mut p = vec![0u8; pb * s.h as usize];
                for y in 0..s.h as usize {
                    let src = &s.raw[y * rb..(y + 1) * rb];
                    let dst = &mut p[y * pb..(y + 1) * pb];
                    for x in 0..s.w as usize {
                        let v = get_sample(src, x * s.spp as usize + c, s.bits, le);
                        put_sample(dst, x, s.bits, le, v);
                    }
                }
                (p, 1)
            })
            .collect()
    } else if s.planar {
        let bps = s.bits as usize / 8;
        (0..s.spp as usize)
            .map(|c| {
                let mut p = Vec::new();
                for px in s.raw.chunks_exact(bps * s.spp as usize) {
                    p.extend_from_slice(&px[c * bps..(c + 1) * bps]);
                }
                (p, 1)
            })
            .collect()
    } else {
        vec![(s.raw.clone(), s.spp)]
    };
    let mut out = Vec::new();
    for (data, spp) in &planes {
        let rb = row_bytes(s.w, *spp, s.bits);
        let (uw, uh) = s.tile.unwrap_or((s.w, s.rows_per_strip));
        let urb = row_bytes(uw, *spp, s.bits);
        for uy in 0..s.h.div_ceil(uh) {
            for ux in 0..s.w.div_ceil(uw) {
                let mut u = Vec::new();
                let rows = if s.tile.is_some() {
                    uh
                } else {
                    uh.min(s.h - uy * uh)
                };
                for r in 0..rows {
                    let y = uy * uh + r;
                    let mut row = vec![0u8; urb];
                    if y < s.h {
                        let x0 = (ux * uw) as usize;
                        let bits_per_px = *spp as usize * s.bits as usize;
                        assert!((x0 * bits_per_px).is_multiple_of(8) || ux == 0);
                        let b0 = x0 * bits_per_px / 8;
                        let n = urb.min(rb - b0);
                        row[..n]
                            .copy_from_slice(&data[y as usize * rb + b0..y as usize * rb + b0 + n]);
                    }
                    if s.predictor == 2 {
                        predict(&mut row, s.bits, *spp as usize, le);
                    }
                    u.extend_from_slice(&row);
                }
                out.push(compress(s.comp, &u));
            }
        }
    }
    out
}

struct W {
    buf: Vec<u8>,
    le: bool,
    big: bool,
}

impl W {
    fn u16(&mut self, v: u16) {
        let b = if self.le {
            v.to_le_bytes()
        } else {
            v.to_be_bytes()
        };
        self.buf.extend_from_slice(&b);
    }
    fn u32(&mut self, v: u32) {
        let b = if self.le {
            v.to_le_bytes()
        } else {
            v.to_be_bytes()
        };
        self.buf.extend_from_slice(&b);
    }
    fn u64(&mut self, v: u64) {
        let b = if self.le {
            v.to_le_bytes()
        } else {
            v.to_be_bytes()
        };
        self.buf.extend_from_slice(&b);
    }
    fn off(&mut self, v: u64) {
        if self.big {
            self.u64(v)
        } else {
            self.u32(v as u32)
        }
    }
    fn patch_off(&mut self, at: usize, v: u64) {
        let b: Vec<u8> = if self.big {
            if self.le {
                v.to_le_bytes().to_vec()
            } else {
                v.to_be_bytes().to_vec()
            }
        } else if self.le {
            (v as u32).to_le_bytes().to_vec()
        } else {
            (v as u32).to_be_bytes().to_vec()
        };
        self.buf[at..at + b.len()].copy_from_slice(&b);
    }
    fn value_bytes(&self, typ: u16, vals: &[u64]) -> Vec<u8> {
        let mut out = Vec::new();
        for &v in vals {
            match typ {
                1 | 2 | 7 => out.push(v as u8),
                3 => out.extend_from_slice(&if self.le {
                    (v as u16).to_le_bytes()
                } else {
                    (v as u16).to_be_bytes()
                }),
                4 | 13 => out.extend_from_slice(&if self.le {
                    (v as u32).to_le_bytes()
                } else {
                    (v as u32).to_be_bytes()
                }),
                // RATIONAL / SRATIONAL: numerator in the high 32 bits, denominator low.
                5 | 10 => {
                    for w in [(v >> 32) as u32, v as u32] {
                        out.extend_from_slice(&if self.le {
                            w.to_le_bytes()
                        } else {
                            w.to_be_bytes()
                        });
                    }
                }
                16 | 18 => out.extend_from_slice(&if self.le {
                    v.to_le_bytes()
                } else {
                    v.to_be_bytes()
                }),
                _ => panic!("type"),
            }
        }
        out
    }

    /// Write one IFD (and its data); returns (ifd offset, position of its next pointer).
    fn ifd(&mut self, s: &Spec) -> (u64, usize) {
        let subs: Vec<u64> = s.sub_ifds.iter().map(|sub| self.ifd(sub).0).collect();
        let chunks = units(s, self.le);
        let mut offs = Vec::new();
        for c in &chunks {
            offs.push(self.buf.len() as u64);
            self.buf.extend_from_slice(c);
        }
        let counts: Vec<u64> = chunks.iter().map(|c| c.len() as u64).collect();
        let ot = if self.big { 16 } else { 4 };
        let mut tags: Vec<(u16, u16, Vec<u64>)> = vec![
            (254, 4, vec![s.subfile as u64]),
            (256, 4, vec![s.w as u64]),
            (257, 4, vec![s.h as u64]),
            (258, 3, vec![s.bits as u64; s.spp as usize]),
            (259, 3, vec![comp_tag(s.comp)]),
            (262, 3, vec![s.photometric as u64]),
            (277, 3, vec![s.spp as u64]),
            (284, 3, vec![if s.planar { 2 } else { 1 }]),
            (339, 3, vec![s.sample_format as u64; s.spp as usize]),
        ];
        if s.predictor != 1 {
            tags.push((317, 3, vec![s.predictor as u64]));
        }
        match s.tile {
            Some((tw, th)) => {
                tags.push((322, 4, vec![tw as u64]));
                tags.push((323, 4, vec![th as u64]));
                tags.push((324, ot, offs));
                tags.push((325, ot, counts));
            }
            None => {
                tags.push((273, ot, offs));
                tags.push((278, 4, vec![s.rows_per_strip as u64]));
                tags.push((279, ot, counts));
            }
        }
        if !subs.is_empty() {
            tags.push((330, if self.big { 18 } else { 13 }, subs));
        }
        tags.extend(s.extra.iter().cloned());
        tags.sort_by_key(|t| t.0);
        // Out-of-line values.
        let inline = if self.big { 8 } else { 4 };
        let mut placed = Vec::new();
        for (tag, typ, vals) in &tags {
            let b = self.value_bytes(*typ, vals);
            if b.len() > inline {
                if self.buf.len() % 2 == 1 {
                    self.buf.push(0);
                }
                placed.push((
                    *tag,
                    *typ,
                    vals.len(),
                    Some(self.buf.len() as u64),
                    b.clone(),
                ));
                self.buf.extend_from_slice(&b);
            } else {
                placed.push((*tag, *typ, vals.len(), None, b));
            }
        }
        if self.buf.len() % 2 == 1 {
            self.buf.push(0);
        }
        let at = self.buf.len() as u64;
        if self.big {
            self.u64(placed.len() as u64);
        } else {
            self.u16(placed.len() as u16);
        }
        for (tag, typ, n, pos, b) in placed {
            self.u16(tag);
            self.u16(typ);
            if self.big {
                self.u64(n as u64)
            } else {
                self.u32(n as u32)
            }
            match pos {
                Some(p) => self.off(p),
                None => {
                    let mut v = b.clone();
                    v.resize(inline, 0);
                    self.buf.extend_from_slice(&v);
                }
            }
        }
        let next_at = self.buf.len();
        self.off(0);
        (at, next_at)
    }
}

/// Write a file whose main IFD chain is `chain`.
pub fn write(chain: &[Spec], big: bool, le: bool) -> Vec<u8> {
    let mut w = W {
        buf: Vec::new(),
        le,
        big,
    };
    w.buf.extend_from_slice(if le { b"II" } else { b"MM" });
    let first_at;
    if big {
        w.u16(43);
        w.u16(8);
        w.u16(0);
        first_at = w.buf.len();
        w.u64(0);
    } else {
        w.u16(42);
        first_at = w.buf.len();
        w.u32(0);
    }
    let mut prev = first_at;
    for s in chain {
        let (at, next) = w.ifd(s);
        w.patch_off(prev, at);
        prev = next;
    }
    w.buf
}
