//! Monkey's Audio files from versions 3.80 to 3.94 (file versions 3800–3949), as the BSD
//! reference SDK reads them, reimplemented.
//!
//! - 3930–3949: the current frame layout (one range-coded stream, channels interleaved), with
//!   the predictor of the time: neural-network filters, then a fixed four-tap adaptive
//!   filter and a first-order integrator ([`Pred3930`]).
//! - 3800–3929: each channel's whole frame is entropy coded on its own (adaptive Rice codes
//!   before 3.86, Rice with a running sum until 3.89, the range coder from 3.891), then run
//!   through the "anti-predictor" of the compression level ([`normal_3800`],
//!   [`extra_high_3800`]). Frames before 3.81 need not start on a byte.
//! - Only the normal and extra high levels are read before 3.93: the fast and high levels
//!   of those versions have no sample to check them against.

use crate::predict::NnFilter;
use crate::range::{Bad, Reader, State};

/// The predictor of files 3930–3949, for one channel.
pub(crate) struct Pred3930 {
    nn: Option<NnFilter<i32, i16>>,
    nn1: Option<NnFilter<i32, i16>>,
    m: [i32; 4],
    hist: [i32; 4],
    last: i32,
}

impl Pred3930 {
    /// `None` for a level the version did not have (insane).
    pub fn new(level: u16, version: u16) -> Option<Pred3930> {
        let (nn, nn1) = match level {
            1000 => (None, None),
            2000 => (Some(NnFilter::new(16, 11, version)), None),
            3000 => (Some(NnFilter::new(64, 11, version)), None),
            4000 => (
                Some(NnFilter::new(256, 13, version)),
                Some(NnFilter::new(32, 10, version)),
            ),
            _ => return None,
        };
        let mut p = Pred3930 {
            nn,
            nn1,
            m: [0; 4],
            hist: [0; 4],
            last: 0,
        };
        p.flush();
        Some(p)
    }

    pub fn flush(&mut self) {
        if let Some(f) = &mut self.nn {
            f.flush();
        }
        if let Some(f) = &mut self.nn1 {
            f.flush();
        }
        self.m = [360, 317, -109, 98];
        self.hist = [0; 4];
        self.last = 0;
    }

    pub fn decompress(&mut self, input: i64) -> i32 {
        let mut x = input as i32;
        if let Some(f) = &mut self.nn1 {
            x = f.decompress(x);
        }
        if let Some(f) = &mut self.nn {
            x = f.decompress(x);
        }
        // hist[0] is the newest reconstructed value
        let h = self.hist;
        let p = [
            h[0],
            h[0].wrapping_sub(h[1]),
            h[1].wrapping_sub(h[2]),
            h[2].wrapping_sub(h[3]),
        ];
        let dot = p
            .iter()
            .zip(&self.m)
            .fold(0i32, |a, (&p, &m)| a.wrapping_add(p.wrapping_mul(m)));
        let v = x.wrapping_add(dot >> 9);
        // each weight moves against the sign of the residual, by the sign of its input
        let s = i32::from(x < 0) - i32::from(x > 0);
        for (m, &p) in self.m.iter_mut().zip(&p) {
            *m = m.wrapping_add((((p >> 30) & 2) - 1) * s);
        }
        self.hist = [v, h[0], h[1], h[2]];
        let out = v.wrapping_add(self.last.wrapping_mul(31) >> 5);
        self.last = out;
        out
    }
}

/// Bit length of `x` (0 for 0): the Rice parameter of a running mean.
fn bit_len(x: u32) -> u32 {
    32 - x.leading_zeros()
}

/// A Rice code: a unary count of zeros ended by a one, then `k` bits.
fn rice(rd: &mut Reader, k: u32) -> Result<u32, Bad> {
    let q = rd.unary()?;
    Ok(if k == 0 { q } else { (q << k) | rd.bits(k) })
}

