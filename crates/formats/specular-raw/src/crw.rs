//! Canon CRW (CIFF) sensor codec: 10-bit values in blocks of 64 samples, each block coded like
//! a JPEG AC block (one prefix-coded run/length symbol per non-zero difference, `FF 00`
//! stuffing) with three fixed table pairs (first coefficient / the rest) chosen by the file's
//! DecoderTable. The first difference of a block chains from the previous block; the rest add
//! to the previous sample of the same column parity, which restarts at 512 on every row. Some
//! bodies store two more low-order bits per sample in a separate area (four samples per byte),
//! making 12-bit values.

use crate::bits::BitReader;

/// Code-length counts (lengths 1..=16) followed by the symbols, per table set: the first
/// coefficient's table, then the table for the others. Symbols are `run << 4 | length`;
/// `0x00` after the first coefficient ends the block, `0xff` skips one position.
const FIRST: [[u8; 29]; 3] = [
    [
        0, 1, 4, 2, 3, 1, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, //
        0x04, 0x03, 0x05, 0x06, 0x02, 0x07, 0x01, 0x08, 0x09, 0x00, 0x0a, 0x0b, 0xff,
    ],
    [
        0, 2, 2, 3, 1, 1, 1, 1, 2, 0, 0, 0, 0, 0, 0, 0, //
        0x03, 0x02, 0x04, 0x01, 0x05, 0x00, 0x06, 0x07, 0x09, 0x08, 0x0a, 0x0b, 0xff,
    ],
    [
        0, 0, 6, 3, 1, 1, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, //
        0x06, 0x05, 0x07, 0x04, 0x08, 0x03, 0x09, 0x02, 0x00, 0x0a, 0x01, 0x0b, 0xff,
    ],
];

