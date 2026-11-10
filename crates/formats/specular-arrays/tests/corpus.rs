//! Arrays read against the values numpy, scipy.io and zarr-python read from the same files
//! (`gen/make_samples.py`): the committed set in `tests/data`, and a larger generated corpus
//! when `SPECULAR_ARRAYS_CORPUS` names its folder.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::Value;
use specular_arrays::mat::{self, Kind};
use specular_arrays::zarr::{self, DirStore, Store, ZipStore};
use specular_arrays::{Error, Part, PlaneSpec, npy, npz};

#[derive(Default, Debug)]
struct Tally {
    files: usize,
    arrays: usize,
    values: u64,
    mismatches: u64,
    refused: usize,
    texts: usize,
    broken: usize,
    failures: Vec<String>,
}

/// Every plane of an array of `shape`, compared with `want` (C order of the logical shape).
fn compare(
    t: &mut Tally,
    what: &str,
    shape: &[u64],
    want: &[f32],
    complex: bool,
    read: &mut dyn FnMut(&PlaneSpec) -> Result<Vec<f32>, Error>,
) {
    let nd = shape.len();
    let (xd, yd) = match nd {
        0 | 1 => (0, None),
        _ => (nd - 1, Some(nd - 2)),
    };
    let c: Vec<u64> = specular_arrays::strides(shape, false);
    let others: Vec<usize> = (0..nd).filter(|&d| d != xd && Some(d) != yd).collect();
    let planes: u64 = others.iter().map(|&d| shape[d]).product();
    let mut bad = 0u64;
    for p in 0..planes {
        let mut index = vec![0u64; nd];
        let mut r = p;
        for &d in others.iter().rev() {
            index[d] = r % shape[d];
            r /= shape[d];
        }
        let spec = PlaneSpec {
            x_dim: xd,
            y_dim: yd,
            index: index.clone(),
        };
        let got = match read(&spec) {
            Ok(v) => v,
            Err(e) => {
                t.failures.push(format!("{what}: plane {p}: {e}"));
                return;
            }
        };
        let w = shape.get(xd).copied().unwrap_or(1);
        for (k, g) in got.iter().enumerate() {
            let (y, x) = (k as u64 / w.max(1), k as u64 % w.max(1));
            let mut flat = 0u64;
            for d in 0..nd {
                let i = if d == xd {
                    x
                } else if Some(d) == yd {
                    y
                } else {
                    index[d]
                };
                flat += i * c[d];
            }
            let e = want[flat as usize];
            let same = if complex {
                (g - e).abs() <= 1e-6 * e.abs().max(1e-30) || g.to_bits() == e.to_bits()
            } else {
                g.to_bits() == e.to_bits() || (g.is_nan() && e.is_nan())
            };
            if !same {
                if bad < 3 {
                    t.failures
                        .push(format!("{what}: value {flat}: got {g}, want {e}"));
                }
                bad += 1;
            }
        }
        t.values += got.len() as u64;
    }
    t.mismatches += bad;
    t.arrays += 1;
}

fn load_f32(p: &Path) -> Vec<f32> {
    std::fs::read(p)
        .map(|b| {
            b.chunks_exact(4)
                .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
                .collect()
        })
        .unwrap_or_default()
}

