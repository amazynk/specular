//! File Geodatabases against GDAL's OpenFileGDB driver (through pyogrio): layers, feature
//! counts, vertex counts, attributes and coordinates (1 mm, or 1e-8 degrees).
//!
//! The expectations come from `gen/filegdb_oracle.py` (run by
//! `tools/corpus/fetch-gis-formats.sh`) into the corpus's `expected/filegdb.jsonl`; the
//! corpus is read from SPECULAR_GIS_FORMATS_CORPUS, else tools/corpus/gis-formats, and the
//! test skips without it. SPECULAR_FGDB_EXTRA may name a folder standing in for
//! `gdal/filegdb` (zipped geodatabases unpacked elsewhere).

use serde_json::Value as J;
use specular_gis::{Geometry, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn corpus() -> Option<PathBuf> {
    let dir = std::env::var_os("SPECULAR_GIS_FORMATS_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/gis-formats")
        });
    if dir.join("expected/filegdb.jsonl").is_file() {
        Some(dir)
    } else {
        eprintln!("no File Geodatabase oracle at {}; skipping", dir.display());
        None
    }
}

fn locate(dir: &Path, file: &str) -> PathBuf {
    let p = dir.join(file);
    if p.exists() {
        return p;
    }
    if let (Some(extra), Some(rest)) = (
        std::env::var_os("SPECULAR_FGDB_EXTRA"),
        file.strip_prefix("gdal/filegdb/"),
    ) {
        return PathBuf::from(extra).join(rest);
    }
    p
}

/// Layers read differently by design: (file, layer, reason).
const LAYER_SKIP: &[(&str, &str, &str)] = &[];

/// Geodatabases refused by design: (file, part of the message).
const REFUSED: &[(&str, &str)] = &[(
    "gdal/filegdb/objectid64/with_holes_8.gdb",
    "64-bit object ids and gaps between them are not supported",
)];

/// Layers with arcs, Béziers or elliptic arcs: GDAL strokes them with its own step, so
/// their features are compared by bounding box (to 0.5 % of its size), not point by point.
const CURVE_LAYERS: &[(&str, &str)] = &[
    (
        "gdal/filegdb/filegdb_polygonzm_m_not_closing_with_curves.gdb",
        "polygonArcsZM1",
    ),
    (
        "gdal/filegdb/filegdb_polygonzm_nan_m_with_curves.gdb",
        "sample2",
    ),
    ("gdal/filegdb/curves.gdb", "polygon"),
    ("gdal/filegdb/curves.gdb", "line"),
    ("gdal/filegdb/curves.gdb", "multipolygon"),
    ("gdal/filegdb/curves.gdb", "multiline"),
    ("gdal/filegdb/curve_circle_by_center.gdb", "test"),
];

/// Geometries read differently by design: (file, feature index, reason).
const GEOMETRY_SKIP: &[(&str, usize, &str)] = &[(
    "gdal/filegdb/curve_circle_by_center.gdb",
    5,
    "an arc whose stored centre is 1 from its start and 2 from its end: GDAL fits another circle",
)];

fn bbox(parts: &[Part]) -> Option<[f64; 4]> {
    let mut b = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    for p in parts.iter().flatten() {
        b = [
            b[0].min(p[0]),
            b[1].min(p[1]),
            b[2].max(p[0]),
            b[3].max(p[1]),
        ];
    }
    (b[0] <= b[2]).then_some(b)
}

fn same_bbox(a: &[Part], b: &[Part]) -> bool {
    match (bbox(a), bbox(b)) {
        (Some(a), Some(b)) => {
            let size = (a[2] - a[0]).max(a[3] - a[1]).max(1e-9);
            a.iter().zip(&b).all(|(x, y)| (x - y).abs() <= 0.005 * size)
        }
        (None, None) => true,
        _ => false,
    }
}

fn close(a: f64, b: f64) -> bool {
    let tol = if a.abs().max(b.abs()) <= 360.0 {
        1e-8
    } else {
        1e-3
    };
    (a - b).abs() <= tol || (a.is_nan() && b.is_nan())
}

type Part = Vec<[f64; 2]>;

fn parts(g: &Geometry, out: &mut Vec<Part>) {
    match g {
        Geometry::Point(p) => out.push(vec![*p]),
        Geometry::MultiPoint(v) => out.extend(v.iter().map(|p| vec![*p])),
        Geometry::LineString(v) => out.push(v.clone()),
        Geometry::MultiLineString(v) | Geometry::Polygon(v) => out.extend(v.iter().cloned()),
        Geometry::MultiPolygon(v) => out.extend(v.iter().flatten().cloned()),
        Geometry::Collection(g) => g.iter().for_each(|g| parts(g, out)),
    }
}

fn normalise(mut parts: Vec<Part>) -> Vec<Part> {
    parts.retain(|p| !p.is_empty());
    for p in &mut parts {
        if p.len() > 2 && p.first() == p.last() {
            p.pop();
        }
    }
    parts
}