const REST: [[u8; 180]; 3] = [
    [
        0, 2, 2, 2, 1, 4, 2, 1, 2, 5, 1, 1, 0, 0, 0, 139, //
        0x03, 0x04, 0x02, 0x05, 0x01, 0x06, 0x07, 0x08, 0x12, 0x13, 0x11, 0x14, 0x09, 0x15, 0x22,
        0x00, 0x21, 0x16, 0x0a, 0xf0, 0x23, 0x17, 0x24, 0x31, 0x32, 0x18, 0x19, 0x33, 0x25, 0x41,
        0x34, 0x42, 0x35, 0x51, 0x36, 0x37, 0x38, 0x29, 0x79, 0x26, 0x1a, 0x39, 0x56, 0x57, 0x28,
        0x27, 0x52, 0x55, 0x58, 0x43, 0x76, 0x59, 0x77, 0x54, 0x61, 0xf9, 0x71, 0x78, 0x75, 0x96,
        0x97, 0x49, 0xb7, 0x53, 0xd7, 0x74, 0xb6, 0x98, 0x47, 0x48, 0x95, 0x69, 0x99, 0x91, 0xfa,
        0xb8, 0x68, 0xb5, 0xb9, 0xd6, 0xf7, 0xd8, 0x67, 0x46, 0x45, 0x94, 0x89, 0xf8, 0x81, 0xd5,
        0xf6, 0xb4, 0x88, 0xb1, 0x2a, 0x44, 0x72, 0xd9, 0x87, 0x66, 0xd4, 0xf5, 0x3a, 0xa7, 0x73,
        0xa9, 0xa8, 0x86, 0x62, 0xc7, 0x65, 0xc8, 0xc9, 0xa1, 0xf4, 0xd1, 0xe9, 0x5a, 0x92, 0x85,
        0xa6, 0xe7, 0x93, 0xe8, 0xc1, 0xc6, 0x7a, 0x64, 0xe1, 0x4a, 0x6a, 0xe6, 0xb3, 0xf1, 0xd3,
        0xa5, 0x8a, 0xb2, 0x9a, 0xba, 0x84, 0xa4, 0x63, 0xe5, 0xc5, 0xf3, 0xd2, 0xc4, 0x82, 0xaa,
        0xda, 0xe4, 0xf2, 0xca, 0x83, 0xa3, 0xa2, 0xc3, 0xea, 0xc2, 0xe2, 0xe3, 0xff, 0xff,
    ],
    [
        0, 2, 2, 1, 4, 1, 4, 1, 3, 3, 1, 0, 0, 0, 0, 140, //
        0x02, 0x03, 0x01, 0x04, 0x05, 0x12, 0x11, 0x06, 0x13, 0x07, 0x08, 0x14, 0x22, 0x09, 0x21,
        0x00, 0x23, 0x15, 0x31, 0x32, 0x0a, 0x16, 0xf0, 0x24, 0x33, 0x41, 0x42, 0x19, 0x17, 0x25,
        0x18, 0x51, 0x34, 0x43, 0x52, 0x29, 0x35, 0x61, 0x39, 0x71, 0x62, 0x36, 0x53, 0x26, 0x38,
        0x1a, 0x37, 0x81, 0x27, 0x91, 0x79, 0x55, 0x45, 0x28, 0x72, 0x59, 0xa1, 0xb1, 0x44, 0x69,
        0x54, 0x58, 0xd1, 0xfa, 0x57, 0xe1, 0xf1, 0xb9, 0x49, 0x47, 0x63, 0x6a, 0xf9, 0x56, 0x46,
        0xa8, 0x2a, 0x4a, 0x78, 0x99, 0x3a, 0x75, 0x74, 0x86, 0x65, 0xc1, 0x76, 0xb6, 0x96, 0xd6,
        0x89, 0x85, 0xc9, 0xf5, 0x95, 0xb4, 0xc7, 0xf7, 0x8a, 0x97, 0xb8, 0x73, 0xb7, 0xd8, 0xd9,
        0x87, 0xa7, 0x7a, 0x48, 0x82, 0x84, 0xea, 0xf4, 0xa6, 0xc5, 0x5a, 0x94, 0xa4, 0xc6, 0x92,
        0xc3, 0x68, 0xb5, 0xc8, 0xe4, 0xe5, 0xe6, 0xe9, 0xa2, 0xa3, 0xe3, 0xc2, 0x66, 0x67, 0x93,
        0xaa, 0xd4, 0xd5, 0xe7, 0xf8, 0x88, 0x9a, 0xd7, 0x77, 0xc4, 0x64, 0xe2, 0x98, 0xa5, 0xca,
        0xda, 0xe8, 0xf3, 0xf6, 0xa9, 0xb2, 0xb3, 0xf2, 0xd2, 0x83, 0xba, 0xd3, 0xff, 0xff,
    ],
    [
        0, 0, 6, 2, 1, 3, 3, 2, 5, 1, 2, 2, 8, 10, 0, 117, //
        0x04, 0x05, 0x03, 0x06, 0x02, 0x07, 0x01, 0x08, 0x09, 0x12, 0x13, 0x14, 0x11, 0x15, 0x0a,
        0x16, 0x17, 0xf0, 0x00, 0x22, 0x21, 0x18, 0x23, 0x19, 0x24, 0x32, 0x31, 0x25, 0x33, 0x38,
        0x37, 0x34, 0x35, 0x36, 0x39, 0x79, 0x57, 0x58, 0x59, 0x28, 0x56, 0x78, 0x27, 0x41, 0x29,
        0x77, 0x26, 0x42, 0x76, 0x99, 0x1a, 0x55, 0x98, 0x97, 0xf9, 0x48, 0x54, 0x96, 0x89, 0x47,
        0xb7, 0x49, 0xfa, 0x75, 0x68, 0xb6, 0x67, 0x69, 0xb9, 0xb8, 0xd8, 0x52, 0xd7, 0x88, 0xb5,
        0x74, 0x51, 0x46, 0xd9, 0xf8, 0x3a, 0xd6, 0x87, 0x45, 0x7a, 0x95, 0xd5, 0xf6, 0x86, 0xb4,
        0xa9, 0x94, 0x53, 0x2a, 0xa8, 0x43, 0xf5, 0xf7, 0xd4, 0x66, 0xa7, 0x5a, 0x44, 0x8a, 0xc9,
        0xe8, 0xc8, 0xe7, 0x9a, 0x6a, 0x73, 0x4a, 0x61, 0xc7, 0xf4, 0xc6, 0x65, 0xe9, 0x72, 0xe6,
        0x71, 0x91, 0x93, 0xa6, 0xda, 0x92, 0x85, 0x62, 0xf3, 0xc5, 0xb2, 0xa4, 0x84, 0xba, 0x64,
        0xa5, 0xb3, 0xd2, 0x81, 0xe5, 0xd3, 0xaa, 0xc4, 0xca, 0xf2, 0xb1, 0xe4, 0xd1, 0x83, 0x63,
        0xea, 0xc3, 0xe2, 0x82, 0xf1, 0xa3, 0xc2, 0xa1, 0xc1, 0xe3, 0xa2, 0xe1, 0xff, 0xff,
    ],
];