/// An unsigned code folded to signed: odd up, even down.
fn signed(v: u32) -> i32 {
    if v & 1 != 0 {
        (v >> 1) as i32 + 1
    } else {
        -((v >> 1) as i32)
    }
}

/// The running-sum bounds of the oldest Rice coder (before 3.86), by `k`.
fn old_min(k: u32) -> u32 {
    match k {
        0 => 0,
        1..=25 => 1 << (k + 6),
        _ => 0,
    }
}
fn old_max(k: u32) -> u32 {
    if k <= 24 { 1 << (k + 7) } else { 0 }
}

/// The running-sum bounds of the Rice coder of 3.86–3.89 (and the range coder's), by `k`.
fn new_min(k: u32) -> u32 {
    match k {
        1..=27 => 1 << (k + 4),
        _ => 0,
    }
}
fn new_max(k: u32) -> u32 {
    if k <= 26 { 1 << (k + 5) } else { 0 }
}

/// One channel's residuals of a frame, before 3.86: the first five at k = 10, the next up
/// to 64 with k from their running mean, then k kept between bounds by a sum over a
/// 64-value window.
fn rice_old(rd: &mut Reader, out: &mut [i32]) -> Result<(), Bad> {
    let n = out.len();
    let mut raw = vec![0u32; n];
    for v in raw.iter_mut().take(n.min(5)) {
        *v = rice(rd, 10)?;
    }
    if n > 5 {
        let mut sum: u32 = raw[..5].iter().fold(0u32, |a, &v| a.wrapping_add(v));
        let mut k = bit_len(sum / 10);
        for (q, r) in raw.iter_mut().enumerate().take(n.min(64)).skip(5) {
            *r = rice(rd, k)?;
            sum = sum.wrapping_add(*r);
            k = bit_len(sum / (q as u32 + 1) / 2);
        }
        if n > 64 {
            k = bit_len(sum >> 7);
            let (mut kmin, mut kmax) = (old_min(k), old_max(k));
            for q in 64..n {
                raw[q] = rice(rd, k)?;
                sum = sum.wrapping_add(raw[q]).wrapping_sub(raw[q - 64]);
                if sum < kmin || sum >= kmax {
                    if sum < kmin {
                        loop {
                            k = k.checked_sub(1).ok_or(Bad)?;
                            if sum >= old_min(k) {
                                break;
                            }
                        }
                    } else {
                        while old_max(k + 1) != 0 {
                            k += 1;
                            if sum < old_max(k) {
                                break;
                            }
                        }
                    }
                    kmin = old_min(k);
                    kmax = old_max(k);
                }
            }
        }
    }
    for (o, &v) in out.iter_mut().zip(&raw) {
        *o = signed(v);
    }
    Ok(())
}

/// One channel's residuals of a frame, 3.86–3.89: Rice codes with k following a decaying
/// sum; from 3.89 each 16 zeros of the unary part raise k by 4.
fn rice_new(rd: &mut Reader, out: &mut [i32], cap: bool) -> Result<(), Bad> {
    let (mut k, mut sum) = (10u32, 1024u32 * 16);
    for o in out.iter_mut() {
        let mut q = rd.unary()?;
        if cap {
            while q >= 16 {
                k += 4;
                q -= 16;
            }
        }
        let v = if k != 0 {
            (q.wrapping_shl(k)) | rd.bits(k.min(32))
        } else {
            q
        };
        sum = sum.wrapping_add(v).wrapping_sub((sum.wrapping_add(8)) >> 4);
        k = k.min(31);
        if sum < new_min(k) {
            k = k.saturating_sub(1);
        } else if new_max(k) != 0 && sum >= new_max(k) {
            k += 1;
        }
        *o = signed(v);
    }
    Ok(())
}

/// One channel's residuals of a frame, 3.891–3.929: the range coder, started and finished
/// for each channel.
fn range(rd: &mut Reader, out: &mut [i32]) -> Result<(), Bad> {
    let mut s = State::default();
    rd.start_range();
    for o in out.iter_mut() {
        *o = rd.value(&mut s)? as i32;
    }
    rd.finish_range();
    Ok(())
}

