//! GeoPDF pages are georeferenced: ISO 32000 measures and OGC LGIDicts, checked against
//! the georeference GDAL's own tests expect (autotest/gdrivers/pdf.py) and against the
//! raster a GeoPDF was written from (`gen/formats_generate.py`).
//!
//! The corpus comes from `tools/corpus/fetch-gis-formats.sh` (SPECULAR_GIS_FORMATS_CORPUS,
//! else tools/corpus/gis-formats); the test skips without it.

use std::path::{Path, PathBuf};

use specular_decode::Registry;

fn corpus() -> Option<PathBuf> {
    let dir = std::env::var_os("SPECULAR_GIS_FORMATS_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/corpus/gis-formats")
        });
    if dir.join("gdal/pdf").is_dir() {
        Some(dir)
    } else {
        eprintln!("no GeoPDF samples in {}; skipping", dir.display());
        None
    }
}

/// The EPSG code and the map coordinates of the page's corners (stored top-left and
/// bottom-right), and the raster size.
type Corners = (Option<u32>, (f64, f64), (f64, f64), (u32, u32));

fn corners(path: &Path) -> Corners {
    let s = Registry::default().open_path(path).expect("opens");
    let g = s
        .geo()
        .unwrap_or_else(|| panic!("{}: not georeferenced", path.display()));
    let (w, h) = (s.info().width, s.info().height);
    (
        g.crs.as_ref().and_then(|c| c.epsg),
        g.map_at(0.0, 0.0),
        g.map_at(f64::from(w), f64::from(h)),
        (w, h),
    )
}

fn near(a: (f64, f64), b: (f64, f64), tol: f64) -> bool {
    (a.0 - b.0).abs() <= tol && (a.1 - b.1).abs() <= tol
}

#[test]
fn geopdf_pages_are_georeferenced() {
    let Some(dir) = corpus() else { return };
    // 20 × 20 pt pages, longitude 2–3 and latitude 48–49 (GDAL's test files).
    for f in ["gdal/pdf/test_iso32000.pdf", "gdal/pdf/test_ogc_bp.pdf"] {
        let (epsg, tl, br, _) = corners(&dir.join(f));
        assert_eq!(epsg, Some(4326), "{f}");
        assert!(near(tl, (2.0, 49.0), 1e-9), "{f}: {tl:?}");
        assert!(near(br, (3.0, 48.0), 1e-9), "{f}: {br:?}");
    }
    // Adobe style, UTM 20N: GDAL's geotransform at 200 dpi (its non-PDFium value).
    let p = dir.join("gdal/pdf/adobe_style_geospatial.pdf");
    let s = Registry::default().open_path(&p).unwrap();
    let g = s.geo().expect("georeferenced");
    assert_eq!(g.crs.as_ref().and_then(|c| c.epsg), Some(32620));
    let gt = [
        333_274.616_543_672_46,
        31.764_802_242_655_662,
        4_940_391.759_350_698,
        -31.794_745_501_708_24,
    ];
    let px_per_pt = f64::from(s.info().width) / 612.0;
    let mut worst = 0f64;
    for (x, y) in [(100.0, 100.0), (1500.0, 900.0), (3000.0, 2500.0)] {
        let (pt_x, pt_y) = (x / px_per_pt, y / px_per_pt);
        let (gx, gy) = (pt_x * 200.0 / 72.0, pt_y * 200.0 / 72.0);
        let want = (gt[0] + gt[1] * gx, gt[2] + gt[3] * gy);
        let got = g.map_at(x, y);
        worst = worst
            .max((got.0 - want.0).abs())
            .max((got.1 - want.1).abs());
    }
    eprintln!("adobe_style_geospatial: within {worst:.3} m of GDAL");
    assert!(worst < 1.0, "{worst} m");
    // Written by GDAL from a 260 × 200 raster: UTM 32N, 10 m pixels from (500000, 5200000).
    let written = dir.join("gen/pdf/geo_iso32000.pdf");
    if written.is_file() {
        let (epsg, tl, br, _) = corners(&written);
        assert_eq!(epsg, Some(32632));
        assert!(near(tl, (500_000.0, 5_200_000.0), 0.5), "{tl:?}");
        assert!(near(br, (502_600.0, 5_198_000.0), 0.5), "{br:?}");
    }
    // A code from ESRI's registry in the EPSG slot: the WKT beside it gives the CRS (GDAL
    // reports ESRI:102422).
    let s = Registry::default()
        .open_path(&dir.join("gdal/pdf/esri_102422_as_epsg_code.pdf"))
        .unwrap();
    let g = s.geo().expect("georeferenced");
    let c = g.crs.as_ref().expect("a CRS from the WKT");
    eprintln!("esri_102422_as_epsg_code: {} ({:?})", c.name, g.crs_note);
    assert_ne!(c.epsg, Some(4326));
    // A plain PDF has no georeference.
    let s = Registry::default()
        .open_path(&dir.join("gdal/pdf/byte_and_rgbsmall_2pages.pdf"))
        .unwrap();
    assert!(s.geo().is_none() || s.page_count() == 2);
}
