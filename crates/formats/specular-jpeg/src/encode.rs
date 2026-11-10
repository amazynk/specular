//! A baseline JPEG encoder for files Specular writes (batch convert): 8-bit greyscale or
//! YCbCr at 4:4:4 or 4:2:0, quality-scaled Annex K quantisation tables, a separable float
//! AAN forward DCT (`std::simd`, eight columns at a time) and optimised Huffman tables (two
//! passes: the quantised blocks are kept, their symbol statistics give per-image tables as in
//! Annex K.2, then the scan is coded). Exif, XMP and ICC (split over APP2 chunks) go into their
//! usual application segments.
//!
//! Every MCU row is a restart interval, so both passes run row-parallel on the current rayon
//! pool and the bytes do not depend on the thread count; the markers cost about 3 bytes a row.

use rayon::prelude::*;
use std::fmt;
use std::simd::StdFloat;
use std::simd::prelude::*;

/// Chroma sampling of a colour image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Subsampling {
    /// Full-resolution chroma.
    S444,
    /// Chroma halved in both directions (2×2 averaged).
    S420,
}

/// The pixels to encode: interleaved 8-bit samples, rows top to bottom without padding.
#[derive(Clone, Copy, Debug)]
pub enum Samples<'a> {
    Gray(&'a [u8]),
    Rgb(&'a [u8]),
}

/// How to encode, and the metadata to embed.
#[derive(Clone, Copy, Debug)]
pub struct EncodeOptions<'a> {
    /// 1 (smallest) ..= 100 (best), the usual IJG scaling of the Annex K tables.
    pub quality: u8,
    pub subsampling: Subsampling,
    /// TIFF-structured Exif (written after "Exif\0\0" in APP1; dropped when over 64 KiB).
    pub exif: Option<&'a [u8]>,
    /// An XMP packet (APP1; dropped when over the 64 KiB a standard packet may use).
    pub xmp: Option<&'a [u8]>,
    /// An ICC profile (APP2 chunks).
    pub icc: Option<&'a [u8]>,
}

impl Default for EncodeOptions<'_> {
    fn default() -> Self {
        EncodeOptions {
            quality: 90,
            subsampling: Subsampling::S420,
            exif: None,
            xmp: None,
            icc: None,
        }
    }
}

/// Why an image could not be encoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EncodeError {
    /// Zero or more than 65535 pixels wide or high (the frame header's limit).
    Size(u32, u32),
    /// The sample buffer does not hold `width × height` pixels.
    DataLength { expected: usize, got: usize },
}

impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EncodeError::Size(w, h) => write!(f, "JPEG cannot hold {w}×{h} pixels"),
            EncodeError::DataLength { expected, got } => {
                write!(f, "expected {expected} bytes of pixel data, got {got}")
            }
        }
    }
}

impl std::error::Error for EncodeError {}

