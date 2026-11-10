//! The EPSG table, projections and datum shifts against PROJ (pyproj), from the points
//! `gen/proj_oracle.py` writes. The file is read from SPECULAR_GIS_PROJ_ORACLE; the test
//! skips without it.
//!
//! - Projection: for every supported projected CRS, (lon, lat) on its own datum forward to
//!   (x, y) within 1 mm, and back within 1e-8° (about 1 mm).
//! - Datum shift: (lon, lat) on WGS 84 to the CRS's coordinates; PROJ picks the same
//!   Helmert where it has no grid, so most agree to millimetres. The rest is counted by
//!   size, not failed: PROJ uses grid files where installed and some operations
//!   specular-gis does not have (concatenated, time-dependent).

use specular_gis::crs::{self, CrsKind};

/// Metres of agreement asked for. EPSG:23700 (Hungary's EOV) is defined as Hotine oblique
/// Mercator (variant B), which specular-gis evaluates by EPSG's formulas; PROJ evaluates
/// it with the Swiss oblique Mercator formulas, 4–5 cm apart across Hungary.
fn tolerance(code: u32) -> f64 {
    match code {
        23700 => 0.06,
        _ => 1e-3,
    }
}

fn metres(c: &specular_gis::crs::Crs) -> f64 {
    c.unit.as_ref().map_or(1.0, |u| u.metres)
}

#[test]
fn against_proj() {
    let Some(path) = std::env::var_os("SPECULAR_GIS_PROJ_ORACLE") else {
        eprintln!("SPECULAR_GIS_PROJ_ORACLE not set; skipping");
        return;
    };
    let text = std::fs::read_to_string(path).unwrap();
    let mut codes = std::collections::BTreeSet::new();
    let (mut proj_n, mut proj_bad, mut proj_max) = (0usize, 0usize, 0.0f64);
    let mut inv_max = 0.0f64;
    let mut unsupported = std::collections::BTreeMap::<String, usize>::new();
    let mut missing = 0usize;
    // Datum shift errors (metres) in classes.
    let mut wgs = [0usize; 5]; // ≤ 1 mm, ≤ 1 cm, ≤ 1 m, ≤ 10 m, more
    let mut wgs_unknown = 0usize;
    let (mut ballpark, mut grid) = (0usize, 0usize);
    let mut worst: Vec<(f64, u32)> = Vec::new();
    let mut bad_examples = Vec::new();
    for line in text.lines() {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        let code = v["code"].as_u64().unwrap() as u32;
        let (lon, lat) = (v["lon"].as_f64().unwrap(), v["lat"].as_f64().unwrap());
        let Some(c) = crs::from_epsg(code) else {
            missing += 1;
            continue;
        };
        codes.insert(code);
        if c.kind == CrsKind::Projected && c.projection.is_none() {
            *unsupported
                .entry(c.method_name.clone().unwrap_or_default())
                .or_default() += 1;
            continue;
        }
        let m = if c.kind == CrsKind::Projected {
            metres(&c)
        } else {
            1.0
        };
        if let (Some(x), Some(y)) = (v["x"].as_f64(), v["y"].as_f64())
            && c.kind == CrsKind::Projected
        {
            proj_n += 1;
            match c.from_lonlat(lon, lat) {
                Some((ox, oy)) => {
                    let err = (ox - x).hypot(oy - y) * m;
                    proj_max = proj_max.max(err);
                    if err > tolerance(code) {
                        proj_bad += 1;
                        if bad_examples.len() < 30 {
                            bad_examples.push(format!(
                                "{code} {}: ({lon}, {lat}) -> ({ox}, {oy}), PROJ ({x}, {y}), {err:.4} m",
                                c.name
                            ));
                        }
                    }
                    if let Some((ilon, ilat)) = c.to_lonlat(x, y) {
                        // ±180° is one meridian.
                        let dl = ((ilon - lon + 180.0).rem_euclid(360.0) - 180.0).abs();
                        let d = dl.max((ilat - lat).abs());
                        inv_max = inv_max.max(d);
                        if d > 1e-8 * tolerance(code) / 1e-3 {
                            proj_bad += 1;
                            if bad_examples.len() < 30 {
                                bad_examples.push(format!(
                                    "{code} {}: inverse of ({x}, {y}) = ({ilon}, {ilat}), want ({lon}, {lat})",
                                    c.name
                                ));
                            }
                        }
                    }
                }
                None => {
                    proj_bad += 1;
                    if bad_examples.len() < 30 {
                        bad_examples
                            .push(format!("{code} {}: no forward at ({lon}, {lat})", c.name));
                    }
                }
            }
        }
        let op = v["op"].as_str().unwrap_or("");
        if op.starts_with("Ballpark") || op.contains("Ballpark geographic offset") {
            // PROJ has no transformation for this point (it shifts nothing).
            ballpark += 1;
        } else if v["grid"].as_bool() == Some(true) {
            grid += 1;
        } else if let (Some(wx), Some(wy)) = (v["wx"].as_f64(), v["wy"].as_f64()) {
            match c.from_wgs84(lon, lat) {
                None => wgs_unknown += 1,
                Some((ox, oy)) => {
                    let err = if c.kind == CrsKind::Geographic {
                        // Degrees to metres on the ground.
                        let k = 111_320.0;
                        ((ox - wx) * k * lat.to_radians().cos()).hypot((oy - wy) * k)
                    } else {
                        (ox - wx).hypot(oy - wy) * m
                    };
                    let class = match err {
                        e if e <= 1e-3 => 0,
                        e if e <= 1e-2 => 1,
                        e if e <= 1.0 => 2,
                        e if e <= 10.0 => 3,
                        _ => 4,
                    };
                    wgs[class] += 1;
                    if class >= 3 {
                        worst.push((err, code));
                    }
                }
            }
        }
    }
    worst.sort_by(|a, b| b.0.total_cmp(&a.0));
    worst.dedup_by_key(|w| w.1);
    eprintln!("codes {}; not in the table {missing} points", codes.len());
    eprintln!(
        "projection: {proj_n} points, {proj_bad} beyond 1 mm / 1e-8°, max {proj_max:.2e} m forward, {inv_max:.2e}° inverse"
    );
    eprintln!("unsupported methods (points): {unsupported:?}");
    eprintln!(
        "datum shift from WGS 84: ≤1 mm {}, ≤1 cm {}, ≤1 m {}, ≤10 m {}, >10 m {}; unknown datum {wgs_unknown}",
        wgs[0], wgs[1], wgs[2], wgs[3], wgs[4]
    );
    eprintln!(
        "not compared: PROJ without a transformation (ballpark) {ballpark}, PROJ with a grid {grid}"
    );
    eprintln!("worst datum codes: {:?}", &worst[..worst.len().min(25)]);
    for b in &bad_examples {
        eprintln!("  {b}");
    }
    assert_eq!(proj_bad, 0, "projection mismatches");
}
