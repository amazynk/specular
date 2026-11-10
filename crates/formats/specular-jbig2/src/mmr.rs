//! MMR (ITU-T T.6, "Group 4") bitmap decoding as JBIG2 uses it: no EOLs, an optional
//! EOFB at the end, and the number of bytes used reported (halftone grey-scale planes and
//! collective symbol bitmaps follow one another).
//!
//! Runs are resolved by one table lookup of the next 12 (white) or 13 (black) bits and the
//! coding modes by a lookup of the next 7; rows are kept as lists of changing elements.

use crate::bitmap::{Bitmap, Budget};
use crate::{Error, Result};
use std::sync::OnceLock;

/// White terminating codes 0..=63 (T.4 table 2).
const WHITE_TERM: [&str; 64] = [
    "00110101", "000111", "0111", "1000", "1011", "1100", "1110", "1111", "10011", "10100",
    "00111", "01000", "001000", "000011", "110100", "110101", "101010", "101011", "0100111",
    "0001100", "0001000", "0010111", "0000011", "0000100", "0101000", "0101011", "0010011",
    "0100100", "0011000", "00000010", "00000011", "00011010", "00011011", "00010010", "00010011",
    "00010100", "00010101", "00010110", "00010111", "00101000", "00101001", "00101010", "00101011",
    "00101100", "00101101", "00000100", "00000101", "00001010", "00001011", "01010010", "01010011",
    "01010100", "01010101", "00100100", "00100101", "01011000", "01011001", "01011010", "01011011",
    "01001010", "01001011", "00110010", "00110011", "00110100",
];

/// White makeup codes 64..=1728 in steps of 64.
const WHITE_MAKEUP: [&str; 27] = [
    "11011",
    "10010",
    "010111",
    "0110111",
    "00110110",
    "00110111",
    "01100100",
    "01100101",
    "01101000",
    "01100111",
    "011001100",
    "011001101",
    "011010010",
    "011010011",
    "011010100",
    "011010101",
    "011010110",
    "011010111",
    "011011000",
    "011011001",
    "011011010",
    "011011011",
    "010011000",
    "010011001",
    "010011010",
    "011000",
    "010011011",
];

/// Black terminating codes 0..=63.
const BLACK_TERM: [&str; 64] = [
    "0000110111",
    "010",
    "11",
    "10",
    "011",
    "0011",
    "0010",
    "00011",
    "000101",
    "000100",
    "0000100",
    "0000101",
    "0000111",
    "00000100",
    "00000111",
    "000011000",
    "0000010111",
    "0000011000",
    "0000001000",
    "00001100111",
    "00001101000",
    "00001101100",
    "00000110111",
    "00000101000",
    "00000010111",
    "00000011000",
    "000011001010",
    "000011001011",
    "000011001100",
    "000011001101",
    "000001101000",
    "000001101001",
    "000001101010",
    "000001101011",
    "000011010010",
    "000011010011",
    "000011010100",
    "000011010101",
    "000011010110",
    "000011010111",
    "000001101100",
    "000001101101",
    "000011011010",
    "000011011011",
    "000001010100",
    "000001010101",
    "000001010110",
    "000001010111",
    "000001100100",
    "000001100101",
    "000001010010",
    "000001010011",
    "000000100100",
    "000000110111",
    "000000111000",
    "000000100111",
    "000000101000",
    "000001011000",
    "000001011001",
    "000000101011",
    "000000101100",
    "000001011010",
    "000001100110",
    "000001100111",
];

/// Black makeup codes 64..=1728 in steps of 64.
const BLACK_MAKEUP: [&str; 27] = [
    "0000001111",
    "000011001000",
    "000011001001",
    "000001011011",
    "000000110011",
    "000000110100",
    "000000110101",
    "0000001101100",
    "0000001101101",
    "0000001001010",
    "0000001001011",
    "0000001001100",
    "0000001001101",
    "0000001110010",
    "0000001110011",
    "0000001110100",
    "0000001110101",
    "0000001110110",
    "0000001110111",
    "0000001010010",
    "0000001010011",
    "0000001010100",
    "0000001010101",
    "0000001011010",
    "0000001011011",
    "0000001100100",
    "0000001100101",
];

/// Extended makeup codes 1792..=2560 in steps of 64, both colours.
const EXT_MAKEUP: [&str; 13] = [
    "00000001000",
    "00000001100",
    "00000001101",
    "000000010010",
    "000000010011",
    "000000010100",
    "000000010101",
    "000000010110",
    "000000010111",
    "000000011100",
    "000000011101",
    "000000011110",
    "000000011111",
];

