//! Grid definitions: where each value of a field lies, and the north-up plane they make.
//!
//! The plane has column 0 at the west (or smallest x) and row 0 at the north (or largest
//! y), whatever the message's scanning mode. Reduced Gaussian rows are interpolated
//! linearly, with wrap-around on global grids, to the longest row's length.

use specular_gis::crs::{Crs, CrsKind, LinearUnit};
use specular_gis::geotiff::Affine2;
use specular_gis::proj::{Ellipsoid, Method, Projection};

/// The figure of the Earth a grid is defined on.
#[derive(Clone, Debug, PartialEq)]
pub struct Earth {
    /// Semi-major axis, metres.
    pub a: f64,
    /// Flattening (0 for a sphere).
    pub f: f64,
    /// "sphere, R = 6371229 m", "WGS 84".
    pub name: String,
    /// The datum is WGS 84 (shape 5, or the WGS 84 sphere of shape 8 is not).
    pub wgs84: bool,
}

impl Earth {
    pub fn sphere(r: f64) -> Earth {
        Earth {
            a: r,
            f: 0.0,
            name: format!("sphere, R = {} m", fmt_num(r)),
            wgs84: false,
        }
    }

    pub fn oblate(a: f64, b: f64, name: Option<&str>) -> Earth {
        Earth {
            a,
            f: if a > 0.0 { (a - b) / a } else { 0.0 },
            name: name.map_or_else(
                || format!("a = {} m, b = {} m", fmt_num(a), fmt_num(b)),
                str::to_owned,
            ),
            wgs84: false,
        }
    }

    pub fn ellipsoid(&self) -> Ellipsoid {
        Ellipsoid {
            a: self.a,
            f: self.f,
        }
    }
}

fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

/// A rotated pole (templates 3.1, 3.41; GRIB1 types 10, 14): the grid's south pole and the
/// rotation about the new axis, degrees.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rotation {
    pub pole_lat: f64,
    pub pole_lon: f64,
    pub angle: f64,
}

/// The kind of grid, in degrees and metres.
#[derive(Clone, Debug, PartialEq)]
pub enum GridKind {
    /// Equally spaced latitudes and longitudes (rotated ones with [`Grid::rotation`]).
    LatLon {
        lat1: f64,
        lon1: f64,
        lat2: f64,
        lon2: f64,
        /// Increments as stored, when given.
        di: Option<f64>,
        dj: Option<f64>,
    },
    /// Gaussian latitudes (`n` between a pole and the equator); reduced when
    /// [`Grid::pl`] is set.
    Gaussian {
        lat1: f64,
        lon1: f64,
        lat2: f64,
        lon2: f64,
        n: u32,
        di: Option<f64>,
    },
    PolarStereographic {
        lat1: f64,
        lon1: f64,
        /// The meridian pointing down from the pole (LoV).
        lov: f64,
        /// The latitude where `dx` and `dy` hold (LaD).
        lad: f64,
        dx: f64,
        dy: f64,
        south: bool,
    },
    Lambert {
        lat1: f64,
        lon1: f64,
        lov: f64,
        lad: f64,
        latin1: f64,
        latin2: f64,
        dx: f64,
        dy: f64,
        south: bool,
    },
}

/// A grid definition.
#[derive(Clone, Debug, PartialEq)]
pub struct Grid {
    pub kind: GridKind,
    pub rotation: Option<Rotation>,
    /// Points along a row (0 for a reduced grid).
    pub ni: u32,
    /// Rows.
    pub nj: u32,
    /// Points in each row of a reduced grid.
    pub pl: Option<Vec<u32>>,
    /// Scanning mode flags (0x80 −i, 0x40 +j, 0x20 j consecutive, 0x10 alternate rows).
    pub scan: u8,
    pub earth: Earth,
}

/// A field laid out north up.
#[derive(Clone, Debug, PartialEq)]
pub struct Plane {
    pub width: u32,
    pub height: u32,
    /// Rows top-down, NaN where missing.
    pub values: Vec<f32>,
}

