//! Hamamatsu VMS / VMU (tools/corpus/fetch-cytometry.sh: synthetic slides written by
//! tools/corpus/cytometry-oracle.py and a VMS made of the real CMU-1's bottom row) against
//! OpenSlide's reading of them (`expected.tsv`: slide facts, level regions, associated
//! images). Reads SPECULAR_CYTOMETRY_CORPUS, else tools/corpus/cytometry; skips when it has no
//! expected.tsv.
//!
//! JPEG files are decoded by different decoders (and OpenSlide's DCT scaling is libjpeg's),
//! so regions match within a mean absolute difference and a share of clearly different
//! samples; uncompressed VMU pixels match exactly.

use std::path::{Path, PathBuf};

use specular_wsi::{AssociatedKind, Files, ReadOptions, Region, Slide, vms};

fn corpus() -> Option<PathBuf> {
    let dir = std::env::var_os("SPECULAR_CYTOMETRY_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/cytometry")
        });
    dir.join("expected.tsv").is_file().then_some(dir)
}

/// The index and its files' bytes, in [`vms::files`] order.
fn load(path: &Path) -> Vec<Vec<u8>> {
    let index = std::fs::read(path).unwrap();
    let folder = path.parent().unwrap();
    let mut out = vec![index.clone()];
    for name in vms::files(&index).unwrap() {
        out.push(std::fs::read(folder.join(&name)).unwrap_or_default());
    }
    out
}

/// Mean absolute difference and share of samples further apart than 24.
fn compare(ours: &[u8], want: &[u8]) -> (f64, f64) {
    if ours.len() / 4 * 3 != want.len() {
        return (f64::INFINITY, 1.0);
    }
    let (mut sum, mut far, mut n) = (0u64, 0u64, 0u64);
    for (a, b) in ours.chunks_exact(4).zip(want.chunks_exact(3)) {
        for k in 0..3 {
            let d = u64::from(a[k].abs_diff(b[k]));
            sum += d;
            far += u64::from(d > 24);
            n += 1;
        }
    }
    (sum as f64 / n.max(1) as f64, far as f64 / n.max(1) as f64)
}

const MAX_MAD: f64 = 3.5;
const MAX_FAR: f64 = 0.01;

#[test]
fn corpus_matches_openslide() {
    let Some(dir) = corpus() else {
        eprintln!("cytometry corpus absent: run tools/corpus/fetch-cytometry.sh");
        return;
    };
    let text = std::fs::read_to_string(dir.join("expected.tsv")).unwrap();
    let mut open: Option<(String, Vec<Vec<u8>>, Slide)> = None;
    let mut mismatches = Vec::new();
    let (mut checked, mut worst) = (0usize, 0f64);
    for line in text.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        if !matches!(f[0], "slide" | "region" | "assoc") {
            continue;
        }
        let rel = f[1].to_owned();
        if open.as_ref().is_none_or(|o| o.0 != rel) {
            let data = load(&dir.join(&rel));
            let refs: Vec<&[u8]> = data.iter().map(Vec::as_slice).collect();
            let s = Slide::open_vms(Files { data: &refs }).unwrap_or_else(|e| panic!("{rel}: {e}"));
            open = Some((rel.clone(), data, s));
        }
        let (_, data, s) = open.as_ref().unwrap();
        let refs: Vec<&[u8]> = data.iter().map(Vec::as_slice).collect();
        let files = Files { data: &refs };
        let n = |i: usize| f[i].parse::<u32>().unwrap();
        checked += 1;
        match f[0] {
            "slide" => {
                if (s.width, s.height) != (n(2), n(3)) {
                    mismatches.push(format!("{rel}: size {}x{}", s.width, s.height));
                }
                if let Ok(m) = f[4].parse::<f64>()
                    && s.props.mpp.is_none_or(|(x, _)| (x - m).abs() > 1e-6 * m)
                {
                    mismatches.push(format!("{rel}: MPP {:?}, OpenSlide {m}", s.props.mpp));
                }
                if let Ok(o) = f[5].parse::<f64>()
                    && s.props.objective != Some(o)
                {
                    mismatches.push(format!("{rel}: objective {:?}", s.props.objective));
                }
            }
            "region" => {
                let r = Region {
                    x: n(3),
                    y: n(4),
                    w: n(5),
                    h: n(6),
                };
                let want = std::fs::read(dir.join(f[7])).unwrap();
                let o = ReadOptions {
                    scale: n(2),
                    region: Some(r),
                    parallel: true,
                    ..Default::default()
                };
                match s.read(files, &o) {
                    Ok(px) => {
                        let (mad, far) = compare(&px.data, &want);
                        worst = worst.max(mad);
                        let exact = rel.ends_with(".vmu") && n(2) == 1;
                        if (exact && mad != 0.0) || mad > MAX_MAD || far > MAX_FAR {
                            mismatches.push(format!(
                                "{rel} 1/{} {r:?}: mean difference {mad:.3}, {:.2}% far",
                                n(2),
                                far * 100.0
                            ));
                        }
                    }
                    Err(e) => mismatches.push(format!("{rel} 1/{} {r:?}: {e}", n(2))),
                }
            }
            "assoc" => {
                let k = s
                    .associated
                    .iter()
                    .position(|a| a.0 == AssociatedKind::Macro && f[2] == "macro");
                match k.map(|k| s.read_associated(files, k, None)) {
                    Some(Ok(px)) if (px.width, px.height) == (n(3), n(4)) => {
                        let want = std::fs::read(dir.join(f[5])).unwrap();
                        let (mad, far) = compare(&px.data, &want);
                        if mad > MAX_MAD || far > MAX_FAR {
                            mismatches.push(format!("{rel} {}: mean difference {mad:.3}", f[2]));
                        }
                    }
                    other => mismatches.push(format!(
                        "{rel} {}: {:?}",
                        f[2],
                        other.map(|r| r.map(|p| (p.width, p.height)))
                    )),
                }
            }
            _ => {}
        }
    }
    eprintln!(
        "VMS/VMU corpus: {checked} checks, {} mismatches, worst region mean difference {worst:.3}",
        mismatches.len()
    );
    for m in &mismatches {
        eprintln!("  {m}");
    }
    assert!(checked > 0);
    assert!(mismatches.is_empty(), "{} mismatches", mismatches.len());
}
