//! CELT bit allocation (RFC 6716 section 4.3.3) and the PVQ pulse cache.

use crate::celt_data::{
    CACHE_BITS50, CACHE_CAPS50, CACHE_INDEX50, EBAND_5MS, LOG_N400, LOG2_FRAC_TABLE,
};
use crate::range::RangeDecoder;
use crate::rfc_tables::CELT_ALLOC;

pub(crate) const NB_EBANDS: usize = 21;
pub(crate) const BITRES: i32 = 3;
const ALLOC_STEPS: i32 = 6;
const FINE_OFFSET: i32 = 21;
pub(crate) const MAX_FINE_BITS: i32 = 8;
const LOG_MAX_PSEUDO: usize = 6;

/// Band edge `i` of the 2.5 ms layout.
#[inline]
pub(crate) fn eb(i: usize) -> i32 {
    i32::from(EBAND_5MS[i])
}

/// The cached bit costs for band `band` at (LM + 1) = `lmp1`.
#[inline]
pub(crate) fn cache(lmp1: usize, band: usize) -> &'static [u8] {
    let idx = CACHE_INDEX50[lmp1 * NB_EBANDS + band];
    &CACHE_BITS50[idx.max(0) as usize..]
}

/// Pulses from a pseudo-pulse index.
#[inline]
pub(crate) fn get_pulses(i: i32) -> i32 {
    if i < 8 {
        i
    } else {
        (8 + (i & 7)) << ((i >> 3) - 1)
    }
}

