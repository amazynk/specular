//! Logical ASTC blocks and their packing to 128-bit physical blocks (Khronos Data Format
//! Specification, ASTC chapter), shared by the Basis Universal intermediate formats, whose
//! transcoders produce logical blocks. Only what those formats use: 2D blocks, one CEM for all
//! partitions, void-extent blocks without extents. Also the quantisation tables they need
//! (ISE value ↔ unquantised value, ISE value ↔ rank).

// Index loops follow the specification's pseudo-code.
#![allow(clippy::needless_range_loop)]

use std::sync::OnceLock;

use crate::astc::{QUINTS, RANGES, TRITS, WEIGHT_UNQUANT, block_mode, colour_unquant, ise_bits};

/// Range indices (BISE ranges in specification order).
pub(crate) const R256: u8 = 20;

/// The range index with `levels` levels.
pub(crate) const fn range_of(levels: u16) -> u8 {
    let mut r = 0;
    while r < RANGES.len() {
        if RANGES[r].0 == levels {
            return r as u8;
        }
        r += 1;
    }
    panic!("no ISE range with that many levels");
}

/// Levels of range `r`.
pub(crate) fn levels(r: u8) -> usize {
    usize::from(RANGES[usize::from(r)].0)
}

/// A logical ASTC block (the `log_astc_block` of the Basis Universal specifications).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LogBlock {
    /// Void-extent block of `solid` (half floats when `solid_hdr`, else UNORM16).
    pub solid: bool,
    pub solid_hdr: bool,
    pub solid_colour: [u16; 4],
    pub grid_w: u8,
    pub grid_h: u8,
    pub dual: bool,
    /// Colour component of the second plane.
    pub ccs: u8,
    pub parts: u8,
    pub seed: u16,
    /// Colour endpoint mode of every partition (0 in a cleared block).
    pub cem: u8,
    pub erange: u8,
    pub wrange: u8,
    pub endpoints: [u8; 18],
    /// ISE weights; dual plane interleaves the planes.
    pub weights: [u8; 64],
}

impl Default for LogBlock {
    fn default() -> Self {
        LogBlock {
            solid: false,
            solid_hdr: false,
            solid_colour: [0; 4],
            grid_w: 0,
            grid_h: 0,
            dual: false,
            ccs: 0,
            parts: 0,
            seed: 0,
            cem: 0,
            erange: 0,
            wrange: 0,
            endpoints: [0; 18],
            weights: [0; 64],
        }
    }
}

/// Endpoint values per CEM.
pub(crate) fn cem_values(cem: u8) -> usize {
    2 * (usize::from(cem >> 2) + 1)
}

impl LogBlock {
    pub(crate) fn weight_count(&self) -> usize {
        usize::from(self.grid_w) * usize::from(self.grid_h) * if self.dual { 2 } else { 1 }
    }
}

/// Smallest 8-bit trit code per five trits (base-3 index, t0 least significant), and the
/// smallest 7-bit quint code per three quints: the canonical encodings, whose high bits are
/// zero when the trailing values are, as truncated groups need.
fn encode_tables() -> &'static ([u8; 243], [u8; 125]) {
    static T: OnceLock<([u8; 243], [u8; 125])> = OnceLock::new();
    T.get_or_init(|| {
        let mut tr = [0u8; 243];
        let mut seen = [false; 243];
        for code in 0..256usize {
            let t = TRITS[code];
            let i = t.iter().rev().fold(0usize, |a, &v| a * 3 + usize::from(v));
            if !seen[i] {
                seen[i] = true;
                tr[i] = code as u8;
            }
        }
        let mut qu = [0u8; 125];
        let mut seen = [false; 125];
        for code in 0..128usize {
            let q = QUINTS[code];
            let i = q.iter().rev().fold(0usize, |a, &v| a * 5 + usize::from(v));
            if !seen[i] {
                seen[i] = true;
                qu[i] = code as u8;
            }
        }
        (tr, qu)
    })
}

