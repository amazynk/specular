//! Shared by the corpus tests: comparing what the readers make of vector files with the
//! oracle's records (feature counts, attributes, coordinates within 1 mm or 1e-8 degrees).
#![allow(dead_code)]

use serde_json::Value as J;
use specular_gis::{Geometry, Value, VectorKind};
use std::collections::BTreeMap;
use std::path::Path;

/// SQLite in process, for the corpus run only (the viewer queries it in the decode host).
struct LocalDb(std::sync::Mutex<rusqlite::Connection>);

impl LocalDb {
    fn open(path: &Path) -> LocalDb {
        let flags = rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY;
        LocalDb(std::sync::Mutex::new(
            rusqlite::Connection::open_with_flags(path, flags).unwrap(),
        ))
    }
}

impl specular_gis::sql::SqlDb for LocalDb {
    fn query(
        &self,
        sql: &str,
        params: &[specular_gis::sql::SqlValue],
        limits: specular_gis::sql::SqlLimits,
    ) -> Result<specular_gis::sql::SqlRows, String> {
        use rusqlite::types::ValueRef;
        use specular_gis::sql::{SqlRows, SqlValue};
        let c = self.0.lock().unwrap();
        let mut st = c.prepare(sql).map_err(|e| e.to_string())?;
        for (i, p) in params.iter().enumerate() {
            let r = match p {
                SqlValue::Null => st.raw_bind_parameter(i + 1, rusqlite::types::Null),
                SqlValue::Int(v) => st.raw_bind_parameter(i + 1, v),
                SqlValue::Real(v) => st.raw_bind_parameter(i + 1, v),
                SqlValue::Text(v) => st.raw_bind_parameter(i + 1, v),
                SqlValue::Blob(v) => st.raw_bind_parameter(i + 1, v),
            };
            r.map_err(|e| e.to_string())?;
        }
        let columns = st.column_count();
        let mut it = st.raw_query();
        let mut out = SqlRows {
            columns,
            ..SqlRows::default()
        };
        while let Some(r) = it.next().map_err(|e| e.to_string())? {
            if out.len() >= limits.max_rows as usize {
                out.truncated = true;
                break;
            }
            for i in 0..columns {
                out.values
                    .push(match r.get_ref(i).map_err(|e| e.to_string())? {
                        ValueRef::Null => SqlValue::Null,
                        ValueRef::Integer(v) => SqlValue::Int(v),
                        ValueRef::Real(v) => SqlValue::Real(v),
                        ValueRef::Text(t) => SqlValue::Text(String::from_utf8_lossy(t).into()),
                        ValueRef::Blob(b) => SqlValue::Blob(b.to_vec()),
                    });
            }
        }
        Ok(out)
    }
}

/// Files, geometries and layers the oracle reads differently by design, with the reason.
pub struct Skips<'a> {
    pub vector: &'a [(&'a str, &'a str)],
    pub geometry: &'a [(&'a str, usize, &'a str)],
    pub layer: &'a [(&'a str, &'a str)],
}

pub struct Report {
    pub files: usize,
    pub layers: usize,
    pub features: usize,
    pub vertices: usize,
    pub crs_checked: usize,
    pub mismatches: Vec<String>,
}

impl Report {
    pub fn print(&self, what: &str) {
        eprintln!(
            "{what}: {} files, {} layers, {} features, {} vertices, {} CRSs; {} mismatches",
            self.files,
            self.layers,
            self.features,
            self.vertices,
            self.crs_checked,
            self.mismatches.len()
        );
        for m in &self.mismatches {
            eprintln!("  {m}");
        }
    }
}

