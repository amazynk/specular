//! Testo `.bmt` and Fluke `.is2`.
//!
//! The synthetic fixtures (gen/make_bmt_is2_fixtures.py) are checked pixel by pixel against
//! the temperatures that script computes on its own (tests/data/bmt_is2_expected.tsv),
//! within 1e-4 °C.
//!
//! With `SPECULAR_THERMAL_HANDHELD_DIR` naming the folder tools/corpus/fetch-thermal-handheld.sh
//! fills, public files are checked too:
//! * each ZIP-layout `.is2` against the temperature table Fluke SmartView exported beside
//!   it (`.txt`, °F to one decimal): every pixel within 0.07 °F (the export's rounding of
//!   0.05 °F plus 0.02 °F);
//! * the older `.is2` files are recognised as such;
//! * each `.bmt` against `expected.tsv` (gen/bmt_oracle.py, an independent reading of
//!   Testo's published layout), the camera's own recorded minimum and maximum, and the
//!   hot and cold spot positions it recorded.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use specular_fmt_thermal::{
    Conditions, Error, Kind, Mapping, NO_IR_DATA_REASON, Pixels, Thermal, is_kind, is_old_bmt,
    is_old_is2, read, sniff,
};

fn data(name: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/data")
            .join(name),
    )
    .unwrap()
}

#[derive(Default)]
struct Case {
    size: (usize, usize),
    cond: Vec<f64>,
    px: Vec<(usize, u16, f64)>,
    px2: Vec<(usize, u16, f64)>,
}

fn expected() -> BTreeMap<String, Case> {
    let text = String::from_utf8(data("bmt_is2_expected.tsv")).unwrap();
    let mut out: BTreeMap<String, Case> = BTreeMap::new();
    for l in text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
    {
        let f: Vec<&str> = l.split('\t').collect();
        let c = out.entry(f[1].to_owned()).or_default();
        let n = |i: usize| f[i].parse::<f64>().unwrap();
        match f[2] {
            "size" => c.size = (f[3].parse().unwrap(), f[4].parse().unwrap()),
            "cond" => c.cond = (3..f.len()).map(n).collect(),
            "px" => {
                c.px.push((f[3].parse().unwrap(), f[4].parse().unwrap(), n(5)))
            }
            "px2" => c
                .px2
                .push((f[3].parse().unwrap(), f[4].parse().unwrap(), n(5))),
            t => panic!("unknown row {t}"),
        }
    }
    out
}

fn counts(t: &Thermal) -> &[u16] {
    match &t.pixels {
        Pixels::Counts { values, .. } => values,
        Pixels::Celsius(_) => panic!("counts expected"),
    }
}

