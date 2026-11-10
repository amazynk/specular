//! Test helpers: a lossless JPEG (LJ92) encoder and minimal DNG and Sony lossless ARW writers.
#![allow(dead_code)]

use std::path::PathBuf;

// ---------------------------------------------------------------------------------------------
// LJ92 encoder (mirrors the decoder's prediction rules)

struct BitWriter {
    out: Vec<u8>,
    acc: u32,
    n: u32,
}

impl BitWriter {
    fn put(&mut self, v: u32, len: u32) {
        for i in (0..len).rev() {
            self.acc = (self.acc << 1) | ((v >> i) & 1);
            self.n += 1;
            if self.n == 8 {
                let b = self.acc as u8;
                self.out.push(b);
                if b == 0xff {
                    self.out.push(0);
                }
                self.acc = 0;
                self.n = 0;
            }
        }
    }
    fn flush(&mut self) {
        while self.n != 0 {
            self.put(1, 1);
        }
    }
}

pub struct Lj92 {
    pub width: u32,
    pub height: u32,
    pub components: u32,
    pub precision: u8,
    pub predictor: u8,
    pub point_transform: u8,
    pub restart_interval: u16,
}

/// Encode `data` (`width * components` samples per row). All categories use 5-bit codes.
pub fn lj92_encode(p: &Lj92, data: &[u16]) -> Vec<u8> {
    let (w, h, nc) = (p.width as usize, p.height as usize, p.components as usize);
    assert_eq!(data.len(), w * h * nc);
    let mut o = vec![0xff, 0xd8];
    // DHT: table 0, 17 symbols of length 5.
    let mut dht = vec![0x00u8];
    let mut counts = [0u8; 16];
    counts[4] = 17;
    dht.extend_from_slice(&counts);
    dht.extend(0..=16u8);
    o.extend_from_slice(&[0xff, 0xc4]);
    o.extend_from_slice(&((dht.len() + 2) as u16).to_be_bytes());
    o.extend_from_slice(&dht);
    // SOF3
    o.extend_from_slice(&[0xff, 0xc3]);
    o.extend_from_slice(&((8 + 3 * nc) as u16).to_be_bytes());
    o.push(p.precision);
    o.extend_from_slice(&(h as u16).to_be_bytes());
    o.extend_from_slice(&(w as u16).to_be_bytes());
    o.push(nc as u8);
    for c in 0..nc {
        o.extend_from_slice(&[c as u8 + 1, 0x11, 0]);
    }
    if p.restart_interval > 0 {
        o.extend_from_slice(&[0xff, 0xdd, 0, 4]);
        o.extend_from_slice(&p.restart_interval.to_be_bytes());
    }
    // SOS
    o.extend_from_slice(&[0xff, 0xda]);
    o.extend_from_slice(&((6 + 2 * nc) as u16).to_be_bytes());
    o.push(nc as u8);
    for c in 0..nc {
        o.extend_from_slice(&[c as u8 + 1, 0x00]);
    }
    o.extend_from_slice(&[p.predictor, 0, p.point_transform]);

    let pt = p.point_transform as u32;
    let initial = 1i32 << (p.precision as u32 - pt - 1);
    let mask = (1i32 << (p.precision as u32 - pt)) - 1;
    let v = |i: usize| (data[i] >> pt) as i32 & mask;
    let rowlen = w * nc;
    let mut bw = BitWriter {
        out: o,
        acc: 0,
        n: 0,
    };
    let ri = p.restart_interval as usize;
    let mut mcus_left = if ri > 0 { ri } else { usize::MAX };
    let mut rst = 0u8;
    let mut first_line = true;
    for row in 0..h {
        if row > 0 {
            first_line = false;
        }
        for col in 0..w {
            let mut reset = row == 0 && col == 0;
            if ri > 0 {
                if mcus_left == 0 {
                    bw.flush();
                    bw.out.extend_from_slice(&[0xff, 0xd0 + rst]);
                    rst = (rst + 1) & 7;
                    mcus_left = ri;
                    reset = true;
                    first_line = true;
                }
                mcus_left -= 1;
            }
            for c in 0..nc {
                let i = row * rowlen + col * nc + c;
                let pred = if reset {
                    initial
                } else if col == 0 {
                    v(i - rowlen)
                } else if first_line {
                    v(i - nc)
                } else {
                    let (ra, rb, rc) = (v(i - nc), v(i - rowlen), v(i - rowlen - nc));
                    match p.predictor {
                        1 => ra,
                        2 => rb,
                        3 => rc,
                        4 => ra + rb - rc,
                        5 => ra + ((rb - rc) >> 1),
                        6 => rb + ((ra - rc) >> 1),
                        _ => (ra + rb) >> 1,
                    }
                };
                // Difference modulo 2^16 in -32767..=32768.
                let mut d = (v(i) - pred).rem_euclid(65536);
                if d > 32768 {
                    d -= 65536;
                }
                let s = if d == 0 {
                    0
                } else {
                    32 - (d.unsigned_abs()).leading_zeros()
                };
                bw.put(s, 5);
                if s > 0 && s < 16 {
                    let bits = if d < 0 {
                        (d + (1 << s) - 1) as u32
                    } else {
                        d as u32
                    };
                    bw.put(bits, s);
                }
            }
        }
    }
    bw.flush();
    let mut o = bw.out;
    o.extend_from_slice(&[0xff, 0xd9]);
    o
}

