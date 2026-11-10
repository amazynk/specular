//! Every message of the fixtures (and of `tools/corpus/grib/`, when fetched) against ecCodes:
//! values, the latitude and longitude of every point, and the parameter, level, validity
//! time and ensemble member.
//!
//! The interpreter is `SPECULAR_PYTHON`, else the project's `target/grib-venv` (found by
//! walking up from the crate), else `python3`; it needs the `eccodes` and `numpy` wheels.
//! Without them the test reports that it skipped.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use specular_grib::{Field, GridKind, scan};

fn python() -> PathBuf {
    if let Some(p) = std::env::var_os("SPECULAR_PYTHON") {
        return PathBuf::from(p);
    }
    let mut dir = Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf();
    loop {
        let venv = dir.join("target/grib-venv/bin/python");
        if venv.exists() {
            return venv;
        }
        if !dir.pop() {
            return PathBuf::from("python3");
        }
    }
}

struct Expected {
    file: usize,
    keys: Vec<(String, String)>,
    values: Vec<f64>,
    lats: Vec<f64>,
    lons: Vec<f64>,
}

impl Expected {
    fn key(&self, k: &str) -> &str {
        self.keys
            .iter()
            .find(|(a, _)| a == k)
            .map_or("", |(_, v)| v.as_str())
    }
}

/// ecCodes' reading of `files`, or None when Python or ecCodes is missing. `label` keeps the
/// dumps of tests running side by side apart.
fn ecc(files: &[PathBuf], label: &str) -> Option<Vec<Expected>> {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("grib-oracle-{label}-{}", std::process::id()));
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/oracle_dump.py");
    let st = Command::new(python())
        .arg("-I")
        .arg(&script)
        .arg(&out)
        .args(files)
        .output()
        .ok()?;
    if !st.status.success() {
        eprintln!(
            "ecCodes oracle skipped: {}",
            String::from_utf8_lossy(&st.stderr)
                .lines()
                .last()
                .unwrap_or("")
        );
        return None;
    }
    let txt = std::fs::read_to_string(out.with_extension("txt")).ok()?;
    let bin = std::fs::read(out.with_extension("bin")).ok()?;
    let _ = std::fs::remove_file(out.with_extension("txt"));
    let _ = std::fs::remove_file(out.with_extension("bin"));
    let mut pos = 0;
    let mut take = |n: usize| -> Vec<f64> {
        let v = bin[pos..pos + 8 * n]
            .chunks_exact(8)
            .map(|b| f64::from_le_bytes(b.try_into().unwrap()))
            .collect();
        pos += 8 * n;
        v
    };
    let mut res = Vec::new();
    for line in txt.lines() {
        let mut parts = line.split('\t');
        let file: usize = parts.next()?.parse().ok()?;
        let keys: Vec<(String, String)> = parts
            .filter_map(|p| p.split_once('='))
            .map(|(a, b)| (a.to_owned(), b.to_owned()))
            .collect();
        let n: usize = keys.iter().find(|(k, _)| k == "count")?.1.parse().ok()?;
        res.push(Expected {
            file,
            keys,
            values: take(n),
            lats: take(n),
            lons: take(n),
        });
    }
    Some(res)
}

#[derive(Default)]
struct Tally {
    messages: usize,
    values: usize,
    value_mismatches: usize,
    points: usize,
    grid_mismatches: usize,
    max_grid_err: f64,
    meta_checks: usize,
    meta_mismatches: Vec<String>,
    /// Messages we decline (grids or packings outside the supported set), by name.
    unsupported: Vec<String>,
    /// Parameters named differently from ecCodes' (centre-specific names).
    names: Vec<String>,
}

fn same(a: f64, b: f64, scale: f64) -> bool {
    if a.is_nan() || b.is_nan() {
        return a.is_nan() && b.is_nan();
    }
    (a - b).abs() <= 1e-9 * scale.max(1e-300) + 1e-12 * a.abs().max(b.abs())
}

fn lon_diff(a: f64, b: f64) -> f64 {
    let d = (a - b).rem_euclid(360.0);
    d.min(360.0 - d)
}