#[test]
fn fixtures_match_the_independent_readings() {
    let cases = expected();
    assert_eq!(cases.len(), 4);
    for (name, c) in &cases {
        let t = read(&data(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!((t.width, t.height), c.size, "{name}");
        let temps = t.temperatures(&t.conditions);
        let mut worst = 0f64;
        for &(i, raw, want) in &c.px {
            assert_eq!(counts(&t)[i], raw, "{name} pixel {i}");
            worst = worst.max((f64::from(temps[i]) - want).abs());
        }
        assert!(worst < 1e-4, "{name}: worst {worst}");
        if name.ends_with(".is2") {
            assert_eq!(t.kind, Kind::FlukeIs2);
            assert!(t.adjustable());
            assert!((t.conditions.emissivity.unwrap() - c.cond[0]).abs() < 1e-9);
            assert_eq!(t.conditions.reflected, Some(c.cond[1]));
            assert_eq!(t.conditions.window_transmission, Some(c.cond[2]));
            let other = Conditions {
                emissivity: Some(0.7),
                reflected: Some(35.0),
                ..t.conditions
            };
            let temps = t.temperatures(&other);
            let worst = c
                .px2
                .iter()
                .map(|&(i, _, want)| (f64::from(temps[i]) - want).abs())
                .fold(0.0, f64::max);
            assert!(worst < 1e-4, "{name} recomputed: worst {worst}");
            assert_eq!(t.camera.range, Some((-10.0, 80.0)));
            let v = t.visible.as_ref().unwrap();
            assert_eq!((v.width, v.height), (64, 48), "the largest photo");
            assert_eq!(t.camera.serial, "G13080059");
        } else {
            assert_eq!(t.kind, Kind::TestoBmt);
            assert!(!t.adjustable());
            assert!(matches!(
                t.pixels,
                Pixels::Counts {
                    mapping: Mapping::Linear { .. },
                    ..
                }
            ));
            let o = &t.conditions;
            assert!((o.emissivity.unwrap() - c.cond[0]).abs() < 1e-6);
            assert!((o.reflected.unwrap() - c.cond[1]).abs() < 1e-4);
            assert!((o.atmospheric.unwrap() - c.cond[2]).abs() < 1e-4);
            assert!((o.humidity.unwrap() - c.cond[3]).abs() < 1e-6);
            assert_eq!(t.camera.model, "testo 872");
            assert_eq!(t.camera.serial, "61823573");
            assert_eq!(t.camera.software, "firmware 1.9.0");
            assert_eq!(t.camera.date.as_deref(), Some("2018-01-06 13:51:32"));
            let r = t.camera.range.unwrap();
            assert!((r.0 + 30.0).abs() < 1e-4 && (r.1 - 100.0).abs() < 1e-4);
            let p = t.palette.as_ref().unwrap();
            assert_eq!(p.name, "Ironbow");
            assert_eq!(p.colours.len(), 4);
            let v = t.visible.as_ref().unwrap();
            assert_eq!((v.width, v.height), (16, 12));
        }
    }
    let json = read(&data("is2_json.is2")).unwrap();
    assert_eq!(json.camera.model, "Fluke Ti400");
    assert_eq!(json.camera.date.as_deref(), Some("2013-09-04 00:00:48"));
    assert_eq!(
        read(&data("is2_bare.is2")).unwrap().camera.model,
        "Fluke Thermography"
    );
}

#[test]
fn sniffing() {
    // A Fluke IS2 by its directory; a Testo BMT with its preview only when asked by kind.
    assert_eq!(sniff(&data("is2_json.is2")), Some(Kind::FlukeIs2));
    assert_eq!(sniff(&data("bmt_872.bmt")), None);
    assert!(is_kind(&data("bmt_872.bmt"), Kind::TestoBmt));
    assert_eq!(sniff(&data("bmt_bare.bmt")), Some(Kind::TestoBmt));
    // Neither claims the other's files, a FLIR R-JPEG or the refused layouts.
    for name in ["rjpeg.jpg", "flir_noraw.jpg", "is2_old.is2", "bmt_old.bmt"] {
        let d = data(name);
        assert!(!is_kind(&d, Kind::TestoBmt), "{name}");
        assert!(!is_kind(&d, Kind::FlukeIs2), "{name}");
    }
    assert!(!is_kind(&data("is2_json.is2"), Kind::TestoBmt));
    assert!(is_old_is2(&data("is2_old.is2")));
    assert!(!is_old_is2(&data("is2_json.is2")));
    assert!(is_old_bmt(&data("bmt_old.bmt")));
    assert!(!is_old_bmt(&data("bmt_872.bmt")));
}

#[test]
fn refusals_say_why() {
    match read(&data("is2_noir.is2")) {
        Err(Error::Unsupported { what, .. }) => assert_eq!(what, NO_IR_DATA_REASON),
        r => panic!("{r:?}"),
    }
    match read(&data("is2_ambiguous.is2")) {
        Err(Error::Unsupported { what, .. }) => assert!(what.contains("calibration")),
        r => panic!("{r:?}"),
    }
    assert_eq!(read(&data("is2_old.is2")), Err(Error::NotThermal));
    assert_eq!(read(&data("bmt_old.bmt")), Err(Error::NotThermal));
    // Cut short anywhere: an error, never a panic.
    for name in ["bmt_872.bmt", "is2_json.is2"] {
        let d = data(name);
        for n in (0..d.len()).step_by(7) {
            let _ = read(&d[..n]);
        }
    }
}

/// UTF-16 SmartView export: a title, the file name, a header row of column numbers, then
/// one row per image row (its number, then °F with a decimal comma or point).
fn smartview(path: &Path) -> Vec<Vec<f64>> {
    let b = std::fs::read(path).unwrap();
    let units: Vec<u16> = b[2..]
        .chunks_exact(2)
        .map(|p| u16::from_le_bytes([p[0], p[1]]))
        .collect();
    String::from_utf16_lossy(&units)
        .lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            let first = f.first()?.trim();
            (f.len() > 2 && !first.is_empty() && first.parse::<usize>().is_ok()).then(|| {
                f[1..]
                    .iter()
                    .filter(|v| !v.trim().is_empty())
                    .map(|v| v.trim().replace(',', ".").parse().unwrap())
                    .collect()
            })
        })
        .collect()
}

fn corpus() -> Option<PathBuf> {
    std::env::var_os("SPECULAR_THERMAL_HANDHELD_DIR").map(PathBuf::from)
}