// ---------------------------------------------------------------------------------------------
// DNG writer

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DngCodec {
    /// 16 bits per sample, little-endian.
    U16,
    /// `bits` per sample packed MSB-first (DNG bit packing).
    Packed(u32),
    /// LJ92 tiles of `tile x tile` pixels, encoded as `tile/2 x tile` with 2 components.
    Lj92Tiles(u32),
    /// LJ92 single strip, 1 component.
    Lj92Strip,
    /// LJ92 tiles of `tile x tile` pixels with another frame shape or scan: `wide` codes two
    /// tile rows per frame row (1 component, `2 * tile` wide, `tile / 2` high), otherwise as
    /// [`DngCodec::Lj92Tiles`]; `predictor` and `restart` are the scan's.
    Lj92TilesFrame {
        tile: u32,
        wide: bool,
        predictor: u8,
        restart: u16,
    },
    /// Tiles of `tile_w x tile_h` already coded with `compression`, from [`DngSpec::pre`]
    /// (the samples passed to [`write_dng`] are then ignored).
    PreTiled {
        tile_w: u32,
        tile_h: u32,
        compression: u16,
    },
}

pub struct DngSpec {
    pub width: u32,
    pub height: u32,
    pub bits: u32,
    pub cfa: [u8; 4],
    pub codec: DngCodec,
    pub black: u32,
    pub white: u32,
    pub neutral: [f64; 3],
    /// ColorMatrix1 (XYZ → camera), D65.
    pub color_matrix: [f64; 9],
    pub active_area: Option<[u32; 4]>,
    pub linearization: Option<Vec<u16>>,
    /// LinearRaw (3 samples per pixel) instead of CFA.
    pub linear_raw: bool,
    pub orientation: u16,
    /// Extra IFD 0 tags `(tag, type, count, little-endian value bytes)`; a tag listed here
    /// replaces the writer's own entry of that tag.
    pub extra: Vec<Tag>,
    /// Coded tiles for [`DngCodec::PreTiled`], row-major.
    pub pre: Vec<Vec<u8>>,
}

/// An extra FLOAT tag for [`DngSpec::extra`].
/// A raw TIFF entry: tag, type, count, value bytes (little endian).
pub type Tag = (u16, u16, u32, Vec<u8>);

pub fn tag_floats(tag: u16, v: &[f32]) -> Tag {
    let d = v.iter().flat_map(|x| x.to_le_bytes()).collect();
    (tag, 11, v.len() as u32, d)
}

/// An extra LONG tag for [`DngSpec::extra`].
pub fn tag_longs(tag: u16, v: &[u32]) -> Tag {
    let d = v.iter().flat_map(|x| x.to_le_bytes()).collect();
    (tag, 4, v.len() as u32, d)
}

/// An extra ASCII tag for [`DngSpec::extra`].
pub fn tag_ascii(tag: u16, s: &str) -> Tag {
    let mut d = s.as_bytes().to_vec();
    d.push(0);
    (tag, 2, d.len() as u32, d)
}

