//! The HDR maths of `specular_core::hdr` against reference values computed independently by
//! `tools/quality/hdr_reference.py` (colour-science for the sRGB, PQ and HLG EOTFs and the
//! primaries matrices; the ISO 21496-1 / Ultra HDR gain-map equations in float64). The GPU
//! paths are checked against these CPU functions in specular-render's `hdr_tests`.

use specular_core::hdr::{self, GainMapParams, Primaries};

/// Relative error with an absolute floor (f32 maths against float64 references).
fn err(got: f64, want: f64) -> f64 {
    (got - want).abs() / want.abs().max(1e-3)
}

fn nums(s: &str) -> Vec<f64> {
    s.split_whitespace().map(|v| v.parse().unwrap()).collect()
}

#[test]
fn hdr_maths_match_references() {
    let text = include_str!("data/hdr_reference.txt");
    let mut worst: std::collections::BTreeMap<&str, (f64, usize, String)> = Default::default();
    let mut bad = Vec::new();
    for line in text.lines().filter(|l| !l.starts_with('#')) {
        let (lhs, rhs) = line.split_once(" = ").unwrap();
        let (kind, args) = lhs.split_once(' ').unwrap_or((lhs, ""));
        let a = if kind == "matrix" {
            Vec::new()
        } else {
            nums(args)
        };
        let want = nums(rhs);
        let (got, tol): (Vec<f64>, f64) = match kind {
            "srgb" => (vec![hdr::srgb_to_linear(a[0] as f32) as f64], 2e-6),
            "pq" => (vec![hdr::pq_to_linear(a[0] as f32) as f64], 1e-4),
            "hlg" => (
                hdr::hlg_to_linear([a[0] as f32, a[1] as f32, a[2] as f32])
                    .map(f64::from)
                    .to_vec(),
                2e-5,
            ),
            "matrix" => {
                let p = match args {
                    "bt2020" => Primaries::Bt2020,
                    _ => Primaries::DisplayP3,
                };
                (
                    hdr::to_bt709(p)
                        .iter()
                        .flatten()
                        .map(|&v| v as f64)
                        .collect(),
                    // Absolute on matrix entries (four to six significant digits are kept).
                    5e-4,
                )
            }
            "weight" | "gain" => {
                let p = GainMapParams {
                    gain_min: [a[0] as f32; 3],
                    gain_max: [a[1] as f32; 3],
                    gamma: [a[2] as f32; 3],
                    base_offset: [a[3] as f32; 3],
                    alt_offset: [a[4] as f32; 3],
                    base_headroom: a[5] as f32,
                    alt_headroom: a[6] as f32,
                    base_is_hdr: a[5] > a[6],
                    space: Primaries::Bt709,
                };
                if kind == "weight" {
                    (vec![p.weight(a[7] as f32) as f64], 1e-6)
                } else {
                    (
                        vec![p.apply(0, a[8] as f32, a[9] as f32, a[7] as f32) as f64],
                        2e-6,
                    )
                }
            }
            k => panic!("unknown kind {k}"),
        };
        assert_eq!(got.len(), want.len(), "{line}");
        for (g, w) in got.iter().zip(&want) {
            let e = if kind == "matrix" {
                (g - w).abs()
            } else {
                err(*g, *w)
            };
            let entry = worst.entry(kind).or_insert((0.0, 0, String::new()));
            entry.1 += 1;
            if e > entry.0 {
                *entry = (e, entry.1, line.to_string());
            }
            if e > tol {
                bad.push(format!("{line}: got {got:?} (error {e:.2e} > {tol:.0e})"));
            }
        }
    }
    for (k, (e, n, at)) in &worst {
        eprintln!("{k}: {n} values, worst error {e:.2e} at {at}");
    }
    assert!(
        worst.len() == 6,
        "reference kinds missing: {:?}",
        worst.keys()
    );
    assert!(
        bad.is_empty(),
        "{} mismatches:\n{}",
        bad.len(),
        bad.join("\n")
    );
}

/// Properties of the tone-mapping operator (no external reference: it is our own curve):
/// identity below the knee, continuous with slope 1 there, monotonic, content peak onto the
/// display peak.
#[test]
fn tone_map_curve_properties() {
    for (content, display) in [(4.93f32, 1.0f32), (49.3, 4.0), (10.0, 8.0), (3.0, 2.9)] {
        let f = |x: f32| x * hdr::tone_map_scale(x, content, display);
        let k = 0.8 * display;
        assert_eq!(f(k * 0.99), k * 0.99);
        let d = 1e-3 * display;
        let slope = (f(k + d) - f(k)) / d;
        assert!((slope - 1.0).abs() < 0.02, "slope {slope} at the knee");
        assert!((f(content) - display).abs() < 1e-4 * display);
        let mut prev = 0.0;
        for i in 0..=1000 {
            let y = f(content * i as f32 / 1000.0);
            assert!(y >= prev - 1e-6, "not monotonic at {i}");
            prev = y;
        }
        assert!(f(content * 2.0) <= display + 1e-6);
    }
    assert_eq!(
        hdr::tone_map_scale(3.0, 2.0, 4.0),
        1.0,
        "no tone mapping when it fits"
    );
}
