//! The synthetic fixtures (`gen/gen_fixtures.py`): structure, the orix oracle's records
//! (`tests/data/expected`, from `gen/oracle.py`) and the IPF-Z golden of the hexagonal map.

#[path = "common/oracle.rs"]
mod oracle;

use std::path::PathBuf;

use specular_ebsd::{read, read_ang, read_ctf};
use specular_em::dataset::{AxisKind, Field};

fn data(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(name)
}

#[test]
fn square_ang() {
    let file = std::fs::read(data("square.ang")).unwrap();
    let d = read_ang(&file).unwrap();
    assert_eq!(d.pages.len(), 4);
    let p = &d.pages[0];
    assert_eq!((p.width, p.height), (40, 30));
    assert_eq!(p.field, Field::Ebsd);
    assert!(p.rgb && p.planes.len() == 3);
    assert_eq!(p.title.as_deref(), Some("IPF-Z"));
    assert_eq!(p.x.as_ref().unwrap().step, 0.5);
    assert_eq!(p.x.as_ref().unwrap().unit, "µm");
    let maps = &d.pages[3];
    assert_eq!(maps.axis.kind, AxisKind::Channel);
    let labels: Vec<_> = maps.planes.iter().map(|p| p.label.as_str()).collect();
    assert_eq!(labels, ["IQ", "CI", "Phase", "Detector signal", "Fit"]);
    let o = p.orientations.as_ref().unwrap();
    assert_eq!(o.phases[0].name, "Nickel");
    assert_eq!(o.phases[0].symmetry, "m-3m");
    // Single-phase OIM files number the phase 0: it is still the header's phase.
    let indexed = (0..30)
        .flat_map(|y| (0..40).map(move |x| (x, y)))
        .filter(|&(x, y)| o.at(x, y).is_some_and(|(_, ph)| ph.is_some()))
        .count();
    assert!(indexed > 1000, "{indexed}");
    assert!(
        p.meta
            .iter()
            .any(|(k, v)| k == "Scan" && v == "specular test")
    );
}

#[test]
fn hex_ang() {
    let file = std::fs::read(data("hex.ang")).unwrap();
    let d = read(&file).unwrap();
    let p = &d.pages[0];
    assert_eq!((p.width, p.height), (31, 24));
    let y = p.y.as_ref().unwrap();
    assert!((y.step - 3f64.sqrt() / 2.0).abs() < 1e-5);
    let o = p.orientations.as_ref().unwrap();
    let names: Vec<_> = o
        .phases
        .iter()
        .map(|p| (p.name.as_str(), p.symmetry.as_str()))
        .collect();
    assert_eq!(names, [("Titanium", "6/mmm"), ("Iron", "m-3m")]);
    // Every pixel is filled: short rows repeat their last point.
    let r = p.read_plane(&[], 0).unwrap().0;
    assert!(r.iter().all(|v| v.is_finite()));
    let labels: Vec<_> = d.pages[3].planes.iter().map(|p| p.label.as_str()).collect();
    assert_eq!(labels, ["IQ", "CI", "Phase", "Detector signal", "Fit"]);
}

#[test]
fn mixed_ctf() {
    let file = std::fs::read(data("mixed.ctf")).unwrap();
    let d = read_ctf(&file).unwrap();
    let p = &d.pages[0];
    assert_eq!((p.width, p.height), (40, 30));
    assert_eq!(p.voltage, Some(20e3));
    assert_eq!(p.magnification, Some(2000.0));
    let o = p.orientations.as_ref().unwrap();
    assert_eq!(o.phases[1].symmetry, "6/mmm");
    let labels: Vec<_> = d.pages[3].planes.iter().map(|p| p.label.as_str()).collect();
    assert_eq!(
        labels,
        [
            "Band contrast",
            "Band slope",
            "MAD",
            "Bands",
            "Error",
            "Phase"
        ]
    );
    assert_eq!(d.pages[3].planes[2].unit, "°");
    // Not indexed: black.
    let i = (0..p.width * p.height)
        .find(|&i| o.phase[i] == u8::MAX)
        .expect("some points are not indexed");
    let red = p.read_plane(&[], 0).unwrap().0;
    assert_eq!(red[i], 0.0);
}

#[test]
fn fixtures_against_orix() {
    let mut total = oracle::Tally::default();
    for name in ["square.ang", "hex.ang", "mixed.ctf"] {
        let file = std::fs::read(data(name)).unwrap();
        let d = read(&file).unwrap();
        let rec = oracle::load(&data("expected").join(name)).unwrap();
        let t = oracle::compare(&d, &rec);
        eprintln!("{name}: {t:?}");
        assert!(t.points > 500);
        assert_eq!(t.mismatches(), 0, "{name}: {t:?}");
        total.add(&t);
    }
    eprintln!(
        "fixtures: {} points, 0 mismatches; mean |ours − orix's own key| {:.3} per channel",
        total.points,
        total.orix_key_sum / total.orix_key_n as f64
    );
}

/// IPF-Z of the hexagonal map as RGB8 (`SPECULAR_BLESS=1` rewrites it).
#[test]
fn hex_ipf_golden() {
    let file = std::fs::read(data("hex.ang")).unwrap();
    let d = read(&file).unwrap();
    let p = &d.pages[0];
    let planes: Vec<Vec<f32>> = (0..3).map(|k| p.read_plane(&[], k).unwrap().0).collect();
    let mut rgb = Vec::with_capacity(p.width * p.height * 3);
    for i in 0..p.width * p.height {
        for pl in &planes {
            rgb.push((pl[i].clamp(0.0, 1.0) * 255.0).round() as u8);
        }
    }
    let golden = data("hex_ipfz.rgb");
    if std::env::var_os("SPECULAR_BLESS").is_some() {
        std::fs::write(&golden, &rgb).unwrap();
    }
    let want = std::fs::read(&golden).expect("golden (SPECULAR_BLESS=1 writes it)");
    assert_eq!(want.len(), rgb.len());
    let off = want
        .iter()
        .zip(&rgb)
        .filter(|(a, b)| a.abs_diff(**b) > 1)
        .count();
    assert_eq!(off, 0, "{off} channels differ from the golden");
}
