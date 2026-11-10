//! The geospatial corpus against the oracle (GDAL through pyogrio and rasterio, PROJ
//! through pyproj): feature counts, attributes and coordinates of every vector layer, and
//! the CRS, transform, nodata, overviews and longitude/latitude of every GeoTIFF.
//!
//! The corpus and its `expected/` files come from `tools/corpus/fetch-gis.sh`; it is read
//! from SPECULAR_GIS_CORPUS, else tools/corpus/gis, and the tests skip without it.

mod common;

use common::records;
use specular_gis::VectorKind;
use specular_gis::geotiff;
use std::path::{Path, PathBuf};

fn corpus() -> Option<PathBuf> {
    let dir = std::env::var_os("SPECULAR_GIS_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/gis"));
    if dir.join("expected").is_dir() {
        Some(dir)
    } else {
        eprintln!("no geospatial corpus at {}; skipping", dir.display());
        None
    }
}

/// Files GDAL reads differently by design, with the reason.
const VECTOR_SKIP: &[(&str, &str)] = &[(
    "gdal/ogr/geojson/stac_item.json",
    "not valid JSON (a trailing comma), which GDAL's parser lets through",
)];

/// Geometries GDAL reads differently by design: (file, feature index, reason).
const GEOMETRY_SKIP: &[(&str, usize, &str)] = &[
    (
        "gdal/ogr/kml/geometries.kml",
        9,
        "<Point/> without coordinates: GDAL makes POINT (0 0)",
    ),
    (
        "gdal/ogr/kml/geometries.kml",
        10,
        "empty <coordinates>: GDAL makes POINT (0 0)",
    ),
];

/// Layers GDAL makes that the viewer does not, by design.
const LAYER_SKIP: &[(&str, &str)] = &[
    ("route_points", "GPX: GDAL's per-point copy of the routes"),
    ("track_points", "GPX: GDAL's per-point copy of the tracks"),
    (
        "Ground Overlays",
        "KML: ground overlays are images, not drawn",
    ),
];

fn kind_of(path: &Path) -> Option<VectorKind> {
    let head = std::fs::read(path).ok()?;
    let ext = path.extension()?.to_str()?;
    specular_gis::kind_from_extension(ext)
        .or_else(|| specular_gis::sniff(&head[..head.len().min(4096)]))
        .or((ext == "json").then_some(VectorKind::GeoJson))
}

#[test]
fn vectors_match_ogr() {
    let Some(dir) = corpus() else { return };
    let r = common::compare(
        &dir,
        records(&dir, "vectors.jsonl"),
        kind_of,
        &common::Skips {
            vector: VECTOR_SKIP,
            geometry: GEOMETRY_SKIP,
            layer: LAYER_SKIP,
        },
    );
    r.print("vectors");
    assert!(r.mismatches.is_empty(), "{} mismatches", r.mismatches.len());
}

/// GeoTIFFs read differently by design, with the reason.
const RASTER_SKIP: &[(&str, &str)] = &[
    (
        "gdal/gcore/cfloat64.tif",
        "complex samples: not an image the viewer shows",
    ),
    // GeoTIFF 1.1 with a vertical CRS: GDAL reports the compound or 3D code, the viewer
    // the horizontal CRS (4326) and the vertical one beside it.
    (
        "gdal/gcore/epsg4326_3855_geotiff1_1.tif",
        "compound CRS (GDAL 9518)",
    ),
    (
        "gdal/gcore/epsg4326_5030_geotiff1_1.tif",
        "3D geographic CRS (GDAL 4979)",
    ),
    (
        "gdal/gcore/epsg4979_geotiff1_1.tif",
        "3D geographic CRS (GDAL 4979)",
    ),
    // User-defined Lambert conic keys GDAL matches to EPSG:26746 by searching the whole
    // EPSG database; the viewer shows the citation's name and projects with the keys.
    (
        "gdal/gcore/spaf27_correct.tif",
        "user-defined CRS identified by database search",
    ),
    (
        "gdal/gcore/spaf27_markedcorrect.tif",
        "user-defined CRS identified by database search",
    ),
];

