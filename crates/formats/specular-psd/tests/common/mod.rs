//! Tiny PSD/PSB writer for tests and benches.
#![allow(dead_code)]

use std::io::Write;

pub fn packbits(row: &[u8], out: &mut Vec<u8>) {
    let n = row.len();
    let mut i = 0;
    while i < n {
        // Run of at least 3?
        let mut run = 1;
        while i + run < n && run < 128 && row[i + run] == row[i] {
            run += 1;
        }
        if run >= 3 {
            out.push((1i32 - run as i32) as i8 as u8);
            out.push(row[i]);
            i += run;
            continue;
        }
        // Literal until the next run of 3 or 128 bytes.
        let start = i;
        while i < n && i - start < 128 {
            if i + 2 < n && row[i] == row[i + 1] && row[i] == row[i + 2] {
                break;
            }
            i += 1;
        }
        out.push((i - start - 1) as u8);
        out.extend_from_slice(&row[start..i]);
    }
}

pub fn zlib(data: &[u8]) -> Vec<u8> {
    miniz_oxide::deflate::compress_to_vec_zlib(data, 6)
}

/// A layer's user mask: bounds (top, left, bottom, right), default colour, flags.
#[derive(Clone, Default)]
pub struct MaskSpec {
    pub bounds: [i32; 4],
    pub default: u8,
    pub flags: u8,
    /// The "real" user mask beside a vector mask (channel −3): bounds, default, flags.
    pub real: Option<([i32; 4], u8, u8)>,
    /// Mask parameters written after the flags (and the real mask), when flag 0x10 is set.
    pub params: Vec<u8>,
}

#[derive(Clone, Default)]
pub struct LayerSpec {
    pub name: String,
    pub bounds: [i32; 4],
    pub opacity: u8,
    pub hidden: bool,
    /// lsct section type (0 = none).
    pub section: u32,
    /// Channel id and its stored big-endian rows over the layer's bounds (the mask's for
    /// −2 / −3).
    pub channels: Vec<(i16, Vec<u8>)>,
    /// Compression of every channel (0 raw, 1 RLE, 2 ZIP, 3 ZIP with prediction).
    pub compression: u16,
    pub mask: Option<MaskSpec>,
    /// `iOpa`.
    pub fill_opacity: Option<u8>,
    /// Extra additional-layer-info keys written with a 4-byte zero body (e.g. `levl`).
    pub keys: Vec<[u8; 4]>,
    /// Blend-mode key (default `norm`).
    pub blend: Option<[u8; 4]>,
    /// Clipped to the layer below.
    pub clipping: bool,
    /// Additional-layer-info blocks with their bodies (padded to 4 bytes).
    pub blocks: Vec<([u8; 4], Vec<u8>)>,
    /// Blending ranges (Blend If) as stored; empty writes none.
    pub ranges: Vec<u8>,
}

impl LayerSpec {
    /// The bounds channel `id` covers.
    fn channel_bounds(&self, id: i16) -> [i32; 4] {
        match (id, &self.mask) {
            (-2, Some(m)) => m.bounds,
            (-3, Some(MaskSpec { real: Some(r), .. })) => r.0,
            _ => self.bounds,
        }
    }
}

/// One stored layer channel: compression word, then the rows (`h` rows of `rb` bytes).
pub fn encode_channel(
    rows: &[u8],
    rb: usize,
    h: usize,
    comp: u16,
    depth: u16,
    psb: bool,
) -> Vec<u8> {
    let mut out = Vec::new();
    be16(&mut out, comp);
    match comp {
        0 => out.extend_from_slice(rows),
        1 => {
            let mut body = Vec::new();
            for row in rows.chunks(rb.max(1)).take(h) {
                let before = body.len();
                packbits(row, &mut body);
                let n = body.len() - before;
                if psb {
                    be32(&mut out, n as u32);
                } else {
                    be16(&mut out, n as u16);
                }
            }
            out.extend(body);
        }
        2 => out.extend(zlib(rows)),
        3 => {
            let mut all = Vec::new();
            for row in rows.chunks(rb.max(1)) {
                all.extend(predict(row, depth));
            }
            out.extend(zlib(&all));
        }
        _ => unreachable!(),
    }
    out
}

#[derive(Clone)]
pub struct Spec {
    pub psb: bool,
    pub width: u32,
    pub height: u32,
    pub depth: u16,
    pub mode: u16,
    pub compression: u16,
    /// Per channel: `height` rows of stored big-endian bytes.
    pub planes: Vec<Vec<u8>>,
    pub color_data: Vec<u8>,
    pub resources: Vec<(u16, Vec<u8>)>,
    /// Negative: merged alpha; written with `layers.len() == |count|`.
    pub layer_count: i16,
    pub layers: Vec<LayerSpec>,
    /// Put layer info into an `Lr16` block (16-bit documents).
    pub lr16: bool,
    /// Further tagged blocks after the layer info (key, body).
    pub blocks: Vec<([u8; 4], Vec<u8>)>,
}

