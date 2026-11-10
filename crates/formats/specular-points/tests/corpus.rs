//! The downloaded corpus (tools/corpus/fetch-points.sh): every cloud is read without a panic,
//! and where the oracle (laspy + lazrs, pye57, open3d) read it too, the point count, the
//! bounds and its sampled points agree: each sampled point is found among the decoded ones at
//! its position with its colour, intensity and class.
//! Reads SPECULAR_POINTS_CORPUS, else tools/corpus/points; skips when neither exists.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use specular_points::{Limits, PointCloud, PointFormat, read};

fn corpus() -> Option<PathBuf> {
    let dir = std::env::var_os("SPECULAR_POINTS_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/points")
        });
    dir.is_dir().then_some(dir)
}

fn format_of(p: &Path) -> Option<PointFormat> {
    let ext = p.extension()?.to_str()?.to_ascii_lowercase();
    PointFormat::from_extension(&ext)
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            if p.file_name().is_some_and(|n| n != "oracle") {
                walk(&p, out);
            }
        } else if format_of(&p).is_some() {
            out.push(p);
        }
    }
}

struct Expected {
    points: u64,
    min: [f64; 3],
    max: [f64; 3],
    rgbmax: Option<u32>,
}

fn v3(s: &str) -> [f64; 3] {
    let v: Vec<f64> = s.split(' ').map(|x| x.parse().unwrap()).collect();
    [v[0], v[1], v[2]]
}

fn expected(dir: &Path) -> HashMap<String, Expected> {
    let Ok(s) = std::fs::read_to_string(dir.join("oracle/expected.tsv")) else {
        return HashMap::new();
    };
    s.lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            (f.len() == 5).then(|| {
                (
                    f[0].to_owned(),
                    Expected {
                        points: f[1].parse().unwrap(),
                        min: v3(f[2]),
                        max: v3(f[3]),
                        rgbmax: f[4].parse().ok(),
                    },
                )
            })
        })
        .collect()
}

/// A sampled point: its position, colour (as stored), intensity and class where known.
struct Sample {
    p: [f64; 3],
    rgb: Option<[u32; 3]>,
    intensity: Option<u32>,
    class: Option<u32>,
}

fn samples(path: &Path) -> Vec<Sample> {
    let Ok(s) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    s.lines()
        .filter(|l| !l.is_empty())
        .map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            let c: Vec<Option<u32>> = f[1].split(' ').map(|x| x.parse().ok()).collect();
            Sample {
                p: v3(f[0]),
                rgb: c[0].map(|r| [r, c[1].unwrap(), c[2].unwrap()]),
                intensity: f[2].parse().ok(),
                class: f[3].parse().ok(),
            }
        })
        .collect()
}

/// Position tolerance of a cloud: a millimetre, or the f32 resolution of its positions.
fn tolerance(c: &PointCloud) -> f64 {
    let b = &c.bounds;
    let far = (0..3)
        .map(|i| f64::from(b.min[i].abs().max(b.max[i].abs())))
        .fold(0.0, f64::max);
    1e-3_f64.max(far * 4.0 * f64::from(f32::EPSILON))
}

/// Sampled points not found among the decoded ones (position within the tolerance and the
/// same colour, intensity and class).
fn unmatched(c: &PointCloud, want: &[Sample], format: PointFormat, rgbmax: Option<u32>) -> usize {
    let tol = tolerance(c);
    let key = |p: [f64; 3]| p.map(|v| (v / tol).floor() as i64);
    // Each sample sits in its cell's 27 neighbours, so a decoded point needs one lookup.
    let mut cells: HashMap<[i64; 3], Vec<u32>> = HashMap::new();
    for (i, s) in want.iter().enumerate() {
        let k = key(s.p);
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    cells
                        .entry([k[0] + dx, k[1] + dy, k[2] + dz])
                        .or_default()
                        .push(i as u32);
                }
            }
        }
    }
    // LAS colours are 16-bit unless the file's largest component says they were stored 8-bit.
    let shift = match format {
        PointFormat::Las | PointFormat::Laz if rgbmax.is_some_and(|m| m > 255) => 8,
        _ => 0,
    };
    let las = matches!(format, PointFormat::Las | PointFormat::Laz);
    let mut found = vec![false; want.len()];
    for (p, a) in c.positions.iter().zip(&c.attrs) {
        let w = [0, 1, 2].map(|i| c.origin[i] + f64::from(p[i]));
        let Some(near) = cells.get(&key(w)) else {
            continue;
        };
        for &i in near {
            let s = &want[i as usize];
            if found[i as usize] || (0..3).any(|j| (s.p[j] - w[j]).abs() > tol) {
                continue;
            }
            let got = [a[0] & 0xFF, (a[0] >> 8) & 0xFF, (a[0] >> 16) & 0xFF];
            let rgb_ok = s.rgb.is_none_or(|rgb| {
                // E57 colours may be stored above 8 bits; only 8-bit ones compare directly.
                rgb.iter().any(|&v| (v >> shift) > 255)
                    || (0..3).all(|j| (rgb[j] >> shift).abs_diff(got[j]) <= u32::from(!las))
            });
            let int_ok = !las || s.intensity.is_none_or(|v| v == a[1] & 0xFFFF);
            let class_ok = !las || s.class.is_none_or(|v| v == a[0] >> 24);
            if rgb_ok && int_ok && class_ok {
                found[i as usize] = true;
            }
        }
    }
    found.iter().filter(|f| !**f).count()
}