/// Where the plane lies on the Earth.
#[derive(Clone, Debug)]
pub struct GridGeo {
    pub crs: Crs,
    /// Plane pixel position (continuous, (0, 0) the top-left corner of the top-left
    /// pixel) to map coordinates.
    pub to_map: Affine2,
    /// Min x, min y, max x, max y of the plane's pixel corners.
    pub bounds: [f64; 4],
    /// A caveat about the georeference ("Gaussian latitudes spaced evenly").
    pub note: Option<String>,
}

const I_NEG: u8 = 0x80;
const J_POS: u8 = 0x40;
const J_CONSECUTIVE: u8 = 0x20;
const BOUSTROPHEDON: u8 = 0x10;

impl Grid {
    /// Grid points.
    pub fn points(&self) -> usize {
        match &self.pl {
            Some(pl) => pl.iter().map(|&n| n as usize).sum(),
            None => self.ni as usize * self.nj as usize,
        }
    }

    /// The plane's size.
    pub fn plane_size(&self) -> (u32, u32) {
        match &self.pl {
            Some(pl) => (pl.iter().copied().max().unwrap_or(0), self.nj),
            None => (self.ni, self.nj),
        }
    }

    /// "regular lat/lon", "reduced Gaussian N32", ...
    pub fn name(&self) -> String {
        let rot = if self.rotation.is_some() {
            "rotated "
        } else {
            ""
        };
        match &self.kind {
            GridKind::LatLon { .. } => format!("{rot}regular lat/lon"),
            GridKind::Gaussian { n, .. } => format!(
                "{rot}{} Gaussian N{n}",
                if self.pl.is_some() {
                    "reduced"
                } else {
                    "regular"
                }
            ),
            GridKind::PolarStereographic { south, .. } => format!(
                "polar stereographic ({} pole)",
                if *south { "south" } else { "north" }
            ),
            GridKind::Lambert { .. } => "Lambert conformal".to_owned(),
        }
    }

    /// Spacing ("0.25° × 0.25°", "3 km × 3 km"), for the info panel.
    pub fn spacing(&self) -> Option<String> {
        match &self.kind {
            GridKind::LatLon { .. } => {
                let (_, _, di, dj) = self.latlon_frame();
                Some(format!("{}° × {}°", round6(di), round6(dj)))
            }
            GridKind::Gaussian { .. } => {
                let (_, _, di, _) = self.latlon_frame();
                Some(format!("{}° in longitude", round6(di)))
            }
            GridKind::PolarStereographic { dx, dy, .. } | GridKind::Lambert { dx, dy, .. } => {
                Some(format!("{} km × {} km", round6(dx / 1e3), round6(dy / 1e3)))
            }
        }
    }

    /// Position in the plane (column, row) of the field's point `k`, regular grids only.
    fn cell(&self, k: usize) -> (usize, usize) {
        let (ni, nj) = (self.ni as usize, self.nj as usize);
        let (mut i, mut j) = if self.scan & J_CONSECUTIVE == 0 {
            (k % ni, k / ni)
        } else {
            (k / nj, k % nj)
        };
        if self.scan & BOUSTROPHEDON != 0 {
            if self.scan & J_CONSECUTIVE == 0 {
                if j % 2 == 1 {
                    i = ni - 1 - i;
                }
            } else if i % 2 == 1 {
                j = nj - 1 - j;
            }
        }
        (self.col(i), self.row(j))
    }

    fn col(&self, i: usize) -> usize {
        if self.scan & I_NEG != 0 {
            self.ni as usize - 1 - i
        } else {
            i
        }
    }

    fn row(&self, j: usize) -> usize {
        if self.scan & J_POS != 0 {
            self.nj as usize - 1 - j
        } else {
            j
        }
    }

