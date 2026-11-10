//! MapInfo TAB and MIF files against GDAL's "MapInfo File" driver (through pyogrio):
//! feature counts, vertices, EPSG codes, attributes and coordinates.
//!
//! The corpus comes from `tools/corpus/fetch-gis-formats.sh` and the expectations from
//! `gen/mitab_oracle.py` (`expected/mitab.jsonl`); it is read from
//! SPECULAR_GIS_FORMATS_CORPUS, else tools/corpus/gis-formats, and the test skips without it.

use serde_json::Value as J;
use specular_gis::mitab;
use specular_gis::{Geometry, Value};
use std::path::{Path, PathBuf};

fn corpus() -> Option<PathBuf> {
    let dir = std::env::var_os("SPECULAR_GIS_FORMATS_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/gis-formats")
        });
    if dir.join("expected/mitab.jsonl").is_file() {
        Some(dir)
    } else {
        eprintln!("no MapInfo oracle under {}; skipping", dir.display());
        None
    }
}

/// Files read differently by design, with the reason.
const SKIP: &[(&str, &str)] = &[
    (
        "seamless.tab",
        "seamless tables are refused (open the member tables)",
    ),
    (
        "view_first_table_second_table.tab",
        "views are refused (open the joined tables)",
    ),
    (
        "view_select_all_first_table_second_table.tab",
        "views are refused",
    ),
];

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
        (Value::Null, J::String(s)) => s.is_empty(),
        (Value::Text(s), J::Null) | (Value::Date(s), J::Null) => s.is_empty(),
        (Value::Bool(a), J::Bool(b)) => a == b,
        (Value::Int(a), J::Number(n)) => n.as_f64() == Some(*a as f64),
        (Value::Float(a), J::Number(n)) => {
            let b = n.as_f64().unwrap();
            (a - b).abs() <= 1e-9 * a.abs().max(b.abs()).max(1.0)
        }
        (Value::Text(a), J::String(b)) => a.trim_end() == b.trim_end(),
        (Value::Date(a), J::String(b)) => {
            let n = |s: &str| {
                s.replace(['/', 'T'], " ")
                    .trim_end_matches(['Z', '0', ':', '.', ' '])
                    .to_string()
            };
            n(a) == n(b)
        }
        _ => false,
    }
}

#[test]
fn mapinfo_matches_ogr() {
    let Some(dir) = corpus() else { return };
    let text = std::fs::read_to_string(dir.join("expected/mitab.jsonl")).unwrap();
    let (mut files, mut features, mut vertices, mut crs_checked) = (0, 0, 0usize, 0);
    let mut mismatches: Vec<String> = Vec::new();
    for line in text.lines() {
        let r: J = serde_json::from_str(line).unwrap();
        let file = r["file"].as_str().unwrap();
        let name = Path::new(file).file_name().unwrap().to_str().unwrap();
        if SKIP.iter().any(|(f, _)| *f == name) {
            continue;
        }
        let path = dir.join(file);
        let data = std::fs::read(&path).unwrap();
        let tab = name.to_ascii_lowercase().ends_with(".tab");
        let ds = if tab {
            mitab::read_tab(&path, &data)
        } else {
            mitab::read_mif(&path, &data)
        };
        let gdal_failed = r.get("error").is_some();
        let ds = match ds {
            Ok(ds) => ds,
            Err(e) => {
                if !gdal_failed {
                    mismatches.push(format!("{file}: {e}"));
                }
                continue;
            }
        };
        if gdal_failed {
            continue;
        }
        files += 1;
        let layer = &ds.layers[0];
        let count = r["count"].as_u64().unwrap() as usize;
        if layer.features.len() != count {
            mismatches.push(format!(
                "{file}: {} features, OGR {count}",
                layer.features.len()
            ));
            continue;
        }
        features += count;
        let ours_v: usize = layer
            .features
            .iter()
            .filter_map(|f| f.geometry.as_ref())
            .map(Geometry::vertex_count)
            .sum();
        vertices += ours_v;
        let theirs_v = r["vertices"].as_u64().unwrap() as usize;
        if ours_v != theirs_v {
            mismatches.push(format!("{file}: {ours_v} vertices, OGR {theirs_v}"));
        }
        if let (Some(theirs), Some(e)) = (r["epsg"].as_u64(), ds.crs.as_ref().and_then(|c| c.epsg))
        {
            crs_checked += 1;
            if u64::from(e) != theirs {
                mismatches.push(format!("{file}: EPSG {e}, OGR {theirs}"));
            }
        } else if r["epsg"].is_u64() {
            mismatches.push(format!(
                "{file}: no EPSG code ({:?}), OGR {}",
                ds.crs.as_ref().map(|c| &c.name),
                r["epsg"]
            ));
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
            .filter_map(|(t, n)| {
                layer
                    .fields()
                    .iter()
                    .position(|o| o.eq_ignore_ascii_case(n))
                    .map(|o| (o, t))
            })
            .collect();
        if map.len() != fields.len() {
            mismatches.push(format!(
                "{file}: fields {:?}, OGR {fields:?}",
                layer.fields()
            ));
        }
        let mut bad = 0;
        for (i, f) in r["features"].as_array().unwrap().iter().enumerate() {
            let ours = &layer.features[i];
            if ours.id.as_deref() != Some(&f["fid"].to_string()) && bad < 3 {
                bad += 1;
                mismatches.push(format!("{file} #{i}: fid {:?}, OGR {}", ours.id, f["fid"]));
            }
            let attrs = f["attrs"].as_array().unwrap();
            for &(o, t) in &map {
                let v = ours.values.get(o).unwrap_or(&Value::Null);
                if !same_value(v, &attrs[t]) && bad < 3 {
                    bad += 1;
                    mismatches.push(format!(
                        "{file} #{i} {}: {v:?}, OGR {}",
                        fields[t], attrs[t]
                    ));
                }
            }
            if let Some(theirs) = oracle_parts(&f["parts"]) {
                let mut p = Vec::new();
                if let Some(g) = &ours.geometry {
                    parts(g, &mut p);
                }
                if !same_parts(normalise(p.clone()), normalise(theirs.clone())) && bad < 3 {
                    bad += 1;
                    mismatches.push(format!(
                        "{file} #{i}: geometry {} parts / {} points ({:?}), OGR {} / {} ({:?})",
                        p.len(),
                        p.iter().map(Vec::len).sum::<usize>(),
                        p.first().and_then(|p| p.first()),
                        theirs.len(),
                        theirs.iter().map(Vec::len).sum::<usize>(),
                        theirs.first().and_then(|p| p.first()),
                    ));
                }
            }
        }
    }
    eprintln!(
        "{files} files, {features} features, {vertices} vertices, {crs_checked} CRSs; {} mismatches",
        mismatches.len()
    );
    for m in &mismatches {
        eprintln!("  {m}");
    }
    assert!(mismatches.is_empty(), "{} mismatches", mismatches.len());
}
