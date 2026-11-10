//! The fixtures in tests/data (written by gen/make_fixtures.py with nibabel, and by hand
//! where nibabel does not write the case) against nibabel's reading of them
//! (tests/data/expected.tsv, written by gen/oracle.py): GIFTI arrays (intent, type,
//! dimensions, sums and strided values), coordinate transforms and label tables; CIFTI-2
//! index maps, brain models (vertex and voxel indices), volume spaces, map names, label
//! tables and every map's values, dense connectivity rows; TrackVis and MRtrix streamlines in RAS millimetres with
//! their scalars and properties.
//!
//! The fetched corpus (tools/corpus/fetch-neurosurf.sh) is checked the same way when
//! present, at `SPECULAR_NEURO_DIR` with `SPECULAR_NEURO_ORACLE` (default:
//! tools/corpus/neurosurf and its expected.tsv).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use specular_neuro::{Tractogram, cifti, gifti, tck, trk};

fn num(s: &str) -> f64 {
    match s {
        "nan" => f64::NAN,
        "inf" => f64::INFINITY,
        "-inf" => f64::NEG_INFINITY,
        t => t.parse().unwrap_or_else(|_| panic!("number {t}")),
    }
}

/// Rows by file, then by kind.
type Rows = BTreeMap<String, BTreeMap<String, Vec<Vec<String>>>>;

fn load(tsv: &Path) -> Rows {
    let text = std::fs::read_to_string(tsv).expect("oracle tsv");
    let mut rows: Rows = BTreeMap::new();
    for line in text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
    {
        let f: Vec<String> = line.split('\t').map(str::to_owned).collect();
        rows.entry(f[1].clone())
            .or_default()
            .entry(f[0].clone())
            .or_default()
            .push(f[2..].to_vec());
    }
    rows
}

fn close(a: f64, b: f64, rel: f64, abs: f64) -> bool {
    (a.is_nan() && b.is_nan()) || a == b || (a - b).abs() <= abs.max(rel * a.abs().max(b.abs()))
}

#[derive(Default)]
struct Tally {
    files: usize,
    arrays: usize,
    values: usize,
    labels: usize,
    models: usize,
    maps: usize,
    streamlines: usize,
    points: usize,
    refused_both: usize,
    mismatches: Vec<String>,
}

impl Tally {
    fn check(&mut self, ok: bool, what: impl FnOnce() -> String) {
        if !ok {
            self.mismatches.push(what());
        }
    }
}

fn sums(v: impl Iterator<Item = f64>) -> (usize, f64, f64) {
    v.fold((0, 0.0, 0.0), |(n, s, q), x| (n + 1, s + x, q + x * x))
}

