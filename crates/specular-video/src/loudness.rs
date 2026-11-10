//! Loudness and true peak by ITU-R BS.1770-4 and EBU R 128 (EBU Tech 3341 and 3342).
//!
//! - K-weighting: the high shelf and the high-pass of BS.1770, made for any sampling rate
//!   from their analogue parameters (at 48 kHz they are the standard's own coefficients).
//! - Loudness of a span: −0.691 + 10·log10(Σ Gᵢ·mean(yᵢ²)), Gᵢ = 1 for the front channels,
//!   1.41 for the surrounds, 0 for the LFE, by the WAVE channel order: quad L R Ls Rs,
//!   5.0 L R C Ls Rs, 5.1 and up L R C LFE Ls Rs ….
//! - The sound is measured in 100 ms steps: momentary loudness is the last 4 steps (400 ms),
//!   short-term the last 30 (3 s). Integrated loudness gates the 400 ms blocks (75 %
//!   overlap) at −70 LUFS, then 10 LU under their mean; the loudness range (LRA) is the
//!   10th to 95th percentile of the short-term values gated at −70 LUFS and 20 LU under
//!   their mean.
//! - True peak: 4× oversampling (BS.1770 annex 2) by a 65-tap windowed-sinc interpolator
//!   whose phase 0 is the samples themselves, so it never reads under the sample peak.

use std::sync::OnceLock;

/// The absolute gate (LUFS).
pub const ABS_GATE: f64 = -70.0;
/// The relative gates of integrated loudness and of the loudness range (LU).
const REL_GATE_I: f64 = -10.0;
const REL_GATE_LRA: f64 = -20.0;
/// Steps a second, and steps in a momentary and a short-term window.
pub const STEPS_PER_S: u32 = 10;
pub const MOMENTARY_STEPS: usize = 4;
pub const SHORT_TERM_STEPS: usize = 30;
/// Oversampling of the true peak, and the interpolator's taps per phase.
const OVER: usize = 4;
const TAPS: usize = 16;
/// The interpolator's delay in input samples (its centre tap).
const CENTRE: usize = TAPS / 2;

/// LUFS of a K-weighted mean square sum (−∞ for silence).
pub fn lufs(energy: f64) -> f64 {
    if energy > 0.0 {
        -0.691 + 10.0 * energy.log10()
    } else {
        f64::NEG_INFINITY
    }
}

/// The K-weighted mean square sum whose loudness is `l` LUFS.
fn energy(l: f64) -> f64 {
    10f64.powf((l + 0.691) / 10.0)
}

/// dB of a linear level (−∞ for zero).
pub fn db(v: f64) -> f64 {
    if v > 0.0 {
        20.0 * v.log10()
    } else {
        f64::NEG_INFINITY
    }
}

/// One biquad section, direct form II transposed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Biquad {
    pub b: [f64; 3],
    /// a1, a2 (a0 = 1).
    pub a: [f64; 2],
    z: [f64; 2],
}

impl Biquad {
    fn new(b: [f64; 3], a: [f64; 2]) -> Biquad {
        Biquad { b, a, z: [0.0; 2] }
    }

    #[inline]
    fn run(&mut self, x: f64) -> f64 {
        let y = self.b[0] * x + self.z[0];
        self.z[0] = self.b[1] * x - self.a[0] * y + self.z[1];
        self.z[1] = self.b[2] * x - self.a[1] * y;
        y
    }

    /// The magnitude response at `f` Hz for sampling rate `rate`.
    fn gain(&self, f: f64, rate: f64) -> f64 {
        let w = 2.0 * std::f64::consts::PI * f / rate;
        // H(z) at z = e^{jw}: (b0 + b1 z⁻¹ + b2 z⁻²) / (1 + a1 z⁻¹ + a2 z⁻²).
        let (c1, s1, c2, s2) = (w.cos(), -w.sin(), (2.0 * w).cos(), -(2.0 * w).sin());
        let nr = self.b[0] + self.b[1] * c1 + self.b[2] * c2;
        let ni = self.b[1] * s1 + self.b[2] * s2;
        let dr = 1.0 + self.a[0] * c1 + self.a[1] * c2;
        let di = self.a[0] * s1 + self.a[1] * s2;
        ((nr * nr + ni * ni) / (dr * dr + di * di)).sqrt()
    }
}