#[test]
fn corpus_matches_the_oracles() {
    let Some(dir) = corpus() else {
        eprintln!("SPECULAR_THERMAL_HANDHELD_DIR not set: corpus skipped");
        return;
    };
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    files.sort();
    let (mut is2, mut old, mut px, mut bad) = (0, 0, 0usize, 0usize);
    let mut worst = 0f64;
    for p in files
        .iter()
        .filter(|p| p.extension().is_some_and(|e| e == "is2"))
    {
        let d = std::fs::read(p).unwrap();
        if is_old_is2(&d) {
            assert_eq!(sniff(&d), None, "{}", p.display());
            old += 1;
            continue;
        }
        let t = read(&d).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        let table = smartview(&p.with_extension("txt"));
        assert_eq!(table.len(), t.height, "{}", p.display());
        let temps = t.temperatures(&t.conditions);
        let mut file_worst = 0f64;
        for (y, row) in table.iter().enumerate() {
            assert_eq!(row.len(), t.width);
            for (x, &want) in row.iter().enumerate() {
                let f = f64::from(temps[y * t.width + x]) * 9.0 / 5.0 + 32.0;
                let d = (f - want).abs();
                file_worst = file_worst.max(d);
                bad += usize::from(d > 0.07);
                px += 1;
            }
        }
        eprintln!(
            "{}: {}×{}, worst {file_worst:.4} °F",
            p.display(),
            t.width,
            t.height
        );
        worst = worst.max(file_worst);
        is2 += 1;
    }
    eprintln!("IS2: {is2} files, {px} pixels, {bad} over 0.07 °F (worst {worst:.4}); {old} older");
    assert_eq!(bad, 0);
    assert!(is2 > 0);

    let oracle = std::fs::read_to_string(dir.join("expected.tsv")).unwrap();
    let (mut bmt, mut bpx, mut bbad) = (0, 0usize, 0usize);
    let mut bworst = 0f64;
    let mut current: Option<(String, Thermal, Vec<f32>)> = None;
    for l in oracle
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
    {
        let f: Vec<&str> = l.split('\t').collect();
        if current.as_ref().is_none_or(|c| c.0 != f[0]) {
            let d = std::fs::read(dir.join(f[0])).unwrap();
            assert!(is_kind(&d, Kind::TestoBmt), "{}", f[0]);
            let t = read(&d).unwrap();
            let temps = t.temperatures(&t.conditions);
            current = Some((f[0].to_owned(), t, temps));
            bmt += 1;
        }
        let (_, t, temps) = current.as_ref().unwrap();
        let n = |i: usize| f[i].parse::<f64>().unwrap();
        match f[1] {
            "size" => assert_eq!((t.width, t.height), (n(2) as usize, n(3) as usize)),
            // The camera's recorded scale minimum and maximum: the image's extremes.
            "minmax" => {
                let (lo, hi) = temps
                    .iter()
                    .fold((f32::MAX, f32::MIN), |(a, b), &v| (a.min(v), b.max(v)));
                assert!((f64::from(lo) - n(2)).abs() < 1e-3, "{} min", f[0]);
                assert!((f64::from(hi) - n(3)).abs() < 1e-3, "{} max", f[0]);
            }
            // The hot and cold spots the camera marked hold the extremes.
            "spot" => {
                let v = counts(t)[n(3) as usize * t.width + n(2) as usize];
                let (lo, hi) = t.raw_range().unwrap();
                assert_eq!(v, if f[4] == "hot" { hi } else { lo }, "{} {}", f[0], f[4]);
            }
            "px" => {
                let i = n(2) as usize;
                assert_eq!(f64::from(counts(t)[i]), n(3));
                let d = (f64::from(temps[i]) - n(4)).abs();
                bworst = bworst.max(d);
                bbad += usize::from(d > 1e-4);
                bpx += 1;
            }
            r => panic!("unknown row {r}"),
        }
    }
    eprintln!("BMT: {bmt} files, {bpx} pixels, {bbad} over 1e-4 °C (worst {bworst:.2e})");
    assert_eq!(bbad, 0);
}

#[test]
fn corpus_timings() {
    let Some(dir) = corpus() else {
        return;
    };
    for name in ["Compressor.is2", "Haus2.BMT"] {
        let Ok(d) = std::fs::read(dir.join(name)) else {
            continue;
        };
        let t0 = std::time::Instant::now();
        let t = read(&d).unwrap();
        let open = t0.elapsed();
        let t1 = std::time::Instant::now();
        let temps = t.temperatures(&t.conditions);
        eprintln!(
            "{name}: read {:.2} ms, {} temperatures {:.2} ms",
            open.as_secs_f64() * 1e3,
            temps.len(),
            t1.elapsed().as_secs_f64() * 1e3
        );
    }
}