    /// The field's values (in point order) as the north-up plane.
    pub fn plane(&self, values: &[f64]) -> Plane {
        let (w, h) = self.plane_size();
        let (wu, hu) = (w as usize, h as usize);
        let mut out = vec![f32::NAN; wu * hu];
        match &self.pl {
            None => {
                if self.scan & (I_NEG | J_POS | J_CONSECUTIVE | BOUSTROPHEDON) == 0 {
                    for (o, &v) in out.iter_mut().zip(values) {
                        *o = v as f32;
                    }
                } else {
                    for (k, &v) in values.iter().enumerate().take(wu * hu) {
                        let (c, r) = self.cell(k);
                        out[r * wu + c] = v as f32;
                    }
                }
            }
            Some(pl) => {
                let global = self.reduced_global();
                let mut start = 0usize;
                for (j, &n) in pl.iter().enumerate() {
                    let n = n as usize;
                    let row = values.get(start..start + n).unwrap_or(&[]);
                    start += n;
                    let r = self.row(j);
                    let dst = &mut out[r * wu..(r + 1) * wu];
                    resample_row(row, dst, global, self.scan & I_NEG != 0);
                }
            }
        }
        Plane {
            width: w,
            height: h,
            values: out,
        }
    }

    /// Whether a reduced grid's rows go round the globe.
    fn reduced_global(&self) -> bool {
        let (lon1, lon2) = match self.kind {
            GridKind::Gaussian { lon1, lon2, .. } | GridKind::LatLon { lon1, lon2, .. } => {
                (lon1, lon2)
            }
            _ => return false,
        };
        let maxpl = self.plane_size().0.max(1);
        let span = (lon2 - lon1).rem_euclid(360.0);
        span + 360.0 / f64::from(maxpl) > 359.9
    }

    /// West longitude, north latitude, and the column and row spacing of the plane, degrees
    /// (rotated degrees on a rotated grid; the mean spacing of Gaussian rows).
    fn latlon_frame(&self) -> (f64, f64, f64, f64) {
        let (lat1, lon1, lat2, lon2, di, dj) = match self.kind {
            GridKind::LatLon {
                lat1,
                lon1,
                lat2,
                lon2,
                di,
                dj,
            } => (lat1, lon1, lat2, lon2, di, dj),
            GridKind::Gaussian {
                lat1,
                lon1,
                lat2,
                lon2,
                di,
                ..
            } => (lat1, lon1, lat2, lon2, di, None),
            _ => return (0.0, 0.0, 1.0, 1.0),
        };
        let (w, h) = self.plane_size();
        let ineg = self.scan & I_NEG != 0;
        let jpos = self.scan & J_POS != 0;
        let span = if self.pl.is_some() && self.reduced_global() {
            360.0
        } else {
            let s = if ineg { lon1 - lon2 } else { lon2 - lon1 };
            if s < 0.0 { s + 360.0 } else { s }
        };
        let di = if self.pl.is_some() {
            if self.reduced_global() {
                span / f64::from(w.max(1))
            } else {
                span / f64::from(w.max(2) - 1)
            }
        } else {
            match di {
                // A Gaussian grid's increment is rounded (2.813 for 2.8125): its end points
                // define the spacing, as in ecCodes.
                Some(d) if d > 0.0 && matches!(self.kind, GridKind::LatLon { .. }) => d,
                _ => span / f64::from(w.max(2) - 1),
            }
        };
        let lat_span = (lat2 - lat1).abs();
        let dj = match dj {
            Some(d) if d > 0.0 => d,
            _ => lat_span / f64::from(h.max(2) - 1),
        };
        let west = if ineg {
            lon1 - f64::from(w.saturating_sub(1)) * di
        } else {
            lon1
        };
        let north = if jpos { lat1.max(lat2) } else { lat1 };
        let north = if jpos {
            north
        } else {
            north.max(lat1 - f64::from(h.saturating_sub(1)) * dj)
        };
        (west, north, di, dj)
    }

