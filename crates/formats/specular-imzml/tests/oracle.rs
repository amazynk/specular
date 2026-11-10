//! The fixtures in tests/data (written by gen/make_imzml.py) against pyimzML's reading of
//! them (tests/data/expected.tsv, written by gen/oracle.py): spectrum counts, every
//! spectrum's pixel, array lengths, sums and strided values, the total ion current and the
//! summed intensity in several m/z windows of every spectrum, and the images they make.
//!
//! When `SPECULAR_IMZML_ORACLE` names another TSV and `SPECULAR_IMZML_DIR` its files (the
//! fetched corpus, tools/corpus/fetch-imzml.sh), those are checked the same way.

use std::collections::BTreeMap;
use std::path::Path;

use specular_imzml::{Error, ImzMl, Polarity, Tolerance, Window};

#[derive(Default)]
struct Case {
    file: Vec<String>,
    coord: Vec<(usize, u32, u32, u32)>,
    spec: Vec<(usize, usize, f64, f64, f64, f64)>,
    val: Vec<(usize, usize, f64, f64)>,
    tic: Vec<(usize, f64)>,
    win: Vec<(f64, f64)>,
    ion: Vec<(usize, usize, f64, f64)>,
    error: Option<String>,
}

fn load(tsv: &Path) -> BTreeMap<String, Case> {
    let text = std::fs::read_to_string(tsv).expect("oracle tsv");
    let mut cases: BTreeMap<String, Case> = BTreeMap::new();
    for line in text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
    {
        let f: Vec<&str> = line.split('\t').collect();
        let c = cases.entry(f[1].to_owned()).or_default();
        let n = |i: usize| f[i].parse::<usize>().unwrap();
        let u = |i: usize| f[i].parse::<u32>().unwrap();
        let x = |i: usize| f[i].parse::<f64>().unwrap();
        match f[0] {
            "file" => c.file = f[2..].iter().map(|s| (*s).to_owned()).collect(),
            "coord" => c.coord.push((n(2), u(3), u(4), u(5))),
            "spec" => c.spec.push((n(2), n(3), x(4), x(5), x(6), x(7))),
            "val" => c.val.push((n(2), n(3), x(4), x(5))),
            "tic" => c.tic.push((n(2), x(3))),
            "win" => c.win.push((x(3), x(4))),
            "ion" => c.ion.push((n(2), n(3), x(4), x(5))),
            "error" => c.error = Some(f[2].to_owned()),
            t => panic!("unknown row {t}"),
        }
    }
    cases
}

/// Equal to f64 rounding of sums taken in another order (both sides sum in f64).
fn close(a: f64, b: f64) -> bool {
    (a.is_nan() && b.is_nan()) || a == b || (a - b).abs() <= 1e-9 * a.abs().max(b.abs())
}

/// Our f32 images against pyimzML's f64 sums.
fn same32(a: f32, b: f64) -> bool {
    a == b as f32 || (f64::from(a) - b).abs() <= 1e-6 * b.abs()
}

#[derive(Default, Debug)]
struct Tally {
    files: usize,
    refused: usize,
    spectra: usize,
    values: usize,
    windows: usize,
    mismatches: usize,
    /// Window sums where pyimzML's `_bisect_spectrum` takes the first value although the
    /// window lies below it or between the first two values (its `ix_u < 1` branch).
    pyimzml_edge: usize,
    /// Files whose polarity pyimzML judged from the first spectrum only (we read all).
    polarity_first_only: usize,
}

