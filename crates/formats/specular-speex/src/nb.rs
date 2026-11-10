//! Narrowband Speex (8 kHz, also the low band of the wider modes): 20 ms frames of four
//! 5 ms subframes. A frame carries its sub-mode, line spectral pairs, an open-loop pitch and
//! gain where the sub-mode has them; each subframe an adaptive (pitch) contribution and a
//! split-VQ innovation. The excitation is enhanced by a comb filter around the pitch, then
//! shaped by the LPC synthesis filter (the previous subframe's, so the output runs half a
//! subframe behind the excitation) and high-passed.

use crate::tables::*;
use crate::{Bits, Error, Frame, Stereo};

pub(crate) const FRAME: usize = 160;
pub(crate) const SUB: usize = 40;
const SUBS: usize = 4;
const ORDER: usize = 10;
const PITCH_START: i32 = 17;
const PITCH_END: i32 = 144;
/// The excitation history: 2 × the longest pitch, a subframe and some slack before the
/// frame, and 6 samples after it (always zero).
const BUF: usize = FRAME + 2 * PITCH_END as usize + SUB + 12;
const EXC: usize = 2 * PITCH_END as usize + SUB + 6;
const LSP_MARGIN: f32 = 0.002;

/// Bits each wideband sub-mode takes (to skip a wideband layer in a narrowband decode).
const WB_SKIP: [i32; 8] = [4, 36, 112, 192, 352, 4, 4, 4];

#[derive(Clone, Copy)]
enum Lsp {
    Nb,
    Lbr,
}