impl DngSpec {
    pub fn new(width: u32, height: u32, codec: DngCodec) -> DngSpec {
        DngSpec {
            width,
            height,
            bits: 16,
            cfa: [0, 1, 1, 2],
            codec,
            black: 0,
            white: 65535,
            neutral: [1.0, 1.0, 1.0],
            color_matrix: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
            active_area: None,
            linearization: None,
            linear_raw: false,
            orientation: 1,
            extra: Vec::new(),
            pre: Vec::new(),
        }
    }
}

struct Ent {
    tag: u16,
    typ: u16,
    count: u32,
    data: Vec<u8>,
}

fn e_short(tag: u16, v: &[u16]) -> Ent {
    Ent {
        tag,
        typ: 3,
        count: v.len() as u32,
        data: v.iter().flat_map(|x| x.to_le_bytes()).collect(),
    }
}
fn e_long(tag: u16, v: &[u32]) -> Ent {
    Ent {
        tag,
        typ: 4,
        count: v.len() as u32,
        data: v.iter().flat_map(|x| x.to_le_bytes()).collect(),
    }
}
fn e_byte(tag: u16, v: &[u8]) -> Ent {
    Ent {
        tag,
        typ: 1,
        count: v.len() as u32,
        data: v.to_vec(),
    }
}
fn e_ascii(tag: u16, s: &str) -> Ent {
    let mut d = s.as_bytes().to_vec();
    d.push(0);
    Ent {
        tag,
        typ: 2,
        count: d.len() as u32,
        data: d,
    }
}
fn e_srational(tag: u16, v: &[f64]) -> Ent {
    let d = v
        .iter()
        .flat_map(|x| {
            [
                ((x * 10000.0).round() as i32).to_le_bytes(),
                10000i32.to_le_bytes(),
            ]
            .concat()
        })
        .collect();
    Ent {
        tag,
        typ: 10,
        count: v.len() as u32,
        data: d,
    }
}
fn e_rational(tag: u16, v: &[f64]) -> Ent {
    let d = v
        .iter()
        .flat_map(|x| {
            [
                ((x * 1_000_000.0).round() as u32).to_le_bytes(),
                1_000_000u32.to_le_bytes(),
            ]
            .concat()
        })
        .collect();
    Ent {
        tag,
        typ: 5,
        count: v.len() as u32,
        data: d,
    }
}

fn pack_msb(samples: &[u16], bits: u32, rowlen: usize) -> Vec<u8> {
    let mut out = Vec::new();
    for row in samples.chunks(rowlen) {
        let (mut acc, mut n) = (0u64, 0u32);
        for &s in row {
            acc = (acc << bits) | s as u64;
            n += bits;
            while n >= 8 {
                out.push((acc >> (n - 8)) as u8);
                n -= 8;
            }
        }
        if n > 0 {
            out.push((acc << (8 - n)) as u8);
        }
    }
    out
}

