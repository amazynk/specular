//! The AC coefficient code of SMPTE 314M Table 25 (the same as IEC 61834-2), with the code
//! lengths of Table 24.
//!
//! Provenance: typed from the free SMPTE ST 314:2005 PDF (pub.smpte.org). Table 25 has three
//! misprints, corrected here from Table 24's lengths and the code's prefix structure:
//! (7, 1) and (8, 1) print 7 bits where they are 8 (`11100000s`, `11100001s`), and (1, 16) and
//! (1, 17) print 11 bits where they are 12 (`111110111110s`, `111110111111s`). The unit tests
//! check that the corrected code is prefix-free and complete.

use std::sync::OnceLock;

/// One code word: `bits` (MSB first, without the sign bit), its length, and the (run, amp)
/// pair it codes. Amplitude 0 codes have no sign bit.
pub(crate) const CODES: &[(&str, u8, u8)] = &[
    ("00", 0, 1),
    ("010", 0, 2),
    ("0111", 1, 1),
    ("1000", 0, 3),
    ("1001", 0, 4),
    ("10100", 2, 1),
    ("10101", 1, 2),
    ("10110", 0, 5),
    ("10111", 0, 6),
    ("110000", 3, 1),
    ("110001", 4, 1),
    ("110010", 0, 7),
    ("110011", 0, 8),
    ("1101000", 5, 1),
    ("1101001", 6, 1),
    ("1101010", 2, 2),
    ("1101011", 1, 3),
    ("1101100", 1, 4),
    ("1101101", 0, 9),
    ("1101110", 0, 10),
    ("1101111", 0, 11),
    ("11100000", 7, 1),
    ("11100001", 8, 1),
    ("11100010", 9, 1),
    ("11100011", 10, 1),
    ("11100100", 3, 2),
    ("11100101", 4, 2),
    ("11100110", 2, 3),
    ("11100111", 1, 5),
    ("11101000", 1, 6),
    ("11101001", 1, 7),
    ("11101010", 0, 12),
    ("11101011", 0, 13),
    ("11101100", 0, 14),
    ("11101101", 0, 15),
    ("11101110", 0, 16),
    ("11101111", 0, 17),
    ("111100000", 11, 1),
    ("111100001", 12, 1),
    ("111100010", 13, 1),
    ("111100011", 14, 1),
    ("111100100", 5, 2),
    ("111100101", 6, 2),
    ("111100110", 3, 3),
    ("111100111", 4, 3),
    ("111101000", 2, 4),
    ("111101001", 2, 5),
    ("111101010", 1, 8),
    ("111101011", 0, 18),
    ("111101100", 0, 19),
    ("111101101", 0, 20),
    ("111101110", 0, 21),
    ("111101111", 0, 22),
    ("1111100000", 5, 3),
    ("1111100001", 3, 4),
    ("1111100010", 3, 5),
    ("1111100011", 2, 6),
    ("1111100100", 1, 9),
    ("1111100101", 1, 10),
    ("1111100110", 1, 11),
    ("11111001110", 0, 0),
    ("11111001111", 1, 0),
    ("11111010000", 6, 3),
    ("11111010001", 4, 4),
    ("11111010010", 3, 6),
    ("11111010011", 1, 12),
    ("11111010100", 1, 13),
    ("11111010101", 1, 14),
    ("111110101100", 2, 0),
    ("111110101101", 3, 0),
    ("111110101110", 4, 0),
    ("111110101111", 5, 0),
    ("111110110000", 7, 2),
    ("111110110001", 8, 2),
    ("111110110010", 9, 2),
    ("111110110011", 10, 2),
    ("111110110100", 7, 3),
    ("111110110101", 8, 3),
    ("111110110110", 4, 5),
    ("111110110111", 3, 7),
    ("111110111000", 2, 7),
    ("111110111001", 2, 8),
    ("111110111010", 2, 9),
    ("111110111011", 2, 10),
    ("111110111100", 2, 11),
    ("111110111101", 1, 15),
    ("111110111110", 1, 16),
    ("111110111111", 1, 17),
];

/// The end-of-block code `0110`.
const EOB: (u16, u8) = (0b0110, 4);

/// A decoded entry: what a 16-bit window starting at a code word holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Sym {
    /// No code word starts with these bits.
    Invalid,
    EndOfBlock,
    /// `run` zero coefficients, then one of amplitude `amp` (which may be 0) and sign `neg`;
    /// `len` bits including the sign.
    Coef {
        len: u8,
        run: u8,
        amp: u8,
        neg: bool,
    },
}

impl Sym {
    pub(crate) fn len(self) -> u32 {
        match self {
            Sym::Invalid => 0,
            Sym::EndOfBlock => u32::from(EOB.1),
            Sym::Coef { len, .. } => u32::from(len),
        }
    }
}

