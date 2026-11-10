//! Cache entry pixels: a small baseline JPEG encoder (4:2:0, Annex K tables) for opaque
//! thumbnails, decoded again with `specular-jpeg`. About 10–20 KB for a 256 px thumbnail.
//! There is no other JPEG encoder in the tree (DESIGN.md §2: own codecs), and thumbnails need
//! nothing more than this: no restart markers, no optimised Huffman tables, no progression.

/// Encodes straight RGBA8 (alpha ignored) as a baseline JFIF-less JPEG at `quality` 1..=100.
pub fn encode_jpeg(rgba: &[u8], w: usize, h: usize, quality: u8) -> Vec<u8> {
    assert!(w > 0 && h > 0 && w <= 65535 && h <= 65535 && rgba.len() >= w * h * 4);
    let ql = scaled_table(&LUM_Q, quality);
    let qc = scaled_table(&CHROMA_Q, quality);
    let mut out = Vec::with_capacity(w * h / 2 + 700);
    out.extend_from_slice(&[0xFF, 0xD8]);
    // DQT: two 8-bit tables in zigzag order.
    out.extend_from_slice(&[0xFF, 0xDB, 0, 132, 0]);
    out.extend(ZIGZAG.iter().map(|&i| ql[i] as u8));
    out.push(1);
    out.extend(ZIGZAG.iter().map(|&i| qc[i] as u8));
    // SOF0: 8-bit, 3 components, Y 2x2, Cb/Cr 1x1.
    out.extend_from_slice(&[0xFF, 0xC0, 0, 17, 8]);
    out.extend_from_slice(&(h as u16).to_be_bytes());
    out.extend_from_slice(&(w as u16).to_be_bytes());
    out.extend_from_slice(&[3, 1, 0x22, 0, 2, 0x11, 1, 3, 0x11, 1]);
    // DHT: the four standard tables.
    let tables: [(u8, &[u8; 16], &[u8]); 4] = [
        (0x00, &DC_LUM_BITS, &DC_VALS),
        (0x10, &AC_LUM_BITS, &AC_LUM_VALS),
        (0x01, &DC_CHROMA_BITS, &DC_VALS),
        (0x11, &AC_CHROMA_BITS, &AC_CHROMA_VALS),
    ];
    for (class, bits, vals) in tables {
        let len = 2 + 1 + 16 + vals.len();
        out.extend_from_slice(&[0xFF, 0xC4]);
        out.extend_from_slice(&(len as u16).to_be_bytes());
        out.push(class);
        out.extend_from_slice(bits);
        out.extend_from_slice(vals);
    }
    // SOS
    out.extend_from_slice(&[0xFF, 0xDA, 0, 12, 3, 1, 0x00, 2, 0x11, 3, 0x11, 0, 63, 0]);

    let dc_l = Huff::new(&DC_LUM_BITS, &DC_VALS);
    let ac_l = Huff::new(&AC_LUM_BITS, &AC_LUM_VALS);
    let dc_c = Huff::new(&DC_CHROMA_BITS, &DC_VALS);
    let ac_c = Huff::new(&AC_CHROMA_BITS, &AC_CHROMA_VALS);
    let ql: [f32; 64] = std::array::from_fn(|i| ql[i] as f32);
    let qc: [f32; 64] = std::array::from_fn(|i| qc[i] as f32);
    let cos = cos_table();

    let mut bw = BitWriter { out, acc: 0, n: 0 };
    let mut pred = [0i32; 3];
    let px = |x: usize, y: usize| -> [f32; 3] {
        let i = (y.min(h - 1) * w + x.min(w - 1)) * 4;
        let (r, g, b) = (rgba[i] as f32, rgba[i + 1] as f32, rgba[i + 2] as f32);
        [
            0.299 * r + 0.587 * g + 0.114 * b - 128.0,
            -0.168_736 * r - 0.331_264 * g + 0.5 * b,
            0.5 * r - 0.418_688 * g - 0.081_312 * b,
        ]
    };
    let mut ycc = [[[0f32; 3]; 16]; 16];
    for my in (0..h).step_by(16) {
        for mx in (0..w).step_by(16) {
            for (y, row) in ycc.iter_mut().enumerate() {
                for (x, v) in row.iter_mut().enumerate() {
                    *v = px(mx + x, my + y);
                }
            }
            for b in 0..4 {
                let (ox, oy) = ((b & 1) * 8, (b >> 1) * 8);
                let blk: [f32; 64] = std::array::from_fn(|i| ycc[oy + i / 8][ox + i % 8][0]);
                encode_block(&mut bw, &blk, &ql, &cos, &mut pred[0], &dc_l, &ac_l);
            }
            for c in 1..3 {
                let blk: [f32; 64] = std::array::from_fn(|i| {
                    let (x, y) = ((i % 8) * 2, (i / 8) * 2);
                    (ycc[y][x][c] + ycc[y][x + 1][c] + ycc[y + 1][x][c] + ycc[y + 1][x + 1][c])
                        * 0.25
                });
                encode_block(&mut bw, &blk, &qc, &cos, &mut pred[c], &dc_c, &ac_c);
            }
        }
    }
    bw.flush();
    let mut out = bw.out;
    out.extend_from_slice(&[0xFF, 0xD9]);
    out
}