/// The two K-weighting stages for sampling rate `rate`: the head's high shelf (+4 dB above
/// about 1.5 kHz) and the RLB high-pass (about 38 Hz).
pub fn k_weighting(rate: u32) -> [Biquad; 2] {
    let fs = f64::from(rate.max(1));
    let pi = std::f64::consts::PI;
    // Stage 1: high shelf.
    let (f0, g, q) = (1681.974450955533, 3.999843853973347, 0.7071752369554196);
    let k = (pi * f0 / fs).tan();
    let vh = 10f64.powf(g / 20.0);
    let vb = vh.powf(0.4996667741545416);
    let a0 = 1.0 + k / q + k * k;
    let shelf = Biquad::new(
        [
            (vh + vb * k / q + k * k) / a0,
            2.0 * (k * k - vh) / a0,
            (vh - vb * k / q + k * k) / a0,
        ],
        [2.0 * (k * k - 1.0) / a0, (1.0 - k / q + k * k) / a0],
    );
    // Stage 2: high-pass.
    let (f0, q) = (38.13547087602444, 0.5003270373238773);
    let k = (pi * f0 / fs).tan();
    let a0 = 1.0 + k / q + k * k;
    let high = Biquad::new(
        [1.0, -2.0, 1.0],
        [2.0 * (k * k - 1.0) / a0, (1.0 - k / q + k * k) / a0],
    );
    [shelf, high]
}

/// The K-weighting's gain in dB at `f` Hz for sampling rate `rate`.
pub fn k_response_db(rate: u32, f: f64) -> f64 {
    let [a, b] = k_weighting(rate);
    20.0 * (a.gain(f, f64::from(rate)) * b.gain(f, f64::from(rate))).log10()
}

/// Channel `i`'s weight of `ch` (WAVE order, [`crate::speakers::wave_layout`]): the LFE
/// is left out, the surrounds count 1.41 (+1.5 dB). Past eight channels, the first two
/// count 1 and the rest 1.41.
pub fn channel_weight(i: usize, ch: usize) -> f64 {
    match crate::speakers::wave_layout(ch).and_then(|l| l.get(i)) {
        Some(s) => s.loudness_weight(),
        None if i < 2 => 1.0,
        None => 1.41,
    }
}

/// The interpolator's phases 1 to 3: windowed sinc (Kaiser, β 8), each phase's taps summing
/// to 1 (unity gain at DC).
fn phases() -> &'static [[f32; TAPS]; OVER - 1] {
    static P: OnceLock<[[f32; TAPS]; OVER - 1]> = OnceLock::new();
    P.get_or_init(|| {
        let len = OVER * TAPS + 1;
        let c = (len / 2) as f64;
        let beta = 8.0;
        let i0 = |x: f64| {
            // Modified Bessel function of the first kind, order 0 (series).
            let (mut sum, mut term) = (1.0, 1.0);
            for k in 1..40 {
                term *= (x / 2.0) / k as f64;
                sum += term * term;
            }
            sum
        };
        let h = |m: usize| {
            let t = (m as f64 - c) / OVER as f64;
            let sinc = if t == 0.0 {
                1.0
            } else {
                (std::f64::consts::PI * t).sin() / (std::f64::consts::PI * t)
            };
            let r = (m as f64 - c) / c;
            sinc * i0(beta * (1.0 - r * r).max(0.0).sqrt()) / i0(beta)
        };
        let mut out = [[0f32; TAPS]; OVER - 1];
        for (p, taps) in out.iter_mut().enumerate() {
            let p = p + 1;
            let raw: Vec<f64> = (0..TAPS).map(|j| h(OVER * j + p)).collect();
            let sum: f64 = raw.iter().sum();
            for (t, r) in taps.iter_mut().zip(raw) {
                *t = (r / sum) as f32;
            }
        }
        out
    })
}

/// A channel's true-peak meter: each sample in gives the highest level of the signal
/// between the previous sample and this one, [`CENTRE`] samples late.
#[derive(Clone, Debug)]
pub struct TruePeak {
    hist: [f32; TAPS],
    at: usize,
}

impl Default for TruePeak {
    fn default() -> Self {
        TruePeak {
            hist: [0.0; TAPS],
            at: 0,
        }
    }
}

