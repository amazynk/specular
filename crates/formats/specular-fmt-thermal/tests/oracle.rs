//! The fixtures in tests/data (written by gen/make_fixtures.py) against independent readings
//! of them (tests/data/expected.tsv, written by gen/oracle.py): exiftool's raw thermal image
//! and Planck tags converted in numpy for FLIR, `infiray_irg` for IRG and the `irb` Java
//! reader for IRB. Counts must match exactly, temperatures within 0.01 degrees.
//!
//! When `SPECULAR_THERMAL_ORACLE` names another TSV and `SPECULAR_THERMAL_DIR` its files (the
//! fetched corpus, tools/corpus/fetch-thermal.sh), those are checked the same way.

use std::collections::BTreeMap;
use std::path::Path;

use specular_fmt_thermal::{Conditions, read};

#[derive(Default)]
struct Case {
    size: (usize, usize),
    cond: Option<[f64; 7]>,
    px: Vec<(usize, Option<u16>, f64)>,
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
        match f[0] {
            "file" => c.size = (f[3].parse().unwrap(), f[4].parse().unwrap()),
            "cond" => {
                let mut v = [0.0; 7];
                for (i, x) in f[2..9].iter().enumerate() {
                    v[i] = x.parse().unwrap();
                }
                c.cond = Some(v);
            }
            "px" => c.px.push((
                f[2].parse().unwrap(),
                f[3].parse().ok(),
                f[4].parse().unwrap_or(f64::NAN),
            )),
            "error" => c.error = Some(f[2].to_owned()),
            t => panic!("unknown row {t}"),
        }
    }
    cases
}

#[derive(Default, Debug)]
struct Tally {
    files: usize,
    oracle_refused: usize,
    raw_values: usize,
    raw_mismatches: usize,
    temperatures: usize,
    temperature_mismatches: usize,
    max_abs_diff: f64,
    other_mismatches: usize,
}

fn check(dir: &Path, cases: &BTreeMap<String, Case>) -> Tally {
    let mut t = Tally::default();
    for (name, c) in cases {
        if let Some(e) = &c.error {
            eprintln!("{name}: oracle refused ({e})");
            t.oracle_refused += 1;
            continue;
        }
        let data = std::fs::read(dir.join(name)).expect("file");
        let th = match read(&data) {
            Ok(th) => th,
            Err(e) => {
                eprintln!("{name}: we refused: {e}");
                t.other_mismatches += 1;
                continue;
            }
        };
        t.files += 1;
        if (th.width, th.height) != c.size {
            eprintln!("{name}: size {}x{} vs {:?}", th.width, th.height, c.size);
            t.other_mismatches += 1;
            continue;
        }
        let k = &th.conditions;
        if let Some(o) = c.cond {
            let ours = [
                k.emissivity,
                k.distance,
                k.reflected,
                k.atmospheric,
                k.window_temperature,
                k.window_transmission,
                k.humidity,
            ];
            for (i, (a, b)) in ours.iter().zip(o).enumerate() {
                if a.is_none_or(|a| (a - b).abs() > 1e-6) {
                    eprintln!("{name}: condition {i}: {a:?} vs {b}");
                    t.other_mismatches += 1;
                }
            }
        }
        let temps = th.temperatures(&th.conditions);
        let counts = match &th.pixels {
            specular_fmt_thermal::Pixels::Counts { values, .. } => Some(values),
            specular_fmt_thermal::Pixels::Celsius(_) => None,
        };
        for &(i, raw, celsius) in &c.px {
            if let Some(r) = raw {
                t.raw_values += 1;
                if counts.map(|v| v[i]) != Some(r) {
                    if t.raw_mismatches < 5 {
                        eprintln!("{name}: raw {i}: {:?} vs {r}", counts.map(|v| v[i]));
                    }
                    t.raw_mismatches += 1;
                }
            }
            t.temperatures += 1;
            let ours = f64::from(temps[i]);
            let both_nan = ours.is_nan() && celsius.is_nan();
            let d = (ours - celsius).abs();
            if !both_nan && d.is_finite() {
                t.max_abs_diff = t.max_abs_diff.max(d);
            }
            if !both_nan && (d.is_nan() || d > 0.01) {
                if t.temperature_mismatches < 5 {
                    eprintln!("{name}: temperature {i}: {ours} vs {celsius}");
                }
                t.temperature_mismatches += 1;
            }
        }
    }
    t
}

#[test]
fn fixtures_match_the_oracles() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data");
    let cases = load(&dir.join("expected.tsv"));
    let t = check(&dir, &cases);
    eprintln!("fixtures: {t:?}");
    assert_eq!(t.files, 13);
    assert_eq!(t.oracle_refused, 0);
    assert_eq!(
        t.raw_mismatches + t.temperature_mismatches + t.other_mismatches,
        0,
        "{t:?}"
    );
}

#[test]
fn corpus_matches_the_oracles() {
    let (Ok(tsv), Ok(dir)) = (
        std::env::var("SPECULAR_THERMAL_ORACLE"),
        std::env::var("SPECULAR_THERMAL_DIR"),
    ) else {
        return;
    };
    let t = check(Path::new(&dir), &load(Path::new(&tsv)));
    eprintln!("corpus: {t:?}");
    assert_eq!(
        t.raw_mismatches + t.temperature_mismatches + t.other_mismatches,
        0,
        "{t:?}"
    );
}

#[test]
fn emissivity_changes_flir_temperatures_only() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data");
    let flir = read(&std::fs::read(dir.join("fff_raw_le.fff")).unwrap()).unwrap();
    assert!(flir.adjustable());
    let hot = flir.temperatures(&Conditions {
        emissivity: Some(0.5),
        ..flir.conditions
    });
    let base = flir.temperatures(&flir.conditions);
    let i = base
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .unwrap()
        .0;
    assert!(hot[i] > base[i] + 10.0, "{} {}", hot[i], base[i]);
    let irg = read(&std::fs::read(dir.join("irg_c201.irg")).unwrap()).unwrap();
    assert!(!irg.adjustable());
    assert_eq!(
        irg.temperatures(&Conditions {
            emissivity: Some(0.5),
            ..irg.conditions
        }),
        irg.temperatures(&irg.conditions)
    );
}
