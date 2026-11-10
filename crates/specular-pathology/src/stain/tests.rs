//! Stain separation against analytic synthetic stains.

use super::*;

/// A pixel made of concentrations `c` of the rows of `m` (Beer–Lambert), as codes.
fn synth(c: V3, m: &M3) -> V3 {
    vec_mat(c, m).map(|x| 10f64.powf(-x))
}

fn angle(a: V3, b: V3) -> f64 {
    (dot(unit(a), unit(b)).clamp(-1.0, 1.0)).acos().to_degrees()
}

/// A small deterministic generator (xorshift).
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

#[test]
fn published_vectors_are_unit_and_invertible() {
    for set in StainSet::ALL {
        let s = Stains::published(set);
        for r in s.vectors {
            assert!((dot(r, r) - 1.0).abs() < 1e-12);
        }
        let u = s.unmix().unwrap();
        let id = mat_mul(&s.vectors, &u);
        for (i, row) in id.iter().enumerate() {
            for (j, v) in row.iter().enumerate() {
                let want = if i == j { 1.0 } else { 0.0 };
                assert!((v - want).abs() < 1e-12, "{set:?} {i} {j} {v}");
            }
        }
        assert_eq!(StainSet::from_id(set.id()), Some(set));
    }
    // The residual is perpendicular to both stains.
    let he = Stains::published(StainSet::He);
    assert!(dot(he.vectors[2], he.vectors[0]).abs() < 1e-12);
    assert!(dot(he.vectors[2], he.vectors[1]).abs() < 1e-12);
}

#[test]
fn deconvolution_recovers_synthetic_concentrations() {
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    for set in StainSet::ALL {
        let s = Stains::published(set);
        let u = s.unmix().unwrap();
        for _ in 0..1000 {
            let mut c = [rng.next() * 1.5, rng.next() * 1.5, rng.next() * 0.8];
            if set.count() == 2 {
                c[2] = 0.0;
            }
            let got = concentrations(synth(c, &s.vectors), &u);
            for k in 0..3 {
                assert!((got[k] - c[k]).abs() < 1e-9, "{set:?} {c:?} {got:?}");
            }
        }
    }
}

#[test]
fn intensity_view_of_every_stain_is_the_pixel() {
    let s = Stains::published(StainSet::HeDab);
    let d = display(&s, None).unwrap();
    let c = [0.4, 0.3, 0.2];
    let px = synth(c, &s.vectors);
    let all = render(px, &d, [true; 3], Mode::Intensity);
    for k in 0..3 {
        assert!((all[k] - px[k]).abs() < 1e-9);
    }
    // One stain alone: exactly that stain's light.
    let h = render(px, &d, [true, false, false], Mode::Intensity);
    let want = synth([0.4, 0.0, 0.0], &s.vectors);
    for k in 0..3 {
        assert!((h[k] - want[k]).abs() < 1e-9);
    }
    // OD view: one stain in grey at its concentration.
    let g = render(px, &d, [false, true, false], Mode::Od);
    assert!(g.iter().all(|v| (v - 0.3 / OD_MAX).abs() < 1e-9));
    // White is no stain at all; black is the floor's OD, clamped.
    let w = render([1.0; 3], &d, [true; 3], Mode::Od);
    assert_eq!(w, [0.0; 3]);
}

#[test]
fn negative_concentrations_are_clamped() {
    let s = Stains::published(StainSet::He);
    let d = display(&s, None).unwrap();
    // Colours no H&E mix makes: some concentrations come out negative.
    let odd = [[0.05, 0.9, 0.9], [0.9, 0.05, 0.9], [0.9, 0.9, 0.05]];
    assert!(
        odd.iter()
            .any(|px| concentrations(*px, &d.unmix).iter().any(|v| *v < 0.0))
    );
    for px in odd {
        for mode in [Mode::Od, Mode::Intensity] {
            let shown = render(px, &d, [true, true, true], mode);
            assert!(shown.iter().all(|v| (0.0..=1.0).contains(v)));
        }
    }
}