/// Offset of the coded data from the start of the RawData record (after a 514-byte header);
/// the low-order bits, when present, come first.
pub(crate) const CODED_AT: usize = 514;

/// A lookup over the longest code length: `(length, symbol)` for every bit pattern, assigned
/// in canonical order (shorter codes first). Patterns no code covers read as a zero-length 0.
struct Table {
    max: u32,
    lut: Vec<(u8, u8)>,
}

impl Table {
    fn new(spec: &[u8]) -> Table {
        let counts = &spec[..16];
        let max = (1..=16).rev().find(|&l| counts[l - 1] != 0).unwrap_or(0);
        let size = 1usize << max;
        let mut lut = Vec::with_capacity(size);
        let mut syms = spec[16..].iter();
        'fill: for len in 1..=max {
            for _ in 0..counts[len - 1] {
                let Some(&s) = syms.next() else { break 'fill };
                for _ in 0..1usize << (max - len) {
                    if lut.len() == size {
                        break 'fill;
                    }
                    lut.push((len as u8, s));
                }
            }
        }
        lut.resize(size, (0, 0));
        Table {
            max: max as u32,
            lut,
        }
    }

    #[inline(always)]
    fn decode(&self, br: &mut BitReader) -> u8 {
        if self.max == 0 {
            return 0;
        }
        br.refill();
        let (len, s) = self.lut[br.peek(self.max) as usize];
        br.consume(len as u32);
        s
    }
}

/// True when the record holds low-order bits before the coded data: coded data never has an
/// `FF` followed by anything but `00`.
pub(crate) fn has_low_bits(rec: &[u8]) -> bool {
    let test = rec.get(CODED_AT..rec.len().min(0x4000 - 26)).unwrap_or(&[]);
    let mut low = true;
    for p in test.windows(2) {
        if p[0] == 0xff {
            if p[1] != 0 {
                return true;
            }
            low = false;
        }
    }
    low
}

/// Decodes `w x h` samples from the RawData record `rec` with table set `table` (clamped to
/// 0..=2). Returns true when the data ran short.
pub(crate) fn decode(
    rec: &[u8],
    table: u32,
    low_bits: bool,
    w: usize,
    h: usize,
    out: &mut [u16],
) -> bool {
    let t = (table as usize).min(2);
    let first = Table::new(&FIRST[t]);
    let rest = Table::new(&REST[t]);
    let coded_at = CODED_AT + if low_bits { w * h / 4 } else { 0 };
    let mut br = BitReader::new(rec.get(coded_at..).unwrap_or(&[]), true);
    let mut carry = 0i32;
    let mut base = [512i32; 2];
    let mut pnum = 0usize;
    let mut diff = [0i32; 64];
    let mut short = rec.len() < coded_at;
    let out = &mut out[..w * h];
    for (band, rows) in out.chunks_mut(8 * w).enumerate() {
        for block in rows.chunks_exact_mut(64) {
            diff.fill(0);
            let mut i = 0usize;
            while i < 64 {
                let leaf = if i == 0 { &first } else { &rest }.decode(&mut br);
                if leaf == 0 && i > 0 {
                    break;
                }
                if leaf != 0xff {
                    i += (leaf >> 4) as usize;
                    let len = (leaf & 15) as u32;
                    if len > 0 {
                        br.refill();
                        let mut v = br.bits(len) as i32;
                        if v & (1 << (len - 1)) == 0 {
                            v -= (1 << len) - 1;
                        }
                        if i < 64 {
                            diff[i] = v;
                        }
                    }
                }
                i += 1;
            }
            diff[0] += carry;
            carry = diff[0];
            for (k, (o, &d)) in block.iter_mut().zip(&diff).enumerate() {
                if pnum.is_multiple_of(w) {
                    base = [512; 2];
                }
                pnum += 1;
                base[k & 1] += d;
                *o = base[k & 1] as u16;
            }
        }
        if low_bits {
            let at = band * 8 * w / 4;
            let low = rec.get(at..at + rows.len() / 4).unwrap_or(&[]);
            short |= low.len() < rows.len() / 4;
            for (px, &c) in rows.chunks_exact_mut(4).zip(low) {
                for (k, p) in px.iter_mut().enumerate() {
                    let mut v = (*p << 2) | ((c >> (2 * k)) & 3) as u16;
                    if w == 2672 && v < 512 {
                        v += 2;
                    }
                    *p = v;
                }
            }
        }
    }
    short || br.overrun()
}