/// Encodes `samples` as a baseline JPEG.
pub fn encode(
    width: u32,
    height: u32,
    samples: Samples<'_>,
    opts: &EncodeOptions<'_>,
) -> Result<Vec<u8>, EncodeError> {
    if width == 0 || height == 0 || width > 65535 || height > 65535 {
        return Err(EncodeError::Size(width, height));
    }
    let (w, h) = (width as usize, height as usize);
    let (data, channels) = match samples {
        Samples::Gray(d) => (d, 1),
        Samples::Rgb(d) => (d, 3),
    };
    if data.len() != w * h * channels {
        return Err(EncodeError::DataLength {
            expected: w * h * channels,
            got: data.len(),
        });
    }
    let color = channels == 3;
    let sub = if color && opts.subsampling == Subsampling::S420 {
        2
    } else {
        1
    };
    let ql = scaled_table(&LUM_Q, opts.quality);
    let qc = scaled_table(&CHROMA_Q, opts.quality);

    // Pass 1: quantised blocks of each MCU row (a restart interval), and their statistics.
    let comps = if color { 3 } else { 1 };
    let per_mcu = if color { sub * sub + 2 } else { 1 };
    let rows = transform(data, w, h, color, sub, &ql, &qc);
    // DC luma, AC luma, DC chroma, AC chroma.
    let freq = rows
        .par_iter()
        .map(|blocks| {
            let mut freq = Box::new([[0u32; 257]; 4]);
            let mut pred = [0i32; 3];
            for (i, b) in blocks.iter().enumerate() {
                let c = component_of(i % per_mcu, sub * sub, comps);
                let t = usize::from(c > 0) * 2;
                let diff = i32::from(b[0]) - pred[c];
                pred[c] = i32::from(b[0]);
                freq[t][magnitude(diff).0 as usize] += 1;
                ac_symbols(b, |sym| freq[t + 1][sym as usize] += 1);
            }
            freq
        })
        .reduce(
            || Box::new([[0u32; 257]; 4]),
            |mut a, b| {
                for (x, y) in a.iter_mut().flatten().zip(b.iter().flatten()) {
                    *x += y;
                }
                a
            },
        );
    let tables: Vec<(u8, [u8; 16], Vec<u8>)> = (0..if color { 4 } else { 2 })
        .map(|t| {
            let (bits, vals) = optimal_table(&freq[t]);
            let class = if t % 2 == 0 { 0x00 } else { 0x10 } | (t / 2) as u8;
            (class, bits, vals)
        })
        .collect();
    let huff: Vec<Huff> = tables.iter().map(|(_, b, v)| Huff::new(b, v)).collect();

    // Headers.
    let mut out = Vec::with_capacity(4096);
    out.extend_from_slice(&[0xFF, 0xD8]);
    let exif = opts.exif.filter(|e| e.len() + 8 <= 65535);
    match exif {
        Some(e) => {
            let mut p = b"Exif\0\0".to_vec();
            p.extend_from_slice(e);
            segment(&mut out, 0xE1, &p);
        }
        // JFIF 1.01, no density unit, 1:1 aspect, no thumbnail.
        None => segment(&mut out, 0xE0, b"JFIF\0\x01\x01\0\0\x01\0\x01\0\0"),
    }
    const XMP_SIG: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";
    if let Some(x) = opts.xmp.filter(|x| x.len() + XMP_SIG.len() + 2 <= 65535) {
        let mut p = XMP_SIG.to_vec();
        p.extend_from_slice(x);
        segment(&mut out, 0xE1, &p);
    }
    if let Some(icc) = opts.icc.filter(|i| !i.is_empty()) {
        const MAX: usize = 65535 - 2 - 14;
        let n = icc.len().div_ceil(MAX);
        if n <= 255 {
            for (k, part) in icc.chunks(MAX).enumerate() {
                let mut p = b"ICC_PROFILE\0".to_vec();
                p.push(k as u8 + 1);
                p.push(n as u8);
                p.extend_from_slice(part);
                segment(&mut out, 0xE2, &p);
            }
        }
    }
    let mut dqt = vec![0u8];
    dqt.extend(ZIGZAG.iter().map(|&i| ql[i] as u8));
    if color {
        dqt.push(1);
        dqt.extend(ZIGZAG.iter().map(|&i| qc[i] as u8));
    }
    segment(&mut out, 0xDB, &dqt);
    let mut sof = vec![8];
    sof.extend_from_slice(&(height as u16).to_be_bytes());
    sof.extend_from_slice(&(width as u16).to_be_bytes());
    if color {
        let s = (sub as u8) << 4 | sub as u8;
        sof.extend_from_slice(&[3, 1, s, 0, 2, 0x11, 1, 3, 0x11, 1]);
    } else {
        sof.extend_from_slice(&[1, 1, 0x11, 0]);
    }
    segment(&mut out, 0xC0, &sof);
    let mut dht = Vec::new();
    for (class, bits, vals) in &tables {
        dht.push(*class);
        dht.extend_from_slice(bits);
        dht.extend_from_slice(vals);
    }
    segment(&mut out, 0xC4, &dht);
    // Restart interval: one MCU row (at most 8192 MCUs, as width ≤ 65535).
    let mw = w.div_ceil(8 * sub);
    segment(&mut out, 0xDD, &(mw as u16).to_be_bytes());
    if color {
        segment(&mut out, 0xDA, &[3, 1, 0x00, 2, 0x11, 3, 0x11, 0, 63, 0]);
    } else {
        segment(&mut out, 0xDA, &[1, 1, 0x00, 0, 63, 0]);
    }

    // Pass 2: the scan, each restart interval on its own, joined by RSTm markers.
    let coded: Vec<Vec<u8>> = rows
        .par_iter()
        .map(|blocks| {
            let mut bw = BitWriter {
                out: Vec::with_capacity(blocks.len() * 24),
                acc: 0,
                n: 0,
            };
            let mut pred = [0i32; 3];
            for (i, b) in blocks.iter().enumerate() {
                let c = component_of(i % per_mcu, sub * sub, comps);
                let t = usize::from(c > 0) * 2;
                let diff = i32::from(b[0]) - pred[c];
                pred[c] = i32::from(b[0]);
                let (n, bits) = magnitude(diff);
                huff[t].put(&mut bw, n as u8);
                bw.put(bits, n);
                encode_ac(&mut bw, b, &huff[t + 1]);
            }
            bw.flush();
            bw.out
        })
        .collect();
    drop(rows);
    out.reserve(coded.iter().map(|c| c.len() + 2).sum::<usize>() + 2);
    for (k, c) in coded.iter().enumerate() {
        if k > 0 {
            out.extend_from_slice(&[0xFF, 0xD0 + ((k - 1) % 8) as u8]);
        }
        out.extend_from_slice(c);
    }
    out.extend_from_slice(&[0xFF, 0xD9]);
    Ok(out)
}

