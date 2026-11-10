//! Tiny test-only encoders for every format in specular-legacy.
#![allow(dead_code)]

pub fn rng(seed: u64) -> impl FnMut() -> u64 {
    let mut s = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    move || {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        s
    }
}

/// Bytes with some runs (so RLE encoders produce both packet kinds).
pub fn pattern(n: usize, seed: u64, max: u8) -> Vec<u8> {
    let mut r = rng(seed);
    let mut v = Vec::with_capacity(n);
    while v.len() < n {
        let x = (r() % (max as u64 + 1)) as u8;
        let rep = if r().is_multiple_of(3) {
            1 + r() % 9
        } else {
            1
        };
        for _ in 0..rep {
            v.push(x);
        }
    }
    v.truncate(n);
    v
}

// ---- PCX ------------------------------------------------------------------------------------

pub fn pcx_rle(line: &[u8], out: &mut Vec<u8>) {
    let mut i = 0;
    while i < line.len() {
        let v = line[i];
        let mut n = 1;
        while i + n < line.len() && line[i + n] == v && n < 63 {
            n += 1;
        }
        if n > 1 || v >= 0xC0 {
            out.extend([0xC0 | n as u8, v]);
        } else {
            out.push(v);
        }
        i += n;
    }
}

/// `lines[y]` = the raw scan line (planes × bpl bytes).
#[allow(clippy::too_many_arguments)]
pub fn pcx(
    w: u16,
    h: u16,
    bpp: u8,
    planes: u8,
    bpl: u16,
    rle: bool,
    pal16: &[[u8; 3]],
    lines: &[Vec<u8>],
    vga: Option<&[[u8; 3]; 256]>,
) -> Vec<u8> {
    let mut d = vec![0u8; 128];
    d[0] = 0x0A;
    d[1] = 5;
    d[2] = rle as u8;
    d[3] = bpp;
    d[8..10].copy_from_slice(&(w - 1).to_le_bytes());
    d[10..12].copy_from_slice(&(h - 1).to_le_bytes());
    for (i, c) in pal16.iter().take(16).enumerate() {
        d[16 + i * 3..19 + i * 3].copy_from_slice(c);
    }
    d[65] = planes;
    d[66..68].copy_from_slice(&bpl.to_le_bytes());
    d[68] = 1;
    for l in lines {
        if rle {
            pcx_rle(l, &mut d);
        } else {
            d.extend_from_slice(l);
        }
    }
    if let Some(p) = vga {
        d.push(0x0C);
        for c in p {
            d.extend_from_slice(c);
        }
    }
    d
}

// ---- SGI ------------------------------------------------------------------------------------

fn sgi_rle_row(row: &[u16], bpc: usize, out: &mut Vec<u8>) {
    let put = |out: &mut Vec<u8>, v: u16| {
        if bpc == 1 {
            out.push(v as u8)
        } else {
            out.extend(v.to_be_bytes())
        }
    };
    let mut i = 0;
    while i < row.len() {
        let mut n = 1;
        while i + n < row.len() && row[i + n] == row[i] && n < 127 {
            n += 1;
        }
        if n >= 2 {
            put(out, n as u16);
            put(out, row[i]);
            i += n;
        } else {
            let s = i;
            while i < row.len() && i - s < 127 && !(i + 1 < row.len() && row[i + 1] == row[i]) {
                i += 1;
            }
            if i == s {
                i += 1;
            }
            put(out, 0x80 | (i - s) as u16);
            for &v in &row[s..i] {
                put(out, v);
            }
        }
    }
    put(out, 0);
}

/// `chans[c][y * w + x]`, top row first (stored bottom-up).
pub fn sgi(w: u16, h: u16, bpc: u8, rle: bool, chans: &[Vec<u16>]) -> Vec<u8> {
    let z = chans.len();
    let mut d = vec![0u8; 512];
    d[0..2].copy_from_slice(&[0x01, 0xDA]);
    d[2] = rle as u8;
    d[3] = bpc;
    d[4..6].copy_from_slice(&(if z == 1 { 2u16 } else { 3 }).to_be_bytes());
    d[6..8].copy_from_slice(&w.to_be_bytes());
    d[8..10].copy_from_slice(&h.to_be_bytes());
    d[10..12].copy_from_slice(&(z as u16).to_be_bytes());
    d[16..20].copy_from_slice(&(if bpc == 1 { 255u32 } else { 65535 }).to_be_bytes());
    let (w, h) = (w as usize, h as usize);
    let rows = |c: usize, y: usize| &chans[c][(h - 1 - y) * w..(h - y) * w];
    if rle {
        let tl = h * z * 4;
        let mut starts = Vec::new();
        let mut lens = Vec::new();
        let mut body = Vec::new();
        for c in 0..z {
            for y in 0..h {
                let s = body.len();
                sgi_rle_row(rows(c, y), bpc as usize, &mut body);
                starts.extend(((512 + 2 * tl + s) as u32).to_be_bytes());
                lens.extend(((body.len() - s) as u32).to_be_bytes());
            }
        }
        d.extend(starts);
        d.extend(lens);
        d.extend(body);
    } else {
        for c in 0..z {
            for y in 0..h {
                for &v in rows(c, y) {
                    if bpc == 1 {
                        d.push(v as u8)
                    } else {
                        d.extend(v.to_be_bytes())
                    }
                }
            }
        }
    }
    d
}