/// A run code: length 0 = invalid; `term` = terminating code.
#[derive(Clone, Copy, Default)]
struct Run {
    len: u8,
    term: bool,
    run: u16,
}

struct Tables {
    white: Vec<Run>,
    black: Vec<Run>,
    /// 2D modes by the next 7 bits: (mode, length).
    modes: [(Mode, u8); 128],
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Invalid,
    Pass,
    Horizontal,
    Vertical(i8),
    Extension,
}

fn fill(table: &mut [Run], bits: u32, code: &str, run: u16, term: bool) {
    let len = code.len() as u32;
    let v = u32::from_str_radix(code, 2).unwrap_or(0);
    let shift = bits - len;
    for i in 0..(1u32 << shift) {
        table[((v << shift) | i) as usize] = Run {
            len: len as u8,
            term,
            run,
        };
    }
}

fn tables() -> &'static Tables {
    static T: OnceLock<Tables> = OnceLock::new();
    T.get_or_init(|| {
        let mut white = vec![Run::default(); 1 << 12];
        let mut black = vec![Run::default(); 1 << 13];
        for (i, c) in WHITE_TERM.iter().enumerate() {
            fill(&mut white, 12, c, i as u16, true);
        }
        for (i, c) in WHITE_MAKEUP.iter().enumerate() {
            fill(&mut white, 12, c, (i as u16 + 1) * 64, false);
        }
        for (i, c) in BLACK_TERM.iter().enumerate() {
            fill(&mut black, 13, c, i as u16, true);
        }
        for (i, c) in BLACK_MAKEUP.iter().enumerate() {
            fill(&mut black, 13, c, (i as u16 + 1) * 64, false);
        }
        for (i, c) in EXT_MAKEUP.iter().enumerate() {
            fill(&mut white, 12, c, 1792 + i as u16 * 64, false);
            fill(&mut black, 13, c, 1792 + i as u16 * 64, false);
        }
        let mut modes = [(Mode::Invalid, 0u8); 128];
        for (code, m) in [
            ("1", Mode::Vertical(0)),
            ("011", Mode::Vertical(1)),
            ("000011", Mode::Vertical(2)),
            ("0000011", Mode::Vertical(3)),
            ("010", Mode::Vertical(-1)),
            ("000010", Mode::Vertical(-2)),
            ("0000010", Mode::Vertical(-3)),
            ("001", Mode::Horizontal),
            ("0001", Mode::Pass),
            ("0000001", Mode::Extension),
        ] {
            let len = code.len() as u32;
            let v = u32::from_str_radix(code, 2).unwrap_or(0);
            for i in 0..(1u32 << (7 - len)) {
                modes[((v << (7 - len)) | i) as usize] = (m, len as u8);
            }
        }
        Tables {
            white,
            black,
            modes,
        }
    })
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl Reader<'_> {
    #[inline]
    fn peek(&self, n: u32) -> u32 {
        let byte = self.pos >> 3;
        let mut v: u64 = 0;
        for i in 0..5 {
            v = (v << 8) | self.data.get(byte + i).copied().unwrap_or(0) as u64;
        }
        let v = v << (24 + (self.pos & 7)); // top bit of v is the next bit
        (v >> (64 - n)) as u32
    }

    #[inline]
    fn skip(&mut self, n: u32) {
        self.pos += n as usize;
    }

    fn past_end(&self) -> bool {
        self.pos > self.data.len() * 8
    }
}

fn run(r: &mut Reader, black: bool) -> Result<u32> {
    let t = tables();
    let mut total = 0u32;
    loop {
        let e = if black {
            t.black[r.peek(13) as usize]
        } else {
            t.white[r.peek(12) as usize]
        };
        if e.len == 0 || r.past_end() {
            return Err(Error::Malformed("invalid MMR run code".into()));
        }
        r.skip(e.len as u32);
        total = total.saturating_add(e.run as u32);
        if e.term {
            return Ok(total);
        }
    }
}