    /// Latitudes of a Gaussian grid's rows, top-down in the plane.
    fn gaussian_rows(&self) -> Option<Vec<f64>> {
        let GridKind::Gaussian { lat1, lat2, n, .. } = self.kind else {
            return None;
        };
        if n == 0 || n > 8000 {
            return None;
        }
        let all = gaussian_latitudes(n as usize);
        let north = lat1.max(lat2);
        let first = all
            .iter()
            .enumerate()
            .min_by(|a, b| (a.1 - north).abs().total_cmp(&(b.1 - north).abs()))?
            .0;
        let rows = all.get(first..first + self.nj as usize)?;
        Some(rows.to_vec())
    }

    /// Longitude and latitude (degrees) of every point, in the field's point order.
    pub fn coordinates(&self) -> Vec<(f64, f64)> {
        let n = self.points();
        let mut out = Vec::with_capacity(n);
        let (w, h) = self.plane_size();
        let rows = self.gaussian_rows();
        let (west, north, di, dj) = self.latlon_frame();
        let lat_of_row = |r: usize| match &rows {
            Some(l) => l.get(r).copied().unwrap_or(f64::NAN),
            None => north - r as f64 * dj,
        };
        match &self.pl {
            Some(pl) => {
                let global = self.reduced_global();
                let span = if global {
                    360.0
                } else {
                    di * f64::from(w.max(2) - 1)
                };
                let ineg = self.scan & I_NEG != 0;
                for (j, &np) in pl.iter().enumerate() {
                    let lat = lat_of_row(self.row(j));
                    let step = if global {
                        span / f64::from(np.max(1))
                    } else {
                        span / f64::from(np.max(2) - 1)
                    };
                    for i in 0..np as usize {
                        let c = if ineg { np as usize - 1 - i } else { i };
                        out.push((west + c as f64 * step, lat));
                    }
                }
            }
            None => match &self.kind {
                GridKind::LatLon { .. } | GridKind::Gaussian { .. } => {
                    let proj = self.rotation.map(|r| self.rotated_projection(r));
                    for k in 0..n {
                        let (c, r) = self.cell(k);
                        let (x, y) = (west + c as f64 * di, lat_of_row(r));
                        out.push(match &proj {
                            Some(p) => {
                                let a = self.earth.a.to_radians();
                                p.inverse(x * a, y * a).unwrap_or((f64::NAN, f64::NAN))
                            }
                            None => (x, y),
                        });
                    }
                }
                _ => {
                    let Some((p, x0, y1, dx, dy)) = self.projected_frame() else {
                        return vec![(f64::NAN, f64::NAN); n];
                    };
                    for k in 0..n {
                        let (c, r) = self.cell(k);
                        let (x, y) = (x0 + c as f64 * dx, y1 - r as f64 * dy);
                        out.push(p.inverse(x, y).unwrap_or((f64::NAN, f64::NAN)));
                    }
                }
            },
        }
        let _ = h;
        out
    }

    fn rotated_projection(&self, r: Rotation) -> Projection {
        Projection::new(self.rotated_method(r), self.earth.ellipsoid())
    }

    fn rotated_method(&self, r: Rotation) -> Method {
        Method::RotatedPole {
            pole_lat: r.pole_lat,
            pole_lon: r.pole_lon,
            angle: r.angle,
        }
    }

    fn projected_method(&self) -> Option<Method> {
        Some(match self.kind {
            GridKind::PolarStereographic {
                lov, lad, south, ..
            } => Method::PolarStereographic {
                lat_ts: lad,
                lon0: lov,
                south,
                fe: 0.0,
                fn_: 0.0,
            },
            GridKind::Lambert {
                lov,
                latin1,
                latin2,
                ..
            } => Method::LambertConic {
                lat0: latin1,
                lon0: lov,
                lat1: latin1,
                lat2: latin2,
                k0: 1.0,
                fe: 0.0,
                fn_: 0.0,
            },
            _ => return None,
        })
    }

