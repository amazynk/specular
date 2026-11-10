//! ER Mapper, SAGA, Arc/Info grids, BSB/KAP charts, S-57 cells, TopoJSON, IGC and NMEA
//! against the oracle (`tools/corpus/gisfmt-oracle.py`: rasterio and pyogrio over GDAL,
//! pynmea2, and the generator's own values for IGC): every value of every band, transforms,
//! CRS codes, chart palettes and placement, every feature's geometry and the attributes
//! both name.
//!
//! The corpus comes from `tools/corpus/fetch-gisfmt.sh` plus the oracle; it is read from
//! SPECULAR_GISFMT_CORPUS, else $CARGO_TARGET_DIR/corpus/gisfmt, and the tests skip
//! without its `expected/` folder.

use serde_json::Value as J;
use specular_gis::model::{Geometry, Pt, Value};
use specular_gis::{aig, bsb, ers, saga};
use std::path::{Path, PathBuf};

fn corpus() -> Option<PathBuf> {
    let dir = std::env::var_os("SPECULAR_GISFMT_CORPUS")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("CARGO_TARGET_DIR").map(|t| PathBuf::from(t).join("corpus/gisfmt"))
        })?;
    if dir.join("expected").is_dir() {
        Some(dir)
    } else {
        eprintln!("no gisfmt oracle in {}; skipping", dir.display());
        None
    }
}

fn expected(dir: &Path, rel: &str) -> J {
    let p = dir.join("expected").join(format!("{rel}.json"));
    serde_json::from_slice(&std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display())))
        .unwrap()
}

fn band(dir: &Path, rel: &str, b: usize) -> Vec<f64> {
    let p = dir.join("expected").join(format!("{rel}.b{b}.f64"));
    std::fs::read(&p)
        .unwrap_or_else(|e| panic!("{}: {e}", p.display()))
        .chunks_exact(8)
        .map(|c| f64::from_le_bytes(c.try_into().unwrap()))
        .collect()
}

fn files(dir: &Path, sub: &str, ext: &str) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir.join(sub))
        .map(|r| r.flatten().map(|e| e.path()).collect())
        .unwrap_or_default();
    v.retain(|p| {
        p.extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case(ext))
    });
    v.sort();
    v
}

/// Compares our values (no data NaN) with GDAL's (no data as stored); returns mismatches.
fn compare(name: &str, ours: &[f32], gdal: &[f64], nodata: Option<f64>) -> Vec<String> {
    let mut bad = Vec::new();
    if ours.len() != gdal.len() {
        return vec![format!(
            "{name}: {} values, GDAL {}",
            ours.len(),
            gdal.len()
        )];
    }
    for (i, (&a, &b)) in ours.iter().zip(gdal).enumerate() {
        let gdal_nodata = nodata.is_some_and(|n| b == n || (n.is_nan() && b.is_nan()));
        let ok = if a.is_nan() {
            gdal_nodata || b.is_nan()
        } else {
            !gdal_nodata && f64::from(a) == (b as f32 as f64)
        };
        if !ok && bad.len() < 5 {
            bad.push(format!("{name}: value {i}: ours {a}, GDAL {b}"));
        }
    }
    bad
}

fn transform_matches(t: &specular_gis::geotiff::Affine2, gdal: &J) -> bool {
    let g: Vec<f64> = gdal
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap())
        .collect();
    let ours = [t.c, t.a, t.b, t.f, t.d, t.e];
    ours.iter()
        .zip(&g)
        .all(|(a, b)| (a - b).abs() <= 1e-9 * b.abs().max(1.0))
}

fn epsg(j: &J) -> Option<u32> {
    j.get("crs")?.get("epsg")?.as_u64().map(|e| e as u32)
}

fn rel(dir: &Path, p: &Path) -> String {
    p.strip_prefix(dir).unwrap().to_string_lossy().into_owned()
}

