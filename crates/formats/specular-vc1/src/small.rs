//! The short code tables of SMPTE ST 421:2013, typed in from the standard's text (table numbers
//! as given there). The long tables are generated into `tables.rs`.

use std::sync::LazyLock;

use crate::vlc::Vlc;

/// Table 36: PQINDEX to PQUANT with the implicit quantizer (index 0 unused).
pub const PQINDEX_TO_PQUANT: [u8; 32] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
    24, 25, 27, 29, 31,
];

/// Transform types.
pub const TT_8X8: u8 = 0;
pub const TT_8X4: u8 = 1;
pub const TT_4X8: u8 = 2;
pub const TT_4X4: u8 = 3;

/// Sub-block patterns of 8x4 / 4x8 transforms: bit 1 the top (left) half, bit 0 the bottom
/// (right) half.
const BOTH: u8 = 3;
const FIRST: u8 = 2;
const SECOND: u8 = 1;

/// A TTMB symbol: transform type, macroblock-level signalling, sub-block pattern.
fn ttmb_sym(tt: u8, mb: bool, pat: u8) -> u16 {
    u16::from(tt) | (u16::from(mb) << 2) | (u16::from(pat) << 3)
}

/// Splits a TTMB / TTBLK symbol.
pub fn tt_parts(sym: u16) -> (u8, bool, u8) {
    ((sym & 3) as u8, sym & 4 != 0, (sym >> 3) as u8 & 3)
}

fn bin(s: &str) -> (u32, u8) {
    (u32::from_str_radix(s, 2).unwrap_or(0), s.len() as u8)
}

fn table(rows: &[(&str, u16)]) -> Vlc {
    let list: Vec<(u32, u8, u16)> = rows
        .iter()
        .map(|&(s, v)| {
            let (c, l) = bin(s);
            (c, l, v)
        })
        .collect();
    Vlc::with_symbols(&list)
}

/// Tables 54–56: TTMB, by PQUANT < 5, 5..=12, >= 13.
pub static TTMB: LazyLock<[Vlc; 3]> = LazyLock::new(|| {
    let s = ttmb_sym;
    [
        table(&[
            ("11", s(TT_8X8, false, 0)),
            ("101110", s(TT_8X4, false, SECOND)),
            ("1011111", s(TT_8X4, false, FIRST)),
            ("00", s(TT_8X4, false, BOTH)),
            ("10110", s(TT_4X8, false, SECOND)),
            ("10101", s(TT_4X8, false, FIRST)),
            ("01", s(TT_4X8, false, BOTH)),
            ("100", s(TT_4X4, false, 0)),
            ("10100", s(TT_8X8, true, 0)),
            ("1011110001", s(TT_8X4, true, SECOND)),
            ("101111001", s(TT_8X4, true, FIRST)),
            ("101111011", s(TT_8X4, true, BOTH)),
            ("101111000000", s(TT_4X8, true, SECOND)),
            ("101111000001", s(TT_4X8, true, FIRST)),
            ("10111100001", s(TT_4X8, true, BOTH)),
            ("101111010", s(TT_4X4, true, 0)),
        ]),
        table(&[
            ("110", s(TT_8X8, false, 0)),
            ("0110", s(TT_8X4, false, SECOND)),
            ("0011", s(TT_8X4, false, FIRST)),
            ("0111", s(TT_8X4, false, BOTH)),
            ("1111", s(TT_4X8, false, SECOND)),
            ("1110", s(TT_4X8, false, FIRST)),
            ("000", s(TT_4X8, false, BOTH)),
            ("010", s(TT_4X4, false, 0)),
            ("10", s(TT_8X8, true, 0)),
            ("0010100", s(TT_8X4, true, SECOND)),
            ("0010001", s(TT_8X4, true, FIRST)),
            ("001011", s(TT_8X4, true, BOTH)),
            ("001001", s(TT_4X8, true, SECOND)),
            ("00100001", s(TT_4X8, true, FIRST)),
            ("0010101", s(TT_4X8, true, BOTH)),
            ("00100000", s(TT_4X4, true, 0)),
        ]),
        table(&[
            ("110", s(TT_8X8, false, 0)),
            ("000", s(TT_8X4, false, SECOND)),
            ("1110", s(TT_8X4, false, FIRST)),
            ("00101", s(TT_8X4, false, BOTH)),
            ("010", s(TT_4X8, false, SECOND)),
            ("011", s(TT_4X8, false, FIRST)),
            ("0011", s(TT_4X8, false, BOTH)),
            ("1111", s(TT_4X4, false, 0)),
            ("10", s(TT_8X8, true, 0)),
            ("0010000001", s(TT_8X4, true, SECOND)),
            ("00100001", s(TT_8X4, true, FIRST)),
            ("001001", s(TT_8X4, true, BOTH)),
            ("00100000001", s(TT_4X8, true, SECOND)),
            ("001000001", s(TT_4X8, true, FIRST)),
            ("0010001", s(TT_4X8, true, BOTH)),
            ("00100000000", s(TT_4X4, true, 0)),
        ]),
    ]
});