    /// A projected grid's projection, the x of the plane's first column, the y of its first
    /// row and the spacing, metres.
    fn projected_frame(&self) -> Option<(Projection, f64, f64, f64, f64)> {
        let (lat1, lon1, dx, dy) = match self.kind {
            GridKind::PolarStereographic {
                lat1, lon1, dx, dy, ..
            }
            | GridKind::Lambert {
                lat1, lon1, dx, dy, ..
            } => (lat1, lon1, dx, dy),
            _ => return None,
        };
        let p = Projection::new(self.projected_method()?, self.earth.ellipsoid());
        let (x1, y1) = p.forward(lon1, lat1)?;
        let (ni, nj) = (f64::from(self.ni.max(1) - 1), f64::from(self.nj.max(1) - 1));
        let x0 = if self.scan & I_NEG != 0 {
            x1 - ni * dx
        } else {
            x1
        };
        let ytop = if self.scan & J_POS != 0 {
            y1 + nj * dy
        } else {
            y1
        };
        Some((p, x0, ytop, dx, dy))
    }

    /// The plane's georeference.
    pub fn geo(&self) -> Option<GridGeo> {
        let (w, h) = self.plane_size();
        let ell = self.earth.ellipsoid();
        let (crs, to_map, note) = match &self.kind {
            GridKind::LatLon { .. } | GridKind::Gaussian { .. } => {
                let (west, north, di, dj) = self.latlon_frame();
                let mut note = None;
                let (top, step) = match self.gaussian_rows() {
                    Some(rows) if rows.len() >= 2 => {
                        note = Some("Gaussian latitudes spaced evenly".to_owned());
                        let step = (rows[0] - rows[rows.len() - 1]) / (rows.len() - 1) as f64;
                        (rows[0], step)
                    }
                    _ => (north, dj),
                };
                let to_map = Affine2 {
                    a: di,
                    b: 0.0,
                    c: west - di / 2.0,
                    d: 0.0,
                    e: -step,
                    f: top + step / 2.0,
                };
                let crs = match self.rotation {
                    Some(r) => {
                        let mut c = Crs::custom_projected(
                            format!(
                                "Rotated pole (south pole {}, {})",
                                lat_text(r.pole_lat),
                                lon_text(r.pole_lon)
                            ),
                            ell,
                            self.rotated_method(r),
                            LinearUnit {
                                name: "degree".into(),
                                metres: self.earth.a.to_radians(),
                            },
                        );
                        c.ellipsoid_name = Some(self.earth.name.clone());
                        c
                    }
                    None => self.geographic(),
                };
                (crs, to_map, note)
            }
            _ => {
                let (_, x0, ytop, dx, dy) = self.projected_frame()?;
                let mut crs = Crs::custom_projected(
                    self.name(),
                    ell,
                    self.projected_method()?,
                    LinearUnit::metre(),
                );
                crs.ellipsoid_name = Some(self.earth.name.clone());
                let to_map = Affine2 {
                    a: dx,
                    b: 0.0,
                    c: x0 - dx / 2.0,
                    d: 0.0,
                    e: -dy,
                    f: ytop + dy / 2.0,
                };
                (crs, to_map, None)
            }
        };
        let (x0, y0) = to_map.apply(0.0, 0.0);
        let (x1, y1) = to_map.apply(f64::from(w), f64::from(h));
        let bounds = [x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1)];
        let ok = [to_map.a, to_map.c, to_map.e, to_map.f]
            .iter()
            .all(|v| v.is_finite());
        ok.then_some(GridGeo {
            crs,
            to_map,
            bounds,
            note,
        })
    }

    fn geographic(&self) -> Crs {
        if self.earth.wgs84
            && let Some(c) = specular_gis::crs::from_epsg(4326)
        {
            return c;
        }
        let mut c = Crs::named(
            format!("Geographic ({})", self.earth.name),
            CrsKind::Geographic,
        );
        c.ellipsoid = self.earth.ellipsoid();
        c.ellipsoid_name = Some(self.earth.name.clone());
        c
    }
}

