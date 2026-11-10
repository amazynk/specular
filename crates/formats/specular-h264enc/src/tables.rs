//! Constant tables of ITU-T H.264: CAVLC code tables (9-5, 9-7, 9-8, 9-9, 9-10), the coded
//! block pattern mapping (9-4), quantisation and dequantisation factors (8.5.9), chroma QP
//! (8-15) and the deblocking thresholds (8-16, 8-17).
//!
//! Codes are `(length, value)` pairs; `(0, 0)` marks combinations that cannot occur.

/// coeff_token for 0 ≤ nC < 2, indexed `[TotalCoeff][TrailingOnes]`.
pub const COEFF_TOKEN_0: [[(u8, u16); 4]; 17] = [
    [(1, 1), (0, 0), (0, 0), (0, 0)],
    [(6, 5), (2, 1), (0, 0), (0, 0)],
    [(8, 7), (6, 4), (3, 1), (0, 0)],
    [(9, 7), (8, 6), (7, 5), (5, 3)],
    [(10, 7), (9, 6), (8, 5), (6, 3)],
    [(11, 7), (10, 6), (9, 5), (7, 4)],
    [(13, 15), (11, 6), (10, 5), (8, 4)],
    [(13, 11), (13, 14), (11, 5), (9, 4)],
    [(13, 8), (13, 10), (13, 13), (10, 4)],
    [(14, 15), (14, 14), (13, 9), (11, 4)],
    [(14, 11), (14, 10), (14, 13), (13, 12)],
    [(15, 15), (15, 14), (14, 9), (14, 12)],
    [(15, 11), (15, 10), (15, 13), (14, 8)],
    [(16, 15), (15, 1), (15, 9), (15, 12)],
    [(16, 11), (16, 14), (16, 13), (15, 8)],
    [(16, 7), (16, 10), (16, 9), (16, 12)],
    [(16, 4), (16, 6), (16, 5), (16, 8)],
];

/// coeff_token for 2 ≤ nC < 4.
pub const COEFF_TOKEN_2: [[(u8, u16); 4]; 17] = [
    [(2, 3), (0, 0), (0, 0), (0, 0)],
    [(6, 11), (2, 2), (0, 0), (0, 0)],
    [(6, 7), (5, 7), (3, 3), (0, 0)],
    [(7, 7), (6, 10), (6, 9), (4, 5)],
    [(8, 7), (6, 6), (6, 5), (4, 4)],
    [(8, 4), (7, 6), (7, 5), (5, 6)],
    [(9, 7), (8, 6), (8, 5), (6, 8)],
    [(11, 15), (9, 6), (9, 5), (6, 4)],
    [(11, 11), (11, 14), (11, 13), (7, 4)],
    [(12, 15), (11, 10), (11, 9), (9, 4)],
    [(12, 11), (12, 14), (12, 13), (11, 12)],
    [(12, 8), (12, 10), (12, 9), (11, 8)],
    [(13, 15), (13, 14), (13, 13), (12, 12)],
    [(13, 11), (13, 10), (13, 9), (13, 12)],
    [(13, 7), (14, 11), (13, 6), (13, 8)],
    [(14, 9), (14, 8), (14, 10), (13, 1)],
    [(14, 7), (14, 6), (14, 5), (14, 4)],
];

/// coeff_token for 4 ≤ nC < 8.
pub const COEFF_TOKEN_4: [[(u8, u16); 4]; 17] = [
    [(4, 15), (0, 0), (0, 0), (0, 0)],
    [(6, 15), (4, 14), (0, 0), (0, 0)],
    [(6, 11), (5, 15), (4, 13), (0, 0)],
    [(6, 8), (5, 12), (5, 14), (4, 12)],
    [(7, 15), (5, 10), (5, 11), (4, 11)],
    [(7, 11), (5, 8), (5, 9), (4, 10)],
    [(7, 9), (6, 14), (6, 13), (4, 9)],
    [(7, 8), (6, 10), (6, 9), (4, 8)],
    [(8, 15), (7, 14), (7, 13), (5, 13)],
    [(8, 11), (8, 14), (7, 10), (6, 12)],
    [(9, 15), (8, 10), (8, 13), (7, 12)],
    [(9, 11), (9, 14), (8, 9), (8, 12)],
    [(9, 8), (9, 10), (9, 13), (8, 8)],
    [(10, 13), (9, 7), (9, 9), (9, 12)],
    [(10, 9), (10, 12), (10, 11), (10, 10)],
    [(10, 5), (10, 8), (10, 7), (10, 6)],
    [(10, 1), (10, 4), (10, 3), (10, 2)],
];

