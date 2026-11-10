//! TIN measures (plan area, surface area, volume above a datum, elevation range) and contours
//! by marching triangles.

use crate::Surface;

/// What a surface measures.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Measures {
    /// Plan (2D) area and sloped (3D) area of the visible triangles.
    pub area_2d: f64,
    pub area_3d: f64,
    /// Volume between the surface and the horizontal plane at `z_min`.
    pub volume: f64,
    pub z_min: f64,
    pub z_max: f64,
}

fn corners(s: &Surface, f: &[u32; 3]) -> Option<[[f64; 3]; 3]> {
    let p = |i: u32| s.points.get(i as usize).copied();
    Some([p(f[0])?, p(f[1])?, p(f[2])?])
}

/// The surface's measures; `None` without triangles.
pub fn measures(s: &Surface) -> Option<Measures> {
    let mut z_min = f64::INFINITY;
    let mut z_max = f64::NEG_INFINITY;
    for f in &s.faces {
        for p in corners(s, f)? {
            z_min = z_min.min(p[2]);
            z_max = z_max.max(p[2]);
        }
    }
    if !z_min.is_finite() {
        return None;
    }
    let mut m = Measures {
        z_min,
        z_max,
        ..Measures::default()
    };
    for f in &s.faces {
        let [a, b, c] = corners(s, f)?;
        let (u, v) = (
            [b[0] - a[0], b[1] - a[1], b[2] - a[2]],
            [c[0] - a[0], c[1] - a[1], c[2] - a[2]],
        );
        let cross = [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ];
        let a2 = cross[2].abs() / 2.0;
        m.area_2d += a2;
        m.area_3d += (cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2]).sqrt() / 2.0;
        m.volume += a2 * ((a[2] + b[2] + c[2]) / 3.0 - z_min);
    }
    Some(m)
}

/// A contour interval giving about `target` lines over `range`: 1, 2 or 5 × 10ⁿ.
pub fn interval(range: f64, target: f64) -> f64 {
    if !(range.is_finite() && range > 0.0) {
        return 1.0;
    }
    let raw = range / target.max(1.0);
    let p = 10f64.powf(raw.log10().floor());
    let m = raw / p;
    p * if m < 1.5 {
        1.0
    } else if m < 3.5 {
        2.0
    } else if m < 7.5 {
        5.0
    } else {
        10.0
    }
}

/// Contour segments at every multiple of `step` in the surface's range, with their level,
/// at most `max` segments.
pub fn contours(s: &Surface, step: f64, max: usize) -> Vec<(f64, [f64; 2], [f64; 2])> {
    let mut out = Vec::new();
    if !(step.is_finite() && step > 0.0) {
        return out;
    }
    for f in &s.faces {
        let Some(t) = corners(s, f) else { continue };
        let lo = t.iter().map(|p| p[2]).fold(f64::INFINITY, f64::min);
        let hi = t.iter().map(|p| p[2]).fold(f64::NEG_INFINITY, f64::max);
        let mut k = (lo / step).ceil();
        while k * step < hi {
            let z = k * step;
            k += 1.0;
            // The two edges the level crosses (a vertex exactly at z counts on one side).
            let mut pts = Vec::with_capacity(2);
            for (i, j) in [(0, 1), (1, 2), (2, 0)] {
                let (a, b) = (t[i], t[j]);
                if (a[2] < z) != (b[2] < z) {
                    let u = (z - a[2]) / (b[2] - a[2]);
                    pts.push([a[0] + (b[0] - a[0]) * u, a[1] + (b[1] - a[1]) * u]);
                }
            }
            if pts.len() == 2 {
                out.push((z, pts[0], pts[1]));
                if out.len() >= max {
                    return out;
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A square pyramid: 4 triangles, apex 1 m over a 2 × 2 base.
    fn pyramid() -> Surface {
        Surface {
            name: "p".into(),
            points: vec![
                [0.0, 0.0, 0.0],
                [2.0, 0.0, 0.0],
                [2.0, 2.0, 0.0],
                [0.0, 2.0, 0.0],
                [1.0, 1.0, 1.0],
            ],
            faces: vec![[0, 1, 4], [1, 2, 4], [2, 3, 4], [3, 0, 4]],
            ..Surface::default()
        }
    }

    #[test]
    fn measures_of_a_pyramid() {
        let m = measures(&pyramid()).unwrap();
        assert!((m.area_2d - 4.0).abs() < 1e-12);
        // Each face: base 2, slant height √2.
        assert!((m.area_3d - 4.0 * 2f64.sqrt()).abs() < 1e-12);
        // Prisms under each triangle: mean height 1/3 × area 1.
        assert!((m.volume - 4.0 / 3.0).abs() < 1e-12);
        assert_eq!((m.z_min, m.z_max), (0.0, 1.0));
    }

    #[test]
    fn contour_lines() {
        let c = contours(&pyramid(), 0.5, 1000);
        // Level 0.5 crosses all four faces.
        assert_eq!(c.len(), 4);
        let len: f64 = c
            .iter()
            .map(|(_, a, b)| (a[0] - b[0]).hypot(a[1] - b[1]))
            .sum();
        assert!((len - 4.0).abs() < 1e-9, "{len}");
        assert_eq!(interval(37.0, 15.0), 2.0);
        assert_eq!(interval(0.4, 15.0), 0.02);
    }
}