impl TruePeak {
    /// Takes `x`; the absolute true peak around the sample [`CENTRE`] samples back.
    #[inline]
    pub fn push(&mut self, x: f32) -> f32 {
        self.hist[self.at] = x;
        self.at = (self.at + 1) % TAPS;
        // hist[(at + j) % TAPS] is the sample j steps after the oldest.
        let mut peak = self.hist[(self.at + TAPS - 1 - CENTRE) % TAPS].abs();
        for taps in phases() {
            let mut y = 0.0;
            for (j, t) in taps.iter().enumerate() {
                // Tap j multiplies x[n − j].
                y += t * self.hist[(self.at + TAPS - 1 - j) % TAPS];
            }
            peak = peak.max(y.abs());
        }
        peak
    }
}

/// A 100 ms step: the K-weighted, channel-weighted sum of squares and its sample count.
#[derive(Clone, Copy, Debug, Default)]
struct Step {
    sum: f64,
    n: u32,
}

/// What a whole sound measured.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Report {
    /// Integrated loudness (LUFS); `None` when everything is under the gates.
    pub integrated: Option<f64>,
    /// Loudness range (LU); `None` with fewer than two short-term values over the gates.
    pub range: Option<f64>,
    /// Per channel: the highest true peak and sample peak (linear, 1 = full scale).
    pub true_peak: Vec<f32>,
    pub sample_peak: Vec<f32>,
    /// Per 100 ms step (the step ending at (k + 1) / 10 s): momentary and short-term
    /// loudness (LUFS; the first steps over what there is so far; −∞ silent).
    pub momentary: Vec<f32>,
    pub short_term: Vec<f32>,
}

impl Report {
    /// The highest true peak of any channel (dBTP).
    pub fn max_true_peak_db(&self) -> f64 {
        db(f64::from(
            self.true_peak.iter().copied().fold(0.0, f32::max),
        ))
    }

    /// The highest momentary and short-term loudness (LUFS).
    pub fn max_momentary(&self) -> f64 {
        f64::from(
            self.momentary
                .iter()
                .copied()
                .fold(f32::NEG_INFINITY, f32::max),
        )
    }

    pub fn max_short_term(&self) -> f64 {
        f64::from(
            self.short_term
                .iter()
                .copied()
                .fold(f32::NEG_INFINITY, f32::max),
        )
    }
}

/// Measures interleaved sound frame by frame.
pub struct Meter {
    rate: u32,
    ch: usize,
    weights: Vec<f64>,
    filters: Vec<[Biquad; 2]>,
    peaks: Vec<TruePeak>,
    true_peak: Vec<f32>,
    sample_peak: Vec<f32>,
    /// Frames measured, and the frame the current step ends at.
    pos: u64,
    step_end: u64,
    cur: Step,
    steps: Vec<Step>,
}

impl Meter {
    pub fn new(rate: u32, ch: usize) -> Meter {
        let ch = ch.max(1);
        let rate = rate.max(1);
        Meter {
            rate,
            ch,
            weights: (0..ch).map(|i| channel_weight(i, ch)).collect(),
            filters: vec![k_weighting(rate); ch],
            peaks: vec![TruePeak::default(); ch],
            true_peak: vec![0.0; ch],
            sample_peak: vec![0.0; ch],
            pos: 0,
            step_end: Self::end_of(rate, 0),
            cur: Step::default(),
            steps: Vec::new(),
        }
    }

    /// The frame step `k` ends at.
    fn end_of(rate: u32, k: u64) -> u64 {
        ((u128::from(k) + 1) * u128::from(rate) / u128::from(STEPS_PER_S)) as u64
    }

    pub fn channels(&self) -> usize {
        self.ch
    }

