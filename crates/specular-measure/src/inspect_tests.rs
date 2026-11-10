use super::*;
use crate::calib::{Source, Unit};

fn grey(w: u32, h: u32, f: impl Fn(u32, u32) -> f64) -> Grey {
    let data = (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .map(|(x, y)| f(x, y))
        .collect();
    Grey::new(0, 0, w, h, data).unwrap()
}

/// The projected thickness of a round wire of diameter `d` at offset `x` from its axis.
fn chord(x: f64, d: f64) -> f64 {
    let r = d / 2.0;
    if x.abs() >= r {
        0.0
    } else {
        2.0 * (r * r - x * x).sqrt()
    }
}

fn blur(v: &[f64], sigma: f64) -> Vec<f64> {
    if sigma <= 0.0 {
        return v.to_vec();
    }
    let k = (4.0 * sigma).ceil() as i64;
    let w: Vec<f64> = (-k..=k)
        .map(|i| (-(i * i) as f64 / (2.0 * sigma * sigma)).exp())
        .collect();
    let sum: f64 = w.iter().sum();
    (0..v.len() as i64)
        .map(|i| {
            (-k..=k)
                .map(|j| {
                    let idx = (i + j).clamp(0, v.len() as i64 - 1) as usize;
                    v[idx] * w[(j + k) as usize]
                })
                .sum::<f64>()
                / sum
        })
        .collect()
}

#[test]
fn profile_of_a_ramp_is_exact() {
    let g = grey(20, 10, |x, _| f64::from(x) * 10.0);
    let (v, step) = profile(&g, P::new(2.5, 3.5), P::new(12.5, 3.5), 1);
    assert_eq!(v.len(), 11);
    assert!((step - 1.0).abs() < 1e-12);
    for (k, x) in v.iter().enumerate() {
        assert!((x - (20.0 + 10.0 * k as f64)).abs() < 1e-9, "{k}: {x}");
    }
    // Averaged across: a ramp along x is the same on every parallel line.
    let (w, _) = profile(&g, P::new(2.5, 5.0), P::new(12.5, 5.0), 5);
    assert_eq!(v, w);
}

#[test]
fn density_is_linear_between_two_points() {
    let d = Density::new([1000.0, 31000.0], [4.0, 1.0]).unwrap();
    assert!((d.at(16000.0) - 2.5).abs() < 1e-12);
    assert!(Density::new([5.0, 5.0], [1.0, 2.0]).is_none());
}

#[test]
fn snr_region_grows_to_the_standard_size() {
    assert_eq!(
        snr_region((10.0, 10.0, 20.0, 15.0), 200, 100),
        Some((0, 3, 55, 23))
    );
    // Upright: 20 wide, 55 tall, centred.
    assert_eq!(
        snr_region((50.0, 40.0, 52.0, 60.0), 200, 200),
        Some((41, 23, 61, 78))
    );
    // Larger regions keep their size.
    assert_eq!(
        snr_region((10.0, 10.0, 110.0, 40.0), 200, 200),
        Some((10, 10, 110, 40))
    );
    assert_eq!(snr_region((0.0, 0.0, 5.0, 5.0), 40, 40), None);
}

#[test]
fn snr_of_known_values() {
    // Alternating 90/110: mean 100, sample sd over 1100 values.
    let g = grey(60, 30, |x, y| if (x + y) % 2 == 0 { 90.0 } else { 110.0 });
    let s = snr(&g, (0, 0, 55, 20), Some(100.0)).unwrap();
    assert_eq!(s.count, 1100);
    assert!((s.mean - 100.0).abs() < 1e-9);
    let sd = (1100.0 * 100.0 / 1099.0f64).sqrt();
    assert!((s.sd - sd).abs() < 1e-9);
    assert!((s.snr - 100.0 / sd).abs() < 1e-9);
    let (srb, n) = s.normalised.unwrap();
    assert_eq!(srb, 100.0);
    assert!((n - s.snr * 0.886).abs() < 1e-9);
    assert!(snr(&grey(60, 30, |_, _| 5.0), (0, 0, 55, 20), None).is_none());
}

/// A duplex IQI profile: pairs D1… at 0.02 mm a sample, pitch 4 mm, dark wires on 30000.
fn duplex_profile(sigma_mm: f64, reversed: bool) -> (Vec<f64>, f64) {
    let mm = 0.02;
    let n = (DUPLEX.len() as f64 * 4.0 / mm) as usize + 200;
    let mut t = vec![0.0; n];
    for (i, (_, d)) in DUPLEX.iter().enumerate() {
        let c = 2.0 + i as f64 * 4.0;
        for (k, v) in t.iter_mut().enumerate() {
            let x = k as f64 * mm;
            *v += chord(x - (c - d), *d) + chord(x - (c + d), *d);
        }
    }
    let v: Vec<f64> = t.iter().map(|x| 30000.0 * (-0.8 * x).exp()).collect();
    let mut v = blur(&v, sigma_mm / mm);
    if reversed {
        v.reverse();
    }
    (v, mm)
}

/// The analytic dip (%) of a pair of wires of diameter `d` under the same blur.
fn analytic_dip(d: f64, sigma_mm: f64) -> f64 {
    let mm = d / 50.0;
    let len = (8.0 * d / mm) as usize;
    let mut t = vec![0.0; len];
    for (k, v) in t.iter_mut().enumerate() {
        let x = k as f64 * mm;
        *v = chord(x - 3.0 * d, d) + chord(x - 5.0 * d, d);
    }
    // The wire signal (background − grey), blurred like the image.
    let s: Vec<f64> = t.iter().map(|x| 1.0 - (-0.8 * x).exp()).collect();
    let s = blur(&s, sigma_mm / mm);
    let mid = (4.0 * d / mm) as usize;
    let a = s[..mid].iter().copied().fold(0.0, f64::max);
    let c = s[mid];
    (2.0 * a - 2.0 * c) / (2.0 * a) * 100.0
}

/// The first pair whose analytic dip is below 20 %, by the same blur (the ground truth).
fn analytic_first_unresolved(sigma_mm: f64) -> u8 {
    for (n, d) in DUPLEX {
        if analytic_dip(d, sigma_mm) < DIP_RESOLVED {
            return n;
        }
    }
    0
}

/// The diameter whose analytic dip is 20 % (bisection; the dip grows with d).
fn analytic_isrb(sigma_mm: f64) -> f64 {
    let (mut lo, mut hi) = (0.02f64, 0.8f64);
    for _ in 0..40 {
        let m = (lo * hi).sqrt();
        if analytic_dip(m, sigma_mm) < DIP_RESOLVED {
            lo = m;
        } else {
            hi = m;
        }
    }
    (lo * hi).sqrt()
}

#[test]
fn duplex_interpolates_the_twenty_percent_dip() {
    for sigma in [0.04, 0.08, 0.15] {
        for rev in [false, true] {
            let (v, mm) = duplex_profile(sigma, rev);
            let d = duplex(&v, mm).unwrap();
            let (n, srb) = d.srb().unwrap();
            let isrb = d.isrb.unwrap();
            // Between the first unresolved pair and the last resolved one.
            let prev = DUPLEX[usize::from(n) - 2].1;
            assert!(
                isrb >= srb && isrb <= prev,
                "σ {sigma}: {isrb} not in {srb}…{prev}"
            );
            // Against the analytic crossing where the pairs are numbered as the analytic
            // dips are (the 3-sample smoothing lowers the smallest pairs' dips).
            if n != analytic_first_unresolved(sigma) {
                continue;
            }
            let want = analytic_isrb(sigma);
            assert!(
                (isrb / want - 1.0).abs() < 0.15,
                "σ {sigma} reversed {rev}: iSRb {isrb}, analytic {want}"
            );
        }
    }
}

/// Pairs with set dips, numbered from D1.
fn pairs(dips: &[f64]) -> Vec<Pair> {
    dips.iter()
        .enumerate()
        .map(|(i, &dip)| Pair {
            number: Some(DUPLEX[i]),
            span: (i * 10, i * 10 + 5),
            dip,
            resolved: dip >= DIP_RESOLVED,
        })
        .collect()
}

#[test]
fn isrb_fits_a_parabola_through_the_neighbours() {
    // Dips on an exact parabola in d: the fit reproduces its 20 % crossing.
    let f = |d: f64| 300.0 * d * d + 40.0 * d - 15.0;
    let p = pairs(&[95.0, 90.0, f(0.50), f(0.40), f(0.32), f(0.25), f(0.20), 0.0]);
    let fu = p.iter().position(|p| !p.resolved).unwrap();
    assert_eq!(fu, 5);
    let x = interpolated_srb(&p, fu, 0.001).unwrap();
    // 300 d² + 40 d − 35 = 0.
    let want = (-40.0 + (1600.0f64 + 4.0 * 300.0 * 35.0).sqrt()) / 600.0;
    assert!((x - want).abs() < 1e-9, "{x} vs {want}");
    // Only one point on each side: linear between them.
    let p = pairs(&[30.0, 10.0]);
    let x = interpolated_srb(&p, 1, 0.001).unwrap();
    assert!((x - (0.63 + 0.17 * 0.5)).abs() < 1e-9, "{x}");
    // Never below the sample spacing, and nothing without a resolved pair.
    assert_eq!(interpolated_srb(&p, 1, 0.75), Some(0.75));
    assert_eq!(interpolated_srb(&pairs(&[10.0]), 0, 0.001), None);
    // The first unresolved pair at dip 0 (too faint to see) is used when none is above 0.
    let p = pairs(&[60.0, 40.0, 0.0]);
    let x = interpolated_srb(&p, 2, 0.001).unwrap();
    assert!(x > 0.50 && x < 0.63, "{x}");
}

#[test]
fn duplex_finds_the_first_unresolved_pair() {
    for sigma in [0.04, 0.08, 0.15] {
        for rev in [false, true] {
            let (v, mm) = duplex_profile(sigma, rev);
            let d = duplex(&v, mm).unwrap();
            let want = analytic_first_unresolved(sigma);
            let (n, dia) = d.srb().unwrap();
            // The analytic dip ignores the 3-sample smoothing: a borderline pair may tip.
            assert!(
                n.abs_diff(want) <= 1,
                "σ {sigma} reversed {rev}: want D{want}, {d:?}"
            );
            assert_eq!(d.pairs[0].number.unwrap().0, 1);
            assert!((dia - DUPLEX[usize::from(n) - 1].1).abs() < 1e-12);
        }
    }
}

#[test]
fn wire_width_names_the_wire() {
    for (n, d) in [(12u8, 0.25), (9, 0.5), (16, 0.1)] {
        let mm = 0.005;
        let v: Vec<f64> = (0..400)
            .map(|k| 20000.0 - 4000.0 * chord(k as f64 * mm - 1.0, d))
            .collect();
        let w = wire(&v, mm).unwrap();
        assert_eq!(w.number, n, "{w:?}");
        assert!((w.diameter - d).abs() < 0.02 * d + 2.0 * mm, "{w:?}");
    }
    assert_eq!(wire(&[1.0; 50], 0.1), Err(Refusal::NothingFound));
}

#[test]
fn crack_widths_along_a_line() {
    // A dark band 6 px wide around x = 40 (anti-aliased edges), on 200.
    let g = grey(80, 60, |x, _| {
        let c = f64::from(x) + 0.5;
        if (c - 40.0).abs() <= 3.0 { 40.0 } else { 200.0 }
    });
    let cal = Calibration::spacing(0.1, 0.1, Unit::Mm, Source::User).unwrap();
    let c = crack(&g, &[P::new(40.0, 5.0), P::new(40.0, 55.0)], 10, 15.0, &cal).unwrap();
    assert_eq!(c.cuts.len(), 10);
    for cut in &c.cuts {
        assert!((cut.px - 6.0).abs() < 0.3, "{cut:?}");
    }
    assert!((c.widths.1 - 0.6).abs() < 0.03, "{:?}", c.widths);
    assert!(c.widths.0 <= c.widths.1 + 1e-12 && c.widths.1 <= c.widths.2 + 1e-12);
}

#[test]
fn probes_round_trip_their_keys() {
    for p in Probe::ALL {
        assert_eq!(Probe::from_key(p.key()), Some(p));
    }
}

#[test]
fn readouts_name_their_values() {
    let r = Reading::Wire(Wire {
        fwhm: 0.2,
        diameter: 0.23,
        number: 12,
        nominal: 0.25,
    });
    let keys: Vec<_> = r
        .readouts(P::default(), P::new(1.0, 0.0), "mm")
        .into_iter()
        .map(|r| r.key)
        .collect();
    assert_eq!(keys, ["wire", "nominal", "fwhm", "diameter"]);
}

#[test]
fn duplex_readouts_add_the_interpolated_values() {
    let mut p = pairs(&[60.0, 10.0]);
    p[1].resolved = false;
    let r = Reading::Duplex(Duplex {
        pairs: p,
        first_unresolved: Some(1),
        isrb: Some(0.7),
    });
    let out = r.readouts(P::default(), P::new(1.0, 0.0), "mm");
    let keys: Vec<_> = out.iter().map(|r| r.key.as_str()).collect();
    assert_eq!(
        keys,
        [
            "D1-dip",
            "D2-dip",
            "first-unresolved",
            "unsharpness",
            "srb",
            "isrb",
            "iunsharpness"
        ]
    );
    assert!((out[5].value - 700.0).abs() < 1e-9 && (out[6].value - 1.4).abs() < 1e-12);
}

#[test]
fn mm_per_px_needs_a_length_unit() {
    let cal = Calibration::spacing(0.05, 0.05, Unit::Mm, Source::User).unwrap();
    let v = mm_per_px(&cal, P::default(), P::new(3.0, 4.0)).unwrap();
    assert!((v - 0.05).abs() < 1e-12);
    assert_eq!(
        mm_per_px(&Calibration::uncalibrated(), P::default(), P::new(1.0, 0.0)),
        None
    );
}