/// coeff_token for chroma DC (nC = −1), TotalCoeff 0..=4.
pub const COEFF_TOKEN_CDC: [[(u8, u16); 4]; 5] = [
    [(2, 1), (0, 0), (0, 0), (0, 0)],
    [(6, 7), (1, 1), (0, 0), (0, 0)],
    [(6, 4), (6, 6), (3, 1), (0, 0)],
    [(6, 3), (7, 3), (7, 2), (6, 5)],
    [(6, 2), (8, 3), (8, 2), (7, 0)],
];

/// The coeff_token code for `nc` (−1 for chroma DC).
#[inline]
pub fn coeff_token(nc: i32, total: usize, t1: usize) -> (u32, u32) {
    let (l, c) = match nc {
        -1 => COEFF_TOKEN_CDC[total][t1],
        0 | 1 => COEFF_TOKEN_0[total][t1],
        2 | 3 => COEFF_TOKEN_2[total][t1],
        4..=7 => COEFF_TOKEN_4[total][t1],
        _ => {
            // Fixed 6-bit codes: TotalCoeff − 1 and TrailingOnes, or 000011 for none.
            if total == 0 {
                (6, 3)
            } else {
                (6, (((total - 1) << 2) | t1) as u16)
            }
        }
    };
    debug_assert!(l > 0);
    (u32::from(l), u32::from(c))
}

/// total_zeros for 4×4 blocks (`maxNumCoeff` 15 or 16), indexed `[TotalCoeff − 1][total_zeros]`,
/// as `(length, value)`.
pub const TOTAL_ZEROS: [&[(u8, u8)]; 15] = [
    &[
        (1, 1),
        (3, 3),
        (3, 2),
        (4, 3),
        (4, 2),
        (5, 3),
        (5, 2),
        (6, 3),
        (6, 2),
        (7, 3),
        (7, 2),
        (8, 3),
        (8, 2),
        (9, 3),
        (9, 2),
        (9, 1),
    ],
    &[
        (3, 7),
        (3, 6),
        (3, 5),
        (3, 4),
        (3, 3),
        (4, 5),
        (4, 4),
        (4, 3),
        (4, 2),
        (5, 3),
        (5, 2),
        (6, 3),
        (6, 2),
        (6, 1),
        (6, 0),
    ],
    &[
        (4, 5),
        (3, 7),
        (3, 6),
        (3, 5),
        (4, 4),
        (4, 3),
        (3, 4),
        (3, 3),
        (4, 2),
        (5, 3),
        (5, 2),
        (6, 1),
        (5, 1),
        (6, 0),
    ],
    &[
        (5, 3),
        (3, 7),
        (4, 5),
        (4, 4),
        (3, 6),
        (3, 5),
        (3, 4),
        (4, 3),
        (3, 3),
        (4, 2),
        (5, 2),
        (5, 1),
        (5, 0),
    ],
    &[
        (4, 5),
        (4, 4),
        (4, 3),
        (3, 7),
        (3, 6),
        (3, 5),
        (3, 4),
        (3, 3),
        (4, 2),
        (5, 1),
        (4, 1),
        (5, 0),
    ],
    &[
        (6, 1),
        (5, 1),
        (3, 7),
        (3, 6),
        (3, 5),
        (3, 4),
        (3, 3),
        (3, 2),
        (4, 1),
        (3, 1),
        (6, 0),
    ],
    &[
        (6, 1),
        (5, 1),
        (3, 5),
        (3, 4),
        (3, 3),
        (2, 3),
        (3, 2),
        (4, 1),
        (3, 1),
        (6, 0),
    ],
    &[
        (6, 1),
        (4, 1),
        (5, 1),
        (3, 3),
        (2, 3),
        (2, 2),
        (3, 2),
        (3, 1),
        (6, 0),
    ],
    &[
        (6, 1),
        (6, 0),
        (4, 1),
        (2, 3),
        (2, 2),
        (3, 1),
        (2, 1),
        (5, 1),
    ],
    &[(5, 1), (5, 0), (3, 1), (2, 3), (2, 2), (2, 1), (4, 1)],
    &[(4, 0), (4, 1), (3, 1), (3, 2), (1, 1), (3, 3)],
    &[(4, 0), (4, 1), (2, 1), (1, 1), (3, 1)],
    &[(3, 0), (3, 1), (1, 1), (2, 1)],
    &[(2, 0), (2, 1), (1, 1)],
    &[(1, 0), (1, 1)],
];

/// total_zeros for chroma DC, `[TotalCoeff − 1][total_zeros]`.
pub const TOTAL_ZEROS_CDC: [&[(u8, u8)]; 3] = [
    &[(1, 1), (2, 1), (3, 1), (3, 0)],
    &[(1, 1), (2, 1), (2, 0)],
    &[(1, 1), (1, 0)],
];

