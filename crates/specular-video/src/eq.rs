//! The graphic equaliser: ten ISO octave bands (31 Hz to 16 kHz), each ±12 dB, built from
//! second-order sections designed with the formulas of R. Bristow-Johnson's "Cookbook
//! formulae for audio EQ biquad filter coefficients": a low shelf for 31 Hz, peaking
//! filters an octave wide for 62 Hz to 8 kHz, a high shelf for 16 kHz. A preamp, and
//! automatic headroom: the loudest boost of the combined response is taken off the level,
//! so a boosted band does not push full-scale sound into the limiter.
//!
//! Filters run in `f64` (transposed direct form II): a 31 Hz band at 48 kHz has poles close
//! to the unit circle, where `f32` state adds audible noise. A flat equaliser is not run at
//! all (see [`Equaliser::is_flat`]): its sound is bit-identical to none.

/// The bands' centre frequencies (Hz), ISO 266 octaves.
pub const BANDS: [f64; 10] = [
    31.25, 62.5, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0,
];
/// A band's and the preamp's range (dB, either way).
pub const MAX_DB: f32 = 12.0;

/// The built-in presets: their ids and band gains (dB, [`BANDS`] order). Names are
/// `eq-preset-<id>` in the UI language.
pub const PRESETS: &[(&str, [f32; 10])] = &[
    ("flat", [0.0; 10]),
    (
        "bass-boost",
        [6.0, 5.0, 4.0, 2.5, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    ),
    (
        "treble-boost",
        [0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 2.5, 4.0, 5.0, 6.0],
    ),
    (
        "vocal",
        [-3.0, -2.0, -1.0, 1.0, 3.0, 4.0, 4.0, 3.0, 1.0, 0.0],
    ),
    (
        "classical",
        [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, -1.5, -3.0, -3.0, -4.0],
    ),
    ("rock", [5.0, 4.0, 2.5, 1.0, -1.0, -1.5, 0.5, 2.5, 3.5, 4.5]),
    (
        "pop",
        [-1.5, -0.5, 1.5, 3.5, 4.5, 3.5, 1.5, 0.0, -1.0, -1.5],
    ),
    ("jazz", [3.0, 2.0, 1.0, 2.0, -1.5, -1.5, 0.0, 1.5, 2.5, 3.0]),
    (
        "loudness",
        [6.0, 4.0, 1.0, 0.0, -1.0, 0.0, -0.5, 1.0, 4.0, 5.0],
    ),
    (
        "spoken-word",
        [-6.0, -4.0, -1.5, 1.0, 3.0, 4.0, 4.0, 2.0, 0.0, -2.0],
    ),
];

/// The gains of built-in preset `id`.
pub fn preset(id: &str) -> Option<[f32; 10]> {
    PRESETS.iter().find(|(p, _)| *p == id).map(|(_, g)| *g)
}

/// Band gains as stored in the settings: ten numbers in dB, separated by spaces.
pub fn format_bands(g: &[f32; 10]) -> String {
    g.iter()
        .map(|v| {
            let v = (v * 10.0).round() / 10.0;
            if v == 0.0 {
                "0".to_owned()
            } else {
                format!("{v}")
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Band gains from [`format_bands`]' text; missing or bad numbers are 0, all clamped to
/// ±[`MAX_DB`].
pub fn parse_bands(s: &str) -> [f32; 10] {
    let mut g = [0.0; 10];
    for (d, v) in g.iter_mut().zip(s.split_whitespace()) {
        *d = v
            .parse::<f32>()
            .ok()
            .filter(|v| v.is_finite())
            .map_or(0.0, |v| v.clamp(-MAX_DB, MAX_DB));
    }
    g
}

/// User presets as stored in the settings: one per line, `name = g1 g2 … g10`.
pub fn parse_user_presets(s: &str) -> Vec<(String, [f32; 10])> {
    s.lines()
        .filter_map(|l| {
            let (name, gains) = l.rsplit_once('=')?;
            let name = name.trim();
            (!name.is_empty()).then(|| (name.to_owned(), parse_bands(gains)))
        })
        .collect()
}

/// [`parse_user_presets`]' text of `presets`.
pub fn format_user_presets(presets: &[(String, [f32; 10])]) -> String {
    presets
        .iter()
        .map(|(n, g)| {
            format!(
                "{} = {}",
                n.replace(['\n', '='], " ").trim(),
                format_bands(g)
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// What the equaliser is set to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EqSettings {
    /// Band gains, dB.
    pub bands: [f32; 10],
    /// Level before the bands, dB (the automatic headroom comes on top).
    pub preamp: f32,
}

impl EqSettings {
    pub const FLAT: EqSettings = EqSettings {
        bands: [0.0; 10],
        preamp: 0.0,
    };
}

/// A second-order section's coefficients, normalised (`a0` = 1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Biquad {
    pub b: [f64; 3],
    pub a: [f64; 2],
}

/// The filter shapes of the cookbook used here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    Peak,
    LowShelf,
    HighShelf,
}

impl Biquad {
    pub const IDENTITY: Biquad = Biquad {
        b: [1.0, 0.0, 0.0],
        a: [0.0, 0.0],
    };

    /// A cookbook filter at `f0` Hz for a `rate` Hz stream, `db` gain; `q` is the peak's
    /// quality (shelves use slope 1).
    pub fn design(shape: Shape, f0: f64, q: f64, db: f64, rate: f64) -> Biquad {
        if db == 0.0 {
            return Biquad::IDENTITY;
        }
        // Keep the centre clear of Nyquist (a 16 kHz band on a 32 kHz device).
        let f0 = f0.min(rate * 0.45);
        let a = 10f64.powf(db / 40.0);
        let w0 = std::f64::consts::TAU * f0 / rate;
        let (sn, cs) = w0.sin_cos();
        let (b0, b1, b2, a0, a1, a2) = match shape {
            Shape::Peak => {
                let alpha = sn / (2.0 * q);
                (
                    1.0 + alpha * a,
                    -2.0 * cs,
                    1.0 - alpha * a,
                    1.0 + alpha / a,
                    -2.0 * cs,
                    1.0 - alpha / a,
                )
            }
            Shape::LowShelf | Shape::HighShelf => {
                // Slope S = 1: alpha = sin(w0)/2 · √2.
                let alpha = sn / 2.0 * std::f64::consts::SQRT_2;
                let k = 2.0 * a.sqrt() * alpha;
                if shape == Shape::LowShelf {
                    (
                        a * ((a + 1.0) - (a - 1.0) * cs + k),
                        2.0 * a * ((a - 1.0) - (a + 1.0) * cs),
                        a * ((a + 1.0) - (a - 1.0) * cs - k),
                        (a + 1.0) + (a - 1.0) * cs + k,
                        -2.0 * ((a - 1.0) + (a + 1.0) * cs),
                        (a + 1.0) + (a - 1.0) * cs - k,
                    )
                } else {
                    (
                        a * ((a + 1.0) + (a - 1.0) * cs + k),
                        -2.0 * a * ((a - 1.0) + (a + 1.0) * cs),
                        a * ((a + 1.0) + (a - 1.0) * cs - k),
                        (a + 1.0) - (a - 1.0) * cs + k,
                        2.0 * ((a - 1.0) - (a + 1.0) * cs),
                        (a + 1.0) - (a - 1.0) * cs - k,
                    )
                }
            }
        };
        Biquad {
            b: [b0 / a0, b1 / a0, b2 / a0],
            a: [a1 / a0, a2 / a0],
        }
    }

    /// The magnitude response at `f` Hz (dB) for a `rate` Hz stream.
    pub fn response_db(&self, f: f64, rate: f64) -> f64 {
        let w = std::f64::consts::TAU * f / rate;
        // |B(e^jw)|² / |A(e^jw)|² with z^-1 = e^-jw.
        let mag2 = |c0: f64, c1: f64, c2: f64| {
            let re = c0 + c1 * w.cos() + c2 * (2.0 * w).cos();
            let im = -(c1 * w.sin() + c2 * (2.0 * w).sin());
            re * re + im * im
        };
        let num = mag2(self.b[0], self.b[1], self.b[2]);
        let den = mag2(1.0, self.a[0], self.a[1]);
        10.0 * (num / den).log10()
    }

    pub fn is_identity(&self) -> bool {
        *self == Biquad::IDENTITY
    }
}

/// One section's state for `ch` interleaved channels (transposed direct form II).
#[derive(Clone, Debug)]
pub struct Section {
    pub c: Biquad,
    z: Vec<[f64; 2]>,
}

impl Section {
    pub fn new(c: Biquad, ch: usize) -> Section {
        Section {
            c,
            z: vec![[0.0; 2]; ch.max(1)],
        }
    }

    /// New coefficients, keeping the state (a slider moved while playing: no click).
    pub fn set(&mut self, c: Biquad) {
        self.c = c;
    }

    pub fn reset(&mut self) {
        for z in &mut self.z {
            *z = [0.0; 2];
        }
    }

    /// Filters one sample of channel `c`.
    #[inline]
    pub fn tick(&mut self, c: usize, x: f64) -> f64 {
        let k = &self.c;
        let z = &mut self.z[c];
        let y = k.b[0] * x + z[0];
        z[0] = k.b[1] * x - k.a[0] * y + z[1];
        z[1] = k.b[2] * x - k.a[1] * y;
        y
    }

    /// Filters interleaved frames in place.
    pub fn run(&mut self, buf: &mut [f32]) {
        let ch = self.z.len();
        for f in buf.chunks_exact_mut(ch) {
            for (c, s) in f.iter_mut().enumerate() {
                *s = self.tick(c, f64::from(*s)) as f32;
            }
        }
    }
}

/// A band's filter for `rate`.
fn band(i: usize, db: f32, rate: f64) -> Biquad {
    // An octave wide: Q = √2 / (2^1 − 1).
    const Q: f64 = std::f64::consts::SQRT_2;
    let db = f64::from(db.clamp(-MAX_DB, MAX_DB));
    match i {
        // The shelves' midpoints half an octave inside the end bands: the band's centre is
        // near the shelf's full gain.
        0 => Biquad::design(
            Shape::LowShelf,
            BANDS[0] * std::f64::consts::SQRT_2,
            Q,
            db,
            rate,
        ),
        9 => Biquad::design(
            Shape::HighShelf,
            BANDS[9] / std::f64::consts::SQRT_2,
            Q,
            db,
            rate,
        ),
        _ => Biquad::design(Shape::Peak, BANDS[i], Q, db, rate),
    }
}

/// The equaliser for one stream.
#[derive(Clone, Debug)]
pub struct Equaliser {
    rate: f64,
    sections: Vec<Section>,
    /// Linear level: the preamp with the headroom.
    level: f64,
    settings: EqSettings,
}

impl Equaliser {
    pub fn new(ch: usize, rate: u32, s: EqSettings) -> Equaliser {
        let rate = f64::from(rate.max(1));
        let mut e = Equaliser {
            rate,
            sections: (0..10)
                .map(|_| Section::new(Biquad::IDENTITY, ch))
                .collect(),
            level: 1.0,
            settings: EqSettings::FLAT,
        };
        e.set(s);
        e
    }

    pub fn settings(&self) -> EqSettings {
        self.settings
    }

    /// New gains (the filters' state is kept).
    pub fn set(&mut self, s: EqSettings) {
        self.settings = s;
        for (i, sec) in self.sections.iter_mut().enumerate() {
            sec.set(band(i, s.bands[i], self.rate));
        }
        let pre = f64::from(s.preamp.clamp(-MAX_DB, MAX_DB));
        self.level = 10f64.powf((pre - headroom_db(&s.bands, self.rate)) / 20.0);
    }

    /// Flat bands and no preamp: the sound passes untouched (not run).
    pub fn is_flat(&self) -> bool {
        self.settings.preamp == 0.0 && self.settings.bands.iter().all(|&g| g == 0.0)
    }

    pub fn reset(&mut self) {
        for s in &mut self.sections {
            s.reset();
        }
    }

    /// The combined response at `f` Hz (dB), with the level.
    pub fn response_db(&self, f: f64) -> f64 {
        self.sections
            .iter()
            .map(|s| s.c.response_db(f, self.rate))
            .sum::<f64>()
            + 20.0 * self.level.log10()
    }

    /// Filters interleaved frames in place (nothing when flat).
    pub fn run(&mut self, buf: &mut [f32]) {
        if self.is_flat() {
            return;
        }
        let ch = self.sections[0].z.len();
        let level = self.level;
        let active: Vec<usize> = (0..10)
            .filter(|&i| !self.sections[i].c.is_identity())
            .collect();
        for f in buf.chunks_exact_mut(ch) {
            for (c, s) in f.iter_mut().enumerate() {
                let mut x = f64::from(*s) * level;
                for &i in &active {
                    x = self.sections[i].tick(c, x);
                }
                *s = x as f32;
            }
        }
    }
}

/// The loudest boost of `bands`' combined response (dB, 0 when nothing is boosted), over
/// 20 Hz to 20 kHz (or Nyquist) on a third-of-an-octave grid and every band centre.
pub fn headroom_db(bands: &[f32; 10], rate: f64) -> f64 {
    if bands.iter().all(|&g| g <= 0.0) {
        return 0.0;
    }
    let filters: Vec<Biquad> = (0..10).map(|i| band(i, bands[i], rate)).collect();
    let at = |f: f64| filters.iter().map(|b| b.response_db(f, rate)).sum::<f64>();
    let top = (rate * 0.49).min(20_000.0);
    let mut peak = 0.0f64;
    let mut f = 20.0;
    while f <= top {
        peak = peak.max(at(f));
        f *= 2f64.powf(1.0 / 12.0);
    }
    for &f in BANDS.iter().filter(|&&f| f < top) {
        peak = peak.max(at(f));
    }
    peak.max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The RMS gain (dB) of a sine at `f` through `e`, after it settles.
    fn sine_gain(e: &mut Equaliser, f: f64, rate: u32) -> f64 {
        let n = rate as usize * 2;
        let mut buf: Vec<f32> = (0..n)
            .map(|i| (0.25 * (std::f64::consts::TAU * f * i as f64 / f64::from(rate)).sin()) as f32)
            .collect();
        let input = buf.clone();
        e.run(&mut buf);
        let tail = n / 2;
        let rms = |v: &[f32]| (v[tail..].iter().map(|&s| f64::from(s).powi(2)).sum::<f64>()).sqrt();
        20.0 * (rms(&buf) / rms(&input)).log10()
    }

    #[test]
    fn bands_follow_their_design() {
        for rate in [44_100u32, 48_000] {
            for i in 0..10 {
                for db in [12.0f32, -12.0, 5.0] {
                    let mut g = [0.0; 10];
                    g[i] = db;
                    // Without the headroom: compare the bands themselves.
                    let mut e = Equaliser::new(
                        1,
                        rate,
                        EqSettings {
                            bands: g,
                            preamp: 0.0,
                        },
                    );
                    e.level = 1.0;
                    let f = BANDS[i];
                    let design = e.response_db(f);
                    let got = sine_gain(&mut e, f, rate);
                    assert!(
                        (got - design).abs() < 0.5,
                        "band {i} {db} dB at {rate}: {got} vs {design}"
                    );
                    if (1..9).contains(&i) {
                        // A peak's centre is its gain.
                        assert!((design - f64::from(db)).abs() < 0.01, "{design}");
                    } else {
                        // The shelves give most of their gain at the end bands' centres.
                        let share = design / f64::from(db);
                        assert!(share > 0.7 && share <= 1.0, "{i}: {design}");
                    }
                    // An octave away the band has let go of most of its gain.
                    let away = if i < 9 { f * 2.0 } else { f / 2.0 };
                    if (1..9).contains(&i) {
                        assert!(e.response_db(away).abs() < f64::from(db.abs()) * 0.5);
                    }
                }
            }
        }
    }

    #[test]
    fn headroom_takes_off_the_boost() {
        let mut g = [0.0; 10];
        g[5] = 9.0;
        let e = Equaliser::new(
            2,
            48_000,
            EqSettings {
                bands: g,
                preamp: 0.0,
            },
        );
        // The 1 kHz peak is 9 dB; the level takes 9 dB off, so the response tops at 0 dB.
        assert!(
            (e.response_db(1000.0)).abs() < 0.05,
            "{}",
            e.response_db(1000.0)
        );
        assert!(headroom_db(&[-6.0; 10], 48_000.0) == 0.0);
        // Cuts only: no headroom, and the rest is unity.
        let e = Equaliser::new(
            2,
            48_000,
            EqSettings {
                bands: [-6.0; 10],
                preamp: 0.0,
            },
        );
        assert!(e.response_db(1000.0) < -5.0);
    }

    #[test]
    fn flat_is_untouched() {
        let mut e = Equaliser::new(2, 48_000, EqSettings::FLAT);
        let mut buf: Vec<f32> = (0..2000).map(|i| (i as f32 * 0.37).sin() * 0.3).collect();
        let before = buf.clone();
        e.run(&mut buf);
        assert!(
            buf.iter()
                .zip(&before)
                .all(|(a, b)| a.to_bits() == b.to_bits())
        );
    }

    #[test]
    fn bands_and_presets_round_trip() {
        let g = preset("rock").unwrap();
        assert_eq!(parse_bands(&format_bands(&g)), g);
        assert_eq!(
            parse_bands("99 -40 x"),
            [12.0, -12.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]
        );
        let u = vec![("Car".to_owned(), g), ("Late night".to_owned(), [1.5; 10])];
        assert_eq!(parse_user_presets(&format_user_presets(&u)), u);
        assert_eq!(PRESETS.len(), 10);
    }
}