/// Build a little-endian DNG holding `data` (`width * height * cpp` samples).
pub fn write_dng(spec: &DngSpec, data: &[u16]) -> Vec<u8> {
    let cpp = if spec.linear_raw { 3 } else { 1 };
    let (w, h) = (spec.width as usize, spec.height as usize);
    if !matches!(spec.codec, DngCodec::PreTiled { .. }) {
        assert_eq!(data.len(), w * h * cpp);
    }
    // Image blocks.
    let mut blocks: Vec<Vec<u8>> = Vec::new();
    let mut tiled: Option<(usize, usize)> = None;
    let mut compression = 1u16;
    match spec.codec {
        DngCodec::U16 => blocks.push(data.iter().flat_map(|v| v.to_le_bytes()).collect()),
        DngCodec::Packed(b) => blocks.push(pack_msb(data, b, w * cpp)),
        DngCodec::PreTiled {
            tile_w,
            tile_h,
            compression: c,
        } => {
            compression = c;
            tiled = Some((tile_w as usize, tile_h as usize));
            blocks.extend(spec.pre.iter().cloned());
        }
        DngCodec::Lj92Strip => {
            compression = 7;
            let p = Lj92 {
                width: (w * cpp) as u32,
                height: h as u32,
                components: 1,
                precision: spec.bits as u8,
                predictor: 1,
                point_transform: 0,
                restart_interval: 0,
            };
            blocks.push(lj92_encode(&p, data));
        }
        DngCodec::Lj92Tiles(_) | DngCodec::Lj92TilesFrame { .. } => {
            let (t, wide, predictor, restart_interval) = match spec.codec {
                DngCodec::Lj92TilesFrame {
                    tile,
                    wide,
                    predictor,
                    restart,
                } => (tile, wide, predictor, restart),
                DngCodec::Lj92Tiles(t) => (t, false, 1, 0),
                _ => unreachable!(),
            };
            compression = 7;
            let t = t as usize;
            tiled = Some((t, t));
            for ty in (0..h).step_by(t) {
                for tx in (0..w).step_by(t) {
                    // Pad partial tiles by edge replication.
                    let mut tile = Vec::with_capacity(t * t * cpp);
                    for y in 0..t {
                        for x in 0..t {
                            let (yy, xx) = ((ty + y).min(h - 1), (tx + x).min(w - 1));
                            for c in 0..cpp {
                                tile.push(data[(yy * w + xx) * cpp + c]);
                            }
                        }
                    }
                    let (width, height, components) = if wide {
                        (t * cpp * 2, t / 2, 1)
                    } else {
                        (t * cpp / 2, t, 2)
                    };
                    let p = Lj92 {
                        width: width as u32,
                        height: height as u32,
                        components,
                        precision: spec.bits as u8,
                        predictor,
                        point_transform: 0,
                        restart_interval,
                    };
                    blocks.push(lj92_encode(&p, &tile));
                }
            }
        }
    }
    let mut out = vec![b'I', b'I', 42, 0, 0, 0, 0, 0];
    let mut offs = Vec::new();
    let mut lens = Vec::new();
    for b in &blocks {
        offs.push(out.len() as u32);
        lens.push(b.len() as u32);
        out.extend_from_slice(b);
        if out.len() % 2 == 1 {
            out.push(0);
        }
    }
    let mut ents = vec![
        e_long(0x00fe, &[0]),
        e_long(0x0100, &[spec.width]),
        e_long(0x0101, &[spec.height]),
        e_short(0x0102, &vec![spec.bits as u16; cpp]),
        e_short(0x0103, &[compression]),
        e_short(0x0106, &[if spec.linear_raw { 34892 } else { 32803 }]),
        e_ascii(0x010f, "Specular"),
        e_ascii(0x0110, "Synthetic DNG"),
        e_short(0x0112, &[spec.orientation]),
        e_short(0x0115, &[cpp as u16]),
        e_byte(0xc612, &[1, 4, 0, 0]),
        e_long(0xc61a, &[spec.black]),
        e_long(0xc61d, &[spec.white]),
        e_srational(0xc621, &spec.color_matrix),
        e_rational(0xc628, &spec.neutral),
        e_short(0xc65a, &[21]),
    ];
    if !spec.linear_raw {
        ents.push(e_short(0x828d, &[2, 2]));
        ents.push(e_byte(0x828e, &spec.cfa));
    }
    if let Some((tw, th)) = tiled {
        ents.push(e_long(0x0142, &[tw as u32]));
        ents.push(e_long(0x0143, &[th as u32]));
        ents.push(e_long(0x0144, &offs));
        ents.push(e_long(0x0145, &lens));
    } else {
        ents.push(e_long(0x0111, &offs));
        ents.push(e_long(0x0116, &[spec.height]));
        ents.push(e_long(0x0117, &lens));
    }
    if let Some(a) = spec.active_area {
        ents.push(e_long(0xc68d, &a));
    }
    if let Some(l) = &spec.linearization {
        ents.push(e_short(0xc618, l));
    }
    for (tag, typ, count, data) in &spec.extra {
        ents.retain(|e| e.tag != *tag);
        ents.push(Ent {
            tag: *tag,
            typ: *typ,
            count: *count,
            data: data.clone(),
        });
    }
    ents.sort_by_key(|e| e.tag);
    // IFD after the data, out-of-line values after the IFD.
    let ifd = out.len() + out.len() % 2;
    out.resize(ifd, 0);
    out[4..8].copy_from_slice(&(ifd as u32).to_le_bytes());
    let mut extra_at = ifd + 2 + ents.len() * 12 + 4;
    let mut extra = Vec::new();
    out.extend_from_slice(&(ents.len() as u16).to_le_bytes());
    for e in &ents {
        out.extend_from_slice(&e.tag.to_le_bytes());
        out.extend_from_slice(&e.typ.to_le_bytes());
        out.extend_from_slice(&e.count.to_le_bytes());
        if e.data.len() <= 4 {
            let mut v = e.data.clone();
            v.resize(4, 0);
            out.extend_from_slice(&v);
        } else {
            out.extend_from_slice(&(extra_at as u32).to_le_bytes());
            extra.extend_from_slice(&e.data);
            if extra.len() % 2 == 1 {
                extra.push(0);
            }
            extra_at = ifd + 2 + ents.len() * 12 + 4 + extra.len();
        }
    }
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&extra);
    out
}