fn check_gifti(name: &str, path: &Path, rows: &BTreeMap<String, Vec<Vec<String>>>, t: &mut Tally) {
    let g = match gifti::open(path) {
        Ok(g) => g,
        Err(e) => return t.check(false, || format!("{name}: refused: {e}")),
    };
    let head = &rows["gifti"][0];
    t.check(g.arrays.len() == head[0].parse::<usize>().unwrap(), || {
        format!("{name}: {} arrays, nibabel {}", g.arrays.len(), head[0])
    });
    t.check(g.labels.len() == head[1].parse::<usize>().unwrap(), || {
        format!("{name}: {} labels, nibabel {}", g.labels.len(), head[1])
    });
    for r in rows.get("garray").into_iter().flatten() {
        let i: usize = r[0].parse().unwrap();
        let Some(a) = g.arrays.get(i) else { continue };
        t.arrays += 1;
        t.check(a.intent == r[1], || {
            format!("{name}[{i}]: intent {} vs {}", a.intent, r[1])
        });
        t.check(a.values.type_name() == r[2], || {
            format!("{name}[{i}]: type {} vs {}", a.values.type_name(), r[2])
        });
        let dims = a
            .dims
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",");
        t.check(dims == r[3], || {
            format!("{name}[{i}]: dims {dims} vs {}", r[3])
        });
        let (n, s, q) = sums((0..a.values.len()).map(|k| a.values.get(k)));
        t.check(n == r[4].parse::<usize>().unwrap(), || {
            format!("{name}[{i}]: {n} values vs {}", r[4])
        });
        t.check(close(s, num(&r[5]), 1e-9, 1e-9), || {
            format!("{name}[{i}]: sum {s} vs {}", r[5])
        });
        t.check(close(q, num(&r[6]), 1e-9, 1e-9), || {
            format!("{name}[{i}]: sumsq {q} vs {}", r[6])
        });
    }
    for r in rows.get("gname").into_iter().flatten() {
        let i: usize = r[0].parse().unwrap();
        let ours = g.arrays.get(i).and_then(|a| a.name()).unwrap_or("");
        t.check(ours == r[1], || {
            format!("{name}[{i}]: name {ours:?} vs {:?}", r[1])
        });
    }
    for r in rows.get("gval").into_iter().flatten() {
        let (i, k): (usize, usize) = (r[0].parse().unwrap(), r[1].parse().unwrap());
        t.values += 1;
        let ours = g
            .arrays
            .get(i)
            .filter(|a| k < a.values.len())
            .map(|a| a.values.get(k));
        t.check(ours.is_some_and(|v| close(v, num(&r[2]), 0.0, 0.0)), || {
            format!("{name}[{i}][{k}]: {ours:?} vs {}", r[2])
        });
    }
    for r in rows.get("gxform").into_iter().flatten() {
        let (i, k): (usize, usize) = (r[0].parse().unwrap(), r[1].parse().unwrap());
        let c = g.arrays.get(i).and_then(|a| a.coord.get(k));
        // nibabel gives an array without a transform an identity between unknown spaces.
        let default = r[2] == "NIFTI_XFORM_UNKNOWN"
            && r[3] == "NIFTI_XFORM_UNKNOWN"
            && (0..16).all(|m| num(&r[4 + m]) == if m % 5 == 0 { 1.0 } else { 0.0 });
        let ok = (c.is_none() && default && i < g.arrays.len())
            || c.is_some_and(|c| {
                c.data_space == r[2]
                    && c.transformed_space == r[3]
                    && (0..16).all(|m| close(c.matrix[m / 4][m % 4], num(&r[4 + m]), 1e-12, 1e-12))
            });
        t.check(ok, || {
            format!("{name}[{i}]: transform {k}: {c:?} vs {:?}", &r[2..])
        });
    }
    for r in rows.get("glabel").into_iter().flatten() {
        t.labels += 1;
        let key: i32 = r[0].parse().unwrap();
        let l = g.label(key);
        let ok = l.is_some_and(|l| {
            l.name == r[1]
                && (0..4).all(|c| close(f64::from(l.rgba[c]), num(&r[2 + c]), 1e-6, 1e-6))
        });
        t.check(ok, || {
            format!("{name}: label {key}: {l:?} vs {:?}", &r[1..])
        });
    }
}

fn model_checksum(m: &cifti::BrainModel) -> (&'static str, usize, i64) {
    match &m.model {
        cifti::Model::Surface {
            vertices_total,
            vertices,
        } => (
            "surface",
            *vertices_total,
            vertices.iter().map(|&v| i64::from(v)).sum(),
        ),
        cifti::Model::Voxels(v) => (
            "voxels",
            0,
            v.iter()
                .map(|p| i64::from(p[0]) + 1000 * i64::from(p[1]) + 1_000_000 * i64::from(p[2]))
                .sum(),
        ),
    }
}

