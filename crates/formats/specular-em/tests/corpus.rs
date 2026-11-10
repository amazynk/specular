//! Compares DM and MRC planes and calibrations with the oracles (`gen/oracle.py`: RosettaSciIO
//! for DM, mrcfile for MRC) on the committed fixtures (`tests/data`) and on the corpus
//! (`$SPECULAR_EM_CORPUS`, else `tools/corpus/em`, fetched by `tools/corpus/fetch-em.sh`;
//! skipped when absent), and opens every corpus file.

#[path = "common/expect.rs"]
mod expect;

use std::path::{Path, PathBuf};
use std::time::Instant;

use expect::{Expect, close, compare};
use specular_em::{cube, dm, mrc};

fn corpus() -> Option<PathBuf> {
    let dir = match std::env::var_os("SPECULAR_EM_CORPUS") {
        Some(d) => PathBuf::from(d),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/em"),
    };
    dir.join("expected").is_dir().then_some(dir)
}

/// The diffraction-and-volumes corpus: crystallographic maps and Gaussian cube files
/// (`tools/corpus/fetch-diffraction.sh`).
fn diffraction_corpus() -> Option<PathBuf> {
    let dir = match std::env::var_os("SPECULAR_DIFFRACTION_CORPUS") {
        Some(d) => PathBuf::from(d),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/diffraction"),
    };
    dir.join("expected").is_dir().then_some(dir)
}

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data")
}

fn ext(rel: &str) -> String {
    Path::new(rel)
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default()
}

#[derive(Default)]
struct Tally {
    images: usize,
    planes: usize,
    failures: Vec<String>,
}

/// Odd-width new-packed FFTs: RosettaSciIO unpacks them to one column fewer than the image's
/// dimensions say; the columns it has are compared.
fn odd_packed(e: &Expect, w: usize) -> bool {
    e.size.0 + 1 == w && w % 2 == 1
}

fn check_dm(e: &Expect, file: &[u8], t: &mut Tally) {
    let parsed = dm::parse(file);
    if e.error.is_some() {
        return;
    }
    let d = match parsed {
        Ok(d) => d,
        Err(err) => {
            t.failures.push(format!("{}: {err}", e.name()));
            return;
        }
    };
    let Some(im) = d.images.get(e.k) else {
        t.failures
            .push(format!("{}: only {} images", e.name(), d.images.len()));
        return;
    };
    let (w, h, p) = (im.width(), im.height(), im.planes());
    let odd = odd_packed(e, w) && h == e.size.1;
    if (w, h, p) != e.size && !odd {
        t.failures.push(format!(
            "{}: size {:?} vs {:?}",
            e.name(),
            (w, h, p),
            e.size
        ));
        return;
    }
    t.images += 1;
    for (i, &pi) in e.planes.iter().enumerate() {
        let mut out = vec![0f32; w * h];
        if let Err(err) = im.read_plane(file, pi, &mut out) {
            t.failures.push(format!("{} plane {pi}: {err}", e.name()));
            continue;
        }
        let got: Vec<f32> = if odd {
            out.chunks_exact(w)
                .flat_map(|r| r[..w - 1].iter().copied())
                .collect()
        } else {
            out
        };
        let (bad, first) = compare(&got, e.plane(i), e.complex || im.is_complex());
        t.planes += 1;
        if bad > 0 {
            t.failures.push(format!(
                "{} plane {pi}: {bad} mismatches, first {}",
                e.name(),
                first.unwrap()
            ));
        }
    }
    // Axes, x first; the oracle drops trailing dimensions of 1 as we do.
    for (i, (scale, offset, unit)) in e.axes.iter().enumerate() {
        let Some(c) = im.calibrations.get(i) else {
            t.failures.push(format!("{}: no axis {i}", e.name()));
            continue;
        };
        let off = -c.origin * c.scale;
        if !close(c.scale, *scale) || !close(off, *offset) || c.units != *unit {
            t.failures.push(format!(
                "{} axis {i}: {} {} {:?} vs {scale} {offset} {unit:?}",
                e.name(),
                c.scale,
                off,
                c.units
            ));
        }
    }
}

