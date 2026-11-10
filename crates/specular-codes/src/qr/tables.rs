//! QR code structure (ISO/IEC 18004:2015): error correction blocks per version and level
//! (Table 9, transcribed), alignment pattern positions, raw module counts (computed).

/// Error correction level, in the order of the format information's two bits' meaning.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    L,
    M,
    Q,
    H,
}

impl Level {
    /// From the format information's two bits (01 L, 00 M, 11 Q, 10 H).
    pub(crate) fn from_bits(b: u8) -> Level {
        match b & 3 {
            1 => Level::L,
            0 => Level::M,
            3 => Level::Q,
            _ => Level::H,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Level::L => "L",
            Level::M => "M",
            Level::Q => "Q",
            Level::H => "H",
        }
    }

    fn index(self) -> usize {
        match self {
            Level::L => 0,
            Level::M => 1,
            Level::Q => 2,
            Level::H => 3,
        }
    }
}

/// Per version (1..=40) and level (L, M, Q, H): check codewords per block, then the
/// number of blocks and data codewords per block of the first group, then of the second.
#[rustfmt::skip]
const BLOCKS: [[[u8; 5]; 4]; 40] = [
    [[7, 1, 19, 0, 0], [10, 1, 16, 0, 0], [13, 1, 13, 0, 0], [17, 1, 9, 0, 0]],
    [[10, 1, 34, 0, 0], [16, 1, 28, 0, 0], [22, 1, 22, 0, 0], [28, 1, 16, 0, 0]],
    [[15, 1, 55, 0, 0], [26, 1, 44, 0, 0], [18, 2, 17, 0, 0], [22, 2, 13, 0, 0]],
    [[20, 1, 80, 0, 0], [18, 2, 32, 0, 0], [26, 2, 24, 0, 0], [16, 4, 9, 0, 0]],
    [[26, 1, 108, 0, 0], [24, 2, 43, 0, 0], [18, 2, 15, 2, 16], [22, 2, 11, 2, 12]],
    [[18, 2, 68, 0, 0], [16, 4, 27, 0, 0], [24, 4, 19, 0, 0], [28, 4, 15, 0, 0]],
    [[20, 2, 78, 0, 0], [18, 4, 31, 0, 0], [18, 2, 14, 4, 15], [26, 4, 13, 1, 14]],
    [[24, 2, 97, 0, 0], [22, 2, 38, 2, 39], [22, 4, 18, 2, 19], [26, 4, 14, 2, 15]],
    [[30, 2, 116, 0, 0], [22, 3, 36, 2, 37], [20, 4, 16, 4, 17], [24, 4, 12, 4, 13]],
    [[18, 2, 68, 2, 69], [26, 4, 43, 1, 44], [24, 6, 19, 2, 20], [28, 6, 15, 2, 16]],
    [[20, 4, 81, 0, 0], [30, 1, 50, 4, 51], [28, 4, 22, 4, 23], [24, 3, 12, 8, 13]],
    [[24, 2, 92, 2, 93], [22, 6, 36, 2, 37], [26, 4, 20, 6, 21], [28, 7, 14, 4, 15]],
    [[26, 4, 107, 0, 0], [22, 8, 37, 1, 38], [24, 8, 20, 4, 21], [22, 12, 11, 4, 12]],
    [[30, 3, 115, 1, 116], [24, 4, 40, 5, 41], [20, 11, 16, 5, 17], [24, 11, 12, 5, 13]],
    [[22, 5, 87, 1, 88], [24, 5, 41, 5, 42], [30, 5, 24, 7, 25], [24, 11, 12, 7, 13]],
    [[24, 5, 98, 1, 99], [28, 7, 45, 3, 46], [24, 15, 19, 2, 20], [30, 3, 15, 13, 16]],
    [[28, 1, 107, 5, 108], [28, 10, 46, 1, 47], [28, 1, 22, 15, 23], [28, 2, 14, 17, 15]],
    [[30, 5, 120, 1, 121], [26, 9, 43, 4, 44], [28, 17, 22, 1, 23], [28, 2, 14, 19, 15]],
    [[28, 3, 113, 4, 114], [26, 3, 44, 11, 45], [26, 17, 21, 4, 22], [26, 9, 13, 16, 14]],
    [[28, 3, 107, 5, 108], [26, 3, 41, 13, 42], [30, 15, 24, 5, 25], [28, 15, 15, 10, 16]],
    [[28, 4, 116, 4, 117], [26, 17, 42, 0, 0], [28, 17, 22, 6, 23], [30, 19, 16, 6, 17]],
    [[28, 2, 111, 7, 112], [28, 17, 46, 0, 0], [30, 7, 24, 16, 25], [24, 34, 13, 0, 0]],
    [[30, 4, 121, 5, 122], [28, 4, 47, 14, 48], [30, 11, 24, 14, 25], [30, 16, 15, 14, 16]],
    [[30, 6, 117, 4, 118], [28, 6, 45, 14, 46], [30, 11, 24, 16, 25], [30, 30, 16, 2, 17]],
    [[26, 8, 106, 4, 107], [28, 8, 47, 13, 48], [30, 7, 24, 22, 25], [30, 22, 15, 13, 16]],
    [[28, 10, 114, 2, 115], [28, 19, 46, 4, 47], [28, 28, 22, 6, 23], [30, 33, 16, 4, 17]],
    [[30, 8, 122, 4, 123], [28, 22, 45, 3, 46], [30, 8, 23, 26, 24], [30, 12, 15, 28, 16]],
    [[30, 3, 117, 10, 118], [28, 3, 45, 23, 46], [30, 4, 24, 31, 25], [30, 11, 15, 31, 16]],
    [[30, 7, 116, 7, 117], [28, 21, 45, 7, 46], [30, 1, 23, 37, 24], [30, 19, 15, 26, 16]],
    [[30, 5, 115, 10, 116], [28, 19, 47, 10, 48], [30, 15, 24, 25, 25], [30, 23, 15, 25, 16]],
    [[30, 13, 115, 3, 116], [28, 2, 46, 29, 47], [30, 42, 24, 1, 25], [30, 23, 15, 28, 16]],
    [[30, 17, 115, 0, 0], [28, 10, 46, 23, 47], [30, 10, 24, 35, 25], [30, 19, 15, 35, 16]],
    [[30, 17, 115, 1, 116], [28, 14, 46, 21, 47], [30, 29, 24, 19, 25], [30, 11, 15, 46, 16]],
    [[30, 13, 115, 6, 116], [28, 14, 46, 23, 47], [30, 44, 24, 7, 25], [30, 59, 16, 1, 17]],
    [[30, 12, 121, 7, 122], [28, 12, 47, 26, 48], [30, 39, 24, 14, 25], [30, 22, 15, 41, 16]],
    [[30, 6, 121, 14, 122], [28, 6, 47, 34, 48], [30, 46, 24, 10, 25], [30, 2, 15, 64, 16]],
    [[30, 17, 122, 4, 123], [28, 29, 46, 14, 47], [30, 49, 24, 10, 25], [30, 24, 15, 46, 16]],
    [[30, 4, 122, 18, 123], [28, 13, 46, 32, 47], [30, 48, 24, 14, 25], [30, 42, 15, 32, 16]],
    [[30, 20, 117, 4, 118], [28, 40, 47, 7, 48], [30, 43, 24, 22, 25], [30, 10, 15, 67, 16]],
    [[30, 19, 118, 6, 119], [28, 18, 47, 31, 48], [30, 34, 24, 34, 25], [30, 20, 15, 61, 16]],
];