fn oracle_parts(j: &J) -> Option<Vec<Part>> {
    Some(
        j.as_array()?
            .iter()
            .map(|part| {
                part.as_array()
                    .unwrap()
                    .iter()
                    .map(|c| {
                        let c = c.as_array().unwrap();
                        [
                            c[0].as_f64().unwrap_or(f64::NAN),
                            c[1].as_f64().unwrap_or(f64::NAN),
                        ]
                    })
                    .collect()
            })
            .collect(),
    )
}

fn same_parts(mut ours: Vec<Part>, theirs: Vec<Part>) -> bool {
    if ours.len() != theirs.len() {
        return false;
    }
    let eq = |a: &Part, b: &Part| {
        a.len() == b.len()
            && (a
                .iter()
                .zip(b)
                .all(|(p, q)| close(p[0], q[0]) && close(p[1], q[1]))
                || a.iter()
                    .zip(b.iter().rev())
                    .all(|(p, q)| close(p[0], q[0]) && close(p[1], q[1])))
    };
    'next: for t in &theirs {
        for i in 0..ours.len() {
            if eq(&ours[i], t) {
                ours.swap_remove(i);
                continue 'next;
            }
        }
        return false;
    }
    true
}

fn same_value(ours: &Value, theirs: &J) -> bool {
    match (ours, theirs) {
        (Value::Null, J::Null) => true,
        (Value::Text(s), J::Null) | (Value::Date(s), J::Null) => s.is_empty(),
        (Value::Int(a), J::Number(n)) => n.as_f64() == Some(*a as f64),
        (Value::Float(a), J::Number(n)) => {
            let b = n.as_f64().unwrap();
            (a - b).abs() <= 1e-9 * a.abs().max(b.abs()).max(1.0)
        }
        (Value::Float(a), J::Null) => a.is_nan(),
        (Value::Text(a), J::String(b)) => a.trim_end() == b.trim_end(),
        (Value::Date(a), J::String(b)) => {
            let n = |s: &str| {
                s.replace(['/', 'T'], " ")
                    .trim_end_matches(['Z', '0', ':', '.', ' '])
                    .to_string()
            };
            n(a) == n(b)
        }
        (Value::Blob(a), J::Object(o)) => o.get("blob").and_then(J::as_u64) == Some(*a as u64),
        _ => false,
    }
}