// ---- Sun Raster -----------------------------------------------------------------------------

pub fn sun_rle(src: &[u8]) -> Vec<u8> {
    let mut o = Vec::new();
    let mut i = 0;
    while i < src.len() {
        let v = src[i];
        let mut n = 1;
        while i + n < src.len() && src[i + n] == v && n < 256 {
            n += 1;
        }
        if n >= 3 || v == 0x80 {
            if n == 1 {
                o.extend([0x80, 0]);
            } else if n == 2 && v == 0x80 {
                o.extend([0x80, 0, 0x80, 0]);
            } else {
                o.extend([0x80, (n - 1) as u8, v]);
            }
        } else {
            o.extend(std::iter::repeat_n(v, n));
        }
        i += n;
    }
    o
}

/// `rows` = padded raw rows concatenated.
pub fn sun(w: u32, h: u32, depth: u32, kind: u32, map: &[[u8; 3]], raw: &[u8]) -> Vec<u8> {
    let body = if kind == 2 {
        sun_rle(raw)
    } else {
        raw.to_vec()
    };
    let mut d = Vec::new();
    for v in [
        0x59A6_6A95,
        w,
        h,
        depth,
        body.len() as u32,
        kind,
        if map.is_empty() { 0 } else { 1 },
        map.len() as u32 * 3,
    ] {
        d.extend(v.to_be_bytes());
    }
    for c in 0..3 {
        d.extend(map.iter().map(|m| m[c]));
    }
    d.extend(body);
    d
}

// ---- Text formats ---------------------------------------------------------------------------

/// `bits[y * w + x]` true = black.
pub fn xbm(w: usize, h: usize, bits: &[bool], x10: bool) -> Vec<u8> {
    let unit = if x10 { 16 } else { 8 };
    let mut s = format!(
        "#define t_width {w}\n#define t_height {h}\nstatic {} t_bits[] = {{\n",
        if x10 { "short" } else { "unsigned char" }
    );
    for y in 0..h {
        for u in 0..w.div_ceil(unit) {
            let mut v = 0u32;
            for b in 0..unit {
                let x = u * unit + b;
                if x < w && bits[y * w + x] {
                    v |= 1 << b;
                }
            }
            if (y + u) % 2 == 0 {
                s += &format!("0x{v:02x}, ");
            } else {
                s += &format!("{v}, ");
            }
        }
        s.push('\n');
    }
    s += "};\n";
    s.into_bytes()
}

pub fn wbmp(w: u32, h: u32, white: &[bool]) -> Vec<u8> {
    fn mbi(v: u32, d: &mut Vec<u8>) {
        let mut groups = vec![(v & 0x7F) as u8];
        let mut v = v >> 7;
        while v > 0 {
            groups.push((v & 0x7F) as u8 | 0x80);
            v >>= 7;
        }
        d.extend(groups.iter().rev());
    }
    let mut d = vec![0, 0];
    mbi(w, &mut d);
    mbi(h, &mut d);
    let (w, h) = (w as usize, h as usize);
    for y in 0..h {
        for b in 0..w.div_ceil(8) {
            let mut v = 0u8;
            for k in 0..8 {
                let x = b * 8 + k;
                if x < w && white[y * w + x] {
                    v |= 0x80 >> k;
                }
            }
            d.push(v);
        }
    }
    d
}

pub fn farbfeld(w: u32, h: u32, px: &[u16]) -> Vec<u8> {
    let mut d = b"farbfeld".to_vec();
    d.extend(w.to_be_bytes());
    d.extend(h.to_be_bytes());
    for v in px {
        d.extend(v.to_be_bytes());
    }
    d
}

