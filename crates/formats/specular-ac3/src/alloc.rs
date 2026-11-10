//! Exponent decoding (A/52 §7.1) and the parametric bit allocation (§7.2), with E-AC-3's
//! high efficiency pointers (Annex E §3.4.3).

use crate::bits::Bits;
use crate::tables::{BAPTAB, HEBAPTAB, HTH, LATAB, MASKTAB};

/// Exponent strategies: reuse, D15, D25, D45.
pub(crate) const REUSE: u8 = 0;

/// Table 7.12: first bin of each band (the sizes follow from the next start).
const BNDTAB: [u16; 51] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
    26, 27, 28, 31, 34, 37, 40, 43, 46, 49, 55, 61, 67, 73, 79, 85, 97, 109, 121, 133, 157, 181,
    205, 229, 253,
];
const SLOWDEC: [i32; 4] = [0x0f, 0x11, 0x13, 0x15];
const FASTDEC: [i32; 4] = [0x3f, 0x53, 0x67, 0x7b];
const SLOWGAIN: [i32; 4] = [0x540, 0x4d8, 0x478, 0x410];
const DBPBTAB: [i32; 4] = [0x000, 0x700, 0x900, 0xb00];
const FLOORTAB: [i32; 8] = [0x2f0, 0x2b0, 0x270, 0x230, 0x1f0, 0x170, 0x0f0, -2048];
pub(crate) const FASTGAIN: [i32; 8] = [0x080, 0x100, 0x180, 0x200, 0x280, 0x300, 0x380, 0x400];

/// Decodes grouped exponents: `ngrps` 7-bit groups of three differences, each difference
/// applying to `1 << (strategy - 1)` bins, from `absexp` onwards. Bins `first..` are written
/// (at most up to `end`). False when an exponent leaves 0..=24.
pub(crate) fn exponents(
    r: &mut Bits,
    strategy: u8,
    absexp: i32,
    ngrps: usize,
    first: usize,
    end: usize,
    exps: &mut [u8; 256],
) -> bool {
    let grpsize = 1usize << (strategy.clamp(1, 3) - 1);
    let mut prev = absexp;
    let mut bin = first;
    let mut ok = true;
    for _ in 0..ngrps {
        let g = r.u(7) as i32;
        if g >= 125 {
            ok = false;
        }
        for d in [g / 25, (g % 25) / 5, g % 5] {
            prev += d - 2;
            if !(0..=24).contains(&prev) {
                ok = false;
                prev = prev.clamp(0, 24);
            }
            for _ in 0..grpsize {
                if bin < end.min(256) {
                    exps[bin] = prev as u8;
                }
                bin += 1;
            }
        }
    }
    ok
}

/// The bit allocation parameters of a block (from `sdcycod`… and `floorcod`).
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct BaParams {
    pub sdcycod: u8,
    pub fdcycod: u8,
    pub sgaincod: u8,
    pub dbpbcod: u8,
    pub floorcod: u8,
}

/// Delta bit allocation of a channel.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Dba {
    /// 0 reuse, 1 new, 2 none, 3 reserved (as none).
    pub mode: u8,
    pub nseg: usize,
    pub offst: [u8; 8],
    pub len: [u8; 8],
    pub ba: [u8; 8],
}

impl Dba {
    pub(crate) fn read(&mut self, r: &mut Bits) {
        self.nseg = r.u(3) as usize + 1;
        for s in 0..self.nseg {
            self.offst[s] = r.u(5) as u8;
            self.len[s] = r.u(4) as u8;
            self.ba[s] = r.u(3) as u8;
        }
    }
}

/// One channel's allocation inputs.
pub(crate) struct Alloc<'a> {
    pub exps: &'a [u8; 256],
    pub start: usize,
    pub end: usize,
    pub params: BaParams,
    pub fscod: usize,
    pub fgaincod: u8,
    pub snroffset: i32,
    /// Coupling channel: the leak initialisation `(fastleak, slowleak)` codes.
    pub cpl_leak: Option<(u8, u8)>,
    pub lfe: bool,
    pub dba: Option<&'a Dba>,
    /// E-AC-3 AHT: high efficiency pointers.
    pub hebap: bool,
}

#[inline]
fn logadd(a: i32, b: i32) -> i32 {
    let c = a - b;
    let addr = ((c.abs() >> 1) as usize).min(255);
    if c >= 0 {
        a + i32::from(LATAB[addr])
    } else {
        b + i32::from(LATAB[addr])
    }
}

fn calc_lowcomp(a: i32, b0: i32, b1: i32, bin: usize) -> i32 {
    if bin < 7 {
        if b0 + 256 == b1 {
            384
        } else if b0 > b1 {
            (a - 64).max(0)
        } else {
            a
        }
    } else if bin < 20 {
        if b0 + 256 == b1 {
            320
        } else if b0 > b1 {
            (a - 64).max(0)
        } else {
            a
        }
    } else {
        (a - 128).max(0)
    }
}

