//! Bounding volumes in earth-fixed (or the tileset's own) coordinates: oriented boxes and
//! spheres. A `region` (longitudes, latitudes, heights on WGS84) becomes the box that holds
//! a 9 × 9 grid of its points at both heights, with the edges' midpoints, in the
//! east-north-up frame at its centre (the oracle builds it the same way).

use serde_json::Value;

use crate::math::{self, M4, V3, add, cross, dot, len, scale, sub, unit};

/// Grid lines across a region per direction for its box.
const REGION_GRID: usize = 9;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Volume {
    /// Centre and three half-axes.
    Box {
        c: V3,
        axes: [V3; 3],
    },
    Sphere {
        c: V3,
        r: f64,
    },
}

/// A volume as the tileset gives it (before the tile's transform), kept for implicit
/// subdivision.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Given {
    Box([f64; 12]),
    /// West, south, east, north (radians), lowest and highest height (metres).
    Region([f64; 6]),
    Sphere([f64; 4]),
}

impl Given {
    /// Reads a `boundingVolume` object.
    pub fn parse(v: &Value) -> Option<Given> {
        let nums = |k: &str, n: usize| -> Option<Vec<f64>> {
            let a = v.get(k)?.as_array()?;
            let out: Vec<f64> = a.iter().filter_map(Value::as_f64).collect();
            (out.len() == n && a.len() == n && out.iter().all(|x| x.is_finite())).then_some(out)
        };
        if let Some(b) = nums("box", 12) {
            let mut a = [0.0; 12];
            a.copy_from_slice(&b);
            return Some(Given::Box(a));
        }
        if let Some(r) = nums("region", 6) {
            let mut a = [0.0; 6];
            a.copy_from_slice(&r);
            return Some(Given::Region(a));
        }
        if let Some(s) = nums("sphere", 4)
            && s[3] >= 0.0
        {
            return Some(Given::Sphere([s[0], s[1], s[2], s[3]]));
        }
        None
    }

    /// In world coordinates under `m` (regions are already earth-fixed).
    pub fn world(&self, m: &M4) -> Volume {
        match *self {
            Given::Box(b) => Volume::Box {
                c: math::point(m, [b[0], b[1], b[2]]),
                axes: [
                    math::vector(m, [b[3], b[4], b[5]]),
                    math::vector(m, [b[6], b[7], b[8]]),
                    math::vector(m, [b[9], b[10], b[11]]),
                ],
            },
            Given::Sphere(s) => Volume::Sphere {
                c: math::point(m, [s[0], s[1], s[2]]),
                r: s[3] * math::max_scale(m),
            },
            Given::Region(r) => region_box(r),
        }
    }

    /// The `i`-th of `2^level` parts along each split axis (implicit tiling): `xyz` the
    /// tile's coordinates, `octree` whether z (height) is split too. Spheres don't split.
    pub fn part(&self, level: u32, xyz: [u64; 3], octree: bool) -> Option<Given> {
        let n = 2f64.powi(level.min(62) as i32);
        let frac = |i: u64| (i as f64 + 0.5) / n;
        match *self {
            Given::Box(b) => {
                let mut out = b;
                let split = [true, true, octree];
                for (k, &s) in split.iter().enumerate() {
                    if !s {
                        continue;
                    }
                    let f = -1.0 + 2.0 * frac(xyz[k]);
                    for d in 0..3 {
                        out[d] += b[3 + 3 * k + d] * f;
                        out[3 + 3 * k + d] = b[3 + 3 * k + d] / n;
                    }
                }
                Some(Given::Box(out))
            }
            Given::Region(r) => {
                let east = if r[2] < r[0] {
                    r[2] + std::f64::consts::TAU
                } else {
                    r[2]
                };
                let lerp = |a: f64, b: f64, i: u64| {
                    let t0 = i as f64 / n;
                    let t1 = (i + 1) as f64 / n;
                    (a + (b - a) * t0, a + (b - a) * t1)
                };
                let (w, e) = lerp(r[0], east, xyz[0]);
                let (s, nn) = lerp(r[1], r[3], xyz[1]);
                let (lo, hi) = if octree {
                    lerp(r[4], r[5], xyz[2])
                } else {
                    (r[4], r[5])
                };
                let wrap = |x: f64| {
                    if x > std::f64::consts::PI {
                        x - std::f64::consts::TAU
                    } else {
                        x
                    }
                };
                Some(Given::Region([wrap(w), s, wrap(e), nn, lo, hi]))
            }
            Given::Sphere(_) => None,
        }
    }
}

