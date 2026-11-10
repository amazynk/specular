//! `alf_data()` (7.3.2.18) and its semantics (7.4.3.18).

use crate::bits::{BitReader, ceil_log2};
use crate::error::{Error, Result};

/// NumAlfFilters.
pub const NUM_ALF_FILTERS: usize = 25;

/// An ALF APS with its filters derived. Clipping values are kept as clipping indices
/// (alf_luma_clip_idx / alf_chroma_clip_idx), turned into AlfClipL / AlfClipC with the bit
/// depth of the picture by [`clip_value`].
#[derive(Clone, Debug, Default)]
pub struct AlfAps {
    pub luma: bool,
    pub chroma: bool,
    pub cc_cb: bool,
    pub cc_cr: bool,
    /// AlfCoeffL[ filtIdx ][ j ].
    pub luma_coeff: [[i8; 12]; NUM_ALF_FILTERS],
    /// Clipping index per filter class and tap.
    pub luma_clip: [[u8; 12]; NUM_ALF_FILTERS],
    /// AlfCoeffC[ altIdx ][ j ] for alf_chroma_num_alt_filters_minus1 + 1 filters.
    pub chroma_coeff: Vec<[i8; 6]>,
    pub chroma_clip: Vec<[u8; 6]>,
    /// CcAlfApsCoeffCb / CcAlfApsCoeffCr [ k ][ j ].
    pub cc_cb_coeff: Vec<[i16; 7]>,
    pub cc_cr_coeff: Vec<[i16; 7]>,
}

/// AlfClip for a bit depth and clipping index (Table 8).
#[inline]
pub fn clip_value(bit_depth: u32, clip_idx: u8) -> i32 {
    let s = match clip_idx {
        0 => 0,
        1 => 3,
        2 => 5,
        _ => 7,
    };
    1 << (bit_depth - s)
}

fn coeff(r: &mut BitReader<'_>, what: &'static str) -> Result<i8> {
    let a = r.ue_max(128, what)?;
    let v = if a != 0 && r.flag()? {
        -(a as i32)
    } else {
        a as i32
    };
    if !(-128..=127).contains(&v) {
        return Err(Error::InvalidData(what));
    }
    Ok(v as i8)
}

fn cc_filters(r: &mut BitReader<'_>) -> Result<Vec<[i16; 7]>> {
    let n = r.ue_max(3, "too many CC-ALF filters")? as usize + 1;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let mut f = [0i16; 7];
        for c in f.iter_mut() {
            let a = r.u(3)?;
            if a != 0 {
                let neg = r.flag()?;
                let m = 1i16 << (a - 1);
                *c = if neg { -m } else { m };
            }
        }
        out.push(f);
    }
    Ok(out)
}

