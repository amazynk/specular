//! Samsung SRW sensor codecs (NX cameras).
//!
//! - Version 1 (compression 32770 with a row offset table, NX300 and relatives): rows start at
//!   listed offsets; blocks of 16 samples carry a direction bit, four adaptive lengths and the
//!   differences of the even then the odd columns, predicted from the rows above or from the
//!   previous block. Afterwards the two greens of every 2x2 block trade places.
//! - Version 2 (compression 32772, NX mini): one prefix-coded stream of JPEG-style differences
//!   with the previous sample of the same colour as the prediction.
//! - Version 3 (compression 32773, NX1, NX500): rows aligned to 16 bytes, blocks of 16 samples
//!   with a selectable predictor from the rows above, adaptive lengths and a scaled difference.
//!
//! Versions 1 and 3 read their bits MSB-first from little-endian 32-bit words.

use crate::bits::BitReader;

/// MSB-first bits from little-endian 32-bit words, fetched whole on demand (so the stream
/// position is always a word boundary, as the row alignment of version 3 needs).
struct Words<'a> {
    src: &'a [u8],
    pos: usize,
    buf: u64,
    n: u32,
    short: bool,
}

impl<'a> Words<'a> {
    fn new(src: &'a [u8], pos: usize) -> Words<'a> {
        Words {
            src,
            pos,
            buf: 0,
            n: 0,
            short: false,
        }
    }

    /// Reads `n` (0..=32) bits.
    #[inline(always)]
    fn bits(&mut self, n: u32) -> u32 {
        if n == 0 {
            return 0;
        }
        if self.n < n {
            let w = match self.src.get(self.pos..self.pos.saturating_add(4)) {
                Some(b) => u32::from_le_bytes([b[0], b[1], b[2], b[3]]),
                None => {
                    self.short = true;
                    0
                }
            };
            self.pos = self.pos.saturating_add(4);
            self.buf = (self.buf << 32) | w as u64;
            self.n += 32;
        }
        self.n -= n;
        let v = (self.buf >> self.n) as u32 & (u32::MAX >> (32 - n));
        self.buf &= (1u64 << self.n) - 1;
        v
    }

    /// Restarts at byte `pos`, dropping buffered bits.
    fn seek(&mut self, pos: usize) {
        self.pos = pos;
        self.buf = 0;
        self.n = 0;
    }
}

/// `v` of `n` bits as a two's complement number.
#[inline(always)]
fn signed(v: u32, n: u32) -> i32 {
    if n == 0 || n >= 32 {
        v as i32
    } else {
        ((v << (32 - n)) as i32) >> (32 - n)
    }
}

/// Version 1: `src` from the data start, `rows` the LE offset of each row from there. Returns
/// whether the data ran short.
pub(crate) fn decode_v1(src: &[u8], rows: &[u8], w: usize, h: usize, out: &mut [u16]) -> bool {
    let out = &mut out[..w * h];
    let mut short = false;
    let mut br = Words::new(src, 0);
    for row in 0..h {
        let Some(o) = rows.get(row * 4..row * 4 + 4) else {
            return true;
        };
        br.seek(u32::from_le_bytes([o[0], o[1], o[2], o[3]]) as usize);
        let mut len = if row < 2 { [7u32; 4] } else { [4u32; 4] };
        for col in (0..w).step_by(16) {
            let dir = br.bits(1) != 0;
            let op = [br.bits(2), br.bits(2), br.bits(2), br.bits(2)];
            for (l, op) in len.iter_mut().zip(op) {
                match op {
                    3 => *l = br.bits(4),
                    2 => *l = l.saturating_sub(1),
                    1 => *l = (*l + 1).min(32),
                    _ => {}
                }
            }
            // Even columns, then odd ones.
            for c in (0..16).step_by(2).chain((1..16).step_by(2)) {
                let n = len[((c & 1) << 1) | (c >> 3)];
                let diff = signed(br.bits(n), n);
                let pred = if dir {
                    // Even columns from the row above, odd ones from two rows up.
                    let up = if c & 1 == 0 { 1 } else { 2 };
                    if row >= up && col + c < w {
                        out[(row - up) * w + col + c] as i32
                    } else {
                        0
                    }
                } else if col > 0 {
                    out[row * w + col - 2 + (c & 1)] as i32
                } else {
                    128
                };
                if col + c < w {
                    out[row * w + col + c] = (pred + diff) as u16;
                }
            }
        }
        short |= br.short;
        br.short = false;
    }
    // The coded order has the greens of each 2x2 block crossed.
    for row in (0..h.saturating_sub(1)).step_by(2) {
        let (a, b) = out[row * w..(row + 2) * w].split_at_mut(w);
        for col in (0..w.saturating_sub(1)).step_by(2) {
            std::mem::swap(&mut a[col + 1], &mut b[col]);
        }
    }
    short
}

/// Version 2 prefix codes: code length in the high byte, difference length in the low one,
/// codes assigned in this order.
const V2_CODES: [u16; 14] = [
    0x304, 0x307, 0x206, 0x205, 0x403, 0x600, 0x709, 0x80a, 0x90b, 0xa0c, 0xa0d, 0x501, 0x408,
    0x402,
];

/// Version 2. Returns whether the data ran short.
pub(crate) fn decode_v2(src: &[u8], w: usize, h: usize, out: &mut [u16]) -> bool {
    let mut lut = [0u16; 1024];
    let mut n = 0;
    for &c in &V2_CODES {
        let k = 1024 >> (c >> 8);
        lut[n..n + k].fill(c);
        n += k;
    }
    let out = &mut out[..w * h];
    let mut br = BitReader::new(src, false);
    let mut vpred = [[0u16; 2]; 2];
    for (row, line) in out.chunks_exact_mut(w).enumerate() {
        let mut hpred = [0u16; 2];
        for (col, o) in line.iter_mut().enumerate() {
            br.refill();
            let e = lut[br.peek(10) as usize];
            br.consume((e >> 8) as u32);
            let len = (e & 0xff) as u32;
            let diff = if len == 0 {
                0
            } else {
                let v = br.bits(len) as i32;
                if v & (1 << (len - 1)) == 0 {
                    v - ((1 << len) - 1)
                } else {
                    v
                }
            };
            if col < 2 {
                let v = &mut vpred[row & 1][col];
                *v = v.wrapping_add(diff as u16);
                hpred[col] = *v;
            } else {
                hpred[col & 1] = hpred[col & 1].wrapping_add(diff as u16);
            }
            *o = hpred[col & 1];
        }
        if br.overrun() {
            return true;
        }
    }
    false
}

/// Version 3. Returns whether the data ran short.
pub(crate) fn decode_v3(src: &[u8], w: usize, h: usize, out: &mut [u16]) -> bool {
    let out = &mut out[..w * h];
    let Some(head) = src.get(..14) else {
        return true;
    };
    let opt = head[9];
    let init = u16::from_le_bytes([head[12], head[13]]) as i32;
    let last = out.len() - 1;
    let at = |out: &[u16], i: isize| out[(i.max(0) as usize).min(last)] as i32;
    let mut br = Words::new(src, 14);
    for row in 0..h {
        br.seek(br.pos.next_multiple_of(16));
        let mut mag: i32 = 0;
        let mut pmode = 7;
        let mut lent = if row < 2 {
            [[7u32; 2]; 3]
        } else {
            [[4u32; 2]; 3]
        };
        let mut len = [0u32; 4];
        let odd = row & 1;
        // Flat index of the prediction rows for the even and odd columns: the row above for
        // the greens, two rows up for red and blue.
        let w_ = w as isize;
        let r = row as isize;
        let mut prow = [0isize; 2];
        prow[odd] = (r - 1) * w_ + 1 - ((odd as isize) << 1);
        prow[odd ^ 1] = (r - 2) * w_;
        for tab in (0..w.saturating_sub(15)).step_by(16) {
            if opt & 4 == 0 && tab & 63 == 0 {
                let i = br.bits(2);
                mag = match i {
                    0 => mag,
                    1 => mag - 2,
                    2 => mag + 2,
                    _ => br.bits(12) as i32,
                };
            }
            if opt & 2 != 0 {
                pmode = 7 - 4 * br.bits(1) as usize;
            } else if br.bits(1) == 0 {
                pmode = br.bits(3) as usize;
            }
            if opt & 1 != 0 || br.bits(1) == 0 {
                for l in len.iter_mut() {
                    *l = br.bits(2);
                }
                for (c, l) in len.iter_mut().enumerate() {
                    let i = ((odd << 1) | (c & 1)) % 3;
                    *l = match *l {
                        0 => lent[i][0],
                        1 => (lent[i][0] + 1).min(32),
                        2 => lent[i][0].saturating_sub(1),
                        _ => br.bits(4),
                    };
                    lent[i] = [lent[i][1], *l];
                }
            }
            for c in 0..16 {
                let col = tab + (((c & 7) << 1) ^ (c >> 3) ^ odd);
                let pred = if pmode == 7 || row < 2 {
                    if tab > 0 {
                        out[row * w + tab - 2 + (col & 1)] as i32
                    } else {
                        init
                    }
                } else {
                    const A: [isize; 7] = [0, 2, 2, 4, 4, 6, 8];
                    const B: [isize; 7] = [0, 2, 4, 4, 6, 6, 8];
                    let base = prow[col & 1] + col as isize - 4;
                    (at(out, base + A[pmode]) + at(out, base + B[pmode]) + 1) >> 1
                };
                let n = len[c >> 2];
                let diff = signed(br.bits(n), n) * (mag * 2 + 1) + mag;
                out[row * w + col] = (pred + diff) as u16;
            }
        }
        if br.short {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Packs `(value, bits)` MSB-first into little-endian 32-bit words.
    fn words(fields: &[(u32, u32)]) -> Vec<u8> {
        let mut bits: Vec<bool> = Vec::new();
        for &(v, n) in fields {
            for i in (0..n).rev() {
                bits.push(v >> i & 1 != 0);
            }
        }
        bits.resize(bits.len().next_multiple_of(32), false);
        bits.chunks(32)
            .flat_map(|c| c.iter().fold(0u32, |a, &b| a << 1 | b as u32).to_le_bytes())
            .collect()
    }

    #[test]
    fn v1_block() {
        // One row of 16: no direction, lengths kept at 7, all differences +1 from 128, except
        // the first even one +5.
        let mut f = vec![(0, 1), (0, 2), (0, 2), (0, 2), (0, 2)];
        f.push((5, 7));
        f.extend(std::iter::repeat_n((1, 7), 15));
        let data = words(&f);
        let rows = 0u32.to_le_bytes();
        let mut out = vec![0u16; 16];
        assert!(!decode_v1(&data, &rows, 16, 1, &mut out));
        assert_eq!(out[0], 133);
        assert!(out[1..].iter().all(|&v| v == 129));
        assert!(decode_v1(&data, &[], 16, 1, &mut out));
        assert!(decode_v1(&[], &rows, 16, 1, &mut out));
    }

    #[test]
    fn v1_green_swap() {
        // Two rows of 16, each at its own listed offset: even columns +1 (row 0) and +2 (row
        // 1) from 128, odd columns +3.
        let row = |d: u32| {
            let mut f = vec![(0, 1), (0, 2), (0, 2), (0, 2), (0, 2)];
            f.extend((0..16).map(|c| (if c < 8 { d } else { 3 }, 7)));
            words(&f)
        };
        let mut stream = row(1);
        let second = stream.len() as u32;
        stream.extend(row(2));
        let mut rows = 0u32.to_le_bytes().to_vec();
        rows.extend(second.to_le_bytes());
        let mut out = vec![0u16; 32];
        assert!(!decode_v1(&stream, &rows, 16, 2, &mut out));
        // Row 0 column 1 (131) and row 1 column 0 (130) trade places.
        assert_eq!(&out[..2], &[129, 130]);
        assert_eq!(&out[16..18], &[131, 131]);
    }

    #[test]
    fn v2_codes() {
        // Codes: 000 -> 4 bits, 001 -> 7 bits, 01 -> 6, 10 -> 5, ...
        // Two samples: "000" + 1010 (+10), "000" + 0011 (3 - 15 = -12).
        let mut bits = String::from("0001010") + "0000011";
        while !bits.len().is_multiple_of(8) {
            bits.push('0');
        }
        bits.push_str(&"0".repeat(64));
        let src: Vec<u8> = bits
            .as_bytes()
            .chunks(8)
            .map(|c| c.iter().fold(0u8, |a, &b| a << 1 | (b - b'0')))
            .collect();
        let mut out = vec![0u16; 2];
        assert!(!decode_v2(&src, 2, 1, &mut out));
        assert_eq!(out, [10, (-12i32) as u16]);
        assert!(decode_v2(&[], 64, 64, &mut vec![0; 4096]));
    }

    #[test]
    fn v3_flat() {
        // Header: opt 7 (no magnitude, pmode bit, lengths every block), init 1000.
        let mut src = vec![0u8; 16];
        src[9] = 7;
        src[12..14].copy_from_slice(&1000u16.to_le_bytes());
        // One row of 16: pmode bit 1 (pmode 3 -> treated as row < 2 anyway), lengths 0 (kept
        // at 7), then sixteen 7-bit differences of +1.
        let mut f = vec![(1, 1), (0, 2), (0, 2), (0, 2), (0, 2)];
        f.extend(std::iter::repeat_n((1, 7), 16));
        src.extend(words(&f));
        let mut out = vec![0u16; 16];
        assert!(!decode_v3(&src, 16, 1, &mut out));
        assert!(out.iter().all(|&v| v == 1001), "{out:?}");
        assert!(decode_v3(&src[..10], 16, 1, &mut out));
        assert!(decode_v3(&src[..20], 16, 4, &mut [0; 64]));
    }
}