/// One channel's residuals of a frame, by version.
pub(crate) fn residuals(rd: &mut Reader, version: u16, out: &mut [i32]) -> Result<(), Bad> {
    if version < 3860 {
        rice_old(rd, out)
    } else if version <= 3890 {
        rice_new(rd, out, version > 3880)
    } else {
        range(rd, out)
    }
}

/// The old checksum (files without the CRC flag): a Rice code with k = 30.
pub(crate) fn old_checksum(rd: &mut Reader) -> Result<u32, Bad> {
    rice(rd, 30)
}

/// `((x >> 30) & 2) - 1`: 1 for a negative value, else −1.
#[inline]
fn neg1(x: i32) -> i32 {
    ((x >> 30) & 2) - 1
}

/// The shared tail of the 3.80–3.92 anti-predictors: a three-tap adaptive stage on the
/// filtered input, a second-order stage on its output (`c_shift`), then the integrator.
struct Stages {
    m2: i32,
    m3: i32,
    m4: i32,
    m5: i32,
    m6: i32,
    p2: i32,
    p3: i32,
    p4: i32,
    ipp2: i32,
    p7: i32,
    opp: i32,
    c_shift: u32,
}

impl Stages {
    /// Starts from the first `first` input values.
    fn new(input: &[i32], first: usize, c_shift: u32) -> Stages {
        let (a, b, c) = (input[first - 1], input[first - 2], input[first - 3]);
        Stages {
            m2: 64,
            m3: 115,
            m4: 64,
            m5: 740,
            m6: 0,
            p4: a,
            p3: a.wrapping_sub(b) << 1,
            p2: a.wrapping_add(c.wrapping_sub(b) << 3),
            ipp2: b,
            p7: (2i32).wrapping_mul(a).wrapping_sub(b),
            opp: a,
            c_shift,
        }
    }

    /// One value through both stages; `prev` is the previous output.
    #[inline]
    fn run(&mut self, i: i32, prev: i32) -> i32 {
        let dot = self
            .p2
            .wrapping_mul(self.m2)
            .wrapping_add(self.p3.wrapping_mul(self.m3))
            .wrapping_add(self.p4.wrapping_mul(self.m4));
        let mut o = i.wrapping_add(dot >> 11);
        let d2 = neg1(self.p2);
        let d3 = ((self.p3 >> 28) & 8) - 4;
        let d4 = ((self.p4 >> 28) & 8) - 4;
        if i > 0 {
            self.m2 -= d2;
            self.m3 -= d3;
            self.m4 -= d4;
        } else if i < 0 {
            self.m2 += d2;
            self.m3 += d3;
            self.m4 += d4;
        }
        self.p2 = o.wrapping_add(self.ipp2.wrapping_sub(self.p4) << 3);
        self.p3 = o.wrapping_sub(self.p4) << 1;
        self.ipp2 = self.p4;
        self.p4 = o;
        o = o.wrapping_add(
            self.p7
                .wrapping_mul(self.m5)
                .wrapping_sub(self.opp.wrapping_mul(self.m6))
                >> self.c_shift,
        );
        let d5 = ((self.p7 >> 29) & 4) - 2;
        let d6 = neg1(self.opp);
        if self.p4 > 0 {
            self.m5 -= d5;
            self.m6 += d6;
        } else if self.p4 < 0 {
            self.m5 += d5;
            self.m6 -= d6;
        }
        self.p7 = (2i32).wrapping_mul(o).wrapping_sub(self.opp);
        self.opp = o;
        o.wrapping_add(prev.wrapping_mul(31) >> 5)
    }
}

