//! `profile_tier_level` (clause 7.3.3).

use crate::bits::BitReader;
use crate::error::Result;

/// The parts of `profile_tier_level` this decoder looks at.
#[derive(Debug, Clone, Copy, Default)]
pub struct ProfileTierLevel {
    /// `general_profile_idc`.
    pub profile_idc: u8,
    /// `general_level_idc`.
    pub level_idc: u8,
    /// `general_profile_compatibility_flag[i]` packed with bit `i` as `1 << i`.
    pub compat: u32,
}

impl ProfileTierLevel {
    /// Whether the stream declares a version 1 profile: Main, Main 10 or Main Still
    /// Picture, by `general_profile_idc` or, when that is 0, by compatibility flags 1 to 3
    /// alone.
    pub fn is_v1(&self) -> bool {
        match self.profile_idc {
            1..=3 => true,
            0 => self.compat != 0 && self.compat & !0b1110 == 0,
            _ => false,
        }
    }
}

/// Parses `profile_tier_level(profilePresentFlag, maxNumSubLayersMinus1)`.
pub fn parse(
    r: &mut BitReader<'_>,
    profile_present: bool,
    max_sub_minus1: u32,
) -> Result<ProfileTierLevel> {
    let mut p = ProfileTierLevel::default();
    if profile_present {
        r.skip(3)?; // general_profile_space, general_tier_flag
        p.profile_idc = r.u(5)? as u8;
        let mut compat = 0u32;
        for i in 0..32 {
            if r.u1()? != 0 {
                compat |= 1 << i;
            }
        }
        p.compat = compat;
        r.skip(4)?; // progressive/interlaced/non-packed/frame-only source flags
        r.skip(43)?; // general_reserved_zero_43bits
        r.skip(1)?; // general_inbld_flag or reserved bit
    }
    p.level_idc = r.u(8)? as u8;
    let n = max_sub_minus1 as usize;
    let mut prof = [false; 8];
    let mut lvl = [false; 8];
    for i in 0..n {
        prof[i] = r.u1()? != 0;
        lvl[i] = r.u1()? != 0;
    }
    if n > 0 {
        for _ in n..8 {
            r.skip(2)?;
        }
    }
    for i in 0..n {
        if prof[i] {
            r.skip(2 + 1 + 5 + 32 + 4 + 43 + 1)?;
        }
        if lvl[i] {
            r.skip(8)?;
        }
    }
    Ok(p)
}
