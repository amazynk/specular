//! MSB-first bit reading and the Huffman tables of T.88 Annex B (standard tables B.1–B.15
//! and table segments).

use crate::{Error, Result};

/// MSB-first bit reader over a segment's data. Reading past the end yields zero bits and
/// counts the overrun.
pub(crate) struct Bits<'a> {
    pub(crate) data: &'a [u8],
    /// Position in bits.
    pos: usize,
}

impl<'a> Bits<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Bits<'a> {
        Bits { data, pos: 0 }
    }

    #[inline]
    pub(crate) fn bit(&mut self) -> u32 {
        let b = self.data.get(self.pos >> 3).copied().unwrap_or(0);
        let v = (b >> (7 - (self.pos & 7))) & 1;
        self.pos += 1;
        v as u32
    }

    pub(crate) fn bits(&mut self, n: u32) -> u64 {
        let mut v = 0u64;
        for _ in 0..n {
            v = (v << 1) | self.bit() as u64;
        }
        v
    }

    pub(crate) fn align(&mut self) {
        self.pos = self.pos.next_multiple_of(8);
    }

    /// Byte position (after [`Bits::align`]).
    pub(crate) fn byte_pos(&self) -> usize {
        self.pos.div_ceil(8)
    }

    pub(crate) fn skip_bytes(&mut self, n: usize) {
        self.pos = self.byte_pos().saturating_add(n).saturating_mul(8);
    }

    /// The rest of the data from the current byte.
    pub(crate) fn rest(&self) -> &'a [u8] {
        self.data.get(self.byte_pos()..).unwrap_or(&[])
    }

    pub(crate) fn exhausted(&self) -> bool {
        self.pos > self.data.len() * 8 + 2048
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Normal,
    Lower,
    Upper,
    Oob,
}

#[derive(Clone, Copy, Debug)]
struct Line {
    preflen: u8,
    rangelen: u8,
    low: i64,
    kind: Kind,
}

/// A Huffman table with its canonical codes assigned (B.3).
#[derive(Clone, Debug)]
pub(crate) struct Table {
    /// Per prefix length: the first code and the lines with that length, in order.
    first: Vec<u32>,
    by_len: Vec<Vec<Line>>,
}

impl Table {
    fn build(lines: &[Line]) -> Result<Table> {
        let max = lines.iter().map(|l| l.preflen).max().unwrap_or(0) as usize;
        if max > 32 {
            return Err(Error::Malformed(
                "Huffman prefix longer than 32 bits".into(),
            ));
        }
        let mut by_len = vec![Vec::new(); max + 1];
        for l in lines.iter().filter(|l| l.preflen > 0) {
            by_len[l.preflen as usize].push(*l);
        }
        let mut first = vec![0u32; max + 1];
        let mut code: u64 = 0;
        for len in 1..=max {
            code = (code + by_len[len - 1].len() as u64) << 1;
            if code + by_len[len].len() as u64 > 1u64 << len {
                return Err(Error::Malformed("Huffman table is over-full".into()));
            }
            first[len] = code as u32;
        }
        Ok(Table { first, by_len })
    }

    /// Decodes a value; `None` is OOB.
    pub(crate) fn decode(&self, b: &mut Bits) -> Result<Option<i64>> {
        let mut code = 0u32;
        for len in 1..self.first.len() {
            code = (code << 1) | b.bit();
            let i = code.wrapping_sub(self.first[len]) as usize;
            if code >= self.first[len] && i < self.by_len[len].len() {
                let l = self.by_len[len][i];
                return Ok(match l.kind {
                    Kind::Oob => None,
                    Kind::Normal => Some(l.low + b.bits(l.rangelen as u32) as i64),
                    Kind::Lower => Some(l.low - b.bits(32) as i64),
                    Kind::Upper => Some(l.low + b.bits(32) as i64),
                });
            }
        }
        Err(Error::Malformed("invalid Huffman code".into()))
    }

    /// A value that must not be OOB.
    pub(crate) fn value(&self, b: &mut Bits) -> Result<i64> {
        self.decode(b)?
            .ok_or_else(|| Error::Malformed("unexpected out-of-band Huffman value".into()))
    }

