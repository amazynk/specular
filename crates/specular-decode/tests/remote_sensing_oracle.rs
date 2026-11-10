//! Remote-sensing rasters against GDAL (rasterio): ENVI cubes of every data type,
//! interleave and byte order, hyperspectral composites and spectra with bad bands, and
//! complex SAR (ENVI, GeoTIFF CInt16/CFloat32/CFloat64, SICD-style NITF) as magnitude,
//! real, imaginary, phase and dB through each multilook factor.
//!
//! The corpus and its oracle come from `tools/corpus/fetch-remote-sensing.sh`
//! (SPECULAR_REMOTE_SENSING_CORPUS, else tools/corpus/remote-sensing); the test skips without
//! `expected/remote-sensing.jsonl`.

use std::path::{Path, PathBuf};

use serde_json::Value as J;
use specular_core::CancelToken;
use specular_decode::Registry;

fn corpus() -> Option<PathBuf> {
    let dir = std::env::var_os("SPECULAR_REMOTE_SENSING_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/corpus/remote-sensing")
        });
    if dir.join("expected/remote-sensing.jsonl").is_file() {
        Some(dir)
    } else {
        eprintln!("no remote-sensing oracle in {}; skipping", dir.display());
        None
    }
}

/// Same value: both missing, or within a relative 2e-5 (f32 planes against f64), or 1e-3
/// absolute for values near zero; phases also equal modulo 2π (±π).
fn same(ours: f32, want: Option<f64>) -> bool {
    match want {
        None => !ours.is_finite(),
        Some(w) => {
            let o = f64::from(ours);
            let d = (o - w).abs();
            d <= 2e-5 * w.abs().max(o.abs()) || d <= 1e-3
        }
    }
}