// ---- FLI / FLC ------------------------------------------------------------------------------

fn sub(kind: u16, body: &[u8]) -> Vec<u8> {
    let mut c = ((body.len() + 6) as u32).to_le_bytes().to_vec();
    c.extend(kind.to_le_bytes());
    c.extend_from_slice(body);
    c
}

fn color_chunk(pal: &[[u8; 3]; 256], six_bit: bool) -> Vec<u8> {
    let mut b = vec![1, 0, 0, 0]; // one packet: skip 0, count 0 = 256
    for c in pal {
        for &v in c {
            b.push(if six_bit { v >> 2 } else { v });
        }
    }
    sub(if six_bit { 11 } else { 4 }, &b)
}

fn brun(img: &[u8], w: usize) -> Vec<u8> {
    let mut b = Vec::new();
    for line in img.chunks(w) {
        b.push(0);
        let mut i = 0;
        while i < w {
            let v = line[i];
            let mut n = 1;
            while i + n < w && line[i + n] == v && n < 127 {
                n += 1;
            }
            if n >= 3 {
                b.extend([n as u8, v]);
                i += n;
            } else {
                let s = i;
                while i < w
                    && i - s < 120
                    && !(i + 2 < w && line[i + 1] == line[i] && line[i + 2] == line[i])
                {
                    i += 1;
                }
                if i == s {
                    i += 1;
                }
                b.push((-((i - s) as i32)) as u8);
                b.extend_from_slice(&line[s..i]);
            }
        }
    }
    sub(15, &b)
}

/// Changed-span packets for one line (byte units); `unit` 1 = LC, 2 = SS2 words.
fn line_packets(prev: &[u8], cur: &[u8], unit: usize) -> (u16, Vec<u8>) {
    let n = cur.len() / unit;
    fn at(s: &[u8], i: usize, unit: usize) -> &[u8] {
        &s[i * unit..(i + 1) * unit]
    }
    let word = |s, i| at(s, i, unit);
    let mut out = Vec::new();
    let mut packets = 0u16;
    let mut i = 0;
    let mut x = 0; // position in units after the last packet
    while i < n {
        if word(prev, i) == word(cur, i) {
            i += 1;
            continue;
        }
        // Skip (bytes, max 255): emit empty literal packets for long gaps.
        while (i - x) * unit > 254 {
            let step = 254 / unit;
            out.extend([(step * unit) as u8, 0]);
            packets += 1;
            x += step;
        }
        let skip = ((i - x) * unit) as u8;
        let mut r = 1;
        while i + r < n && word(cur, i + r) == word(cur, i) && r < 100 {
            r += 1;
        }
        if r >= 3 {
            out.extend([skip, (-(r as i32)) as u8]);
            out.extend_from_slice(word(cur, i));
            i += r;
        } else {
            let s = i;
            while i < n && i - s < 100 && word(prev, i) != word(cur, i) {
                i += 1;
            }
            out.extend([skip, (i - s) as u8]);
            out.extend_from_slice(&cur[s * unit..i * unit]);
        }
        packets += 1;
        x = i;
    }
    (packets, out)
}

fn lc(prev: &[u8], cur: &[u8], w: usize) -> Vec<u8> {
    let h = cur.len() / w;
    let changed: Vec<usize> = (0..h)
        .filter(|&y| prev[y * w..(y + 1) * w] != cur[y * w..(y + 1) * w])
        .collect();
    let (first, last) = match (changed.first(), changed.last()) {
        (Some(&f), Some(&l)) => (f, l),
        _ => (0, 0),
    };
    let lines = if changed.is_empty() {
        0
    } else {
        last - first + 1
    };
    let mut b = (first as u16).to_le_bytes().to_vec();
    b.extend((lines as u16).to_le_bytes());
    for y in first..first + lines {
        let (p, body) = line_packets(&prev[y * w..(y + 1) * w], &cur[y * w..(y + 1) * w], 1);
        b.push(p as u8);
        b.extend(body);
    }
    sub(12, &b)
}

fn ss2(prev: &[u8], cur: &[u8], w: usize) -> Vec<u8> {
    let h = cur.len() / w;
    let ew = w & !1;
    let mut b = vec![0, 0];
    let mut lines = 0u16;
    let mut skipped = 0i16;
    for y in 0..h {
        let (p, c) = (&prev[y * w..(y + 1) * w], &cur[y * w..(y + 1) * w]);
        if p == c {
            skipped += 1;
            continue;
        }
        if skipped > 0 {
            b.extend((-skipped).to_le_bytes());
            skipped = 0;
        }
        if w % 2 == 1 {
            b.extend((0x8000u16 | c[w - 1] as u16).to_le_bytes());
        }
        let (n, body) = line_packets(&p[..ew], &c[..ew], 2);
        b.extend(n.to_le_bytes());
        b.extend(body);
        lines += 1;
    }
    b[0..2].copy_from_slice(&lines.to_le_bytes());
    sub(7, &b)
}