/// The pseudo-pulse count whose cost is nearest `bits` (1/8 bits) without exceeding it.
pub(crate) fn bits2pulses(band: usize, lm: i32, bits: i32) -> i32 {
    let c = cache((lm + 1) as usize, band);
    let mut lo = 0i32;
    let mut hi = i32::from(c[0]);
    let bits = bits - 1;
    for _ in 0..LOG_MAX_PSEUDO {
        let mid = (lo + hi + 1) >> 1;
        if i32::from(c[mid as usize]) >= bits {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    let lo_bits = if lo == 0 {
        -1
    } else {
        i32::from(c[lo as usize])
    };
    if bits - lo_bits <= i32::from(c[hi as usize]) - bits {
        lo
    } else {
        hi
    }
}

/// Cost in 1/8 bits of `pulses` pseudo-pulses.
#[inline]
pub(crate) fn pulses2bits(band: usize, lm: i32, pulses: i32) -> i32 {
    if pulses == 0 {
        0
    } else {
        i32::from(cache((lm + 1) as usize, band)[pulses as usize]) + 1
    }
}

/// log2 of the band width in 1/8 bits.
#[inline]
pub(crate) fn log_n(band: usize) -> i32 {
    i32::from(LOG_N400[band])
}

/// Maximum allocation per band (init_caps).
pub(crate) fn init_caps(cap: &mut [i32; NB_EBANDS], lm: usize, c: usize) {
    for (i, cp) in cap.iter_mut().enumerate() {
        let n = (eb(i + 1) - eb(i)) << lm;
        *cp =
            ((i32::from(CACHE_CAPS50[NB_EBANDS * (2 * lm + c - 1) + i]) + 64) * c as i32 * n) >> 2;
    }
}

/// Result of the allocation.
pub(crate) struct Allocation {
    pub(crate) coded_bands: usize,
    pub(crate) intensity: usize,
    pub(crate) dual_stereo: bool,
    pub(crate) balance: i32,
    pub(crate) pulses: [i32; NB_EBANDS],
    pub(crate) fine_quant: [i32; NB_EBANDS],
    pub(crate) fine_priority: [i32; NB_EBANDS],
}

#[inline]
fn alloc_vec(q: usize, band: usize) -> i32 {
    i32::from(CELT_ALLOC[band][q])
}

/// Computes the allocation of `total` 1/8 bits, decoding the skip, intensity and dual-stereo
/// symbols on the way (clt_compute_allocation).
#[allow(clippy::too_many_arguments)]
pub(crate) fn compute_allocation(
    start: usize,
    end: usize,
    offsets: &[i32; NB_EBANDS],
    cap: &[i32; NB_EBANDS],
    alloc_trim: i32,
    total: i32,
    c: usize,
    lm: usize,
    ec: &mut RangeDecoder,
) -> Allocation {
    let ci = c as i32;
    let lmi = lm as i32;
    let mut total = total.max(0);
    let len = NB_EBANDS;
    let mut skip_start = start;
    let skip_rsv = if total >= 1 << BITRES { 1 << BITRES } else { 0 };
    total -= skip_rsv;
    let mut intensity_rsv = 0;
    let mut dual_stereo_rsv = 0;
    if c == 2 {
        intensity_rsv = i32::from(LOG2_FRAC_TABLE[end - start]);
        if intensity_rsv > total {
            intensity_rsv = 0;
        } else {
            total -= intensity_rsv;
            dual_stereo_rsv = if total >= 1 << BITRES { 1 << BITRES } else { 0 };
            total -= dual_stereo_rsv;
        }
    }
    let mut thresh = [0i32; NB_EBANDS];
    let mut trim_offset = [0i32; NB_EBANDS];
    for j in start..end {
        let n = eb(j + 1) - eb(j);
        thresh[j] = (ci << BITRES).max(((3 * n) << lmi << BITRES) >> 4);
        trim_offset[j] =
            (ci * n * (alloc_trim - 5 - lmi) * (end as i32 - j as i32 - 1) * (1 << (lmi + BITRES)))
                >> 6;
        if n << lmi == 1 {
            trim_offset[j] -= ci << BITRES;
        }
    }
    let nb_alloc = CELT_ALLOC[0].len() as i32;
    let mut lo = 1i32;
    let mut hi = nb_alloc - 1;
    loop {
        let mut done = false;
        let mut psum = 0i32;
        let mid = (lo + hi) >> 1;
        for j in (start..end).rev() {
            let n = eb(j + 1) - eb(j);
            let mut bitsj = (ci * n * alloc_vec(mid as usize, j)) << lmi >> 2;
            if bitsj > 0 {
                bitsj = (bitsj + trim_offset[j]).max(0);
            }
            bitsj += offsets[j];
            if bitsj >= thresh[j] || done {
                done = true;
                psum += bitsj.min(cap[j]);
            } else if bitsj >= ci << BITRES {
                psum += ci << BITRES;
            }
        }
        if psum > total {
            hi = mid - 1;
        } else {
            lo = mid + 1;
        }
        if lo > hi {
            break;
        }
    }
    hi = lo;
    lo -= 1;
    let mut bits1 = [0i32; NB_EBANDS];
    let mut bits2 = [0i32; NB_EBANDS];
    for j in start..end {
        let n = eb(j + 1) - eb(j);
        let mut bits1j = (ci * n * alloc_vec(lo as usize, j)) << lmi >> 2;
        let mut bits2j = if hi >= nb_alloc {
            cap[j]
        } else {
            (ci * n * alloc_vec(hi as usize, j)) << lmi >> 2
        };
        if bits1j > 0 {
            bits1j = (bits1j + trim_offset[j]).max(0);
        }
        if bits2j > 0 {
            bits2j = (bits2j + trim_offset[j]).max(0);
        }
        if lo > 0 {
            bits1j += offsets[j];
        }
        bits2j += offsets[j];
        if offsets[j] > 0 {
            skip_start = j;
        }
        bits2j = (bits2j - bits1j).max(0);
        bits1[j] = bits1j;
        bits2[j] = bits2j;
    }
    let _ = len;
    interp_bits2pulses(
        start,
        end,
        skip_start,
        &bits1,
        &bits2,
        &thresh,
        cap,
        total,
        skip_rsv,
        intensity_rsv,
        dual_stereo_rsv,
        c,
        lm,
        ec,
    )
}

/// Unsigned division with C's int-to-unsigned conversion of the operands.
#[inline]
fn udiv(n: i32, d: i32) -> i32 {
    ((n as u32) / (d as u32)) as i32
}

#[allow(clippy::too_many_arguments)]
fn interp_bits2pulses(
    start: usize,
    end: usize,
    skip_start: usize,
    bits1: &[i32; NB_EBANDS],
    bits2: &[i32; NB_EBANDS],
    thresh: &[i32; NB_EBANDS],
    cap: &[i32; NB_EBANDS],
    mut total: i32,
    skip_rsv: i32,
    mut intensity_rsv: i32,
    mut dual_stereo_rsv: i32,
    c: usize,
    lm: usize,
    ec: &mut RangeDecoder,
) -> Allocation {
    let ci = c as i32;
    let alloc_floor = ci << BITRES;
    let stereo = i32::from(c > 1);
    let log_m = (lm as i32) << BITRES;
    let mut lo = 0i32;
    let mut hi = 1i32 << ALLOC_STEPS;
    for _ in 0..ALLOC_STEPS {
        let mid = (lo + hi) >> 1;
        let mut psum = 0i32;
        let mut done = false;
        for j in (start..end).rev() {
            let tmp = bits1[j] + ((mid * bits2[j]) >> ALLOC_STEPS);
            if tmp >= thresh[j] || done {
                done = true;
                psum += tmp.min(cap[j]);
            } else if tmp >= alloc_floor {
                psum += alloc_floor;
            }
        }
        if psum > total {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    let mut psum = 0i32;
    let mut done = false;
    let mut bits = [0i32; NB_EBANDS];
    for j in (start..end).rev() {
        let mut tmp = bits1[j] + ((lo * bits2[j]) >> ALLOC_STEPS);
        if tmp < thresh[j] && !done {
            tmp = if tmp >= alloc_floor { alloc_floor } else { 0 };
        } else {
            done = true;
        }
        tmp = tmp.min(cap[j]);
        bits[j] = tmp;
        psum += tmp;
    }

    // Decide which bands to skip, working backwards from the end.
    let mut coded_bands = end;
    loop {
        let j = coded_bands - 1;
        if j <= skip_start {
            total += skip_rsv;
            break;
        }
        let mut left = total - psum;
        let span = eb(coded_bands) - eb(start);
        let percoeff = udiv(left, span);
        left = left.wrapping_sub(span.wrapping_mul(percoeff));
        let rem = (left - (eb(j) - eb(start))).max(0);
        let band_width = eb(coded_bands) - eb(j);
        let mut band_bits = bits[j]
            .wrapping_add(percoeff.wrapping_mul(band_width))
            .wrapping_add(rem);
        if band_bits >= thresh[j].max(alloc_floor + (1 << BITRES)) {
            if ec.bit_logp(1) {
                break;
            }
            psum += 1 << BITRES;
            band_bits -= 1 << BITRES;
        }
        psum -= bits[j] + intensity_rsv;
        if intensity_rsv > 0 {
            intensity_rsv = i32::from(LOG2_FRAC_TABLE[j - start]);
        }
        psum += intensity_rsv;
        if band_bits >= alloc_floor {
            psum += alloc_floor;
            bits[j] = alloc_floor;
        } else {
            bits[j] = 0;
        }
        coded_bands -= 1;
    }

    let intensity = if intensity_rsv > 0 {
        start + ec.uint((coded_bands + 1 - start) as u32) as usize
    } else {
        0
    };
    if intensity <= start {
        total += dual_stereo_rsv;
        dual_stereo_rsv = 0;
    }
    let dual_stereo = if dual_stereo_rsv > 0 {
        ec.bit_logp(1)
    } else {
        false
    };

    // Allocate the remaining bits.
    let mut left = total - psum;
    let span = eb(coded_bands) - eb(start);
    let percoeff = udiv(left, span);
    left = left.wrapping_sub(span.wrapping_mul(percoeff));
    for j in start..coded_bands {
        bits[j] = bits[j].wrapping_add(percoeff.wrapping_mul(eb(j + 1) - eb(j)));
    }
    for j in start..coded_bands {
        let tmp = left.min(eb(j + 1) - eb(j));
        bits[j] += tmp;
        left -= tmp;
    }

    let mut ebits = [0i32; NB_EBANDS];
    let mut fine_priority = [0i32; NB_EBANDS];
    let mut balance = 0i32;
    let mut j = start;
    while j < coded_bands {
        let n0 = eb(j + 1) - eb(j);
        let n = n0 << lm;
        let bit = bits[j] + balance;
        let mut excess;
        if n > 1 {
            excess = (bit - cap[j]).max(0);
            bits[j] = bit - excess;
            let den = ci * n + i32::from(c == 2 && n > 2 && !dual_stereo && j < intensity);
            let nclogn = den * (log_n(j) + log_m);
            let mut offset = (nclogn >> 1) - den * FINE_OFFSET;
            if n == 2 {
                offset += den << BITRES >> 2;
            }
            if bits[j] + offset < (den * 2) << BITRES {
                offset += nclogn >> 2;
            } else if bits[j] + offset < (den * 3) << BITRES {
                offset += nclogn >> 3;
            }
            let mut e = (bits[j] + offset + (den << (BITRES - 1))).max(0);
            e = (e / den) >> BITRES;
            if ci * e > bits[j] >> BITRES {
                e = bits[j] >> stereo >> BITRES;
            }
            e = e.min(MAX_FINE_BITS);
            ebits[j] = e;
            fine_priority[j] = i32::from(e * (den << BITRES) >= bits[j] + offset);
            bits[j] -= (ci * e) << BITRES;
        } else {
            excess = (bit - (ci << BITRES)).max(0);
            bits[j] = bit - excess;
            ebits[j] = 0;
            fine_priority[j] = 1;
        }
        if excess > 0 {
            let extra_fine = (excess >> (stereo + BITRES)).min(MAX_FINE_BITS - ebits[j]);
            ebits[j] += extra_fine;
            let extra_bits = (extra_fine * ci) << BITRES;
            fine_priority[j] = i32::from(extra_bits >= excess - balance);
            excess -= extra_bits;
        }
        balance = excess;
        j += 1;
    }
    // The skipped bands use all their bits for fine energy.
    while j < end {
        ebits[j] = bits[j] >> stereo >> BITRES;
        bits[j] = 0;
        fine_priority[j] = i32::from(ebits[j] < 1);
        j += 1;
    }
    Allocation {
        coded_bands,
        intensity,
        dual_stereo,
        balance,
        pulses: bits,
        fine_quant: ebits,
        fine_priority,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pulses_round_trip() {
        assert_eq!(get_pulses(7), 7);
        assert_eq!(get_pulses(8), 8);
        assert_eq!(get_pulses(16), 16);
        assert_eq!(get_pulses(17), 18);
        for band in 0..NB_EBANDS {
            for lm in 0..4 {
                let c = cache(lm + 1, band);
                for q in 1..=i32::from(c[0]) {
                    let b = pulses2bits(band, lm as i32, q);
                    // Pulse counts of equal cost map back to the smallest: same cost.
                    let back = bits2pulses(band, lm as i32, b);
                    assert_eq!(
                        pulses2bits(band, lm as i32, back),
                        b,
                        "band {band} lm {lm} q {q}"
                    );
                }
            }
        }
    }
}