    /// A table of plain prefix codes with values 0..n (text region symbol IDs, run codes):
    /// `lens[i]` is the code length of value i (0: unused).
    pub(crate) fn from_lengths(lens: &[u8]) -> Result<Table> {
        let lines: Vec<Line> = lens
            .iter()
            .enumerate()
            .map(|(i, &l)| Line {
                preflen: l,
                rangelen: 0,
                low: i as i64,
                kind: Kind::Normal,
            })
            .collect();
        Table::build(&lines)
    }

    /// A table segment (7.4.13 / B.2).
    pub(crate) fn parse(data: &[u8]) -> Result<Table> {
        if data.len() < 9 {
            return Err(Error::Truncated("Huffman table segment".into()));
        }
        let flags = data[0];
        let oob = flags & 1 != 0;
        let ps = ((flags >> 1) & 7) as u32 + 1;
        let rs = ((flags >> 4) & 7) as u32 + 1;
        let low = i32::from_be_bytes(data[1..5].try_into().unwrap_or_default()) as i64;
        let high = i32::from_be_bytes(data[5..9].try_into().unwrap_or_default()) as i64;
        if high <= low {
            return Err(Error::Malformed("Huffman table range is empty".into()));
        }
        let mut b = Bits::new(&data[9..]);
        let mut lines = Vec::new();
        let mut cur = low;
        while cur < high {
            if lines.len() > 1 << 16 || b.exhausted() {
                return Err(Error::Malformed("Huffman table segment is too long".into()));
            }
            let preflen = b.bits(ps) as u8;
            let rangelen = b.bits(rs) as u8;
            if rangelen > 32 {
                return Err(Error::Malformed("Huffman range too wide".into()));
            }
            lines.push(Line {
                preflen,
                rangelen,
                low: cur,
                kind: Kind::Normal,
            });
            cur += 1i64 << rangelen;
        }
        let pl = b.bits(ps) as u8;
        lines.push(Line {
            preflen: pl,
            rangelen: 32,
            low: low - 1,
            kind: Kind::Lower,
        });
        let pu = b.bits(ps) as u8;
        lines.push(Line {
            preflen: pu,
            rangelen: 32,
            low: high,
            kind: Kind::Upper,
        });
        if oob {
            let po = b.bits(ps) as u8;
            lines.push(Line {
                preflen: po,
                rangelen: 0,
                low: 0,
                kind: Kind::Oob,
            });
        }
        Table::build(&lines)
    }

    /// Standard table B.`n` (1..=15).
    pub(crate) fn standard(n: usize) -> Table {
        let rows = STANDARD[n - 1];
        let lines: Vec<Line> = rows
            .iter()
            .map(|&(low, preflen, rangelen, k)| Line {
                preflen,
                rangelen,
                low,
                kind: match k {
                    L => Kind::Lower,
                    U => Kind::Upper,
                    O => Kind::Oob,
                    _ => Kind::Normal,
                },
            })
            .collect();
        Table::build(&lines).unwrap_or(Table {
            first: Vec::new(),
            by_len: Vec::new(),
        })
    }
}

const N: u8 = 0;
const L: u8 = 1;
const U: u8 = 2;
const O: u8 = 3;

type Row = (i64, u8, u8, u8);