fn frame(chunks: &[Vec<u8>], delay: u16) -> Vec<u8> {
    let size: usize = 16 + chunks.iter().map(Vec::len).sum::<usize>();
    let mut f = (size as u32).to_le_bytes().to_vec();
    f.extend(0xF1FAu16.to_le_bytes());
    f.extend((chunks.len() as u16).to_le_bytes());
    f.extend(delay.to_le_bytes());
    f.extend([0; 6]);
    for c in chunks {
        f.extend_from_slice(c);
    }
    f
}

/// (index image, palette, per-frame delay override (FLC), clear with BLACK first)
pub type FliFrameSpec = (Vec<u8>, [[u8; 3]; 256], u16, bool);

pub struct FliSpec {
    pub flc: bool,
    pub w: usize,
    pub h: usize,
    /// FLI jiffies or FLC milliseconds.
    pub speed: u32,
    /// (index image, palette, per-frame delay override (FLC), clear with BLACK first)
    pub frames: Vec<FliFrameSpec>,
    /// Use COPY instead of BRUN for the first frame.
    pub copy_first: bool,
    /// Add a PSTAMP chunk to the first frame and a prefix chunk before it.
    pub extras: bool,
}

pub fn fli(spec: &FliSpec) -> Vec<u8> {
    let (w, h) = (spec.w, spec.h);
    let mut body = Vec::new();
    if spec.extras {
        // 0xF100 prefix chunk with one dummy sub-chunk.
        let inner = sub(0x0003, &[0; 10]);
        body.extend(((6 + inner.len()) as u32).to_le_bytes());
        body.extend(0xF100u16.to_le_bytes());
        body.extend(inner);
    }
    let first_at = 128 + body.len();
    let mut prev = vec![0u8; w * h];
    let mut prev_pal = [[0u8; 3]; 256];
    let n = spec.frames.len();
    for k in 0..=n {
        let ring = k == n;
        let (img, pal, delay, black) = &spec.frames[if ring { 0 } else { k }];
        let mut chunks = Vec::new();
        if *pal != prev_pal || k == 0 {
            chunks.push(color_chunk(pal, !spec.flc));
        }
        if k == 0 {
            if spec.extras {
                chunks.push(sub(18, &[0; 12]));
            }
            chunks.push(if spec.copy_first {
                sub(16, img)
            } else {
                brun(img, w)
            });
        } else {
            let base = if *black && !ring {
                chunks.push(sub(13, &[]));
                vec![0u8; w * h]
            } else {
                prev.clone()
            };
            chunks.push(if spec.flc {
                ss2(&base, img, w)
            } else {
                lc(&base, img, w)
            });
        }
        body.extend(frame(&chunks, *delay));
        prev = img.clone();
        prev_pal = *pal;
    }
    let mut d = vec![0u8; 128];
    d[0..4].copy_from_slice(&((128 + body.len()) as u32).to_le_bytes());
    d[4..6].copy_from_slice(&(if spec.flc { 0xAF12u16 } else { 0xAF11 }).to_le_bytes());
    d[6..8].copy_from_slice(&(n as u16).to_le_bytes());
    d[8..10].copy_from_slice(&(w as u16).to_le_bytes());
    d[10..12].copy_from_slice(&(h as u16).to_le_bytes());
    d[12..14].copy_from_slice(&8u16.to_le_bytes());
    d[14..16].copy_from_slice(&3u16.to_le_bytes());
    if spec.flc {
        d[16..20].copy_from_slice(&spec.speed.to_le_bytes());
        d[80..84].copy_from_slice(&(first_at as u32).to_le_bytes());
    } else {
        d[16..18].copy_from_slice(&(spec.speed as u16).to_le_bytes());
    }
    d.extend(body);
    d
}

/// RGBA of an index image through a palette.
pub fn rgba(idx: &[u8], pal: &[[u8; 3]; 256]) -> Vec<u8> {
    idx.iter()
        .flat_map(|&i| {
            let [r, g, b] = pal[i as usize];
            [r, g, b, 255]
        })
        .collect()
}
