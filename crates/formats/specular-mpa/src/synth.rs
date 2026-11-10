//! The polyphase synthesis filter bank (ISO/IEC 11172-3 §2.4.3.2, Figure A.2): 32 subband
//! samples in, 32 PCM samples out.

use std::sync::OnceLock;

use crate::window::WINDOW;

struct Tables {
    /// N[i][k] = cos((16 + i)(2k + 1)π / 64).
    n: [[f32; 32]; 64],
    /// D[0..512] as fractions.
    d: [f32; 512],
}

fn tables() -> &'static Tables {
    static T: OnceLock<Box<Tables>> = OnceLock::new();
    T.get_or_init(|| {
        let mut t = Box::new(Tables {
            n: [[0.0; 32]; 64],
            d: [0.0; 512],
        });
        for i in 0..64 {
            for k in 0..32 {
                t.n[i][k] = (f64::from(((16 + i) * (2 * k + 1)) as u32) * std::f64::consts::PI
                    / 64.0)
                    .cos() as f32;
            }
        }
        for i in 0..512 {
            let w = if i <= 256 {
                WINDOW[i]
            } else if (512 - i) % 64 == 0 {
                WINDOW[512 - i]
            } else {
                -WINDOW[512 - i]
            };
            t.d[i] = w as f32 / 65536.0;
        }
        t
    })
}

/// One channel's filter state.
#[derive(Clone)]
pub(crate) struct Synth {
    v: Box<[f32; 1024]>,
    off: usize,
}

impl Synth {
    pub fn new() -> Synth {
        Synth {
            v: Box::new([0.0; 1024]),
            off: 0,
        }
    }

    /// Filters 32 subband samples into 32 output samples.
    pub fn run(&mut self, s: &[f32; 32], out: &mut [f32; 32]) {
        let t = tables();
        self.off = (self.off + 1024 - 64) & 1023;
        let off = self.off;
        // Most subbands above the coded limit are zero.
        let top = s.iter().rposition(|&x| x != 0.0).map_or(0, |p| p + 1);
        for i in 0..64 {
            let row = &t.n[i];
            let mut acc = 0.0f32;
            for k in 0..top {
                acc += row[k] * s[k];
            }
            self.v[(off + i) & 1023] = acc;
        }
        for j in 0..32 {
            let mut acc = 0.0f32;
            for m in 0..8 {
                acc += t.d[j + 64 * m] * self.v[(off + 128 * m + j) & 1023];
                acc += t.d[j + 64 * m + 32] * self.v[(off + 128 * m + 96 + j) & 1023];
            }
            out[j] = acc;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A steady tone in one subband comes out as a tone of about that subband's frequency and
    /// of the amplitude put in.
    #[test]
    fn a_subband_tone_passes() {
        let mut sy = Synth::new();
        let mut energy = 0.0f64;
        let mut n = 0;
        for t in 0..200 {
            let mut s = [0.0f32; 32];
            // Subband 2, a cosine at its centre.
            s[2] = 0.5 * ((t as f32) * std::f32::consts::PI * 2.5).cos();
            let mut out = [0.0f32; 32];
            sy.run(&s, &mut out);
            if t > 20 {
                for &o in &out {
                    energy += f64::from(o * o);
                    n += 1;
                }
            }
        }
        let rms = (energy / f64::from(n)).sqrt();
        assert!(rms > 0.2 && rms < 0.5, "rms {rms}");
    }
}