#[derive(Clone, Copy)]
enum Pitch {
    /// A single tap at the open-loop pitch with the frame's gain.
    Forced,
    /// Three taps: (gain codebook, gain bits, pitch bits).
    Taps(&'static [i8], u32, u32),
}

#[derive(Clone, Copy)]
pub(crate) enum Innov {
    Noise,
    /// (subvector size, subvectors, shapes, shape bits, sign bit).
    Split(usize, usize, &'static [i8], u32, bool),
}

#[derive(Clone, Copy)]
struct Submode {
    lbr_pitch: i32,
    forced_pitch_gain: bool,
    subframe_gain: u8,
    double_codebook: bool,
    lsp: Lsp,
    pitch: Pitch,
    innov: Innov,
    comb_gain: f32,
}

#[allow(clippy::too_many_arguments)]
const fn sm(
    lbr_pitch: i32,
    forced_pitch_gain: bool,
    subframe_gain: u8,
    double_codebook: bool,
    lsp: Lsp,
    pitch: Pitch,
    innov: Innov,
    comb_gain: f32,
) -> Submode {
    Submode {
        lbr_pitch,
        forced_pitch_gain,
        subframe_gain,
        double_codebook,
        lsp,
        pitch,
        innov,
        comb_gain,
    }
}

/// libspeex's `modes.c`: sub-modes 1 to 8.
const SUBMODES: [Option<Submode>; 9] = [
    None,
    Some(sm(
        0,
        true,
        0,
        false,
        Lsp::Lbr,
        Pitch::Forced,
        Innov::Noise,
        -1.0,
    )),
    Some(sm(
        0,
        false,
        0,
        false,
        Lsp::Lbr,
        Pitch::Taps(&GAIN_LBR, 5, 0),
        Innov::Split(10, 4, &EXC_10_16, 4, false),
        0.6,
    )),
    Some(sm(
        -1,
        false,
        1,
        false,
        Lsp::Lbr,
        Pitch::Taps(&GAIN_LBR, 5, 7),
        Innov::Split(10, 4, &EXC_10_32, 5, false),
        0.55,
    )),
    Some(sm(
        -1,
        false,
        1,
        false,
        Lsp::Lbr,
        Pitch::Taps(&GAIN_LBR, 5, 7),
        Innov::Split(8, 5, &EXC_8_128, 7, false),
        0.45,
    )),
    Some(sm(
        -1,
        false,
        3,
        false,
        Lsp::Nb,
        Pitch::Taps(&GAIN_NB, 7, 7),
        Innov::Split(5, 8, &EXC_5_64, 6, false),
        0.25,
    )),
    Some(sm(
        -1,
        false,
        3,
        false,
        Lsp::Nb,
        Pitch::Taps(&GAIN_NB, 7, 7),
        Innov::Split(5, 8, &EXC_5_256, 8, false),
        0.15,
    )),
    Some(sm(
        -1,
        false,
        3,
        true,
        Lsp::Nb,
        Pitch::Taps(&GAIN_NB, 7, 7),
        Innov::Split(5, 8, &EXC_5_64, 6, false),
        0.05,
    )),
    Some(sm(
        0,
        true,
        0,
        false,
        Lsp::Lbr,
        Pitch::Forced,
        Innov::Split(20, 2, &EXC_20_32, 5, false),
        0.5,
    )),
];

const GAIN_SCAL3: [f32; 8] = [
    0.061130, 0.163546, 0.310413, 0.428220, 0.555887, 0.719055, 0.938694, 1.326874,
];
const GAIN_SCAL1: [f32; 2] = [0.70469, 1.05127];

/// libspeex's linear congruential noise, uniform with deviation `std`.
pub(crate) fn rand(std: f32, seed: &mut u32) -> f32 {
    *seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    let f = f32::from_bits(0x3f80_0000 | (*seed & 0x007f_ffff)) - 1.5;
    3.4642 * std * f
}

pub(crate) fn rms(x: &[f32]) -> f32 {
    let sum: f32 = x.iter().map(|v| v * v).sum();
    (0.1 + sum / x.len() as f32).sqrt()
}

/// Sums of products, four at a time (as libspeex adds them).
fn inner(x: &[f32], y: &[f32]) -> f32 {
    let mut sum = 0.0f32;
    for (a, b) in x.chunks_exact(4).zip(y.chunks_exact(4)) {
        let part = a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3];
        sum += part;
    }
    sum
}

/// libspeex's cosine approximation for the LSP to LPC conversion.
#[allow(clippy::excessive_precision)]
fn lsp_cos(mut x: f32) -> f32 {
    const C1: f32 = 0.9999932946;
    const C2: f32 = -0.4999124376;
    const C3: f32 = 0.0414877472;
    const C4: f32 = -0.0012712095;
    let neg = x >= std::f32::consts::FRAC_PI_2;
    if neg {
        x = std::f32::consts::PI - x;
    }
    x *= x;
    let v = C1 + x * (C2 + x * (C3 + C4 * x));
    if neg { -v } else { v }
}

/// LSPs (radians) to the LPC filter's coefficients (without the leading 1).
pub(crate) fn lsp_to_lpc(freq: &[f32], ak: &mut [f32]) {
    let order = freq.len();
    let m = order / 2;
    let mut w = vec![0f32; 4 * m + 2];
    let x: Vec<f32> = freq.iter().map(|&f| lsp_cos(f)).collect();
    let (mut xin1, mut xin2) = (1.0f32, 1.0f32);
    for j in 0..=order {
        for i in 0..m {
            let n = 4 * i;
            let xout1 = xin1 - 2.0 * x[2 * i] * w[n] + w[n + 1];
            let xout2 = xin2 - 2.0 * x[2 * i + 1] * w[n + 2] + w[n + 3];
            w[n + 1] = w[n];
            w[n + 3] = w[n + 2];
            w[n] = xin1;
            w[n + 2] = xin2;
            xin1 = xout1;
            xin2 = xout2;
        }
        let last = 4 * (m - 1) + 3;
        let xout1 = xin1 + w[last + 1];
        let xout2 = xin2 - w[last + 2];
        if j > 0 {
            ak[j - 1] = (xout1 + xout2) * 0.5;
        }
        w[last + 1] = xin1;
        w[last + 2] = xin2;
        xin1 = 0.0;
        xin2 = 0.0;
    }
}

/// The LSPs of subframe `sub` of `subs`, between the last frame's and this one's, kept
/// `margin` apart.
pub(crate) fn lsp_interpolate(
    old: &[f32],
    new: &[f32],
    out: &mut [f32],
    sub: usize,
    subs: usize,
    margin: f32,
) {
    let len = out.len();
    let t = (1.0 + sub as f32) / subs as f32;
    for i in 0..len {
        out[i] = (1.0 - t) * old[i] + t * new[i];
    }
    if out[0] < margin {
        out[0] = margin;
    }
    if out[len - 1] > std::f32::consts::PI - margin {
        out[len - 1] = (std::f64::consts::PI - f64::from(margin)) as f32;
    }
    for i in 1..len - 1 {
        if out[i] < out[i - 1] + margin {
            out[i] = out[i - 1] + margin;
        }
        if out[i] > out[i + 1] - margin {
            out[i] = 0.5 * (out[i] + out[i + 1] - margin);
        }
    }
}

/// The all-pole synthesis filter `1 / (1 + Σ den z⁻ⁱ)` with its memory, in place.
pub(crate) fn iir(x: &mut [f32], den: &[f32], mem: &mut [f32]) {
    let ord = den.len();
    for v in x.iter_mut() {
        let yi = *v + mem[0];
        let nyi = -yi;
        for j in 0..ord - 1 {
            mem[j] = mem[j + 1] + den[j] * nyi;
        }
        mem[ord - 1] = den[ord - 1] * nyi;
        *v = yi;
    }
}

/// The output high-pass (`filters.c`'s second-order sections: 1 narrowband, 3 wideband).
fn highpass(x: &mut [f32], wideband: bool, mem: &mut [f32; 2]) {
    let (den, num): ([f32; 3], [f32; 3]) = if wideband {
        ([1.0, -1.97226, 0.97332], [0.98645, -1.97277, 0.98645])
    } else {
        ([1.0, -1.92683, 0.93071], [0.96446, -1.92879, 0.96446])
    };
    for v in x.iter_mut() {
        let xi = *v;
        let vout = num[0] * xi + mem[0];
        mem[0] = mem[1] + num[1] * xi + -den[1] * vout;
        mem[1] = num[2] * xi + -den[2] * vout;
        *v = vout;
    }
}

/// `filters.c`'s fractional delays for the enhancer's pitch interpolation.
#[allow(clippy::excessive_precision)]
const SHIFT_FILT: [[f32; 7]; 3] = [
    [
        -0.011915, 0.046995, -0.152373, 0.614108, 0.614108, -0.152373, 0.046995,
    ],
    [
        -0.0324855, 0.0859768, -0.2042986, 0.9640297, 0.2086420, -0.0302054, -0.0063646,
    ],
    [
        -0.0063646, -0.0302054, 0.2086420, 0.9640297, -0.2042986, 0.0859768, -0.0324855,
    ],
];

/// The excitation `pitch` back (or ahead, negative), at the fractional delay around it that
/// correlates best; `exc` is the buffer, `at` the first sample.
fn interp_pitch(buf: &[f32], at: usize, out: &mut [f32], pitch: i32, len: usize) {
    let idx = |i: isize| -> f32 { buf.get((at as isize + i) as usize).copied().unwrap_or(0.0) };
    let window = |off: isize| -> Vec<f32> { (0..len as isize).map(|i| idx(i + off)).collect() };
    let x = window(0);
    let mut corr = [[0f32; 7]; 4];
    for i in 0..7 {
        corr[0][i] = inner(&x, &window(-(pitch as isize) - 3 + i as isize));
    }
    for i in 0..3 {
        for j in 0..7 {
            let i1 = (3 - j as isize).max(0) as usize;
            let i2 = (10 - j).min(7);
            let mut tmp = 0.0;
            for k in i1..i2 {
                tmp += SHIFT_FILT[i][k] * corr[0][j + k - 3];
            }
            corr[i + 1][j] = tmp;
        }
    }
    let (mut maxi, mut maxj, mut maxcorr) = (0, 0, corr[0][0]);
    for i in 0..4 {
        for j in 0..7 {
            if corr[i][j] > maxcorr {
                maxcorr = corr[i][j];
                maxi = i;
                maxj = j;
            }
        }
    }
    let lag = (pitch - maxj as i32 + 3) as isize;
    for i in 0..len {
        out[i] = if maxi > 0 {
            (0..7)
                .map(|k| idx(i as isize - lag + k - 3) * SHIFT_FILT[maxi - 1][k as usize])
                .sum()
        } else {
            idx(i as isize - lag)
        };
    }
}

/// The decoder's perceptual enhancement: a comb filter on the excitation around the pitch
/// (one and two periods back, or one back and one ahead), its energy kept.
fn multicomb(buf: &[f32], at: usize, out: &mut [f32], pitch: i32, comb_gain: f32) {
    const NSF: usize = 2 * SUB;
    const MAX_PITCH: i32 = 40;
    let exc = &buf[at..at + NSF];
    let mut iexc = [0f32; 2 * NSF];
    interp_pitch(buf, at, &mut iexc[..NSF], pitch, NSF);
    let second = if pitch > MAX_PITCH { 2 * pitch } else { -pitch };
    interp_pitch(buf, at, &mut iexc[NSF..], second, NSF);
    let iexc0_mag = (1000.0 + inner(&iexc[..NSF], &iexc[..NSF])).sqrt();
    let iexc1_mag = (1000.0 + inner(&iexc[NSF..], &iexc[NSF..])).sqrt();
    let exc_mag = (1.0 + inner(exc, exc)).sqrt();
    let corr0 = inner(&iexc[..NSF], exc).max(0.0);
    let corr1 = inner(&iexc[NSF..], exc).max(0.0);
    let pgain1 = if corr0 > iexc0_mag * exc_mag {
        1.0
    } else {
        corr0 / exc_mag / iexc0_mag
    };
    let pgain2 = if corr1 > iexc1_mag * exc_mag {
        1.0
    } else {
        corr1 / exc_mag / iexc1_mag
    };
    let gg1 = exc_mag / iexc0_mag;
    let gg2 = exc_mag / iexc1_mag;
    let (c1, c2) = if comb_gain > 0.0 {
        let c1 = 0.4 * comb_gain + 0.07;
        (c1, 0.5 + 1.72 * (c1 - 0.07))
    } else {
        (0.0, 0.0)
    };
    let g1 = c1 / (1.0 - c2 * pgain1 * pgain1).max(c1);
    let g2 = c1 / (1.0 - c2 * pgain2 * pgain2).max(c1);
    let (gain0, gain1) = if pitch > MAX_PITCH {
        (0.7 * g1 * gg1, 0.3 * g2 * gg2)
    } else {
        (0.6 * g1 * gg1, 0.6 * g2 * gg2)
    };
    for i in 0..NSF {
        out[i] = exc[i] + (gain0 * iexc[i] + gain1 * iexc[i + NSF]);
    }
    let new_ener = rms(&out[..NSF]).max(1.0);
    let old_ener = rms(exc).max(1.0).min(new_ener);
    let ngain = old_ener / new_ener;
    for v in out[..NSF].iter_mut() {
        *v *= ngain;
    }
}

/// Reads a split-VQ innovation into `exc` (added).
pub(crate) fn innovation(innov: Innov, exc: &mut [f32], b: &mut Bits, seed: &mut u32) {
    match innov {
        Innov::Noise => {
            for v in exc.iter_mut() {
                *v = rand(1.0, seed);
            }
        }
        Innov::Split(size, count, cb, bits, have_sign) => {
            let mut ind = [(0usize, false); 8];
            for k in ind.iter_mut().take(count) {
                let sign = have_sign && b.read(1) != 0;
                *k = (b.read(bits) as usize, sign);
            }
            for (i, &(idx, sign)) in ind.iter().take(count).enumerate() {
                let s = if sign { -1.0 } else { 1.0 };
                for j in 0..size {
                    exc[size * i + j] += s * 0.03125 * f32::from(cb[idx * size + j]);
                }
            }
        }
    }
}

/// What a wideband decoder needs from the narrowband one.
pub(crate) struct LowBand<'a> {
    /// The innovation of each subframe (for spectral folding).
    pub innov: &'a mut [f32],
    pub pi_gain: &'a mut [f32; SUBS],
    pub exc_rms: &'a mut [f32; SUBS],
    pub dtx: &'a mut bool,
}