fn check_dir(dir: &Path) -> Tally {
    let mut t = Tally::default();
    let exp = dir.join("expected");
    let mut names: Vec<PathBuf> = std::fs::read_dir(&exp)
        .unwrap_or_else(|e| panic!("{}: {e}", exp.display()))
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    names.sort();
    for m in names {
        let man: Value = serde_json::from_slice(&std::fs::read(&m).unwrap()).unwrap();
        let name = m.file_stem().unwrap().to_str().unwrap().to_owned();
        let values = load_f32(&exp.join(format!("{name}.f32")));
        let path = dir.join(&name);
        t.files += 1;
        let kind = man["kind"].as_str().unwrap();
        let refused: Vec<&str> = man["refused"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        let arrays = man["arrays"].as_array().unwrap();
        let expect = |a: &Value| -> (String, Vec<u64>, Vec<f32>, Option<Vec<f32>>) {
            let shape: Vec<u64> = a["shape"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap())
                .collect();
            let n = shape.iter().product::<u64>() as usize;
            let off = a["offset"].as_u64().unwrap() as usize;
            let re = a
                .get("re_offset")
                .and_then(Value::as_u64)
                .map(|o| values[o as usize..o as usize + n].to_vec());
            (
                a["name"].as_str().unwrap().to_owned(),
                shape,
                values[off..off + n].to_vec(),
                re,
            )
        };
        match kind {
            "npy" => {
                let data = std::fs::read(&path).unwrap();
                match npy::parse(&data) {
                    Ok(h) => {
                        for a in arrays {
                            let (_, shape, want, re) = expect(a);
                            assert_eq!(h.shape, shape, "{name}");
                            for (part, want) in [(Part::Magnitude, Some(want)), (Part::Real, re)] {
                                let Some(want) = want else { continue };
                                compare(
                                    &mut t,
                                    &name,
                                    &shape,
                                    &want,
                                    part == Part::Magnitude && h.dtype.elem.is_complex(),
                                    &mut |s| {
                                        let mut out = Vec::new();
                                        h.read_plane(&data, s, part, &mut out).map(|_| out)
                                    },
                                );
                            }
                        }
                    }
                    Err(Error::Unsupported(_)) if !refused.is_empty() => t.refused += 1,
                    Err(e) => t.failures.push(format!("{name}: {e}")),
                }
            }
            "npz" => {
                let data = std::fs::read(&path).unwrap();
                let z = npz::parse(&data).unwrap();
                for r in &refused {
                    let m = z.members.iter().find(|m| m.name == *r).unwrap();
                    assert!(m.npy.is_err(), "{name}: {r}");
                    t.refused += 1;
                }
                for a in arrays {
                    let (an, shape, want, re) = expect(a);
                    let m = z.members.iter().find(|m| m.name == an).unwrap();
                    let h = m.npy.as_ref().unwrap();
                    let bytes = if m.compressed() {
                        z.inflate(&data, m, 1 << 30).unwrap()
                    } else {
                        let r = z.zip.data_range(&data, &m.entry).unwrap();
                        data[r].to_vec()
                    };
                    for (part, want) in [(Part::Magnitude, Some(want)), (Part::Real, re)] {
                        let Some(want) = want else { continue };
                        compare(
                            &mut t,
                            &format!("{name}:{an}"),
                            &shape,
                            &want,
                            part == Part::Magnitude && h.dtype.elem.is_complex(),
                            &mut |s| {
                                let mut out = Vec::new();
                                h.read_plane(&bytes, s, part, &mut out).map(|_| out)
                            },
                        );
                    }
                }
            }
            "mat" => {
                let data = std::fs::read(&path).unwrap();
                if man.get("broken").is_some() {
                    // scipy could not read it: it must not panic us, and whatever we list
                    // must read.
                    t.broken += 1;
                    if let Ok(m) = mat::parse(&data) {
                        for v in &m.vars {
                            if let Kind::Numeric { re, .. } = &v.kind
                                && re.buf.is_none()
                            {
                                let mut out = Vec::new();
                                let nd = v.dims.len();
                                let _ = m.read_plane(
                                    &data,
                                    None,
                                    v,
                                    &PlaneSpec {
                                        x_dim: 1,
                                        y_dim: Some(0),
                                        index: vec![0; nd],
                                    },
                                    Part::Magnitude,
                                    &mut out,
                                );
                            }
                        }
                    }
                    continue;
                }
                let m = mat::parse(&data).unwrap_or_else(|e| panic!("{name}: {e}"));
                for r in &refused {
                    match m.vars.iter().find(|v| v.path == *r).map(|v| &v.kind) {
                        Some(Kind::Refused(_)) => t.refused += 1,
                        k => t
                            .failures
                            .push(format!("{name}: {r} should be refused: {k:?}")),
                    }
                }
                let inflated: Vec<Vec<u8>> = (0..m.compressed.len())
                    .map(|i| m.inflate(&data, i, 1 << 30).unwrap())
                    .collect();
                for (p, text) in man["texts"].as_object().unwrap() {
                    let v = m.vars.iter().find(|v| v.path == *p);
                    match v.map(|v| &v.kind) {
                        Some(Kind::Text(s)) if s == text.as_str().unwrap() => t.texts += 1,
                        k => t.failures.push(format!("{name}: text {p}: {k:?}")),
                    }
                }
                for a in arrays {
                    let (an, shape, want, re) = expect(a);
                    let Some(v) = m.vars.iter().find(|v| v.path == an) else {
                        t.failures.push(format!("{name}: no variable {an}"));
                        continue;
                    };
                    let inf = |v: &mat::Var| -> Option<&[u8]> {
                        let buf = match &v.kind {
                            Kind::Numeric { re, .. } | Kind::Sparse { re, .. } => re.buf,
                            _ => None,
                        };
                        buf.map(|b| inflated[b].as_slice())
                    };
                    // scipy drops trailing singleton dimensions past the second as MATLAB
                    // does; compare on the expected shape.
                    let complex = v.complex;
                    for (part, want) in [(Part::Magnitude, Some(want)), (Part::Real, re)] {
                        let Some(want) = want else { continue };
                        let what = format!("{name}:{an}");
                        match &v.kind {
                            Kind::Sparse { .. } | Kind::Triplets { .. } => {
                                let dense = m.sparse_dense(&data, inf(v), v, part).unwrap();
                                // Column-major m × n: compare as the transposed C array.
                                let (rows, cols) = (shape[0] as usize, shape[1] as usize);
                                let mut c_order = vec![0f32; rows * cols];
                                for r in 0..rows {
                                    for c in 0..cols {
                                        c_order[r * cols + c] = dense[c * rows + r];
                                    }
                                }
                                compare(
                                    &mut t,
                                    &what,
                                    &shape,
                                    &want,
                                    complex && part == Part::Magnitude,
                                    &mut |s| {
                                        let (w, h) = (shape[1] as usize, shape[0] as usize);
                                        let _ = s;
                                        Ok(c_order[..w * h].to_vec())
                                    },
                                );
                            }
                            Kind::Numeric { .. } => {
                                let mut dims = v.dims.clone();
                                while dims.len() > shape.len() && dims.last() == Some(&1) {
                                    dims.pop();
                                }
                                assert_eq!(dims, shape, "{what}");
                                compare(
                                    &mut t,
                                    &what,
                                    &shape,
                                    &want,
                                    complex && part == Part::Magnitude,
                                    &mut |s| {
                                        let mut out = Vec::new();
                                        let mut s = s.clone();
                                        s.index.resize(v.dims.len(), 0);
                                        m.read_plane(&data, inf(v), v, &s, part, &mut out)
                                            .map(|_| out)
                                    },
                                );
                            }
                            k => t.failures.push(format!("{what}: {k:?}")),
                        }
                    }
                }
            }
            "zarr" => {
                let store: Arc<dyn Store> = if path.is_dir() {
                    Arc::new(DirStore::new(&path))
                } else {
                    Arc::new(ZipStore::new(std::fs::read(&path).unwrap()).unwrap())
                };
                let z = match zarr::open(store) {
                    Ok(z) => z,
                    Err(e) => {
                        t.failures.push(format!("{name}: {e}"));
                        continue;
                    }
                };
                for (p, why) in &z.refused {
                    t.failures.push(format!("{name}: {p} refused: {why}"));
                }
                for a in arrays {
                    let (an, shape, want, re) = expect(a);
                    let Some(za) = z.arrays.iter().find(|x| x.path == an) else {
                        t.failures.push(format!("{name}: no array {an}"));
                        continue;
                    };
                    assert_eq!(za.shape, shape, "{name}:{an}");
                    for (part, want) in [(Part::Magnitude, Some(want)), (Part::Real, re)] {
                        let Some(want) = want else { continue };
                        compare(
                            &mut t,
                            &format!("{name}:{an}"),
                            &shape,
                            &want,
                            part == Part::Magnitude && za.dtype.elem.is_complex(),
                            &mut |s| {
                                let mut out = Vec::new();
                                z.read_plane(za, s, part, &mut out).map(|_| out)
                            },
                        );
                    }
                }
            }
            k => panic!("{k}"),
        }
    }
    t
}

fn report(dir: &Path, t: &Tally) {
    eprintln!(
        "{}: {} files, {} arrays, {} values, {} mismatches, {} refused as expected, {} texts, \
         {} unreadable by the oracle",
        dir.display(),
        t.files,
        t.arrays,
        t.values,
        t.mismatches,
        t.refused,
        t.texts,
        t.broken
    );
    for f in t.failures.iter().take(40) {
        eprintln!("  {f}");
    }
}

#[test]
fn committed_fixtures_match_the_oracles() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data");
    let t = check_dir(&dir);
    report(&dir, &t);
    assert!(t.files > 40, "{t:?}");
    assert!(t.failures.is_empty() && t.mismatches == 0);
}

#[test]
fn generated_corpus_matches_the_oracles() {
    let Some(dir) = std::env::var_os("SPECULAR_ARRAYS_CORPUS") else {
        return;
    };
    let dir = PathBuf::from(dir);
    let t0 = std::time::Instant::now();
    let t = check_dir(&dir);
    report(&dir, &t);
    eprintln!("checked in {:.1} s", t0.elapsed().as_secs_f64());
    assert!(t.failures.is_empty() && t.mismatches == 0);
}