    /// Measures one frame (`ch` samples); `tp` gets each channel's true peak around it
    /// (linear, [`CENTRE`] frames late).
    #[inline]
    pub fn push_frame(&mut self, frame: &[f32], tp: &mut [f32]) {
        let mut sum = 0.0;
        for (i, &x) in frame.iter().enumerate().take(self.ch) {
            let [a, b] = &mut self.filters[i];
            let y = b.run(a.run(f64::from(x)));
            sum += self.weights[i] * y * y;
            let p = self.peaks[i].push(x);
            if let Some(t) = tp.get_mut(i) {
                *t = p;
            }
            self.true_peak[i] = self.true_peak[i].max(p);
            self.sample_peak[i] = self.sample_peak[i].max(x.abs());
        }
        self.cur.sum += sum;
        self.cur.n += 1;
        self.pos += 1;
        if self.pos >= self.step_end {
            self.steps.push(self.cur);
            self.cur = Step::default();
            self.step_end = Self::end_of(self.rate, self.steps.len() as u64);
        }
    }

    /// Measures interleaved frames.
    pub fn push(&mut self, pcm: &[f32]) {
        let mut tp = vec![0.0f32; self.ch];
        for f in pcm.chunks_exact(self.ch) {
            self.push_frame(f, &mut tp);
        }
    }

    /// The loudness (energy) of steps `from..to`.
    fn span(steps: &[Step], from: usize, to: usize) -> f64 {
        let (s, n) = steps[from..to]
            .iter()
            .fold((0.0, 0u64), |(s, n), x| (s + x.sum, n + u64::from(x.n)));
        if n == 0 { 0.0 } else { s / n as f64 }
    }

    /// The measurement of everything pushed (whole steps only).
    pub fn report(&self) -> Report {
        let steps = &self.steps;
        let n = steps.len();
        let window = |k: usize, len: usize| Self::span(steps, (k + 1).saturating_sub(len), k + 1);
        let momentary: Vec<f32> = (0..n)
            .map(|k| lufs(window(k, MOMENTARY_STEPS)) as f32)
            .collect();
        let short_term: Vec<f32> = (0..n)
            .map(|k| lufs(window(k, SHORT_TERM_STEPS)) as f32)
            .collect();
        // Integrated: the whole 400 ms blocks.
        let blocks: Vec<f64> = (MOMENTARY_STEPS - 1..n)
            .map(|k| window(k, MOMENTARY_STEPS))
            .collect();
        let integrated = gated_mean(&blocks, REL_GATE_I).map(lufs);
        // Range: the whole 3 s short-term values.
        let st: Vec<f64> = (SHORT_TERM_STEPS - 1..n)
            .map(|k| window(k, SHORT_TERM_STEPS))
            .collect();
        let range = loudness_range(&st);
        Report {
            integrated,
            range,
            true_peak: self.true_peak.clone(),
            sample_peak: self.sample_peak.clone(),
            momentary,
            short_term,
        }
    }
}

/// The mean energy of `blocks` over the absolute gate and `rel` LU under the mean of those.
fn gated_mean(blocks: &[f64], rel: f64) -> Option<f64> {
    let abs = energy(ABS_GATE);
    let over: Vec<f64> = blocks.iter().copied().filter(|&e| e > abs).collect();
    if over.is_empty() {
        return None;
    }
    let mean = over.iter().sum::<f64>() / over.len() as f64;
    let gate = energy(lufs(mean) + rel);
    let kept: Vec<f64> = over.into_iter().filter(|&e| e > gate).collect();
    (!kept.is_empty()).then(|| kept.iter().sum::<f64>() / kept.len() as f64)
}

/// EBU Tech 3342: the spread from the 10th to the 95th percentile of the gated short-term
/// loudness values (energies).
fn loudness_range(st: &[f64]) -> Option<f64> {
    let abs = energy(ABS_GATE);
    let over: Vec<f64> = st.iter().copied().filter(|&e| e > abs).collect();
    if over.is_empty() {
        return None;
    }
    let mean = over.iter().sum::<f64>() / over.len() as f64;
    let gate = energy(lufs(mean) + REL_GATE_LRA);
    let mut kept: Vec<f64> = over.into_iter().filter(|&e| e > gate).map(lufs).collect();
    if kept.len() < 2 {
        return None;
    }
    kept.sort_by(f64::total_cmp);
    let pct = |p: f64| {
        let x = p * (kept.len() - 1) as f64;
        let i = x.floor() as usize;
        let t = x - i as f64;
        kept[i] + (kept[(i + 1).min(kept.len() - 1)] - kept[i]) * t
    };
    Some(pct(0.95) - pct(0.10))
}

#[cfg(test)]
#[path = "loudness_tests.rs"]
mod tests;
