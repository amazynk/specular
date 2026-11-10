//! Short-term reference picture sets (clauses 7.3.7 and 7.4.8).

use crate::bits::BitReader;
use crate::error::{Error, Result};
use alloc::vec::Vec;

/// Most pictures one short-term set can name (`sps_max_dec_pic_buffering`
/// is at most 16).
pub const MAX_DELTAS: usize = 16;

/// One short-term reference picture set, in derived form.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StRps {
    /// `NumNegativePics`.
    pub num_negative: u8,
    /// `NumPositivePics`.
    pub num_positive: u8,
    /// `DeltaPocS0[]` then `DeltaPocS1[]`.
    pub delta_poc: [i32; MAX_DELTAS],
    /// `UsedByCurrPicS0[]` then `UsedByCurrPicS1[]`.
    pub used: [bool; MAX_DELTAS],
}

impl StRps {
    /// `NumDeltaPocs`.
    #[inline]
    pub fn num_delta(&self) -> usize {
        usize::from(self.num_negative) + usize::from(self.num_positive)
    }

    /// `(DeltaPocS0[i], UsedByCurrPicS0[i])` for `i < NumNegativePics`.
    pub fn negative(&self) -> impl Iterator<Item = (i32, bool)> + '_ {
        (0..usize::from(self.num_negative)).map(|i| (self.delta_poc[i], self.used[i]))
    }

    /// `(DeltaPocS1[i], UsedByCurrPicS1[i])` for `i < NumPositivePics`.
    pub fn positive(&self) -> impl Iterator<Item = (i32, bool)> + '_ {
        let n = usize::from(self.num_negative);
        (n..self.num_delta()).map(|i| (self.delta_poc[i], self.used[i]))
    }

    /// Number of pictures the set marks as used by the current picture.
    pub fn num_used(&self) -> usize {
        self.used[..self.num_delta()].iter().filter(|&&u| u).count()
    }
}

