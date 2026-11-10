//! Fixture generation through Python (h5py, hdf5plugin, netCDF4) as the oracle.
//!
//! The interpreter is `SPECULAR_PYTHON`, else the project's `target/hdf5-venv` (found by
//! walking up from the crate), else `python3`. Without h5py the oracle tests are skipped.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

fn python() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("SPECULAR_PYTHON") {
        return Some(PathBuf::from(p));
    }
    let mut dir = Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf();
    loop {
        let venv = dir.join("target/hdf5-venv/bin/python");
        if venv.exists() {
            return Some(venv);
        }
        if !dir.pop() {
            break;
        }
    }
    Some(PathBuf::from("python3"))
}

/// The fixture directory, generated once per test binary; `None` when no oracle is available.
pub fn fixtures() -> Option<&'static Path> {
    static DIR: OnceLock<Option<PathBuf>> = OnceLock::new();
    DIR.get_or_init(|| {
        let out = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(concat!("fixtures-", env!("CARGO_PKG_NAME")));
        // Shared by the crates of the workspace that test against these fixtures.
        let script = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .map(|d| d.join("crates/formats/specular-hdf5/gen/make_fixtures.py"))
            .find(|p| p.exists())?;
        let stamp = out.join("stamp");
        let fresh = match (std::fs::metadata(&stamp), std::fs::metadata(&script)) {
            (Ok(s), Ok(g)) => s.modified().ok() >= g.modified().ok(),
            _ => false,
        };
        if fresh {
            return Some(out);
        }
        let py = python()?;
        let tmp = Path::new(env!("CARGO_TARGET_TMPDIR")).join("py-tmp");
        let _ = std::fs::create_dir_all(&tmp);
        let r = Command::new(&py)
            .arg(&script)
            .arg(&out)
            .env("TMPDIR", &tmp)
            .env("PYTHONDONTWRITEBYTECODE", "1")
            .output();
        match r {
            Ok(o) if o.status.success() => {
                std::fs::write(&stamp, b"ok").ok()?;
                Some(out)
            }
            Ok(o) => {
                eprintln!(
                    "fixture generation failed, oracle tests skipped:\n{}",
                    String::from_utf8_lossy(&o.stderr)
                );
                None
            }
            Err(e) => {
                eprintln!("no python ({e}), oracle tests skipped");
                None
            }
        }
    })
    .as_deref()
}

/// Expected values of a fixture's datasets: (path, shape, values).
pub fn expected(dir: &Path, name: &str) -> Vec<(String, Vec<u64>, Vec<f64>)> {
    let j: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.join(format!("{name}.json"))).unwrap()).unwrap();
    let mut out = Vec::new();
    for (path, d) in j["datasets"].as_object().unwrap() {
        let shape: Vec<u64> = d["shape"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap())
            .collect();
        let raw = std::fs::read(dir.join(d["bin"].as_str().unwrap())).unwrap();
        let vals = raw
            .chunks_exact(8)
            .map(|c| f64::from_le_bytes(c.try_into().unwrap()))
            .collect();
        out.push((path.clone(), shape, vals));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// Little-endian f64 values of a file.
pub fn read_f64(path: &Path) -> Vec<f64> {
    std::fs::read(path)
        .unwrap()
        .chunks_exact(8)
        .map(|c| f64::from_le_bytes(c.try_into().unwrap()))
        .collect()
}

/// Expected attributes of a fixture: path → name → JSON value.
pub fn expected_attrs(dir: &Path, name: &str) -> serde_json::Value {
    let j: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.join(format!("{name}.json"))).unwrap()).unwrap();
    j["attrs"].clone()
}

/// Mismatches between two value lists, NaN equal to NaN, compared as `f32` when `as_f32`.
pub fn mismatches(got: &[f64], want: &[f64]) -> usize {
    got.iter()
        .zip(want)
        .filter(|(g, w)| !(g.to_bits() == w.to_bits() || (g.is_nan() && w.is_nan())))
        .count()
        + got.len().abs_diff(want.len())
}
