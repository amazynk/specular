//! The synthetic experiments of tests/gen_fixtures.py.

use std::path::PathBuf;

use specular_spectra::{Error, Plot, Technique};

fn data(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(name)
}

/// The ppm of the `n` highest local maxima of a plot's first trace.
fn peaks(p: &Plot, n: usize) -> Vec<f64> {
    let t = &p.traces[0];
    let mut idx: Vec<usize> = (3..t.y.len() - 3)
        .filter(|&i| (i - 3..=i + 3).all(|j| t.y[j] <= t.y[i]))
        .collect();
    idx.sort_by(|&a, &b| t.y[b].total_cmp(&t.y[a]));
    idx.truncate(n);
    let mut v: Vec<f64> = idx.iter().map(|&i| t.x[i]).collect();
    v.sort_by(|a, b| b.total_cmp(a));
    v
}

/// Negative intensity (below the median) over all intensity, squared.
fn negative_fraction(p: &Plot) -> f64 {
    let y = &p.traces[0].y;
    let mut s = y.clone();
    s.sort_by(f64::total_cmp);
    let med = s[s.len() / 2];
    let neg: f64 = y.iter().map(|v| (v - med).min(0.0).powi(2)).sum();
    let all: f64 = y.iter().map(|v| (v - med).powi(2)).sum();
    neg / all
}

fn near_all(got: &[f64], want: &[f64], tol: f64) -> bool {
    got.len() == want.len() && got.iter().zip(want).all(|(a, b)| (a - b).abs() <= tol)
}

#[test]
fn processed_spectrum_and_our_processing_agree_on_the_lines() {
    let s = specular_nmr::open(&data("fid1d")).expect("opens");
    let titles: Vec<&str> = s.pages.iter().map(|p| p.title.as_str()).collect();
    assert_eq!(titles, ["Spectrum", "Spectrum from the FID", "FID"]);
    let want = [7.26, 3.70, 1.20];
    // Points are 5000 Hz / 400.13 MHz / 8192 ≈ 0.0015 ppm apart.
    let tol = 0.004;
    let stored = &s.pages[0];
    assert!(stored.x.reversed);
    assert!(
        near_all(&peaks(stored, 3), &want, tol),
        "{:?}",
        peaks(stored, 3)
    );
    let ours = &s.pages[1];
    assert_eq!(ours.traces[0].y.len(), 8192);
    assert!(
        near_all(&peaks(ours, 3), &want, tol),
        "{:?}",
        peaks(ours, 3)
    );
    assert!(
        negative_fraction(ours) < 0.01,
        "{}",
        negative_fraction(ours)
    );
    assert!(ours.notes.iter().any(|n| n.contains("automatically")));
    assert!(ours.notes.iter().any(|n| n.contains("0.5 Hz")));
    // The stored peak list, with the spectrum's intensity at each.
    let ppm: Vec<f64> = stored.peaks.iter().map(|p| p.x).collect();
    assert_eq!(ppm, want);
    assert!(stored.peaks.iter().all(|p| p.y > 0.0));
    // The FID page: real and imaginary against time.
    assert_eq!(s.pages[2].traces.len(), 2);
    assert_eq!(s.pages[2].traces[0].x[1], 1.0 / 5000.0);
    let f = &s.facts;
    assert_eq!(f.format, "Bruker NMR");
    assert_eq!(f.version, "TopSpin 4.1.4");
    assert_eq!(f.technique, Technique::Nmr);
    assert_eq!(f.nucleus.as_deref(), Some("1H"));
    assert_eq!(f.solvent.as_deref(), Some("CDCl3"));
    assert_eq!(f.pulse_program.as_deref(), Some("zg30"));
    assert_eq!(f.title.as_deref(), Some("Three lines synthetic fixture"));
    assert_eq!(f.date.as_deref(), Some("2025-10-19 12:00 UTC"));
    assert_eq!(f.points, 4096);
    assert_eq!(f.spectra, 3);
    assert!(f.fields.iter().any(|(k, v)| k == "D1" && v == "1"));
    assert!(f.fields.iter().any(|(k, v)| k == "AQ" && v == "0.41"));
}