/// run_before, `[min(zerosLeft, 7) − 1][run_before]`.
pub const RUN_BEFORE: [&[(u8, u8)]; 7] = [
    &[(1, 1), (1, 0)],
    &[(1, 1), (2, 1), (2, 0)],
    &[(2, 3), (2, 2), (2, 1), (2, 0)],
    &[(2, 3), (2, 2), (2, 1), (3, 1), (3, 0)],
    &[(2, 3), (2, 2), (3, 3), (3, 2), (3, 1), (3, 0)],
    &[(2, 3), (3, 0), (3, 1), (3, 3), (3, 2), (3, 5), (3, 4)],
    &[
        (3, 7),
        (3, 6),
        (3, 5),
        (3, 4),
        (3, 3),
        (3, 2),
        (3, 1),
        (4, 1),
        (5, 1),
        (6, 1),
        (7, 1),
        (8, 1),
        (9, 1),
        (10, 1),
        (11, 1),
    ],
];

/// codeNum of coded_block_pattern for Intra_4x4 macroblocks, indexed by the pattern
/// (bits 0–3 luma 8×8 blocks, bits 4–5 chroma).
pub const CBP_INTRA: [u8; 48] = invert(&CBP_CODE_INTRA);
/// codeNum of coded_block_pattern for inter macroblocks.
pub const CBP_INTER: [u8; 48] = invert(&CBP_CODE_INTER);

/// Table 9-4 as the decoder reads it: codeNum → pattern.
const CBP_CODE_INTRA: [u8; 48] = [
    47, 31, 15, 0, 23, 27, 29, 30, 7, 11, 13, 14, 39, 43, 45, 46, 16, 3, 5, 10, 12, 19, 21, 26, 28,
    35, 37, 42, 44, 1, 2, 4, 8, 17, 18, 20, 24, 6, 9, 22, 25, 32, 33, 34, 36, 40, 38, 41,
];
const CBP_CODE_INTER: [u8; 48] = [
    0, 16, 1, 2, 4, 8, 32, 3, 5, 10, 12, 15, 47, 7, 11, 13, 14, 6, 9, 31, 35, 37, 42, 44, 33, 34,
    36, 40, 39, 43, 45, 46, 17, 18, 20, 24, 19, 21, 26, 28, 23, 27, 29, 30, 22, 25, 38, 41,
];

const fn invert(t: &[u8; 48]) -> [u8; 48] {
    let mut out = [0u8; 48];
    let mut i = 0;
    while i < 48 {
        out[t[i] as usize] = i as u8;
        i += 1;
    }
    out
}

/// Frame zig-zag scan: scan position → raster index in the 4×4 block.
pub const ZIGZAG: [usize; 16] = [0, 1, 4, 8, 5, 2, 3, 6, 9, 12, 13, 10, 7, 11, 14, 15];

/// Forward quantisation multipliers by `qp % 6` for the three coefficient position classes
/// (both even, both odd, mixed).
pub const QUANT_MF: [[i32; 3]; 6] = [
    [13107, 5243, 8066],
    [11916, 4660, 7490],
    [10082, 4194, 6554],
    [9362, 3647, 5825],
    [8192, 3355, 5243],
    [7282, 2893, 4559],
];

/// Dequantisation scales `v` by `qp % 6` and position class (LevelScale = 16 · v with flat
/// matrices).
pub const DEQUANT_V: [[i32; 3]; 6] = [
    [10, 16, 13],
    [11, 18, 14],
    [13, 20, 16],
    [14, 23, 18],
    [16, 25, 20],
    [18, 29, 23],
];

/// Position class of raster index `i` in a 4×4 block.
pub const POS_CLASS: [usize; 16] = {
    let mut t = [0usize; 16];
    let mut i = 0;
    while i < 16 {
        let (x, y) = (i % 4, i / 4);
        t[i] = if x % 2 == 0 && y % 2 == 0 {
            0
        } else if x % 2 == 1 && y % 2 == 1 {
            1
        } else {
            2
        };
        i += 1;
    }
    t
};

/// QPc for qPi 0..=51 (chroma_format_idc 1).
pub const CHROMA_QP: [u8; 52] = {
    let tail = [
        29u8, 30, 31, 32, 32, 33, 34, 34, 35, 35, 36, 36, 37, 37, 37, 38, 38, 38, 39, 39, 39, 39,
    ];
    let mut t = [0u8; 52];
    let mut i = 0;
    while i < 52 {
        t[i] = if i < 30 { i as u8 } else { tail[i - 30] };
        i += 1;
    }
    t
};