/// The component of the `k`-th block of an MCU holding `luma` luma blocks.
fn component_of(k: usize, luma: usize, comps: usize) -> usize {
    if comps == 1 || k < luma {
        0
    } else {
        k - luma + 1
    }
}

pub(crate) fn segment(out: &mut Vec<u8>, marker: u8, payload: &[u8]) {
    out.extend_from_slice(&[0xFF, marker]);
    out.extend_from_slice(&((payload.len() + 2) as u16).to_be_bytes());
    out.extend_from_slice(payload);
}

/// Quantised blocks (zigzag order) of every MCU row, each in scan order: for colour, `sub²`
/// luma blocks then Cb and Cr; edge pixels are repeated to fill partial MCUs. Rows are
/// transformed in parallel.
fn transform(
    data: &[u8],
    w: usize,
    h: usize,
    color: bool,
    sub: usize,
    ql: &[u16; 64],
    qc: &[u16; 64],
) -> Vec<Vec<[i16; 64]>> {
    let dl = divisors(ql);
    let dc = divisors(qc);
    let mcu = 8 * sub;
    let (mw, mh) = (w.div_ceil(mcu), h.div_ceil(mcu));
    let rw = mw * mcu;
    (0..mh)
        .into_par_iter()
        .map_init(
            || McuRow::new(rw, mcu, color, sub),
            |s, my| s.blocks(data, w, h, my, &dl, &dc),
        )
        .collect()
}

/// Scratch for one MCU row of Y, Cb, Cr as level-shifted floats, `rw` wide; with 4:2:0 also
/// the 2×2-averaged chroma, `rw / 2` wide and 8 rows high.
struct McuRow {
    rw: usize,
    mcu: usize,
    color: bool,
    sub: usize,
    ys: Vec<f32>,
    cbs: Vec<f32>,
    crs: Vec<f32>,
    cbh: Vec<f32>,
    crh: Vec<f32>,
}

impl McuRow {
    fn new(rw: usize, mcu: usize, color: bool, sub: usize) -> McuRow {
        let full = if color { rw * mcu } else { 0 };
        let half = if color && sub == 2 { rw / 2 * 8 } else { 0 };
        McuRow {
            rw,
            mcu,
            color,
            sub,
            ys: vec![0.0; rw * mcu],
            cbs: vec![0.0; full],
            crs: vec![0.0; full],
            cbh: vec![0.0; half],
            crh: vec![0.0; half],
        }
    }