pub fn records(dir: &Path, name: &str) -> Vec<J> {
    let text = std::fs::read_to_string(dir.join("expected").join(name)).unwrap();
    text.lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

/// Coordinates agree to 1 mm (projected) or about 1 mm of arc (geographic: 1e-8 degrees).
pub fn close(a: f64, b: f64) -> bool {
    let tol = if a.abs().max(b.abs()) <= 360.0 {
        1e-8
    } else {
        1e-3
    };
    (a - b).abs() <= tol || (a.is_nan() && b.is_nan())
}

pub type Part = Vec<[f64; 2]>;

/// A geometry as parts, the way the oracle flattens it: one per point, line or ring, with
/// a closing point that repeats the first dropped (readers differ on closing rings).
pub fn parts(g: &Geometry, out: &mut Vec<Part>) {
    match g {
        Geometry::Point(p) => out.push(vec![*p]),
        Geometry::MultiPoint(v) => out.extend(v.iter().map(|p| vec![*p])),
        Geometry::LineString(v) => out.push(v.clone()),
        Geometry::MultiLineString(v) | Geometry::Polygon(v) => out.extend(v.iter().cloned()),
        Geometry::MultiPolygon(v) => out.extend(v.iter().flatten().cloned()),
        Geometry::Collection(g) => g.iter().for_each(|g| parts(g, out)),
    }
}

pub fn normalise(mut parts: Vec<Part>) -> Vec<Part> {
    parts.retain(|p| !p.is_empty());
    for p in &mut parts {
        if p.len() > 2 && p.first() == p.last() {
            p.pop();
        }
    }
    parts
}

pub fn oracle_parts(j: &J) -> Option<Vec<Part>> {
    let a = j.as_array()?;
    Some(
        a.iter()
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

/// The same parts in any order (each matched once), each the same points in order or
/// reversed (ring orientation is the writer's choice in some formats).
pub fn same_parts(mut ours: Vec<Part>, theirs: Vec<Part>) -> bool {
    if ours.len() != theirs.len() {
        return false;
    }
    // The same points in order, reversed, or (rings, whose start is the writer's choice)
    // rotated either way.
    let eq = |a: &Part, b: &Part| {
        let n = a.len();
        if n != b.len() {
            return false;
        }
        let at = |p: &[f64; 2], q: &[f64; 2]| close(p[0], q[0]) && close(p[1], q[1]);
        let starts: Vec<usize> = if n >= 3 {
            (0..n).filter(|&k| at(&a[0], &b[k])).collect()
        } else {
            vec![0, n.saturating_sub(1)]
        };
        starts.into_iter().any(|k| {
            (0..n).all(|i| at(&a[i], &b[(k + i) % n]))
                || (0..n).all(|i| at(&a[i], &b[(k + n - i % n) % n]))
        })
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

pub fn same_value(ours: &Value, theirs: &J) -> bool {
    match (ours, theirs) {
        (Value::Null, J::Null) => true,
        (Value::Null, J::String(s)) => s.is_empty(),
        (Value::Text(s), J::Null) | (Value::Date(s), J::Null) => s.is_empty(),
        (Value::Bool(a), J::Bool(b)) => a == b,
        (Value::Bool(a), J::Number(n)) => n.as_i64() == Some(*a as i64),
        (Value::Int(a), J::Number(n)) => n.as_f64() == Some(*a as f64),
        (Value::Float(a), J::Number(n)) => {
            let b = n.as_f64().unwrap();
            (a - b).abs() <= 1e-9 * a.abs().max(b.abs()).max(1.0)
        }
        (Value::Float(a), J::Null) => a.is_nan(),
        // Markup (KML descriptions) compared as its text: GDAL re-serialises it.
        (Value::Text(a), J::String(b)) if a.contains('<') => {
            let text = |s: &str| {
                let mut out = String::new();
                let mut tag = false;
                for c in s.chars() {
                    match c {
                        '<' => tag = true,
                        '>' => tag = false,
                        c if !tag && !c.is_whitespace() => out.push(c),
                        _ => {}
                    }
                }
                out
            };
            text(a) == text(b)
        }
        // A list (repeated GML property): numpy's "['a' 'b']" or "[6.1 7.1]" against
        // our "a, b".
        (Value::Text(a), J::String(b)) if b.starts_with('[') && b.ends_with(']') => {
            let inner = &b[1..b.len() - 1];
            let theirs: Vec<String> = if inner.contains('\'') {
                inner
                    .split('\'')
                    .skip(1)
                    .step_by(2)
                    .map(str::to_string)
                    .collect()
            } else {
                inner.split_whitespace().map(str::to_string).collect()
            };
            let ours: Vec<&str> = a.split(", ").collect();
            a == b
                || ours.len() == theirs.len()
                    && ours.iter().zip(&theirs).all(|(o, t)| {
                        o == t
                            || o.parse::<f64>()
                                .ok()
                                .zip(t.parse::<f64>().ok())
                                .is_some_and(|(x, y)| x == y)
                    })
        }
        (Value::Text(a), J::String(b)) => a.trim_end() == b.trim_end(),
        (Value::Date(a), J::String(b)) => {
            // Date-times compared without separators' style, trailing zero fractions or a
            // UTC zone.
            let n = |s: &str| {
                let s = s.replace(['/', 'T'], " ");
                let s = s.trim().trim_end_matches('Z');
                let (dt, tz) = match s.rfind(['+', '-']) {
                    Some(i) if i > 10 => (&s[..i], &s[i..]),
                    _ => (s, ""),
                };
                let tz = if tz == "+00:00" { "" } else { tz };
                let dt = match dt.split_once('.') {
                    Some((a, f)) => {
                        // To the microsecond (Python's precision).
                        let f = f.get(..6).unwrap_or(f).trim_end_matches('0');
                        if f.is_empty() {
                            a.to_string()
                        } else {
                            format!("{a}.{f}")
                        }
                    }
                    None => dt.to_string(),
                };
                let dt = dt
                    .trim_end_matches(" 00:00:00")
                    .trim_end_matches(":00")
                    .to_string();
                format!("{dt}{tz}")
            };
            n(a) == n(b)
        }
        (Value::Blob(a), J::Object(o)) => o.get("blob").and_then(J::as_u64) == Some(*a as u64),
        // OGR types numbers in text fields (GeoJSON mixed columns) as text.
        (Value::Int(a), J::String(b)) => b.trim() == a.to_string(),
        (Value::Float(a), J::String(b)) => b
            .trim()
            .parse::<f64>()
            .is_ok_and(|b| (a - b).abs() <= 1e-9 * a.abs().max(1.0)),
        (Value::Text(a), J::Number(n)) => a.trim().parse::<f64>().ok() == n.as_f64(),
        _ => false,
    }
}

/// Compares every file of the oracle's records with what the readers make of it.
pub fn compare(
    dir: &Path,
    records: Vec<J>,
    kind_of: impl Fn(&Path) -> Option<VectorKind>,
    skips: &Skips<'_>,
) -> Report {
    let mut by_file: BTreeMap<String, Vec<J>> = BTreeMap::new();
    for r in records {
        by_file
            .entry(r["file"].as_str().unwrap().to_string())
            .or_default()
            .push(r);
    }
    let (mut layers, mut features, mut vertices, mut crs_checked) = (0, 0, 0usize, 0);
    let mut mismatches: Vec<String> = Vec::new();
    for (file, recs) in &by_file {
        if skips.vector.iter().any(|(f, _)| f == file) {
            continue;
        }
        let path = dir.join(file);
        let gdal_failed = recs.iter().all(|r| r.get("error").is_some());
        let Some(kind) = kind_of(&path) else {
            if !gdal_failed {
                mismatches.push(format!("{file}: not recognised"));
            }
            continue;
        };
        let data = if path.is_dir() {
            Vec::new()
        } else {
            std::fs::read(&path).unwrap()
        };
        let ds = match if kind == VectorKind::GeoPackage {
            specular_gis::gpkg::read(&LocalDb::open(&path))
        } else {
            specular_gis::read(kind, &path, &data)
        } {
            Ok(ds) => ds,
            Err(e) => {
                if !gdal_failed {
                    mismatches.push(format!("{file}: {e}"));
                }
                continue;
            }
        };
        for r in recs {
            if r.get("error").is_some() {
                continue;
            }
            let lname = r["layer"].as_str().unwrap();
            if skips.layer.iter().any(|(l, _)| *l == lname) {
                continue;
            }
            let count = r["count"].as_u64().unwrap() as usize;
            let layer = ds
                .layers
                .iter()
                .find(|l| l.name == lname)
                .or_else(|| (ds.layers.len() == 1 && recs.len() == 1).then(|| &ds.layers[0]));
            let Some(layer) = layer else {
                if count > 0 {
                    let ours: Vec<_> = ds.layers.iter().map(|l| l.name.as_str()).collect();
                    mismatches.push(format!("{file}: no layer {lname:?} (ours {ours:?})"));
                }
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
            if let (Some(theirs), Some(c)) = (r["epsg"].as_u64(), &ds.crs) {
                crs_checked += 1;
                // GDAL calls GeoJSON with heights EPSG:4979 (3D WGS 84); the map is 4326.
                if c.epsg
                    .is_some_and(|e| e as u64 != theirs && !(e == 4326 && theirs == 4979))
                {
                    mismatches.push(format!("{file}: EPSG {:?}, OGR {theirs}", c.epsg));
                }
            }
            let ours_vertices: usize = layer
                .features
                .iter()
                .filter_map(|f| f.geometry.as_ref())
                .map(Geometry::vertex_count)
                .sum();
            vertices += ours_vertices;
            let fields: Vec<&str> = r["fields"]
                .as_array()
                .unwrap()
                .iter()
                .map(|f| f.as_str().unwrap())
                .collect();
            // Fields matched by name, ignoring case (KML "description" is our "Description").
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
            let mut bad = 0;
            for (i, f) in r["features"].as_array().unwrap().iter().enumerate() {
                let ours = &layer.features[i];
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
                if skips
                    .geometry
                    .iter()
                    .any(|(sf, si, _)| sf == file && *si == i)
                {
                    continue;
                }
                if let Some(theirs) = oracle_parts(&f["parts"]) {
                    let mut p = Vec::new();
                    if let Some(g) = &ours.geometry {
                        parts(g, &mut p);
                    }
                    if !same_parts(normalise(p.clone()), normalise(theirs.clone())) && bad < 3 {
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
    Report {
        files: by_file.len(),
        layers,
        features,
        vertices,
        crs_checked,
        mismatches,
    }
}
