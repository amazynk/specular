//! LJ2K (compression 12, lossy HTJ2K) against OpenEXR: files written by OpenEXR 3.5.3 (header
//! magic `HL`, NLT type 4 in the codestream) and 3.5.1 (legacy: magic `HT`, transfer function
//! before coding) with `tools/corpus/exr-lj2k.py`. Each `.lj2k.exr` comes with `.ref.exr`
//! (OpenEXR's own decode of it) and `.raw.exr` (the source pixels). The 9/7 wavelet and the
//! table interpolation run in floating point in both decoders, so samples may differ in the
//! last bits: they must stay within a small relative error of OpenEXR's, and our error against
//! the source must be no larger than OpenEXR's own. Lossless channels (past the first three)
//! must match exactly. The small set in `tests/data/lj2k` always runs; the full set
//! (`<conformance>/exr-lj2k`) when present.

use std::path::{Path, PathBuf};

use specular_exr::{ChannelSelect, Compression, DecodeOptions, Pixels, SampleFormat};

fn channel(file: &[u8], name: &str) -> Vec<f32> {
    let opts = DecodeOptions {
        channels: ChannelSelect::Slots([Some(name.into()), None, None, None]),
        format: SampleFormat::F32,
        ..Default::default()
    };
    let img = specular_exr::decode(file, &opts).unwrap_or_else(|e| panic!("{name}: {e}"));
    assert_eq!(img.damaged_chunks, 0, "{name}: damaged chunks");
    match img.pixels {
        Pixels::F32(v) => v.chunks_exact(4).map(|p| p[0]).collect(),
        Pixels::F16(_) => unreachable!(),
    }
}

/// Relative difference, with values below `floor` compared on the absolute scale.
fn rel(a: f32, b: f32, floor: f32) -> f32 {
    if a == b || (a.is_nan() && b.is_nan()) {
        return 0.0;
    }
    (a - b).abs() / b.abs().max(floor)
}

#[derive(Default, Debug)]
struct Stats {
    samples: usize,
    exact: usize,
    /// Largest relative difference from OpenEXR's decode.
    worst_ref: f32,
    /// Mean relative error against the source: ours, OpenEXR's.
    err_ours: f64,
    err_ref: f64,
    lossless_mismatch: usize,
}

fn compare(dir: &Path, stem: &str) -> Stats {
    let read = |s: &str| std::fs::read(dir.join(format!("{stem}.{s}.exr"))).unwrap();
    let (lj, re, raw) = (read("lj2k"), read("ref"), read("raw"));
    let info = specular_exr::read_info(&lj).unwrap();
    let part = &info.parts[0];
    assert_eq!(part.compression, Compression::Lj2k, "{stem}");
    let mut st = Stats::default();
    // Channels are stored in name order; the codestream's first three are the lossy ones
    // when the part has R, G and B.
    let rgb = ["R", "G", "B"];
    for ch in &part.channels {
        let (a, b, s) = (
            channel(&lj, &ch.name),
            channel(&re, &ch.name),
            channel(&raw, &ch.name),
        );
        let lossy = rgb.contains(&ch.name.as_str());
        for ((&x, &y), &z) in a.iter().zip(&b).zip(&s) {
            st.samples += 1;
            st.exact += (x.to_bits() == y.to_bits()) as usize;
            if !lossy {
                st.lossless_mismatch += (x.to_bits() != y.to_bits()) as usize;
                continue;
            }
            st.worst_ref = st.worst_ref.max(rel(x, y, 1e-3));
            st.err_ours += rel(x, z, 1e-3) as f64;
            st.err_ref += rel(y, z, 1e-3) as f64;
        }
    }
    st
}

fn stems(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| {
            let name = e.ok()?.file_name().into_string().ok()?;
            name.strip_suffix(".lj2k.exr").map(str::to_owned)
        })
        .collect();
    v.sort();
    v
}

fn check(dir: &Path, expect_files: Option<usize>) {
    let stems = stems(dir);
    if let Some(n) = expect_files {
        assert_eq!(stems.len(), n);
    }
    let mut fails = Vec::new();
    for stem in stems {
        let st = compare(dir, &stem);
        let n = st.samples as f64;
        eprintln!(
            "{stem}: {} of {} samples exact; worst relative difference {:.2e}; mean error \
             against the source {:.2e} (OpenEXR {:.2e})",
            st.exact,
            st.samples,
            st.worst_ref,
            st.err_ours / n,
            st.err_ref / n
        );
        if st.lossless_mismatch != 0
            || st.worst_ref.is_nan()
            || st.worst_ref >= 2e-2
            || st.err_ours > st.err_ref * 1.05 + 1e-6
        {
            fails.push(stem);
        }
    }
    assert!(fails.is_empty(), "outside the tolerance: {fails:?}");
}

#[test]
fn small_set_matches_openexr() {
    check(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/lj2k"),
        Some(4),
    );
}

#[test]
fn full_set_matches_openexr() {
    let root = std::env::var_os("SPECULAR_CONFORMANCE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/conformance")
        });
    let dir = root.join("exr-lj2k");
    if !dir.is_dir() {
        eprintln!("no {}; skipped", dir.display());
        return;
    }
    check(&dir, None);
}