fn round6(v: f64) -> f64 {
    (v * 1e6).round() / 1e6
}

fn lat_text(v: f64) -> String {
    format!("{}°{}", round6(v.abs()), if v < 0.0 { 'S' } else { 'N' })
}

fn lon_text(v: f64) -> String {
    let v = (v + 180.0).rem_euclid(360.0) - 180.0;
    format!("{}°{}", round6(v.abs()), if v < 0.0 { 'W' } else { 'E' })
}

/// Linear interpolation of a reduced row onto `dst`'s regular points (periodic when
/// `global`), nearest where a neighbour is missing.
fn resample_row(src: &[f64], dst: &mut [f32], global: bool, reversed: bool) {
    let (n, w) = (src.len(), dst.len());
    if n == 0 || w == 0 {
        return;
    }
    let at = |i: usize| {
        if reversed { src[n - 1 - i] } else { src[i] }
    };
    if n == w {
        for (c, d) in dst.iter_mut().enumerate() {
            *d = at(c) as f32;
        }
        return;
    }
    for (c, d) in dst.iter_mut().enumerate() {
        let p = if global {
            c as f64 * n as f64 / w as f64
        } else if w > 1 {
            c as f64 * (n - 1) as f64 / (w - 1) as f64
        } else {
            0.0
        };
        let i0 = (p.floor() as usize).min(n - 1);
        let t = p - i0 as f64;
        let i1 = if global {
            (i0 + 1) % n
        } else {
            (i0 + 1).min(n - 1)
        };
        let (a, b) = (at(i0), at(i1));
        *d = if t == 0.0 {
            a
        } else if a.is_nan() || b.is_nan() {
            if t < 0.5 { a } else { b }
        } else {
            a + (b - a) * t
        } as f32;
    }
}

/// The 2N Gaussian latitudes (degrees), north to south: the zeros of the Legendre
/// polynomial of degree 2N, by Newton's method.
pub fn gaussian_latitudes(n: usize) -> Vec<f64> {
    let m = 2 * n;
    let mut out = vec![0.0; m];
    for k in 0..n {
        // Initial guess (Tricomi): cos(π (k + 0.75) / (m + 0.5)).
        let mut x = (std::f64::consts::PI * (k as f64 + 0.75) / (m as f64 + 0.5)).cos();
        for _ in 0..100 {
            let (mut p0, mut p1) = (1.0, x);
            for l in 2..=m {
                let p2 = ((2 * l - 1) as f64 * x * p1 - (l - 1) as f64 * p0) / l as f64;
                p0 = p1;
                p1 = p2;
            }
            // P'_m(x) = m (x P_m − P_{m−1}) / (x² − 1).
            let dp = m as f64 * (x * p1 - p0) / (x * x - 1.0);
            let dx = p1 / dp;
            x -= dx;
            if dx.abs() < 1e-15 {
                break;
            }
        }
        let lat = x.asin().to_degrees();
        out[k] = lat;
        out[m - 1 - k] = -lat;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gaussian_n32() {
        let l = gaussian_latitudes(32);
        assert_eq!(l.len(), 64);
        // ECMWF's N32 first latitude (as in the reduced_gg_pl_32 sample, 87.863799).
        assert!((l[0] - 87.863_799).abs() < 1e-6, "{}", l[0]);
        assert!((l[31] + l[32]).abs() < 1e-12);
    }

    #[test]
    fn row_resampling() {
        let mut d = [0f32; 8];
        resample_row(&[0.0, 4.0, 8.0, 4.0], &mut d, true, false);
        assert_eq!(d, [0.0, 2.0, 4.0, 6.0, 8.0, 6.0, 4.0, 2.0]);
        let mut d = [0f32; 5];
        resample_row(&[0.0, 4.0, 8.0], &mut d, false, false);
        assert_eq!(d, [0.0, 2.0, 4.0, 6.0, 8.0]);
    }
}
