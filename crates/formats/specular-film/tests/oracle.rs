//! Differential tests against the readings stored beside the files: the committed fixtures
//! (always), and the corpus `tools/corpus/fetch-raster-film.sh` builds (ignored unless
//! `SPECULAR_RASTER_CORPUS_FILM` names it):
//!
//! `SPECULAR_RASTER_CORPUS_FILM=tools/corpus/raster/film cargo test -p specular-film --test oracle
//! -- --ignored --nocapture`

mod common;

use common::Report;
use std::path::Path;

fn files(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut v: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| !p.to_string_lossy().ends_with(".pam"))
        .collect();
    v.sort();
    v
}

#[test]
fn fixtures_match_oracles() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut r = Report::default();
    for f in files(&dir) {
        r.check(&f, &dir);
    }
    r.print();
    assert!(r.refused.is_empty());
    assert!(r.raw_magick.files >= 5);
    assert_eq!(r.raw_magick.pass, r.raw_magick.files);
    // ImageMagick reads method B wrongly (as method A); OpenImageIO is the oracle there.
    assert_eq!((r.raw_oiio.files, r.raw_oiio.pass), (1, 1));
    assert_eq!(r.lin.files, 2);
    assert_eq!(r.lin.pass, r.lin.files);
}

#[test]
#[ignore]
fn corpus_against_oracles() {
    let Some(root) = std::env::var_os("SPECULAR_RASTER_CORPUS_FILM") else {
        eprintln!("SPECULAR_RASTER_CORPUS_FILM not set");
        return;
    };
    let root = Path::new(&root);
    let mut r = Report {
        oracle_lines: true,
        ..Report::default()
    };
    for f in files(&root.join("files")) {
        r.check(&f, &root.join("expect"));
    }
    r.print();
}

/// Luma-only DPX (descriptor 6): Sembiance's single-channel samples and the luma files
/// OpenImageIO and ImageMagick write at 8 to 16 bits, against both
/// (tools/corpus/fetch-oracle-samples.sh film). Reads `SPECULAR_ORACLE_SAMPLES_CORPUS`/film, else
/// `$CARGO_TARGET_DIR/corpus/oracle-samples/film`.
#[test]
#[ignore]
fn oracle_samples_luma_against_oracles() {
    let Some(root) = std::env::var_os("SPECULAR_ORACLE_SAMPLES_CORPUS")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("CARGO_TARGET_DIR")
                .map(|t| std::path::PathBuf::from(t).join("corpus/oracle-samples"))
        })
        .map(|b| b.join("film"))
        .filter(|d| d.join("files").is_dir())
    else {
        eprintln!(
            "oracle-samples film corpus missing: run tools/corpus/fetch-oracle-samples.sh film"
        );
        return;
    };
    let mut r = Report {
        oracle_lines: true,
        ..Report::default()
    };
    for f in files(&root.join("files")) {
        r.check(&f, &root.join("expect"));
    }
    r.print();
    assert!(r.refused.is_empty(), "{:?}", r.refused);
}