/// α′ by indexA.
pub const ALPHA: [u8; 52] = pad16([
    4, 4, 5, 6, 7, 8, 9, 10, 12, 13, 15, 17, 20, 22, 25, 28, 32, 36, 40, 45, 50, 56, 63, 71, 80,
    90, 101, 113, 127, 144, 162, 182, 203, 226, 255, 255,
]);
/// β′ by indexB.
pub const BETA: [u8; 52] = pad16([
    2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13, 14, 14,
    15, 15, 16, 16, 17, 17, 18, 18,
]);

const fn pad16(v: [u8; 36]) -> [u8; 52] {
    let mut t = [0u8; 52];
    let mut i = 0;
    while i < 36 {
        t[16 + i] = v[i];
        i += 1;
    }
    t
}

/// tC0 by indexA for bS 1, 2 and 3.
pub const TC0: [[u8; 3]; 52] = {
    let mut t = [[0u8; 3]; 52];
    let mut i = 0;
    while i < 52 {
        t[i] = match i {
            0..=16 => [0, 0, 0],
            17..=20 => [0, 0, 1],
            21..=22 => [0, 1, 1],
            23..=26 => [1, 1, 1],
            27..=30 => [1, 1, 2],
            31..=32 => [1, 2, 3],
            33 => [2, 2, 3],
            34 => [2, 2, 4],
            35..=36 => [2, 3, 4],
            37 => [3, 3, 5],
            38..=39 => [3, 4, 6],
            40 => [4, 5, 7],
            41 => [4, 5, 8],
            42 => [4, 6, 9],
            43 => [5, 7, 10],
            44 => [6, 8, 11],
            45 => [6, 8, 13],
            46 => [7, 10, 14],
            47 => [8, 11, 16],
            48 => [9, 12, 18],
            49 => [10, 13, 20],
            50 => [11, 15, 23],
            _ => [13, 17, 25],
        };
        i += 1;
    }
    t
};

/// Raster position (x, y in 4×4 block units) of luma4x4BlkIdx.
pub const BLK_XY: [(usize, usize); 16] = [
    (0, 0),
    (1, 0),
    (0, 1),
    (1, 1),
    (2, 0),
    (3, 0),
    (2, 1),
    (3, 1),
    (0, 2),
    (1, 2),
    (0, 3),
    (1, 3),
    (2, 2),
    (3, 2),
    (2, 3),
    (3, 3),
];

#[cfg(test)]
mod tests {
    use super::*;

    /// Every complete prefix code has Kraft sum exactly 1 (or < 1 where the table leaves a
    /// code unused); none may exceed 1.
    fn kraft(codes: &[(u8, u8)]) -> f64 {
        codes.iter().map(|&(l, _)| 0.5f64.powi(i32::from(l))).sum()
    }

    fn prefix_free(codes: &[(u32, u32)]) {
        for (i, &(la, ca)) in codes.iter().enumerate() {
            for (j, &(lb, cb)) in codes.iter().enumerate() {
                if i == j {
                    continue;
                }
                if la <= lb {
                    assert_ne!(cb >> (lb - la), ca, "{la}/{ca} is a prefix of {lb}/{cb}");
                }
            }
        }
    }

    #[test]
    fn total_zeros_tables_are_prefix_codes() {
        for t in TOTAL_ZEROS.iter().chain(TOTAL_ZEROS_CDC.iter()) {
            assert!(kraft(t) <= 1.0 + 1e-12);
            prefix_free(
                &t.iter()
                    .map(|&(l, c)| (u32::from(l), u32::from(c)))
                    .collect::<Vec<_>>(),
            );
        }
        for t in RUN_BEFORE.iter() {
            assert!(kraft(t) <= 1.0 + 1e-12);
            prefix_free(
                &t.iter()
                    .map(|&(l, c)| (u32::from(l), u32::from(c)))
                    .collect::<Vec<_>>(),
            );
        }
    }

    #[test]
    fn coeff_token_tables_are_prefix_codes() {
        for nc in [-1, 0, 2, 4] {
            let max = if nc == -1 { 4 } else { 16 };
            let mut codes = Vec::new();
            for tc in 0..=max {
                for t1 in 0..=tc.min(3) {
                    codes.push(coeff_token(nc, tc, t1));
                }
            }
            let k: f64 = codes.iter().map(|&(l, _)| 0.5f64.powi(l as i32)).sum();
            assert!(k <= 1.0 + 1e-12, "nC {nc}: {k}");
            prefix_free(&codes);
        }
    }

    #[test]
    fn cbp_tables_are_permutations() {
        let mut seen = [false; 48];
        for &c in &CBP_INTRA {
            seen[c as usize] = true;
        }
        assert!(seen.iter().all(|&s| s));
        assert_eq!(CBP_INTRA[47], 0);
        assert_eq!(CBP_INTER[0], 0);
        assert_eq!(CBP_INTER[47], 12);
    }
}