impl Spec {
    pub fn new(width: u32, height: u32, depth: u16, mode: u16, planes: Vec<Vec<u8>>) -> Spec {
        Spec {
            psb: false,
            width,
            height,
            depth,
            mode,
            compression: 0,
            planes,
            color_data: Vec::new(),
            resources: Vec::new(),
            layer_count: 0,
            layers: Vec::new(),
            lr16: false,
            blocks: Vec::new(),
        }
    }

    pub fn row_bytes(&self) -> usize {
        if self.depth == 1 {
            (self.width as usize).div_ceil(8)
        } else {
            self.width as usize * self.depth as usize / 8
        }
    }
}

fn be16(v: &mut Vec<u8>, x: u16) {
    v.extend_from_slice(&x.to_be_bytes());
}
fn be32(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_be_bytes());
}
fn len_field(v: &mut Vec<u8>, wide: bool, x: u64) {
    if wide {
        v.extend_from_slice(&x.to_be_bytes());
    } else {
        be32(v, x as u32);
    }
}

pub fn unicode(s: &str) -> Vec<u8> {
    let u: Vec<u16> = s.encode_utf16().collect();
    let mut v = Vec::new();
    be32(&mut v, u.len() as u32);
    for c in u {
        be16(&mut v, c);
    }
    v
}

/// Resource 1057 body.
pub fn version_info(real_merged: bool, writer: &str) -> Vec<u8> {
    let mut v = Vec::new();
    be32(&mut v, 1);
    v.push(real_merged as u8);
    v.extend(unicode(writer));
    v.extend(unicode(writer));
    be32(&mut v, 1);
    v
}

fn layer_info(s: &Spec) -> Vec<u8> {
    let mut li = Vec::new();
    let mut channel_data = Vec::new();
    be16(&mut li, s.layer_count as u16);
    for l in &s.layers {
        for b in l.bounds {
            be32(&mut li, b as u32);
        }
        be16(&mut li, l.channels.len() as u16);
        for (id, rows) in &l.channels {
            let [t, lf, b, r] = l.channel_bounds(*id);
            let span = |a: i32, b: i32| (b as i64 - a as i64).clamp(0, 1 << 20) as usize;
            let rb = span(lf, r) * s.depth as usize / 8;
            let h = span(t, b);
            let enc = encode_channel(rows, rb, h, l.compression, s.depth, s.psb);
            be16(&mut li, *id as u16);
            len_field(&mut li, s.psb, enc.len() as u64);
            channel_data.extend(enc);
        }
        li.extend_from_slice(b"8BIM");
        li.extend_from_slice(&l.blend.unwrap_or(*b"norm"));
        li.push(l.opacity);
        li.push(l.clipping as u8);
        li.push(if l.hidden { 2 } else { 0 });
        li.push(0);
        let mut extra = Vec::new();
        match &l.mask {
            None => be32(&mut extra, 0),
            Some(m) => {
                let mut md = Vec::new();
                for b in m.bounds {
                    be32(&mut md, b as u32);
                }
                md.push(m.default);
                md.push(m.flags);
                match m.real {
                    None if m.params.is_empty() => md.extend_from_slice(&[0, 0]),
                    None => {}
                    Some((rb, rd, rf)) => {
                        md.push(rf);
                        md.push(rd);
                        for b in rb {
                            be32(&mut md, b as u32);
                        }
                    }
                }
                md.extend_from_slice(&m.params);
                be32(&mut extra, md.len() as u32);
                extra.extend(md);
            }
        }
        be32(&mut extra, l.ranges.len() as u32);
        extra.extend_from_slice(&l.ranges);
        let name: Vec<u8> = l.name.bytes().filter(|b| b.is_ascii()).take(255).collect();
        extra.push(name.len() as u8);
        extra.extend_from_slice(&name);
        while extra.len() % 4 != 0 {
            extra.push(0);
        }
        let mut luni = unicode(&l.name);
        while !luni.len().is_multiple_of(4) {
            luni.push(0);
        }
        extra.extend_from_slice(b"8BIMluni");
        be32(&mut extra, luni.len() as u32);
        extra.extend(luni);
        if l.section != 0 {
            extra.extend_from_slice(b"8BIMlsct");
            be32(&mut extra, 4);
            be32(&mut extra, l.section);
        }
        if let Some(f) = l.fill_opacity {
            extra.extend_from_slice(b"8BIMiOpa");
            be32(&mut extra, 4);
            extra.extend_from_slice(&[f, 0, 0, 0]);
        }
        for k in &l.keys {
            extra.extend_from_slice(b"8BIM");
            extra.extend_from_slice(k);
            be32(&mut extra, 4);
            be32(&mut extra, 0);
        }
        for (k, body) in &l.blocks {
            extra.extend_from_slice(b"8BIM");
            extra.extend_from_slice(k);
            let mut b = body.clone();
            while b.len() % 4 != 0 {
                b.push(0);
            }
            be32(&mut extra, b.len() as u32);
            extra.extend(b);
        }
        be32(&mut li, extra.len() as u32);
        li.extend(extra);
    }
    li.extend(channel_data);
    if li.len() % 2 == 1 {
        li.push(0);
    }
    li
}

