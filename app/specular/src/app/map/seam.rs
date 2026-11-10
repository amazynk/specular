//! The map view's georeference: what turns the current image's stored pixels into longitude
//! and latitude ([`GeoTransform`]) and gives heights ([`HeightSource`]), for every kind of
//! georeferenced file there is today:
//!
//! - a session's [`GeoRef`] (vector maps in their CRS, GeoTIFF and other rasters, DTED, GRIB
//!   and NetCDF fields): its affine to map coordinates, then the CRS's inverse and datum
//!   shift to WGS 84;
//! - geolocation arrays of a science image (an HDF-EOS swath): longitude and latitude per
//!   pixel, no way back;
//! - a science image whose pixels step in degrees (SRTM tiles): [`DegreeGrid`].
//!
//! Better coordinate systems arrive through the CRS behind [`GeoRef`]; nothing here changes
//! for them.

use std::sync::Arc;

use specular_decode::geo::GeoRef;
use specular_decode::science::{LonLatGrid, NdPlane, PixelCalibration};
use specular_geodesy::seam::{DegreeGrid, GeoTransform, HeightSource};

/// A session's georeference.
pub(crate) struct GeoRefTransform(pub Arc<GeoRef>);

fn plausible((lon, lat): (f64, f64)) -> Option<(f64, f64)> {
    (lon.is_finite() && lat.is_finite() && lat.abs() <= 90.0 && lon.abs() <= 540.0)
        .then_some((lon, lat))
}

impl GeoTransform for GeoRefTransform {
    fn to_lonlat(&self, px: f64, py: f64) -> Option<(f64, f64)> {
        let g = &self.0;
        let crs = g.crs.as_ref()?;
        let (x, y) = g.map_at(px, py);
        // On WGS 84 where the datum's shift is known (geodesics and GPS positions are on
        // WGS 84), else on the CRS's own datum.
        crs.to_wgs84(x, y)
            .or_else(|| crs.to_lonlat(x, y))
            .and_then(plausible)
    }

    fn from_lonlat(&self, lon: f64, lat: f64) -> Option<(f64, f64)> {
        let g = &self.0;
        let crs = g.crs.as_ref()?;
        let (x, y) = crs
            .from_wgs84(lon, lat)
            .or_else(|| crs.from_lonlat(lon, lat))?;
        let (px, py) = g.to_map.inverse()?.apply(x, y);
        (px.is_finite() && py.is_finite()).then_some((px, py))
    }

    fn crs_name(&self) -> String {
        self.0
            .crs
            .as_ref()
            .map(|c| c.name.clone())
            .or_else(|| self.0.crs_note.clone())
            .unwrap_or_default()
    }
}

/// Geolocation arrays: longitude and latitude per pixel.
pub(crate) struct SwathTransform(pub Arc<LonLatGrid>);

impl GeoTransform for SwathTransform {
    fn to_lonlat(&self, px: f64, py: f64) -> Option<(f64, f64)> {
        self.0.at(px, py).and_then(plausible)
    }

    fn from_lonlat(&self, _lon: f64, _lat: f64) -> Option<(f64, f64)> {
        None
    }

    fn crs_name(&self) -> String {
        "WGS 84".to_owned()
    }
}

/// A science image's pixel calibration in degrees (SRTM): `None` for other units.
pub(crate) fn degree_grid(p: &PixelCalibration) -> Option<DegreeGrid> {
    let deg = |u: &str| matches!(u.trim(), "°" | "deg" | "degree" | "degrees");
    (deg(&p.unit_x) && deg(&p.unit_y) && p.x.step != 0.0 && p.y.step != 0.0).then_some(DegreeGrid {
        // The calibration gives pixel centres; the grid starts at the outer edge.
        lon0: p.x.offset - p.x.step * 0.5,
        dlon: p.x.step,
        lat0: p.y.offset - p.y.step * 0.5,
        dlat: p.y.step,
        name: "WGS 84",
    })
}

/// Heights from a plane of values in metres (scaled from feet when the unit says so),
/// bilinear between pixel centres; NaN is no data.
pub(crate) struct PlaneHeights {
    pub plane: Arc<NdPlane>,
    /// Metres per value unit.
    pub metres: f64,
}

impl HeightSource for PlaneHeights {
    fn height_at(&self, px: f64, py: f64) -> Option<f64> {
        let p = &self.plane;
        let (w, h) = (p.width as usize, p.height as usize);
        if w == 0 || h == 0 || p.values.len() < w * h {
            return None;
        }
        if !(px >= 0.0 && py >= 0.0 && px <= w as f64 && py <= h as f64) {
            return None;
        }
        let fx = (px - 0.5).clamp(0.0, (w - 1) as f64);
        let fy = (py - 0.5).clamp(0.0, (h - 1) as f64);
        let (x0, y0) = (fx.floor() as usize, fy.floor() as usize);
        let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
        let (tx, ty) = (fx - x0 as f64, fy - y0 as f64);
        let (mut sum, mut wsum) = (0.0, 0.0);
        for (x, y, wt) in [
            (x0, y0, (1.0 - tx) * (1.0 - ty)),
            (x1, y0, tx * (1.0 - ty)),
            (x0, y1, (1.0 - tx) * ty),
            (x1, y1, tx * ty),
        ] {
            let v = p.values[y * w + x];
            if v.is_finite() && wt > 0.0 {
                sum += f64::from(v) * wt;
                wsum += wt;
            }
        }
        let v = if wsum > 0.0 {
            sum / wsum
        } else {
            let v = p.values[y0 * w + x0];
            if !v.is_finite() {
                return None;
            }
            f64::from(v)
        };
        Some(v * self.metres)
    }
}

