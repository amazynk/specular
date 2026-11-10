//! Double-precision vectors and transforms, and the WGS84 ellipsoid.

pub type V3 = [f64; 3];

/// A 4 × 4 transform, row-major (`m[row][col]`); 3D Tiles writes them column-major.
pub type M4 = [[f64; 4]; 4];

pub const IDENTITY: M4 = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

/// glTF's y-up to 3D Tiles' z-up: (x, y, z) → (x, −z, y).
pub const Y_UP_TO_Z_UP: M4 = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 0.0, -1.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

/// glTF content declared x-up (3D Tiles 1.0 `gltfUpAxis`): −90° about y,
/// (x, y, z) → (−z, y, x).
pub const X_UP_TO_Z_UP: M4 = [
    [0.0, 0.0, -1.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

pub fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

pub fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub fn scale(a: V3, s: f64) -> V3 {
    [a[0] * s, a[1] * s, a[2] * s]
}

pub fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

pub fn len(a: V3) -> f64 {
    dot(a, a).sqrt()
}

/// `a` scaled to length 1 (zero stays zero).
pub fn unit(a: V3) -> V3 {
    let l = len(a);
    if l > 0.0 && l.is_finite() {
        scale(a, 1.0 / l)
    } else {
        [0.0; 3]
    }
}

/// From 16 numbers in column-major order; `None` unless all are finite.
pub fn from_column_major(a: &[f64]) -> Option<M4> {
    if a.len() != 16 || a.iter().any(|v| !v.is_finite()) {
        return None;
    }
    let mut m = [[0.0; 4]; 4];
    for (c, col) in a.chunks_exact(4).enumerate() {
        for (r, v) in col.iter().enumerate() {
            m[r][c] = *v;
        }
    }
    Some(m)
}

/// `a · b` (`b` applied first).
pub fn mul(a: &M4, b: &M4) -> M4 {
    let mut m = [[0.0; 4]; 4];
    for (r, row) in m.iter_mut().enumerate() {
        for (c, v) in row.iter_mut().enumerate() {
            *v = (0..4).map(|k| a[r][k] * b[k][c]).sum();
        }
    }
    m
}

pub fn point(m: &M4, p: V3) -> V3 {
    let r = |i: usize| m[i][0] * p[0] + m[i][1] * p[1] + m[i][2] * p[2] + m[i][3];
    [r(0), r(1), r(2)]
}

pub fn vector(m: &M4, v: V3) -> V3 {
    let r = |i: usize| m[i][0] * v[0] + m[i][1] * v[1] + m[i][2] * v[2];
    [r(0), r(1), r(2)]
}

pub fn translation(t: V3) -> M4 {
    let mut m = IDENTITY;
    m[0][3] = t[0];
    m[1][3] = t[1];
    m[2][3] = t[2];
    m
}

/// The largest scale of `m`'s columns (a sphere's radius grows by it).
pub fn max_scale(m: &M4) -> f64 {
    (0..3)
        .map(|c| len([m[0][c], m[1][c], m[2][c]]))
        .fold(0.0, f64::max)
}

/// The inverse of a rigid transform (rotation and translation).
pub fn rigid_inverse(m: &M4) -> M4 {
    let mut r = IDENTITY;
    for i in 0..3 {
        for j in 0..3 {
            r[i][j] = m[j][i];
        }
    }
    let t = vector(&r, [m[0][3], m[1][3], m[2][3]]);
    r[0][3] = -t[0];
    r[1][3] = -t[1];
    r[2][3] = -t[2];
    r
}

/// The first three rows (an affine for `specular_mesh::read_gltf_scene`).
pub fn affine(m: &M4) -> [[f64; 4]; 3] {
    [m[0], m[1], m[2]]
}

/// WGS84 semi-major axis and first eccentricity squared.
const WGS84_A: f64 = 6_378_137.0;
const WGS84_E2: f64 = 6.694_379_990_141_316e-3;

/// Earth-centred, earth-fixed metres of a longitude and latitude (radians) and an
/// ellipsoidal height (metres).
pub fn geodetic_to_ecef(lon: f64, lat: f64, h: f64) -> V3 {
    let (sl, cl) = lat.sin_cos();
    let n = WGS84_A / (1.0 - WGS84_E2 * sl * sl).sqrt();
    let (so, co) = lon.sin_cos();
    [
        (n + h) * cl * co,
        (n + h) * cl * so,
        (n * (1.0 - WGS84_E2) + h) * sl,
    ]
}

/// The east-north-up frame at an earth-fixed point: local → earth-fixed. At the poles (or
/// the centre) east is +x.
pub fn enu_at(p: V3) -> M4 {
    // The geodetic surface normal: the ellipsoid's gradient at p.
    let b2 = 1.0 - WGS84_E2;
    let up = unit([p[0], p[1], p[2] / b2]);
    let mut east = [-p[1], p[0], 0.0];
    if len(east) < 1e-9 {
        east = [1.0, 0.0, 0.0];
    }
    let east = unit(east);
    let up = if len(up) < 0.5 { [0.0, 0.0, 1.0] } else { up };
    let north = cross(up, east);
    [
        [east[0], north[0], up[0], p[0]],
        [east[1], north[1], up[1], p[1]],
        [east[2], north[2], up[2], p[2]],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

/// Longitude, latitude (radians) and height of an earth-fixed point (Bowring's iteration;
/// for the info panel).
pub fn ecef_to_geodetic(p: V3) -> (f64, f64, f64) {
    let lon = p[1].atan2(p[0]);
    let r = p[0].hypot(p[1]);
    let mut lat = p[2].atan2(r * (1.0 - WGS84_E2));
    let mut h = 0.0;
    for _ in 0..6 {
        let s = lat.sin();
        let n = WGS84_A / (1.0 - WGS84_E2 * s * s).sqrt();
        h = if lat.cos().abs() > 1e-9 {
            r / lat.cos() - n
        } else {
            p[2].abs() - n * (1.0 - WGS84_E2)
        };
        lat = p[2].atan2(r * (1.0 - WGS84_E2 * n / (n + h)));
    }
    (lon, lat, h)
}