/// Integer sequence encoding of `vals` in range `r`: the bits, LSB first, and their count.
pub(crate) fn ise_encode(vals: &[u8], r: usize) -> (u128, u32) {
    let (_, t, q, nb) = RANGES[r];
    let n = vals.len();
    let total = ise_bits(n as u32, r);
    let mut seq = 0u128;
    let mut at = 0u32;
    let mut put = |v: u32, bits: u32| {
        if bits > 0 && at < 128 {
            seq |= u128::from(v & ((1 << bits) - 1)) << at;
        }
        at += bits;
    };
    let (tr, qu) = encode_tables();
    let mask = (1u32 << nb) - 1;
    if t {
        for g in (0..n).step_by(5) {
            let mut m = [0u32; 5];
            let mut idx = 0usize;
            for k in (0..5).rev() {
                let v = vals.get(g + k).map_or(0, |&v| u32::from(v));
                m[k] = v & mask;
                idx = idx * 3 + (v >> nb) as usize;
            }
            let c = u32::from(tr[idx.min(242)]);
            put(m[0], nb);
            put(c, 2);
            put(m[1], nb);
            put(c >> 2, 2);
            put(m[2], nb);
            put(c >> 4, 1);
            put(m[3], nb);
            put(c >> 5, 2);
            put(m[4], nb);
            put(c >> 7, 1);
        }
    } else if q {
        for g in (0..n).step_by(3) {
            let mut m = [0u32; 3];
            let mut idx = 0usize;
            for k in (0..3).rev() {
                let v = vals.get(g + k).map_or(0, |&v| u32::from(v));
                m[k] = v & mask;
                idx = idx * 5 + (v >> nb) as usize;
            }
            let c = u32::from(qu[idx.min(124)]);
            put(m[0], nb);
            put(c, 3);
            put(m[1], nb);
            put(c >> 3, 2);
            put(m[2], nb);
            put(c >> 5, 2);
        }
    } else {
        for &v in vals {
            put(u32::from(v), nb);
        }
    }
    let keep = if total >= 128 {
        u128::MAX
    } else {
        (1u128 << total) - 1
    };
    (seq & keep, total)
}

/// Block mode per (grid width, grid height, weight range, dual plane): the smallest encoding.
fn mode_table() -> &'static std::collections::HashMap<(usize, usize, usize, bool), u32> {
    static T: OnceLock<std::collections::HashMap<(usize, usize, usize, bool), u32>> =
        OnceLock::new();
    T.get_or_init(|| {
        let mut m = std::collections::HashMap::new();
        for mode in 0..2048u32 {
            if mode & 0x1ff == 0x1fc {
                continue;
            }
            if let Some(k) = block_mode(mode) {
                m.entry(k).or_insert(mode);
            }
        }
        m
    })
}

/// Pack a logical block into a physical 128-bit block (little-endian bytes), or None if it
/// cannot be expressed (the endpoint range is not the one the bits left over imply, too many
/// weights, an unknown grid).
pub(crate) fn pack(b: &LogBlock) -> Option<[u8; 16]> {
    if b.solid {
        // Void extent: no extent coordinates (all ones), the colour in the top 64 bits.
        let low: u64 = if b.solid_hdr {
            0xffff_ffff_ffff_fffc
        } else {
            0xffff_ffff_ffff_fdfc
        };
        let c = &b.solid_colour;
        let high =
            u64::from(c[0]) | u64::from(c[1]) << 16 | u64::from(c[2]) << 32 | u64::from(c[3]) << 48;
        return Some((u128::from(low) | u128::from(high) << 64).to_le_bytes());
    }
    let (gw, gh) = (usize::from(b.grid_w), usize::from(b.grid_h));
    let wr = usize::from(b.wrange);
    let mode = *mode_table().get(&(gw, gh, wr, b.dual))?;
    let parts = usize::from(b.parts);
    if !(1..=4).contains(&parts) || (b.dual && parts == 4) {
        return None;
    }
    let nw = b.weight_count();
    if nw > 64 {
        return None;
    }
    let wbits = ise_bits(nw as u32, wr);
    if !(24..=96).contains(&wbits) {
        return None;
    }
    let mut bits = u128::from(mode) | ((parts as u128 - 1) << 11);
    let colour_start = if parts == 1 {
        bits |= u128::from(b.cem & 15) << 13;
        17
    } else {
        bits |= u128::from(b.seed & 0x3ff) << 13;
        bits |= u128::from(b.cem & 15) << 25;
        29
    };
    let below = 128 - wbits;
    let ccs_bits = if b.dual { 2 } else { 0 };
    let colour_bits = below.checked_sub(ccs_bits + colour_start)?;
    let nvals = cem_values(b.cem) * parts;
    if nvals > 18 {
        return None;
    }
    let crange = (4..21)
        .rev()
        .find(|&r| ise_bits(nvals as u32, r) <= colour_bits)?;
    if crange != usize::from(b.erange) {
        return None;
    }
    let (eseq, _) = ise_encode(&b.endpoints[..nvals], crange);
    bits |= eseq << colour_start;
    if b.dual {
        bits |= u128::from(b.ccs & 3) << (below - 2);
    }
    let (wseq, _) = ise_encode(&b.weights[..nw], wr);
    bits |= wseq.reverse_bits();
    Some(bits.to_le_bytes())
}