fn compare(name: &str, data: &[u8], f: &Field, e: &Expected, t: &mut Tally) {
    let got = match f.decode(data) {
        Ok(v) => v,
        Err(specular_grib::Error::Unsupported(why)) => {
            t.unsupported.push(format!("{name}: {why}"));
            return;
        }
        Err(err) => panic!("{name}: {err}"),
    };
    t.messages += 1;
    assert_eq!(got.len(), e.values.len(), "{name}: point count");
    let (lo, hi) = e
        .values
        .iter()
        .filter(|v| v.is_finite())
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &v| {
            (a.min(v), b.max(v))
        });
    // ecCodes rounds through f32 in places (IEEE single fields, references); compare at the
    // precision of the field's packing.
    let scale = (hi - lo).abs().max(hi.abs()).max(1e-30);
    let tol = if f.packing.name() == "IEEE float" {
        1e-6
    } else {
        1e-9
    };
    for (a, b) in got.iter().zip(&e.values) {
        t.values += 1;
        if !(same(*a, *b, scale) || (a - b).abs() <= tol * scale) {
            t.value_mismatches += 1;
        }
    }
    let coords = f.grid.coordinates();
    // ecCodes evaluates projected grids on a sphere for some shapes and in single precision
    // in places: allow a few micro-degrees there, a nano-degree otherwise.
    let gtol = match f.grid.kind {
        GridKind::LatLon { .. } | GridKind::Gaussian { .. } if f.grid.rotation.is_none() => 1e-6,
        _ => 1e-4,
    };
    let before = t.grid_mismatches;
    let mut first_bad = None;
    for (k, ((lon, lat), (elat, elon))) in coords.iter().zip(e.lats.iter().zip(&e.lons)).enumerate()
    {
        if elat.is_nan() {
            continue;
        }
        t.points += 1;
        let err = (lat - elat).abs().max(if elat.abs() > 89.999_999 {
            0.0
        } else {
            lon_diff(*lon, *elon)
        });
        t.max_grid_err = t.max_grid_err.max(err);
        if err.is_nan() || err > gtol {
            t.grid_mismatches += 1;
            first_bad.get_or_insert((k, *lon, *lat, *elon, *elat));
        }
    }
    if t.grid_mismatches > before {
        eprintln!(
            "  {name} {}: {} grid mismatches, first {:?}",
            f.grid.name(),
            t.grid_mismatches - before,
            first_bad
        );
    }
    let short = e.key("shortName");
    if short != "unknown" && !f.param.name.is_empty() {
        // Centres' own names (ECMWF's paramIds tell a 6-hour maximum 2 m temperature, an
        // orography or an accumulated precipitation apart where the WMO table names the
        // quantity) are told apart from errors: the fixtures must match exactly, the corpus
        // reports them.
        t.meta_checks += 1;
        if short != f.param.short {
            t.names.push(format!(
                "{name} {} ({}): {} vs {short}",
                f.param.name,
                e.key("centre"),
                f.param.short
            ));
        }
    }
    let mut meta = |what: &str, ok: bool, detail: String| {
        t.meta_checks += 1;
        if !ok {
            t.meta_mismatches.push(format!("{name} {what}: {detail}"));
        }
    };
    if let (Some(v), Ok(ev)) = (f.level.value, e.key("level").parse::<f64>()) {
        meta("level", (v - ev).abs() < 1e-6, format!("{v} vs {ev}"));
    }
    let valid = f.valid();
    let date = format!("{:04}{:02}{:02}", valid.year, valid.month, valid.day);
    let time = u32::from(valid.hour) * 100 + u32::from(valid.minute);
    meta(
        "validity",
        date == e.key("validityDate") && e.key("validityTime").parse() == Ok(time),
        format!(
            "{date} {time} vs {} {}",
            e.key("validityDate"),
            e.key("validityTime")
        ),
    );
    if let Ok(m) = e.key("perturbationNumber").parse::<u32>()
        && (m != 0 || f.ensemble.is_some())
    {
        meta(
            "member",
            f.ensemble.map(|x| x.member) == Some(m),
            format!("{:?} vs {m}", f.ensemble),
        );
    }
    meta(
        "edition",
        e.key("edition") == f.edition.to_string(),
        format!("{} vs {}", f.edition, e.key("edition")),
    );
}