pub fn write(s: &Spec) -> Vec<u8> {
    let mut f = Vec::new();
    f.extend_from_slice(b"8BPS");
    be16(&mut f, if s.psb { 2 } else { 1 });
    f.extend_from_slice(&[0; 6]);
    be16(&mut f, s.planes.len() as u16);
    be32(&mut f, s.height);
    be32(&mut f, s.width);
    be16(&mut f, s.depth);
    be16(&mut f, s.mode);
    be32(&mut f, s.color_data.len() as u32);
    f.extend_from_slice(&s.color_data);
    let mut res = Vec::new();
    for (id, body) in &s.resources {
        res.extend_from_slice(b"8BIM");
        be16(&mut res, *id);
        res.extend_from_slice(&[0, 0]); // empty name, padded
        be32(&mut res, body.len() as u32);
        res.extend_from_slice(body);
        if body.len() % 2 == 1 {
            res.push(0);
        }
    }
    be32(&mut f, res.len() as u32);
    f.extend(res);
    // Layer and mask information.
    let mut lm = Vec::new();
    if s.layer_count != 0 || !s.layers.is_empty() {
        let li = layer_info(s);
        if s.lr16 {
            len_field(&mut lm, s.psb, 0);
            be32(&mut lm, 0); // global mask
            lm.extend_from_slice(b"8BIMLr16");
            len_field(&mut lm, s.psb, li.len() as u64);
            lm.extend(&li);
            while lm.len() % 4 != 0 {
                lm.push(0);
            }
        } else {
            len_field(&mut lm, s.psb, li.len() as u64);
            lm.extend(li);
            be32(&mut lm, 0);
        }
        for (key, body) in &s.blocks {
            lm.extend_from_slice(b"8BIM");
            lm.extend_from_slice(key);
            be32(&mut lm, body.len() as u32);
            lm.extend_from_slice(body);
            while lm.len() % 4 != 0 {
                lm.push(0);
            }
        }
    }
    len_field(&mut f, s.psb, lm.len() as u64);
    f.extend(lm);
    // Image data.
    be16(&mut f, s.compression);
    let rb = s.row_bytes();
    match s.compression {
        0 => s.planes.iter().for_each(|p| f.extend_from_slice(p)),
        1 => {
            let mut table = Vec::new();
            let mut body = Vec::new();
            for p in &s.planes {
                for row in p.chunks(rb) {
                    let before = body.len();
                    packbits(row, &mut body);
                    let n = body.len() - before;
                    if s.psb {
                        be32(&mut table, n as u32);
                    } else {
                        be16(&mut table, n as u16);
                    }
                }
            }
            f.extend(table);
            f.extend(body);
        }
        2 => f.extend(zlib(&s.planes.concat())),
        3 => {
            let mut all = Vec::new();
            for p in &s.planes {
                for row in p.chunks(rb) {
                    all.extend(predict(row, s.depth));
                }
            }
            f.extend(zlib(&all));
        }
        _ => unreachable!(),
    }
    f
}

/// ZIP-with-prediction encoding of one big-endian row.
pub fn predict(row: &[u8], depth: u16) -> Vec<u8> {
    match depth {
        16 => {
            let mut out = Vec::with_capacity(row.len());
            let mut prev = 0u16;
            for c in row.chunks_exact(2) {
                let v = u16::from_be_bytes([c[0], c[1]]);
                out.write_all(&v.wrapping_sub(prev).to_be_bytes()).unwrap();
                prev = v;
            }
            out
        }
        32 => {
            let w = row.len() / 4;
            let mut sh = vec![0u8; row.len()];
            for x in 0..w {
                for b in 0..4 {
                    sh[b * w + x] = row[x * 4 + b];
                }
            }
            delta8(&sh)
        }
        _ => delta8(row),
    }
}

fn delta8(row: &[u8]) -> Vec<u8> {
    let mut out = row.to_vec();
    for i in (1..out.len()).rev() {
        out[i] = out[i].wrapping_sub(out[i - 1]);
    }
    out
}

/// Deterministic test pattern with runs (compressible) and noise (literals).
pub fn pattern(w: u32, h: u32, seed: u32, bytes_per_sample: usize) -> Vec<u8> {
    let mut s = seed.wrapping_mul(2_654_435_761).wrapping_add(1);
    let mut out = Vec::with_capacity(w as usize * h as usize * bytes_per_sample);
    for y in 0..h {
        for x in 0..w {
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            let v = if (x / 7 + y / 5 + seed).is_multiple_of(3) {
                s as u64
            } else {
                ((x / 16 * 37 + y * 11 + seed * 50) as u64) * 0x0101_0101
            };
            for b in 0..bytes_per_sample {
                out.push((v >> (8 * b)) as u8);
            }
        }
    }
    out
}

pub fn be_u16s(p: &[u8]) -> Vec<u16> {
    p.chunks_exact(2)
        .map(|c| u16::from_be_bytes([c[0], c[1]]))
        .collect()
}

pub fn f32_plane(vals: &[f32]) -> Vec<u8> {
    vals.iter().flat_map(|v| v.to_be_bytes()).collect()
}