/// One block's sizes: data codewords and check codewords.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Block {
    pub data: usize,
    pub ecc: usize,
}

/// The blocks of `version` at `level`, the shorter first.
pub(crate) fn blocks(version: usize, level: Level) -> Vec<Block> {
    let [ecc, n1, d1, n2, d2] = BLOCKS[version - 1][level.index()];
    let ecc = usize::from(ecc);
    let mut v = vec![
        Block {
            data: usize::from(d1),
            ecc
        };
        usize::from(n1)
    ];
    v.extend(std::iter::repeat_n(
        Block {
            data: usize::from(d2),
            ecc,
        },
        usize::from(n2),
    ));
    v
}

/// Modules per side.
pub(crate) fn size(version: usize) -> usize {
    17 + 4 * version
}

/// Modules that carry data and check codewords (and remainder bits).
pub(crate) fn raw_modules(version: usize) -> usize {
    let v = version;
    let mut n = (16 * v + 128) * v + 64;
    if v >= 2 {
        let a = v / 7 + 2;
        n -= (25 * a - 10) * a - 55;
        if v >= 7 {
            n -= 36;
        }
    }
    n
}

/// Centre coordinates (on both axes) of the alignment patterns.
pub(crate) fn alignment(version: usize) -> Vec<usize> {
    if version == 1 {
        return Vec::new();
    }
    let n = version / 7 + 2;
    let step = if version == 32 {
        26
    } else {
        (version * 4 + n * 2 + 1) / (n * 2 - 2) * 2
    };
    let last = size(version) - 7;
    let mut v: Vec<usize> = (0..n - 1).map(|i| last - i * step).collect();
    v.push(6);
    v.reverse();
    v
}

/// Character count indicator lengths for numeric, alphanumeric, byte and kanji modes.
pub(crate) fn count_bits(version: usize) -> [u32; 4] {
    match version {
        1..=9 => [10, 9, 8, 8],
        10..=26 => [12, 11, 16, 10],
        _ => [14, 13, 16, 12],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_fill_every_symbol() {
        for v in 1..=40 {
            let total = raw_modules(v) / 8;
            for level in [Level::L, Level::M, Level::Q, Level::H] {
                let sum: usize = blocks(v, level).iter().map(|b| b.data + b.ecc).sum();
                assert_eq!(sum, total, "version {v} {level:?}");
            }
        }
    }

    #[test]
    fn alignment_positions() {
        assert_eq!(alignment(2), vec![6, 18]);
        assert_eq!(alignment(7), vec![6, 22, 38]);
        assert_eq!(alignment(32), vec![6, 34, 60, 86, 112, 138]);
        assert_eq!(alignment(40), vec![6, 30, 58, 86, 114, 142, 170]);
        assert_eq!(raw_modules(1), 208);
        assert_eq!(raw_modules(40), 29648);
    }
}
