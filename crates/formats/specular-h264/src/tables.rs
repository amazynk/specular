//! Derived lookup tables: scans, QP mappings, dequantisation factors and the CAVLC VLC
//! decoding tables built from the spec's code tables at first use.

use std::sync::OnceLock;

use crate::bits::BitReader;
use crate::spec_tables::{
    COEFF_TOKEN, COEFF_TOKEN_DC422, RUN_BEFORE, TOTAL_ZEROS, TOTAL_ZEROS_DC, TOTAL_ZEROS_DC422,
};

/// 4x4 zig-zag (frame) scan: scan position to raster index.
pub const ZIGZAG4: [u8; 16] = [0, 1, 4, 8, 5, 2, 3, 6, 9, 12, 13, 10, 7, 11, 14, 15];

/// 4x4 field scan (Table 8-13): scan position to raster index.
pub const FIELD4: [u8; 16] = [0, 4, 1, 8, 12, 5, 9, 13, 2, 6, 10, 14, 3, 7, 11, 15];

/// 8x8 field scan (Table 8-14): scan position to raster index.
#[rustfmt::skip]
pub static FIELD8: [u8; 64] = [
    0, 8, 16, 1, 9, 24, 32, 17, 2, 25, 40, 48, 56, 33, 10, 3,
    18, 41, 49, 57, 26, 11, 4, 19, 34, 42, 50, 58, 27, 12, 5, 20,
    35, 43, 51, 59, 28, 13, 6, 21, 36, 44, 52, 60, 29, 14, 22, 37,
    45, 53, 61, 30, 7, 15, 38, 46, 54, 62, 23, 31, 39, 47, 55, 63,
];

/// 8x8 zig-zag (frame) scan: scan position to raster index.
pub static ZIGZAG8: [u8; 64] = zigzag8();

const fn zigzag8() -> [u8; 64] {
    let mut out = [0u8; 64];
    let mut i = 0;
    let mut x: i32 = 0;
    let mut y: i32 = 0;
    // Walk the anti-diagonals, alternating direction (up-right first).
    while i < 64 {
        out[i] = (y * 8 + x) as u8;
        i += 1;
        if (x + y) % 2 == 0 {
            if x == 7 {
                y += 1;
            } else if y == 0 {
                x += 1;
            } else {
                x += 1;
                y -= 1;
            }
        } else if y == 7 {
            x += 1;
        } else if x == 0 {
            y += 1;
        } else {
            x -= 1;
            y += 1;
        }
    }
    out
}

/// QPc as a function of qPI (Table 8-15); qPI is clipped to `-off..=51` first, `off` being
/// QpBdOffsetC (0 at 8 bits).
pub const fn qpc(qpi: i32, off: i32) -> i32 {
    const HI: [u8; 22] = [
        29, 30, 31, 32, 32, 33, 34, 34, 35, 35, 36, 36, 37, 37, 37, 38, 38, 38, 39, 39, 39, 39,
    ];
    let q = if qpi < -off {
        -off
    } else if qpi > 51 {
        51
    } else {
        qpi
    };
    if q < 30 {
        q
    } else {
        HI[(q - 30) as usize] as i32
    }
}

const NORM4: [[i32; 3]; 6] = [
    [10, 16, 13],
    [11, 18, 14],
    [13, 20, 16],
    [14, 23, 18],
    [16, 25, 20],
    [18, 29, 23],
];

const NORM8: [[i32; 6]; 6] = [
    [20, 18, 32, 19, 25, 24],
    [22, 19, 35, 21, 28, 26],
    [26, 23, 42, 24, 33, 31],
    [28, 25, 45, 26, 35, 33],
    [32, 28, 51, 30, 40, 38],
    [36, 32, 58, 34, 46, 43],
];

/// normAdjust4x4(m, i, j) for raster index `r` = 4i + j.
pub fn norm4(m: usize, r: usize) -> i32 {
    let (i, j) = (r >> 2, r & 3);
    let k = if i % 2 == 0 && j % 2 == 0 {
        0
    } else if i % 2 == 1 && j % 2 == 1 {
        1
    } else {
        2
    };
    NORM4[m][k]
}

/// normAdjust8x8(m, i, j) for raster index `r` = 8i + j.
pub fn norm8(m: usize, r: usize) -> i32 {
    let (i, j) = (r >> 3, r & 7);
    let k = if i % 4 == 0 && j % 4 == 0 {
        0
    } else if i % 2 == 1 && j % 2 == 1 {
        1
    } else if i % 4 == 2 && j % 4 == 2 {
        2
    } else if (i % 4 == 0 && j % 2 == 1) || (i % 2 == 1 && j % 4 == 0) {
        3
    } else if (i % 4 == 0 && j % 4 == 2) || (i % 4 == 2 && j % 4 == 0) {
        4
    } else {
        5
    };
    NORM8[m][k]
}