/// Decodes a cache JPEG to RGBA8 (one thread: thumbnails are small and many run at once).
pub fn decode_jpeg(data: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let opts = specular_jpeg::DecodeOptions {
        scale: specular_jpeg::Scale::Full,
        region: None,
        cancel: None,
        parallelism: specular_jpeg::Parallelism::Single,
    };
    let p = specular_jpeg::decode(data, &opts).ok()?;
    Some((p.width, p.height, specular_jpeg::planes_to_rgba(&p)))
}

fn encode_block(
    bw: &mut BitWriter,
    blk: &[f32; 64],
    q: &[f32; 64],
    cos: &[[f32; 8]; 8],
    pred: &mut i32,
    dc: &Huff,
    ac: &Huff,
) {
    // Separable float DCT-II: rows, then columns.
    let mut tmp = [0f32; 64];
    for y in 0..8 {
        for u in 0..8 {
            let mut s = 0.0;
            for x in 0..8 {
                s += blk[y * 8 + x] * cos[u][x];
            }
            tmp[y * 8 + u] = s;
        }
    }
    let mut coef = [0i32; 64];
    for v in 0..8 {
        for u in 0..8 {
            let mut s = 0.0;
            for y in 0..8 {
                s += tmp[y * 8 + u] * cos[v][y];
            }
            coef[v * 8 + u] = (s / q[v * 8 + u]).round() as i32;
        }
    }
    let diff = coef[0] - *pred;
    *pred = coef[0];
    let (n, bits) = magnitude(diff);
    dc.put(bw, n as u8);
    bw.put(bits, n);
    let mut run = 0;
    for &zi in &ZIGZAG[1..] {
        let c = coef[zi];
        if c == 0 {
            run += 1;
            continue;
        }
        while run > 15 {
            ac.put(bw, 0xF0);
            run -= 16;
        }
        let (n, bits) = magnitude(c);
        ac.put(bw, (run << 4 | n) as u8);
        bw.put(bits, n);
        run = 0;
    }
    if run > 0 {
        ac.put(bw, 0x00);
    }
}

/// Bit count and the JPEG representation of a coefficient (negative values one's-complemented).
fn magnitude(v: i32) -> (u32, u32) {
    let a = v.unsigned_abs();
    let n = 32 - a.leading_zeros();
    let bits = if v < 0 {
        (v - 1) as u32 & ((1 << n) - 1)
    } else {
        v as u32
    };
    (n, bits)
}

/// `cos[u][x] = C(u)/2 · cos((2x+1)uπ/16)`, so a row pass and a column pass give the
/// orthonormal-scaled JPEG DCT.
fn cos_table() -> [[f32; 8]; 8] {
    std::array::from_fn(|u| {
        std::array::from_fn(|x| {
            let c = if u == 0 {
                std::f32::consts::FRAC_1_SQRT_2
            } else {
                1.0
            };
            0.5 * c * (((2 * x + 1) * u) as f32 * std::f32::consts::PI / 16.0).cos()
        })
    })
}