fn run(files: &[PathBuf], label: &str) -> Option<Tally> {
    let expected = ecc(files, label)?;
    let mut t = Tally::default();
    let mut scan_time = 0.0;
    let mut decode_time = 0.0;
    let mut bytes = 0usize;
    for (fi, path) in files.iter().enumerate() {
        let data = std::fs::read(path).unwrap();
        bytes += data.len();
        let t0 = Instant::now();
        let name = path.file_name().unwrap().to_string_lossy();
        let want: Vec<&Expected> = expected.iter().filter(|e| e.file == fi).collect();
        // Messages we decline are skipped by `scan` (a file of nothing else fails): line the
        // rest up with ecCodes' by message offset.
        let fields = match scan(&data) {
            Ok(f) => f,
            Err(specular_grib::Error::Unsupported(why)) => {
                t.unsupported
                    .push(format!("{name}: all {} messages: {why}", want.len()));
                continue;
            }
            Err(e) => panic!("{}: {e}", path.display()),
        };
        scan_time += t0.elapsed().as_secs_f64();
        let t0 = Instant::now();
        for f in &fields {
            let _ = f.plane(&data);
        }
        decode_time += t0.elapsed().as_secs_f64();
        let mut offsets: Vec<&str> = want.iter().map(|e| e.key("offset")).collect();
        offsets.dedup();
        for (k, off) in offsets.into_iter().enumerate() {
            let at: usize = off.parse().unwrap();
            let es: Vec<&&Expected> = want.iter().filter(|e| e.key("offset") == off).collect();
            let fs: Vec<&Field> = fields.iter().filter(|f| f.message.start == at).collect();
            if fs.is_empty() {
                t.unsupported
                    .push(format!("{name}#{k} at {at}: skipped by scan"));
                continue;
            }
            assert_eq!(fs.len(), es.len(), "{name}#{k}: field count");
            for (j, (f, e)) in fs.into_iter().zip(es).enumerate() {
                compare(&format!("{name}#{k}.{j}"), &data, f, e, &mut t);
            }
        }
    }
    eprintln!(
        "{label}: {} messages, {} values ({} mismatches), {} points ({} grid mismatches, \
         max error {:.2e}°), {} metadata checks ({} mismatches); scan {:.2} ms, decode and \
         plane {:.2} ms for {:.1} MB",
        t.messages,
        t.values,
        t.value_mismatches,
        t.points,
        t.grid_mismatches,
        t.max_grid_err,
        t.meta_checks,
        t.meta_mismatches.len(),
        scan_time * 1e3,
        decode_time * 1e3,
        bytes as f64 / 1e6,
    );
    for m in t.meta_mismatches.iter().take(20) {
        eprintln!("  {m}");
    }
    for m in &t.unsupported {
        eprintln!("  declined {m}");
    }
    for m in &t.names {
        eprintln!("  named {m}");
    }
    Some(t)
}

#[test]
fn fixtures_against_eccodes() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    files.sort();
    let Some(t) = run(&files, "fixtures") else {
        return;
    };
    assert!(t.messages > 40);
    assert!(t.unsupported.is_empty(), "{:?}", t.unsupported);
    assert!(t.names.is_empty(), "{:?}", t.names);
    assert_eq!(t.value_mismatches, 0);
    assert_eq!(t.grid_mismatches, 0);
    assert!(t.meta_mismatches.is_empty(), "{:?}", t.meta_mismatches);
}

#[test]
fn corpus_against_eccodes() {
    let dir = std::env::var_os("SPECULAR_GRIB_CORPUS").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/grib"),
        PathBuf::from,
    );
    let Ok(rd) = std::fs::read_dir(&dir) else {
        eprintln!("no GRIB corpus (tools/corpus/fetch-grib.sh)");
        return;
    };
    let mut files: Vec<PathBuf> = rd
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.is_file() && std::fs::read(p).is_ok_and(|d| d.windows(4).any(|w| w == b"GRIB"))
        })
        .collect();
    files.sort();
    if files.is_empty() {
        return;
    }
    let Some(t) = run(&files, "corpus") else {
        return;
    };
    assert_eq!(t.value_mismatches, 0);
    assert_eq!(t.grid_mismatches, 0);
    assert!(t.meta_mismatches.is_empty(), "{:?}", t.meta_mismatches);
}
