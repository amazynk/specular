//! The polyphase synthesis filter bank Musepack shares with MPEG-1 audio (ISO/IEC 11172-3
//! §2.4.3.2): 32 subband samples in, 32 PCM samples out. Sums are taken in double
//! precision.

use std::sync::OnceLock;

use crate::window::WINDOW;

struct Tables {
    /// cos((16 + i)(2k + 1)π / 64) for the 64 outputs of the matrixing (half are used: row
    /// 16 + j is minus row 16 - j, row 48 + j equals row 48 - j).
    n: [[f64; 32]; 64],
    /// D[0..512] as fractions.
    d: [f64; 512],
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
                t.n[i][k] = (((16 + i) * (2 * k + 1)) as f64 * std::f64::consts::PI / 64.0).cos();
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
            t.d[i] = f64::from(w) / 65536.0;
        }
        t
    })
}

/// One channel's filter state.
#[derive(Clone)]
pub(crate) struct Synth {
    v: Box<[f64; 1024]>,
    off: usize,
}

impl Synth {
    pub(crate) fn new() -> Synth {
        Synth {
            v: Box::new([0.0; 1024]),
            off: 0,
        }
    }

    pub(crate) fn reset(&mut self) {
        self.v.fill(0.0);
        self.off = 0;
    }

    /// Filters 32 subband samples into 32 output samples, written `stride` apart.
    pub(crate) fn run(&mut self, s: &[f32; 32], out: &mut [f32], stride: usize) {
        let t = tables();
        self.off = (self.off + 1024 - 64) & 1023;
        let off = self.off;
        let top = s.iter().rposition(|&x| x != 0.0).map_or(0, |p| p + 1);
        let s64: [f64; 32] = std::array::from_fn(|k| f64::from(s[k]));
        // V[16 + j] = -V[16 - j] (so V[16] is 0) and V[48 + j] = V[48 - j]: 32 rows to sum.
        let mut v = [0.0f64; 64];
        for i in (0..16).chain(33..=48) {
            let row = &t.n[i];
            v[i] = (0..top).map(|k| row[k] * s64[k]).sum();
        }
        for j in 1..=16 {
            v[16 + j] = -v[16 - j];
        }
        for j in 1..16 {
            v[48 + j] = v[48 - j];
        }
        for (i, x) in v.iter().enumerate() {
            self.v[(off + i) & 1023] = *x;
        }
        for j in 0..32 {
            let mut acc = 0.0f64;
            for m in 0..8 {
                acc += t.d[j + 64 * m] * self.v[(off + 128 * m + j) & 1023];
                acc += t.d[j + 64 * m + 32] * self.v[(off + 128 * m + 96 + j) & 1023];
            }
            out[j * stride] = acc as f32;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symmetries_hold() {
        let t = tables();
        for j in 1..16 {
            for k in 0..32 {
                assert!((t.n[16 + j][k] + t.n[16 - j][k]).abs() < 1e-12);
                assert!((t.n[48 + j][k] - t.n[48 - j][k]).abs() < 1e-12);
            }
        }
        assert!(t.n[16].iter().all(|x| x.abs() < 1e-12));
    }

    #[test]
    fn silence_stays_silent() {
        let mut s = Synth::new();
        let mut out = [1.0f32; 32];
        s.run(&[0.0; 32], &mut out, 1);
        assert!(out.iter().all(|&x| x == 0.0));
    }
}