fn scaled_table(base: &[u16; 64], quality: u8) -> [u16; 64] {
    let q = quality.clamp(1, 100) as u32;
    let s = if q < 50 { 5000 / q } else { 200 - 2 * q };
    std::array::from_fn(|i| ((base[i] as u32 * s + 50) / 100).clamp(1, 255) as u16)
}

struct Huff {
    code: [u16; 256],
    len: [u8; 256],
}

impl Huff {
    fn new(bits: &[u8; 16], vals: &[u8]) -> Huff {
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
    fn put(&self, bw: &mut BitWriter, sym: u8) {
        bw.put(
            self.code[sym as usize] as u32,
            self.len[sym as usize] as u32,
        );
    }
}

struct BitWriter {
    out: Vec<u8>,
    acc: u32,
    n: u32,
}

impl BitWriter {
    fn put(&mut self, bits: u32, n: u32) {
        if n == 0 {
            return;
        }
        self.acc = self.acc << n | (bits & ((1 << n) - 1));
        self.n += n;
        while self.n >= 8 {
            let b = (self.acc >> (self.n - 8)) as u8;
            self.out.push(b);
            if b == 0xFF {
                self.out.push(0);
            }
            self.n -= 8;
        }
        self.acc &= (1u32 << self.n).wrapping_sub(1);
    }
    fn flush(&mut self) {
        let pad = (8 - self.n % 8) % 8;
        self.put((1 << pad) - 1, pad);
    }
}

const ZIGZAG: [usize; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20,
    13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59,
    52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
];

#[rustfmt::skip]
const LUM_Q: [u16; 64] = [
    16, 11, 10, 16, 24, 40, 51, 61,
    12, 12, 14, 19, 26, 58, 60, 55,
    14, 13, 16, 24, 40, 57, 69, 56,
    14, 17, 22, 29, 51, 87, 80, 62,
    18, 22, 37, 56, 68, 109, 103, 77,
    24, 35, 55, 64, 81, 104, 113, 92,
    49, 64, 78, 87, 103, 121, 120, 101,
    72, 92, 95, 98, 112, 100, 103, 99,
];

#[rustfmt::skip]
const CHROMA_Q: [u16; 64] = [
    17, 18, 24, 47, 99, 99, 99, 99,
    18, 21, 26, 66, 99, 99, 99, 99,
    24, 26, 56, 99, 99, 99, 99, 99,
    47, 66, 99, 99, 99, 99, 99, 99,
    99, 99, 99, 99, 99, 99, 99, 99,
    99, 99, 99, 99, 99, 99, 99, 99,
    99, 99, 99, 99, 99, 99, 99, 99,
    99, 99, 99, 99, 99, 99, 99, 99,
];

const DC_LUM_BITS: [u8; 16] = [0, 1, 5, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0];
const DC_CHROMA_BITS: [u8; 16] = [0, 3, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0];
const DC_VALS: [u8; 12] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
const AC_LUM_BITS: [u8; 16] = [0, 2, 1, 3, 3, 2, 4, 3, 5, 5, 4, 4, 0, 0, 1, 0x7d];
const AC_CHROMA_BITS: [u8; 16] = [0, 2, 1, 2, 4, 4, 3, 4, 7, 5, 4, 4, 0, 1, 2, 0x77];

#[rustfmt::skip]
const AC_LUM_VALS: [u8; 162] = [
    0x01, 0x02, 0x03, 0x00, 0x04, 0x11, 0x05, 0x12, 0x21, 0x31, 0x41, 0x06, 0x13, 0x51, 0x61, 0x07,
    0x22, 0x71, 0x14, 0x32, 0x81, 0x91, 0xa1, 0x08, 0x23, 0x42, 0xb1, 0xc1, 0x15, 0x52, 0xd1, 0xf0,
    0x24, 0x33, 0x62, 0x72, 0x82, 0x09, 0x0a, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x25, 0x26, 0x27, 0x28,
    0x29, 0x2a, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49,
    0x4a, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5a, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69,
    0x6a, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7a, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89,
    0x8a, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7,
    0xa8, 0xa9, 0xaa, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7, 0xb8, 0xb9, 0xba, 0xc2, 0xc3, 0xc4, 0xc5,
    0xc6, 0xc7, 0xc8, 0xc9, 0xca, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda, 0xe1, 0xe2,
    0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xe9, 0xea, 0xf1, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8,
    0xf9, 0xfa,
];

#[rustfmt::skip]
const AC_CHROMA_VALS: [u8; 162] = [
    0x00, 0x01, 0x02, 0x03, 0x11, 0x04, 0x05, 0x21, 0x31, 0x06, 0x12, 0x41, 0x51, 0x07, 0x61, 0x71,
    0x13, 0x22, 0x32, 0x81, 0x08, 0x14, 0x42, 0x91, 0xa1, 0xb1, 0xc1, 0x09, 0x23, 0x33, 0x52, 0xf0,
    0x15, 0x62, 0x72, 0xd1, 0x0a, 0x16, 0x24, 0x34, 0xe1, 0x25, 0xf1, 0x17, 0x18, 0x19, 0x1a, 0x26,
    0x27, 0x28, 0x29, 0x2a, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48,
    0x49, 0x4a, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5a, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68,
    0x69, 0x6a, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7a, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87,
    0x88, 0x89, 0x8a, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a, 0xa2, 0xa3, 0xa4, 0xa5,
    0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7, 0xb8, 0xb9, 0xba, 0xc2, 0xc3,
    0xc4, 0xc5, 0xc6, 0xc7, 0xc8, 0xc9, 0xca, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda,
    0xe2, 0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xe9, 0xea, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8,
    0xf9, 0xfa,
];

#[cfg(test)]
mod tests {
    use super::*;