// ---------------------------------------------------------------------------------------------
// Sony lossless compressed ARW writer

/// A Sony lossless ARW (TIFF Compression 7, LJ92 tiles in a raw SubIFD).
pub struct ArwSpec {
    pub width: u32,
    pub height: u32,
    pub tile: u32,
    /// Sony's layout: 4-component frames of half the tile's width and height, one MCU per 2x2
    /// block. Otherwise 2-component frames of half the width, as DNG converters write.
    pub quad: bool,
    pub precision: u8,
    pub predictor: u8,
    pub restart: u16,
    pub black: u16,
    pub white: u32,
    /// Crop (DefaultCropOrigin / DefaultCropSize).
    pub crop: Option<[u32; 4]>,
    /// As-shot white balance (R G G B levels) in an encrypted SR2 private IFD.
    pub wb: Option<[u16; 4]>,
}

impl ArwSpec {
    pub fn new(width: u32, height: u32, tile: u32) -> ArwSpec {
        ArwSpec {
            width,
            height,
            tile,
            quad: true,
            precision: 14,
            predictor: 1,
            restart: 0,
            black: 512,
            white: 16383,
            crop: None,
            wb: None,
        }
    }
}

/// Write an IFD (entries sorted, values after it) at the end of `out`; returns its offset.
fn emit_ifd(out: &mut Vec<u8>, mut ents: Vec<Ent>) -> u32 {
    ents.sort_by_key(|e| e.tag);
    if out.len() % 2 == 1 {
        out.push(0);
    }
    let ifd = out.len();
    let mut extra_at = ifd + 2 + ents.len() * 12 + 4;
    let mut extra = Vec::new();
    out.extend_from_slice(&(ents.len() as u16).to_le_bytes());
    for e in &ents {
        out.extend_from_slice(&e.tag.to_le_bytes());
        out.extend_from_slice(&e.typ.to_le_bytes());
        out.extend_from_slice(&e.count.to_le_bytes());
        if e.data.len() <= 4 {
            let mut v = e.data.clone();
            v.resize(4, 0);
            out.extend_from_slice(&v);
        } else {
            out.extend_from_slice(&(extra_at as u32).to_le_bytes());
            extra.extend_from_slice(&e.data);
            if extra.len() % 2 == 1 {
                extra.push(0);
            }
            extra_at = ifd + 2 + ents.len() * 12 + 4 + extra.len();
        }
    }
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&extra);
    ifd as u32
}

/// Encode one `t x t` tile (row-major samples) the way `spec` says.
fn arw_tile(spec: &ArwSpec, tile: &[u16], t: usize) -> Vec<u8> {
    let mut p = Lj92 {
        width: (t / 2) as u32,
        height: t as u32,
        components: 2,
        precision: spec.precision,
        predictor: spec.predictor,
        point_transform: 0,
        restart_interval: spec.restart,
    };
    if !spec.quad {
        return lj92_encode(&p, tile);
    }
    p.height = (t / 2) as u32;
    p.components = 4;
    let mut mcus = Vec::with_capacity(tile.len());
    for r in 0..t / 2 {
        for c in 0..t / 2 {
            let (y, x) = (2 * r, 2 * c);
            mcus.extend_from_slice(&[
                tile[y * t + x],
                tile[y * t + x + 1],
                tile[(y + 1) * t + x],
                tile[(y + 1) * t + x + 1],
            ]);
        }
    }
    lj92_encode(&p, &mcus)
}

