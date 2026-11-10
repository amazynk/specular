//! MIRAX, Ventana BIF, Olympus VSI and OIR, SPE and SIF through the registry: each corpus
//! file (tools/corpus/fetch-microscopy.sh; skipped when absent) opens as its format, with its
//! pages, facts and a decode; incomplete files are refused in plain words (Zeiss CZI files
//! are tested in tests/czi.rs).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use specular_core::{CancelToken, Source};
use specular_decode::science::ScienceKind;
use specular_decode::{DecodeError, DecodeRequest, Format, Registry};

fn corpus() -> Option<PathBuf> {
    let d = std::env::var_os("SPECULAR_MICROSCOPY_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/corpus/microscopy")
        });
    d.join("expected.tsv").is_file().then_some(d)
}

/// A CZI header with nothing behind it: refused (libCZI's reason, why libCZI cannot run, or
/// that it needs libCZI), never a crash.
#[test]
fn broken_czi_is_refused() {
    let mut b = b"ZISRAWFILE".to_vec();
    b.resize(512, 0);
    let e = Registry::new()
        .open(Arc::new(Source::from_vec("a.czi", b)))
        .err()
        .unwrap();
    assert!(!matches!(&e, DecodeError::Internal(_)), "{e}");
}

#[test]
fn corpus_opens_through_the_registry() {
    let Some(dir) = corpus() else {
        eprintln!("microscopy corpus absent: run tools/corpus/fetch-microscopy.sh");
        return;
    };
    let reg = Registry::new();
    let cancel = CancelToken::new();
    let cases: &[(&str, Format, ScienceKind, u32)] = &[
        (
            "mrxs/CMU-1-Saved-1_16.mrxs",
            Format::Mrxs,
            ScienceKind::Slide,
            4,
        ),
        (
            "mrxs/Mirax2-Fluorescence-2.mrxs",
            Format::Mrxs,
            ScienceKind::Slide,
            4,
        ),
        ("bif/Ventana-1.bif", Format::Bif, ScienceKind::Slide, 2),
        (
            "bif/synthetic-right.bif",
            Format::Bif,
            ScienceKind::Slide,
            2,
        ),
        (
            "vsi/Image_V4.1_FL_2D.vsi",
            Format::Vsi,
            ScienceKind::Slide,
            5,
        ),
        (
            "vsi/metadataTest_01.vsi",
            Format::Vsi,
            ScienceKind::Slide,
            2,
        ),
        (
            "oir/DAPI_mCherry_4Z_5Lambda.oir",
            Format::Oir,
            ScienceKind::Microscopy,
            1,
        ),
        (
            "oir/1202-interval_10sec_sequence_frame.oir",
            Format::Oir,
            ScienceKind::Microscopy,
            1,
        ),
        (
            "spe/ten_frames_two_rois_different_binning.spe",
            Format::Spe,
            ScienceKind::Spectroscopy,
            20,
        ),
        (
            "spe/test_000_.SPE",
            Format::Spe,
            ScienceKind::Spectroscopy,
            2,
        ),
        ("sif/raman1.sif", Format::Sif, ScienceKind::Spectroscopy, 1),
        (
            "sif/nd_lum_image_no_glue.sif",
            Format::Sif,
            ScienceKind::Spectroscopy,
            10,
        ),
    ];
    for &(rel, format, kind, pages) in cases {
        let t0 = Instant::now();
        let s = reg
            .open_path(&dir.join(rel))
            .unwrap_or_else(|e| panic!("{rel}: {e}"));
        let opened = t0.elapsed();
        assert_eq!(s.info().format, format, "{rel}");
        assert_eq!(s.page_count(), pages, "{rel}");
        let sci = s
            .science()
            .unwrap_or_else(|| panic!("{rel}: no science summary"));
        assert_eq!(sci.kind, kind, "{rel}");
        // A view of the whole image at the coarsest scale offered, as a first paint would.
        let scale = *s.info().scales.last().unwrap();
        let t1 = Instant::now();
        let r = s
            .decode(&DecodeRequest::full(scale), &cancel)
            .unwrap_or_else(|e| panic!("{rel}: {e}"));
        let painted = t1.elapsed();
        assert!(r.width > 0 && r.height > 0, "{rel}");
        eprintln!(
            "{rel}: {}x{}, {} pages, open {:.1} ms, 1/{scale} view {:.1} ms",
            s.info().width,
            s.info().height,
            pages,
            opened.as_secs_f64() * 1e3,
            painted.as_secs_f64() * 1e3
        );
        match kind {
            ScienceKind::Slide => {
                assert!(sci.pixel.is_some(), "{rel}: physical pixel size");
                for p in 1..pages {
                    let a = s.open_page(p).unwrap();
                    a.decode(&DecodeRequest::full(8), &cancel)
                        .unwrap_or_else(|e| panic!("{rel} page {p}: {e}"));
                }
            }
            ScienceKind::Spectroscopy => {
                let sp = sci.spectra.as_ref().unwrap();
                assert!(sp.at(0, 0, &[]).is_some(), "{rel}: spectrum");
                let last = s.open_page(pages - 1).unwrap();
                assert!(last.nd_plane(&[], &cancel).unwrap().is_ok());
            }
            _ => {
                let last: Vec<u32> = sci.axes.iter().map(|a| a.len - 1).collect();
                assert!(s.nd_plane(&last, &cancel).unwrap().is_ok(), "{rel}");
                assert!(sci.pixel.is_some(), "{rel}: physical pixel size");
            }
        }
    }
    // λ, Z and channel axes of OIR files.
    let s = reg
        .open_path(&dir.join("oir/DAPI_mCherry_4Z_5Lambda.oir"))
        .unwrap();
    let names: Vec<&str> = s
        .science()
        .unwrap()
        .axes
        .iter()
        .map(|a| a.name.as_str())
        .collect();
    assert_eq!(names, ["λ", "Z"]);
    let s = reg.open_path(&dir.join("vsi/metadataTest_01.vsi")).unwrap();
    let names: Vec<&str> = s
        .science()
        .unwrap()
        .axes
        .iter()
        .map(|a| a.name.as_str())
        .collect();
    assert_eq!(names, ["C", "Z", "T"]);
    // Plain refusals.
    let e = reg.open_path(&dir.join("oir/Map_A01.oir")).err().unwrap();
    assert!(e.to_string().contains("no image"), "{e}");
    let e = reg
        .open_path(&dir.join("vsi/missing-frames.vsi"))
        .err()
        .unwrap();
    assert!(e.to_string().contains(".ets"), "{e}");
}