fn check_mrc(e: &Expect, file: &[u8], t: &mut Tally) {
    let parsed = mrc::parse(file);
    if e.error.is_some() {
        return;
    }
    let m = match parsed {
        Ok(m) => m,
        Err(err) => {
            t.failures.push(format!("{}: {err}", e.name()));
            return;
        }
    };
    // Sections in x, y, z order (gemmi's for maps stored in another axis order).
    let [sx, sy, sz] = m.size_xyz();
    if (sx, sy, sz) != e.size {
        t.failures.push(format!(
            "{}: size {:?} vs {:?}",
            e.name(),
            (sx, sy, sz),
            e.size
        ));
        return;
    }
    t.images += 1;
    if let Some(want) = e.origin {
        let got = m.origin_xyz();
        if (0..3).any(|a| (got[a] - want[a]).abs() > 1e-5 * want[a].abs().max(1.0)) {
            t.failures
                .push(format!("{}: origin {got:?} vs {want:?}", e.name()));
        }
    }
    for (i, &z) in e.planes.iter().enumerate() {
        let mut out = vec![0f32; sx * sy];
        if let Err(err) = m.read_xyz(file, z, &mut out) {
            t.failures.push(format!("{} section {z}: {err}", e.name()));
            continue;
        }
        let (bad, first) = compare(&out, e.plane(i), e.complex);
        t.planes += 1;
        if bad > 0 {
            t.failures.push(format!(
                "{} section {z}: {bad} mismatches, first {}",
                e.name(),
                first.unwrap()
            ));
        }
    }
    // mrcfile's voxel size is cella / mx in float32; ours is in f64 and refuses nonsense.
    if let (Some(want), Some(got)) = (e.voxel, m.voxel) {
        for a in 0..3 {
            let want = want[a].abs();
            if want != 0.0 && (got[a] - want).abs() > 1e-6 * want {
                t.failures
                    .push(format!("{}: voxel {got:?} vs {:?}", e.name(), e.voxel));
                break;
            }
        }
    }
}

/// A cube's values against ASE's: section z of the oracle is (y, x) at that z.
fn check_cube(e: &Expect, file: &[u8], t: &mut Tally) {
    if e.error.is_some() {
        return;
    }
    let start = Instant::now();
    let c = match cube::parse(file) {
        Ok(c) => c,
        Err(err) => {
            t.failures.push(format!("{}: {err}", e.name()));
            return;
        }
    };
    eprintln!(
        "{}: {:?} parsed in {:.1} ms",
        e.rel,
        c.size,
        start.elapsed().as_secs_f64() * 1e3
    );
    let [nx, ny, nz] = c.size;
    if (nx, ny, nz) != e.size {
        t.failures
            .push(format!("{}: size {:?} vs {:?}", e.name(), c.size, e.size));
        return;
    }
    t.images += 1;
    let per = c.per_point;
    for (i, &z) in e.planes.iter().enumerate() {
        let mut out = vec![0f32; nx * ny];
        for y in 0..ny {
            for x in 0..nx {
                out[y * nx + x] = c.values[((x * ny + y) * nz + z) * per];
            }
        }
        let (bad, first) = compare(&out, e.plane(i), false);
        t.planes += 1;
        if bad > 0 {
            t.failures.push(format!(
                "{} section {z}: {bad} mismatches, first {}",
                e.name(),
                first.unwrap()
            ));
        }
    }
}

/// Big-endian DM pixel data: RosettaSciIO reads the tags in the header's byte order but the
/// pixels as little-endian (and ncempy refuses the file), so these are checked against the
/// generator's formula in `big_endian_dm` instead.
const ORACLE_LE_ONLY: &[&str] = &["be.dm3"];

