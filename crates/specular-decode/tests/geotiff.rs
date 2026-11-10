//! GeoTIFF elevation models through the TIFF decoder: the georeference, the science view's
//! values with no-data as NaN, the auto-stretched grey with no-data transparent, the
//! overviews as levels; and a Shapefile with its sidecars on disk.

use std::sync::Arc;

use specular_core::{CancelToken, Source};
use specular_decode::geo::GeoFacts;
use specular_decode::geo::specular_gis::fixtures::{self, GeoGrid};
use specular_decode::{DecodeRequest, Format, Registry};

const GRID: GeoGrid = GeoGrid {
    epsg: 32633,
    origin: (500_000.0, 5_000_000.0),
    pixel: (10.0, 10.0),
};

#[test]
fn dem_is_georeferenced_values() {
    let (w, h) = (700, 500);
    let vals = fixtures::dem_values(w, h, true);
    let tif = fixtures::geotiff_f32(&vals, w, h, GRID, Some(-9999.0), 256);
    let s = Registry::default()
        .open(Arc::new(Source::from_vec("dem.tif", tif)))
        .expect("opens");
    assert_eq!(s.info().format, Format::GeoTiff);
    let g = s.geo().expect("georeferenced");
    assert_eq!(g.crs.as_ref().and_then(|c| c.epsg), Some(32633));
    assert_eq!(g.bounds, [500_000.0, 4_995_000.0, 507_000.0, 5_000_000.0]);
    match &g.facts {
        GeoFacts::Raster(r) => {
            assert!(r.values);
            assert_eq!(r.nodata, Some(-9999.0));
            assert_eq!(r.levels, vec![(700, 500), (350, 250), (175, 125)]);
        }
        f => panic!("{f:?}"),
    }
    let sci = s.science().expect("values");
    assert!(sci.geo.is_some());
    assert_eq!(sci.sample, "32-bit float");
    let plane = s
        .nd_plane(&[], &CancelToken::new())
        .expect("a plane")
        .expect("reads");
    assert_eq!((plane.width, plane.height), (700, 500));
    assert!(plane.values[0].is_nan());
    let i = 300 * w + 400;
    assert_eq!(plane.values[i], vals[i]);
    // The grey: no-data transparent, the rest opaque.
    let r = s
        .decode(&DecodeRequest::full(2), &CancelToken::new())
        .expect("decodes");
    assert_eq!((r.width, r.height), (350, 250));
    let px = &r.planes[0].data;
    assert_eq!(px[3], 0);
    let o = (200 * 350 + 300) * 4;
    assert_eq!(px[o + 3], 255);
}

#[test]
fn shapefile_with_sidecars() {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("gis-shp");
    std::fs::create_dir_all(&dir).unwrap();
    let feats = fixtures::parcels(4, 50.0, 3, (500_000.0, 5_000_000.0));
    let p = fixtures::shapefile_polygons(&feats, fixtures::UTM33N_WKT);
    std::fs::write(dir.join("parcels.shp"), &p.shp).unwrap();
    std::fs::write(dir.join("parcels.shx"), &p.shx).unwrap();
    std::fs::write(dir.join("parcels.dbf"), &p.dbf).unwrap();
    std::fs::write(dir.join("parcels.prj"), &p.prj).unwrap();
    let s = Registry::default()
        .open_path(&dir.join("parcels.shp"))
        .expect("opens");
    assert_eq!(s.info().format, Format::Shapefile);
    assert!(s.drawing().is_some());
    let g = s.geo().expect("georeferenced");
    assert_eq!(g.crs.as_ref().and_then(|c| c.epsg), Some(32633));
    let idx = g.features.as_ref().expect("features");
    let hit = idx.hit([500_020.0, 5_000_005.0], 0.1).expect("parcel 0");
    let a = idx.dataset.layers[hit.layer].attributes(hit.feature);
    assert!(
        a.iter().any(|(k, v)| k == "NAME" && v.text() == "Parcel 0"),
        "{a:?}"
    );
    // The metres per stored pixel: the map is 200 m across 4096 pixels less margins.
    let m = g.metres_per_pixel(100.0, 100.0).expect("metres");
    assert!(m > 0.04 && m < 0.06, "{m}");
}

/// A small photograph-like grid (the value histogram of GDAL's 20×20 `uint16.tif`, 74–255):
/// its stretched grey keeps the tones. The limits came from a 1/8 decode, nine values, whose
/// zscale fit sent 41 % of the picture to black and 11 % to white (the visual review).
#[test]
fn small_grid_limits_use_enough_samples() {
    let hist: [(f32, usize); 18] = [
        (74.0, 1),
        (90.0, 6),
        (99.0, 37),
        (107.0, 57),
        (115.0, 62),
        (123.0, 63),
        (132.0, 72),
        (140.0, 31),
        (148.0, 24),
        (156.0, 12),
        (165.0, 7),
        (173.0, 12),
        (181.0, 5),
        (189.0, 3),
        (197.0, 1),
        (206.0, 2),
        (247.0, 1),
        (255.0, 1),
    ];
    let mut vals: Vec<f32> = hist
        .iter()
        .flat_map(|&(v, n)| std::iter::repeat_n(v, n))
        .collect();
    assert_eq!(vals.len(), 397);
    vals.extend([123.0; 3]);
    // A smooth field: sorted values along a serpentine, as a photograph's tones run.
    let (w, h) = (20, 20);
    let mut grid = vec![0f32; w * h];
    for (i, v) in vals.iter().enumerate() {
        let (y, x) = (i / w, i % w);
        let x = if y % 2 == 1 { w - 1 - x } else { x };
        grid[y * w + x] = *v;
    }
    let tif = fixtures::geotiff_f32(&grid, w, h, GRID, None, 16);
    let s = Registry::default()
        .open(Arc::new(Source::from_vec("photo.tif", tif)))
        .expect("opens");
    let r = s
        .decode(&DecodeRequest::full(1), &CancelToken::new())
        .expect("decodes");
    let px = &r.planes[0].data;
    let n = (w * h) as f64;
    let black = px.chunks_exact(4).filter(|p| p[0] == 0).count() as f64 / n;
    let white = px.chunks_exact(4).filter(|p| p[0] == 255).count() as f64 / n;
    assert!(
        black <= 0.02 && white <= 0.02,
        "black {black}, white {white}"
    );
}
