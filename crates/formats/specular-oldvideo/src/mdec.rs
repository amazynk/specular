//! PlayStation MDEC pictures (the "BS" bitstream of `.str` movies), from the psx-spx
//! description of the PlayStation's macroblock decoder and of the BS compression versions
//! 1, 2 and 3.
//!
//! A frame starts with an 8-byte header (MDEC size / 4, the id 3800h, the quantiser scale,
//! the version), then the macroblocks top to bottom in columns of 16 pixels, left to right.
//! Each macroblock is six 8×8 blocks: Cr, Cb, then the four luma blocks. A block is a DC
//! value (10 bits in v1/v2; a difference to the previous block of its kind, Huffman coded,
//! in v3) and AC run/level codes (the MPEG-1 shaped table every version shares) up to an
//! end-of-block code. The 16-bit words are little-endian, read from their top bit.
//!
//! Coefficients are scaled by the PlayStation's default quantiser table (MPEG-1's intra
//! table with 2 for DC), times the frame's scale, over 8; the 8×8 inverse DCT is the exact
//! one. Colours are full-range YCbCr, chroma at half size each way.

use crate::{Error, Result, bad};

/// The PlayStation's default quantiser table (ISO/IEC 11172-2's default intra matrix, DC
/// entry 2), in raster order.
const QUANT: [u8; 64] = [
    2, 16, 19, 22, 26, 27, 29, 34, 16, 16, 22, 24, 27, 29, 34, 37, 19, 22, 26, 27, 29, 34, 34, 38,
    22, 22, 26, 27, 29, 34, 37, 40, 22, 26, 27, 29, 32, 35, 40, 48, 26, 27, 29, 32, 35, 40, 48, 58,
    26, 27, 29, 34, 38, 46, 56, 69, 27, 29, 35, 38, 46, 56, 69, 83,
];

/// Scan position → raster position.
const ZIGZAG: [u8; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20,
    13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59,
    52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
];

/// The AC codes past their leading zeros and the 1 that ends them: for each count of leading
/// zeros, the number of index bits after the 1 and the 16-bit MDEC values (run in the top 6
/// bits, level in the low 10) they index; a sign bit follows every code but end of block.
/// One leading zero group (`1…`) is special: `10` ends the block, `11s` is 0001h.
const AC: [(u8, &[u16]); 12] = [
    (0, &[]),
    // 011s, 010xs
    (0, &[]),
    // 0011xs, 00101s, 00100xxxs
    (0, &[]),
    (2, &[0x1C01, 0x1801, 0x0402, 0x1401]),
    (2, &[0x0802, 0x2401, 0x0004, 0x2001]),
    // escape
    (0, &[]),
    (
        3,
        &[
            0x4001, 0x1402, 0x0007, 0x0803, 0x0404, 0x3C01, 0x3801, 0x1002,
        ],
    ),
    (
        4,
        &[
            0x000B, 0x2002, 0x1003, 0x000A, 0x0804, 0x1C02, 0x5401, 0x5001, 0x0009, 0x4C01, 0x4801,
            0x0405, 0x0C03, 0x0008, 0x1802, 0x4401,
        ],
    ),
    (
        4,
        &[
            0x2802, 0x2402, 0x1403, 0x0C04, 0x0805, 0x0407, 0x0406, 0x000F, 0x000E, 0x000D, 0x000C,
            0x6801, 0x6401, 0x6001, 0x5C01, 0x5801,
        ],
    ),
    (
        4,
        &[
            0x001F, 0x001E, 0x001D, 0x001C, 0x001B, 0x001A, 0x0019, 0x0018, 0x0017, 0x0016, 0x0015,
            0x0014, 0x0013, 0x0012, 0x0011, 0x0010,
        ],
    ),
    (
        4,
        &[
            0x0028, 0x0027, 0x0026, 0x0025, 0x0024, 0x0023, 0x0022, 0x0021, 0x0020, 0x040E, 0x040D,
            0x040C, 0x040B, 0x040A, 0x0409, 0x0408,
        ],
    ),
    (
        4,
        &[
            0x0412, 0x0411, 0x0410, 0x040F, 0x1803, 0x4002, 0x3C02, 0x3802, 0x3402, 0x3002, 0x2C02,
            0x7C01, 0x7801, 0x7401, 0x7001, 0x6C01,
        ],
    ),
];

const AC_00100: [u16; 8] = [
    0x3401, 0x0006, 0x3001, 0x2C01, 0x0C02, 0x0403, 0x0005, 0x2801,
];

/// Bits of 16-bit little-endian words, each from its top bit.
struct Bits<'a> {
    data: &'a [u8],
    at: usize,
    /// Bits not yet used, top-aligned.
    cache: u64,
    have: u32,
}

