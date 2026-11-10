//! The range coder's two overflow models (symbol widths out of 65536), copied as data from
//! the Monkey's Audio SDK 13.43 (`Source/MACLib/UnBitArrayBase.h`, `RANGE_WIDTH_1` and
//! `RANGE_WIDTH_2`), 3-clause BSD licence, Copyright 2000-2026 Matthew T. Ashland.
//! See `tables.PROVENANCE.md`. The cumulative totals are summed from them at start-up.

/// File versions 3891–3989.
pub(crate) const WIDTH_1: [u32; 64] = [
    14824, 13400, 11124, 8507, 6139, 4177, 2755, 1756, 1104, 677, 415, 248, 150, 89, 54, 31, 19,
    11, 7, 4, 2, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
];

/// File versions 3990 and later.
pub(crate) const WIDTH_2: [u32; 64] = [
    19578, 16582, 12257, 7906, 4576, 2366, 1170, 536, 261, 119, 65, 31, 19, 10, 6, 3, 3, 2, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
];

/// The cumulative totals of a width table (65 entries, the last 65536).
pub(crate) const fn totals(w: &[u32; 64]) -> [u32; 65] {
    let mut t = [0u32; 65];
    let mut i = 0;
    while i < 64 {
        t[i + 1] = t[i] + w[i];
        i += 1;
    }
    t
}

pub(crate) const TOTAL_1: [u32; 65] = totals(&WIDTH_1);
pub(crate) const TOTAL_2: [u32; 65] = totals(&WIDTH_2);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn models_cover_the_range() {
        assert_eq!(TOTAL_1[64], 65536);
        assert_eq!(TOTAL_2[64], 65536);
        // Spot values of the SDK's printed totals.
        assert_eq!(TOTAL_1[4], 47855);
        assert_eq!(TOTAL_2[4], 56323);
    }
}