/// The normal level's anti-predictor of 3.80–3.92.
pub(crate) fn normal_3800(input: &[i32], out: &mut [i32]) {
    const FIRST: usize = 4;
    let n = input.len();
    if n < 8 {
        out[..n].copy_from_slice(input);
        return;
    }
    out[..FIRST].copy_from_slice(&input[..FIRST]);
    let mut st = Stages::new(input, FIRST, 10);
    for q in 1..FIRST {
        out[q] = out[q].wrapping_add(out[q - 1]);
    }
    for q in FIRST..n {
        out[q] = st.run(input[q], out[q - 1]);
    }
}

/// The extra high level's anti-predictor of 3.80–3.92: from 3.83 an eight-tap sign-sign
/// filter, then a 128- or 256-tap one (on 16-bit copies of the values), then the stages of
/// the normal level.
pub(crate) fn extra_high_3800(input: &mut [i32], out: &mut [i32], version: u16) {
    let early = version < 3830;
    let taps = if early { 128 } else { 256 };
    let shift = if early { 11 } else { 12 };
    let min_len = if early { 134 } else { 262 };
    let c_shift = if early { 10 } else { 11 };
    let n = input.len();
    if n < min_len {
        out[..n].copy_from_slice(input);
        return;
    }
    out[..taps].copy_from_slice(&input[..taps]);
    let mut st = Stages::new(input, taps, c_shift);
    for q in 1..taps {
        out[q] = out[q].wrapping_add(out[q - 1]);
    }
    let mut short = vec![0i16; n];
    let mut adapt = vec![0i16; n];
    for q in 0..taps {
        short[q] = input[q] as i16;
        adapt[q] = neg1(input[q]) as i16;
    }
    let mut bm = vec![0i16; taps];
    let mut fm = [0i32; 9];
    let mut fp = [0i32; 9];
    for q in taps..n {
        if !early {
            fp[0] = input[q];
            let mut dot = 0i32;
            for j in (1..=8).rev() {
                dot = dot.wrapping_add(fp[j].wrapping_mul(fm[j]));
                if fp[0] > 0 {
                    fm[j] = fm[j].wrapping_add(neg1(fp[j]));
                } else if fp[0] < 0 {
                    fm[j] = fm[j].wrapping_sub(neg1(fp[j]));
                }
                fp[j] = fp[j - 1];
            }
            input[q] = input[q].wrapping_sub(dot >> 9);
        }
        let original = input[q];
        short[q] = original as i16;
        adapt[q] = neg1(original) as i16;
        let w = q - taps;
        let mut dot = 0i32;
        for ((&s, b), &a) in short[w..q].iter().zip(bm.iter_mut()).zip(&adapt[w..q]) {
            dot = dot.wrapping_add(i32::from(s) * i32::from(*b));
            if original > 0 {
                *b = b.wrapping_add(a);
            } else if original < 0 {
                *b = b.wrapping_sub(a);
            }
        }
        input[q] = input[q].wrapping_sub(dot >> shift);
        short[q] = input[q] as i16;
        adapt[q] = neg1(input[q]) as i16;
        out[q] = st.run(input[q], out[q - 1]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_match_the_tables() {
        assert_eq!((old_min(1), old_min(25), old_min(26)), (128, 1 << 31, 0));
        assert_eq!((old_max(0), old_max(24), old_max(25)), (128, 1 << 31, 0));
        assert_eq!((new_min(1), new_min(27), new_min(28)), (32, 1 << 31, 0));
        assert_eq!((new_max(0), new_max(26), new_max(27)), (32, 1 << 31, 0));
        assert_eq!(
            (bit_len(0), bit_len(1), bit_len(2), bit_len(255)),
            (0, 1, 2, 8)
        );
    }

    #[test]
    fn short_frames_pass_through() {
        let mut input = vec![1, 2, 3];
        let mut out = vec![0; 3];
        normal_3800(&input, &mut out);
        assert_eq!(out, [1, 2, 3]);
        extra_high_3800(&mut input, &mut out, 3880);
        assert_eq!(out, [1, 2, 3]);
    }
}