pub(crate) struct Nb {
    buf: Vec<f32>,
    old_qlsp: [f32; ORDER],
    interp_qlpc: [f32; ORDER],
    mem_sp: [f32; ORDER],
    mem_hp: [f32; 2],
    first: bool,
    seed: u32,
    submode: usize,
    voc_m1: f32,
    voc_m2: f32,
    voc_mean: f32,
    voc_offset: i32,
    dtx: bool,
    wideband: bool,
    last_pitch: i32,
}

impl Nb {
    pub(crate) fn new(wideband: bool) -> Nb {
        Nb {
            buf: vec![0.0; BUF],
            old_qlsp: [0.0; ORDER],
            interp_qlpc: [0.0; ORDER],
            mem_sp: [0.0; ORDER],
            mem_hp: [0.0; 2],
            first: true,
            seed: 1000,
            submode: 5,
            voc_m1: 0.0,
            voc_m2: 0.0,
            voc_mean: 0.0,
            voc_offset: 0,
            dtx: false,
            wideband,
            last_pitch: 40,
        }
    }

    /// Decodes a frame into `out` (160 samples, in libspeex's ±32768 scale).
    pub(crate) fn decode(
        &mut self,
        b: &mut Bits,
        stereo: &mut Stereo,
        out: &mut [f32],
        mut low: Option<LowBand>,
    ) -> Result<Frame, Error> {
        // The sub-mode, past wideband layers and in-band messages.
        let m = loop {
            if b.remaining() < 5 {
                return Ok(Frame::End);
            }
            let mut layers = 0;
            while b.read(1) != 0 {
                layers += 1;
                if layers > 2 {
                    return Err(Error::Malformed("more than two wideband layers".into()));
                }
                let sub = b.read(3) as usize;
                let adv = WB_SKIP[sub];
                if adv < 0 {
                    return Err(Error::Malformed("an invalid wideband mode".into()));
                }
                b.skip((adv - 4) as usize);
                if b.remaining() < 5 {
                    return Ok(Frame::End);
                }
            }
            if b.remaining() < 4 {
                return Ok(Frame::End);
            }
            let m = b.read(4) as usize;
            match m {
                15 => return Ok(Frame::End),
                14 => {
                    let id = b.read(4);
                    if id == 9 {
                        stereo.read(b);
                    } else {
                        b.skip(match id {
                            0..2 => 1,
                            2..8 => 4,
                            8..10 => 8,
                            10..12 => 16,
                            12..14 => 32,
                            _ => 64,
                        });
                    }
                }
                13 => {
                    // A user message: its size in 4 bits, then 5 + 8 × size bits.
                    let n = b.read(4) as usize;
                    b.skip(5 + 8 * n);
                }
                9..=12 => return Err(Error::Malformed("an invalid mode".into())),
                m => break m,
            }
        };
        self.submode = m;
        self.buf.copy_within(FRAME..FRAME + EXC + 6, 0);
        let Some(mode) = SUBMODES[m] else {
            // Sub-mode 0: noise through the last filter, widened.
            let mut lpc = [0f32; ORDER];
            let mut g = 0.93f32;
            for (o, &a) in lpc.iter_mut().zip(&self.interp_qlpc) {
                *o = g * a;
                g *= 0.93;
            }
            let gain = rms(&self.buf[EXC..EXC + FRAME]);
            for i in 0..FRAME {
                self.buf[EXC + i] = rand(gain, &mut self.seed);
            }
            self.first = true;
            out[..FRAME].copy_from_slice(&self.buf[EXC..EXC + FRAME]);
            iir(&mut out[..FRAME], &lpc, &mut self.mem_sp);
            if let Some(l) = low.as_mut() {
                l.innov[..FRAME].fill(0.0);
                *l.dtx = self.dtx;
                for s in 0..SUBS {
                    l.exc_rms[s] = rms(&self.buf[EXC + s * SUB..EXC + (s + 1) * SUB]);
                }
            }
            return Ok(Frame::Decoded);
        };
        // Line spectral pairs.
        let mut qlsp = [0f32; ORDER];
        for (i, q) in qlsp.iter_mut().enumerate() {
            *q = 0.25 * i as f32 + 0.25;
        }
        let add = |q: &mut [f32], cb: &[i8], dim: usize, scale: f32, b: &mut Bits| {
            let id = b.read(6) as usize;
            for (i, v) in q.iter_mut().enumerate() {
                *v += scale * f32::from(cb[id * dim + i]);
            }
        };
        add(&mut qlsp, &LSP_NB, 10, 0.0039062, b);
        match mode.lsp {
            Lsp::Nb => {
                add(&mut qlsp[..5], &LSP_NB_LOW1, 5, 0.0019531, b);
                add(&mut qlsp[..5], &LSP_NB_LOW2, 5, 0.00097656, b);
                add(&mut qlsp[5..], &LSP_NB_HIGH1, 5, 0.0019531, b);
                add(&mut qlsp[5..], &LSP_NB_HIGH2, 5, 0.00097656, b);
            }
            Lsp::Lbr => {
                add(&mut qlsp[..5], &LSP_NB_LOW1, 5, 0.0019531, b);
                add(&mut qlsp[5..], &LSP_NB_HIGH1, 5, 0.0019531, b);
            }
        }
        if self.first {
            self.old_qlsp = qlsp;
        }
        let ol_pitch = if mode.lbr_pitch != -1 {
            PITCH_START + b.read(7) as i32
        } else {
            0
        };
        let ol_pitch_coef = if mode.forced_pitch_gain {
            0.066667 * b.read(4) as f32
        } else {
            0.0
        };
        let ol_gain = (b.read(5) as f32 / 3.5).exp();
        if m == 1 {
            self.dtx = b.read(4) == 15;
        } else if m > 1 {
            self.dtx = false;
        }
        let mut best_pitch = 40i32;
        let mut best_gain = 0f32;
        for sub in 0..SUBS {
            let offset = sub * SUB;
            let e = EXC + offset;
            self.buf[e..e + SUB].fill(0.0);
            let (pit_min, pit_max) = if mode.lbr_pitch != -1 {
                (ol_pitch, ol_pitch)
            } else {
                (PITCH_START, PITCH_END)
            };
            let _ = pit_max;
            let mut exc32 = [0f32; SUB];
            let mut pitch = pit_min;
            let gains: [f32; 3] = match mode.pitch {
                Pitch::Forced => {
                    let coef = ol_pitch_coef.min(0.99);
                    let start = pit_min as usize;
                    for i in 0..SUB {
                        exc32[i] = self.buf[e + i - start] * coef;
                        self.buf[e + i] = exc32[i];
                    }
                    [0.0, coef, 0.0]
                }
                Pitch::Taps(cdbk, gain_bits, pitch_bits) => {
                    pitch = b.read(pitch_bits) as i32 + pit_min;
                    let gi = b.read(gain_bits) as usize;
                    let g = |k: usize| 0.015625 * f32::from(cdbk[gi * 4 + k]) + 0.5;
                    let gain = [g(0), g(1), g(2)];
                    for i in 0..3 {
                        let pp = pitch + 1 - i as i32;
                        let tmp1 = (SUB as i32).min(pp).max(0) as usize;
                        for j in 0..tmp1 {
                            exc32[j] += gain[2 - i] * self.buf[(e + j) - pp as usize];
                        }
                        let tmp3 = (SUB as i32).min(pp + pitch).max(0) as usize;
                        for j in tmp1..tmp3 {
                            exc32[j] += gain[2 - i] * self.buf[(e + j) - (pp + pitch) as usize];
                        }
                    }
                    gain
                }
            };
            for v in exc32.iter_mut() {
                *v = if v.is_nan() {
                    0.0
                } else {
                    v.clamp(-32000.0, 32000.0)
                };
            }
            let tmp = gains[1].abs()
                + if gains[0] > 0.0 {
                    gains[0]
                } else {
                    -0.5 * gains[0]
                }
                + if gains[2] > 0.0 {
                    gains[2]
                } else {
                    -0.5 * gains[2]
                };
            let bp = best_pitch;
            let near = |a: i32, b: i32, k: i32| (a - b).abs() < k;
            if (tmp > best_gain
                && (2 * bp - pitch).abs() >= 3
                && (3 * bp - pitch).abs() >= 4
                && (4 * bp - pitch).abs() >= 5)
                || (tmp > 0.6 * best_gain
                    && (near(bp, 2 * pitch, 3) || near(bp, 3 * pitch, 4) || near(bp, 4 * pitch, 5)))
                || (0.67 * tmp > best_gain
                    && (near(2 * bp, pitch, 3) || near(3 * bp, pitch, 4) || near(4 * bp, pitch, 5)))
            {
                best_pitch = pitch;
                if tmp > best_gain {
                    best_gain = tmp;
                }
            }
            // The innovation.
            let ener = match mode.subframe_gain {
                3 => GAIN_SCAL3[b.read(3) as usize] * ol_gain,
                1 => GAIN_SCAL1[b.read(1) as usize] * ol_gain,
                _ => ol_gain,
            };
            let mut innov = [0f32; SUB];
            innovation(mode.innov, &mut innov, b, &mut self.seed);
            for v in innov.iter_mut() {
                *v *= ener;
            }
            if mode.double_codebook {
                let mut innov2 = [0f32; SUB];
                innovation(mode.innov, &mut innov2, b, &mut self.seed);
                for (v, w) in innov.iter_mut().zip(innov2) {
                    *v += w * (0.454545 * ener);
                }
            }
            for i in 0..SUB {
                self.buf[e + i] = exc32[i] + innov[i];
            }
            if let Some(l) = low.as_mut() {
                l.innov[offset..offset + SUB].copy_from_slice(&innov);
            }
            if m == 1 {
                self.vocoder(e, ol_pitch, ol_pitch_coef, ol_gain, &innov);
            }
        }
        // Enhancement, half a subframe behind.
        if mode.comb_gain > 0.0 {
            multicomb(
                &self.buf,
                EXC - SUB,
                &mut out[..2 * SUB],
                best_pitch,
                mode.comb_gain,
            );
            multicomb(
                &self.buf,
                EXC + SUB,
                &mut out[2 * SUB..4 * SUB],
                best_pitch,
                mode.comb_gain,
            );
        } else {
            out[..FRAME].copy_from_slice(&self.buf[EXC - SUB..EXC - SUB + FRAME]);
        }
        let mut interp = [0f32; ORDER];
        let mut ak = [0f32; ORDER];
        for sub in 0..SUBS {
            lsp_interpolate(&self.old_qlsp, &qlsp, &mut interp, sub, SUBS, LSP_MARGIN);
            lsp_to_lpc(&interp, &mut ak);
            if let Some(l) = low.as_mut() {
                let mut pi = 1.0f32;
                for i in (0..ORDER).step_by(2) {
                    pi += ak[i + 1] - ak[i];
                }
                l.pi_gain[sub] = pi;
            }
            iir(
                &mut out[sub * SUB..(sub + 1) * SUB],
                &self.interp_qlpc,
                &mut self.mem_sp,
            );
            self.interp_qlpc = ak;
        }
        highpass(&mut out[..FRAME], self.wideband, &mut self.mem_hp);
        if let Some(l) = low.as_mut() {
            *l.dtx = self.dtx;
            for s in 0..SUBS {
                l.exc_rms[s] = rms(&self.buf[EXC + s * SUB..EXC + (s + 1) * SUB]);
            }
        }
        self.old_qlsp = qlsp;
        self.first = false;
        self.last_pitch = best_pitch;
        Ok(Frame::Decoded)
    }

    /// Sub-mode 1's vocoder excitation: pulses at the pitch mixed with the noise.
    fn vocoder(&mut self, e: usize, ol_pitch: i32, coef: f32, ol_gain: f32, innov: &[f32; SUB]) {
        let g = (1.5 * (coef - 0.2)).clamp(0.0, 1.0);
        self.buf[e..e + SUB].fill(0.0);
        while self.voc_offset < SUB as i32 {
            if self.voc_offset >= 0 {
                self.buf[e + self.voc_offset as usize] =
                    (2.0 * ol_pitch as f32).sqrt() * g * ol_gain;
            }
            self.voc_offset += ol_pitch.max(1);
        }
        self.voc_offset -= SUB as i32;
        for i in 0..SUB {
            let exci = self.buf[e + i];
            let v = 0.7 * exci
                + 0.3 * self.voc_m1
                + ((1.0 - 0.85 * g) * innov[i] - 0.15 * g * self.voc_m2);
            self.voc_m1 = exci;
            self.voc_m2 = innov[i];
            self.voc_mean = 0.8 * self.voc_mean + 0.2 * v;
            self.buf[e + i] = v - self.voc_mean;
        }
    }
}