/// Parses `alf_data()`; `chroma_present` is aps_chroma_present_flag.
pub fn parse_alf_data(r: &mut BitReader<'_>, chroma_present: bool) -> Result<AlfAps> {
    let mut a = AlfAps {
        luma: r.flag()?,
        ..AlfAps::default()
    };
    if chroma_present {
        a.chroma = r.flag()?;
        a.cc_cb = r.flag()?;
        a.cc_cr = r.flag()?;
    }
    if a.luma {
        let clip = r.flag()?;
        let n = r.ue_max(NUM_ALF_FILTERS as u32 - 1, "too many ALF luma filters")? as usize + 1;
        let mut delta = [0usize; NUM_ALF_FILTERS];
        if n > 1 {
            let bits = ceil_log2(n as u32);
            for d in delta.iter_mut() {
                *d = r.u(bits)? as usize;
                if *d >= n {
                    return Err(Error::InvalidData("alf_luma_coeff_delta_idx out of range"));
                }
            }
        }
        let mut coeffs = vec![[0i8; 12]; n];
        for f in coeffs.iter_mut() {
            for c in f.iter_mut() {
                *c = coeff(r, "ALF luma coefficient out of range")?;
            }
        }
        let mut clips = vec![[0u8; 12]; n];
        if clip {
            for f in clips.iter_mut() {
                for c in f.iter_mut() {
                    *c = r.u(2)? as u8;
                }
            }
        }
        for (k, &d) in delta.iter().enumerate() {
            a.luma_coeff[k] = coeffs[d];
            a.luma_clip[k] = clips[d];
        }
    }
    if a.chroma {
        let clip = r.flag()?;
        let n = r.ue_max(7, "too many ALF chroma filters")? as usize + 1;
        for _ in 0..n {
            let mut f = [0i8; 6];
            for c in f.iter_mut() {
                *c = coeff(r, "ALF chroma coefficient out of range")?;
            }
            let mut cl = [0u8; 6];
            if clip {
                for c in cl.iter_mut() {
                    *c = r.u(2)? as u8;
                }
            }
            a.chroma_coeff.push(f);
            a.chroma_clip.push(cl);
        }
    }
    if a.cc_cb {
        a.cc_cb_coeff = cc_filters(r)?;
    }
    if a.cc_cr {
        a.cc_cr_coeff = cc_filters(r)?;
    }
    Ok(a)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// MSB-first bit writer for hand-built syntax.
    struct W(Vec<bool>);
    impl W {
        fn u(&mut self, v: u32, n: u32) {
            for i in (0..n).rev() {
                self.0.push((v >> i) & 1 == 1);
            }
        }
        fn ue(&mut self, v: u32) {
            let x = v + 1;
            let n = 32 - x.leading_zeros();
            self.u(0, n - 1);
            self.u(x, n);
        }
        fn bytes(mut self) -> Vec<u8> {
            self.0.push(true);
            while !self.0.len().is_multiple_of(8) {
                self.0.push(false);
            }
            self.0
                .chunks(8)
                .map(|c| c.iter().fold(0u8, |a, &b| (a << 1) | u8::from(b)))
                .collect()
        }
    }

    #[test]
    fn parses_luma_chroma_and_cc_filters() {
        let mut w = W(Vec::new());
        w.u(1, 1); // luma
        w.u(1, 1); // chroma
        w.u(1, 1); // cc cb
        w.u(0, 1); // cc cr
        w.u(1, 1); // luma clip
        w.ue(1); // two luma filters
        for k in 0..NUM_ALF_FILTERS {
            w.u(u32::from(k % 3 == 0), 1); // delta idx, 1 bit
        }
        for f in 0..2 {
            for j in 0..12u32 {
                let v = j + f;
                w.ue(v);
                if v != 0 {
                    w.u(u32::from(j == 5), 1);
                }
            }
        }
        for f in 0..2u32 {
            for j in 0..12u32 {
                w.u((j + f) & 3, 2);
            }
        }
        w.u(0, 1); // chroma clip
        w.ue(0); // one chroma filter
        for j in 0..6u32 {
            w.ue(j);
            if j != 0 {
                w.u(1, 1);
            }
        }
        w.ue(0); // one CC Cb filter
        for j in 0..7u32 {
            w.u(j & 7, 3);
            if j & 7 != 0 {
                w.u(u32::from(j == 3), 1);
            }
        }
        let d = w.bytes();
        let mut r = BitReader::new(&d);
        let a = parse_alf_data(&mut r, true).unwrap();
        assert!(a.luma && a.chroma && a.cc_cb && !a.cc_cr);
        // Class 0 uses signalled filter 1 (delta idx 1), class 1 filter 0.
        assert_eq!(a.luma_coeff[0][5], -6);
        assert_eq!(a.luma_coeff[0][11], 12);
        assert_eq!(a.luma_coeff[1][2], 2);
        assert_eq!(a.luma_coeff[3][0], 1);
        assert_eq!(a.luma_clip[3][0], 1);
        assert_eq!(a.luma_clip[1][3], 3);
        assert_eq!(a.chroma_coeff, vec![[0, -1, -2, -3, -4, -5]]);
        assert_eq!(a.cc_cb_coeff, vec![[0, 1, 2, -4, 8, 16, 32]]);
        assert_eq!(clip_value(10, 0), 1024);
        assert_eq!(clip_value(10, 3), 8);
    }
}