fn check(dir: &Path, cases: &BTreeMap<String, Case>) -> Tally {
    let mut t = Tally::default();
    let bad = |what: String, t: &mut Tally| {
        if t.mismatches < 40 {
            eprintln!("mismatch: {what}");
        }
        t.mismatches += 1;
    };
    for (name, c) in cases {
        let path = dir.join(name);
        let xml = std::fs::read(&path).unwrap();
        let ibd = std::fs::read(path.with_extension("ibd")).unwrap_or_default();
        let opened = ImzMl::parse(&xml).and_then(|m| m.check_ibd(&ibd).map(|()| m));
        if let Some(e) = &c.error {
            match opened {
                Err(Error::Compressed { .. }) if e == "compressed" => t.refused += 1,
                Err(ours) => {
                    eprintln!("{name}: pyimzML: {e}; ours: {ours}");
                    t.refused += 1;
                }
                Ok(_) => bad(
                    format!("{name}: pyimzML failed ({e}), we opened it"),
                    &mut t,
                ),
            }
            continue;
        }
        let m = match opened {
            Ok(m) => m,
            Err(e) => {
                bad(format!("{name}: {e}"), &mut t);
                continue;
            }
        };
        t.files += 1;
        let f = &c.file;
        let n: usize = f[0].parse().unwrap();
        if m.entries.len() != n {
            bad(
                format!("{name}: {} spectra, want {n}", m.entries.len()),
                &mut t,
            );
            continue;
        }
        let maxes = (
            m.origin.0 + m.width - 1,
            m.origin.1 + m.height - 1,
            *m.zs.last().unwrap(),
        );
        let want: (u32, u32, u32) = (
            f[1].parse().unwrap(),
            f[2].parse().unwrap(),
            f[3].parse().unwrap(),
        );
        if maxes != want {
            bad(
                format!("{name}: max x/y/z {maxes:?}, want {want:?}"),
                &mut t,
            );
        }
        let pol = match m.polarity {
            Some(Polarity::Positive) => "positive",
            Some(Polarity::Negative) => "negative",
            Some(Polarity::Mixed) => "mixed",
            None => "None",
        };
        if pol != f[4] {
            if pol == "mixed" {
                t.polarity_first_only += 1;
            } else {
                bad(format!("{name}: polarity {pol}, want {}", f[4]), &mut t);
            }
        }
        let mode = match m.centroid {
            Some(true) => "centroid",
            Some(false) => "profile",
            None => "None",
        };
        if mode != f[5] {
            bad(
                format!("{name}: spectrum mode {mode}, want {}", f[5]),
                &mut t,
            );
        }
        for &(i, x, y, z) in &c.coord {
            let e = &m.entries[i];
            if (e.x, e.y, e.z) != (x, y, z) {
                bad(
                    format!(
                        "{name}: spectrum {i} at {:?}, want {:?}",
                        (e.x, e.y, e.z),
                        (x, y, z)
                    ),
                    &mut t,
                );
            }
        }
        for &(i, k, mz_sum, int_sum, first, last) in &c.spec {
            t.spectra += 1;
            let s = m.spectrum(&ibd, i);
            let ms: f64 = s.mz.iter().sum();
            let is: f64 = s.intensity.iter().map(|&v| f64::from(v)).sum();
            let ok = s.mz.len() == k
                && close(ms, mz_sum)
                && (k == 0 || (s.mz[0] == first && s.mz[k - 1] == last))
                && same32(is as f32, int_sum);
            if !ok {
                bad(
                    format!(
                        "{name}: spectrum {i}: {} values, sums {ms} {is}; want {k}, {mz_sum} {int_sum}",
                        s.mz.len()
                    ),
                    &mut t,
                );
            }
        }
        let mut cache: BTreeMap<usize, specular_imzml::Spectrum> = BTreeMap::new();
        for &(i, k, mz, int) in &c.val {
            t.values += 1;
            let s = cache.entry(i).or_insert_with(|| m.spectrum(&ibd, i));
            if s.mz.get(k) != Some(&mz) || !same32(s.intensity[k], int) {
                bad(
                    format!(
                        "{name}: spectrum {i} value {k}: {:?} {:?}, want {mz} {int}",
                        s.mz.get(k),
                        s.intensity.get(k)
                    ),
                    &mut t,
                );
            }
        }
        let never = || false;
        let tic = m.tic(&ibd, &never).unwrap();
        for &(i, v) in &c.tic {
            if !same32(tic[i], v) {
                bad(
                    format!("{name}: TIC of spectrum {i}: {}, want {v}", tic[i]),
                    &mut t,
                );
            }
        }
        // The images put each spectrum's value at its pixel.
        for zi in 0..m.depth() {
            let img = m.image(&tic, zi);
            for (i, e) in m.entries.iter().enumerate() {
                if e.z != m.zs[zi] {
                    continue;
                }
                let at = ((e.y - m.origin.1) * m.width + e.x - m.origin.0) as usize;
                // Later spectra at the same pixel win, as in pyimzML's images.
                let last = m.index_at(e.x - m.origin.0, e.y - m.origin.1, zi);
                if last == Some(i) && img[at].to_bits() != tic[i].to_bits() {
                    bad(format!("{name}: TIC image at spectrum {i}"), &mut t);
                }
            }
        }
        for (w, &(mz, tol)) in c.win.iter().enumerate() {
            t.windows += 1;
            let ion = m
                .ion(
                    &ibd,
                    Window {
                        mz,
                        tolerance: Tolerance::Da(tol),
                    },
                    &never,
                )
                .unwrap();
            for &(_, i, v, pv) in c.ion.iter().filter(|r| r.0 == w) {
                if !same32(ion[i], v) {
                    bad(
                        format!(
                            "{name}: window {w} ({mz} ± {tol}) of spectrum {i}: {}, want {v}",
                            ion[i]
                        ),
                        &mut t,
                    );
                }
                if v != pv {
                    t.pyimzml_edge += 1;
                }
            }
        }
    }
    t
}

#[test]
fn fixtures_match_pyimzml() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data");
    let cases = load(&dir.join("expected.tsv"));
    assert_eq!(cases.len(), 5);
    let t = check(&dir, &cases);
    eprintln!("fixtures: {t:?}");
    assert_eq!(t.mismatches, 0);
    assert_eq!(t.files, 5);
    assert!(t.spectra > 300 && t.windows == 25);
}

#[test]
fn corpus_matches_pyimzml() {
    let (Some(tsv), Some(dir)) = (
        std::env::var_os("SPECULAR_IMZML_ORACLE"),
        std::env::var_os("SPECULAR_IMZML_DIR"),
    ) else {
        return;
    };
    let cases = load(Path::new(&tsv));
    let t = check(Path::new(&dir), &cases);
    eprintln!("corpus: {t:?}");
    assert_eq!(t.mismatches, 0);
}