#[test]
fn geotiffs_match_rasterio() {
    let Some(dir) = corpus() else { return };
    let (mut files, mut lonlat) = (0, 0);
    let mut mismatches: Vec<String> = Vec::new();
    for r in records(&dir, "rasters.jsonl") {
        let file = r["file"].as_str().unwrap();
        if RASTER_SKIP.iter().any(|(f, _)| *f == file) {
            continue;
        }
        let data = std::fs::read(dir.join(file)).unwrap();
        let Ok(tiff) = specular_tiff::Tiff::open(&data) else {
            mismatches.push(format!("{file}: not opened"));
            continue;
        };
        let Some(page) = tiff.pages().first() else {
            continue;
        };
        files += 1;
        let ifd = page.ifd();
        let size = (
            r["width"].as_u64().unwrap() as u32,
            r["height"].as_u64().unwrap() as u32,
        );
        if (ifd.width, ifd.height) != size {
            mismatches.push(format!(
                "{file}: {}x{}, rasterio {size:?}",
                ifd.width, ifd.height
            ));
        }
        let Some(g) = &ifd.geo else {
            if r["epsg"].is_number() {
                mismatches.push(format!("{file}: no GeoTIFF tags"));
            }
            continue;
        };
        let info = geotiff::interpret(
            &g.pixel_scale,
            &g.tiepoints,
            &g.transformation,
            &g.key_directory,
            &g.double_params,
            &g.ascii_params,
        );
        let t: Vec<f64> = r["transform"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_f64().unwrap())
            .collect();
        let identity = t == [1.0, 0.0, 0.0, 0.0, 1.0, 0.0];
        match info.transform {
            Some(m) => {
                let ours = [m.a, m.b, m.c, m.d, m.e, m.f];
                if !ours
                    .iter()
                    .zip(&t)
                    .all(|(a, b)| (a - b).abs() <= 1e-9 * a.abs().max(b.abs()).max(1.0))
                {
                    mismatches.push(format!("{file}: transform {ours:?}, rasterio {t:?}"));
                }
            }
            None if !identity && info.gcps == 0 => {
                mismatches.push(format!("{file}: no transform, rasterio {t:?}"));
            }
            None => {}
        }
        if let Some(theirs) = r["epsg"].as_u64() {
            match info.crs.as_ref().and_then(|c| c.epsg) {
                Some(e) if e as u64 != theirs => {
                    mismatches.push(format!("{file}: EPSG {e}, rasterio {theirs}"))
                }
                None => mismatches.push(format!("{file}: no EPSG code, rasterio {theirs}")),
                _ => {}
            }
        }
        let nodata = g.gdal_nodata.as_deref().and_then(geotiff::parse_nodata);
        match (nodata, r["nodata"].as_f64()) {
            (None, None) => {}
            // GDAL rounds the value to the band's type (float32 here).
            (Some(a), Some(b)) if a as f32 == b as f32 || (a.is_nan() && b.is_nan()) => {}
            (Some(a), None) if a.is_nan() => {}
            (a, b) => mismatches.push(format!("{file}: nodata {a:?}, rasterio {b:?}")),
        }
        // GDAL lists overviews in file order; ours are largest first.
        let mut overviews: Vec<u64> = r["overviews"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap())
            .collect();
        overviews.sort_unstable();
        let ours: Vec<u64> = page.level_sizes()[1..]
            .iter()
            .map(|&(w, _)| (ifd.width as f64 / w as f64).round() as u64)
            .collect();
        if ours != overviews {
            mismatches.push(format!(
                "{file}: overviews {ours:?}, rasterio {overviews:?}"
            ));
        }
        if let (Some(ll), Some(c), Some(crs)) =
            (r["lonlat"].as_array(), r["centre"].as_array(), &info.crs)
        {
            let (x, y) = (c[0].as_f64().unwrap(), c[1].as_f64().unwrap());
            if let Some((lon, lat)) = crs.to_lonlat(x, y) {
                lonlat += 1;
                let (tl, tb) = (ll[0].as_f64().unwrap(), ll[1].as_f64().unwrap());
                if (lon - tl).abs() > 1e-8 || (lat - tb).abs() > 1e-8 {
                    mismatches.push(format!(
                        "{file}: lon/lat ({lon}, {lat}), pyproj ({tl}, {tb})"
                    ));
                }
            }
        }
    }
    eprintln!(
        "{files} GeoTIFFs, {lonlat} lon/lat checks; {} mismatches",
        mismatches.len()
    );
    for m in &mismatches {
        eprintln!("  {m}");
    }
    assert!(mismatches.is_empty(), "{} mismatches", mismatches.len());
}