fn check_cifti(name: &str, path: &Path, rows: &BTreeMap<String, Vec<Vec<String>>>, t: &mut Tally) {
    let raw = std::fs::read(path).unwrap();
    let data = match cifti::inflate(&raw) {
        Ok(d) => d,
        Err(e) => return t.check(false, || format!("{name}: refused: {e}")),
    };
    let c = match cifti::parse(&data) {
        Ok(c) => c,
        Err(e) => return t.check(false, || format!("{name}: refused: {e}")),
    };
    let head = &rows["cifti"][0];
    let ours = [
        c.shape[0].to_string(),
        c.shape[1].to_string(),
        c.maps[0].kind().into(),
        c.maps[1].kind().into(),
    ];
    t.check(ours[..] == head[..4], || {
        format!("{name}: {ours:?} vs {head:?}")
    });
    for (k, r) in rows.get("cmodel").into_iter().flatten().enumerate() {
        t.models += 1;
        let Some(m) = c.models().get(k) else {
            t.check(false, || format!("{name}: no brain model {k}"));
            continue;
        };
        let (ty, total, sum) = model_checksum(m);
        let ours = [
            m.offset.to_string(),
            m.count.to_string(),
            m.structure.clone(),
            ty.into(),
            total.to_string(),
            sum.to_string(),
        ];
        t.check(ours[..] == r[..6], || {
            format!("{name}: model {k}: {ours:?} vs {r:?}")
        });
    }
    // Sums of f32 values match nibabel's to 1e-9, of float64 ones rounded to f32 to 1e-6.
    let tol = if c.nifti.datatype == 64 { 1e-6 } else { 1e-9 };
    // Dense connectivity: dimension 0's brain models, and rows read whole.
    for (k, r) in rows.get("cmodel0").into_iter().flatten().enumerate() {
        t.models += 1;
        let Some(m) = c.models_of(0).get(k) else {
            t.check(false, || format!("{name}: no dimension-0 brain model {k}"));
            continue;
        };
        let (ty, total, sum) = model_checksum(m);
        let ours = [
            m.offset.to_string(),
            m.count.to_string(),
            m.structure.clone(),
            ty.into(),
            total.to_string(),
            sum.to_string(),
        ];
        t.check(ours[..] == r[..6], || {
            format!("{name}: dimension-0 model {k}: {ours:?} vs {r:?}")
        });
    }
    if rows.contains_key("crow") {
        t.check(c.is_dense_connectivity(), || {
            format!("{name}: not read as dense connectivity")
        });
    }
    let mut got: BTreeMap<usize, Vec<f32>> = BTreeMap::new();
    for r in rows.get("crow").into_iter().flatten() {
        let i: usize = r[0].parse().unwrap();
        t.maps += 1;
        let m = match c.row(&data, i) {
            Ok(m) => m,
            Err(e) => {
                t.check(false, || format!("{name}: row {i}: {e}"));
                continue;
            }
        };
        let (n, s, q) = sums(m.iter().map(|&v| f64::from(v)));
        t.check(
            n.to_string() == r[1]
                && close(s, num(&r[2]), tol, tol)
                && close(q, num(&r[3]), tol, tol),
            || format!("{name}: row {i}: ({n}, {s}, {q}) vs {r:?}"),
        );
        got.insert(i, m);
    }
    for r in rows.get("crowval").into_iter().flatten() {
        let (i, k): (usize, usize) = (r[0].parse().unwrap(), r[1].parse().unwrap());
        t.values += 1;
        let v = got.get(&i).and_then(|m| m.get(k)).map(|&v| f64::from(v));
        // Values are f32 here; nibabel's float64 ones are compared after rounding.
        t.check(
            v.is_some_and(|v| close(v, f64::from(num(&r[2]) as f32), 0.0, 0.0)),
            || format!("{name}: row {i}[{k}]: {v:?} vs {}", r[2]),
        );
    }
    for r in rows.get("cvol").into_iter().flatten() {
        let v = c.volume();
        let ok = v.is_some_and(|v| {
            (0..3).all(|a| v.dims[a].to_string() == r[a])
                && (0..16).all(|m| close(v.affine[m / 4][m % 4], num(&r[3 + m]), 1e-9, 1e-9))
        });
        t.check(ok, || format!("{name}: volume {v:?} vs {r:?}"));
    }
    if let Some(cifti::IndexMap::Parcels { parcels, .. }) = c.maps.get(c.spatial_dim()) {
        for r in rows.get("cparcel").into_iter().flatten() {
            let i: usize = r[0].parse().unwrap();
            let p = parcels.get(i);
            let ok = p.is_some_and(|p| {
                p.name == r[1]
                    && p.vertices
                        .iter()
                        .map(|v| v.1.len())
                        .sum::<usize>()
                        .to_string()
                        == r[2]
                    && p.voxels.len().to_string() == r[3]
            });
            t.check(ok, || format!("{name}: parcel {i}: {p:?} vs {r:?}"));
        }
    }
    for r in rows.get("cmapname").into_iter().flatten() {
        let i: usize = r[0].parse().unwrap();
        t.check(c.map_name(i) == r[1], || {
            format!("{name}: map {i} name {:?} vs {:?}", c.map_name(i), r[1])
        });
    }
    for r in rows.get("clabel").into_iter().flatten() {
        t.labels += 1;
        let (i, key): (usize, i32) = (r[0].parse().unwrap(), r[1].parse().unwrap());
        let l = c.labels(i).and_then(|ls| ls.iter().find(|l| l.key == key));
        let ok = l.is_some_and(|l| {
            l.name == r[2]
                && (0..4).all(|k| close(f64::from(l.rgba[k]), num(&r[3 + k]), 1e-6, 1e-6))
        });
        t.check(ok, || {
            format!("{name}: map {i} label {key}: {l:?} vs {r:?}")
        });
    }
    let mut cache: BTreeMap<usize, Vec<f32>> = BTreeMap::new();
    for r in rows.get("cmap").into_iter().flatten() {
        let i: usize = r[0].parse().unwrap();
        t.maps += 1;
        let m = match c.map(&data, i) {
            Ok(m) => m,
            Err(e) => {
                t.check(false, || format!("{name}: map {i}: {e}"));
                continue;
            }
        };
        let (n, s, q) = sums(m.iter().map(|&v| f64::from(v)));
        t.check(
            n.to_string() == r[1]
                && close(s, num(&r[2]), tol, tol)
                && close(q, num(&r[3]), tol, tol),
            || format!("{name}: map {i}: ({n}, {s}, {q}) vs {r:?}"),
        );
        cache.insert(i, m);
    }
    for r in rows.get("cval").into_iter().flatten() {
        let (i, k): (usize, usize) = (r[0].parse().unwrap(), r[1].parse().unwrap());
        t.values += 1;
        let v = cache.get(&i).and_then(|m| m.get(k)).map(|&v| f64::from(v));
        // Maps are f32 here: a float64 file's values are compared after rounding.
        t.check(
            v.is_some_and(|v| close(v, f64::from(num(&r[2]) as f32), 0.0, 0.0)),
            || format!("{name}: map {i}[{k}]: {v:?} vs {}", r[2]),
        );
    }
}