/// The code of `sym` in table set `t` (first-coefficient or other table), for test files.
#[cfg(test)]
pub(crate) fn test_code(first: bool, t: usize, sym: u8) -> (u32, u32) {
    tests::code(if first { &FIRST[t] } else { &REST[t] }, sym)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// MSB-first writer with `FF 00` stuffing.
    struct W {
        acc: u64,
        n: u32,
        out: Vec<u8>,
    }
    impl W {
        fn put(&mut self, v: u32, len: u32) {
            for k in (0..len).rev() {
                self.acc = self.acc << 1 | ((v >> k) & 1) as u64;
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
        fn finish(mut self) -> Vec<u8> {
            while self.n != 0 {
                self.put(1, 1);
            }
            self.out
        }
    }

    /// The canonical code of `sym` in `spec`.
    pub(super) fn code(spec: &[u8], sym: u8) -> (u32, u32) {
        let mut c = 0u32;
        let mut k = 16;
        for len in 1..=16u32 {
            for _ in 0..spec[len as usize - 1] {
                if spec[k] == sym {
                    return (c, len);
                }
                c += 1;
                k += 1;
            }
            c <<= 1;
        }
        panic!("no code for {sym:#x}");
    }

    fn put_diff(w: &mut W, spec: &[u8], run: u8, d: i32) {
        let len = 32 - d.unsigned_abs().leading_zeros();
        let (c, l) = code(spec, run << 4 | len as u8);
        w.put(c, l);
        if len > 0 {
            let v = if d < 0 { d + (1 << len) - 1 } else { d };
            w.put(v as u32, len);
        }
    }

    #[test]
    fn blocks_chain_and_rows_restart() {
        // Two 64-sample blocks = one 128-wide row; then a second row (one band of 2 rows).
        let (w, h) = (128usize, 2usize);
        for t in 0..3 {
            let mut wr = W {
                acc: 0,
                n: 0,
                out: vec![],
            };
            // Block 0: first diff 5, then +3 at position 2 (run 1), end of block.
            put_diff(&mut wr, &FIRST[t], 0, 5);
            put_diff(&mut wr, &REST[t], 1, 3);
            let (c, l) = code(&REST[t], 0);
            wr.put(c, l);
            // Blocks 1..3: first diff -1 each, end of block.
            for _ in 1..4 {
                put_diff(&mut wr, &FIRST[t], 0, -1);
                wr.put(c, l);
            }
            let mut rec = vec![0u8; CODED_AT];
            rec.extend(wr.finish());
            let mut out = vec![0u16; w * h];
            assert!(!decode(&rec, t as u32, false, w, h, &mut out), "table {t}");
            assert_eq!(&out[..4], &[517, 512, 520, 512]);
            assert_eq!(out[63], 512);
            // Block 1 continues the row: carry 5 - 1 = 4 added to the running even column.
            assert_eq!(out[64], 524);
            // Row 1 restarts from 512; blocks 2 and 3 carry 3 then 2.
            assert_eq!(out[128], 515);
            assert_eq!(out[192], 517);
        }
    }

    #[test]
    fn low_bits_and_detection() {
        let (w, h) = (8usize, 8usize);
        let mut rec = vec![0b11_10_01_00u8; w * h / 4];
        rec.resize(rec.len() + CODED_AT, 0xff);
        rec.extend([0x12]);
        assert!(has_low_bits(&rec));
        assert!(!has_low_bits(
            &[0u8; 600]
                .iter()
                .chain(&[0xff, 0])
                .copied()
                .collect::<Vec<_>>()
        ));
        let mut out = vec![0u16; w * h];
        let _ = decode(&rec, 0, true, w, h, &mut out);
        assert_eq!(out[1] & 3, 1);
        assert_eq!(out[3] & 3, 3);
    }

    #[test]
    fn short_and_hostile() {
        let mut out = vec![0u16; 64 * 8];
        assert!(decode(&[], 9, true, 64, 8, &mut out));
        assert!(decode(&[0xff; 600], 1, false, 64, 8, &mut out));
        // Odd sizes that are not multiples of 64 leave the tail at zero.
        let mut out = vec![0u16; 70 * 3];
        let _ = decode(&[0x55; 700], 2, false, 70, 3, &mut out);
    }
}