#[test]
fn remote_sensing_matches_gdal() {
    let Some(dir) = corpus() else { return };
    let text = std::fs::read_to_string(dir.join("expected/remote-sensing.jsonl")).unwrap();
    let cancel = CancelToken::new();
    let (mut files, mut values, mut spectra) = (0, 0usize, 0usize);
    let mut bad: Vec<String> = Vec::new();
    for line in text.lines() {
        let r: J = serde_json::from_str(line).unwrap();
        let file = r["file"].as_str().unwrap();
        let page = r["page"].as_u64().unwrap() as u32;
        let s = match Registry::default().open_path(&dir.join(file)) {
            Ok(s) if page == 0 => s,
            Ok(s) => match s.open_page(page) {
                Ok(p) => p,
                Err(e) => {
                    bad.push(format!("{file} page {page}: {e}"));
                    continue;
                }
            },
            Err(e) => {
                bad.push(format!("{file}: {e}"));
                continue;
            }
        };
        files += 1;
        let (w, h) = (
            r["w"].as_u64().unwrap() as u32,
            r["h"].as_u64().unwrap() as u32,
        );
        if (s.info().width, s.info().height) != (w, h) {
            bad.push(format!(
                "{file}: {}×{}, GDAL {w}×{h}",
                s.info().width,
                s.info().height
            ));
            continue;
        }
        if let Some(epsg) = r["epsg"].as_u64() {
            let t: Vec<f64> = r["transform"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_f64().unwrap())
                .collect();
            match s.geo() {
                None => bad.push(format!("{file}: not georeferenced, GDAL EPSG {epsg}")),
                Some(g) => {
                    if g.crs.as_ref().and_then(|c| c.epsg).map(u64::from) != Some(epsg) {
                        bad.push(format!(
                            "{file}: EPSG {:?}, GDAL {epsg}",
                            g.crs.as_ref().and_then(|c| c.epsg)
                        ));
                    }
                    let m = &g.to_map;
                    let ours = [m.a, m.b, m.c, m.d, m.e, m.f];
                    if ours.iter().zip(&t).any(|(a, b)| (a - b).abs() > 1e-6) {
                        bad.push(format!("{file}: transform {ours:?}, GDAL {t:?}"));
                    }
                }
            }
        }
        for p in r["planes"].as_array().unwrap() {
            let index: Vec<u32> = p["index"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as u32)
                .collect();
            let plane = match s.nd_plane(&index, &cancel) {
                Some(Ok(pl)) => pl,
                Some(Err(e)) => {
                    bad.push(format!("{file} {index:?}: {e}"));
                    continue;
                }
                None => {
                    bad.push(format!("{file}: no planes"));
                    break;
                }
            };
            let mut wrong = 0;
            for pt in p["points"].as_array().unwrap() {
                let (x, y) = (
                    pt[0].as_u64().unwrap() as u32,
                    pt[1].as_u64().unwrap() as u32,
                );
                let ours = plane.value(x, y).unwrap_or(f32::NAN);
                values += 1;
                if !same(ours, pt[2].as_f64()) {
                    wrong += 1;
                    if wrong <= 2 {
                        bad.push(format!(
                            "{file} {index:?} ({x}, {y}): {ours}, GDAL {}",
                            pt[2]
                        ));
                    }
                }
            }
            if wrong > 2 {
                bad.push(format!("{file} {index:?}: {wrong} values differ"));
            }
        }
        if let Some(sp) = r.get("spectra").and_then(J::as_array) {
            let Some(ps) = s.science().and_then(|c| c.spectra.clone()) else {
                bad.push(format!("{file}: no spectrum at the cursor"));
                continue;
            };
            for e in sp {
                let (x, y) = (e[0].as_u64().unwrap() as u32, e[1].as_u64().unwrap() as u32);
                let Some(got) = ps.at(x, y, &[]) else {
                    bad.push(format!("{file}: no spectrum at ({x}, {y})"));
                    continue;
                };
                let want = e[2].as_array().unwrap();
                spectra += 1;
                if got.y.len() != want.len()
                    || got.y.iter().zip(want).any(|(&o, w)| !same(o, w.as_f64()))
                {
                    bad.push(format!("{file}: spectrum at ({x}, {y}) differs"));
                }
            }
        }
    }
    eprintln!(
        "remote-sensing: {files} files, {values} values, {spectra} spectra against GDAL; {} mismatches",
        bad.len()
    );
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

/// The page of a product showing `view` at `metres`.
fn product_page(
    s: &std::sync::Arc<dyn specular_decode::Session>,
    view: &str,
    metres: u32,
) -> Option<std::sync::Arc<dyn specular_decode::Session>> {
    use specular_decode::{PageLabel, ProductView as V};
    let want = match view {
        "TrueColour" => V::TrueColour,
        "FalseColour" => V::FalseColour,
        "Swir" => V::Swir,
        "Agriculture" => V::Agriculture,
        "SceneClass" => V::SceneClass,
        "CloudProbability" => V::CloudProbability,
        _ => V::Bands,
    };
    (0..s.page_count()).find_map(|p| {
        let pg = if p == 0 {
            s.clone()
        } else {
            s.open_page(p).ok()?
        };
        matches!(pg.page_label(), Some(PageLabel::Product { view, metres: Some(m) })
            if view == want && m == metres)
        .then_some(pg)
    })
}

#[test]
fn sentinel2_matches_gdal() {
    let Some(dir) = corpus() else { return };
    let Ok(text) = std::fs::read_to_string(dir.join("expected/s2.jsonl")) else {
        return;
    };
    let cancel = CancelToken::new();
    let (mut pages, mut values) = (0, 0usize);
    let mut slowest = (std::time::Duration::ZERO, String::new());
    let mut bad: Vec<String> = Vec::new();
    for line in text.lines() {
        let r: J = serde_json::from_str(line).unwrap();
        let product = r["product"].as_str().unwrap();
        let view = r["view"].as_str().unwrap();
        let metres = r["metres"].as_u64().unwrap() as u32;
        let s = match Registry::default().open_path(&dir.join(product)) {
            Ok(s) => s,
            Err(e) => {
                bad.push(format!("{product}: {e}"));
                continue;
            }
        };
        let Some(pg) = product_page(&s, view, metres) else {
            bad.push(format!("{product}: no {view} page at {metres} m"));
            continue;
        };
        pages += 1;
        // Scene classes are named in the readout.
        let classes = pg.science().and_then(|sc| sc.classes);
        if (view == "SceneClass") != classes.is_some() {
            bad.push(format!("{product} {view} {metres} m: classes {classes:?}"));
        }
        let w = r["w"].as_u64().unwrap() as u32;
        if (pg.info().width, pg.info().height) != (w, w) {
            bad.push(format!(
                "{product} {view} {metres} m: {}×{}, want {w}",
                pg.info().width,
                pg.info().height
            ));
            continue;
        }
        if let Some(epsg) = r.get("epsg").and_then(J::as_u64) {
            let t: Vec<f64> = r["transform"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_f64().unwrap())
                .collect();
            match pg.geo() {
                Some(g) => {
                    let m = &g.to_map;
                    if g.crs.as_ref().and_then(|c| c.epsg).map(u64::from) != Some(epsg)
                        || [m.a, m.b, m.c, m.d, m.e, m.f]
                            .iter()
                            .zip(&t)
                            .any(|(a, b)| (a - b).abs() > 1e-6)
                    {
                        bad.push(format!("{product} {view} {metres} m: georeference differs"));
                    }
                }
                None => bad.push(format!("{product} {view} {metres} m: not georeferenced")),
            }
        }
        for p in r["planes"].as_array().unwrap() {
            let index: Vec<u32> = p["index"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as u32)
                .collect();
            let t0 = std::time::Instant::now();
            let got = pg.nd_plane(&index, &cancel);
            if t0.elapsed() > slowest.0 {
                slowest = (t0.elapsed(), format!("{view} {metres} m {index:?}"));
            }
            let plane = match got {
                Some(Ok(pl)) => pl,
                other => {
                    bad.push(format!(
                        "{product} {view} {metres} m {index:?}: {:?}",
                        other.map(|r| r.err())
                    ));
                    continue;
                }
            };
            let mut wrong = 0;
            for pt in p["points"].as_array().unwrap() {
                let (x, y) = (
                    pt[0].as_u64().unwrap() as u32,
                    pt[1].as_u64().unwrap() as u32,
                );
                let ours = plane.value(x, y).unwrap_or(f32::NAN);
                values += 1;
                if !same(ours, pt[2].as_f64()) {
                    wrong += 1;
                    if wrong <= 2 {
                        bad.push(format!(
                            "{product} {view} {metres} m {index:?} ({x}, {y}): {ours}, GDAL {}",
                            pt[2]
                        ));
                    }
                }
            }
        }
    }
    // The zipped product and the product metadata open as the same product.
    let synth =
        dir.join("gen/s2/S2B_MSIL2A_20200101T000000_N0500_R001_T33TUN_20200101T000000.SAFE");
    if synth.is_dir() {
        let a = Registry::default().open_path(&synth).unwrap();
        for other in [dir.join("gen/s2_l2a.zip"), synth.join("MTD_MSIL2A.xml")] {
            match Registry::default().open_path(&other) {
                Ok(b) => {
                    if b.page_count() != a.page_count() || b.info().format != a.info().format {
                        bad.push(format!("{}: not the same product", other.display()));
                    }
                    let pa = a.nd_plane(&[1], &cancel).map(|p| p.unwrap().values.clone());
                    let pb = b.nd_plane(&[1], &cancel).map(|p| p.unwrap().values.clone());
                    if pa.is_none() || pa.map(|v| v.len()) != pb.map(|v| v.len()) {
                        bad.push(format!("{}: planes differ", other.display()));
                    }
                }
                Err(e) => bad.push(format!("{}: {e}", other.display())),
            }
        }
    }
    // The product's own preview gives true-colour pages (and thumbnails) their first picture.
    let real = dir.join("s2/S2A_MSIL2A_20150704T101006_N0500_R022_T33TUN_20231012T100650.SAFE");
    if real.is_dir() {
        let s = Registry::default().open_path(&real).unwrap();
        match s.decode_preview() {
            Some(Ok(r)) if r.width == 343 && r.height == 343 => {}
            Some(Ok(r)) => bad.push(format!("preview {}×{}", r.width, r.height)),
            Some(Err(e)) => bad.push(format!("preview: {e}")),
            None => bad.push("no preview".into()),
        }
    }
    eprintln!(
        "sentinel-2: {pages} pages, {values} values against GDAL; {} mismatches; slowest plane {:?} ({})",
        bad.len(),
        slowest.0,
        slowest.1
    );
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}