    fn blocks(
        &mut self,
        data: &[u8],
        w: usize,
        h: usize,
        my: usize,
        dl: &[f32x8; 8],
        dc: &[f32x8; 8],
    ) -> Vec<[i16; 64]> {
        let (rw, mcu, color, sub) = (self.rw, self.mcu, self.color, self.sub);
        let ch = if color { 3 } else { 1 };
        for yy in 0..mcu {
            let sy = (my * mcu + yy).min(h - 1);
            let src = &data[sy * w * ch..(sy + 1) * w * ch];
            let ys = &mut self.ys[yy * rw..(yy + 1) * rw];
            if color {
                let cbs = &mut self.cbs[yy * rw..(yy + 1) * rw];
                let crs = &mut self.crs[yy * rw..(yy + 1) * rw];
                for (((p, y), cb), cr) in src
                    .chunks_exact(3)
                    .zip(ys.iter_mut())
                    .zip(cbs.iter_mut())
                    .zip(crs.iter_mut())
                {
                    let (r, g, b) = (f32::from(p[0]), f32::from(p[1]), f32::from(p[2]));
                    *y = 0.299 * r + 0.587 * g + 0.114 * b - 128.0;
                    *cb = -0.168_736 * r - 0.331_264 * g + 0.5 * b;
                    *cr = 0.5 * r - 0.418_688 * g - 0.081_312 * b;
                }
                let (y, cb, cr) = (ys[w - 1], cbs[w - 1], crs[w - 1]);
                ys[w..].fill(y);
                cbs[w..].fill(cb);
                crs[w..].fill(cr);
            } else {
                for (d, &v) in ys.iter_mut().zip(src) {
                    *d = f32::from(v) - 128.0;
                }
                let y = ys[w - 1];
                ys[w..].fill(y);
            }
        }
        if color && sub == 2 {
            let hw = rw / 2;
            for (full, half) in [(&self.cbs, &mut self.cbh), (&self.crs, &mut self.crh)] {
                for y in 0..8 {
                    let (a, b) = (&full[2 * y * rw..][..rw], &full[(2 * y + 1) * rw..][..rw]);
                    for (x, d) in half[y * hw..(y + 1) * hw].iter_mut().enumerate() {
                        *d = (a[2 * x] + a[2 * x + 1] + b[2 * x] + b[2 * x + 1]) * 0.25;
                    }
                }
            }
        }
        let mw = rw / mcu;
        let per = if color { sub * sub + 2 } else { 1 };
        let mut out = Vec::with_capacity(mw * per);
        let load = |plane: &[f32], stride: usize, x: usize| -> [f32x8; 8] {
            std::array::from_fn(|r| f32x8::from_slice(&plane[r * stride + x..][..8]))
        };
        for mx in 0..mw {
            for by in 0..sub {
                for bx in 0..sub {
                    let rows = load(&self.ys[by * 8 * rw..], rw, mx * mcu + bx * 8);
                    out.push(quantise(rows, dl));
                }
            }
            if color {
                if sub == 1 {
                    out.push(quantise(load(&self.cbs, rw, mx * 8), dc));
                    out.push(quantise(load(&self.crs, rw, mx * 8), dc));
                } else {
                    let hw = rw / 2;
                    out.push(quantise(load(&self.cbh, hw, mx * 8), dc));
                    out.push(quantise(load(&self.crh, hw, mx * 8), dc));
                }
            }
        }
        out
    }
}

/// AAN output scale per frequency: 1 for DC, `√2·cos(kπ/16)` otherwise.
fn aan_scale(k: usize) -> f32 {
    if k == 0 {
        1.0
    } else {
        std::f32::consts::SQRT_2 * (k as f32 * std::f32::consts::PI / 16.0).cos()
    }
}

/// Reciprocals of the effective divisors (quantiser × AAN scales × 8), transposed: vector `h`,
/// lane `v` is the coefficient of vertical frequency `v` and horizontal frequency `h`.
fn divisors(q: &[u16; 64]) -> [f32x8; 8] {
    std::array::from_fn(|h| {
        f32x8::from_array(std::array::from_fn(|v| {
            1.0 / (f32::from(q[v * 8 + h]) * aan_scale(v) * aan_scale(h) * 8.0)
        }))
    })
}

