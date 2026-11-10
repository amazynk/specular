//! NITF images against GDAL (rasterio, `crates/formats/specular-gis/gen/raster_oracle.py`):
//! size, CRS and transform from the IGEOLO corners, the RPC-fitted georeference, and pixel
//! values on a 16 × 16 grid (exact for uncompressed images, within 3 levels for JPEG and
//! JPEG 2000, whose decoders round differently).
//!
//! The corpus comes from `tools/corpus/fetch-gis-formats.sh` (SPECULAR_GIS_FORMATS_CORPUS,
//! else tools/corpus/gis-formats); the test skips without its `expected/rasters.jsonl`.

use std::path::{Path, PathBuf};

use serde_json::Value as J;
use specular_core::{CancelToken, PixelLayout};
use specular_decode::{DecodeError, DecodeRequest, Registry};

fn corpus() -> Option<PathBuf> {
    let dir = std::env::var_os("SPECULAR_GIS_FORMATS_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/corpus/gis-formats")
        });
    if dir.join("expected/rasters.jsonl").is_file() {
        Some(dir)
    } else {
        eprintln!("no raster oracle in {}; skipping", dir.display());
        None
    }
}

/// Files refused by design, with the words the refusal must contain.
const REFUSED: &[(&str, &str)] = &[("gdal/nitf/U_1050A.NTF", "bi-level")];