fn read_tracts(path: &Path) -> specular_neuro::Result<Tractogram> {
    let raw = std::fs::read(path).map_err(|e| specular_neuro::Error::Io(e.to_string()))?;
    if trk::sniff(&raw) {
        trk::parse(&raw).map(|x| x.1)
    } else {
        tck::parse(&raw).map(|x| x.1)
    }
}

fn check_tracts(name: &str, path: &Path, rows: &BTreeMap<String, Vec<Vec<String>>>, t: &mut Tally) {
    let tr = match read_tracts(path) {
        Ok(x) => x,
        Err(e) => return t.check(false, || format!("{name}: refused: {e}")),
    };
    let head = &rows["tract"][0];
    t.streamlines += tr.streamlines();
    t.check(
        tr.streamlines().to_string() == head[0] && tr.points.len().to_string() == head[1],
        || {
            format!(
                "{name}: {} streamlines, {} points vs {head:?}",
                tr.streamlines(),
                tr.points.len()
            )
        },
    );
    // nibabel computes in float32; a few ulps at 100 mm.
    let tol = |a: f64, b: f64| close(a, b, 1e-5, 1e-4);
    for r in rows.get("tsum").into_iter().flatten() {
        let s = (0..3).map(|a| tr.points.iter().map(|p| f64::from(p[a])).sum::<f64>());
        let ok = s.zip(r).all(|(s, e)| close(s, num(e), 1e-6, 1e-2));
        t.check(ok, || format!("{name}: point sums vs {r:?}"));
    }
    for r in rows.get("tpt").into_iter().flatten() {
        let k: usize = r[0].parse().unwrap();
        t.points += 1;
        let p = tr.points.get(k);
        t.check(
            p.is_some_and(|p| (0..3).all(|a| tol(f64::from(p[a]), num(&r[1 + a])))),
            || format!("{name}: point {k}: {p:?} vs {:?}", &r[1..]),
        );
    }
    for r in rows.get("tscalar").into_iter().flatten() {
        let k: usize = r[0].parse().unwrap();
        let s = tr
            .scalars
            .get(k)
            .map(|c| c.1.iter().map(|&v| f64::from(v)).sum::<f64>());
        t.check(s.is_some_and(|s| close(s, num(&r[1]), 1e-6, 1e-6)), || {
            format!("{name}: scalar column {k}: {s:?} vs {}", r[1])
        });
    }
    for r in rows.get("tprop").into_iter().flatten() {
        let k: usize = r[0].parse().unwrap();
        let s = tr
            .properties
            .get(k)
            .map(|c| c.1.iter().map(|&v| f64::from(v)).sum::<f64>());
        t.check(s.is_some_and(|s| close(s, num(&r[1]), 1e-6, 1e-6)), || {
            format!("{name}: property column {k}: {s:?} vs {}", r[1])
        });
    }
}