fn run(root: &Path, label: &str) -> Tally {
    // Starts the `ORACLE` lines on their own line (libtest prints `test NAME ... ` first).
    println!();
    let mut t = Tally::default();
    let mut cache: Option<(String, Vec<u8>)> = None;
    let mut verdicts: std::collections::BTreeMap<String, bool> = Default::default();
    for e in expect::all(root) {
        let x = ext(&e.rel);
        if !matches!(
            x.as_str(),
            "dm3" | "dm4" | "mrc" | "mrcs" | "map" | "ccp4" | "cube"
        ) || ORACLE_LE_ONLY.contains(&e.rel.as_str())
        {
            continue;
        }
        if cache.as_ref().is_none_or(|(r, _)| *r != e.rel) {
            cache = Some((e.rel.clone(), std::fs::read(root.join(&e.rel)).unwrap()));
        }
        let file = &cache.as_ref().unwrap().1;
        let before = t.failures.len();
        if x.starts_with("dm") {
            check_dm(&e, file, &mut t);
        } else if x == "cube" {
            check_cube(&e, file, &mut t);
        } else {
            check_mrc(&e, file, &mut t);
        }
        *verdicts.entry(e.rel.clone()).or_default() |= t.failures.len() > before;
    }
    // One line per file for tools/ci/oracle-pass-rates.sh (a file fails when any of its
    // images does).
    for (rel, failed) in &verdicts {
        let fmt = if label == "fixtures" {
            "FIXTURE".to_string()
        } else {
            ext(rel).to_ascii_uppercase()
        };
        let status = if *failed { "fail" } else { "pass" };
        println!("ORACLE\tEM/{fmt}\t{rel}\t{status}");
    }
    eprintln!(
        "{label}: {} images, {} planes compared, {} failures",
        t.images,
        t.planes,
        t.failures.len()
    );
    for f in &t.failures {
        eprintln!("  {f}");
    }
    t
}

#[test]
fn fixtures_match_oracle() {
    let t = run(&fixtures(), "fixtures");
    assert!(t.images >= 20);
    assert!(t.failures.is_empty());
}

/// `be.dm3`: a big-endian DM3 with two `i16` planes of 6 x 4 holding the generator's ramp
/// `(7x + 13y + 31z) % 97 - 40`.
#[test]
fn big_endian_dm() {
    let f = std::fs::read(fixtures().join("be.dm3")).unwrap();
    let d = dm::parse(&f).unwrap();
    assert!(!d.version.little);
    let im = &d.images[0];
    assert_eq!((im.width(), im.height(), im.planes()), (6, 4, 2));
    for z in 0..2 {
        let mut out = vec![0f32; 24];
        im.read_plane(&f, z, &mut out).unwrap();
        for y in 0..4 {
            for x in 0..6 {
                let want = ((7 * x + 13 * y + 31 * z) % 97) as f32 - 40.0;
                assert_eq!(out[y * 6 + x], want, "plane {z} at {x},{y}");
            }
        }
    }
    assert_eq!(im.calibrations[0].units, "nm");
}

#[test]
fn fixture_metadata() {
    let f = std::fs::read(fixtures().join("stack.dm4")).unwrap();
    let d = dm::parse(&f).unwrap();
    let im = &d.images[0];
    assert_eq!(im.meta.voltage, Some(300000.0));
    assert_eq!(im.meta.magnification, Some(88000.0));
    assert_eq!(im.meta.exposure, Some(0.5));
    assert_eq!(im.meta.microscope.as_deref(), Some("Synthetic TEM"));
    assert_eq!(im.meta.acquired.as_deref(), Some("1/2/2026 3:04:05 PM"));
    assert_eq!(im.calibrations[2].units, "eV");
    let f = std::fs::read(fixtures().join("fei2.mrc")).unwrap();
    let m = mrc::parse(&f).unwrap();
    let fei = m.fei.unwrap();
    assert_eq!(fei.voltage, Some(300000.0));
    assert_eq!(fei.magnification, Some(105000.0));
    assert_eq!(fei.mode, Some("TEM"));
    assert_eq!(fei.pixel_size, Some((1.1e-10, 1.1e-10)));
    assert_eq!(fei.exposure, Some(2.5));
    assert_eq!(fei.microscope.as_deref(), Some("Synthetic Krios"));
    assert_eq!(fei.camera.as_deref(), Some("Falcon synthetic"));
    let rgb = dm::parse(&std::fs::read(fixtures().join("rgb.dm4")).unwrap()).unwrap();
    assert!(rgb.images[0].is_rgb());
}