/// Packed table entry: bit 0..5 len (0 invalid), 6 eob, 7 neg, 8..15 run, 16..23 amp.
fn pack(s: Sym) -> u32 {
    match s {
        Sym::Invalid => 0,
        Sym::EndOfBlock => u32::from(EOB.1) | 1 << 6,
        Sym::Coef { len, run, amp, neg } => {
            u32::from(len) | u32::from(neg) << 7 | u32::from(run) << 8 | u32::from(amp) << 16
        }
    }
}

fn unpack(v: u32) -> Sym {
    let len = (v & 63) as u8;
    if len == 0 {
        Sym::Invalid
    } else if v & 1 << 6 != 0 {
        Sym::EndOfBlock
    } else {
        Sym::Coef {
            len,
            run: (v >> 8) as u8,
            amp: (v >> 16) as u8,
            neg: v & 1 << 7 != 0,
        }
    }
}

fn table() -> &'static [u32] {
    static T: OnceLock<Vec<u32>> = OnceLock::new();
    T.get_or_init(|| {
        let mut t = vec![0u32; 1 << 16];
        let mut put = |bits: u32, len: u32, s: Sym| {
            let shift = 16 - len;
            let base = (bits << shift) as usize;
            for e in &mut t[base..base + (1usize << shift)] {
                *e = pack(s);
            }
        };
        put(u32::from(EOB.0), u32::from(EOB.1), Sym::EndOfBlock);
        let mut add = |bits: u32, len: u32, run: u8, amp: u8| {
            if amp == 0 {
                put(
                    bits,
                    len,
                    Sym::Coef {
                        len: len as u8,
                        run,
                        amp,
                        neg: false,
                    },
                );
            } else {
                for neg in [false, true] {
                    let l = len + 1;
                    put(
                        bits << 1 | u32::from(neg),
                        l,
                        Sym::Coef {
                            len: l as u8,
                            run,
                            amp,
                            neg,
                        },
                    );
                }
            }
        };
        for &(s, run, amp) in CODES {
            add(
                u32::from_str_radix(s, 2).unwrap_or(0),
                s.len() as u32,
                run,
                amp,
            );
        }
        // (R, 0) for R = 6..=61: `1111110` and six bits of R.
        for r in 6u32..=61 {
            add(0b1111110 << 6 | r, 13, r as u8, 0);
        }
        // (0, A) for A = 23..=255: `1111111`, eight bits of A, sign.
        for a in 23u32..=255 {
            add(0b1111111 << 8 | a, 15, 0, a as u8);
        }
        t
    })
}

/// The code word at the head of a 16-bit window (MSB first, zero padded past the data).
#[inline]
pub(crate) fn lookup(window: u16) -> Sym {
    unpack(table()[window as usize])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_is_complete_and_prefix_free() {
        // Every 16-bit pattern starts with exactly one code word: the table is full (Kraft sum
        // of 1) and no fill overwrote another, which the counts below confirm.
        let t = table();
        let mut words: Vec<(u32, u32)> = CODES
            .iter()
            .map(|&(s, _, _)| (u32::from_str_radix(s, 2).unwrap(), s.len() as u32))
            .collect();
        words.push((u32::from(EOB.0), 4));
        words.push((0b1111110, 7));
        words.push((0b1111111, 7));
        for (i, &(a, la)) in words.iter().enumerate() {
            for &(b, lb) in &words[i + 1..] {
                let l = la.min(lb);
                assert_ne!(a >> (la - l), b >> (lb - l), "prefix clash {a:b}/{b:b}");
            }
        }
        let k: f64 = words.iter().map(|&(_, l)| 2f64.powi(-(l as i32))).sum();
        assert!((k - 1.0).abs() < 1e-12, "Kraft sum {k}");
        // The only holes: escape forms with R < 6 or R > 61, or A < 23.
        for (w, &e) in t.iter().enumerate() {
            if e == 0 {
                let top = w >> 9;
                assert!(top == 0b1111110 || top == 0b1111111, "hole at {w:016b}");
            }
        }
    }

    #[test]
    fn lengths_match_table_24() {
        // A few rows of Table 24 (lengths without the sign bit).
        let len = |run: u8, amp: u8| {
            CODES
                .iter()
                .find(|c| c.1 == run && c.2 == amp)
                .map(|c| c.0.len())
                .unwrap()
        };
        assert_eq!(len(0, 1), 2);
        assert_eq!(len(0, 22), 9);
        assert_eq!(len(1, 17), 12);
        assert_eq!(len(7, 1), 8);
        assert_eq!(len(8, 1), 8);
        assert_eq!(len(10, 2), 12);
        assert_eq!(len(14, 1), 9);
        assert_eq!(len(5, 0), 12);
        assert_eq!(len(1, 0), 11);
        assert_eq!(lookup(0b0110_0000_0000_0000), Sym::EndOfBlock);
        assert_eq!(
            lookup(0b1111_1110_0010_1111),
            Sym::Coef {
                len: 16,
                run: 0,
                amp: 23,
                neg: true
            }
        );
        assert_eq!(
            lookup(0b1111_1101_1110_1000),
            Sym::Coef {
                len: 13,
                run: 61,
                amp: 0,
                neg: false
            }
        );
    }
}