/// (RANGELOW, PREFLEN, RANGELEN, kind) of tables B.1–B.15, lines in the order of Annex B.
const STANDARD: [&[Row]; 15] = [
    // B.1
    &[
        (0, 1, 4, N),
        (16, 2, 8, N),
        (272, 3, 16, N),
        (65808, 3, 32, U),
    ],
    // B.2
    &[
        (0, 1, 0, N),
        (1, 2, 0, N),
        (2, 3, 0, N),
        (3, 4, 3, N),
        (11, 5, 6, N),
        (75, 6, 32, U),
        (0, 6, 0, O),
    ],
    // B.3
    &[
        (-256, 8, 8, N),
        (0, 1, 0, N),
        (1, 2, 0, N),
        (2, 3, 0, N),
        (3, 4, 3, N),
        (11, 5, 6, N),
        (-257, 8, 32, L),
        (75, 7, 32, U),
        (0, 6, 0, O),
    ],
    // B.4
    &[
        (1, 1, 0, N),
        (2, 2, 0, N),
        (3, 3, 0, N),
        (4, 4, 3, N),
        (12, 5, 6, N),
        (76, 5, 32, U),
    ],
    // B.5
    &[
        (-255, 7, 8, N),
        (1, 1, 0, N),
        (2, 2, 0, N),
        (3, 3, 0, N),
        (4, 4, 3, N),
        (12, 5, 6, N),
        (-256, 7, 32, L),
        (76, 6, 32, U),
    ],
    // B.6
    &[
        (-2048, 5, 10, N),
        (-1024, 4, 9, N),
        (-512, 4, 8, N),
        (-256, 4, 7, N),
        (-128, 5, 6, N),
        (-64, 5, 5, N),
        (-32, 4, 5, N),
        (0, 2, 7, N),
        (128, 3, 7, N),
        (256, 3, 8, N),
        (512, 4, 9, N),
        (1024, 4, 10, N),
        (-2049, 6, 32, L),
        (2048, 6, 32, U),
    ],
    // B.7
    &[
        (-1024, 4, 9, N),
        (-512, 3, 8, N),
        (-256, 4, 7, N),
        (-128, 5, 6, N),
        (-64, 5, 5, N),
        (-32, 4, 5, N),
        (0, 4, 5, N),
        (32, 5, 5, N),
        (64, 5, 6, N),
        (128, 4, 7, N),
        (256, 3, 8, N),
        (512, 3, 9, N),
        (1024, 3, 10, N),
        (-1025, 5, 32, L),
        (2048, 5, 32, U),
    ],
    // B.8
    &[
        (-15, 8, 3, N),
        (-7, 9, 1, N),
        (-5, 8, 1, N),
        (-3, 9, 0, N),
        (-2, 7, 0, N),
        (-1, 4, 0, N),
        (0, 2, 1, N),
        (2, 5, 0, N),
        (3, 6, 0, N),
        (4, 3, 4, N),
        (20, 6, 1, N),
        (22, 4, 4, N),
        (38, 4, 5, N),
        (70, 5, 6, N),
        (134, 5, 7, N),
        (262, 6, 7, N),
        (390, 7, 8, N),
        (646, 6, 10, N),
        (-16, 9, 32, L),
        (1670, 9, 32, U),
        (0, 2, 0, O),
    ],
    // B.9
    &[
        (-31, 8, 4, N),
        (-15, 9, 2, N),
        (-11, 8, 2, N),
        (-7, 9, 1, N),
        (-5, 7, 1, N),
        (-3, 4, 1, N),
        (-1, 3, 1, N),
        (1, 3, 1, N),
        (3, 5, 1, N),
        (5, 6, 1, N),
        (7, 3, 5, N),
        (39, 6, 2, N),
        (43, 4, 5, N),
        (75, 4, 6, N),
        (139, 5, 7, N),
        (267, 5, 8, N),
        (523, 6, 8, N),
        (779, 7, 9, N),
        (1291, 6, 11, N),
        (-32, 9, 32, L),
        (3339, 9, 32, U),
        (0, 2, 0, O),
    ],
    // B.10
    &[
        (-21, 7, 4, N),
        (-5, 8, 0, N),
        (-4, 7, 0, N),
        (-3, 5, 0, N),
        (-2, 2, 2, N),
        (2, 5, 0, N),
        (3, 6, 0, N),
        (4, 7, 0, N),
        (5, 8, 0, N),
        (6, 2, 6, N),
        (70, 5, 5, N),
        (102, 6, 5, N),
        (134, 6, 6, N),
        (198, 6, 7, N),
        (326, 6, 8, N),
        (582, 6, 9, N),
        (1094, 6, 10, N),
        (2118, 7, 11, N),
        (-22, 8, 32, L),
        (4166, 8, 32, U),
        (0, 2, 0, O),
    ],
    // B.11
    &[
        (1, 1, 0, N),
        (2, 2, 1, N),
        (4, 4, 0, N),
        (5, 4, 1, N),
        (7, 5, 1, N),
        (9, 5, 2, N),
        (13, 6, 2, N),
        (17, 7, 2, N),
        (21, 7, 3, N),
        (29, 7, 4, N),
        (45, 7, 5, N),
        (77, 7, 6, N),
        (141, 7, 32, U),
    ],
    // B.12
    &[
        (1, 1, 0, N),
        (2, 2, 0, N),
        (3, 3, 1, N),
        (5, 5, 0, N),
        (6, 5, 1, N),
        (8, 6, 1, N),
        (10, 7, 0, N),
        (11, 7, 1, N),
        (13, 7, 2, N),
        (17, 7, 3, N),
        (25, 7, 4, N),
        (41, 8, 5, N),
        (73, 8, 32, U),
    ],
    // B.13
    &[
        (1, 1, 0, N),
        (2, 3, 0, N),
        (3, 4, 0, N),
        (4, 5, 0, N),
        (5, 4, 1, N),
        (7, 3, 3, N),
        (15, 6, 1, N),
        (17, 6, 2, N),
        (21, 6, 3, N),
        (29, 6, 4, N),
        (45, 6, 5, N),
        (77, 7, 6, N),
        (141, 7, 32, U),
    ],
    // B.14
    &[
        (-2, 3, 0, N),
        (-1, 3, 0, N),
        (0, 1, 0, N),
        (1, 3, 0, N),
        (2, 3, 0, N),
    ],
    // B.15
    &[
        (-24, 7, 4, N),
        (-8, 6, 2, N),
        (-4, 5, 1, N),
        (-2, 4, 0, N),
        (-1, 3, 0, N),
        (0, 1, 0, N),
        (1, 3, 0, N),
        (2, 4, 0, N),
        (3, 5, 1, N),
        (5, 6, 2, N),
        (9, 7, 4, N),
        (-25, 7, 32, L),
        (25, 7, 32, U),
    ],
];