/// Eight 8-point AAN forward DCTs at once, one per lane, over `v[0..8]`.
#[inline(always)]
fn fdct8(v: &mut [f32x8; 8]) {
    let k = f32x8::splat;
    let (t0, t7) = (v[0] + v[7], v[0] - v[7]);
    let (t1, t6) = (v[1] + v[6], v[1] - v[6]);
    let (t2, t5) = (v[2] + v[5], v[2] - v[5]);
    let (t3, t4) = (v[3] + v[4], v[3] - v[4]);
    // Even part.
    let (t10, t13) = (t0 + t3, t0 - t3);
    let (t11, t12) = (t1 + t2, t1 - t2);
    v[0] = t10 + t11;
    v[4] = t10 - t11;
    let z1 = (t12 + t13) * k(std::f32::consts::FRAC_1_SQRT_2);
    v[2] = t13 + z1;
    v[6] = t13 - z1;
    // Odd part.
    let a = t4 + t5;
    let b = t5 + t6;
    let c = t6 + t7;
    let z5 = (a - c) * k(0.382_683_43);
    let z2 = k(0.541_196_1) * a + z5;
    let z4 = k(1.306_563) * c + z5;
    let z3 = b * k(std::f32::consts::FRAC_1_SQRT_2);
    let z11 = t7 + z3;
    let z13 = t7 - z3;
    v[5] = z13 + z2;
    v[3] = z13 - z2;
    v[1] = z11 + z4;
    v[7] = z11 - z4;
}

fn transpose(v: &[f32x8; 8]) -> [f32x8; 8] {
    let a: [[f32; 8]; 8] = v.map(|r| r.to_array());
    std::array::from_fn(|i| f32x8::from_array(std::array::from_fn(|j| a[j][i])))
}

/// Forward DCT and quantisation of one level-shifted block given as its eight rows; the result
/// is in zigzag order.
fn quantise(rows: [f32x8; 8], div: &[f32x8; 8]) -> [i16; 64] {
    let mut v = rows;
    // Columns (lane = x), then rows: afterwards vector h, lane v holds coefficient (v, h).
    fdct8(&mut v);
    let mut v = transpose(&v);
    fdct8(&mut v);
    let mut t = [0i16; 64];
    for (h, (x, d)) in v.iter().zip(div).enumerate() {
        let q = (*x * *d)
            .round()
            .simd_clamp(f32x8::splat(-2047.0), f32x8::splat(2047.0))
            .cast::<i16>();
        t[h * 8..h * 8 + 8].copy_from_slice(q.as_array());
    }
    std::array::from_fn(|z| {
        let i = ZIGZAG[z];
        t[(i % 8) * 8 + i / 8]
    })
}

/// Calls `f` with each AC symbol (run/size byte) of a zigzag block, as [`encode_ac`] codes them.
fn ac_symbols(b: &[i16; 64], mut f: impl FnMut(u8)) {
    let mut run = 0u32;
    for &c in &b[1..] {
        if c == 0 {
            run += 1;
            continue;
        }
        while run > 15 {
            f(0xF0);
            run -= 16;
        }
        f((run << 4 | magnitude(i32::from(c)).0) as u8);
        run = 0;
    }
    if run > 0 {
        f(0x00);
    }
}

fn encode_ac(bw: &mut BitWriter, b: &[i16; 64], ac: &Huff) {
    let mut run = 0u32;
    for &c in &b[1..] {
        if c == 0 {
            run += 1;
            continue;
        }
        while run > 15 {
            ac.put(bw, 0xF0);
            run -= 16;
        }
        let (n, bits) = magnitude(i32::from(c));
        ac.put(bw, (run << 4 | n) as u8);
        bw.put(bits, n);
        run = 0;
    }
    if run > 0 {
        ac.put(bw, 0x00);
    }
}

/// Bit count and the JPEG representation of a coefficient (negative values one's-complemented).
pub(crate) fn magnitude(v: i32) -> (u32, u32) {
    let a = v.unsigned_abs();
    let n = 32 - a.leading_zeros();
    let bits = if v < 0 {
        (v - 1) as u32 & ((1u32 << n) - 1)
    } else {
        v as u32
    };
    (n, bits)
}

