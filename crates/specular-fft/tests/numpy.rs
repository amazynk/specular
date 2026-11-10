//! The centred log power spectrum against numpy (`gen/oracle.py`, whose planes and expected
//! spectra are checked in under tests/data).
//!
//! A sample mismatches when its power differs from numpy's by more than 1e-4 of the plane's
//! peak power and by more than 0.01 decades: single-precision transforms are exact to about
//! 1e-6 of the peak, so near-zero bins may differ in log space while being equal in power.

use std::path::Path;

fn floats(path: &Path) -> Vec<f32> {
    std::fs::read(path)
        .unwrap()
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

#[test]
fn power_spectrum_matches_numpy() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data");
    let cases = std::fs::read_to_string(dir.join("cases.txt")).unwrap();
    let mut total = 0usize;
    let mut mismatches = 0usize;
    let mut ran = 0usize;
    // ORACLE lines start their own line (the harness's "test ... " has none).
    println!();
    for line in cases.lines().filter(|l| !l.trim().is_empty()) {
        let mut it = line.split_whitespace();
        let name = it.next().unwrap();
        let w: usize = it.next().unwrap().parse().unwrap();
        let h: usize = it.next().unwrap().parse().unwrap();
        let plane = floats(&dir.join(format!("{name}.in.f32")));
        let want = floats(&dir.join(format!("{name}.out.f32")));
        let got = specular_fft::power_spectrum(&plane, w, h).unwrap();
        assert_eq!(got.len(), want.len(), "{name}");
        let peak = want
            .iter()
            .filter(|v| v.is_finite())
            .fold(f32::MIN, |a, &b| a.max(b));
        let tol = 10f64.powf(f64::from(peak)) * 1e-4;
        let mut bad = 0;
        for (i, (&g, &e)) in got.iter().zip(&want).enumerate() {
            let pg = if g.is_finite() {
                10f64.powf(f64::from(g))
            } else {
                0.0
            };
            let pe = if e.is_finite() {
                10f64.powf(f64::from(e))
            } else {
                0.0
            };
            let decades = (f64::from(g) - f64::from(e)).abs();
            if (pg - pe).abs() > tol && (decades > 0.01 || decades.is_nan()) {
                if bad < 5 {
                    eprintln!("{name}[{}, {}]: {g} vs numpy {e}", i % w, i / w);
                }
                bad += 1;
            }
        }
        // The peak (the DC term of these offset planes) agrees closely in log space.
        let gpeak = got
            .iter()
            .filter(|v| v.is_finite())
            .fold(f32::MIN, |a, &b| a.max(b));
        assert!(
            (gpeak - peak).abs() < 1e-4,
            "{name}: peak {gpeak} vs {peak}"
        );
        // One line per plane for tools/ci/oracle-pass-rates.sh.
        let status = if bad == 0 { "pass" } else { "fail" };
        println!("ORACLE\tFFT\t{name}\t{status}");
        total += got.len();
        mismatches += bad;
        ran += 1;
    }
    eprintln!("numpy fft2 oracle: {ran} planes, {total} samples, {mismatches} mismatches");
    assert!(ran >= 7);
    assert_eq!(mismatches, 0);
}

/// Timing on large planes: `cargo test -p specular-fft --test numpy -- --ignored --nocapture`.
#[test]
#[ignore = "timing"]
fn timing() {
    for (w, h) in [(4096usize, 4096usize), (3000, 2000), (4099, 1024)] {
        let plane: Vec<f32> = (0..w * h).map(|i| ((i * 7919) % 1013) as f32).collect();
        let t = std::time::Instant::now();
        let p = specular_fft::power_spectrum(&plane, w, h).unwrap();
        eprintln!("{w}x{h}: {:?} ({} samples)", t.elapsed(), p.len());
    }
}