#[test]
fn ermapper_and_saga_match_gdal() {
    let Some(dir) = corpus() else { return };
    let mut bad = Vec::new();
    let mut values = 0usize;
    let mut n = 0;
    for p in files(&dir, "ers", "ers")
        .into_iter()
        .chain(files(&dir, "gen/ers", "ers"))
    {
        let r = rel(&dir, &p);
        let e = expected(&dir, &r);
        let h = ers::parse(&std::fs::read_to_string(&p).unwrap()).unwrap();
        let data = std::fs::read(ers::data_file(&p, &h).expect("data file")).unwrap();
        let l = h.layout;
        assert_eq!(
            (l.width, l.height, l.bands),
            (
                e["width"].as_u64().unwrap() as u32,
                e["height"].as_u64().unwrap() as u32,
                e["count"].as_u64().unwrap() as u32
            ),
            "{r}"
        );
        if !transform_matches(&h.transform.unwrap(), &e["transform"]) {
            bad.push(format!(
                "{r}: transform {:?} vs {}",
                h.transform, e["transform"]
            ));
        }
        if let Some(code) = epsg(&e)
            && h.crs.as_ref().and_then(|c| c.epsg) != Some(code)
        {
            bad.push(format!(
                "{r}: CRS {:?}, GDAL EPSG:{code}",
                h.crs.as_ref().map(|c| &c.name)
            ));
        }
        let nodata = e["nodata"][0].as_f64();
        for b in 0..l.bands {
            let (v, t) = l.band(&data, b, h.nodata);
            assert!(!t, "{r}");
            let g = band(&dir, &r, b as usize + 1);
            values += g.len();
            bad.extend(compare(&r, &v, &g, nodata));
        }
        n += 1;
    }
    for p in files(&dir, "saga", "sgrd")
        .into_iter()
        .chain(files(&dir, "gen/saga", "sgrd"))
        .chain(files(&dir, "saga", "sg-grd-z"))
    {
        let r = rel(&dir, &p);
        let e = expected(&dir, &r);
        let bytes = std::fs::read(&p).unwrap();
        let (h, data) = if saga::sniff_zip(&bytes) {
            let z = saga::read_zip(&bytes, 1 << 30).unwrap();
            (saga::parse(&z.header).unwrap(), z.data)
        } else {
            let h = saga::parse(&String::from_utf8_lossy(&bytes)).unwrap();
            (h, std::fs::read(p.with_extension("sdat")).unwrap())
        };
        assert_eq!(h.layout.width, e["width"].as_u64().unwrap() as u32, "{r}");
        if !transform_matches(&h.transform, &e["transform"]) {
            bad.push(format!(
                "{r}: transform {:?} vs {}",
                h.transform, e["transform"]
            ));
        }
        let (v, t) = h.values(&data);
        assert!(!t, "{r}");
        let g = band(&dir, &r, 1);
        values += g.len();
        bad.extend(compare(&r, &v, &g, e["nodata"][0].as_f64()));
        n += 1;
    }
    eprintln!(
        "{n} ER Mapper and SAGA files, {values} values, {} mismatches",
        bad.len()
    );
    assert!(n >= 8, "{n} files");
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

#[test]
fn arcinfo_grids_match_gdal() {
    let Some(dir) = corpus() else { return };
    let mut bad = Vec::new();
    let mut n = 0;
    for name in ["abc3x1", "ABC3X1UC", "teststa"] {
        let d = dir.join("aig").join(name);
        let r = format!("aig/{name}");
        let e = expected(&dir, &r);
        let file = |f: &str| {
            let p = specular_gis::sidecar::find_name(&d.join(f), f)?;
            std::fs::read(p).ok()
        };
        let clr = specular_gis::sidecar::find_name(&d.join("x"), &format!("{name}.clr"))
            .or_else(|| specular_gis::sidecar::find_name(&d, &format!("{name}.clr")))
            .map(|p| std::fs::read_to_string(p).unwrap());
        let g = aig::open(&file, clr.as_deref()).unwrap();
        assert_eq!(
            (g.width, g.height),
            (
                e["width"].as_u64().unwrap() as u32,
                e["height"].as_u64().unwrap() as u32
            ),
            "{r}"
        );
        if !transform_matches(&g.transform(), &e["transform"]) {
            bad.push(format!(
                "{r}: transform {:?} vs {}",
                g.transform(),
                e["transform"]
            ));
        }
        if let Some(code) = epsg(&e)
            && g.crs.as_ref().and_then(|c| c.epsg) != Some(code)
        {
            bad.push(format!(
                "{r}: CRS {:?}, GDAL EPSG:{code}",
                g.crs.as_ref().map(|c| &c.name)
            ));
        }
        let (v, t) = g.values(&file);
        assert!(!t, "{r}");
        bad.extend(compare(&r, &v, &band(&dir, &r, 1), e["nodata"][0].as_f64()));
        // The colour file: GDAL's colour table at each value.
        if let Some(cm) = e["colormap"].as_array() {
            for (val, c) in &g.palette {
                let want = &cm[*val as usize];
                let w: Vec<u64> = want
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|x| x.as_u64().unwrap())
                    .collect();
                if w[..3] != [u64::from(c[0]), u64::from(c[1]), u64::from(c[2])] {
                    bad.push(format!("{r}: colour of {val}: {c:?} vs {w:?}"));
                }
            }
        }
        n += 1;
    }
    eprintln!("{n} Arc/Info grids, {} mismatches", bad.len());
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

#[test]
fn bsb_charts_match_gdal() {
    let Some(dir) = corpus() else { return };
    let mut bad = Vec::new();
    let (mut n, mut pixels, mut placed) = (0, 0usize, 0);
    for p in files(&dir, "bsb", "kap") {
        let r = rel(&dir, &p);
        let e = expected(&dir, &r);
        let data = std::fs::read(&p).unwrap();
        let c = bsb::parse(&data).unwrap();
        let (ix, truncated) = c.indices(&data);
        if e.get("error").is_some() {
            // GDAL refuses the cut files; they read with their missing rows reported.
            assert!(truncated, "{r}");
            n += 1;
            continue;
        }
        assert_eq!(c.width, e["width"].as_u64().unwrap() as u32, "{r}");
        // GDAL's values count from 0 where the file's count from 1, its colour table too.
        let g = band(&dir, &r, 1);
        let ours: Vec<f32> = ix.iter().map(|&i| f32::from(i) - 1.0).collect();
        let cut = r.contains("truncated");
        if !cut {
            bad.extend(compare(&r, &ours, &g, None));
        }
        pixels += g.len();
        if let Some(cm) = e["colormap"].as_array() {
            for (i, want) in cm.iter().enumerate().take(c.palette.len() - 1) {
                let w: Vec<u64> = want
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|x| x.as_u64().unwrap())
                    .collect();
                let o = c.palette[i + 1];
                if w[..3] != [u64::from(o[0]), u64::from(o[1]), u64::from(o[2])] {
                    bad.push(format!("{r}: colour {i}: {o:?} vs {w:?}"));
                }
            }
        }
        // Placement: our fit at GDAL's points, in its own projection to within a hundredth
        // of a pixel's span, else (a projection read here as longitude and latitude) only
        // reported.
        if let (Some(ll), Some((t, crs, _, own))) = (e["lonlat"].as_array(), c.georef()) {
            let mut worst = 0.0f64;
            for q in ll {
                let q: Vec<f64> = q
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_f64().unwrap())
                    .collect();
                let (x, y) = t.apply(q[0], q[1]);
                let (lon, lat) = if crs.kind == specular_gis::crs::CrsKind::Geographic {
                    (x, y)
                } else {
                    crs.to_lonlat(x, y).unwrap()
                };
                worst = worst.max((lon - q[2]).abs().max((lat - q[3]).abs()));
            }
            let (px, _) = t.pixel_size();
            let span = if crs.kind == specular_gis::crs::CrsKind::Geographic {
                px.abs()
            } else {
                px.abs() / 111_000.0
            };
            eprintln!(
                "{r}: placed within {worst:.2e}° ({:.3} px), own projection {own}",
                worst / span
            );
            if own && worst > span * 0.05 {
                bad.push(format!("{r}: placed {worst:.2e}° from GDAL"));
            }
            placed += 1;
        }
        n += 1;
    }
    eprintln!(
        "{n} charts, {pixels} pixels, {placed} placed, {} mismatches",
        bad.len()
    );
    assert!(n >= 7);
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