/// luma4x4BlkIdx (parse order) to raster 4x4 block index within the macroblock.
pub const BLK_TO_RASTER: [u8; 16] = [0, 1, 4, 5, 2, 3, 6, 7, 8, 9, 12, 13, 10, 11, 14, 15];
/// Raster 4x4 block index to luma4x4BlkIdx.
pub const RASTER_TO_BLK: [u8; 16] = [0, 1, 4, 5, 2, 3, 6, 7, 8, 9, 12, 13, 10, 11, 14, 15];

/// A multi-level VLC lookup table: the first level indexed by `bits` bits; entries are
/// `(symbol, length)` leaves or `(subtable start, -subtable bits)` links.
pub struct Vlc {
    bits: u32,
    table: Vec<(i16, i8)>,
}

impl Vlc {
    /// `codes`: (code, length, symbol) with lengths 1..=16.
    fn new(codes: &[(u32, u32, i16)], bits: u32) -> Vlc {
        let mut v = Vlc {
            bits,
            table: Vec::new(),
        };
        v.build(codes, bits, 0);
        v
    }

    /// Builds a table for `codes` whose first `prefix_len` bits are already consumed and
    /// returns its start.
    fn build(&mut self, codes: &[(u32, u32, i16)], bits: u32, prefix_len: u32) -> usize {
        let start = self.table.len();
        let size = 1usize << bits;
        self.table.resize(start + size, (0, 0));
        // Leaves.
        for &(code, len, sym) in codes {
            let rem = len - prefix_len;
            if rem <= bits {
                let c = (code & ((1 << rem) - 1)) as usize;
                let shift = bits - rem;
                for k in 0..(1usize << shift) {
                    self.table[start + ((c << shift) | k)] = (sym, rem as i8);
                }
            }
        }
        // Links for longer codes.
        for idx in 0..size {
            let longer: Vec<(u32, u32, i16)> = codes
                .iter()
                .copied()
                .filter(|&(code, len, _)| {
                    let rem = len - prefix_len;
                    rem > bits && ((code >> (rem - bits)) & ((1 << bits) - 1)) as usize == idx
                })
                .collect();
            if longer.is_empty() {
                continue;
            }
            let max_rem = longer
                .iter()
                .map(|&(_, l, _)| l - prefix_len - bits)
                .max()
                .unwrap();
            let sub_bits = max_rem.min(bits);
            let sub = self.build(&longer, sub_bits, prefix_len + bits);
            self.table[start + idx] = (sub as i16, -(sub_bits as i8));
        }
        start
    }

    /// Decodes one symbol; `None` for an invalid code.
    #[inline(always)]
    pub fn read(&self, r: &mut BitReader) -> Option<i16> {
        let mut bits = self.bits;
        let mut base = 0usize;
        loop {
            let (sym, len) = self.table[base + r.peek(bits) as usize];
            if len > 0 {
                r.skip(len as u32);
                return Some(sym);
            }
            if len == 0 {
                return None;
            }
            r.skip(bits);
            base = sym as usize;
            bits = (-len) as u32;
        }
    }
}

pub struct CavlcTables {
    /// coeff_token per nC class (0..2, 2..4, 4..8, 4:2:0 chroma DC, 4:2:2 chroma DC);
    /// symbol = TotalCoeff*4 + T1s.
    pub coeff_token: [Vlc; 5],
    /// total_zeros for 4x4 blocks indexed by TotalCoeff (1..=15).
    pub total_zeros: Vec<Vlc>,
    /// total_zeros for 4:2:0 chroma DC indexed by TotalCoeff (1..=3).
    pub total_zeros_dc: Vec<Vlc>,
    /// total_zeros for 4:2:2 chroma DC indexed by TotalCoeff (1..=7).
    pub total_zeros_dc422: Vec<Vlc>,
    /// run_before indexed by min(zerosLeft, 7) (1..=7).
    pub run_before: Vec<Vlc>,
}