impl<'a> Bits<'a> {
    fn new(data: &'a [u8]) -> Bits<'a> {
        Bits {
            data,
            at: 0,
            cache: 0,
            have: 0,
        }
    }

    fn fill(&mut self) {
        while self.have <= 48 {
            let w = match self.data.get(self.at..self.at + 2) {
                Some(b) => u16::from_le_bytes([b[0], b[1]]),
                // Past the end reads zeros; running out is caught by `over`.
                None => 0,
            };
            self.at += 2;
            self.cache |= u64::from(w) << (48 - self.have);
            self.have += 16;
        }
    }

    fn peek(&mut self, n: u32) -> u32 {
        if self.have < n {
            self.fill();
        }
        (self.cache >> (64 - n)) as u32
    }

    fn skip(&mut self, n: u32) {
        if self.have < n {
            self.fill();
        }
        self.cache <<= n;
        self.have -= n;
    }

    fn get(&mut self, n: u32) -> u32 {
        let v = self.peek(n);
        self.skip(n);
        v
    }

    /// Whether more was read than there is.
    fn over(&self) -> bool {
        self.at.saturating_sub(self.have as usize / 8) > self.data.len() + 2
    }
}

/// One AC code: `None` at end of block, else (run, signed level).
fn ac(b: &mut Bits) -> Result<Option<(u32, i32)>> {
    let w = b.peek(22);
    let lz = (w << 10).leading_zeros().min(12);
    let signed = |b: &mut Bits, v: u16, len: u32| {
        b.skip(len);
        let s = b.get(1);
        let level = i32::from(v & 0x3FF);
        Some((u32::from(v >> 10), if s == 1 { -level } else { level }))
    };
    Ok(match lz {
        0 => {
            if (w >> 20) & 1 == 0 {
                b.skip(2);
                None
            } else {
                signed(b, 0x0001, 2)
            }
        }
        1 => {
            if (w >> 19) & 1 == 1 {
                signed(b, 0x0401, 3)
            } else {
                signed(b, [0x0002, 0x0801][((w >> 18) & 1) as usize], 4)
            }
        }
        2 => match (w >> 17) & 3 {
            2 | 3 => signed(b, [0x1001, 0x0C01][((w >> 17) & 1) as usize], 5),
            1 => signed(b, 0x0003, 5),
            _ => signed(b, AC_00100[((w >> 14) & 7) as usize], 8),
        },
        5 => {
            b.skip(6);
            let v = b.get(16);
            let level = (v & 0x3FF) as i32;
            Some((v >> 10, if level >= 512 { level - 1024 } else { level }))
        }
        12 => return Err(bad("unused AC code")),
        n => {
            let (bits, values) = AC[n as usize];
            let len = n + 1 + u32::from(bits);
            let i = (w >> (22 - len)) & ((1 << bits) - 1);
            signed(b, values[i as usize], len)
        }
    })
}

/// A v3 DC difference (×4), or `None` at the end of the frame.
fn dc3(b: &mut Bits, luma: bool) -> Result<Option<i32>> {
    if b.peek(10) == 0x3FF {
        return Ok(None);
    }
    let w = b.peek(16);
    let ones = (!w << 16).leading_zeros();
    // (prefix length, size)
    let (len, size) = if luma {
        match w >> 13 {
            0b100 => (3, 0),
            0b000 | 0b001 => (2, 1),
            0b010 | 0b011 => (2, 2),
            0b101 => (3, 3),
            _ => match ones {
                2 => (3, 4),
                3 => (4, 5),
                4 => (5, 6),
                5 => (6, 7),
                6 => (7, 8),
                _ => return Err(bad("unused DC code")),
            },
        }
    } else {
        match w >> 14 {
            0 => (2, 0),
            1 => (2, 1),
            2 => (2, 2),
            _ => match ones {
                2..=7 => (ones + 1, ones + 1),
                _ => return Err(bad("unused DC code")),
            },
        }
    };
    b.skip(len);
    if size == 0 {
        return Ok(Some(0));
    }
    let v = b.get(size) as i32;
    let v = if v >> (size - 1) == 0 {
        v - ((1 << size) - 1)
    } else {
        v
    };
    Ok(Some(v * 4))
}

/// The exact 8×8 inverse DCT, in place (raster order).
fn idct(blk: &mut [f32; 64]) {
    use std::sync::OnceLock;
    static COS: OnceLock<[f32; 64]> = OnceLock::new();
    let c = COS.get_or_init(|| {
        let mut t = [0f32; 64];
        for x in 0..8 {
            for u in 0..8 {
                let cu = if u == 0 {
                    std::f64::consts::FRAC_1_SQRT_2
                } else {
                    1.0
                };
                t[x * 8 + u] = (cu / 2.0
                    * ((2 * x + 1) as f64 * u as f64 * std::f64::consts::PI / 16.0).cos())
                    as f32;
            }
        }
        t
    });
    let mut tmp = [0f32; 64];
    for y in 0..8 {
        let row = &blk[y * 8..y * 8 + 8];
        for x in 0..8 {
            tmp[y * 8 + x] = (0..8).map(|u| c[x * 8 + u] * row[u]).sum();
        }
    }
    for x in 0..8 {
        for y in 0..8 {
            blk[y * 8 + x] = (0..8).map(|v| c[y * 8 + v] * tmp[v * 8 + x]).sum();
        }
    }
}

/// A decoded picture: full-range Y, Cb and Cr planes, the chroma at half size, all
/// macroblock-aligned (`width` and `height` multiples of 16).
#[derive(Clone, Debug, Default)]
pub struct Planes {
    pub width: usize,
    pub height: usize,
    pub y: Vec<u8>,
    pub cb: Vec<u8>,
    pub cr: Vec<u8>,
}

/// The BS version of a frame (1, 2 or 3), or why it can't be read.
pub fn version(data: &[u8]) -> Result<u16> {
    let h = data.get(..8).ok_or_else(|| bad("short frame"))?;
    let id = u16::from_le_bytes([h[2], h[3]]);
    let v = u16::from_le_bytes([h[6], h[7]]);
    if id != 0x3800 {
        return Err(Error::Unsupported(
            "this PlayStation video's own picture coding".into(),
        ));
    }
    match v {
        1..=3 => Ok(v),
        // Star Wars games keep the version in bit 15 or hide it (psx-spx).
        _ => Err(Error::Unsupported(format!(
            "PlayStation video pictures of version {v} (encrypted or a game's own variant)"
        ))),
    }
}

/// Decodes one BS frame of `width`×`height` pixels.
pub fn decode_planes(data: &[u8], width: usize, height: usize) -> Result<Planes> {
    let v = version(data)?;
    let q = i32::from(u16::from_le_bytes([data[4], data[5]]) & 0x3F);
    let (mw, mh) = (width.div_ceil(16), height.div_ceil(16));
    let (w, h) = (mw * 16, mh * 16);
    let mut p = Planes {
        width: w,
        height: h,
        y: vec![0; w * h],
        cb: vec![0; w * h / 4],
        cr: vec![0; w * h / 4],
    };
    let mut b = Bits::new(&data[8..]);
    let mut old = [0i32; 3]; // Cr, Cb, Y
    let mut blk = [0f32; 64];
    'frame: for mx in 0..mw {
        for my in 0..mh {
            for k in 0..6 {
                let kind = k.min(2);
                // DC
                let dc = if v == 3 {
                    match dc3(&mut b, kind == 2)? {
                        None => break 'frame,
                        Some(d) => {
                            old[kind] += d;
                            (old[kind] + 512).rem_euclid(1024) - 512
                        }
                    }
                } else {
                    let d = b.get(10) as i32;
                    if k == 0 && d == 0x1FF {
                        break 'frame;
                    }
                    if d >= 512 { d - 1024 } else { d }
                };
                if b.over() {
                    return Err(bad("frame ends early"));
                }
                blk.fill(0.0);
                if q == 0 {
                    blk[0] = (dc * 2).clamp(-0x400, 0x3FF) as f32;
                } else {
                    blk[0] = (dc * i32::from(QUANT[0])).clamp(-0x400, 0x3FF) as f32;
                }
                let mut at = 0usize;
                while let Some((run, level)) = ac(&mut b)? {
                    at += run as usize + 1;
                    if at > 63 {
                        return Err(bad("AC run past the block"));
                    }
                    if b.over() {
                        return Err(bad("frame ends early"));
                    }
                    let (pos, val) = if q == 0 {
                        (at, level * 2)
                    } else {
                        let pos = usize::from(ZIGZAG[at]);
                        (pos, (level * i32::from(QUANT[pos]) * q) / 8)
                    };
                    blk[pos] = val.clamp(-0x400, 0x3FF) as f32;
                }
                idct(&mut blk);
                let px = |f: f32| (f.round() as i32 + 128).clamp(0, 255) as u8;
                if k < 2 {
                    let plane = if k == 0 { &mut p.cr } else { &mut p.cb };
                    let (cw, x0, y0) = (w / 2, mx * 8, my * 8);
                    for r in 0..8 {
                        for c in 0..8 {
                            plane[(y0 + r) * cw + x0 + c] = px(blk[r * 8 + c]);
                        }
                    }
                } else {
                    let (x0, y0) = (mx * 16 + (k - 2) % 2 * 8, my * 16 + (k - 2) / 2 * 8);
                    for r in 0..8 {
                        for c in 0..8 {
                            p.y[(y0 + r) * w + x0 + c] = px(blk[r * 8 + c]);
                        }
                    }
                }
            }
        }
    }
    Ok(p)
}