/// GDAL's geometry: (type, nested coordinates).
fn coords(j: &J, out: &mut Vec<Pt>) {
    match j.as_array() {
        Some(a) if a.len() >= 2 && a[0].is_number() => {
            out.push([a[0].as_f64().unwrap(), a[1].as_f64().unwrap()])
        }
        Some(a) => a.iter().for_each(|x| coords(x, out)),
        None => {}
    }
}

fn points(g: &Geometry) -> Vec<Pt> {
    let mut v = Vec::new();
    g.for_each_pt(&mut |p| v.push(p));
    v
}

fn same_value(ours: &Value, gdal: &J) -> bool {
    match (ours, gdal) {
        (Value::Null, J::Null) => true,
        // GDAL lists an attribute given empty as an empty string or list.
        (Value::Null, J::String(b)) => b.is_empty(),
        (Value::Null, J::Array(b)) => b.iter().all(|x| x.as_str() == Some("")),
        // GDAL keeps 32-bit unsigned subfields (FIDN) in a signed 32-bit field.
        (Value::Int(a), J::Number(b)) => {
            b.as_f64() == Some(*a as f64) || b.as_f64() == Some(f64::from(*a as u32 as i32))
        }
        (Value::Float(a), J::Number(b)) => b.as_f64().is_some_and(|b| (a - b).abs() < 1e-9),
        (Value::Text(a), J::String(b)) => a.trim() == b.trim(),
        (Value::Text(a), J::Array(b)) => {
            let joined: Vec<String> = b
                .iter()
                .map(|x| x.as_str().map_or_else(|| x.to_string(), str::to_owned))
                .collect();
            *a == joined.join(",")
        }
        (Value::Text(a), J::Number(b)) => a.trim().parse::<f64>().ok() == b.as_f64(),
        (Value::Int(a), J::String(b)) => b.trim().parse::<i64>().ok() == Some(*a),
        (Value::Float(a), J::String(b)) => b.trim().parse::<f64>().ok() == Some(*a),
        _ => false,
    }
}