#[test]
fn corpus_reads_and_matches_oracle() {
    let Some(dir) = corpus() else {
        eprintln!("no point cloud corpus; run tools/corpus/fetch-points.sh");
        return;
    };
    let want = expected(&dir);
    let mut files = Vec::new();
    walk(&dir, &mut files);
    files.sort();
    let limits = Limits::default();
    let (mut read_ok, mut refused, mut compared, mut sampled) = (0, 0, 0, 0);
    let mut bad = Vec::new();
    // ORACLE lines start their own line (the harness's "test ... " has none).
    println!();
    for path in &files {
        let rel = path
            .strip_prefix(&dir)
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let format = format_of(path).unwrap();
        // One line per file for tools/ci/oracle-pass-rates.sh.
        let line = |status: &str| println!("ORACLE\tPoints/{format:?}\t{rel}\t{status}");
        let before = bad.len();
        let bytes = std::fs::read(path).unwrap();
        let t = Instant::now();
        let r = read(format, &bytes, &limits);
        let ms = t.elapsed().as_secs_f64() * 1e3;
        let exp = want.get(&rel);
        let c = match r {
            Ok(c) => c,
            Err(e) => {
                refused += 1;
                eprintln!("  refused {rel}: {e}");
                // An oracle that read points the reader refused is a mismatch; an empty
                // file both agree on is not.
                if exp.is_some_and(|x| x.points > 0) {
                    bad.push(format!("{rel}: refused ({e}) but the oracle read it"));
                    line("fail");
                } else {
                    line(if exp.is_some() { "pass" } else { "skip" });
                }
                continue;
            }
        };
        read_ok += 1;
        eprintln!("  {rel}: {} points in {ms:.1} ms", c.len());
        let Some(exp) = exp else {
            line("skip");
            continue;
        };
        compared += 1;
        // `info.total` counts the file's records, invalid ones (E57, NaN in PCD) included.
        if c.len() as u64 != exp.points {
            bad.push(format!("{rel}: {} points, oracle {}", c.len(), exp.points));
            line("fail");
            continue;
        }
        let tol = tolerance(&c);
        let lo = [0, 1, 2].map(|i| c.origin[i] + f64::from(c.bounds.min[i]));
        let hi = [0, 1, 2].map(|i| c.origin[i] + f64::from(c.bounds.max[i]));
        if (0..3).any(|i| (lo[i] - exp.min[i]).abs() > tol || (hi[i] - exp.max[i]).abs() > tol) {
            bad.push(format!(
                "{rel}: bounds {lo:?}..{hi:?}, oracle {:?}..{:?}",
                exp.min, exp.max
            ));
        }
        let s = samples(&dir.join("oracle").join(format!("{rel}.tsv")));
        sampled += s.len();
        let n = unmatched(&c, &s, format, exp.rgbmax);
        if n > 0 {
            bad.push(format!("{rel}: {n} of {} sampled points differ", s.len()));
        }
        line(if bad.len() > before { "fail" } else { "pass" });
    }
    eprintln!(
        "{} files: {read_ok} read, {refused} refused; {compared} compared with the oracle, \
         {sampled} sampled points",
        files.len()
    );
    for b in &bad {
        eprintln!("MISMATCH {b}");
    }
    assert!(bad.is_empty(), "{} mismatches", bad.len());
}

/// LAZ decode rate over the corpus's two big LAZ files (run with --release --ignored).
#[test]
#[ignore = "timing; run with --release"]
fn laz_decode_rate() {
    let Some(dir) = corpus() else {
        return;
    };
    for rel in [
        "copc/autzen-classified.copc.laz",
        "pdal/draco/red-rocks.laz",
    ] {
        let Ok(bytes) = std::fs::read(dir.join(rel)) else {
            continue;
        };
        let format = format_of(Path::new(rel)).unwrap();
        let limits = Limits {
            max_points: u64::MAX,
            ..Limits::default()
        };
        let mut best = f64::MAX;
        let mut n = 0;
        for _ in 0..3 {
            let t = Instant::now();
            let c = read(format, &bytes, &limits).unwrap();
            best = best.min(t.elapsed().as_secs_f64());
            n = c.len();
        }
        eprintln!(
            "{rel}: {n} points in {:.1} ms, {:.1} M points/s",
            best * 1e3,
            n as f64 / best / 1e6
        );
    }
}
