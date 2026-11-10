//! Nellymoser Asao (FLV sound formats 4, 5 and 6), mono: 64-byte blocks of 256 samples. A
//! block starts with 23 band energies (a 6-bit start, then 5-bit steps); the bits of each of
//! the 124 coded lines follow from those energies by a fixed allocation of 198 bits, so the
//! decoder runs the same allocation as the encoder. Two 128-line halves follow, each turned
//! to sound by a 256-point inverse MDCT with a sine window. Lines given no bits are noise at
//! the band's level. Bits are read least significant first.
//!
//! The four tables are data from FFmpeg (MIT; see `nelly_data.rs`); the decoding is ours.

use crate::audio::SoundDecoder;
use crate::nelly_data::{BAND_SIZES, DELTA, DEQUANT, INIT};

const BLOCK: usize = 64;
const LINES: usize = 128;
const FILLED: usize = 124;
const HEADER_BITS: usize = 116;
const DETAIL_BITS: i32 = 198;
const BIT_CAP: i32 = 6;
const BASE_OFF: i32 = 4228;
const BASE_SHIFT: i32 = 19;

/// A left shift by `s`, or a right shift by `-s`.
fn shift_by(v: i32, s: i32) -> i32 {
    if s >= 0 {
        v.wrapping_shl(s as u32)
    } else {
        v >> (-s).min(31)
    }
}

/// Scales `v` up until its top bit is bit 30; the shift (31 for zero).
fn normalise(v: &mut i32) -> i32 {
    if *v == 0 {
        return 31;
    }
    let s = 30 - (31 - v.unsigned_abs().leading_zeros() as i32);
    *v = v.wrapping_shl(s as u32);
    s
}

/// The bits a line gets at energy offset `off`.
fn line_bits(level: i16, off: i32, shift: i32) -> i32 {
    let d = i32::from(level) - off;
    let half = if shift >= 1 {
        d >> (shift - 1)
    } else {
        d << (1 - shift)
    };
    ((half + 1) >> 1).clamp(0, BIT_CAP)
}

fn total_bits(levels: &[i16; FILLED], off: i32, shift: i32) -> i32 {
    levels.iter().map(|&l| line_bits(l, off, shift)).sum()
}

/// The encoder's bit allocation, from the lines' energies (`log2 × 2048`): an energy offset
/// is searched (secant steps, then bisection) so that the lines' bits add up to 198.
fn allocate(energy: &[i32; FILLED]) -> [i32; FILLED] {
    let mut top = energy.iter().copied().max().unwrap_or(0).max(0);
    let mut shift = normalise(&mut top) - 16;
    let mut levels = [0i16; FILLED];
    let mut sum = 0i32;
    for (l, &e) in levels.iter_mut().zip(energy) {
        let v = i32::from(shift_by(e, shift) as i16);
        *l = ((3 * v) >> 2) as i16;
        sum += i32::from(*l);
    }
    shift += 11;
    let base = shift;
    sum -= DETAIL_BITS << base;
    let s = normalise(&mut sum) + base;
    let mut small = (BASE_OFF * (sum >> 16)) >> 15;
    small = shift_by(small, base - (BASE_SHIFT + s - 31));
    let mut bitsum = total_bits(&levels, small, base);
    if bitsum != DETAIL_BITS {
        let mut step = bitsum - DETAIL_BITS;
        let mut k = 0;
        while step.abs() <= 16383 {
            step *= 2;
            k += 1;
        }
        step = (step * BASE_OFF) >> 15;
        step = shift_by(step, base - (BASE_SHIFT + k - 15));
        let (mut last_off, mut last_sum) = (small, bitsum);
        let mut tries = 1;
        while tries < 20 {
            last_off = small;
            small += step;
            last_sum = bitsum;
            bitsum = total_bits(&levels, small, base);
            if (bitsum - DETAIL_BITS) * (last_sum - DETAIL_BITS) <= 0 {
                break;
            }
            tries += 1;
        }
        let (mut big, mut big_sum, mut small_sum);
        if bitsum > DETAIL_BITS {
            big = small;
            big_sum = bitsum;
            small = last_off;
            small_sum = last_sum;
        } else {
            big = last_off;
            big_sum = last_sum;
            small_sum = bitsum;
        }
        while bitsum != DETAIL_BITS && tries <= 19 {
            let mid = (big + small) >> 1;
            bitsum = total_bits(&levels, mid, base);
            if bitsum > DETAIL_BITS {
                big = mid;
                big_sum = bitsum;
            } else {
                small = mid;
                small_sum = bitsum;
            }
            tries += 1;
        }
        if (big_sum - DETAIL_BITS).abs() >= (small_sum - DETAIL_BITS).abs() {
            bitsum = small_sum;
        } else {
            small = big;
            bitsum = big_sum;
        }
    }
    let mut bits = [0i32; FILLED];
    for (b, &l) in bits.iter_mut().zip(&levels) {
        *b = line_bits(l, small, base);
    }
    if bitsum > DETAIL_BITS {
        // Too many: the last line that crosses 198 is cut, the rest get none.
        let mut used = 0;
        let mut i = 0;
        while used < DETAIL_BITS && i < FILLED {
            used += bits[i];
            i += 1;
        }
        if i > 0 {
            bits[i - 1] -= used - DETAIL_BITS;
        }
        bits[i..].fill(0);
    }
    bits
}