/// The box of a region (see the module's note).
pub fn region_box(r: [f64; 6]) -> Volume {
    let (west, south, north) = (r[0], r[1], r[3]);
    let east = if r[2] < west {
        r[2] + std::f64::consts::TAU
    } else {
        r[2]
    };
    let (lo, hi) = (r[4], r[5].max(r[4]));
    let lon_c = (west + east) / 2.0;
    let lat_c = (south + north) / 2.0;
    let frame = math::enu_at(math::geodetic_to_ecef(lon_c, lat_c, lo));
    let inv = math::rigid_inverse(&frame);
    let (mut mn, mut mx) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
    let g = REGION_GRID - 1;
    for i in 0..=g {
        let lon = west + (east - west) * i as f64 / g as f64;
        for j in 0..=g {
            let lat = south + (north - south) * j as f64 / g as f64;
            for h in [lo, hi] {
                let p = math::point(&inv, math::geodetic_to_ecef(lon, lat, h));
                for k in 0..3 {
                    mn[k] = mn[k].min(p[k]);
                    mx[k] = mx[k].max(p[k]);
                }
            }
        }
    }
    let lc = scale(add(mn, mx), 0.5);
    let half = scale(sub(mx, mn), 0.5);
    let c = math::point(&frame, lc);
    let col = |k: usize| [frame[0][k], frame[1][k], frame[2][k]];
    Volume::Box {
        c,
        axes: [
            scale(col(0), half[0]),
            scale(col(1), half[1]),
            scale(col(2), half[2]),
        ],
    }
}

impl Volume {
    pub fn centre(&self) -> V3 {
        match *self {
            Volume::Box { c, .. } | Volume::Sphere { c, .. } => c,
        }
    }

    /// The radius of a sphere around [`Volume::centre`] that holds it.
    pub fn radius(&self) -> f64 {
        match *self {
            Volume::Sphere { r, .. } => r,
            Volume::Box {
                axes: [u, v, w], ..
            } => {
                let mut r: f64 = 0.0;
                for s in [[1.0, 1.0], [1.0, -1.0], [-1.0, 1.0], [-1.0, -1.0]] {
                    r = r.max(len(add(add(scale(u, s[0]), scale(v, s[1])), w)));
                }
                r
            }
        }
    }

    /// The distance from `p` to the volume (0 inside).
    pub fn distance(&self, p: V3) -> f64 {
        match *self {
            Volume::Sphere { c, r } => (len(sub(p, c)) - r).max(0.0),
            Volume::Box { c, axes } => {
                let d = sub(p, c);
                let mut sum = 0.0;
                for k in 0..3 {
                    let h = len(axes[k]);
                    let (dir, h) = if h > 1e-12 {
                        (scale(axes[k], 1.0 / h), h)
                    } else {
                        // A flat box: this axis is the other two's normal.
                        (unit(cross(axes[(k + 1) % 3], axes[(k + 2) % 3])), 0.0)
                    };
                    let e = (dot(d, dir).abs() - h).max(0.0);
                    sum += e * e;
                }
                sum.sqrt()
            }
        }
    }

    /// Whether `p` is inside (a viewer request volume).
    pub fn contains(&self, p: V3) -> bool {
        self.distance(p) <= 0.0
    }

    /// The eight corners (a sphere: its axis-aligned cube's).
    pub fn corners(&self) -> [V3; 8] {
        let (c, axes) = match *self {
            Volume::Box { c, axes } => (c, axes),
            Volume::Sphere { c, r } => (c, [[r, 0.0, 0.0], [0.0, r, 0.0], [0.0, 0.0, r]]),
        };
        let mut out = [[0.0; 3]; 8];
        for (i, o) in out.iter_mut().enumerate() {
            let s = |b: usize| if i & (1 << b) == 0 { -1.0 } else { 1.0 };
            *o = add(
                add(add(c, scale(axes[0], s(0))), scale(axes[1], s(1))),
                scale(axes[2], s(2)),
            );
        }
        out
    }
}