#[test]
fn corpus_matches_oracle() {
    let Some(root) = corpus() else {
        eprintln!("EM corpus not found; skipping");
        return;
    };
    let t = run(&root, "corpus");
    assert!(t.failures.is_empty());
}

/// Crystallographic maps (mrcfile, cross-checked with gemmi) and Gaussian cube files (ASE).
#[test]
fn diffraction_corpus_matches_oracle() {
    let Some(root) = diffraction_corpus() else {
        eprintln!("diffraction corpus not found; skipping");
        return;
    };
    let t = run(&root, "diffraction corpus");
    assert!(t.images > 0);
    assert!(t.failures.is_empty());
}

fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            if p.file_name().is_some_and(|n| n != "expected") {
                files(&p, out);
            }
        } else if matches!(
            p.extension()
                .map(|e| e.to_string_lossy().to_ascii_lowercase())
                .as_deref(),
            Some("dm3" | "dm4" | "mrc" | "mrcs" | "map" | "ccp4")
        ) {
            out.push(p);
        }
    }
}

/// Every corpus file opens and every plane reads.
#[test]
fn corpus_opens() {
    let Some(root) = corpus() else {
        eprintln!("EM corpus not found; skipping");
        return;
    };
    let mut all = Vec::new();
    files(&root, &mut all);
    let mut bad = Vec::new();
    for p in &all {
        let f = std::fs::read(p).unwrap();
        let r = if p
            .extension()
            .is_some_and(|e| e.to_string_lossy().starts_with("dm"))
        {
            dm::parse(&f).map(|d| {
                for im in &d.images {
                    let mut out = vec![0f32; im.width() * im.height()];
                    im.read_plane(&f, im.planes() - 1, &mut out).unwrap();
                }
            })
        } else {
            mrc::parse(&f).map(|m| {
                let mut out = vec![0f32; m.nx * m.ny];
                m.read_plane(&f, m.nz - 1, &mut out).unwrap();
            })
        };
        if let Err(e) = r {
            bad.push(format!("{}: {e}", p.strip_prefix(&root).unwrap().display()));
        }
    }
    eprintln!("{} files, {} refused", all.len(), bad.len());
    for b in &bad {
        eprintln!("  {b}");
    }
    assert!(all.len() > 50);
    assert!(bad.is_empty());
}

/// Open time (header plus every plane) of the largest file of each format.
#[test]
#[ignore = "timing; run with --ignored --release"]
fn open_times() {
    let Some(root) = corpus() else {
        return;
    };
    let mut all = Vec::new();
    files(&root, &mut all);
    for want in ["dm", "mrc"] {
        let Some(p) = all
            .iter()
            .filter(|p| {
                p.extension()
                    .is_some_and(|e| e.to_string_lossy().starts_with(want))
            })
            .max_by_key(|p| std::fs::metadata(p).unwrap().len())
        else {
            continue;
        };
        let f = std::fs::read(p).unwrap();
        let mut best = f64::INFINITY;
        for _ in 0..5 {
            let t = Instant::now();
            if want == "dm" {
                let d = dm::parse(&f).unwrap();
                let im = &d.images[0];
                let mut out = vec![0f32; im.width() * im.height()];
                im.read_plane(&f, 0, &mut out).unwrap();
            } else {
                let m = mrc::parse(&f).unwrap();
                let mut out = vec![0f32; m.nx * m.ny];
                m.read_plane(&f, 0, &mut out).unwrap();
            }
            best = best.min(t.elapsed().as_secs_f64());
        }
        eprintln!(
            "{}: {:.1} MB, header + first plane {:.2} ms",
            p.strip_prefix(&root).unwrap().display(),
            f.len() as f64 / 1e6,
            best * 1e3
        );
    }
}
