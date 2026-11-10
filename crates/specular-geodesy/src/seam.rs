//! Where a map's georeference plugs in. A georeferenced image implements
//! [`GeoTransform`] (and a DEM [`HeightSource`]) for its stored pixels; coordinates,
//! measuring and profiles then work on any CRS the implementation can convert. The
//! implementations here cover grids already in degrees.
//!
//! Longitude and latitude are in the CRS's own datum; datum shifts to WGS 84 belong to the
//! implementation.

use std::sync::Arc;

/// Stored pixel positions (continuous, (0, 0) the top-left corner, y down) of a
/// georeferenced image to and from longitude/latitude in degrees.
pub trait GeoTransform: Send + Sync {
    fn to_lonlat(&self, px: f64, py: f64) -> Option<(f64, f64)>;
    #[allow(clippy::wrong_self_convention)]
    fn from_lonlat(&self, lon: f64, lat: f64) -> Option<(f64, f64)>;
    /// The CRS's name for display ("WGS 84", "WGS 84 / UTM zone 33N").
    fn crs_name(&self) -> String;
}

/// Heights in metres at stored pixel positions: a DEM.
pub trait HeightSource: Send + Sync {
    fn height_at(&self, px: f64, py: f64) -> Option<f64>;
}

/// A grid whose stored pixels step linearly in degrees (SRTM tiles, science arrays with
/// regular latitude and longitude axes, a world file in degrees). `lon0`, `lat0` are the
/// outer edge of pixel (0, 0); `dlat` is usually negative (rows run south).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DegreeGrid {
    pub lon0: f64,
    pub dlon: f64,
    pub lat0: f64,
    pub dlat: f64,
    pub name: &'static str,
}

impl GeoTransform for DegreeGrid {
    fn to_lonlat(&self, px: f64, py: f64) -> Option<(f64, f64)> {
        let lon = self.lon0 + px * self.dlon;
        let lat = self.lat0 + py * self.dlat;
        (lon.is_finite() && lat.is_finite()).then_some((lon, lat))
    }

    fn from_lonlat(&self, lon: f64, lat: f64) -> Option<(f64, f64)> {
        if self.dlon == 0.0 || self.dlat == 0.0 {
            return None;
        }
        let px = (lon - self.lon0) / self.dlon;
        let py = (lat - self.lat0) / self.dlat;
        (px.is_finite() && py.is_finite()).then_some((px, py))
    }

    fn crs_name(&self) -> String {
        self.name.to_owned()
    }
}

/// A general affine from stored pixels to degrees: lon = a·px + b·py + c,
/// lat = d·px + e·py + f (vector maps already in longitude/latitude).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LonLatAffine {
    /// a, b, c, d, e, f.
    pub m: [f64; 6],
    pub name: &'static str,
}

impl GeoTransform for LonLatAffine {
    fn to_lonlat(&self, px: f64, py: f64) -> Option<(f64, f64)> {
        let [a, b, c, d, e, f] = self.m;
        let lon = a * px + b * py + c;
        let lat = d * px + e * py + f;
        (lon.is_finite() && lat.is_finite()).then_some((lon, lat))
    }

    fn from_lonlat(&self, lon: f64, lat: f64) -> Option<(f64, f64)> {
        let [a, b, c, d, e, f] = self.m;
        let det = a * e - b * d;
        if det == 0.0 || !det.is_finite() {
            return None;
        }
        let (x, y) = (lon - c, lat - f);
        let px = (e * x - b * y) / det;
        let py = (a * y - d * x) / det;
        (px.is_finite() && py.is_finite()).then_some((px, py))
    }

    fn crs_name(&self) -> String {
        self.name.to_owned()
    }
}

/// Heights from a row-major grid of `width × height` values (NaN for no data), one per
/// stored pixel, sampled bilinearly between pixel centres (at +0.5). Neighbours without data
/// drop out of the weights; a position with none of its four has no height.
#[derive(Clone, Debug)]
pub struct GridHeights {
    pub width: usize,
    pub height: usize,
    pub values: Arc<[f32]>,
}

impl HeightSource for GridHeights {
    fn height_at(&self, px: f64, py: f64) -> Option<f64> {
        let (w, h) = (self.width, self.height);
        if w == 0 || h == 0 || self.values.len() < w * h {
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
        let mut sum = 0.0;
        let mut wsum = 0.0;
        for (x, y, wt) in [
            (x0, y0, (1.0 - tx) * (1.0 - ty)),
            (x1, y0, tx * (1.0 - ty)),
            (x0, y1, (1.0 - tx) * ty),
            (x1, y1, tx * ty),
        ] {
            let v = self.values[y * w + x];
            if v.is_finite() && wt > 0.0 {
                sum += f64::from(v) * wt;
                wsum += wt;
            }
        }
        if wsum > 0.0 {
            Some(sum / wsum)
        } else {
            // Exactly on a pixel centre whose weight is everything.
            let v = self.values[y0 * w + x0];
            v.is_finite().then_some(f64::from(v))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grids_round_trip() {
        // An SRTM tile N47E011: 1201 samples per degree side, pixel-is-area edges.
        let step = 1.0 / 1200.0;
        let g = DegreeGrid {
            lon0: 11.0 - step / 2.0,
            dlon: step,
            lat0: 48.0 + step / 2.0,
            dlat: -step,
            name: "WGS 84",
        };
        let (lon, lat) = g.to_lonlat(600.5, 600.5).unwrap();
        assert!((lon - 11.5).abs() < 1e-12 && (lat - 47.5).abs() < 1e-12);
        let (px, py) = g.from_lonlat(lon, lat).unwrap();
        assert!((px - 600.5).abs() < 1e-9 && (py - 600.5).abs() < 1e-9);
        let a = LonLatAffine {
            m: [0.01, 0.002, 5.0, 0.001, -0.01, 50.0],
            name: "WGS 84",
        };
        let (lo, la) = a.to_lonlat(30.0, 40.0).unwrap();
        let (x, y) = a.from_lonlat(lo, la).unwrap();
        assert!((x - 30.0).abs() < 1e-9 && (y - 40.0).abs() < 1e-9);
        assert_eq!(a.crs_name(), "WGS 84");
    }

    #[test]
    fn bilinear_heights() {
        let g = GridHeights {
            width: 2,
            height: 2,
            values: Arc::from(vec![0.0f32, 10.0, 20.0, f32::NAN]),
        };
        assert_eq!(g.height_at(0.5, 0.5), Some(0.0));
        assert_eq!(g.height_at(1.0, 0.5), Some(5.0));
        // The missing corner drops out.
        assert_eq!(g.height_at(1.5, 1.5), None);
        assert!((g.height_at(1.0, 1.0).unwrap() - 10.0).abs() < 1e-9);
        assert_eq!(g.height_at(-0.1, 0.5), None);
        assert_eq!(g.height_at(2.0, 2.0), None);
    }
}