fn run(dir: &Path, tsv: &Path) -> Tally {
    let rows = load(tsv);
    let mut t = Tally::default();
    for (name, rs) in &rows {
        // MRtrix images are specular-medvol's (its tests/mif.rs reads those rows).
        if [".mif", ".mif.gz", ".mih"]
            .iter()
            .any(|e| name.ends_with(e))
        {
            continue;
        }
        let path = dir.join(name);
        t.files += 1;
        if let Some(e) = rs.get("error") {
            // nibabel refused it: we must refuse it too, or say what we read.
            let ours: Result<(), String> = if name.ends_with(".gii") {
                gifti::open(&path).map(|_| ()).map_err(|e| e.to_string())
            } else if name.ends_with(".nii") || name.ends_with(".nii.gz") {
                std::fs::read(&path)
                    .map_err(|e| e.to_string())
                    .and_then(|r| {
                        let d = cifti::inflate(&r).map_err(|e| e.to_string())?;
                        cifti::parse(&d).map(|_| ()).map_err(|e| e.to_string())
                    })
            } else {
                read_tracts(&path).map(|_| ()).map_err(|e| e.to_string())
            };
            match ours {
                Err(_) => t.refused_both += 1,
                Ok(()) => eprintln!("{name}: nibabel refused ({:?}), read here", e[0]),
            }
            continue;
        }
        if rs.contains_key("gifti") {
            check_gifti(name, &path, rs, &mut t);
        } else if rs.contains_key("cifti") {
            check_cifti(name, &path, rs, &mut t);
        } else if rs.contains_key("tract") {
            check_tracts(name, &path, rs, &mut t);
        }
    }
    t
}

fn report(what: &str, t: &Tally, secs: f64) {
    eprintln!(
        "{what}: {} files, {} arrays, {} values, {} labels, {} brain models, {} maps, {} streamlines, \
         {} points checked; {} refused by both; {} mismatches in {secs:.2} s",
        t.files,
        t.arrays,
        t.values,
        t.labels,
        t.models,
        t.maps,
        t.streamlines,
        t.points,
        t.refused_both,
        t.mismatches.len()
    );
    for m in t.mismatches.iter().take(40) {
        eprintln!("  {m}");
    }
}

#[test]
fn fixtures_match_nibabel() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data");
    let t0 = Instant::now();
    let t = run(&dir, &dir.join("expected.tsv"));
    report("fixtures", &t, t0.elapsed().as_secs_f64());
    assert!(t.files >= 20, "fixtures missing");
    assert!(t.mismatches.is_empty());
}

#[test]
fn corpus_matches_nibabel() {
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
    let t0 = Instant::now();
    let t = run(&dir, &tsv);
    report("corpus", &t, t0.elapsed().as_secs_f64());
    assert!(t.mismatches.is_empty());
}
