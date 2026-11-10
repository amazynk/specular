//! Upsampling of SILK's 8, 12 or 16 kHz output to 48 kHz (non-normative, RFC 6716 section
//! 4.2.9): polyphase filters from kernels measured against the reference decoder
//! ([`crate::resample_fit`]), whose delays keep SILK and CELT aligned.

/// Streaming integer-ratio upsampler to 48 kHz.
#[derive(Clone)]
pub(crate) struct Resampler {
    pub(crate) up: usize,
    /// Per output phase r: weights for input offsets -(taps-1) ..= 0 relative to sample q.
    pub(crate) phases: Vec<Vec<f32>>,
    /// Input history (the last `taps - 1` samples).
    hist: Vec<f32>,
    pub(crate) taps: usize,
    work: Vec<f32>,
}

impl Resampler {
    pub(crate) fn new(fs_khz: usize) -> Resampler {
        let up = 48 / fs_khz.max(1);
        // The measured kernels (resample_fit.rs): their delays keep SILK aligned with CELT.
        let kernel: &[f32] = match fs_khz {
            8 => &crate::resample_fit::FIT_8,
            12 => &crate::resample_fit::FIT_12,
            _ => &crate::resample_fit::FIT_16,
        };
        #[cfg(test)]
        if let Some(k) = study::KERNEL.with(|k| {
            k.borrow()
                .iter()
                .find(|e| e.0 == fs_khz)
                .map(|e| e.1.clone())
        }) {
            return Resampler::from_kernel(up, &k);
        }
        Resampler::from_kernel(up, kernel)
    }

    fn from_kernel(up: usize, kernel: &[f32]) -> Resampler {
        let taps = kernel.len().div_ceil(up);
        // Output sample up*q + r takes input q - (taps - 1) + idx with weight k[r + up*(taps - 1 - idx)].
        // Each phase scaled to unit gain at DC (the fit leaves them a few 0.1 % off).
        let phases = (0..up)
            .map(|r| {
                let w: Vec<f32> = (0..taps)
                    .map(|idx| {
                        kernel
                            .get(r + up * (taps - 1 - idx))
                            .copied()
                            .unwrap_or(0.0)
                    })
                    .collect();
                let sum: f32 = w.iter().sum();
                w.iter().map(|v| v / sum).collect()
            })
            .collect();
        Resampler {
            up,
            phases,
            hist: vec![0.0; taps - 1],
            taps,
            work: Vec::new(),
        }
    }

    /// Upsamples `inp` into `out` (`out.len() == inp.len() * up`).
    pub(crate) fn process(&mut self, inp: &[f32], out: &mut [f32]) {
        let h = self.taps - 1;
        self.work.clear();
        self.work.extend_from_slice(&self.hist);
        self.work.extend_from_slice(inp);
        for q in 0..inp.len() {
            let win = &self.work[q..q + self.taps];
            for r in 0..self.up {
                out[q * self.up + r] = dot(win, &self.phases[r]);
            }
        }
        let n = self.work.len();
        self.hist.copy_from_slice(&self.work[n - h..]);
    }
}

/// `Σ a[i] b[i]` in eight lanes (vectorises; the kernels are up to 89 taps per phase).
#[inline]
fn dot(a: &[f32], b: &[f32]) -> f32 {
    let mut acc = [0f32; 8];
    let (ac, bc) = (a.chunks_exact(8), b.chunks_exact(8));
    let tail: f32 = ac
        .remainder()
        .iter()
        .zip(bc.remainder())
        .map(|(x, y)| x * y)
        .sum();
    for (x, y) in ac.zip(bc) {
        for k in 0..8 {
            acc[k] += x[k] * y[k];
        }
    }
    acc.iter().sum::<f32>() + tail
}

/// Study aids: the upsampler before the measured kernels, and a per-thread kernel override.
#[cfg(test)]
pub(crate) mod study {
    use std::cell::RefCell;
    use std::f64::consts::PI;

    thread_local! {
        /// Kernels (48 kHz impulse responses) [`super::Resampler::new`] uses instead of the
        /// measured ones, by input rate in kHz.
        pub(crate) static KERNEL: RefCell<Vec<(usize, Vec<f32>)>> = const { RefCell::new(Vec::new()) };
    }

    fn bessel_i0(x: f64) -> f64 {
        let (mut sum, mut term, h) = (1.0, 1.0, x * x / 4.0);
        for k in 1..50 {
            term *= h / (k * k) as f64;
            sum += term;
            if term < 1e-12 * sum {
                break;
            }
        }
        sum
    }

    /// The Kaiser-windowed sinc used before the measured kernels (the base the corrections
    /// are fitted on), as a 48 kHz impulse response with its delay in samples.
    pub(crate) fn kaiser(fs_khz: usize) -> Vec<f32> {
        let up = 48 / fs_khz;
        let (delay, half, cutoff) = match fs_khz {
            8 => (25.8f64, 4.2f64, 0.42f64),
            12 => (33.2, 8.0, 0.45),
            _ => (33.95, 11.0, 0.46),
        };
        let beta = 6.0;
        let i0b = bessel_i0(beta);
        let taps = (delay / up as f64 + half).ceil() as usize + 2;
        let mut h = vec![0f32; up * taps];
        for r in 0..up {
            let pos = (r as f64 - delay) / up as f64;
            let mut ws = vec![0f64; taps];
            for (idx, wv) in ws.iter_mut().enumerate() {
                let tau = pos - (idx as f64 - (taps - 1) as f64);
                if tau.abs() < half {
                    let x = PI * 2.0 * cutoff * tau;
                    let s = if tau == 0.0 { 1.0 } else { x.sin() / x };
                    let ratio = tau / half;
                    *wv = 2.0 * cutoff * s * bessel_i0(beta * (1.0 - ratio * ratio).sqrt()) / i0b;
                }
            }
            let sum: f64 = ws.iter().sum();
            for (idx, v) in ws.iter().enumerate() {
                h[r + up * (taps - 1 - idx)] = (v / sum) as f32;
            }
        }
        h
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dc_passes_with_unit_gain() {
        for fs in [8, 12, 16] {
            let mut r = Resampler::new(fs);
            let inp = vec![0.5f32; fs * 20];
            let mut out = vec![0f32; inp.len() * 48 / fs];
            r.process(&inp, &mut out);
            assert!((out[out.len() - 1] - 0.5).abs() < 1e-4);
        }
    }
}