fn scaled_table(base: &[u16; 64], quality: u8) -> [u16; 64] {
    let q = u32::from(quality.clamp(1, 100));
    let s = if q < 50 { 5000 / q } else { 200 - 2 * q };
    std::array::from_fn(|i| ((u32::from(base[i]) * s + 50) / 100).clamp(1, 255) as u16)
}

/// An optimal length-limited Huffman table (BITS, HUFFVAL) for symbol counts `freq[..256]`,
/// as ITU T.81 Annex K.2 builds it: one extra symbol with count 1 keeps the all-ones code
/// unused, lengths over 16 are folded back, and values are listed by length then symbol.
pub(crate) fn optimal_table(counts: &[u32; 257]) -> ([u8; 16], Vec<u8>) {
    let mut freq: [u64; 257] = std::array::from_fn(|i| u64::from(counts[i]));
    freq[256] = 1;
    let mut size = [0usize; 257];
    let mut others = [usize::MAX; 257];
    loop {
        // v1: smallest non-zero count (the largest symbol on ties); v2: the next smallest.
        let mut v1 = usize::MAX;
        for i in 0..257 {
            if freq[i] > 0 && (v1 == usize::MAX || freq[i] <= freq[v1]) {
                v1 = i;
            }
        }
        let mut v2 = usize::MAX;
        for i in 0..257 {
            if i != v1 && freq[i] > 0 && (v2 == usize::MAX || freq[i] <= freq[v2]) {
                v2 = i;
            }
        }
        if v2 == usize::MAX {
            break;
        }
        freq[v1] += freq[v2];
        freq[v2] = 0;
        let mut k = v1;
        size[k] += 1;
        while others[k] != usize::MAX {
            k = others[k];
            size[k] += 1;
        }
        others[k] = v2;
        let mut k = v2;
        size[k] += 1;
        while others[k] != usize::MAX {
            k = others[k];
            size[k] += 1;
        }
    }
    let mut bits = [0u32; 33];
    for &s in &size {
        if s > 0 {
            bits[s.min(32)] += 1;
        }
    }
    for i in (17..=32).rev() {
        while bits[i] > 0 {
            let mut j = i - 2;
            while bits[j] == 0 {
                j -= 1;
            }
            bits[i] -= 2;
            bits[i - 1] += 1;
            bits[j + 1] += 2;
            bits[j] -= 1;
        }
    }
    let mut i = 16;
    while bits[i] == 0 {
        i -= 1;
    }
    bits[i] -= 1; // the reserved symbol's code
    let mut vals = Vec::new();
    for len in 1..=32 {
        for (sym, &s) in size.iter().enumerate().take(256) {
            if s == len {
                vals.push(sym as u8);
            }
        }
    }
    let out: [u8; 16] = std::array::from_fn(|k| bits[k + 1] as u8);
    (out, vals)
}

pub(crate) struct Huff {
    code: [u16; 256],
    len: [u8; 256],
}

impl Huff {
    pub(crate) fn new(bits: &[u8; 16], vals: &[u8]) -> Huff {
        let mut h = Huff {
            code: [0; 256],
            len: [0; 256],
        };
        let mut code = 0u16;
        let mut k = 0;
        for (l, &n) in bits.iter().enumerate() {
            for _ in 0..n {
                h.code[vals[k] as usize] = code;
                h.len[vals[k] as usize] = l as u8 + 1;
                code += 1;
                k += 1;
            }
            code <<= 1;
        }
        h
    }

    #[inline]
    pub(crate) fn put(&self, bw: &mut BitWriter, sym: u8) {
        bw.put(
            u32::from(self.code[sym as usize]),
            u32::from(self.len[sym as usize]),
        );
    }
}

/// MSB-first bit writer with 0xFF byte stuffing.
pub(crate) struct BitWriter {
    pub(crate) out: Vec<u8>,
    pub(crate) acc: u64,
    pub(crate) n: u32,
}

