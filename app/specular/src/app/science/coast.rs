//! The coordinate grid of a georeferenced science image (a GRIB or NetCDF field, a
//! GeoTIFF elevation model): coastlines (Natural Earth 1:110m) and a longitude/latitude
//! graticule, in stored pixel coordinates.

use specular_decode::geo::GeoRef;
use specular_decode::geo::specular_gis::coastline::coastlines;
use specular_decode::geo::specular_gis::crs::{Crs, CrsKind};

use super::GridLine;

/// Graticule spacings, degrees.
const STEPS: [f64; 8] = [0.5, 1.0, 2.0, 5.0, 10.0, 15.0, 30.0, 45.0];

/// Coastlines then graticule lines (with their labels) over a `w` × `h` image.
pub(super) fn geo_grid(g: &GeoRef, w: u32, h: u32) -> Vec<GridLine> {
    let Some(crs) = &g.crs else {
        return Vec::new();
    };
    let Some(inv) = g.to_map.inverse() else {
        return Vec::new();
    };
    let (fw, fh) = (f64::from(w), f64::from(h));
    // Longitudes repeat every 360 map units on lon/lat and rotated-pole grids.
    let periodic = matches!(crs.kind, CrsKind::Geographic)
        || crs.unit.as_ref().is_some_and(|u| u.name == "degree");
    let mapper = Mapper {
        crs,
        inv,
        periodic,
        bounds: g.bounds,
        jump: fw.max(fh) * 0.5,
        size: (fw, fh),
    };
    let mut out = Vec::new();
    for line in coastlines() {
        let pts: Vec<(f64, f64)> = line
            .iter()
            .map(|p| (f64::from(p[0]), f64::from(p[1])))
            .collect();
        mapper.emit(&pts, String::new(), &mut out);
    }

    // The extent in longitude and latitude, from points over the image.
    let (mut lat0, mut lat1, mut lon0, mut lon1) = (90f64, -90f64, 180f64, -180f64);
    let mut wraps = false;
    for j in 0..=16 {
        for i in 0..=16 {
            let (x, y) = g
                .to_map
                .apply(fw * f64::from(i) / 16.0, fh * f64::from(j) / 16.0);
            if let Some((lon, lat)) = crs.to_lonlat(x, y) {
                lat0 = lat0.min(lat);
                lat1 = lat1.max(lat);
                lon0 = lon0.min(lon);
                lon1 = lon1.max(lon);
                wraps |= !periodic && lon.abs() > 170.0;
            }
        }
    }
    if lat0 > lat1 {
        return out;
    }
    if wraps || lon1 - lon0 > 300.0 {
        (lon0, lon1) = (-180.0, 180.0);
    }
    let span = (lat1 - lat0).max((lon1 - lon0) * 0.5);
    let step = STEPS.into_iter().find(|s| span / s <= 8.0).unwrap_or(45.0);
    let dense = (step / 8.0).min(1.0);
    let first = |a: f64| (a / step).floor() * step;
    let mut lat = first(lat0);
    while lat <= lat1 + step {
        if lat.abs() < 90.0 {
            let n = ((lon1 - lon0 + 2.0 * step) / dense).ceil() as usize;
            let pts: Vec<(f64, f64)> = (0..=n)
                .map(|k| (first(lon0) - step + k as f64 * dense, lat))
                .collect();
            mapper.emit(&pts, lat_label(lat), &mut out);
        }
        lat += step;
    }
    let mut lon = first(lon0);
    while lon <= lon1 + step {
        let (a, b) = (
            (first(lat0) - step).max(-90.0),
            (first(lat1) + 2.0 * step).min(90.0),
        );
        let n = ((b - a) / dense).ceil() as usize;
        let pts: Vec<(f64, f64)> = (0..=n).map(|k| (lon, a + k as f64 * dense)).collect();
        mapper.emit(&pts, lon_label(lon), &mut out);
        lon += step;
    }
    out
}

struct Mapper<'a> {
    crs: &'a Crs,
    inv: specular_decode::geo::specular_gis::geotiff::Affine2,
    periodic: bool,
    bounds: [f64; 4],
    /// Segments longer than this (stored pixels) cross a projection's cut: split there.
    jump: f64,
    size: (f64, f64),
}