/// Tables 62–64: TTBLK (symbols as TTMB, block level).
pub static TTBLK: LazyLock<[Vlc; 3]> = LazyLock::new(|| {
    let s = |t, p| ttmb_sym(t, false, p);
    [
        table(&[
            ("00", s(TT_8X4, BOTH)),
            ("01", s(TT_4X8, BOTH)),
            ("11", s(TT_8X8, 0)),
            ("101", s(TT_4X4, 0)),
            ("10000", s(TT_8X4, FIRST)),
            ("10001", s(TT_8X4, SECOND)),
            ("10010", s(TT_4X8, SECOND)),
            ("10011", s(TT_4X8, FIRST)),
        ]),
        table(&[
            ("11", s(TT_8X8, 0)),
            ("000", s(TT_4X8, SECOND)),
            ("001", s(TT_4X8, FIRST)),
            ("010", s(TT_4X4, 0)),
            ("011", s(TT_8X4, BOTH)),
            ("101", s(TT_4X8, BOTH)),
            ("1000", s(TT_8X4, SECOND)),
            ("1001", s(TT_8X4, FIRST)),
        ]),
        table(&[
            ("01", s(TT_8X8, 0)),
            ("000", s(TT_4X8, BOTH)),
            ("001", s(TT_4X4, 0)),
            ("100", s(TT_8X4, SECOND)),
            ("110", s(TT_4X8, SECOND)),
            ("111", s(TT_4X8, FIRST)),
            ("1010", s(TT_8X4, BOTH)),
            ("1011", s(TT_8X4, FIRST)),
        ]),
    ]
});

/// Tables 65–67: SUBBLKPAT of 4x4 transforms (8·SB0 + 4·SB1 + 2·SB2 + SB3).
pub static SUBBLKPAT: LazyLock<[Vlc; 3]> = LazyLock::new(|| {
    [
        table(&[
            ("1", 15),
            ("0000", 11),
            ("0001", 13),
            ("0010", 7),
            ("00110", 12),
            ("00111", 3),
            ("01000", 10),
            ("01001", 5),
            ("01010", 8),
            ("01011", 4),
            ("01100", 2),
            ("01110", 1),
            ("01111", 14),
            ("011010", 6),
            ("011011", 9),
        ]),
        table(&[
            ("01", 15),
            ("000", 2),
            ("0011", 12),
            ("1000", 3),
            ("1001", 10),
            ("1010", 5),
            ("1101", 8),
            ("1110", 1),
            ("1111", 4),
            ("00100", 6),
            ("00101", 9),
            ("10110", 14),
            ("10111", 7),
            ("11000", 13),
            ("11001", 11),
        ]),
        table(&[
            ("010", 4),
            ("011", 8),
            ("101", 1),
            ("110", 2),
            ("0001", 12),
            ("0010", 3),
            ("0011", 10),
            ("1000", 5),
            ("1111", 15),
            ("00000", 6),
            ("00001", 9),
            ("10010", 14),
            ("10011", 13),
            ("11100", 7),
            ("11101", 11),
        ]),
    ]
});