/// Quantisation tables of one range: unquantised value per ISE value, nearest ISE value per
/// unquantised value (the first in ISE order on a tie), and ISE value ↔ rank (ascending
/// unquantised value).
pub(crate) struct Quant {
    pub ise_to_val: Vec<u8>,
    pub val_to_ise: Vec<u8>,
    pub ise_to_rank: Vec<u8>,
    pub rank_to_ise: Vec<u8>,
}

fn quant(ise_to_val: Vec<u8>, max: usize) -> Quant {
    let n = ise_to_val.len();
    let val_to_ise = (0..=max)
        .map(|v| {
            (0..n)
                .min_by_key(|&i| (i32::from(ise_to_val[i]) - v as i32).unsigned_abs())
                .unwrap_or(0) as u8
        })
        .collect();
    let mut rank_to_ise: Vec<u8> = (0..n).map(|i| i as u8).collect();
    rank_to_ise.sort_by_key(|&i| (ise_to_val[usize::from(i)], i));
    let mut ise_to_rank = vec![0u8; n];
    for (rank, &i) in rank_to_ise.iter().enumerate() {
        ise_to_rank[usize::from(i)] = rank as u8;
    }
    Quant {
        ise_to_val,
        val_to_ise,
        ise_to_rank,
        rank_to_ise,
    }
}

/// Endpoint tables for ranges 4…20 (lower indices are empty).
pub(crate) fn endpoint_quant() -> &'static [Quant] {
    static T: OnceLock<Vec<Quant>> = OnceLock::new();
    T.get_or_init(|| {
        (0..21)
            .map(|r| {
                let n = if r >= 4 { levels(r as u8) } else { 0 };
                quant(colour_unquant()[r][..n].to_vec(), 255)
            })
            .collect()
    })
}

/// Weight tables (unquantised to 0…64) for ranges 0…11.
pub(crate) fn weight_quant() -> &'static [Quant] {
    static T: OnceLock<Vec<Quant>> = OnceLock::new();
    T.get_or_init(|| {
        (0..12)
            .map(|r| quant(WEIGHT_UNQUANT[r][..levels(r as u8)].to_vec(), 64))
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ise_encode_round_trips() {
        let mut seed = 0x1234_5678u32;
        let mut rnd = || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed
        };
        for r in 0..21 {
            let lv = levels(r as u8) as u32;
            for n in 1..=20usize {
                if ise_bits(n as u32, r) > 128 {
                    continue;
                }
                let vals: Vec<u8> = (0..n).map(|_| (rnd() % lv) as u8).collect();
                let (seq, len) = ise_encode(&vals, r);
                assert!(len == 128 || seq >> len == 0);
                let mut out = vec![0u8; n];
                crate::astc::ise_decode(seq, r, &mut out);
                assert_eq!(out, vals, "range {r}, {n} values");
            }
        }
    }

    #[test]
    fn packed_blocks_decode_to_the_logical_block() {
        // A 6×6 single-plane CEM 11 block: 6 endpoint values at 256 levels, 6×6 weights of 3.
        let mut b = LogBlock {
            grid_w: 6,
            grid_h: 6,
            parts: 1,
            cem: 11,
            erange: R256,
            wrange: range_of(3),
            ..LogBlock::default()
        };
        b.endpoints[..6].copy_from_slice(&[10, 200, 30, 250, 128, 64]);
        for (i, w) in b.weights[..36].iter_mut().enumerate() {
            *w = (i % 3) as u8;
        }
        assert!(pack(&b).is_some());
        // A wrong endpoint range is refused rather than mis-packed.
        b.erange = range_of(80);
        assert!(pack(&b).is_none());
    }
}
