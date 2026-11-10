//! Materials science formats through the registry (the fixtures of specular-em, -spm, -surf
//! and -ebsd): each is recognised by content, opens on the science path with the right
//! domain, calibration and planes; TIA series take their `.emi`'s calibration and an `.emi`
//! opened alone shows its first series; a BCF hypermap gives a spectrum per pixel; EBSD maps
//! give Euler angles and phases.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use specular_core::{CancelToken, Source};
use specular_decode::science::ScienceKind;
use specular_decode::{Format, Registry, Session};

fn fixture(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../formats")
        .join(rel)
}

fn open(rel: &str) -> Arc<dyn Session> {
    let p = fixture(rel);
    Registry::default()
        .open(Arc::new(Source::open(&p).unwrap()))
        .unwrap_or_else(|e| panic!("{rel}: {e}"))
}

/// Opened from memory under another name: recognised by content alone.
fn open_anonymous(rel: &str) -> Arc<dyn Session> {
    let data = std::fs::read(fixture(rel)).unwrap();
    Registry::default()
        .open(Arc::new(Source::from_vec("file.bin", data)))
        .unwrap_or_else(|e| panic!("{rel}: {e}"))
}

fn first_plane(s: &dyn Session) -> Vec<f32> {
    let sc = s.science().expect("science");
    let index = vec![0; sc.axes.len()];
    s.nd_plane(&index, &CancelToken::new())
        .expect("planes")
        .expect("plane")
        .values
        .clone()
}

#[test]
fn formats_by_content() {
    let cases = [
        (
            "specular-em/tests/data/matsci/tia_series_1.ser",
            Format::Tia,
            ScienceKind::ElectronMicroscopy,
        ),
        (
            "specular-em/tests/data/matsci/bcf_plain.bcf",
            Format::Bcf,
            ScienceKind::ElectronMicroscopy,
        ),
        (
            "specular-em/tests/data/matsci/bcf_zlib.bcf",
            Format::Bcf,
            ScienceKind::ElectronMicroscopy,
        ),
        (
            "specular-ebsd/tests/data/square.ang",
            Format::Ang,
            ScienceKind::Ebsd,
        ),
        (
            "specular-ebsd/tests/data/hex.ang",
            Format::Ang,
            ScienceKind::Ebsd,
        ),
        (
            "specular-ebsd/tests/data/mixed.ctf",
            Format::Ctf,
            ScienceKind::Ebsd,
        ),
        (
            "specular-spm/tests/data/afm/height.spm",
            Format::Nanoscope,
            ScienceKind::ScanningProbe,
        ),
        (
            "specular-spm/tests/data/afm/old.001",
            Format::Nanoscope,
            ScienceKind::ScanningProbe,
        ),
        (
            "specular-spm/tests/data/afm/scan.sxm",
            Format::Nanonis,
            ScienceKind::ScanningProbe,
        ),
        (
            "specular-spm/tests/data/afm/frames.mdt",
            Format::NtMdt,
            ScienceKind::ScanningProbe,
        ),
        (
            "specular-spm/tests/data/afm/image.jpk",
            Format::Jpk,
            ScienceKind::ScanningProbe,
        ),
        (
            "specular-surf/tests/data/surface/ripple_folder.x3p",
            Format::X3p,
            ScienceKind::Surface,
        ),
        (
            "specular-surf/tests/data/surface/ripple_int16.x3p",
            Format::X3p,
            ScienceKind::Surface,
        ),
        (
            "specular-surf/tests/data/surface/ripple.dat",
            Format::MetroPro,
            ScienceKind::Surface,
        ),
        (
            "specular-surf/tests/data/surface/ripple.datx",
            Format::Datx,
            ScienceKind::Surface,
        ),
        (
            "specular-surf/tests/data/surface/ripple.vk4",
            Format::Vk4,
            ScienceKind::Surface,
        ),
        (
            "specular-surf/tests/data/surface/ripple.plu",
            Format::Plu,
            ScienceKind::Surface,
        ),
        (
            "specular-surf/tests/data/surface/ripple.al3d",
            Format::Al3d,
            ScienceKind::Surface,
        ),
        (
            "specular-surf/tests/data/surface/ripple.sur",
            Format::Sur,
            ScienceKind::Surface,
        ),
        (
            "specular-surf/tests/data/surface/ripple_compressed.sur",
            Format::Sur,
            ScienceKind::Surface,
        ),
    ];
    for (rel, format, kind) in cases {
        for s in [open(rel), open_anonymous(rel)] {
            assert_eq!(s.info().format, format, "{rel}");
            let sc = s.science().unwrap_or_else(|| panic!("{rel}: not science"));
            assert_eq!(sc.kind, kind, "{rel}");
            assert!(sc.width > 0 && sc.height > 0, "{rel}");
            let plane = first_plane(s.as_ref());
            assert_eq!(plane.len(), (sc.width * sc.height) as usize, "{rel}");
            assert!(plane.iter().any(|v| v.is_finite()), "{rel}");
            if kind == ScienceKind::Surface || format == Format::Nanoscope {
                let c = sc
                    .pixel
                    .as_ref()
                    .unwrap_or_else(|| panic!("{rel}: no calibration"));
                assert!(c.x.step > 0.0 && c.y.step != 0.0, "{rel}");
            }
        }
    }
}