/// Parses one `st_ref_pic_set(stRpsIdx)` and derives it.
///
/// `idx` is `stRpsIdx`; `sets` are the sets parsed so far from the SPS, of
/// which there are `num_sets` in all. A set parsed from a slice header passes
/// `idx == num_sets`.
pub fn parse(r: &mut BitReader<'_>, idx: u32, num_sets: u32, sets: &[StRps]) -> Result<StRps> {
    let mut inter_pred = false;
    if idx != 0 {
        inter_pred = r.u1()? != 0;
    }
    let mut out = StRps::default();
    if inter_pred {
        let mut delta_idx = 1u32;
        if idx == num_sets {
            delta_idx = r.ue()? + 1;
        }
        if delta_idx > idx {
            return Err(Error::InvalidData("delta_idx_minus1 out of range"));
        }
        let rf = *sets
            .get((idx - delta_idx) as usize)
            .ok_or(Error::InvalidData("bad RefRpsIdx"))?;
        let sign = r.u1()?;
        let abs = r.ue()?;
        if abs >= 1 << 15 {
            return Err(Error::InvalidData("abs_delta_rps_minus1 out of range"));
        }
        let delta_rps = (1 - 2 * sign as i32) * (abs as i32 + 1);
        let n = rf.num_delta();
        let mut used_flag = [false; MAX_DELTAS + 1];
        let mut use_delta = [true; MAX_DELTAS + 1];
        for j in 0..=n {
            used_flag[j] = r.u1()? != 0;
            if !used_flag[j] {
                use_delta[j] = r.u1()? != 0;
            }
        }
        let rneg = usize::from(rf.num_negative);
        let rpos = usize::from(rf.num_positive);
        let push = |o: &mut StRps, i: &mut usize, d: i32, u: bool| -> Result<()> {
            if *i >= MAX_DELTAS {
                return Err(Error::InvalidData("too many short-term reference pictures"));
            }
            o.delta_poc[*i] = d;
            o.used[*i] = u;
            *i += 1;
            Ok(())
        };
        // Equations 7-61 and 7-62.
        let mut i = 0usize;
        for j in (0..rpos).rev() {
            let d = rf.delta_poc[rneg + j] + delta_rps;
            if d < 0 && use_delta[rneg + j] {
                push(&mut out, &mut i, d, used_flag[rneg + j])?;
            }
        }
        if delta_rps < 0 && use_delta[n] {
            push(&mut out, &mut i, delta_rps, used_flag[n])?;
        }
        for j in 0..rneg {
            let d = rf.delta_poc[j] + delta_rps;
            if d < 0 && use_delta[j] {
                push(&mut out, &mut i, d, used_flag[j])?;
            }
        }
        let neg = i;
        for j in (0..rneg).rev() {
            let d = rf.delta_poc[j] + delta_rps;
            if d > 0 && use_delta[j] {
                push(&mut out, &mut i, d, used_flag[j])?;
            }
        }
        if delta_rps > 0 && use_delta[n] {
            push(&mut out, &mut i, delta_rps, used_flag[n])?;
        }
        for j in 0..rpos {
            let d = rf.delta_poc[rneg + j] + delta_rps;
            if d > 0 && use_delta[rneg + j] {
                push(&mut out, &mut i, d, used_flag[rneg + j])?;
            }
        }
        out.num_negative = neg as u8;
        out.num_positive = (i - neg) as u8;
    } else {
        let neg = r.ue()? as usize;
        let pos = r.ue()? as usize;
        if neg > MAX_DELTAS || pos > MAX_DELTAS || neg + pos > MAX_DELTAS {
            return Err(Error::InvalidData("too many short-term reference pictures"));
        }
        let mut poc = 0i32;
        for i in 0..neg {
            let d = r.ue()?;
            if d >= 1 << 15 {
                return Err(Error::InvalidData("delta_poc_s0_minus1 out of range"));
            }
            poc -= d as i32 + 1;
            out.delta_poc[i] = poc;
            out.used[i] = r.u1()? != 0;
        }
        poc = 0;
        for i in neg..neg + pos {
            let d = r.ue()?;
            if d >= 1 << 15 {
                return Err(Error::InvalidData("delta_poc_s1_minus1 out of range"));
            }
            poc += d as i32 + 1;
            out.delta_poc[i] = poc;
            out.used[i] = r.u1()? != 0;
        }
        out.num_negative = neg as u8;
        out.num_positive = pos as u8;
    }
    Ok(out)
}

/// Parses every `st_ref_pic_set()` of an SPS.
pub fn parse_all(r: &mut BitReader<'_>, num_sets: u32) -> Result<Vec<StRps>> {
    let mut sets = Vec::with_capacity(num_sets as usize);
    for i in 0..num_sets {
        let s = parse(r, i, num_sets, &sets)?;
        sets.push(s);
    }
    Ok(sets)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bits::BitWriter;

    #[test]
    fn explicit_and_predicted_sets_derive() {
        let mut w = BitWriter::default();
        // Set 0: two negative (-1, -3), one positive (+2).
        w.ue(2);
        w.ue(1);
        w.ue(0);
        w.put(1, 1);
        w.ue(1);
        w.put(0, 1);
        w.ue(1);
        w.put(1, 1);
        // Set 1: predicted from set 0 with deltaRps = -1, every picture kept.
        w.put(1, 1); // inter_ref_pic_set_prediction_flag
        w.put(1, 1); // delta_rps_sign
        w.ue(0); // abs_delta_rps_minus1
        for _ in 0..4 {
            w.put(1, 1);
        }
        w.rbsp_trailing();
        let r = &mut BitReader::new(&w.bytes);
        let sets = parse_all(r, 2).expect("parse");
        assert_eq!(sets[0].num_negative, 2);
        assert_eq!(&sets[0].delta_poc[..3], &[-1, -3, 2]);
        assert_eq!(&sets[0].used[..3], &[true, false, true]);
        // dPoc: S1 +2-1 = +1; S0: -1-1 = -2, -3-1 = -4; and deltaRps itself (-1).
        assert_eq!(sets[1].num_negative, 3);
        assert_eq!(sets[1].num_positive, 1);
        assert_eq!(&sets[1].delta_poc[..4], &[-1, -2, -4, 1]);
    }
}
