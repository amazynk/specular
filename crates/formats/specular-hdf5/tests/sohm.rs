//! Shared object-header messages kept in the SOHM table against h5py: the HDF5 test file
//! `h5stat_tsohm.h5` (checked in, `tests/data`) and the files `tools/corpus/hdf5-sohm.py`
//! writes (list and B-tree indexes, one index for all message kinds or one per kind; in
//! `$SPECULAR_HDF5_SOHM`, else `$CARGO_TARGET_DIR/corpus/hdf5-sohm`), each with `NAME.json`:
//! every dataset's values and every attribute as h5py reads them.

use std::path::{Path, PathBuf};

use specular_hdf5::{File, Value};

/// Compares one file with its h5py dump: (values compared, mismatches).
fn compare(h5: &Path) -> (usize, usize) {
    let data = std::fs::read(h5).unwrap();
    let json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(h5.with_extension("json")).unwrap()).unwrap();
    let f = File::open(&data).unwrap_or_else(|e| panic!("{}: {e}", h5.display()));
    assert!(f.warnings.is_empty(), "{}: {:?}", h5.display(), f.warnings);
    let (mut n, mut bad) = (0usize, 0usize);
    for (path, want) in json.as_object().unwrap() {
        let abs = if path.starts_with('/') {
            path.clone()
        } else {
            format!("/{path}")
        };
        let obj = f
            .get(&abs)
            .unwrap_or_else(|| panic!("{}: no {abs}", h5.display()));
        if let Some(values) = want.get("data") {
            let idx = f.index_of(&abs).unwrap();
            let got: Vec<f64> = f.read_all(&data, idx, 1 << 24).unwrap();
            let values: Vec<f64> = values
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_f64().unwrap())
                .collect();
            assert_eq!(got.len(), values.len(), "{abs}");
            n += values.len();
            bad += got.iter().zip(&values).filter(|(a, b)| a != b).count();
        }
        for (name, v) in want["attrs"].as_object().unwrap() {
            n += 1;
            let ok = match (obj.attr(name), v) {
                (Some(Value::Str(s)), serde_json::Value::String(w)) => s.len() == 1 && &s[0] == w,
                (Some(a), w) => {
                    let w: Vec<f64> = match w {
                        serde_json::Value::Array(a) => {
                            a.iter().filter_map(|x| x.as_f64()).collect()
                        }
                        x => x.as_f64().into_iter().collect(),
                    };
                    a.as_f64s().is_some_and(|g| g == w)
                }
                (None, _) => false,
            };
            if !ok {
                eprintln!("{abs} @{name}: {:?} vs {v}", obj.attr(name));
                bad += 1;
            }
        }
    }
    (n, bad)
}

#[test]
fn h5stat_tsohm_matches_h5py() {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/h5stat_tsohm.h5");
    let (n, bad) = compare(&p);
    assert!(n > 0);
    assert_eq!(bad, 0, "{bad} of {n} values differ");
}

fn corpus() -> Option<PathBuf> {
    let dir = std::env::var_os("SPECULAR_HDF5_SOHM")
        .map(PathBuf::from)
        .or_else(|| {
            let t = std::env::var_os("CARGO_TARGET_DIR")?;
            Some(Path::new(&t).join("corpus/hdf5-sohm"))
        })?;
    dir.is_dir().then_some(dir)
}

#[test]
fn generated_files_match_h5py() {
    let Some(dir) = corpus() else {
        eprintln!("skipped: no corpus (tools/corpus/hdf5-sohm.py)");
        return;
    };
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "h5") && p.with_extension("json").exists())
        .collect();
    files.sort();
    let mut fails = Vec::new();
    for p in &files {
        let (n, bad) = compare(p);
        eprintln!("{}: {bad} of {n} values differ from h5py", p.display());
        if bad != 0 {
            fails.push(p.clone());
        }
    }
    assert!(fails.is_empty(), "{fails:?}");
}