/// Pixels of two stains at random concentrations (with pure-stain pixels at both ends),
/// 8-bit, plus white background.
fn slide(a: V3, b: V3, n: usize, seed: u64) -> Vec<[u8; 3]> {
    let m = [unit(a), unit(b), [0.0; 3]];
    let mut rng = Rng(seed);
    (0..n)
        .map(|i| {
            if i % 10 == 0 {
                return [250, 248, 252];
            }
            let mix = rng.next();
            let amount = 0.4 + rng.next() * 0.8;
            let c = [amount * mix, amount * (1.0 - mix), 0.0];
            synth(c, &m).map(|x| (x * 255.0).round().clamp(0.0, 255.0) as u8)
        })
        .collect()
}

#[test]
fn macenko_finds_synthetic_stain_vectors() {
    // Eosin's red OD is small, so its purest pixels fall under BETA in red and are left
    // out (as in every Macenko implementation): its estimate leans towards haematoxylin.
    for (a, b, tol) in [
        (HEMATOXYLIN, EOSIN, 8.0),
        (HEMATOXYLIN, DAB, 2.5),
        ([0.55, 0.75, 0.37], [0.2, 0.85, 0.48], 2.5),
    ] {
        let px = slide(a, b, 20_000, 7);
        let e = macenko(&px).unwrap();
        assert!(angle(e.vectors[0], a) < 2.5, "{:?} vs {a:?}", e.vectors[0]);
        assert!(angle(e.vectors[1], b) < tol, "{:?} vs {b:?}", e.vectors[1]);
        // The background was left out.
        assert!(e.pixels < 19_000);
        assert!(e.max_c[0] > 0.5 && e.max_c[1] > 0.5);
    }
}

#[test]
fn macenko_refuses_background_and_one_colour() {
    assert_eq!(
        macenko(&vec![[255, 255, 255]; 5000]),
        Err(EstimateError::TooFewPixels(0))
    );
    let one = vec![[120, 60, 160]; 5000];
    assert_eq!(macenko(&one), Err(EstimateError::Degenerate));
}

#[test]
fn normalisation_maps_a_scaled_slide_onto_the_reference() {
    let reference = Reference {
        vectors: [
            unit([0.5626, 0.7201, 0.4062]),
            unit([0.2159, 0.8012, 0.5581]),
        ],
        max_c: Some([0.9, 0.5]),
    };
    let src_vectors = [unit(HEMATOXYLIN), unit(EOSIN)];
    let est = Estimate {
        vectors: src_vectors,
        max_c: [1.8, 0.25],
        pixels: 1000,
    };
    let stains = Stains::with_estimate(StainSet::He, src_vectors).unwrap();
    let d = display(&stains, Some((&est, &reference))).unwrap();
    for c in [[1.8, 0.25], [0.9, 0.1], [0.2, 0.2]] {
        let px = synth(
            [c[0], c[1], 0.0],
            &[src_vectors[0], src_vectors[1], [0.0; 3]],
        );
        let got = render(px, &d, [true, true, false], Mode::Intensity);
        let want = synth(
            [c[0] * 0.9 / 1.8, c[1] * 0.5 / 0.25, 0.0],
            &[reference.vectors[0], reference.vectors[1], [0.0; 3]],
        );
        for k in 0..3 {
            assert!((got[k] - want[k]).abs() < 1e-9, "{c:?} {got:?} {want:?}");
        }
    }
    // The standard reference converts natural-log maxima to log10.
    let s = Reference::standard_he().max_c.unwrap();
    assert!((s[0] - 0.8558).abs() < 1e-3 && (s[1] - 0.4477).abs() < 1e-3);
}

#[test]
fn eigen_sorts_and_diagonalises() {
    let a = [[4.0, 1.0, 0.5], [1.0, 3.0, 0.2], [0.5, 0.2, 1.0]];
    let (vals, vecs) = eigen(a);
    assert!(vals[0] >= vals[1] && vals[1] >= vals[2]);
    for i in 0..3 {
        let v = [vecs[0][i], vecs[1][i], vecs[2][i]];
        let av = [dot(a[0], v), dot(a[1], v), dot(a[2], v)];
        for k in 0..3 {
            assert!((av[k] - vals[i] * v[k]).abs() < 1e-10);
        }
    }
}
