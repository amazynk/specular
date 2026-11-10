//! What the listener sets for the sound ([`SoundSettings`]) and the device-side part of the
//! chain ([`Chain`]): the equaliser, dialogue boost, balance and mono. See
//! [`crate::audio`] for the whole chain's order.

use std::time::Duration;

use crate::dialogue::Dialogue;
use crate::eq::{EqSettings, Equaliser};
use crate::levelling::Levelling;
use crate::speakers::{DeviceOrder, Speaker};

/// The sound as the listener sets it.
#[derive(Clone, Debug, PartialEq)]
pub struct SoundSettings {
    pub levelling: Levelling,
    /// The equaliser (`None`: off).
    pub eq: Option<EqSettings>,
    /// Dialogue boost at this strength, 0 to 1 (`None`: off).
    pub dialogue: Option<f32>,
    /// −1 (left only) to 1 (right only).
    pub balance: f32,
    /// Both channels play their sum (one-ear listening).
    pub mono: bool,
    /// Crossfade into the next file (zero: none; never between an album's files).
    pub crossfade: Duration,
}

impl Default for SoundSettings {
    fn default() -> SoundSettings {
        SoundSettings {
            levelling: Levelling::default(),
            eq: None,
            dialogue: None,
            balance: 0.0,
            mono: false,
            crossfade: Duration::ZERO,
        }
    }
}

impl SoundSettings {
    /// The equaliser's settings when it changes the sound.
    fn eq_active(&self) -> Option<EqSettings> {
        self.eq
            .filter(|e| e.preamp != 0.0 || e.bands.iter().any(|&g| g != 0.0))
    }
}

/// The equaliser, dialogue boost, balance and mono for the device's channels and rate.
pub(crate) struct Chain {
    ch: usize,
    /// Each device channel's side for balance: −1 left, 1 right, 0 neither (centre, LFE).
    sides: Vec<i8>,
    rate: u32,
    eq: Option<Equaliser>,
    dialogue: Option<Dialogue>,
    balance: f32,
    mono: bool,
    /// The stream carries its own dynamic range control (not compressed again).
    stream_drc: bool,
}

impl Chain {
    pub fn new(ch: usize, rate: u32, order: DeviceOrder) -> Chain {
        let ch = ch.max(1);
        let sides = match order.speakers(ch) {
            Some(sp) => sp
                .iter()
                .map(|s| match s {
                    Speaker::L | Speaker::Ls | Speaker::Lb => -1,
                    Speaker::R | Speaker::Rs | Speaker::Rb => 1,
                    Speaker::C | Speaker::Lfe | Speaker::Cb => 0,
                })
                .collect(),
            // stereo, or a count with no known layout: the first pair
            None => (0..ch)
                .map(|i| [-1, 1].get(i).copied().unwrap_or(0))
                .collect(),
        };
        Chain {
            ch,
            sides,
            rate,
            eq: None,
            dialogue: None,
            balance: 0.0,
            mono: false,
            stream_drc: false,
        }
    }

    pub fn set(&mut self, s: &SoundSettings) {
        match (s.eq_active(), &mut self.eq) {
            (None, _) => self.eq = None,
            (Some(e), Some(have)) => {
                if have.settings() != e {
                    have.set(e);
                }
            }
            (Some(e), None) => self.eq = Some(Equaliser::new(self.ch, self.rate, e)),
        }
        match (s.dialogue, &mut self.dialogue) {
            (None, _) => self.dialogue = None,
            (Some(st), Some(d)) => d.set(self.rate, st),
            (Some(st), None) => {
                let mut d = Dialogue::new(self.ch, self.rate, st);
                d.set_compress(!self.stream_drc);
                self.dialogue = Some(d);
            }
        }
        self.balance = if s.balance.is_finite() {
            s.balance.clamp(-1.0, 1.0)
        } else {
            0.0
        };
        self.mono = s.mono;
    }

    pub fn set_stream_drc(&mut self, on: bool) {
        self.stream_drc = on;
        if let Some(d) = &mut self.dialogue {
            d.set_compress(!on);
        }
    }

    /// Filters start from silence (after a seek).
    pub fn reset(&mut self) {
        if let Some(e) = &mut self.eq {
            e.reset();
        }
        if let Some(d) = &mut self.dialogue {
            d.reset();
        }
    }

    /// The stages may raise the level past full scale (the limiter must run).
    pub fn may_boost(&self) -> bool {
        self.dialogue.is_some() || self.eq.as_ref().is_some_and(|e| e.settings().preamp > 0.0)
    }

    /// The equaliser and dialogue boost, in place (nothing when both are off).
    pub fn shape(&mut self, buf: &mut [f32]) {
        if let Some(e) = &mut self.eq {
            e.run(buf);
        }
        if let Some(d) = &mut self.dialogue {
            d.run(buf);
        }
    }