impl BitWriter {
    #[inline]
    pub(crate) fn put(&mut self, bits: u32, n: u32) {
        if n == 0 {
            return;
        }
        self.acc = self.acc << n | u64::from(bits & ((1u32 << n) - 1));
        self.n += n;
        while self.n >= 8 {
            let b = (self.acc >> (self.n - 8)) as u8;
            self.out.push(b);
            if b == 0xFF {
                self.out.push(0);
            }
            self.n -= 8;
        }
        self.acc &= (1u64 << self.n) - 1;
    }

    pub(crate) fn flush(&mut self) {
        let pad = (8 - self.n % 8) % 8;
        self.put((1 << pad) - 1, pad);
    }
}

/// Natural (row-major) index of each zigzag position.
const ZIGZAG: [usize; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20,
    13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59,
    52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
];

/// ITU T.81 Annex K.1 luminance table (natural order).
const LUM_Q: [u16; 64] = [
    16, 11, 10, 16, 24, 40, 51, 61, 12, 12, 14, 19, 26, 58, 60, 55, 14, 13, 16, 24, 40, 57, 69, 56,
    14, 17, 22, 29, 51, 87, 80, 62, 18, 22, 37, 56, 68, 109, 103, 77, 24, 35, 55, 64, 81, 104, 113,
    92, 49, 64, 78, 87, 103, 121, 120, 101, 72, 92, 95, 98, 112, 100, 103, 99,
];

/// ITU T.81 Annex K.1 chrominance table (natural order).
const CHROMA_Q: [u16; 64] = [
    17, 18, 24, 47, 99, 99, 99, 99, 18, 21, 26, 66, 99, 99, 99, 99, 24, 26, 56, 99, 99, 99, 99, 99,
    47, 66, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99,
    99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99,
];

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Tests may start unnamed threads.
mod tests {
    use super::*;

    fn pattern(w: usize, h: usize, ch: usize) -> Vec<u8> {
        (0..w * h * ch)
            .map(|i| {
                let (p, c) = (i / ch, i % ch);
                let (x, y) = (p % w, p / w);
                ((x * 255 / w.max(1)) as f32 * 0.6
                    + (y * 255 / h.max(1)) as f32 * 0.3
                    + (c * 30) as f32
                    + if (x / 4 + y / 4) % 2 == 0 { 20.0 } else { 0.0 }) as u8
            })
            .collect()
    }

    fn decode(jpg: &[u8]) -> crate::DecodedPlanes {
        let opts = crate::DecodeOptions {
            scale: crate::Scale::Full,
            region: None,
            cancel: None,
            parallelism: crate::Parallelism::Single,
        };
        crate::decode(jpg, &opts).unwrap()
    }

    fn psnr(a: &[u8], b: &[u8]) -> f64 {
        let mse = a
            .iter()
            .zip(b)
            .map(|(&x, &y)| (f64::from(x) - f64::from(y)).powi(2))
            .sum::<f64>()
            / a.len() as f64;
        10.0 * (255.0f64.powi(2) / mse.max(1e-9)).log10()
    }

    #[test]
    fn colour_roundtrip_both_samplings() {
        for (w, h) in [(1, 1), (7, 5), (16, 16), (33, 47), (200, 120)] {
            let rgb = pattern(w, h, 3);
            for sub in [Subsampling::S444, Subsampling::S420] {
                let opts = EncodeOptions {
                    quality: 95,
                    subsampling: sub,
                    ..EncodeOptions::default()
                };
                let jpg = encode(w as u32, h as u32, Samples::Rgb(&rgb), &opts).unwrap();
                let p = decode(&jpg);
                assert_eq!((p.width, p.height), (w as u32, h as u32));
                let rgba = crate::planes_to_rgba(&p);
                let back: Vec<u8> = rgba.chunks(4).flat_map(|q| q[..3].to_vec()).collect();
                if w * h >= 256 {
                    let db = psnr(&rgb, &back);
                    assert!(db > 30.0, "{w}x{h} {sub:?}: {db:.1} dB");
                }
            }
        }
    }

