//! Two-level lookup tables for the variable length codes of [`crate::tables`], built once.
//!
//! The first 9 bits index the first level; a longer code's entry there points at a second
//! level indexed by the bits after them.

use std::sync::OnceLock;

use crate::bits::Bits;
use crate::tables;

const L1: u32 = 9;
const SUB: u32 = 1 << 31;

pub struct Vlc {
    l1: Vec<u32>,
    l2: Vec<u32>,
}

impl Vlc {
    /// A table from (code, length, value); lengths up to 24.
    pub fn build(codes: &[(u32, u8, u16)]) -> Vlc {
        let mut l1 = vec![0u32; 1 << L1];
        let mut l2 = Vec::new();
        let leaf = |len: u8, v: u16| u32::from(v) | u32::from(len) << 16;
        // Longest code per first-level prefix.
        let mut longest = vec![0u8; 1 << L1];
        for &(code, len, _) in codes {
            let len32 = u32::from(len);
            if len32 > L1 {
                let p = (code >> (len32 - L1)) as usize;
                longest[p] = longest[p].max(len);
            }
        }
        for (p, &m) in longest.iter().enumerate() {
            if m > 0 {
                let n = u32::from(m) - L1;
                l1[p] = SUB | (n << 21) | l2.len() as u32;
                l2.resize(l2.len() + (1 << n), 0);
            }
        }
        for &(code, len, v) in codes {
            let len32 = u32::from(len);
            if len32 <= L1 {
                let first = (code << (L1 - len32)) as usize;
                for e in &mut l1[first..first + (1 << (L1 - len32))] {
                    *e = leaf(len, v);
                }
            } else {
                let p = (code >> (len32 - L1)) as usize;
                let e = l1[p];
                let n = (e >> 21) & 31;
                let base = (e & 0x1F_FFFF) as usize;
                let rest = len32 - L1;
                let first = base + (((code & ((1 << rest) - 1)) << (n - rest)) as usize);
                for e in &mut l2[first..first + (1 << (n - rest))] {
                    *e = leaf(len, v);
                }
            }
        }
        Vlc { l1, l2 }
    }

    /// The next code's value, consumed; `None` for a code not in the table (nothing consumed).
    #[inline(always)]
    pub fn read(&self, r: &mut Bits) -> Option<u16> {
        let p = r.peek32();
        let mut e = self.l1[(p >> (32 - L1)) as usize];
        if e & SUB != 0 {
            let n = (e >> 21) & 31;
            let base = (e & 0x1F_FFFF) as usize;
            e = self.l2[base + ((p << L1) >> (32 - n)) as usize];
        }
        let len = (e >> 16) & 31;
        if len == 0 {
            return None;
        }
        r.skip(len);
        Some(e as u16)
    }
}

/// DCT coefficient table values: run in bits 0..6, level in bits 6..12, and these kinds.
pub(crate) const DCT_EOB: u16 = 1 << 12;
pub(crate) const DCT_ESCAPE: u16 = 2 << 12;

pub(crate) struct Tables {
    pub addr_inc: Vlc,
    pub mb_i: Vlc,
    pub mb_p: Vlc,
    pub mb_b: Vlc,
    pub cbp: Vlc,
    pub motion: Vlc,
    pub dc_luma: Vlc,
    pub dc_chroma: Vlc,
    /// B-14 for coefficients after the first (EOB "10", (0,1) "11").
    pub dct0: Vlc,
    /// B-15.
    pub dct1: Vlc,
}

/// Address increment values beyond 1..=33.
pub(crate) const ADDR_ESCAPE: u16 = 34;
pub(crate) const ADDR_STUFFING: u16 = 35;

fn simple(t: &[(u16, u8)], first: u16) -> Vec<(u32, u8, u16)> {
    t.iter()
        .enumerate()
        .map(|(i, &(c, l))| (u32::from(c), l, first + i as u16))
        .collect()
}