impl Mapper<'_> {
    /// Projects a lon/lat polyline and adds its pieces that reach the image (each copy 360°
    /// apart on periodic grids).
    fn emit(&self, pts: &[(f64, f64)], label: String, out: &mut Vec<GridLine>) {
        let mut map: Vec<Option<(f64, f64)>> = pts
            .iter()
            .map(|&(lon, lat)| self.crs.from_lonlat(lon, lat))
            .collect();
        let shifts: Vec<f64> = if self.periodic {
            // Continuous in x, then copied over the grid's longitudes.
            let mut prev: Option<f64> = None;
            for (x, _) in map.iter_mut().flatten() {
                if let Some(p) = prev {
                    *x -= ((*x - p) / 360.0).round() * 360.0;
                }
                prev = Some(*x);
            }
            let (lo, hi) = map
                .iter()
                .flatten()
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), p| {
                    (a.min(p.0), b.max(p.0))
                });
            (-3..=3)
                .map(|k| f64::from(k) * 360.0)
                .filter(|s| lo + s <= self.bounds[2] && hi + s >= self.bounds[0])
                .collect()
        } else {
            vec![0.0]
        };
        let (w, h) = self.size;
        for s in shifts {
            let mut piece: Vec<(f32, f32)> = Vec::new();
            let flush = |piece: &mut Vec<(f32, f32)>, out: &mut Vec<GridLine>| {
                let inside = piece.iter().any(|p| {
                    let (x, y) = (f64::from(p.0), f64::from(p.1));
                    x >= -w * 0.1 && x <= w * 1.1 && y >= -h * 0.1 && y <= h * 1.1
                });
                if piece.len() >= 2 && inside {
                    out.push(GridLine {
                        pts: std::mem::take(piece),
                        label: label.clone(),
                    });
                }
                piece.clear();
            };
            for p in &map {
                let Some((x, y)) = *p else {
                    flush(&mut piece, out);
                    continue;
                };
                let (px, py) = self.inv.apply(x + s, y);
                if let Some(&(qx, qy)) = piece.last()
                    && (px - f64::from(qx)).hypot(py - f64::from(qy)) > self.jump
                {
                    flush(&mut piece, out);
                }
                piece.push((px as f32, py as f32));
            }
            flush(&mut piece, out);
        }
    }
}

fn lat_label(lat: f64) -> String {
    match lat {
        0.0 => "0°".into(),
        l if l > 0.0 => format!("{}°N", trim(l)),
        l => format!("{}°S", trim(-l)),
    }
}

fn lon_label(lon: f64) -> String {
    let l = (lon + 180.0).rem_euclid(360.0) - 180.0;
    match l {
        0.0 => "0°".into(),
        -180.0 => "180°".into(),
        l if l > 0.0 => format!("{}°E", trim(l)),
        l => format!("{}°W", trim(-l)),
    }
}

fn trim(v: f64) -> String {
    specular_i18n::number(v, 1)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use specular_decode::geo::specular_gis::crs::from_epsg;
    use specular_decode::geo::specular_gis::geotiff::Affine2;
    use specular_decode::geo::{GeoFacts, RasterGeo};

    use super::*;

    fn lonlat(west: f64, step: f64, w: u32, h: u32) -> GeoRef {
        GeoRef {
            crs: from_epsg(4326),
            crs_note: None,
            to_map: Affine2 {
                a: step,
                b: 0.0,
                c: west,
                d: 0.0,
                e: -step,
                f: 90.0,
            },
            bounds: [
                west,
                90.0 - step * f64::from(h),
                west + step * f64::from(w),
                90.0,
            ],
            features: None,
            facts: GeoFacts::Raster(RasterGeo::default()),
        }
    }

    #[test]
    fn global_grids_both_conventions() {
        // 0..360 and −180..180 grids show the same coasts, one shifted half a turn.
        for west in [0.0, -180.0] {
            let g = Arc::new(lonlat(west, 2.5, 144, 72));
            let lines = geo_grid(&g, 144, 72);
            let coast: Vec<_> = lines.iter().filter(|l| l.label.is_empty()).collect();
            assert!(coast.len() > 100, "{west}: {}", coast.len());
            // Gibraltar (5.6°W, 36°N) lands where it should.
            let gx = ((-5.6 - west).rem_euclid(360.0) / 2.5) as f32;
            let gy = ((90.0 - 36.0) / 2.5) as f32;
            assert!(
                coast
                    .iter()
                    .flat_map(|l| &l.pts)
                    .any(|p| (p.0 - gx).abs() < 0.5 && (p.1 - gy).abs() < 0.5),
                "{west}"
            );
            let labels: Vec<_> = lines.iter().map(|l| l.label.as_str()).collect();
            assert!(
                labels.contains(&"30°N") && labels.contains(&"0°"),
                "{labels:?}"
            );
        }
    }

    #[test]
    fn labels() {
        assert_eq!(lat_label(-45.0), "45°S");
        assert_eq!(lon_label(190.0), "170°W");
        assert_eq!(lon_label(180.0), "180°");
        assert_eq!(lon_label(2.5), "2.5°E");
    }
}
