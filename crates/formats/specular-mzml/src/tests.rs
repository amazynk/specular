//! Unit tests over the fixtures in `tests/data` (written by `tests/gen_fixtures.py`).

use std::sync::Arc;

use specular_spectra::{Error, SpectraFile};

use super::*;

fn fixture(name: &str) -> Vec<u8> {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(name);
    std::fs::read(p).expect("fixture")
}

fn opened(name: &str) -> MzFile {
    open(Arc::new(fixture(name))).expect("opens")
}

#[test]
fn sniffs_mzml_mzxml_and_leaves_imzml_alone() {
    assert!(sniff_mzml(&fixture("indexed.mzML")));
    assert!(sniff_mzml(&fixture("plain.mzML")));
    assert!(!sniff_mzml(&fixture("imaging.mzML")));
    assert!(!sniff_mzml(&fixture("scans32.mzXML")));
    assert!(sniff_mzxml(&fixture("scans32.mzXML")));
    assert!(!sniff_mzxml(&fixture("plain.mzML")));
    assert!(matches!(
        open(Arc::new(fixture("imaging.mzML"))),
        Err(Error::NotThisFormat)
    ));
    assert!(matches!(
        open(Arc::new(b"<html/>".to_vec())),
        Err(Error::NotThisFormat)
    ));
}

#[test]
fn indexed_file_pages_and_facts() {
    let f = opened("indexed.mzML");
    assert_eq!(f.kind(), Kind::Mzml);
    assert_eq!(f.spectrum_count(), 3);
    assert_eq!(f.chromatogram_count(), 2);
    let facts = f.facts();
    assert_eq!(facts.format, "mzML");
    assert_eq!(facts.version, "1.1.0");
    assert_eq!(facts.title.as_deref(), Some("run01"));
    assert_eq!(facts.date.as_deref(), Some("2026-01-02T03:04:05Z"));
    assert_eq!(
        facts.instrument.as_deref(),
        Some("Q Exactive (electrospray ionization, orbitrap, inductive detector)")
    );
    assert_eq!(facts.points, 400);
    assert!(facts.warnings.is_empty(), "{:?}", facts.warnings);
    assert!(
        facts
            .fields
            .contains(&("Source file".into(), "sample01.raw".into()))
    );
    assert!(
        facts
            .fields
            .iter()
            .any(|(k, v)| k == "Software" && v.contains("gen_fixtures 1.0"))
    );
    // TIC overview, the SRM chromatogram, three spectra.
    assert_eq!(f.page_count(), 5);
    assert_eq!(f.page_name(0), "TIC");
    assert_eq!(f.page_name(1), "SRM SIC Q1=445.3 Q3=175.1");
    assert_eq!(f.page_name(2), "Scan 1");
    let tic = f.page(0).unwrap();
    assert_eq!(tic.x.unit, "min");
    // 90 s, 91.2 s, 96 s.
    assert_eq!(tic.traces[0].x, vec![1.5, 1.52, 1.6]);
    let srm = f.page(1).unwrap();
    assert_eq!(srm.traces[0].x.len(), 4);
    assert!((srm.traces[0].x[1] - 0.2).abs() < 1e-6);
    let s1 = f.page(2).unwrap();
    assert_eq!(s1.traces[0].x.len(), 400);
    assert!(!s1.traces[0].sticks);
    assert!((s1.peaks[0].x - 445.12).abs() < 2.0, "{:?}", s1.peaks);
    let s2 = f.page(3).unwrap();
    assert!(s2.traces[0].sticks);
    // Each note on its own (the app joins them in its direction).
    assert!(s2.notes.len() > 1, "{:?}", s2.notes);
    let note = &s2.notes.join(" · ");
    assert!(
        note.contains("MS2") && note.contains("Precursor m/z 445.33, charge 2+"),
        "{note}"
    );
    assert!(note.contains("Positive"), "{note}");
    let r = f.spectrum(1).unwrap();
    assert_eq!(r.polarity, Some(Polarity::Positive));
    assert_eq!(
        r.x,
        vec![
            110.071f32 as f64,
            175.119f32 as f64,
            262.151f32 as f64,
            333.188f32 as f64,
            445.33f32 as f64
        ]
    );
    // Numpress linear + zlib m/z within its fixed point, slof + zlib intensities within 0.1 %.
    let r = f.spectrum(2).unwrap();
    assert_eq!(r.x.len(), 300);
    assert!(r.x.windows(2).all(|w| w[0] <= w[1]));
}

#[test]
fn broken_index_falls_back_to_a_scan() {
    let f = opened("broken_index.mzML");
    assert_eq!(f.spectrum_count(), 3);
    assert_eq!(f.chromatogram_count(), 2);
    assert!(f.facts().warnings.iter().any(|w| w.contains("index")));
    let good = opened("indexed.mzML");
    for i in 0..3 {
        assert_eq!(f.spectrum(i).unwrap(), good.spectrum(i).unwrap());
    }
}