/// Compares every layer GDAL lists: feature counts, geometry types (ours without Z),
/// every vertex, and the attributes both have.
fn vectors_match(
    dir: &Path,
    sub: &str,
    ext: &str,
    read: impl Fn(&[u8]) -> specular_gis::Dataset,
) -> (usize, usize, Vec<String>) {
    let mut bad = Vec::new();
    let (mut n, mut vertices) = (0, 0);
    for p in files(dir, sub, ext) {
        let r = rel(dir, &p);
        let e = expected(dir, &r);
        if e.get("error").is_some() {
            continue;
        }
        let ds = read(&std::fs::read(&p).unwrap());
        // An S-57 update file read alone (GDAL builds what geometry it can from its partial
        // records): layers and feature counts only.
        let update = ds
            .warnings
            .iter()
            .any(|w| w.contains("update to a chart cell"));
        for el in e["layers"].as_array().unwrap() {
            let name = el["name"].as_str().unwrap();
            let Some(l) = ds.layers.iter().find(|l| l.name == name) else {
                bad.push(format!("{r}: no layer {name}"));
                continue;
            };
            let feats = el["features"].as_array().unwrap();
            if feats.len() != l.features.len() {
                bad.push(format!(
                    "{r}/{name}: {} features, GDAL {}",
                    l.features.len(),
                    feats.len()
                ));
                continue;
            }
            for (i, f) in feats.iter().enumerate() {
                let ours = &l.features[i];
                if update {
                    continue;
                }
                match (&ours.geometry, f["geometry"].as_array()) {
                    (None, None) => {}
                    (Some(g), Some(gj)) => {
                        let want_type = gj[0].as_str().unwrap();
                        if g.type_name() != want_type {
                            bad.push(format!("{r}/{name}/{i}: {} vs {want_type}", g.type_name()));
                        }
                        let mut want = Vec::new();
                        coords(&gj[1], &mut want);
                        let got = points(g);
                        vertices += got.len();
                        let same = got.len() == want.len()
                            && got.iter().zip(&want).all(|(a, b)| {
                                (a[0] - b[0]).abs() < 1e-9 && (a[1] - b[1]).abs() < 1e-9
                            });
                        // Rings may start at another vertex: compare as sets then.
                        let same = same
                            || (want_type.contains("Polygon")
                                && got.len() == want.len()
                                && want.iter().all(|w| {
                                    got.iter().any(|a| {
                                        (a[0] - w[0]).abs() < 1e-9 && (a[1] - w[1]).abs() < 1e-9
                                    })
                                }));
                        if !same && bad.len() < 50 {
                            bad.push(format!(
                                "{r}/{name}/{i}: {} vertices {:?}…, GDAL {} {:?}…",
                                got.len(),
                                got.first(),
                                want.len(),
                                want.first()
                            ));
                        }
                    }
                    (a, b) => bad.push(format!(
                        "{r}/{name}/{i}: geometry {:?} vs {:?}",
                        a.is_some(),
                        b.is_some()
                    )),
                }
                let attrs: std::collections::HashMap<String, Value> =
                    l.attributes(i).into_iter().collect();
                for (k, v) in f["fields"].as_object().unwrap() {
                    let Some(a) = attrs.get(k) else { continue };
                    if !same_value(a, v)
                        && !(v.is_null() && matches!(a, Value::Null))
                        && bad.len() < 50
                    {
                        bad.push(format!("{r}/{name}/{i}: {k} = {a:?}, GDAL {v}"));
                    }
                }
            }
        }
        n += 1;
    }
    (n, vertices, bad)
}