pub fn cavlc() -> &'static CavlcTables {
    static T: OnceLock<CavlcTables> = OnceLock::new();
    T.get_or_init(|| {
        let ct = |table: &[[(u8, u8); 4]]| {
            let mut codes = Vec::new();
            for (tc, row) in table.iter().enumerate() {
                for (t1, &(code, len)) in row.iter().enumerate() {
                    if len > 0 && t1 <= tc {
                        codes.push((code as u32, len as u32, (tc * 4 + t1) as i16));
                    }
                }
            }
            Vlc::new(&codes, 8)
        };
        let coeff_token = [
            ct(&COEFF_TOKEN[0]),
            ct(&COEFF_TOKEN[1]),
            ct(&COEFF_TOKEN[2]),
            ct(&COEFF_TOKEN[4][..5]),
            ct(&COEFF_TOKEN_DC422),
        ];
        let mut total_zeros = vec![Vlc::new(&[(1, 1, 0)], 1)];
        for tc in 1..16 {
            let codes: Vec<_> = (0..=16 - tc)
                .filter_map(|tz| {
                    let (code, len) = TOTAL_ZEROS[tc][tz];
                    (len > 0).then_some((code as u32, len as u32, tz as i16))
                })
                .collect();
            total_zeros.push(Vlc::new(&codes, 9));
        }
        let mut total_zeros_dc = vec![Vlc::new(&[(1, 1, 0)], 1)];
        for tc in 1..4 {
            let codes: Vec<_> = (0..=4 - tc)
                .filter_map(|tz| {
                    let (code, len) = TOTAL_ZEROS_DC[tc][tz];
                    (len > 0).then_some((code as u32, len as u32, tz as i16))
                })
                .collect();
            total_zeros_dc.push(Vlc::new(&codes, 3));
        }
        let mut total_zeros_dc422 = vec![Vlc::new(&[(1, 1, 0)], 1)];
        for tc in 1..8 {
            let codes: Vec<_> = (0..=8 - tc)
                .filter_map(|tz| {
                    let (code, len) = TOTAL_ZEROS_DC422[tc][tz];
                    (len > 0).then_some((code as u32, len as u32, tz as i16))
                })
                .collect();
            total_zeros_dc422.push(Vlc::new(&codes, 5));
        }
        let mut run_before = vec![Vlc::new(&[(1, 1, 0)], 1)];
        for zl in 1..8 {
            let codes: Vec<_> = (0..15)
                .filter_map(|run| {
                    let (code, len) = RUN_BEFORE[zl][run];
                    (len > 0 && (run <= zl || zl == 7)).then_some((
                        code as u32,
                        len as u32,
                        run as i16,
                    ))
                })
                .collect();
            run_before.push(Vlc::new(&codes, if zl == 7 { 11 } else { 3 }));
        }
        CavlcTables {
            coeff_token,
            total_zeros,
            total_zeros_dc,
            total_zeros_dc422,
            run_before,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bits::{BitReader, unescape};

    #[test]
    fn zigzag8_matches_known_prefix() {
        assert_eq!(&ZIGZAG8[..10], &[0, 1, 8, 16, 9, 2, 3, 10, 17, 24]);
        assert_eq!(ZIGZAG8[63], 63);
        let mut seen = [false; 64];
        for &z in ZIGZAG8.iter() {
            seen[z as usize] = true;
        }
        assert!(seen.iter().all(|&s| s));
    }

    #[test]
    fn coeff_token_codes() {
        let t = cavlc();
        let mut v = Vec::new();
        // "1" -> (0,0); "01" -> (1,1); "000101" -> (1,0) for 0 <= nC < 2.
        let n = unescape(&[0b1010_0010, 0b1000_0000], &mut v);
        let mut r = BitReader::new(&v, n);
        assert_eq!(t.coeff_token[0].read(&mut r), Some(0));
        assert_eq!(t.coeff_token[0].read(&mut r), Some(5));
        assert_eq!(t.coeff_token[0].read(&mut r), Some(4));
        // Every class decodes every code it defines.
        for class in 0..5 {
            for tc in 0..17 {
                for t1 in 0..4 {
                    let (code, len) = match class {
                        4 => COEFF_TOKEN_DC422.get(tc).map_or((0, 0), |r| r[t1]),
                        _ => COEFF_TOKEN[[0, 1, 2, 4][class]][tc][t1],
                    };
                    if len == 0 || t1 > tc {
                        continue;
                    }
                    let bits = (code as u32) << (32 - len as u32);
                    let n = unescape(&bits.to_be_bytes(), &mut v);
                    let mut r = BitReader::new(&v, n);
                    assert_eq!(
                        t.coeff_token[class].read(&mut r),
                        Some((tc * 4 + t1) as i16)
                    );
                    assert_eq!(r.pos(), len as usize);
                }
            }
        }
    }
}