#[test]
fn nitf_matches_gdal() {
    let Some(dir) = corpus() else { return };
    let text = std::fs::read_to_string(dir.join("expected/rasters.jsonl")).unwrap();
    let (mut files, mut georef, mut samples, mut bad_samples) = (0, 0, 0usize, 0usize);
    let mut mismatches: Vec<String> = Vec::new();
    for line in text.lines() {
        let r: J = serde_json::from_str(line).unwrap();
        let file = r["file"].as_str().unwrap();
        if !file.contains("nitf/") {
            continue;
        }
        let path = dir.join(file);
        let opened = Registry::default().open_path(&path);
        if let Some((_, why)) = REFUSED.iter().find(|(f, _)| *f == file) {
            match opened {
                Err(DecodeError::Unsupported(m)) if m.contains(why) => {}
                Err(e) => mismatches.push(format!("{file}: refused as {e}")),
                Ok(_) => mismatches.push(format!("{file}: opened, expected a refusal")),
            }
            continue;
        }
        if r.get("error").is_some() {
            // GDAL cannot read it either; we must not panic.
            continue;
        }
        let s = match opened {
            Ok(s) => s,
            Err(e) => {
                mismatches.push(format!("{file}: {e}"));
                continue;
            }
        };
        files += 1;
        let (w, h) = (
            r["width"].as_u64().unwrap() as u32,
            r["height"].as_u64().unwrap() as u32,
        );
        if (s.info().width, s.info().height) != (w, h) {
            mismatches.push(format!(
                "{file}: {}×{}, GDAL {w}×{h}",
                s.info().width,
                s.info().height
            ));
            continue;
        }
        let t: Vec<f64> = r["transform"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_f64().unwrap())
            .collect();
        let g = s.geo();
        if let Some(epsg) = r["epsg"].as_u64() {
            georef += 1;
            match &g {
                None => mismatches.push(format!("{file}: not georeferenced, GDAL EPSG {epsg}")),
                Some(g) => {
                    if g.crs.as_ref().and_then(|c| c.epsg).map(u64::from) != Some(epsg) {
                        mismatches.push(format!(
                            "{file}: EPSG {:?}, GDAL {epsg}",
                            g.crs.as_ref().and_then(|c| c.epsg)
                        ));
                    }
                    let m = &g.to_map;
                    let ours = [m.a, m.b, m.c, m.d, m.e, m.f];
                    let tol = 1e-9 * (t[0].abs() + t[4].abs()).max(1e-6) * f64::from(w.max(h));
                    if ours
                        .iter()
                        .zip(&t)
                        .any(|(a, b)| (a - b).abs() > tol.max(1e-9))
                    {
                        mismatches.push(format!("{file}: transform {ours:?}, GDAL {t:?}"));
                    }
                }
            }
        } else if let Some(rpc) = r.get("rpc") {
            georef += 1;
            match &g {
                None => mismatches.push(format!("{file}: RPC not used")),
                Some(g) => {
                    // The model's offsets lie at the image centre in the generated sample.
                    let (lon, lat) = g.map_at(f64::from(w) / 2.0, f64::from(h) / 2.0);
                    let (lo, la) = (
                        rpc["long_off"].as_f64().unwrap(),
                        rpc["lat_off"].as_f64().unwrap(),
                    );
                    if (lon - lo).abs() > 1e-3 || (lat - la).abs() > 1e-3 {
                        mismatches.push(format!("{file}: RPC centre {lon} {lat}, model {lo} {la}"));
                    }
                }
            }
        } else if g.is_some() {
            mismatches.push(format!("{file}: georeferenced, GDAL is not"));
        }
        let raster = match s.decode(&DecodeRequest::full(1), &CancelToken::new()) {
            Ok(r) => r,
            Err(e) => {
                mismatches.push(format!("{file}: decode: {e}"));
                continue;
            }
        };
        let lossy = matches!(r["ic"].as_str(), Some("C3" | "M3" | "C8" | "M8"));
        let tol = if lossy { 3.0 } else { 0.0 };
        let abpp: u32 = r["abpp"]
            .as_str()
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(8);
        let xs: Vec<usize> = r["xs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap() as usize)
            .collect();
        let ys: Vec<usize> = r["ys"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap() as usize)
            .collect();
        let p = &raster.planes[0];
        let theirs = r["samples"].as_array().unwrap();
        let mut bad_here = 0;
        for (b, grid) in theirs.iter().enumerate() {
            for (j, row) in grid.as_array().unwrap().iter().enumerate() {
                for (i, v) in row.as_array().unwrap().iter().enumerate() {
                    let (x, y) = (xs[i], ys[j]);
                    let v = v.as_f64().unwrap();
                    let (ours, want) = match raster.layout {
                        // Fewer than 8 bits (bi-level masks) are shown over the full range.
                        PixelLayout::Gray if abpp < 8 => (
                            f64::from(p.data[y * p.stride + x]),
                            (v / (2f64.powi(abpp as i32) - 1.0) * 255.0).round(),
                        ),
                        PixelLayout::Gray => (f64::from(p.data[y * p.stride + x]), v),
                        PixelLayout::Gray16 => {
                            let o = y * p.stride + x * 2;
                            let ours = f64::from(u16::from_ne_bytes([p.data[o], p.data[o + 1]]));
                            (ours, (v / (2f64.powi(abpp as i32) - 1.0) * 65535.0).round())
                        }
                        _ => {
                            let ch = if theirs.len() == 1 { 0 } else { b };
                            // A colour table maps GDAL's indices to what is shown.
                            let want = match r.get("cmap").and_then(J::as_array) {
                                Some(c) => {
                                    c.get(v as usize).and_then(|e| e[ch].as_f64()).unwrap_or(v)
                                }
                                None => v,
                            };
                            (f64::from(p.data[y * p.stride + x * 4 + ch]), want)
                        }
                    };
                    samples += 1;
                    let scale = if raster.layout == PixelLayout::Gray16 {
                        257.0
                    } else {
                        1.0
                    };
                    if (ours - want).abs() > tol * scale + if scale > 1.0 { 1.0 } else { 0.0 } {
                        bad_samples += 1;
                        if bad_here < 3 {
                            bad_here += 1;
                            mismatches
                                .push(format!("{file} band {b} ({x}, {y}): {ours}, GDAL {want}"));
                        }
                    }
                }
            }
        }
    }
    eprintln!(
        "NITF: {files} images, {georef} georeferenced, {samples} samples, {bad_samples} off; {} mismatches",
        mismatches.len()
    );
    for m in &mismatches {
        eprintln!("  {m}");
    }
    assert!(mismatches.is_empty(), "{} mismatches", mismatches.len());
}