    #[test]
    fn rows_are_restart_intervals_and_threads_do_not_change_bytes() {
        let (w, h) = (300, 170);
        let rgb = pattern(w, h, 3);
        let enc = |threads| {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap();
            pool.install(|| {
                encode(
                    w as u32,
                    h as u32,
                    Samples::Rgb(&rgb),
                    &EncodeOptions::default(),
                )
                .unwrap()
            })
        };
        let jpg = enc(1);
        assert_eq!(jpg, enc(4));
        let mw = w.div_ceil(16);
        let dri = [0xFF, 0xDD, 0, 4, (mw >> 8) as u8, mw as u8];
        assert!(jpg.windows(6).any(|x| x == dri));
        let rst = jpg
            .windows(2)
            .filter(|x| x[0] == 0xFF && (0xD0..=0xD7).contains(&x[1]))
            .count();
        assert_eq!(rst, h.div_ceil(16) - 1);
        // An independent decoder reads it too.
        let mut d = jpeg_decoder::Decoder::new(&jpg[..]);
        let px = d.decode().unwrap();
        assert!(psnr(&rgb, &px) > 30.0);
    }

    #[test]
    fn grey_roundtrip_and_quality_order() {
        let (w, h) = (96, 64);
        let g = pattern(w, h, 1);
        let mut sizes = Vec::new();
        for q in [30, 75, 98] {
            let opts = EncodeOptions {
                quality: q,
                ..EncodeOptions::default()
            };
            let jpg = encode(w as u32, h as u32, Samples::Gray(&g), &opts).unwrap();
            let p = decode(&jpg);
            let back: Vec<u8> = crate::planes_to_rgba(&p).chunks(4).map(|q| q[0]).collect();
            let db = psnr(&g, &back);
            assert!(db > if q > 90 { 40.0 } else { 28.0 }, "q{q}: {db:.1} dB");
            sizes.push(jpg.len());
        }
        assert!(sizes[0] < sizes[1] && sizes[1] < sizes[2], "{sizes:?}");
    }

    #[test]
    fn metadata_segments() {
        let rgb = pattern(20, 10, 3);
        let icc = vec![7u8; 70_000]; // two APP2 chunks
        let opts = EncodeOptions {
            exif: Some(b"MM\0\x2a\0\0\0\x08\0\0"),
            xmp: Some(b"<x:xmpmeta/>"),
            icc: Some(&icc),
            ..EncodeOptions::default()
        };
        let jpg = encode(20, 10, Samples::Rgb(&rgb), &opts).unwrap();
        assert_eq!(&jpg[2..4], &[0xFF, 0xE1]);
        assert_eq!(&jpg[6..12], b"Exif\0\0");
        let find = |n: &[u8]| jpg.windows(n.len()).filter(|w| *w == n).count();
        assert_eq!(find(b"ICC_PROFILE\0"), 2);
        assert_eq!(find(b"http://ns.adobe.com/xap/1.0/\0<x:xmpmeta/>"), 1);
        assert_eq!(find(b"JFIF"), 0);
        assert_eq!(decode(&jpg).width, 20);
    }

    #[test]
    fn huffman_tables_are_limited_and_complete() {
        // Geometric counts force codes longer than 16 bits before folding.
        let mut f = [0u32; 257];
        for (i, v) in f.iter_mut().enumerate().take(40) {
            *v = 1 << (i % 30).min(30);
        }
        let (bits, vals) = optimal_table(&f);
        assert_eq!(bits.iter().map(|&b| b as usize).sum::<usize>(), vals.len());
        assert_eq!(vals.len(), 40);
        // Kraft: the code space is not over-full and leaves the all-ones code.
        let k: f64 = bits
            .iter()
            .enumerate()
            .map(|(l, &n)| f64::from(n) / 2f64.powi(l as i32 + 1))
            .sum();
        assert!(k < 1.0, "{k}");
    }

    #[test]
    fn rejects_bad_sizes() {
        assert_eq!(
            encode(0, 1, Samples::Gray(&[]), &EncodeOptions::default()),
            Err(EncodeError::Size(0, 1))
        );
        assert!(matches!(
            encode(2, 2, Samples::Rgb(&[0; 5]), &EncodeOptions::default()),
            Err(EncodeError::DataLength { .. })
        ));
    }
}