/// Sony's SR2 key stream (dcraw / exiftool): XOR `buf` with it (encrypts and decrypts).
fn sony_crypt(buf: &mut [u8], key: u32) {
    let mut pad = [0u32; 128];
    let mut k = key;
    for p in pad.iter_mut().take(4) {
        k = k.wrapping_mul(48828125).wrapping_add(1);
        *p = k;
    }
    pad[3] = (pad[3] << 1) | ((pad[0] ^ pad[2]) >> 31);
    for p in 4..127 {
        pad[p] = ((pad[p - 4] ^ pad[p - 2]) << 1) | ((pad[p - 3] ^ pad[p - 1]) >> 31);
    }
    let mut p: usize = 127;
    for w in buf.chunks_exact_mut(4) {
        p += 1;
        let v = pad[p & 127] ^ pad[(p + 64) & 127];
        pad[(p - 1) & 127] = v;
        for (b, k) in w.iter_mut().zip(v.to_be_bytes()) {
            *b ^= k;
        }
    }
}

/// Append an SR2Private IFD and its encrypted SR2SubIFD holding WB_RGGBLevels; returns the
/// SR2Private IFD's offset.
fn sr2_private(out: &mut Vec<u8>, wb: [u16; 4]) -> u32 {
    const KEY: u32 = 0x1234_5678;
    while !out.len().is_multiple_of(4) {
        out.push(0);
    }
    let sub = out.len() as u32;
    // One entry (WB_RGGBLevels, SHORT[4]), its value after the IFD.
    let mut b = Vec::new();
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&0x7313u16.to_le_bytes());
    b.extend_from_slice(&3u16.to_le_bytes());
    b.extend_from_slice(&4u32.to_le_bytes());
    b.extend_from_slice(&(sub + 18).to_le_bytes());
    b.extend_from_slice(&0u32.to_le_bytes());
    for v in wb {
        b.extend_from_slice(&v.to_le_bytes());
    }
    b.resize(28, 0);
    sony_crypt(&mut b, KEY);
    let len = b.len() as u32;
    out.extend_from_slice(&b);
    emit_ifd(
        out,
        vec![
            e_long(0x7200, &[sub]),
            e_long(0x7201, &[len]),
            e_long(0x7221, &[KEY]),
        ],
    )
}

/// Build a little-endian Sony lossless ARW holding the RGGB mosaic `data` (`width * height`).
pub fn write_arw(spec: &ArwSpec, data: &[u16]) -> Vec<u8> {
    let (w, h, t) = (
        spec.width as usize,
        spec.height as usize,
        spec.tile as usize,
    );
    assert_eq!(data.len(), w * h);
    let mut out = vec![b'I', b'I', 42, 0, 0, 0, 0, 0];
    let (mut offs, mut lens) = (Vec::new(), Vec::new());
    for ty in (0..h).step_by(t) {
        for tx in (0..w).step_by(t) {
            // Pad partial tiles by edge replication.
            let mut tile = Vec::with_capacity(t * t);
            for y in 0..t {
                for x in 0..t {
                    tile.push(data[(ty + y).min(h - 1) * w + (tx + x).min(w - 1)]);
                }
            }
            let b = arw_tile(spec, &tile, t);
            offs.push(out.len() as u32);
            lens.push(b.len() as u32);
            out.extend_from_slice(&b);
            if out.len() % 2 == 1 {
                out.push(0);
            }
        }
    }
    let mut raw = vec![
        e_long(0x00fe, &[0]),
        e_long(0x0100, &[spec.width]),
        e_long(0x0101, &[spec.height]),
        e_short(0x0102, &[14]),
        e_short(0x0103, &[7]),
        e_short(0x0106, &[32803]),
        e_short(0x0115, &[1]),
        e_long(0x0142, &[spec.tile]),
        e_long(0x0143, &[spec.tile]),
        e_long(0x0144, &offs),
        e_long(0x0145, &lens),
        e_short(0x828d, &[2, 2]),
        e_byte(0x828e, &[0, 1, 1, 2]),
        e_short(0x7310, &[spec.black; 4]),
        e_long(0xc61d, &[spec.white]),
    ];
    if let Some([x, y, cw, ch]) = spec.crop {
        raw.push(e_long(0xc61f, &[x, y]));
        raw.push(e_long(0xc620, &[cw, ch]));
    }
    let raw_ifd = emit_ifd(&mut out, raw);
    let mut ifd0 = vec![
        e_long(0x00fe, &[1]),
        e_ascii(0x010f, "SONY"),
        e_ascii(0x0110, "ILCE-TEST"),
        e_short(0x0112, &[1]),
        e_long(0x014a, &[raw_ifd]),
    ];
    if let Some(wb) = spec.wb {
        ifd0.push(e_long(0xc634, &[sr2_private(&mut out, wb)]));
    }
    let ifd0 = emit_ifd(&mut out, ifd0);
    out[4..8].copy_from_slice(&ifd0.to_le_bytes());
    out
}