    fn psnr(a: &[u8], b: &[u8]) -> f64 {
        let mut se = 0.0;
        let mut n = 0.0;
        for (pa, pb) in a.chunks_exact(4).zip(b.chunks_exact(4)) {
            for c in 0..3 {
                let d = pa[c] as f64 - pb[c] as f64;
                se += d * d;
                n += 1.0;
            }
        }
        10.0 * (255.0f64 * 255.0 / (se / n).max(1e-9)).log10()
    }

    fn image(w: usize, h: usize) -> Vec<u8> {
        (0..w * h)
            .flat_map(|i| {
                let (x, y) = (i % w, i / w);
                [
                    (x * 255 / w) as u8,
                    (y * 255 / h) as u8,
                    ((x + y) * 3 % 256) as u8 / 2 + 64,
                    255,
                ]
            })
            .collect()
    }

    #[test]
    fn round_trip_through_specular_jpeg() {
        for (w, h) in [(256, 192), (37, 101), (1, 1), (17, 16), (512, 9)] {
            let src = image(w, h);
            let jpg = encode_jpeg(&src, w, h, 88);
            let (dw, dh, out) = decode_jpeg(&jpg).expect("decodes");
            assert_eq!((dw as usize, dh as usize), (w, h));
            let p = psnr(&src, &out);
            assert!(p > 30.0, "{w}x{h}: psnr {p:.1}");
        }
        // Small: a 256x192 smooth image is a few KB.
        assert!(encode_jpeg(&image(256, 192), 256, 192, 88).len() < 20_000);
    }

    #[test]
    fn saturated_colours_and_stuffing() {
        // Noise produces 0xFF bytes in the entropy data; they must be stuffed.
        let (w, h) = (64, 64);
        let mut s = 1u32;
        let src: Vec<u8> = (0..w * h * 4)
            .map(|_| {
                s = s.wrapping_mul(1_103_515_245).wrapping_add(12345);
                (s >> 16) as u8
            })
            .collect();
        let jpg = encode_jpeg(&src, w, h, 95);
        let (_, _, out) = decode_jpeg(&jpg).unwrap();
        assert_eq!(out.len(), w * h * 4);
        let red: Vec<u8> = (0..w * h).flat_map(|_| [255, 0, 0, 255]).collect();
        let (_, _, out) = decode_jpeg(&encode_jpeg(&red, w, h, 85)).unwrap();
        assert!(
            out[0] > 240 && out[1] < 15 && out[2] < 15,
            "{:?}",
            &out[..4]
        );
    }
}