#[test]
fn s57_cells_match_gdal() {
    let Some(dir) = corpus() else { return };
    let (n, vertices, bad) =
        vectors_match(&dir, "s57", "000", |d| specular_gis::s57::read(d).unwrap());
    eprintln!(
        "{n} S-57 cells, {vertices} vertices, {} mismatches",
        bad.len()
    );
    assert!(n >= 4);
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

#[test]
fn topojson_matches_gdal() {
    let Some(dir) = corpus() else { return };
    // topojson1 is GDAL's robustness file (invalid members GDAL reads in its own way): it
    // must read, and its valid layer is compared.
    let (n, vertices, bad) = vectors_match(&dir, "topojson", "topojson", |d| {
        specular_gis::topojson::read(d).unwrap()
    });
    let bad: Vec<String> = bad
        .into_iter()
        .filter(|b| !b.starts_with("topojson/topojson1.topojson/TopoJSON"))
        .collect();
    eprintln!(
        "{n} TopoJSON files, {vertices} vertices, {} mismatches",
        bad.len()
    );
    assert!(n >= 4);
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

#[test]
fn igc_and_nmea_match_their_oracles() {
    let Some(dir) = corpus() else { return };
    let mut n = 0;
    for p in files(&dir, "gen/igc", "igc") {
        let r = rel(&dir, &p);
        let e = expected(&dir, &r);
        let f = specular_gis::igc::parse(&std::fs::read(&p).unwrap()).unwrap();
        let want = e["fixes"].as_array().unwrap();
        assert_eq!(f.fixes.len(), want.len(), "{r}");
        let t0 = f.fixes[0].t;
        for (x, w) in f.fixes.iter().zip(want) {
            let w: Vec<f64> = w
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_f64().unwrap())
                .collect();
            assert!(
                (x.lon - w[0]).abs() < 1e-9 && (x.lat - w[1]).abs() < 1e-9,
                "{r}"
            );
            assert_eq!(x.t - t0 + 4.0, w[2], "{r}");
            assert_eq!(x.gnss_alt, Some(w[3]), "{r}");
        }
        let pilot = f
            .headers
            .iter()
            .find(|(k, _)| k == "pilot")
            .map(|(_, v)| v.as_str());
        assert_eq!(pilot, e["pilot"].as_str(), "{r}");
        n += 1;
    }
    for p in files(&dir, "gen/nmea", "nmea") {
        let r = rel(&dir, &p);
        let e = expected(&dir, &r);
        let log = specular_gis::nmea::parse(&std::fs::read(&p).unwrap());
        let want = e["fixes"].as_array().unwrap();
        assert_eq!(log.fixes.len(), want.len(), "{r}");
        assert_eq!(log.bad, 1, "{r}");
        for (x, w) in log.fixes.iter().zip(want) {
            let w = w.as_array().unwrap();
            assert!((x.lon - w[0].as_f64().unwrap()).abs() < 1e-9, "{r}");
            assert!((x.lat - w[1].as_f64().unwrap()).abs() < 1e-9, "{r}");
            // pynmea2's time: "2024-03-14T23:58:01+00:00".
            let want_t = w[2].as_str().unwrap().replace("+00:00", "Z");
            assert_eq!(specular_gis::igc::iso(x.t.unwrap()), want_t, "{r}");
        }
        assert_eq!(
            log.waypoints.len(),
            e["waypoints"].as_u64().unwrap() as usize
        );
        n += 1;
    }
    eprintln!("{n} IGC and NMEA files");
    assert!(n >= 3);
}