    /// Mono, then balance, in place (nothing when centred and stereo). Balance turns down
    /// every speaker on the other side (front, side and back pairs), never the centre or LFE.
    pub fn place(&self, buf: &mut [f32]) {
        if self.ch < 2 || (!self.mono && self.balance == 0.0) {
            return;
        }
        let (gl, gr) = ((1.0 - self.balance).min(1.0), (1.0 + self.balance).min(1.0));
        for f in buf.chunks_exact_mut(self.ch) {
            if self.mono && self.ch > 2 {
                // Surround: every speaker plays the sum.
                let m = f.iter().sum::<f32>() / self.ch as f32;
                f.fill(m);
            } else if self.mono {
                let m = (f[0] + f[1]) * 0.5;
                f[0] = m;
                f[1] = m;
            }
            if self.balance != 0.0 {
                for (v, &side) in f.iter_mut().zip(&self.sides) {
                    match side {
                        -1 => *v *= gl,
                        1 => *v *= gr,
                        _ => {}
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Everything on for 48 kHz stereo: the equaliser (every band moved), dialogue boost,
    /// balance, mono, the limiter and a loudness meter take under 3 % of one core.
    #[test]
    fn everything_on_is_cheap() {
        let rate = 48_000;
        let s = SoundSettings {
            eq: Some(EqSettings {
                bands: [3.0, -2.0, 4.0, -1.0, 2.0, -3.0, 1.0, 5.0, -4.0, 6.0],
                preamp: 1.0,
            }),
            dialogue: Some(0.5),
            balance: 0.2,
            mono: true,
            ..SoundSettings::default()
        };
        let mut c = Chain::new(2, rate, DeviceOrder::Wave);
        c.set(&s);
        let mut meter = crate::levelling::Meter::new(2, rate);
        let secs = 5;
        let src: Vec<f32> = (0..rate as usize * secs)
            .flat_map(|i| {
                let t = i as f32 / rate as f32;
                [
                    0.4 * (std::f32::consts::TAU * 220.0 * t).sin(),
                    0.3 * (std::f32::consts::TAU * 3100.0 * t).sin(),
                ]
            })
            .collect();
        let mut best = f64::MAX;
        for _ in 0..3 {
            let mut buf = src.clone();
            let mut lim = 1.0;
            let t = std::time::Instant::now();
            // In device-sized buffers, as the callback runs it.
            for b in buf.chunks_mut(2 * 480) {
                meter.feed(b);
                c.shape(b);
                c.place(b);
                crate::audio::limit(b, 2, &mut lim);
            }
            best = best.min(t.elapsed().as_secs_f64());
            std::hint::black_box(&buf);
        }
        let share = best / secs as f64;
        eprintln!("sound chain: {:.2} % of one core", share * 100.0);
        // 3 % of the sound's length (not judged on an overloaded machine).
        specular_log::timing::assert_within(
            "sound chain, everything on",
            std::time::Duration::from_secs_f64(best),
            std::time::Duration::from_secs_f64(0.03 * secs as f64),
        );
    }

    #[test]
    fn mono_and_balance() {
        let mut c = Chain::new(2, 48_000, DeviceOrder::Wave);
        let mut buf = vec![1.0, 0.0, 0.5, -0.5];
        c.place(&mut buf);
        assert_eq!(buf, [1.0, 0.0, 0.5, -0.5]);
        c.set(&SoundSettings {
            mono: true,
            ..SoundSettings::default()
        });
        c.place(&mut buf);
        assert_eq!(buf, [0.5, 0.5, 0.0, 0.0]);
        c.set(&SoundSettings {
            balance: 0.5,
            ..SoundSettings::default()
        });
        let mut buf = vec![1.0, 1.0];
        c.place(&mut buf);
        assert_eq!(buf, [0.5, 1.0]);
        c.set(&SoundSettings {
            balance: -1.0,
            ..SoundSettings::default()
        });
        let mut buf = vec![1.0, 1.0];
        c.place(&mut buf);
        assert_eq!(buf, [1.0, 0.0]);
    }

    /// Balance turns down every pair's far side on 5.1 and 7.1 in each device order; the
    /// centre and LFE stay.
    #[test]
    fn balance_moves_every_pair() {
        for order in [DeviceOrder::Wave, DeviceOrder::Alsa, DeviceOrder::CoreAudio] {
            for ch in [6, 8] {
                let mut c = Chain::new(ch, 48_000, order);
                c.set(&SoundSettings {
                    balance: 1.0,
                    ..SoundSettings::default()
                });
                let mut buf = vec![1.0; ch];
                c.place(&mut buf);
                let sp = order.speakers(ch).unwrap();
                for (v, s) in buf.iter().zip(sp) {
                    let want = match s {
                        Speaker::L | Speaker::Ls | Speaker::Lb => 0.0,
                        _ => 1.0,
                    };
                    assert_eq!(*v, want, "{order:?} {ch} {s:?}");
                }
            }
        }
    }
}