/// Decodes one BS frame into R, G, B bytes, `width`×`height`.
pub(crate) fn decode(data: &[u8], width: usize, height: usize, out: &mut [u8]) -> Result<()> {
    let p = decode_planes(data, width, height)?;
    let cw = p.width / 2;
    for y in 0..height {
        for x in 0..width {
            let yy = f32::from(p.y[y * p.width + x]);
            let ci = (y / 2) * cw + x / 2;
            let cb = f32::from(p.cb[ci]) - 128.0;
            let cr = f32::from(p.cr[ci]) - 128.0;
            let px = |f: f32| f.round().clamp(0.0, 255.0) as u8;
            let o = 3 * (y * width + x);
            out[o] = px(yy + 1.402 * cr);
            out[o + 1] = px(yy - 0.344_136 * cb - 0.714_136 * cr);
            out[o + 2] = px(yy + 1.772 * cb);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Writes bits into 16-bit little-endian words from their top bit.
    #[derive(Default)]
    pub(crate) struct Writer {
        bits: Vec<bool>,
    }

    impl Writer {
        pub fn put(&mut self, v: u32, n: u32) {
            for i in (0..n).rev() {
                self.bits.push((v >> i) & 1 == 1);
            }
        }
        pub fn bytes(&self) -> Vec<u8> {
            let mut out = Vec::new();
            for w in self.bits.chunks(16) {
                let mut v = 0u16;
                for (i, &b) in w.iter().enumerate() {
                    v |= u16::from(b) << (15 - i);
                }
                out.extend_from_slice(&v.to_le_bytes());
            }
            out
        }
    }

    fn header(q: u16, v: u16) -> Vec<u8> {
        let mut h = vec![0, 0, 0x00, 0x38];
        h.extend_from_slice(&q.to_le_bytes());
        h.extend_from_slice(&v.to_le_bytes());
        h
    }

    #[test]
    fn flat_v2_macroblock() {
        // One 16×16 macroblock: Cr 0, Cb 0, luma DC 200 (→ 50 + 128), all blocks empty.
        let mut w = Writer::default();
        for dc in [0u32, 0, 200, 200, 200, 200] {
            w.put(dc, 10);
            w.put(0b10, 2);
        }
        w.put(0x1FF, 10);
        let mut d = header(1, 2);
        d.extend(w.bytes());
        let p = decode_planes(&d, 16, 16).unwrap();
        assert!(p.y.iter().all(|&v| v == 178), "{:?}", &p.y[..4]);
        assert!(p.cb.iter().all(|&v| v == 128));
    }

    #[test]
    fn ac_codes_decode() {
        // 11s → level 1 run 0; 011s → run 1 level 1; escape: run 3 level -5.
        let mut w = Writer::default();
        w.put(0b110, 3);
        w.put(0b0111, 4);
        w.put(0b000001, 6);
        w.put(3 << 10 | (1024 - 5), 16);
        w.put(0b10, 2);
        let d = w.bytes();
        let mut b = Bits::new(&d);
        assert_eq!(ac(&mut b).unwrap(), Some((0, 1)));
        assert_eq!(ac(&mut b).unwrap(), Some((1, -1)));
        assert_eq!(ac(&mut b).unwrap(), Some((3, -5)));
        assert_eq!(ac(&mut b).unwrap(), None);
    }

    #[test]
    fn v3_dc_differences() {
        let mut w = Writer::default();
        // chroma "10sn" size 2: s=1 n=1 → +3; luma "00s" s=0 → -1; luma "100" → 0.
        w.put(0b1011, 4);
        w.put(0b000, 3);
        w.put(0b100, 3);
        w.put(0x3FF, 10);
        let d = w.bytes();
        let mut b = Bits::new(&d);
        assert_eq!(dc3(&mut b, false).unwrap(), Some(12));
        assert_eq!(dc3(&mut b, true).unwrap(), Some(-4));
        assert_eq!(dc3(&mut b, true).unwrap(), Some(0));
        assert_eq!(dc3(&mut b, true).unwrap(), None);
    }

    #[test]
    fn other_versions_are_refused() {
        assert!(matches!(
            version(&header(1, 0x8123)),
            Err(Error::Unsupported(_))
        ));
        let mut d = header(1, 2);
        d[3] = 0;
        assert!(matches!(version(&d), Err(Error::Unsupported(_))));
    }
}