/// Metres per unit of a height unit ("m", "ft", "US survey foot"); `None` for one that is
/// not a length (a DEM in an unknown unit is still heights, in metres).
pub(crate) fn height_unit_metres(unit: Option<&str>) -> Option<f64> {
    let u = unit
        .map(|u| u.trim().to_ascii_lowercase())
        .unwrap_or_default();
    match u.as_str() {
        "" | "m" | "metre" | "metres" | "meter" | "meters" => Some(1.0),
        "ftus" => Some(1200.0 / 3937.0),
        u if u.contains("survey") => Some(1200.0 / 3937.0),
        u if u == "ft" || u.starts_with("foot") || u.starts_with("feet") => Some(0.3048),
        "km" => Some(1000.0),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_decode::geo::GeoFacts;
    use specular_decode::geo::specular_gis::crs::from_epsg;
    use specular_decode::geo::specular_gis::geotiff::Affine2;
    use specular_decode::science::Linear;

    fn georef(epsg: u32, to_map: Affine2) -> GeoRefTransform {
        GeoRefTransform(Arc::new(GeoRef {
            crs: from_epsg(epsg),
            crs_note: None,
            to_map,
            bounds: [0.0; 4],
            features: None,
            facts: GeoFacts::Raster(Default::default()),
        }))
    }

    #[test]
    fn georef_round_trips_in_utm_and_degrees() {
        let utm = georef(
            32633,
            Affine2 {
                a: 10.0,
                b: 0.0,
                c: 500_000.0,
                d: 0.0,
                e: -10.0,
                f: 5_000_000.0,
            },
        );
        let (lon, lat) = utm.to_lonlat(0.0, 0.0).unwrap();
        assert!((lon - 15.0).abs() < 1e-9, "{lon}");
        let (px, py) = utm.from_lonlat(lon, lat).unwrap();
        assert!(px.abs() < 1e-6 && py.abs() < 1e-6, "{px} {py}");
        assert!(utm.crs_name().contains("UTM"));
        let ll = georef(
            4326,
            Affine2 {
                a: 0.01,
                b: 0.0,
                c: 8.0,
                d: 0.0,
                e: -0.01,
                f: 47.0,
            },
        );
        assert_eq!(ll.to_lonlat(100.0, 100.0), Some((9.0, 46.0)));
        let (px, py) = ll.from_lonlat(9.0, 46.0).unwrap();
        assert!((px - 100.0).abs() < 1e-9 && (py - 100.0).abs() < 1e-9);
    }

    #[test]
    fn srtm_calibration_is_a_degree_grid() {
        // N47E011, 3″: the first centre on the tile's west and north edges.
        let step = 1.0 / 1200.0;
        let p = PixelCalibration {
            x: Linear { offset: 11.0, step },
            y: Linear {
                offset: 48.0,
                step: -step,
            },
            unit_x: "°".into(),
            unit_y: "°".into(),
        };
        let g = degree_grid(&p).unwrap();
        let (lon, lat) = g.to_lonlat(0.5, 0.5).unwrap();
        assert!((lon - 11.0).abs() < 1e-12 && (lat - 48.0).abs() < 1e-12);
        let mm = PixelCalibration {
            unit_x: "mm".into(),
            ..p
        };
        assert!(degree_grid(&mm).is_none());
    }

    #[test]
    fn plane_heights_interpolate_and_skip_no_data() {
        let plane = Arc::new(NdPlane {
            width: 2,
            height: 2,
            values: vec![0.0, 10.0, 20.0, f32::NAN],
            truncated: false,
        });
        let h = PlaneHeights { plane, metres: 1.0 };
        assert_eq!(h.height_at(0.5, 0.5), Some(0.0));
        assert_eq!(h.height_at(1.0, 0.5), Some(5.0));
        // Between three known values and the hole: the three weigh in.
        let v = h.height_at(1.0, 1.0).unwrap();
        assert!((v - 10.0).abs() < 1e-9, "{v}");
        assert_eq!(h.height_at(1.5, 1.5), None);
        assert_eq!(h.height_at(-1.0, 0.0), None);
        assert_eq!(height_unit_metres(Some("ft")), Some(0.3048));
        assert_eq!(height_unit_metres(Some("kelvin")), None);
    }
}