#[cfg(test)]
mod tests {
    use super::*;

    /// The prefix codes Annex B prints for some lines, against the canonical assignment.
    #[test]
    fn canonical_codes_match_annex_b() {
        // (table, line index in STANDARD order, code)
        let cases: &[(usize, usize, u32)] = &[
            (1, 3, 0b111),
            (2, 6, 0b111111),
            (3, 0, 0b11111110),
            (3, 6, 0b11111111),
            (3, 8, 0b111110),
            (5, 6, 0b1111111),
            (6, 0, 0b11100),
            (6, 7, 0b00),
            (6, 12, 0b111110),
            (7, 1, 0b000),
            (7, 14, 0b11111),
            (8, 20, 0b01),
            (8, 18, 0b111111110),
            (9, 21, 0b00),
            (10, 20, 0b10),
            (10, 17, 0b1111101),
            (11, 12, 0b1111111),
            (12, 12, 0b11111111),
            (13, 1, 0b100),
            (13, 5, 0b101),
            (14, 0, 0b100),
            (15, 11, 0b1111110),
        ];
        for &(t, i, want) in cases {
            let row = STANDARD[t - 1][i];
            // Encode `want` then the range bits as zeros and decode it back.
            let len = row.1 as u32;
            let total = len + row.2 as u32;
            let v = (want as u64) << (64 - len);
            let bytes = v.to_be_bytes();
            let mut b = Bits::new(&bytes);
            let got = Table::standard(t).decode(&mut b).unwrap();
            let expect = match row.3 {
                O => None,
                _ => Some(row.0),
            };
            assert_eq!(got, expect, "B.{t} line {i}");
            assert_eq!(b.byte_pos(), (total as usize).div_ceil(8), "B.{t} line {i}");
        }
    }

    #[test]
    fn custom_table_round_trip() {
        // HTOOB 0, HTPS 2 (flags bits 1-3 = 1), HTRS 2 (bits 4-6 = 1): flags 0b0001_0010.
        // HTLOW 0, HTHIGH 8: lines [0..4) (prefix 1, range 2) and [4..8) (prefix 2, range 2),
        // lower prefix 3, upper prefix 3.
        let mut d = vec![0b0001_0010u8, 0, 0, 0, 0, 0, 0, 0, 8];
        // bits: 01 10, 10 10, 11, 11 -> 0110 1010 1111
        d.extend([0b0110_1010, 0b1111_0000]);
        let t = Table::parse(&d).unwrap();
        // value 6: prefix "10" + range 2 bits "10".
        let bytes = [0b1010_0000u8];
        assert_eq!(t.decode(&mut Bits::new(&bytes)).unwrap(), Some(6));
    }
}
