//! The lookup tables of [`crate::tables`], built once, and the escape coding limits
//! (LMAX, RMAX of §7.4.1.3) derived from the coefficient tables.

use std::sync::OnceLock;

use specular_mpeg2::util::Vlc;

use crate::tables;

/// The coefficient tables' escape code. Other values: last in bit 15, run in bits 8..14,
/// level in bits 0..8.
pub(crate) const ESCAPE: u16 = 0x7FFF;

pub(crate) struct Tables {
    /// Values: mb_type << 4 | cbpc (mb_type 0xF: stuffing).
    pub mcbpc_i: Vlc,
    pub mcbpc_p: Vlc,
    pub cbpy: Vlc,
    pub motion: Vlc,
    pub mb_type_b: Vlc,
    pub dc_luma: Vlc,
    pub dc_chroma: Vlc,
    pub intra: Vlc,
    pub inter: Vlc,
    pub sprite_dmv: Vlc,
    /// The largest level per (last, run) and the longest run per (last, level).
    pub intra_lmax: [[u8; 64]; 2],
    pub intra_rmax: [[u8; 64]; 2],
    pub inter_lmax: [[u8; 64]; 2],
    pub inter_rmax: [[u8; 64]; 2],
}

fn coeffs(t: &[(u16, u8, u8, u8, u8)]) -> (Vlc, [[u8; 64]; 2], [[u8; 64]; 2]) {
    let mut v: Vec<(u32, u8, u16)> = t
        .iter()
        .map(|&(c, l, last, run, level)| {
            (
                u32::from(c),
                l,
                u16::from(last) << 15 | u16::from(run) << 8 | u16::from(level),
            )
        })
        .collect();
    v.push((3, 7, ESCAPE));
    let mut lmax = [[0u8; 64]; 2];
    let mut rmax = [[0u8; 64]; 2];
    for &(_, _, last, run, level) in t {
        let l = usize::from(last);
        lmax[l][usize::from(run)] = lmax[l][usize::from(run)].max(level);
        rmax[l][usize::from(level)] = rmax[l][usize::from(level)].max(run);
    }
    (Vlc::build(&v), lmax, rmax)
}

fn simple(t: &[(u16, u8)]) -> Vlc {
    Vlc::build(
        &t.iter()
            .enumerate()
            .map(|(i, &(c, l))| (u32::from(c), l, i as u16))
            .collect::<Vec<_>>(),
    )
}

fn mcbpc(t: &[(u16, u8, u8, u8)]) -> Vlc {
    Vlc::build(
        &t.iter()
            .map(|&(c, l, ty, cbpc)| {
                (
                    u32::from(c),
                    l,
                    u16::from(ty.min(0xF)) << 4 | u16::from(cbpc),
                )
            })
            .collect::<Vec<_>>(),
    )
}

pub(crate) fn tables() -> &'static Tables {
    static T: OnceLock<Tables> = OnceLock::new();
    T.get_or_init(|| {
        let (intra, intra_lmax, intra_rmax) = coeffs(&tables::INTRA_TCOEF);
        let (inter, inter_lmax, inter_rmax) = coeffs(&tables::INTER_TCOEF);
        Tables {
            mcbpc_i: mcbpc(&tables::MCBPC_I),
            mcbpc_p: mcbpc(&tables::MCBPC_P),
            cbpy: simple(&tables::CBPY),
            motion: simple(&tables::MOTION),
            mb_type_b: simple(&tables::MB_TYPE_B),
            dc_luma: simple(&tables::DC_LUMA),
            dc_chroma: simple(&tables::DC_CHROMA),
            intra,
            inter,
            sprite_dmv: simple(&tables::SPRITE_DMV_LENGTH),
            intra_lmax,
            intra_rmax,
            inter_lmax,
            inter_rmax,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_mpeg2::util::Bits;

    #[test]
    fn coefficient_tables_decode_their_codes() {
        let t = tables();
        for (tab, vlc) in [
            (&tables::INTRA_TCOEF, &t.intra),
            (&tables::INTER_TCOEF, &t.inter),
        ] {
            for &(c, l, last, run, level) in tab.iter() {
                let word = (u32::from(c) << (32 - u32::from(l))).to_be_bytes();
                let mut r = Bits::new(&word);
                let v = vlc.read(&mut r).unwrap();
                assert_eq!(v >> 15, u16::from(last));
                assert_eq!((v >> 8) & 63, u16::from(run));
                assert_eq!(v & 0xFF, u16::from(level));
            }
        }
        // H.263 Table 16: LMAX of (last 0, run 0) is 12, RMAX of (last 1, level 1) is 40.
        assert_eq!(t.inter_lmax[0][0], 12);
        assert_eq!(t.inter_rmax[1][1], 40);
        assert_eq!(t.intra_lmax[0][0], 27);
    }
}