#[test]
fn plain_file_every_encoding_and_a_built_tic() {
    let f = opened("plain.mzML");
    assert_eq!(f.spectrum_count(), 6);
    assert_eq!(f.page_count(), 7);
    let tic = f.page(0).unwrap();
    // MS1 spectra only (scan 12 is MS2); RTs in seconds become minutes.
    assert_eq!(tic.traces[0].x, vec![1.0, 1.1, 1.2, 1.3, 4.0 / 3.0]);
    assert!(tic.notes[0].contains("1 of them summed"), "{:?}", tic.notes);
    let ints: Vec<Vec<f64>> = (0..5).map(|i| f.spectrum(i).unwrap().y).collect();
    // pic, pic+zlib (reversed), i32, i64 (doubled) all hold the same integers.
    assert_eq!(ints[0], ints[2]);
    let rev: Vec<f64> = ints[1].iter().rev().copied().collect();
    assert_eq!(ints[0], rev);
    let doubled: Vec<f64> = ints[0].iter().map(|v| v * 2.0).collect();
    assert_eq!(ints[3], doubled);
    // slof within its precision.
    for (a, b) in ints[4].iter().zip(&ints[0]) {
        assert!((a - b).abs() <= b.abs() * 2e-4 + 1e-3, "{a} {b}");
    }
    // Numpress linear m/z against plain f64.
    let lin = f.spectrum(0).unwrap().x;
    let plain = f.spectrum(2).unwrap().x;
    for (a, b) in lin.iter().zip(&plain) {
        assert!((a - b).abs() < 1e-6, "{a} {b}");
    }
    let empty = f.spectrum(5).unwrap();
    assert!(empty.x.is_empty() && empty.y.is_empty());
    assert_eq!(f.page_name(6), "Scan 15");
    assert!(
        f.page(3)
            .unwrap()
            .notes
            .iter()
            .any(|n| n.contains("charge 3+"))
    );
}

#[test]
fn truncated_and_refused_files_fail_per_page() {
    let f = opened("truncated.mzML");
    assert_eq!(f.spectrum_count(), 5);
    assert!(f.spectrum(3).is_ok());
    assert!(
        matches!(f.spectrum(4), Err(Error::Truncated(_))),
        "{:?}",
        f.spectrum(4)
    );
    // The overview still opens, the unreadable spectrum said so.
    let tic = f.page(0).unwrap();
    assert!(
        tic.notes[0].contains("could not be read"),
        "{:?}",
        tic.notes
    );
    let z = opened("zstd.mzML");
    match z.spectrum(0) {
        Err(Error::Unsupported(m)) => assert!(m.contains("zstd compression (MS:1003780)"), "{m}"),
        r => panic!("{r:?}"),
    }
    assert!(z.page(1).is_err());
    for cut in [10, 200, 1000] {
        let b = fixture("indexed.mzML")[..cut].to_vec();
        let _ = open(Arc::new(b));
    }
}

#[test]
fn mzxml_scans_nested_and_compressed() {
    for name in ["scans32.mzXML", "scans64z.mzXML"] {
        let f = opened(name);
        assert_eq!(f.kind(), Kind::Mzxml);
        assert_eq!(f.spectrum_count(), 3, "{name}");
        let facts = f.facts();
        assert_eq!(facts.format, "mzXML");
        assert_eq!(facts.version, "3.2");
        assert_eq!(facts.title.as_deref(), Some("sample02.RAW"));
        assert_eq!(
            facts.instrument.as_deref(),
            Some("Thermo Scientific (LTQ Orbitrap, ESI, FTMS, unknown)")
        );
        assert_eq!(facts.points, 40);
        let s1 = f.spectrum(0).unwrap();
        assert_eq!(s1.x.len(), 40);
        assert_eq!(s1.ms_level, Some(1));
        assert!((s1.rt_min.unwrap() - 60.5 / 60.0).abs() < 1e-12);
        let s2 = f.spectrum(1).unwrap();
        let want: Vec<f64> = if name.contains("64") {
            vec![150.1, 250.2, 350.3]
        } else {
            vec![150.1f32.into(), 250.2f32.into(), 350.3f32.into()]
        };
        assert_eq!(s2.x, want);
        assert_eq!(s2.y, vec![10.0, 30.0, 20.0]);
        assert_eq!(s2.precursor_mz, Some(410.5));
        assert_eq!(s2.charge, Some(2));
        assert_eq!(s2.centroid, Some(true));
        let s3 = f.spectrum(2).unwrap();
        assert_eq!(s3.polarity, Some(Polarity::Negative));
        assert_eq!(f.page_name(1), "Scan 1");
        // Overview from the MS1 scans' totIonCurrent and a summed one.
        let tic = f.page(0).unwrap();
        assert_eq!(tic.traces[0].x.len(), 2);
    }
}

#[test]
fn capped_listing_says_so() {
    let mut s = String::from("<mzML><run><spectrumList>");
    for i in 0..(MAX_ENTRIES + 3) {
        s.push_str(&format!("<spectrum id=\"scan={i}\" index=\"{i}\"/>"));
    }
    s.push_str("</spectrumList></run></mzML>");
    let f = open(Arc::new(s.into_bytes())).unwrap();
    assert_eq!(f.spectrum_count(), MAX_ENTRIES);
    assert!(f.facts().warnings[0].contains("200000"));
    assert_eq!(f.page_name(1), "Scan 0");
}
