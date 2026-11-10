//! GeoTIFFs with user-defined projections written by GDAL, against PROJ's WGS 84 longitude
//! and latitude of sample pixels (`specular-gis/gen/geokeys_oracle.py` writes them into
//! SPECULAR_GEOKEYS_ORACLE; the test skips without it).

use std::sync::Arc;

use specular_core::Source;
use specular_decode::{Format, Registry};

#[test]
fn user_defined_geokeys_against_proj() {
    let Some(dir) = std::env::var_os("SPECULAR_GEOKEYS_ORACLE") else {
        eprintln!("SPECULAR_GEOKEYS_ORACLE not set; skipping");
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    let cases: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.join("expected.json")).unwrap()).unwrap();
    let (mut n, mut bad, mut worst) = (0, Vec::new(), 0.0f64);
    for c in cases.as_array().unwrap() {
        let file = c["file"].as_str().unwrap();
        let s = Registry::default()
            .open(Arc::new(Source::open(dir.join(file)).unwrap()))
            .unwrap();
        assert_eq!(s.info().format, Format::GeoTiff);
        let g = s.geo().unwrap();
        let crs = g.crs.as_ref().unwrap();
        for p in c["points"].as_array().unwrap() {
            let v: Vec<f64> = p
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_f64().unwrap())
                .collect();
            let (mx, my) = g.map_at(v[0], v[1]);
            n += 1;
            // The readout's numbers: on WGS 84, or on the file's own datum when it names
            // none (PROJ then shifts nothing either).
            let got = crs.to_wgs84(mx, my).or_else(|| crs.to_lonlat(mx, my));
            match got {
                Some((lon, lat)) => {
                    let d = (lon - v[4]).abs().max((lat - v[5]).abs());
                    worst = worst.max(d);
                    if d > 1e-7 || (mx - v[2]).abs() > 1e-6 || (my - v[3]).abs() > 1e-6 {
                        bad.push(format!(
                            "{file}: ({}, {}) -> ({lon}, {lat}), PROJ ({}, {}) [{}]",
                            v[0], v[1], v[4], v[5], crs.name
                        ));
                    }
                }
                None => bad.push(format!("{file}: no coordinates ({:?})", crs.method_name)),
            }
        }
    }
    eprintln!("{n} points, {} mismatches, worst {worst:.2e}°", bad.len());
    for b in &bad {
        eprintln!("  {b}");
    }
    assert!(bad.is_empty());
}
