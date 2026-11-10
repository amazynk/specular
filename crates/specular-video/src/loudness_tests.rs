//! BS.1770 and EBU Tech 3341 / 3342 cases, made from their published descriptions.

use super::*;

/// A sine of `hz` at `dbfs` (peak) for `secs`, on every one of `ch` channels.
fn sine(rate: u32, ch: usize, parts: &[(f64, f64)], hz: f64) -> Vec<f32> {
    let mut out = Vec::new();
    let mut n = 0u64;
    for &(dbfs, secs) in parts {
        let a = 10f64.powf(dbfs / 20.0);
        let len = (secs * f64::from(rate)).round() as u64;
        for _ in 0..len {
            let v = a * (2.0 * std::f64::consts::PI * hz * n as f64 / f64::from(rate)).sin();
            for _ in 0..ch {
                out.push(v as f32);
            }
            n += 1;
        }
    }
    out
}

fn measure(rate: u32, ch: usize, pcm: &[f32]) -> Report {
    let mut m = Meter::new(rate, ch);
    m.push(pcm);
    m.report()
}

#[test]
fn k_weighting_at_48k_is_the_standards_table() {
    let [s, h] = k_weighting(48_000);
    let near = |a: f64, b: f64| (a - b).abs() < 1e-8;
    assert!(near(s.b[0], 1.53512485958697), "{s:?}");
    assert!(near(s.b[1], -2.69169618940638));
    assert!(near(s.b[2], 1.19839281085285));
    assert!(near(s.a[0], -1.69065929318241));
    assert!(near(s.a[1], 0.73248077421585));
    assert_eq!(h.b, [1.0, -2.0, 1.0]);
    assert!(near(h.a[0], -1.99004745483398), "{h:?}");
    assert!(near(h.a[1], 0.99007225036621));
}

#[test]
fn k_weighting_response() {
    for rate in [44_100, 48_000, 96_000] {
        // About +0.69 dB at 1 kHz (hence −0.691 in the loudness formula).
        let k1 = k_response_db(rate, 997.0);
        assert!((k1 - 0.691).abs() < 0.02, "{rate}: {k1}");
        // The shelf: close to +4 dB in the treble.
        let k10 = k_response_db(rate, 10_000.0);
        assert!((3.8..4.1).contains(&k10), "{rate}: {k10}");
        // The high-pass: low bass falls away.
        let k20 = k_response_db(rate, 20.0);
        assert!(k20 < -10.0, "{rate}: {k20}");
        let k100 = k_response_db(rate, 100.0);
        assert!((-1.5..0.0).contains(&k100), "{rate}: {k100}");
    }
    // Rates agree with each other where both reach.
    for f in [50.0, 500.0, 2000.0, 8000.0] {
        let d = k_response_db(44_100, f) - k_response_db(48_000, f);
        assert!(d.abs() < 0.05, "{f}: {d}");
    }
}

#[test]
fn tech3341_cases_1_to_6() {
    let r = 48_000;
    let i = |parts: &[(f64, f64)]| {
        measure(r, 2, &sine(r, 2, parts, 1000.0))
            .integrated
            .unwrap()
    };
    let cases: [(&[(f64, f64)], f64); 5] = [
        (&[(-23.0, 20.0)], -23.0),
        (&[(-33.0, 20.0)], -33.0),
        (&[(-36.0, 10.0), (-23.0, 60.0), (-36.0, 10.0)], -23.0),
        (
            &[
                (-72.0, 10.0),
                (-36.0, 10.0),
                (-23.0, 60.0),
                (-36.0, 10.0),
                (-72.0, 10.0),
            ],
            -23.0,
        ),
        (&[(-26.0, 20.0), (-20.0, 20.1), (-26.0, 20.0)], -23.0),
    ];
    for (n, (parts, want)) in cases.iter().enumerate() {
        let got = i(parts);
        assert!((got - want).abs() <= 0.1, "case {}: {got}", n + 1);
    }
    // Case 6: 5.0 channels, L R C Ls Rs at −28, −28, −24, −30, −30 dBFS.
    let levels = [-28.0, -28.0, -24.0, -30.0, -30.0];
    let chans: Vec<Vec<f32>> = levels
        .iter()
        .map(|&l| sine(r, 1, &[(l, 20.0)], 1000.0))
        .collect();
    let pcm: Vec<f32> = (0..chans[0].len())
        .flat_map(|k| chans.iter().map(move |c| c[k]))
        .collect();
    let got = measure(r, 5, &pcm).integrated.unwrap();
    assert!((got + 23.0).abs() <= 0.1, "case 6: {got}");
}

#[test]
fn momentary_and_short_term_of_a_steady_tone() {
    let r = 44_100;
    let rep = measure(r, 2, &sine(r, 2, &[(-23.0, 10.0)], 1000.0));
    assert_eq!(rep.momentary.len(), 100);
    // EBU Tech 3341 cases 9 and 10 in spirit: a steady −23 LUFS tone reads −23 on both,
    // once their windows are full.
    for k in 3..100 {
        assert!(
            (rep.momentary[k] + 23.0).abs() < 0.1,
            "{k}: {}",
            rep.momentary[k]
        );
    }
    for k in 29..100 {
        assert!((rep.short_term[k] + 23.0).abs() < 0.1, "{k}");
    }
    assert!((rep.max_momentary() + 23.0).abs() < 0.1);
}