/// Deterministic pseudo-random samples with some image-like structure.
pub fn test_pattern(w: usize, h: usize, cpp: usize, bits: u32, seed: u32) -> Vec<u16> {
    let max = ((1u32 << bits) - 1) as f64;
    let mut s = seed.wrapping_mul(2_654_435_761).wrapping_add(1);
    let mut out = Vec::with_capacity(w * h * cpp);
    for y in 0..h {
        for x in 0..w {
            for c in 0..cpp {
                s ^= s << 13;
                s ^= s >> 17;
                s ^= s << 5;
                let smooth =
                    0.5 + 0.4 * ((x as f64 * 0.05 + c as f64).sin() * (y as f64 * 0.07).cos());
                let noise = (s % 64) as f64 / 64.0 * 0.05;
                out.push(((smooth + noise).clamp(0.0, 1.0) * max) as u16);
            }
        }
    }
    // A few extremes to exercise large differences.
    if let Some(v) = out.get_mut(3) {
        *v = max as u16;
    }
    if let Some(v) = out.get_mut(4) {
        *v = 0;
    }
    out
}

/// The sample directory (never committed); tests skip when it is missing.
pub fn samples_dir() -> Option<PathBuf> {
    let p =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../target/agent-jpeg/raw-samples");
    p.is_dir().then_some(p)
}

/// Overwrite the (inline, first) value of `tag` in the first IFD of a little-endian TIFF.
pub fn patch_tag(file: &mut [u8], tag: u16, value: u32) -> bool {
    let ifd = u32::from_le_bytes(file[4..8].try_into().unwrap()) as usize;
    let n = u16::from_le_bytes([file[ifd], file[ifd + 1]]) as usize;
    for i in 0..n {
        let e = ifd + 2 + i * 12;
        if u16::from_le_bytes([file[e], file[e + 1]]) == tag {
            let typ = u16::from_le_bytes([file[e + 2], file[e + 3]]);
            match typ {
                3 => file[e + 8..e + 10].copy_from_slice(&(value as u16).to_le_bytes()),
                _ => file[e + 8..e + 12].copy_from_slice(&value.to_le_bytes()),
            }
            return true;
        }
    }
    false
}

/// XYZ (D65) → linear sRGB: a ColorMatrix that makes "camera RGB" equal to linear sRGB.
pub const XYZ_TO_SRGB: [f64; 9] = [
    3.2404542, -1.5371385, -0.4985314, -0.9692660, 1.8760108, 0.0415560, 0.0556434, -0.2040259,
    1.0572252,
];

/// Linear sRGB (D65) → XYZ (D50), Bradford: a ForwardMatrix for a "camera" whose space is
/// linear sRGB.
pub const SRGB_TO_XYZD50: [f32; 9] = [
    0.4360747, 0.3850649, 0.1430804, 0.2225045, 0.7168786, 0.0606169, 0.0139322, 0.0971045,
    0.7141733,
];

/// A hue/saturation/value table of `dims` filled by `f(hue, sat, val)`.
pub fn hsv_table(dims: [u32; 3], f: impl Fn(u32, u32, u32) -> [f32; 3]) -> Vec<f32> {
    let mut d = Vec::new();
    for v in 0..dims[2] {
        for h in 0..dims[0] {
            for s in 0..dims[1] {
                d.extend(f(h, s, v));
            }
        }
    }
    d
}