#[test]
fn tia_calibration_and_emi() {
    let s = open("specular-em/tests/data/matsci/tia_series_1.ser");
    let c = s
        .science()
        .unwrap()
        .pixel
        .clone()
        .expect("calibration from the .emi");
    assert!(c.x.step > 0.0);
    // The .emi alone opens its first series.
    let e = open("specular-em/tests/data/matsci/tia_series.emi");
    assert_eq!(e.info().format, Format::Tia);
    assert_eq!(first_plane(e.as_ref()), first_plane(s.as_ref()));
    // A line spectrum without an .emi still opens.
    let l = open("specular-em/tests/data/matsci/tia_line_1.ser");
    assert_eq!(l.info().format, Format::Tia);
}

#[test]
fn ser_video_is_not_tia() {
    // A minimal SER video header: "LUCAM-RECORDER", one 2×2 8-bit mono frame.
    let mut v = b"LUCAM-RECORDER".to_vec();
    for x in [0u32, 0, 0, 2, 2, 8, 1] {
        v.extend_from_slice(&x.to_le_bytes());
    }
    v.extend_from_slice(&[0u8; 120]);
    v.resize(178, 0);
    v.extend_from_slice(&[1, 2, 3, 4]);
    let reg = Registry::default();
    let d = reg.probe(&v).expect("a decoder");
    assert_ne!(d.format(), Format::Tia);
}

#[test]
fn bcf_spectrum_per_pixel() {
    for rel in [
        "specular-em/tests/data/matsci/bcf_plain.bcf",
        "specular-em/tests/data/matsci/bcf_zlib.bcf",
    ] {
        let s = open(rel);
        let sc = s.science().unwrap();
        let sp = sc
            .spectra
            .as_ref()
            .unwrap_or_else(|| panic!("{rel}: no spectra"));
        assert!(sp.channels() > 1, "{rel}");
        let (name, unit) = sp.x_axis();
        assert_eq!((name.as_str(), unit.as_str()), ("Energy", "keV"), "{rel}");
        let at = sp.at(0, 0, &[]).expect("spectrum");
        let (e0, e1) = at.x_range().unwrap();
        assert!(e1 > e0, "{rel}");
        assert_eq!(at.y.len(), sp.channels(), "{rel}");
        let sum = sp.sum().expect("sum");
        assert!(
            sum.y.iter().sum::<f32>() >= at.y.iter().sum::<f32>(),
            "{rel}"
        );
        assert!(sp.at(sc.width - 1, sc.height - 1, &[]).is_some(), "{rel}");
    }
}

#[test]
fn ebsd_orientations() {
    for rel in [
        "specular-ebsd/tests/data/square.ang",
        "specular-ebsd/tests/data/hex.ang",
        "specular-ebsd/tests/data/mixed.ctf",
    ] {
        let s = open(rel);
        let sc = s.science().unwrap();
        let o = sc
            .orientations
            .as_ref()
            .unwrap_or_else(|| panic!("{rel}: no orientations"));
        assert!(!o.phases.is_empty(), "{rel}");
        let indexed = (0..o.height)
            .flat_map(|y| (0..o.width).map(move |x| (x, y)))
            .filter_map(|(x, y)| o.at(x, y))
            .filter(|(e, p)| p.is_some() && e.iter().all(|a| a.is_finite()))
            .count();
        assert!(indexed > 0, "{rel}");
        // The IPF page is an RGB composite; the maps are further pages.
        assert_eq!(sc.channel_axis, Some(0), "{rel}");
        assert!(s.page_count() >= 2, "{rel}");
    }
}

/// Every corpus file (`tools/corpus/fetch-matsci.sh`) through the registry: recognised by
/// content and opened on the science path. Skipped without `SPECULAR_MATSCI_CORPUS`.
#[test]
fn corpus_through_registry() {
    let Some(root) = std::env::var_os("SPECULAR_MATSCI_CORPUS").map(PathBuf::from) else {
        return;
    };
    const EXTS: [&str; 19] = [
        "ser", "emi", "bcf", "ang", "ctf", "spm", "001", "002", "004", "sxm", "jpk", "mdt", "x3p",
        "dat", "datx", "vk4", "plu", "al3d", "sur",
    ];
    let mut files = Vec::new();
    let mut stack = vec![root];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                if p.file_name().is_some_and(|n| n != "expected") {
                    stack.push(p);
                }
            } else if p
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| EXTS.contains(&e.to_ascii_lowercase().as_str()))
            {
                files.push(p);
            }
        }
    }
    files.sort();
    let (mut ok, mut failed) = (0, Vec::new());
    let t0 = std::time::Instant::now();
    for p in &files {
        let r = Registry::default()
            .open(Arc::new(Source::open(p).unwrap()))
            .map_err(|e| e.to_string())
            .and_then(|s| {
                let sc = s.science().ok_or("not science")?;
                let index = vec![0; sc.axes.len()];
                s.nd_plane(&index, &CancelToken::new())
                    .ok_or("no planes")?
                    .map_err(|e| e.to_string())?;
                Ok(s.info().format)
            });
        // Refused on purpose: rsciio's broken test file, and Digital Surf spectral maps
        // (object types 20 and 21, not images).
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        let refused = name == "Nope.bcf" || name.starts_with("test_spectral_map");
        match r {
            Ok(_) if !refused => ok += 1,
            Ok(f) => failed.push(format!("{}: opened as {f:?}", p.display())),
            Err(_) if refused => ok += 1,
            Err(e) => failed.push(format!("{}: {e}", p.display())),
        }
    }
    eprintln!(
        "matsci corpus through the registry: {ok}/{} open in {:?}",
        files.len(),
        t0.elapsed()
    );
    for f in &failed {
        eprintln!("  {f}");
    }
    assert!(files.len() > 50, "{}", files.len());
    assert!(failed.is_empty());
}