fn dct(t: &[(u16, u8, u8, u8)], eob: (u32, u8)) -> Vlc {
    let mut v: Vec<(u32, u8, u16)> = t
        .iter()
        .map(|&(c, l, run, level)| {
            // With its sign bit: two codes, the value marking the sign in bit 15.
            (u32::from(c), l, u16::from(run) | u16::from(level) << 6)
        })
        .collect();
    v.push((eob.0, eob.1, DCT_EOB));
    v.push((1, 6, DCT_ESCAPE));
    Vlc::build(&v)
}

pub(crate) fn tables() -> &'static Tables {
    static T: OnceLock<Tables> = OnceLock::new();
    T.get_or_init(|| {
        let mut addr = simple(&tables::ADDR_INC, 1);
        addr.push((
            u32::from(tables::ADDR_ESCAPE.0),
            tables::ADDR_ESCAPE.1,
            ADDR_ESCAPE,
        ));
        addr.push((
            u32::from(tables::ADDR_STUFFING.0),
            tables::ADDR_STUFFING.1,
            ADDR_STUFFING,
        ));
        let mb = |t: &[(u16, u8, u8)]| {
            Vlc::build(
                &t.iter()
                    .map(|&(c, l, f)| (u32::from(c), l, u16::from(f)))
                    .collect::<Vec<_>>(),
            )
        };
        Tables {
            addr_inc: Vlc::build(&addr),
            mb_i: mb(&tables::MB_TYPE_I),
            mb_p: mb(&tables::MB_TYPE_P),
            mb_b: mb(&tables::MB_TYPE_B),
            cbp: Vlc::build(&simple(&tables::CBP, 0)),
            motion: Vlc::build(&simple(&tables::MOTION, 0)),
            dc_luma: Vlc::build(&simple(&tables::DC_LUMA, 0)),
            dc_chroma: Vlc::build(&simple(&tables::DC_CHROMA, 0)),
            dct0: dct(&tables::B14, (2, 2)),
            dct1: dct(&tables::B15, (6, 4)),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sum of 2^-length over a table's codes (with a sign bit where `signed`).
    fn kraft(codes: impl Iterator<Item = u8>) -> f64 {
        codes.map(|l| 0.5f64.powi(i32::from(l))).sum()
    }

    #[test]
    fn prefix_codes_fill_their_trees() {
        // Every table but the reserved prefixes is complete: no gaps, no overlaps.
        let k = kraft(tables::CBP.iter().map(|c| c.1));
        assert!((k - (1.0 - 0.5f64.powi(9))).abs() < 1e-12, "cbp {k}");
        let k = kraft(tables::DC_LUMA.iter().map(|c| c.1));
        assert!((k - 1.0).abs() < 1e-12);
        let k = kraft(tables::DC_CHROMA.iter().map(|c| c.1));
        assert!((k - 1.0).abs() < 1e-12);
        let k = kraft(tables::MB_TYPE_B.iter().map(|c| c.1));
        assert!((k - (1.0 - 1.0 / 64.0)).abs() < 1e-12);
        // Each motion code but 0 has two signs.
        let k = kraft(tables::MOTION.iter().map(|c| c.1));
        assert!(k < 1.0 && k > 0.98, "motion {k}");
    }

    #[test]
    fn tables_decode_every_code() {
        let t = tables();
        for (i, &(c, l)) in tables::CBP.iter().enumerate() {
            let word = (u32::from(c) << (32 - u32::from(l))).to_be_bytes();
            let mut r = Bits::new(&word);
            assert_eq!(t.cbp.read(&mut r), Some(i as u16));
            assert_eq!(r.position(), usize::from(l));
        }
        for &(c, l, run, level) in tables::B15.iter() {
            let word = (u32::from(c) << (32 - u32::from(l))).to_be_bytes();
            let mut r = Bits::new(&word);
            let v = t.dct1.read(&mut r).unwrap();
            assert_eq!((v & 63, (v >> 6) & 63), (u16::from(run), u16::from(level)));
        }
        let mut sorted = tables::ZIGZAG;
        sorted.sort_unstable();
        assert!(sorted.iter().enumerate().all(|(i, &v)| usize::from(v) == i));
        let mut sorted = tables::ALTERNATE;
        sorted.sort_unstable();
        assert!(sorted.iter().enumerate().all(|(i, &v)| usize::from(v) == i));
    }
}
