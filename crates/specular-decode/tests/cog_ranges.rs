//! Cloud-Optimized GeoTIFFs read by byte ranges (`specular_gis::cog`): the level fetched
//! opens with GDAL's size, CRS and transform (rasterio, `gen/raster_oracle.py`), decodes to
//! the same pixels as the whole file, and an overview level keeps the georeference scaled.
//! Ranges and requests are counted.
//!
//! The corpus comes from `tools/corpus/fetch-gis-formats.sh` (SPECULAR_GIS_FORMATS_CORPUS,
//! else tools/corpus/gis-formats); the test skips without its `expected/rasters.jsonl`.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::Value as J;
use specular_core::{CancelToken, Source};
use specular_decode::geo::specular_gis::cog;
use specular_decode::geo::specular_gis::pmtiles::RangeRead;
use specular_decode::{DecodeRequest, Registry};

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

/// A file served by ranges, with the bytes asked for counted.
struct Served(Vec<u8>, AtomicU64);

impl RangeRead for Served {
    fn read_range(&self, offset: u64, len: u64) -> Result<Vec<u8>, String> {
        self.1.fetch_add(len, Ordering::Relaxed);
        let (o, l) = (offset as usize, len as usize);
        Ok(self.0[o.min(self.0.len())..(o + l).min(self.0.len())].to_vec())
    }
}

#[test]
fn cog_levels_match() {
    let Some(dir) = corpus() else { return };
    let text = std::fs::read_to_string(dir.join("expected/rasters.jsonl")).unwrap();
    let mut files = 0;
    for line in text.lines() {
        let r: J = serde_json::from_str(line).unwrap();
        let file = r["file"].as_str().unwrap();
        if !file.starts_with("gen/cog/") {
            continue;
        }
        files += 1;
        let data = std::fs::read(dir.join(file)).unwrap();
        let src = Served(data.clone(), AtomicU64::new(0));
        let t0 = std::time::Instant::now();
        let f = cog::fetch_level(&src, u64::MAX, u64::MAX, &mut |_| {}).unwrap();
        let ms = t0.elapsed().as_secs_f64() * 1e3;
        let (w, h) = (
            r["width"].as_u64().unwrap() as u32,
            r["height"].as_u64().unwrap() as u32,
        );
        assert_eq!((f.width, f.height), (w, h), "{file}");
        let fetched = Registry::default()
            .open(Arc::new(Source::from_vec(file, f.image.clone())))
            .unwrap();
        let whole = Registry::default().open_path(&dir.join(file)).unwrap();
        let g = fetched.geo().expect("georeferenced");
        assert_eq!(
            g.crs.as_ref().and_then(|c| c.epsg).map(u64::from),
            r["epsg"].as_u64(),
            "{file}"
        );
        let t: Vec<f64> = r["transform"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_f64().unwrap())
            .collect();
        let m = &g.to_map;
        for (a, b) in [m.a, m.b, m.c, m.d, m.e, m.f].iter().zip(&t) {
            assert!(
                (a - b).abs() <= 1e-9 * b.abs().max(1.0),
                "{file}: {a} vs {b}"
            );
        }
        // The same samples as the whole file's first page (the display stretch of a value
        // grid may differ: the whole file has overviews to take statistics from).
        let opts = specular_tiff::DecodeOptions {
            scale: 1,
            ..Default::default()
        };
        let ta = specular_tiff::Tiff::open(&f.image).unwrap();
        let tb = specular_tiff::Tiff::open(&data).unwrap();
        let a = ta.decode(&f.image, 0, &opts).unwrap();
        let b = tb.decode(&data, 0, &opts).unwrap();
        assert_eq!((a.width, a.height), (b.width, b.height), "{file}");
        assert!(a.data == b.data, "{file}: samples differ");
        let _ = whole
            .decode(&DecodeRequest::full(8), &CancelToken::new())
            .unwrap();
        // An overview: a quarter of the pixels at most.
        let o =
            cog::fetch_level(&src, u64::from(w) * u64::from(h) / 4, u64::MAX, &mut |_| {}).unwrap();
        assert!(o.level >= 1, "{file}");
        let ov = Registry::default()
            .open(Arc::new(Source::from_vec(file, o.image.clone())))
            .unwrap();
        let go = ov.geo().expect("overview georeferenced");
        let k = f64::from(w) / f64::from(o.width);
        assert!(
            (go.to_map.a - t[0] * k).abs() <= 1e-9 * t[0].abs() * k,
            "{file}"
        );
        assert!(
            (go.to_map.c - t[2]).abs() <= 1e-9 * t[2].abs().max(1.0),
            "{file}"
        );
        eprintln!(
            "{file}: {}×{} in {} requests, {} bytes ({:.1} ms, under load); overview {}×{} level {} in {} requests",
            f.width, f.height, f.requests, f.bytes_read, ms, o.width, o.height, o.level, o.requests
        );
    }
    assert!(files > 0, "no COG samples in the oracle");
}