/// Profile tags of an Adobe-style camera profile: ForwardMatrix1, a 2.5D HueSatMap and an
/// sRGB-encoded 3D LookTable (all with gentle, non-identity entries).
pub fn demo_profile() -> Vec<Tag> {
    let hsm = [6, 3, 1];
    let look = [4, 2, 3];
    vec![
        tag_floats(0xc714, &SRGB_TO_XYZD50),
        tag_ascii(0xc6f8, "Adobe Standard"),
        tag_longs(0xc6f9, &hsm),
        tag_floats(
            0xc6fa,
            &hsv_table(hsm, |h, s, _| [h as f32 * 2.0, 1.0 + 0.1 * s as f32, 1.0]),
        ),
        tag_longs(0xc725, &look),
        tag_floats(
            0xc726,
            &hsv_table(look, |h, s, v| {
                [
                    -(h as f32),
                    1.0 + 0.05 * s as f32,
                    if s == 0 { 1.0 } else { 0.98 + 0.01 * v as f32 },
                ]
            }),
        ),
        tag_longs(0xc7a4, &[1]),
    ]
}

// ---------------------------------------------------------------------------------------------
// DNG opcode lists

/// Big-endian opcode parameters.
#[derive(Default)]
pub struct OpParams(pub Vec<u8>);

impl OpParams {
    pub fn u32(mut self, v: &[u32]) -> Self {
        for x in v {
            self.0.extend_from_slice(&x.to_be_bytes());
        }
        self
    }
    pub fn f64(mut self, v: &[f64]) -> Self {
        for x in v {
            self.0.extend_from_slice(&x.to_be_bytes());
        }
        self
    }
    pub fn f32(mut self, v: &[f32]) -> Self {
        for x in v {
            self.0.extend_from_slice(&x.to_be_bytes());
        }
        self
    }
    pub fn u16(mut self, v: &[u16]) -> Self {
        for x in v {
            self.0.extend_from_slice(&x.to_be_bytes());
        }
        self
    }
}

/// One opcode: `(id, flags, parameters)`.
pub type Opcode = (u32, u32, OpParams);

/// An `OpcodeList1/2/3` tag (`list` 1..=3) for [`DngSpec::extra`].
pub fn opcode_list(list: u32, ops: Vec<Opcode>) -> Tag {
    let tag = [0xc740, 0xc741, 0xc74e][list as usize - 1];
    let mut d = (ops.len() as u32).to_be_bytes().to_vec();
    for (id, flags, p) in ops {
        d.extend_from_slice(&id.to_be_bytes());
        d.extend_from_slice(&0x0103_0000u32.to_be_bytes());
        d.extend_from_slice(&flags.to_be_bytes());
        d.extend_from_slice(&(p.0.len() as u32).to_be_bytes());
        d.extend_from_slice(&p.0);
    }
    (tag, 7, d.len() as u32, d)
}

/// `WarpRectilinear` with per-plane `kr0..kr3, kt0, kt1` and centre `c`.
pub fn op_warp(planes: &[[f64; 6]], c: [f64; 2]) -> Opcode {
    let mut p = OpParams::default().u32(&[planes.len() as u32]);
    for k in planes {
        p = p.f64(k);
    }
    (1, 0, p.f64(&c))
}

/// `FixVignetteRadial` with `k0..k4` and centre `c`.
pub fn op_vignette(k: [f64; 5], c: [f64; 2]) -> Opcode {
    (3, 0, OpParams::default().f64(&k).f64(&c))
}

/// `GainMap` over area `t, l, b, r` (planes 0..3, pitch 1) with a `pv x ph` map of `planes`
/// planes spanning the image (`gains[v][h][plane]`).
pub fn op_gain_map(area: [u32; 4], planes: u32, pv: u32, ph: u32, gains: &[f32]) -> Opcode {
    let p = OpParams::default()
        .u32(&area)
        .u32(&[0, 3, 1, 1, pv, ph])
        .f64(&[
            1.0 / (pv.max(2) - 1) as f64,
            1.0 / (ph.max(2) - 1) as f64,
            0.0,
            0.0,
        ])
        .u32(&[planes])
        .f32(gains);
    (9, 0, p)
}