#[test]
fn float64_big_endian_fids_read_the_same() {
    let a = specular_nmr::fid(&data("fid1d")).unwrap().1;
    let b = specular_nmr::fid(&data("fid1d_f64_be")).unwrap().1;
    assert_eq!(a, b);
    // No pdata: our processing comes first.
    let s = specular_nmr::open(&data("fid1d_f64_be")).unwrap();
    assert_eq!(s.pages[0].title, "Spectrum from the FID");
    // Default size (TD) and window (0.3 Hz for 1H).
    assert_eq!(s.pages[0].traces[0].y.len(), 4096);
    assert!(s.pages[0].notes.iter().any(|n| n.contains("0.3 Hz")));
    assert!(near_all(&peaks(&s.pages[0], 3), &[7.26, 3.70, 1.20], 0.006));
}

#[test]
fn old_firmware_takes_its_delay_from_the_table() {
    let s = specular_nmr::open(&data("old_xwin")).unwrap();
    let p = &s.pages[0];
    // 25 kHz over 4096 points at 100.61 MHz: 0.06 ppm a point.
    assert!(
        near_all(&peaks(p, 2), &[128.5, 77.0], 0.13),
        "{:?}",
        peaks(p, 2)
    );
    assert!(negative_fraction(p) < 0.01, "{}", negative_fraction(p));
    assert_eq!(s.facts.version, "XWIN-NMR 3.5");
    let acqus = specular_nmr::fid(&data("old_xwin")).unwrap().0;
    assert_eq!(specular_nmr::group_delay(&acqus).0, 66.625);
}

#[test]
fn two_dimensional_spectra_are_untiled() {
    let s = specular_nmr::open(&data("hsqc2d")).unwrap();
    let g = s.pages[0].grid.as_ref().expect("a grid");
    assert_eq!((g.width, g.height), (64, 32));
    for r in 0..32 {
        for c in 0..64 {
            assert_eq!(g.z[r * 64 + c], (r * 1000 + c) as f32);
        }
    }
    let off2 = 4350.0 / 500.13;
    assert!((g.x_range.0 - off2).abs() < 1e-6);
    assert!((g.x_range.1 - (off2 - 4000.0 / 500.13 * 63.0 / 64.0)).abs() < 1e-6);
    assert_eq!(g.y_range.0, 155.0);
    assert!((g.y_range.1 - (155.0 - 160.0 * 31.0 / 32.0)).abs() < 1e-9);
    assert!(s.pages[0].x.reversed && s.pages[0].y.reversed);
    // The series' first FID, processed and said so.
    let p = &s.pages[1];
    assert_eq!(p.title, "Spectrum from the FID");
    assert!(p.notes[0].contains("first FID"));
    assert!(near_all(&peaks(p, 1), &[2.0], 0.04), "{:?}", peaks(p, 1));
    assert!(
        s.facts
            .fields
            .iter()
            .any(|(k, v)| k == "NUC1 (F1)" && v == "13C")
    );
}

#[test]
fn entry_points_and_refusals() {
    let exp = data("fid1d");
    assert!(specular_nmr::is_experiment_dir(&exp));
    assert!(specular_nmr::is_experiment_dir(&exp.join("pdata/1")));
    assert!(!specular_nmr::is_experiment_dir(&exp.join("pdata")));
    assert!(specular_nmr::is_bruker_file(&exp.join("fid")));
    assert!(specular_nmr::is_bruker_file(&exp.join("pdata/1/1r")));
    assert!(!specular_nmr::is_bruker_file(
        &exp.join("pdata/1/peaklist.xml")
    ));
    assert_eq!(
        specular_nmr::experiment_of(&exp.join("pdata/1/1r")),
        Some((exp.clone(), Some(1)))
    );
    // Opened through a file or the procno folder: the same experiment.
    let a = specular_nmr::open(&exp.join("pdata/1/1r")).unwrap();
    let b = specular_nmr::open(&exp.join("pdata/1")).unwrap();
    assert_eq!(a.pages.len(), 3);
    assert_eq!(a.pages[0], b.pages[0]);
    let v = data("varian");
    assert!(!specular_nmr::is_experiment_dir(&v));
    match specular_nmr::open(&v) {
        Err(Error::Unsupported(m)) => assert!(m.contains("Varian"), "{m}"),
        r => panic!("{:?}", r.map(|s| s.pages.len())),
    }
    assert_eq!(
        specular_nmr::open(&data("missing")).err(),
        Some(Error::NotThisFormat)
    );
}