/// Reads bits least significant first.
struct Bits<'a> {
    data: &'a [u8],
    at: usize,
}

impl Bits<'_> {
    fn read(&mut self, n: i32) -> u32 {
        let mut v = 0u32;
        for k in 0..n.max(0) as u32 {
            let bit = self
                .data
                .get(self.at / 8)
                .map_or(0, |b| (b >> (self.at % 8)) & 1);
            v |= u32::from(bit) << k;
            self.at += 1;
        }
        v
    }
}

/// A Nellymoser track's decoder (mono).
pub(crate) struct Nellymoser {
    /// −cos(π/128 · (n + ½ + 64)(k + ½)) for the 256 outputs of 128 lines (the sign
    /// FFmpeg's transform gives, which the coded gains assume).
    basis: Vec<f32>,
    window: [f32; 2 * LINES],
    /// The second half of the last inverse transform, windowed.
    overlap: [f32; LINES],
    noise: u32,
}

impl Nellymoser {
    pub(crate) fn new() -> Nellymoser {
        let n = LINES as f64;
        let mut basis = Vec::with_capacity(2 * LINES * LINES);
        for t in 0..2 * LINES {
            for k in 0..LINES {
                let a = std::f64::consts::PI / n * (t as f64 + 0.5 + n / 2.0) * (k as f64 + 0.5);
                basis.push(-a.cos() as f32);
            }
        }
        let mut window = [0f32; 2 * LINES];
        for (t, w) in window.iter_mut().enumerate() {
            *w = (std::f64::consts::PI * (t as f64 + 0.5) / (2.0 * n)).sin() as f32;
        }
        Nellymoser {
            basis,
            window,
            overlap: [0.0; LINES],
            noise: 0x1234_5678,
        }
    }

    fn noise_sign(&mut self) -> f32 {
        // xorshift32: any random signs will do for the noise lines.
        self.noise ^= self.noise << 13;
        self.noise ^= self.noise >> 17;
        self.noise ^= self.noise << 5;
        if self.noise & 1 != 0 { -1.0 } else { 1.0 }
    }

    fn block(&mut self, block: &[u8], out: &mut Vec<f32>) {
        let mut b = Bits { data: block, at: 0 };
        let mut energy = [0i32; FILLED];
        let mut gain = [0f32; FILLED];
        let mut val = 0i32;
        let mut line = 0;
        for (band, &size) in BAND_SIZES.iter().enumerate() {
            val = if band == 0 {
                i32::from(INIT[b.read(6) as usize])
            } else {
                val + i32::from(DELTA[b.read(5) as usize])
            };
            let g = -(f64::from(val) / 2048.0).exp2() / (32768.0 * 8.0);
            for _ in 0..size {
                energy[line] = val;
                gain[line] = g as f32;
                line += 1;
            }
        }
        let bits = allocate(&energy);
        for half in 0..2 {
            b.at = HEADER_BITS + half * DETAIL_BITS as usize;
            let mut coef = [0f32; LINES];
            for j in 0..FILLED {
                coef[j] = if bits[j] <= 0 {
                    std::f32::consts::FRAC_1_SQRT_2 * gain[j] * self.noise_sign()
                } else {
                    let v = b.read(bits[j]) as usize;
                    DEQUANT[(1usize << bits[j]) - 1 + v] * gain[j]
                };
            }
            let mut y = [0f32; 2 * LINES];
            for (t, y) in y.iter_mut().enumerate() {
                let row = &self.basis[t * LINES..(t + 1) * LINES];
                *y = row.iter().zip(&coef).map(|(c, x)| c * x).sum::<f32>() * self.window[t];
            }
            out.extend((0..LINES).map(|t| self.overlap[t] + y[t]));
            self.overlap.copy_from_slice(&y[LINES..]);
        }
    }
}

impl SoundDecoder for Nellymoser {
    fn decode(&mut self, sample: &[u8], out: &mut Vec<f32>) -> Result<(), String> {
        for block in sample.chunks_exact(BLOCK) {
            self.block(block, out);
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.overlap = [0.0; LINES];
    }

    fn preroll(&self) -> usize {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocation_spends_198_bits_at_most_6_a_line() {
        for seed in 0..50u32 {
            let mut s = seed.wrapping_mul(2_654_435_761) | 1;
            let mut val = 3000 + (seed as i32 * 397) % 20000;
            let mut energy = [0i32; FILLED];
            let mut line = 0;
            for &size in &BAND_SIZES {
                s ^= s << 13;
                s ^= s >> 17;
                s ^= s << 5;
                val += i32::from(DELTA[(s % 32) as usize]) / 2;
                for _ in 0..size {
                    energy[line] = val;
                    line += 1;
                }
            }
            let bits = allocate(&energy);
            let total: i32 = bits.iter().sum();
            assert!(bits.iter().all(|b| (0..=6).contains(b)), "{bits:?}");
            assert!(total <= DETAIL_BITS, "{seed}: {total}");
        }
    }
}