/// Computes `bap` for bins `start..end` (§7.2.2).
pub(crate) fn allocate(a: &Alloc, bap: &mut [u8; 256]) {
    let (start, end) = (a.start.min(256), a.end.min(256));
    if start >= end {
        return;
    }
    let p = a.params;
    let sdecay = SLOWDEC[usize::from(p.sdcycod & 3)];
    let fdecay = FASTDEC[usize::from(p.fdcycod & 3)];
    let sgain = SLOWGAIN[usize::from(p.sgaincod & 3)];
    let dbknee = DBPBTAB[usize::from(p.dbpbcod & 3)];
    let floor = FLOORTAB[usize::from(p.floorcod & 7)];
    let fgain = FASTGAIN[usize::from(a.fgaincod & 7)];

    let mut psd = [0i32; 256];
    for bin in start..end {
        psd[bin] = 3072 - (i32::from(a.exps[bin]) << 7);
    }
    // PSD integration
    let mut bndpsd = [0i32; 50];
    let mut j = start;
    let mut k = usize::from(MASKTAB[start]);
    loop {
        let lastbin = (usize::from(BNDTAB[k + 1])).min(end);
        bndpsd[k] = psd[j];
        j += 1;
        while j < lastbin {
            bndpsd[k] = logadd(bndpsd[k], psd[j]);
            j += 1;
        }
        k += 1;
        if end <= lastbin || k >= 50 {
            break;
        }
    }
    // excitation
    let bndstrt = usize::from(MASKTAB[start]);
    let bndend = (usize::from(MASKTAB[end - 1]) + 1).min(50);
    let mut excite = [0i32; 50];
    let (mut fastleak, mut slowleak);
    let begin;
    let lfe_last = |bin: usize| bndend == 7 && bin == 6;
    let mut lowcomp = 0;
    if bndstrt == 0 {
        lowcomp = calc_lowcomp(lowcomp, bndpsd[0], bndpsd[1], 0);
        excite[0] = bndpsd[0] - fgain - lowcomp;
        lowcomp = calc_lowcomp(lowcomp, bndpsd[1], bndpsd[2], 1);
        excite[1] = bndpsd[1] - fgain - lowcomp;
        let mut b = 7;
        fastleak = 0;
        slowleak = 0;
        for bin in 2..7 {
            if bin >= bndend {
                b = bndend;
                break;
            }
            if !lfe_last(bin) {
                lowcomp = calc_lowcomp(lowcomp, bndpsd[bin], bndpsd[bin + 1], bin);
            }
            fastleak = bndpsd[bin] - fgain;
            slowleak = bndpsd[bin] - sgain;
            excite[bin] = fastleak - lowcomp;
            if !lfe_last(bin) && bndpsd[bin] <= bndpsd[bin + 1] {
                b = bin + 1;
                break;
            }
        }
        for bin in b..bndend.min(22) {
            if !lfe_last(bin) {
                lowcomp = calc_lowcomp(lowcomp, bndpsd[bin], bndpsd[bin + 1], bin);
            }
            fastleak -= fdecay;
            fastleak = fastleak.max(bndpsd[bin] - fgain);
            slowleak -= sdecay;
            slowleak = slowleak.max(bndpsd[bin] - sgain);
            excite[bin] = (fastleak - lowcomp).max(slowleak);
        }
        begin = 22;
    } else {
        let (f, s) = a.cpl_leak.unwrap_or((0, 0));
        fastleak = (i32::from(f) << 8) + 768;
        slowleak = (i32::from(s) << 8) + 768;
        begin = bndstrt;
    }
    for bin in begin..bndend {
        fastleak -= fdecay;
        fastleak = fastleak.max(bndpsd[bin] - fgain);
        slowleak -= sdecay;
        slowleak = slowleak.max(bndpsd[bin] - sgain);
        excite[bin] = fastleak.max(slowleak);
    }
    // masking curve
    let hth = &HTH[a.fscod.min(2)];
    let mut mask = [0i32; 50];
    for bin in bndstrt..bndend {
        if bndpsd[bin] < dbknee {
            excite[bin] += (dbknee - bndpsd[bin]) >> 2;
        }
        mask[bin] = excite[bin].max(i32::from(hth[bin]));
    }
    // delta bit allocation
    if let Some(d) = a.dba
        && d.mode <= 1
    {
        let mut band = 0usize;
        for s in 0..d.nseg {
            band += usize::from(d.offst[s]);
            let delta = if d.ba[s] >= 4 {
                (i32::from(d.ba[s]) - 3) << 7
            } else {
                (i32::from(d.ba[s]) - 4) << 7
            };
            for _ in 0..d.len[s] {
                if band < 50 {
                    mask[band] += delta;
                }
                band += 1;
            }
        }
    }
    // bit allocation pointers
    let table: &[u8; 64] = if a.hebap { &HEBAPTAB } else { &BAPTAB };
    let mut i = start;
    let mut j = usize::from(MASKTAB[start]);
    loop {
        let lastbin = usize::from(BNDTAB[j + 1]).min(end);
        let mut m = mask[j] - a.snroffset - floor;
        if m < 0 {
            m = 0;
        }
        m &= 0x1fe0;
        m += floor;
        while i < lastbin {
            let addr = ((psd[i] - m) >> 5).clamp(0, 63) as usize;
            bap[i] = table[addr];
            i += 1;
        }
        j += 1;
        if end <= lastbin || j >= 50 {
            break;
        }
    }
    let _ = a.lfe;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn band_table_matches_the_mask_table() {
        for (bin, &b) in MASKTAB.iter().enumerate().take(253) {
            let b = usize::from(b);
            assert!(usize::from(BNDTAB[b]) <= bin && bin < usize::from(BNDTAB[b + 1]));
        }
    }
}
