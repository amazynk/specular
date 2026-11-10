//! Transform coefficient decoding (§8.1.3): DC differentials, the run-level-last AC coding sets
//! with their three escape modes, and dequantization.

use std::sync::LazyLock;

use crate::bits::Bits;
use crate::small::ESCLVLSZ;
use crate::tables as t;
use crate::vlc::Vlc;
use crate::{Error, malformed};

pub struct CodingSet {
    vlc: Vlc,
    run: &'static [u8],
    level: &'static [u8],
    start_last: usize,
    escape: usize,
    delta_level: [&'static [u8]; 2],
    delta_run: [&'static [u8]; 2],
}

#[allow(clippy::too_many_arguments)]
fn make(
    vlc: &[(u32, u8)],
    run: &'static [u8],
    level: &'static [u8],
    start_last: usize,
    dl0: &'static [u8],
    dl1: &'static [u8],
    dr0: &'static [u8],
    dr1: &'static [u8],
) -> CodingSet {
    CodingSet {
        vlc: Vlc::new(vlc),
        run,
        level,
        start_last,
        escape: run.len(),
        delta_level: [dl0, dl1],
        delta_run: [dr0, dr1],
    }
}

/// The eight coding sets: high motion, low motion, mid rate and high rate, each intra then
/// inter.
pub static SETS: LazyLock<[CodingSet; 8]> = LazyLock::new(|| {
    [
        make(
            &t::HIGH_MOTION_INTRA_VLC,
            &t::HIGH_MOTION_INTRA_RUN,
            &t::HIGH_MOTION_INTRA_LEVEL,
            t::HIGH_MOTION_INTRA_START_LAST,
            &t::HIGH_MOTION_INTRA_DELTA_LEVEL0,
            &t::HIGH_MOTION_INTRA_DELTA_LEVEL1,
            &t::HIGH_MOTION_INTRA_DELTA_RUN0,
            &t::HIGH_MOTION_INTRA_DELTA_RUN1,
        ),
        make(
            &t::HIGH_MOTION_INTER_VLC,
            &t::HIGH_MOTION_INTER_RUN,
            &t::HIGH_MOTION_INTER_LEVEL,
            t::HIGH_MOTION_INTER_START_LAST,
            &t::HIGH_MOTION_INTER_DELTA_LEVEL0,
            &t::HIGH_MOTION_INTER_DELTA_LEVEL1,
            &t::HIGH_MOTION_INTER_DELTA_RUN0,
            &t::HIGH_MOTION_INTER_DELTA_RUN1,
        ),
        make(
            &t::LOW_MOTION_INTRA_VLC,
            &t::LOW_MOTION_INTRA_RUN,
            &t::LOW_MOTION_INTRA_LEVEL,
            t::LOW_MOTION_INTRA_START_LAST,
            &t::LOW_MOTION_INTRA_DELTA_LEVEL0,
            &t::LOW_MOTION_INTRA_DELTA_LEVEL1,
            &t::LOW_MOTION_INTRA_DELTA_RUN0,
            &t::LOW_MOTION_INTRA_DELTA_RUN1,
        ),
        make(
            &t::LOW_MOTION_INTER_VLC,
            &t::LOW_MOTION_INTER_RUN,
            &t::LOW_MOTION_INTER_LEVEL,
            t::LOW_MOTION_INTER_START_LAST,
            &t::LOW_MOTION_INTER_DELTA_LEVEL0,
            &t::LOW_MOTION_INTER_DELTA_LEVEL1,
            &t::LOW_MOTION_INTER_DELTA_RUN0,
            &t::LOW_MOTION_INTER_DELTA_RUN1,
        ),
        make(
            &t::MID_RATE_INTRA_VLC,
            &t::MID_RATE_INTRA_RUN,
            &t::MID_RATE_INTRA_LEVEL,
            t::MID_RATE_INTRA_START_LAST,
            &t::MID_RATE_INTRA_DELTA_LEVEL0,
            &t::MID_RATE_INTRA_DELTA_LEVEL1,
            &t::MID_RATE_INTRA_DELTA_RUN0,
            &t::MID_RATE_INTRA_DELTA_RUN1,
        ),
        make(
            &t::MID_RATE_INTER_VLC,
            &t::MID_RATE_INTER_RUN,
            &t::MID_RATE_INTER_LEVEL,
            t::MID_RATE_INTER_START_LAST,
            &t::MID_RATE_INTER_DELTA_LEVEL0,
            &t::MID_RATE_INTER_DELTA_LEVEL1,
            &t::MID_RATE_INTER_DELTA_RUN0,
            &t::MID_RATE_INTER_DELTA_RUN1,
        ),
        make(
            &t::HIGH_RATE_INTRA_VLC,
            &t::HIGH_RATE_INTRA_RUN,
            &t::HIGH_RATE_INTRA_LEVEL,
            t::HIGH_RATE_INTRA_START_LAST,
            &t::HIGH_RATE_INTRA_DELTA_LEVEL0,
            &t::HIGH_RATE_INTRA_DELTA_LEVEL1,
            &t::HIGH_RATE_INTRA_DELTA_RUN0,
            &t::HIGH_RATE_INTRA_DELTA_RUN1,
        ),
        make(
            &t::HIGH_RATE_INTER_VLC,
            &t::HIGH_RATE_INTER_RUN,
            &t::HIGH_RATE_INTER_LEVEL,
            t::HIGH_RATE_INTER_START_LAST,
            &t::HIGH_RATE_INTER_DELTA_LEVEL0,
            &t::HIGH_RATE_INTER_DELTA_LEVEL1,
            &t::HIGH_RATE_INTER_DELTA_RUN0,
            &t::HIGH_RATE_INTER_DELTA_RUN1,
        ),
    ]
});

const HIGH_MOTION: usize = 0;
const LOW_MOTION: usize = 2;
const MID_RATE: usize = 4;
const HIGH_RATE: usize = 6;

/// The coding set of `index` (TRANSACFRM / TRANSACFRM2) for intra or inter blocks (Tables 71,
/// 72, 76–79).
pub fn coding_set(index: usize, pqindex: u8, intra: bool) -> &'static CodingSet {
    let base = match index {
        0 if pqindex <= 8 => HIGH_RATE,
        0 => LOW_MOTION,
        1 => HIGH_MOTION,
        _ => MID_RATE,
    };
    &SETS[base + usize::from(!intra)]
}

pub static DC_TABLES: LazyLock<[Vlc; 4]> = LazyLock::new(|| {
    [
        Vlc::new(&t::DC_LOW_LUMA),
        Vlc::new(&t::DC_LOW_CHROMA),
        Vlc::new(&t::DC_HIGH_LUMA),
        Vlc::new(&t::DC_HIGH_CHROMA),
    ]
});

/// The escape mode 3 code sizes, set by the first mode-3 escape of a picture or slice.
#[derive(Clone, Copy, Default)]
pub struct Esc3 {
    pub level_size: u32,
    pub run_size: u32,
    pub seen: bool,
    /// Table 59 (the conservative table) rather than Table 60.
    pub conservative: bool,
}

/// The DC differential of an intra block (Figure 37). `mquant` selects the extension bits.
pub fn dc_differential(
    r: &mut Bits,
    highmotion: bool,
    chroma: bool,
    mquant: u8,
) -> Result<i32, Error> {
    let vlc = &DC_TABLES[usize::from(highmotion) * 2 + usize::from(chroma)];
    let v = vlc.read(r).ok_or_else(|| malformed("bad DC code"))? as i32;
    if v == 0 {
        return Ok(0);
    }
    let mut d = if v == 119 {
        match mquant {
            1 => r.u(10) as i32,
            2 => r.u(9) as i32,
            _ => r.u(8) as i32,
        }
    } else {
        match mquant {
            1 => v * 4 + r.u(2) as i32 - 3,
            2 => v * 2 + r.u(1) as i32 - 1,
            _ => v,
        }
    };
    if r.flag() {
        d = -d;
    }
    Ok(d)
}

/// Decodes run-level-last symbols (Figure 41) into `coefs` (scan positions) from position
/// `start`; `len` positions in all.
pub fn run_levels(
    r: &mut Bits,
    set: &CodingSet,
    esc: &mut Esc3,
    coefs: &mut [i32; 64],
    start: usize,
    len: usize,
) -> Result<(), Error> {
    let mut pos = start;
    loop {
        let index = set.vlc.read(r).ok_or_else(|| malformed("bad AC code"))? as usize;
        let (run, level, last);
        if index != set.escape {
            run = set.run[index] as usize;
            let mut l = i32::from(set.level[index]);
            if r.flag() {
                l = -l;
            }
            level = l;
            last = index >= set.start_last;
        } else if r.flag() {
            // Mode 1: level + delta level by run.
            let index = set.vlc.read(r).ok_or_else(|| malformed("bad AC code"))? as usize;
            if index >= set.escape {
                return Err(malformed("escape in an escape"));
            }
            run = set.run[index] as usize;
            last = index >= set.start_last;
            let d = set.delta_level[usize::from(last)]
                .get(run)
                .copied()
                .ok_or_else(|| malformed("escape run out of range"))?;
            let mut l = i32::from(set.level[index]) + i32::from(d);
            if r.flag() {
                l = -l;
            }
            level = l;
        } else if r.flag() {
            // Mode 2: run + delta run by level + 1.
            let index = set.vlc.read(r).ok_or_else(|| malformed("bad AC code"))? as usize;
            if index >= set.escape {
                return Err(malformed("escape in an escape"));
            }
            let lv = set.level[index] as usize;
            last = index >= set.start_last;
            let d = set.delta_run[usize::from(last)]
                .get(lv)
                .copied()
                .ok_or_else(|| malformed("escape level out of range"))?;
            run = set.run[index] as usize + d as usize + 1;
            let mut l = lv as i32;
            if r.flag() {
                l = -l;
            }
            level = l;
        } else {
            // Mode 3: fixed-length codes.
            last = r.flag();
            if !esc.seen {
                esc.seen = true;
                let t = &ESCLVLSZ[usize::from(!esc.conservative)];
                esc.level_size = u32::from(t.read(r).ok_or_else(|| malformed("bad ESCLVLSZ"))?);
                esc.run_size = 3 + r.u(2);
            }
            run = r.u(esc.run_size) as usize;
            let neg = r.flag();
            let l = r.u(esc.level_size) as i32;
            level = if neg { -l } else { l };
        }
        pos += run;
        if pos >= len {
            return Err(malformed("coefficients run past the block"));
        }
        coefs[pos] = level;
        pos += 1;
        if last {
            return Ok(());
        }
        if r.overrun() {
            return Err(malformed("block data runs past the picture"));
        }
    }
}

/// DCStepSize of a quantizer (§8.1.3.3).
pub fn dc_step(mquant: u8) -> i32 {
    match mquant {
        0..=2 => 2 * i32::from(mquant),
        3 | 4 => 8,
        _ => i32::from(mquant) / 2 + 6,
    }
}

/// Dequantizes a non-DC coefficient (§8.1.3.8).
#[inline]
pub fn dequant(q: i32, double_quant: i32, mquant: i32, uniform: bool) -> i32 {
    if q == 0 {
        0
    } else if uniform {
        q * double_quant
    } else {
        q * double_quant + q.signum() * mquant
    }
}
