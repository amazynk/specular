//! MRtrix images in tests/data/mif (written by specular-neuro's gen/make_fixtures.py:
//! permuted and reversed layouts, a 4D image whose volume axis is fastest with its data in
//! another file, big-endian floats, gzip and scaling) against an independent reading of
//! them in numpy (tests/data/mif/expected.tsv, from gen/oracle.py): each sampled image
//! voxel's scaled value and RAS position.

use std::path::PathBuf;

#[test]
fn mif_fixtures_match_oracle() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/mif");
    let checked = check(&dir, &dir.join("expected.tsv"), "mif");
    assert!(checked > 100);
}

/// MRtrix3's test images, when the corpus is fetched (tools/corpus/fetch-neurosurf.sh) or
/// given at `SPECULAR_NEURO_DIR` with `SPECULAR_NEURO_ORACLE`.
#[test]
fn mif_corpus_matches_oracle() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let dir = std::env::var_os("SPECULAR_NEURO_DIR")
        .map_or_else(|| root.join("tools/corpus/neurosurf"), PathBuf::from);
    let tsv = std::env::var_os("SPECULAR_NEURO_ORACLE")
        .map_or_else(|| dir.join("expected.tsv"), PathBuf::from);
    if !tsv.exists() {
        eprintln!(
            "no corpus at {} (tools/corpus/fetch-neurosurf.sh)",
            dir.display()
        );
        return;
    }
    check(&dir, &tsv, "mif corpus");
}

/// Checks every image `tsv` describes under `dir`; the voxels checked.
fn check(dir: &std::path::Path, tsv: &std::path::Path, what: &str) -> usize {
    let text = std::fs::read_to_string(tsv).unwrap();
    let mut vols = std::collections::BTreeMap::new();
    let (mut checked, mut mismatches) = (0, Vec::new());
    for line in text.lines().filter(|l| !l.starts_with('#')) {
        let f: Vec<&str> = line.split('\t').collect();
        if !matches!(f[0], "mif" | "mvox") {
            continue;
        }
        let name = f[1];
        let v = vols.entry(name.to_owned()).or_insert_with(|| {
            let v = specular_medvol::open_path(&dir.join(name))
                .unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(v.header.kind, specular_medvol::Kind::Mif);
            v
        });
        let num = |s: &str| s.parse::<f64>().unwrap();
        match f[0] {
            "mif" => {
                let dims = v
                    .header
                    .dims
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(",");
                if dims != f[2] {
                    mismatches.push(format!("{name}: dims {dims} vs {}", f[2]));
                }
            }
            "mvox" => {
                checked += 1;
                let image: Vec<usize> = f[2..6].iter().map(|s| s.parse().unwrap()).collect();
                // The reader keeps the stored order: an axis the layout stores backwards is
                // turned around (its affine column with it).
                let stored = stored_index(&dir.join(name), &image[..3], &v.header.layout.size);
                let value = v.value(stored[0], stored[1], stored[2], image[3], 0);
                let lps = v.header.to_patient(stored.map(|x| x as f64));
                let ras = [-lps[0], -lps[1], lps[2]];
                let want = [num(f[7]), num(f[8]), num(f[9])];
                let ok = (value - num(f[6])).abs() <= 1e-9 * value.abs().max(1.0)
                    && (0..3).all(|a| (ras[a] - want[a]).abs() < 1e-6);
                if !ok {
                    mismatches.push(format!(
                        "{name} {image:?}: {value} at {ras:?} vs {:?}",
                        &f[6..]
                    ));
                }
            }
            _ => {}
        }
    }
    eprintln!(
        "{what}: {} images, {checked} voxels checked, {} mismatches",
        vols.len(),
        mismatches.len()
    );
    for m in mismatches.iter().take(20) {
        eprintln!("  {m}");
    }
    assert!(mismatches.is_empty());
    checked
}

/// The stored index of image voxel `image`: axes the header's layout marks `-` count from
/// their far end.
fn stored_index(path: &std::path::Path, image: &[usize], size: &[usize; 3]) -> [usize; 3] {
    let raw = std::fs::read(path).unwrap();
    let text = if raw.starts_with(&[0x1f, 0x8b]) {
        specular_png::inflate_gzip_prefix(&raw, 4096).unwrap()
    } else {
        raw
    };
    let text = String::from_utf8_lossy(&text);
    let layout = text
        .lines()
        .find_map(|l| l.strip_prefix("layout:"))
        .unwrap_or("+0,+1,+2");
    let back: Vec<bool> = layout
        .split(',')
        .map(|s| s.trim().starts_with('-'))
        .collect();
    [0, 1, 2].map(|a| {
        if back[a] {
            size[a] - 1 - image[a]
        } else {
            image[a]
        }
    })
}