/// Decodes a `w`×`h` MMR bitmap from `data`; returns it and the bytes used (an EOFB
/// included). Rows after an early EOFB are white; a coding error ends the bitmap there and
/// sets the flag.
pub(crate) fn decode(
    data: &[u8],
    w: u32,
    h: u32,
    budget: &Budget,
) -> Result<(Bitmap, usize, bool)> {
    let mut bm = Bitmap::new(w, h, budget)?;
    let t = tables();
    let mut r = Reader { data, pos: 0 };
    let wi = w as i64;
    // Changing elements of the reference and current rows, with sentinels at `w`.
    let mut refl: Vec<i64> = vec![wi, wi, wi];
    let mut cur: Vec<i64> = Vec::with_capacity(64);
    let mut damaged = false;
    'rows: for y in 0..h {
        if r.peek(24) == 0x001001 {
            r.skip(24);
            break;
        }
        cur.clear();
        let mut a0: i64 = -1;
        let mut colour = 0usize; // 0 white, 1 black
        let mut idx = 0usize;
        while a0 < wi {
            if r.past_end() {
                damaged = true;
                break 'rows;
            }
            let (mode, len) = t.modes[r.peek(7) as usize];
            // b1: the first changing element on the reference row right of a0 whose colour
            // is the opposite of a0's (even entries start black runs).
            while idx > 0 && refl[idx - 1] > a0 {
                idx -= 1;
            }
            while idx + 2 < refl.len() && (refl[idx] <= a0 || idx & 1 != colour) {
                idx += 1;
            }
            let b1 = refl[idx];
            let b2 = refl[(idx + 1).min(refl.len() - 1)];
            match mode {
                Mode::Pass => {
                    r.skip(len as u32);
                    a0 = b2;
                }
                Mode::Horizontal => {
                    r.skip(len as u32);
                    let r1 = run(&mut r, colour == 1);
                    let r2 = run(&mut r, colour == 0);
                    let (Ok(r1), Ok(r2)) = (r1, r2) else {
                        damaged = true;
                        break 'rows;
                    };
                    let start = a0.max(0);
                    let a1 = (start + r1 as i64).min(wi);
                    let a2 = (a1 + r2 as i64).min(wi);
                    cur.push(a1);
                    cur.push(a2);
                    a0 = a2;
                }
                Mode::Vertical(d) => {
                    r.skip(len as u32);
                    let a1 = (b1 + d as i64).clamp(0, wi);
                    if a1 < cur.last().copied().unwrap_or(0) {
                        damaged = true;
                        break 'rows;
                    }
                    cur.push(a1);
                    a0 = a1;
                    colour ^= 1;
                }
                Mode::Extension | Mode::Invalid => {
                    damaged = true;
                    break 'rows;
                }
            }
        }
        // Pixels: black between cur[2k] and cur[2k+1].
        let row = &mut bm.d[y as usize * w as usize..(y as usize + 1) * w as usize];
        let mut i = 0;
        while i < cur.len() {
            let s = cur[i].clamp(0, wi) as usize;
            let e = cur.get(i + 1).copied().unwrap_or(wi).clamp(0, wi) as usize;
            if s < e {
                row[s..e].fill(1);
            }
            i += 2;
        }
        std::mem::swap(&mut refl, &mut cur);
        refl.extend([wi, wi, wi]);
    }
    if r.peek(24) == 0x001001 {
        r.skip(24);
    }
    Ok((bm, r.pos.div_ceil(8).min(data.len()), damaged))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Writes bits from a "0101…" string, padded with zeros to bytes.
    fn bytes(s: &str) -> Vec<u8> {
        let bits: Vec<u8> = s
            .bytes()
            .filter(|b| *b == b'0' || *b == b'1')
            .map(|b| b - b'0')
            .collect();
        bits.chunks(8)
            .map(|c| {
                c.iter()
                    .enumerate()
                    .fold(0u8, |a, (i, b)| a | (b << (7 - i)))
            })
            .collect()
    }

    #[test]
    fn horizontal_and_vertical_rows() {
        // 8 pixels wide. Row 0: horizontal, white 2 ("0111"), black 3 ("10"); then
        // horizontal white 3 ("1000"), black 0 ("0000110111"). Row 1: V0 V0 V0 V0 copies it.
        let s = "001 0111 10  001 1000 0000110111  1 1 1 1";
        let (bm, _, damaged) = decode(&bytes(s), 8, 2, &Budget::new()).unwrap();
        assert!(!damaged);
        assert_eq!(bm.row(0), &[0, 0, 1, 1, 1, 0, 0, 0]);
        assert_eq!(bm.row(1), bm.row(0));
    }

    #[test]
    fn eofb_ends_early() {
        let s = "1 000000000001 000000000001";
        let (bm, used, _) = decode(&bytes(s), 8, 4, &Budget::new()).unwrap();
        assert!(bm.d.iter().all(|&p| p == 0));
        assert_eq!(used, 4);
    }
}