#[test]
fn silence_is_gated_away() {
    let rep = measure(48_000, 2, &vec![0.0; 48_000 * 2 * 5]);
    assert_eq!(rep.integrated, None);
    assert_eq!(rep.range, None);
    assert_eq!(rep.max_true_peak_db(), f64::NEG_INFINITY);
    // Under −70 LUFS is gated too.
    let rep = measure(48_000, 2, &sine(48_000, 2, &[(-75.0, 5.0)], 1000.0));
    assert_eq!(rep.integrated, None);
}

#[test]
fn tech3342_loudness_range_cases_1_to_4() {
    let r = 48_000;
    let cases: [(&[(f64, f64)], f64); 4] = [
        (&[(-20.0, 20.0), (-30.0, 20.0)], 10.0),
        (&[(-20.0, 20.0), (-15.0, 20.0)], 5.0),
        (&[(-40.0, 20.0), (-20.0, 20.0)], 20.0),
        (
            &[
                (-50.0, 20.0),
                (-35.0, 20.0),
                (-20.0, 20.0),
                (-35.0, 20.0),
                (-50.0, 20.0),
            ],
            15.0,
        ),
    ];
    for (n, (parts, want)) in cases.iter().enumerate() {
        let got = measure(r, 2, &sine(r, 2, parts, 1000.0)).range.unwrap();
        // The standard allows ±1 LU.
        assert!((got - want).abs() <= 1.0, "case {}: {got}", n + 1);
    }
}

#[test]
fn true_peak_finds_peaks_between_samples() {
    let r = 48_000.0;
    // Tech 3341's kind of signal: a sine at fs/4 whose samples miss its crests by 45°,
    // 3 dB under them.
    for (hz, phase) in [
        (12_000.0, 45.0f64),
        (8_000.0, 60.0),
        (6_000.0, 22.5),
        (9_600.0, 36.0),
        (997.0, 10.0),
        (15_000.0, 30.0),
        (18_000.0, 15.0),
    ] {
        let a = 0.5f64;
        let mut tp = TruePeak::default();
        let (mut peak, mut sample) = (0.0f32, 0.0f32);
        for n in 0..4800 {
            let x = a * (2.0 * std::f64::consts::PI * hz * n as f64 / r + phase.to_radians()).sin();
            sample = sample.max(x.abs() as f32);
            // Past the interpolator's start.
            let p = tp.push(x as f32);
            if n > 2 * TAPS {
                peak = peak.max(p);
            }
        }
        let err = db(f64::from(peak) / a);
        // BS.1770 / Tech 3341: +0.2 / −0.4 dB.
        assert!((-0.4..=0.2).contains(&err), "{hz} Hz {phase}°: {err:.3} dB");
        assert!(peak >= sample * 0.9999, "{hz}");
    }
    // A lone full-scale sample reads full scale (phase 0 is the samples themselves).
    let mut tp = TruePeak::default();
    let mut peak = 0.0f32;
    for n in 0..40 {
        peak = peak.max(tp.push(if n == 10 { 1.0 } else { 0.0 }));
    }
    assert!(peak >= 1.0, "{peak}");
}

#[test]
fn true_peak_of_a_crest_between_samples_reads_over_full_scale() {
    // Samples at ±0.99 of a sine at fs/4 with 45° phase: the waveform crests at 1.4.
    let a = 0.99 / std::f64::consts::FRAC_1_SQRT_2;
    let mut m = Meter::new(48_000, 1);
    let pcm: Vec<f32> = (0..48_000)
        .map(|n| {
            (a * (std::f64::consts::PI / 2.0 * n as f64 + std::f64::consts::FRAC_PI_4).sin()) as f32
        })
        .collect();
    m.push(&pcm);
    let rep = m.report();
    assert!(rep.sample_peak[0] <= 0.9901);
    let tp = rep.max_true_peak_db();
    assert!((tp - db(a)).abs() < 0.2, "{tp} vs {}", db(a));
    assert!(tp > 2.8);
}

#[test]
fn channel_weights() {
    assert_eq!(channel_weight(0, 1), 1.0);
    assert_eq!(channel_weight(1, 2), 1.0);
    // 5.1: the LFE is left out, the surrounds weigh 1.41.
    let w: Vec<f64> = (0..6).map(|i| channel_weight(i, 6)).collect();
    assert_eq!(w, [1.0, 1.0, 1.0, 0.0, 1.41, 1.41]);
    // 5.0 and quad have no LFE.
    let w: Vec<f64> = (0..5).map(|i| channel_weight(i, 5)).collect();
    assert_eq!(w, [1.0, 1.0, 1.0, 1.41, 1.41]);
    let w: Vec<f64> = (0..4).map(|i| channel_weight(i, 4)).collect();
    assert_eq!(w, [1.0, 1.0, 1.41, 1.41]);
}