/// The table index for TTMB, TTBLK and SUBBLKPAT by PQUANT.
pub fn rate_index(pquant: u8) -> usize {
    match pquant {
        0..=4 => 0,
        5..=12 => 1,
        _ => 2,
    }
}

/// Table 69: IMODE.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Imode {
    Raw,
    Norm2,
    Diff2,
    Norm6,
    Diff6,
    RowSkip,
    ColSkip,
}

pub static IMODE: LazyLock<Vlc> = LazyLock::new(|| {
    table(&[
        ("10", 1),
        ("11", 3),
        ("010", 5),
        ("011", 6),
        ("001", 2),
        ("0001", 4),
        ("0000", 0),
    ])
});

pub fn imode(sym: u16) -> Imode {
    match sym {
        1 => Imode::Norm2,
        2 => Imode::Diff2,
        3 => Imode::Norm6,
        4 => Imode::Diff6,
        5 => Imode::RowSkip,
        6 => Imode::ColSkip,
        _ => Imode::Raw,
    }
}

/// Table 80: Norm-2 symbol pairs (bit 1: symbol 2N, bit 0: symbol 2N + 1).
pub static NORM2: LazyLock<Vlc> =
    LazyLock::new(|| table(&[("0", 0), ("100", 2), ("101", 1), ("11", 3)]));

/// Tables 59 and 60: ESCLVLSZ, the conservative and the efficient table.
pub static ESCLVLSZ: LazyLock<[Vlc; 2]> = LazyLock::new(|| {
    [
        table(&[
            ("001", 1),
            ("010", 2),
            ("011", 3),
            ("100", 4),
            ("101", 5),
            ("110", 6),
            ("111", 7),
            ("00000", 8),
            ("00001", 9),
            ("00010", 10),
            ("00011", 11),
        ]),
        table(&[
            ("1", 2),
            ("01", 3),
            ("001", 4),
            ("0001", 5),
            ("00001", 6),
            ("000001", 7),
            ("000000", 8),
        ]),
    ]
});

/// Table 75: k_x, k_y, range_x, range_y by MVRANGE.
pub const MVRANGE: [(u32, u32, i32, i32); 4] = [
    (9, 8, 256, 128),
    (10, 9, 512, 256),
    (12, 10, 2048, 512),
    (13, 11, 4096, 1024),
];

/// Figure 50.
pub const MV_SIZE: [u32; 6] = [0, 2, 3, 4, 5, 8];
pub const MV_OFFSET: [i32; 6] = [0, 1, 3, 7, 15, 31];

/// Figure 70: B fraction numerators and denominators: the short codes 000b..110b, then the
/// long ones 1110000b..1111101b.
pub const BFRACTION_SHORT: [(u8, u8); 7] = [(1, 2), (1, 3), (2, 3), (1, 4), (3, 4), (1, 5), (2, 5)];
pub const BFRACTION_LONG: [(u8, u8); 14] = [
    (3, 5),
    (4, 5),
    (1, 6),
    (5, 6),
    (1, 7),
    (2, 7),
    (3, 7),
    (4, 7),
    (5, 7),
    (6, 7),
    (1, 8),
    (3, 8),
    (5, 8),
    (7, 8),
];
pub const INVERSE: [i32; 8] = [256, 128, 85, 64, 51, 43, 37, 32];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_tables_build() {
        for t in TTMB.iter().chain(TTBLK.iter()).chain(SUBBLKPAT.iter()) {
            let _ = t;
        }
        let _ = (&*IMODE, &*NORM2, &*ESCLVLSZ);
    }
}