#[test]
fn filegdb_matches_ogr() {
    let Some(dir) = corpus() else { return };
    let text = std::fs::read_to_string(dir.join("expected/filegdb.jsonl")).unwrap();
    let mut by_file: BTreeMap<String, Vec<J>> = BTreeMap::new();
    for l in text.lines() {
        let r: J = serde_json::from_str(l).unwrap();
        by_file
            .entry(r["file"].as_str().unwrap().to_string())
            .or_default()
            .push(r);
    }
    let (mut files, mut layers, mut features, mut vertices, mut coords) =
        (0, 0, 0usize, 0usize, 0usize);
    let (mut reprojected, mut refused) = (0, 0);
    let mut mismatches: Vec<String> = Vec::new();
    for (file, recs) in &by_file {
        let path = locate(&dir, file);
        if !path.exists() {
            eprintln!("  {file}: not unpacked; skipped");
            continue;
        }
        let gdal_failed = recs.iter().all(|r| r.get("error").is_some());
        let ds = match specular_gis::filegdb::read(&path) {
            Ok(ds) => ds,
            Err(e) => {
                if REFUSED.iter().any(|(f, m)| f == file && e.contains(m)) {
                    refused += 1;
                } else if !gdal_failed {
                    mismatches.push(format!("{file}: {e}"));
                }
                continue;
            }
        };
        files += 1;
        for r in recs {
            if r.get("error").is_some() {
                continue;
            }
            let lname = r["layer"].as_str().unwrap();
            if LAYER_SKIP.iter().any(|(f, l, _)| f == file && *l == lname) {
                continue;
            }
            let count = r["count"].as_u64().unwrap() as usize;
            let Some(layer) = ds.layers.iter().find(|l| l.name == lname) else {
                let ours: Vec<_> = ds.layers.iter().map(|l| l.name.as_str()).collect();
                mismatches.push(format!("{file}: no layer {lname:?} (ours {ours:?})"));
                continue;
            };
            layers += 1;
            if layer.features.len() != count {
                mismatches.push(format!(
                    "{file}/{lname}: {} features, OGR {count}",
                    layer.features.len()
                ));
                continue;
            }
            features += count;
            // A layer in another CRS than the dataset's is projected into it: compared on
            // counts and attributes only.
            let projected = ds
                .warnings
                .iter()
                .any(|w| w.starts_with(&format!("{lname}: projected from")));
            if projected {
                reprojected += 1;
            } else if let (Some(theirs), Some(c)) = (r["epsg"].as_u64(), &ds.crs)
                // GDAL names a CRS with heights 3D (EPSG:4979); the map is 4326.
                && c.epsg.is_some_and(|e| u64::from(e) != theirs && !(e == 4326 && theirs == 4979))
            {
                mismatches.push(format!("{file}/{lname}: EPSG {:?}, OGR {theirs}", c.epsg));
            }
            let curved = CURVE_LAYERS.iter().any(|(f, l)| f == file && *l == lname);
            let ours_v: usize = layer
                .features
                .iter()
                .filter_map(|f| f.geometry.as_ref())
                .map(Geometry::vertex_count)
                .sum();
            vertices += ours_v;
            let theirs_v = r["vertices"].as_u64().unwrap() as usize;
            if !r["unparsed"].as_bool().unwrap_or(false) && !curved && ours_v != theirs_v {
                mismatches.push(format!("{file}/{lname}: {ours_v} vertices, OGR {theirs_v}"));
            }
            let fields: Vec<&str> = r["fields"]
                .as_array()
                .unwrap()
                .iter()
                .map(|f| f.as_str().unwrap())
                .collect();
            let map: Vec<(usize, usize)> = fields
                .iter()
                .enumerate()
                .filter_map(|(t, name)| {
                    layer
                        .fields()
                        .iter()
                        .position(|o| o.eq_ignore_ascii_case(name))
                        .map(|o| (o, t))
                })
                .collect();
            if map.len() != fields.len() {
                mismatches.push(format!(
                    "{file}/{lname}: fields {:?}, OGR {fields:?}",
                    layer.fields()
                ));
            }
            let mut bad = 0;
            for (i, f) in r["features"].as_array().unwrap().iter().enumerate() {
                let ours = &layer.features[i];
                if ours.id.as_deref() != Some(&f["fid"].to_string()) && bad < 3 {
                    bad += 1;
                    mismatches.push(format!(
                        "{file}/{lname} #{i}: fid {:?}, OGR {}",
                        ours.id, f["fid"]
                    ));
                }
                let attrs = f["attrs"].as_array().unwrap();
                for &(o, t) in &map {
                    let v = ours.values.get(o).unwrap_or(&Value::Null);
                    if !same_value(v, &attrs[t]) && bad < 3 {
                        bad += 1;
                        mismatches.push(format!(
                            "{file}/{lname} #{i} {}: {v:?}, OGR {}",
                            fields[t], attrs[t]
                        ));
                    }
                }
                if let Some(theirs) = oracle_parts(&f["parts"])
                    && !projected
                    && !GEOMETRY_SKIP
                        .iter()
                        .any(|(sf, si, _)| *sf == file && *si == i)
                {
                    let mut p = Vec::new();
                    if let Some(g) = &ours.geometry {
                        parts(g, &mut p);
                    }
                    coords += theirs.iter().map(Vec::len).sum::<usize>();
                    let same = if curved {
                        p.len() == theirs.len() && same_bbox(&p, &theirs)
                    } else {
                        same_parts(normalise(p.clone()), normalise(theirs.clone()))
                    };
                    if !same && bad < 3 {
                        bad += 1;
                        mismatches.push(format!(
                            "{file}/{lname} #{i}: geometry {} parts / {} points, OGR {} / {}",
                            p.len(),
                            p.iter().map(Vec::len).sum::<usize>(),
                            theirs.len(),
                            theirs.iter().map(Vec::len).sum::<usize>()
                        ));
                    }
                }
            }
        }
    }
    eprintln!(
        "{files} geodatabases ({refused} refused by design), {layers} layers ({reprojected} projected into another layer's CRS), {features} features, {vertices} vertices, {coords} coordinates compared; {} mismatches",
        mismatches.len()
    );
    for m in &mismatches {
        eprintln!("  {m}");
    }
    assert!(mismatches.is_empty(), "{} mismatches", mismatches.len());
}

/// Damaged copies of a real geodatabase (truncated and bit-flipped tables) never panic.
#[test]
fn damaged_geodatabases_fail_cleanly() {
    let Some(dir) = corpus() else { return };
    let src = locate(
        &dir,
        "gdal/filegdb/testopenfilegdb.gdb.unzipped/testopenfilegdb.gdb",
    );
    if !src.is_dir() {
        return;
    }
    let work = std::env::temp_dir().join(format!("specular-fgdb-damage-{}", std::process::id()));
    let gdb = work.join("t.gdb");
    std::fs::create_dir_all(&gdb).unwrap();
    for e in std::fs::read_dir(&src).unwrap().flatten() {
        std::fs::copy(e.path(), gdb.join(e.file_name())).unwrap();
    }
    let mut tables: Vec<_> = std::fs::read_dir(&gdb)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .is_some_and(|x| x == "gdbtable" || x == "gdbtablx")
        })
        .collect();
    tables.sort();
    let mut seed = 0x9E37_79B9u32;
    let mut next = || {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        seed
    };
    for t in &tables {
        let orig = std::fs::read(t).unwrap();
        for round in 0..8 {
            let mut b = orig.clone();
            if round % 3 == 0 {
                b.truncate(next() as usize % (b.len() + 1));
            } else if !b.is_empty() {
                for _ in 0..4 {
                    let k = next() as usize % b.len();
                    b[k] ^= (next() >> 24) as u8 | 1;
                }
            }
            std::fs::write(t, &b).unwrap();
            let _ = specular_gis::filegdb::read(&gdb);
        }
        std::fs::write(t, &orig).unwrap();
    }
    let _ = std::fs::remove_dir_all(&work);
}
