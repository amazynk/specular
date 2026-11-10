use super::*;

/// The O(n²) DFT in f64.
fn dft(x: &[Complex]) -> Vec<(f64, f64)> {
    let n = x.len();
    (0..n)
        .map(|k| {
            let mut re = 0f64;
            let mut im = 0f64;
            for (j, v) in x.iter().enumerate() {
                let t = -2.0 * PI * ((j * k) % n) as f64 / n as f64;
                let (s, c) = t.sin_cos();
                re += f64::from(v.re) * c - f64::from(v.im) * s;
                im += f64::from(v.re) * s + f64::from(v.im) * c;
            }
            (re, im)
        })
        .collect()
}

/// Deterministic pseudo-random samples in -1..1.
fn noise(n: usize, seed: u64) -> Vec<Complex> {
    let mut s = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    let mut next = || {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        (s >> 40) as f32 / (1u64 << 23) as f32 - 1.0
    };
    (0..n).map(|_| Complex::new(next(), next())).collect()
}

fn max_error(n: usize) -> f64 {
    let x = noise(n, n as u64);
    let want = dft(&x);
    let plan = Fft::new(n);
    let mut got = x.clone();
    let mut scratch = vec![Complex::ZERO; plan.scratch_len()];
    plan.forward(&mut got, &mut scratch);
    let scale = (n as f64).sqrt().max(1.0);
    got.iter()
        .zip(&want)
        .map(|(g, w)| (f64::from(g.re) - w.0).hypot(f64::from(g.im) - w.1) / scale)
        .fold(0.0, f64::max)
}

#[test]
fn matches_the_dft_for_every_length_to_130() {
    for n in 0..=130 {
        let e = max_error(n);
        assert!(e < 2e-5, "n = {n}: error {e}");
    }
}

#[test]
fn matches_the_dft_for_mixed_and_prime_lengths() {
    // 2310 = 2·3·5·7·11, 1009 and 4099 prime (Bluestein), 59·61 (generic radices),
    // 2·127 (Bluestein over an even length).
    for n in [
        1000,
        1024,
        2310,
        1009,
        4099,
        59 * 61,
        254,
        3 * 3 * 3 * 3 * 3 * 3,
    ] {
        let e = max_error(n);
        assert!(e < 1e-4, "n = {n}: error {e}");
    }
}

#[test]
fn inverse_round_trips() {
    for n in [1, 7, 64, 97, 360, 1031] {
        let x = noise(n, 7);
        let plan = Fft::new(n);
        let mut y = x.clone();
        let mut scratch = vec![Complex::ZERO; plan.scratch_len()];
        plan.forward(&mut y, &mut scratch);
        plan.inverse(&mut y, &mut scratch);
        for (a, b) in x.iter().zip(&y) {
            let b = b.scale(1.0 / n as f32);
            assert!(
                (a.re - b.re).abs() < 1e-4 && (a.im - b.im).abs() < 1e-4,
                "n = {n}"
            );
        }
    }
}

#[test]
fn factors_prefer_fours() {
    assert_eq!(factor(48), [4, 4, 3]);
    assert_eq!(factor(2310), [2, 3, 5, 7, 11]);
    assert_eq!(factor(67), [67]);
    assert!(factor(1).is_empty());
}

#[test]
fn frequency_follows_fftshift() {
    // numpy: fftshift(fftfreq(4, 0.5)) = [-1, -0.5, 0, 0.5]; n = 5, d = 1: [-0.4 .. 0.4].
    assert_eq!(frequency(0.0, 4, 0.5), -1.0);
    assert_eq!(frequency(2.0, 4, 0.5), 0.0);
    assert!((frequency(0.0, 5, 1.0) + 0.4).abs() < 1e-12);
    assert!((frequency(4.0, 5, 1.0) - 0.4).abs() < 1e-12);
}

#[test]
fn constant_plane_is_a_centred_spike() {
    let (w, h) = (6, 5);
    let p = power_spectrum(&vec![2.0; w * h], w, h).unwrap();
    for (i, v) in p.iter().enumerate() {
        if i == (h / 2) * w + w / 2 {
            // |F(0,0)|² = (2·30)² = 3600.
            assert!((v - 3600f32.log10()).abs() < 1e-5);
        } else {
            assert!(v.is_nan() || *v < -6.0, "{i}: {v}");
        }
    }
    assert!(power_spectrum(&[], 0, 3).is_none());
}

#[test]
fn single_cosine_peaks_at_its_frequency() {
    // cos(2π·3x/32) along x: peaks at u = ±3 from the centre column 16.
    let (w, h) = (32, 8);
    let plane: Vec<f32> = (0..w * h)
        .map(|i| (2.0 * PI * 3.0 * (i % w) as f64 / w as f64).cos() as f32)
        .collect();
    let p = power_spectrum(&plane, w, h).unwrap();
    let row = &p[(h / 2) * w..(h / 2 + 1) * w];
    let best = row
        .iter()
        .enumerate()
        .filter(|(_, v)| v.is_finite())
        .max_by(|a, b| a.1.total_cmp(b.1))
        .unwrap()
        .0;
    assert!(best == 13 || best == 19, "{best}");
    assert!((frequency(19.0, w, 1.0) - 3.0 / 32.0).abs() < 1e-12);
}
