//! The dental arch of a CBCT or CT volume, suggested for a panoramic reconstruction.
//!
//! [`suggest`] finds the densest material (the top half percent of values: enamel and
//! dentine), the axial level where most of it lies, and an axial MIP of a slab around that
//! level; the arch is a parabola fitted to the MIP's dense pixels (mean depth per 2 mm of
//! width, weighted by count), returned as control points evenly spaced along it for the user
//! to edit. It only places a curve: nothing is measured or detected.
//!
//! [`phantom`] is a synthetic jaw for tests and screenshots (no patient data).

use specular_dicom::volume::Volume;

use crate::dot;

type V3 = [f64; 3];

/// Control points the suggestion places.
pub const POINTS: usize = 9;
/// Half the slab the MIP covers (mm).
const HALF_SLAB: f64 = 8.0;
/// The share of voxels counted as dense.
const DENSE_SHARE: f64 = 0.005;

/// The suggested arch: control points (patient mm) at one axial level, and the value taken
/// as dense.
#[derive(Clone, Debug, PartialEq)]
pub struct Arch {
    pub points: Vec<V3>,
    pub threshold: f32,
}

/// The arch of `vol`'s densest material, if there is enough of it to fit one.
pub fn suggest(vol: &Volume) -> Option<Arch> {
    let g = &vol.geometry;
    let [w, h, d] = g.dims;
    if w < 8 || h < 8 || d < 2 {
        return None;
    }
    // A strided sample for the threshold (about 1M values at most).
    let total = u64::from(w) * u64::from(h) * u64::from(d);
    let stride = ((total as f64 / 1e6).cbrt().ceil() as u32).max(1);
    let mut sample = Vec::new();
    for k in (0..d).step_by(stride as usize) {
        if !vol.loaded[k as usize] {
            continue;
        }
        for j in (0..h).step_by(stride as usize) {
            for i in (0..w).step_by(stride as usize) {
                sample.push(vol.value(i, j, k));
            }
        }
    }
    if sample.len() < 64 {
        return None;
    }
    sample.sort_unstable_by(f32::total_cmp);
    let at = |q: f64| sample[((sample.len() - 1) as f64 * q).round() as usize];
    let threshold = at(1.0 - DENSE_SHARE);
    if threshold <= at(0.5) {
        return None;
    }
    // The level with most dense voxels, counts smoothed over ±5 mm.
    let step = if u64::from(w) * u64::from(h) > 1 << 20 {
        2
    } else {
        1
    };
    let counts: Vec<f64> = (0..d)
        .map(|k| {
            if !vol.loaded[k as usize] {
                return 0.0;
            }
            let mut n = 0u32;
            for j in (0..h).step_by(step) {
                for i in (0..w).step_by(step) {
                    n += u32::from(vol.value(i, j, k) >= threshold);
                }
            }
            f64::from(n)
        })
        .collect();
    let along = |k: usize| g.along[k];
    let smooth: Vec<f64> = (0..d as usize)
        .map(|k| {
            (0..d as usize)
                .filter(|&m| (along(m) - along(k)).abs() <= 5.0)
                .map(|m| counts[m])
                .sum()
        })
        .collect();
    let level = (0..d as usize).max_by(|&a, &b| smooth[a].total_cmp(&smooth[b]).then(b.cmp(&a)))?;
    if counts[level] == 0.0 && smooth[level] == 0.0 {
        return None;
    }
    // The MIP of the slab around it.
    let mut mip = vec![f32::NEG_INFINITY; (w * h) as usize];
    for k in 0..d as usize {
        if (along(k) - along(level)).abs() > HALF_SLAB || !vol.loaded[k] {
            continue;
        }
        for j in (0..h).step_by(step) {
            for i in (0..w).step_by(step) {
                let m = &mut mip[(j * w + i) as usize];
                *m = m.max(vol.value(i, j, k as u32));
            }
        }
    }
    // Dense pixels in the slice's own axes (u along rows, v down columns), in mm.
    let base = g.to_patient([0.0, 0.0, level as f64]);
    let bin = 2.0;
    let mut bins: std::collections::BTreeMap<i64, (f64, f64)> = Default::default();
    let mut us = Vec::new();
    for j in (0..h).step_by(step) {
        for i in (0..w).step_by(step) {
            if mip[(j * w + i) as usize] < threshold {
                continue;
            }
            let p = g.to_patient([f64::from(i), f64::from(j), level as f64]);
            let (u, v) = (dot(p, g.row), dot(p, g.col));
            let e = bins.entry((u / bin).floor() as i64).or_default();
            e.0 += v;
            e.1 += 1.0;
            us.push(u);
        }
    }
    let rows: Vec<(f64, f64, f64)> = bins
        .iter()
        .filter(|(_, (_, n))| *n >= 2.0)
        .map(|(&b, &(sum, n))| ((b as f64 + 0.5) * bin, sum / n, n))
        .collect();
    if rows.len() < 5 {
        return None;
    }
    let [a, b, c] = fit_parabola(&rows)?;
    us.sort_unstable_by(f64::total_cmp);
    let pick = |q: f64| us[((us.len() - 1) as f64 * q).round() as usize];
    let (lo, hi) = (pick(0.02), pick(0.98));
    if hi - lo < 10.0 {
        return None;
    }
    // Evenly spaced along the parabola.
    let n = 400;
    let pts: Vec<(f64, f64)> = (0..=n)
        .map(|i| {
            let u = lo + (hi - lo) * f64::from(i) / f64::from(n);
            (u, a * u * u + b * u + c)
        })
        .collect();
    let mut arc = vec![0.0];
    for w in pts.windows(2) {
        let l = arc.last().copied().unwrap_or(0.0);
        arc.push(l + (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1));
    }
    let total = *arc.last()?;
    let wn = dot(base, g.normal);
    let points = (0..POINTS)
        .map(|m| {
            let s = total * m as f64 / (POINTS - 1) as f64;
            let i = arc.partition_point(|&x| x < s).min(pts.len() - 1);
            let (u, v) = pts[i];
            [0, 1, 2].map(|c| g.row[c] * u + g.col[c] * v + g.normal[c] * wn)
        })
        .collect();
    Some(Arch { points, threshold })
}

/// Weighted least squares v = a u² + b u + c over (u, v, weight).
fn fit_parabola(rows: &[(f64, f64, f64)]) -> Option<[f64; 3]> {
    // Centred on the mean u for conditioning.
    let wsum: f64 = rows.iter().map(|r| r.2).sum();
    let mu = rows.iter().map(|r| r.0 * r.2).sum::<f64>() / wsum;
    let mut m = [[0.0f64; 3]; 3];
    let mut r = [0.0f64; 3];
    for &(u, v, w) in rows {
        let x = u - mu;
        let f = [x * x, x, 1.0];
        for i in 0..3 {
            for j in 0..3 {
                m[i][j] += w * f[i] * f[j];
            }
            r[i] += w * f[i] * v;
        }
    }
    let s = solve3(m, r)?;
    // Back to u: a (u − mu)² + b' (u − mu) + c'.
    let (a, b1, c1) = (s[0], s[1], s[2]);
    Some([a, b1 - 2.0 * a * mu, a * mu * mu - b1 * mu + c1])
}

fn solve3(m: [[f64; 3]; 3], r: [f64; 3]) -> Option<[f64; 3]> {
    let det3 = |a: [[f64; 3]; 3]| {
        a[0][0] * (a[1][1] * a[2][2] - a[1][2] * a[2][1])
            - a[0][1] * (a[1][0] * a[2][2] - a[1][2] * a[2][0])
            + a[0][2] * (a[1][0] * a[2][1] - a[1][1] * a[2][0])
    };
    let det = det3(m);
    if det.abs() < 1e-12 {
        return None;
    }
    let col = |c: usize| {
        let mut a = m;
        for (row, v) in a.iter_mut().zip(r) {
            row[c] = v;
        }
        det3(a) / det
    };
    Some([col(0), col(1), col(2)])
}

/// A synthetic jaw (mm, LPS, centred on the origin; HU-like values): soft tissue in a head
/// ellipsoid, a U-shaped mandible along the arch [`phantom::arch`], fourteen teeth standing on
/// it with enamel crowns, and a nerve canal through the bone on each side.
pub mod phantom {
    use specular_dicom::volume::{Geometry, Volume, VoxelKind};

    type V3 = [f64; 3];

    pub const AIR: f32 = -1000.0;
    pub const TISSUE: f32 = 40.0;
    pub const BONE: f32 = 1000.0;
    pub const DENTINE: f32 = 2000.0;
    pub const ENAMEL: f32 = 3000.0;
    pub const CANAL: f32 = 60.0;
    /// The arch's half width (mm across, L–R).
    pub const HALF_WIDTH: f64 = 30.0;
    /// The teeth's extent (mm, z) and the crowns' start.
    pub const TOOTH_Z: (f64, f64) = (-12.0, 10.0);
    pub const CROWN_Z: f64 = 2.0;
    pub const TOOTH_RADIUS: f64 = 3.2;
    /// The mandible's extent (z) and half thickness across the arch.
    pub const BONE_Z: (f64, f64) = (-26.0, -6.0);
    pub const BONE_HALF: f64 = 5.5;
    /// The canal's level, radius, and where it ends towards the front (|x|).
    pub const CANAL_Z: f64 = -18.0;
    pub const CANAL_RADIUS: f64 = 1.4;
    pub const CANAL_FRONT: f64 = 14.0;

    /// The arch's depth (y, anterior negative) at width `x`.
    pub fn arch(x: f64) -> f64 {
        40.0 / (HALF_WIDTH * HALF_WIDTH) * x * x - 33.0
    }

    /// The distance from (x, y) to the arch in the axial plane, and the nearest x on it.
    pub fn to_arch(x: f64, y: f64) -> (f64, f64) {
        let mut best = (f64::INFINITY, 0.0);
        let mut u = -HALF_WIDTH - 6.0;
        while u <= HALF_WIDTH + 6.0 {
            let d = (x - u).hypot(y - arch(u));
            if d < best.0 {
                best = (d, u);
            }
            u += 0.25;
        }
        // Refine around the best sample.
        let mut lo = best.1 - 0.25;
        let mut hi = best.1 + 0.25;
        for _ in 0..30 {
            let a = lo + (hi - lo) / 3.0;
            let b = hi - (hi - lo) / 3.0;
            if (x - a).hypot(y - arch(a)) < (x - b).hypot(y - arch(b)) {
                hi = b;
            } else {
                lo = a;
            }
        }
        let u = (lo + hi) / 2.0;
        ((x - u).hypot(y - arch(u)), u)
    }

    /// The teeth's centres across (x), evenly along the arch's length.
    pub fn teeth() -> Vec<f64> {
        let n = 14;
        let mut arc = vec![(0.0, -HALF_WIDTH + 2.0)];
        let mut x = -HALF_WIDTH + 2.0;
        while x < HALF_WIDTH - 2.0 {
            let nx = x + 0.05;
            let l = arc.last().map_or(0.0, |a| a.0) + (nx - x).hypot(arch(nx) - arch(x));
            arc.push((l, nx));
            x = nx;
        }
        let total = arc.last().map_or(0.0, |a| a.0);
        (0..n)
            .map(|i| {
                let s = total * (f64::from(i) + 0.5) / f64::from(n);
                let k = arc.partition_point(|a| a.0 < s).min(arc.len() - 1);
                arc[k].1
            })
            .collect()
    }

    /// The canal's centreline on the side of `sign` (−1 right, +1 left): from the back of
    /// the arch to [`CANAL_FRONT`].
    pub fn canal(sign: f64) -> Vec<V3> {
        (0..6)
            .map(|i| {
                let x = sign
                    * (HALF_WIDTH + 4.0 - (HALF_WIDTH + 4.0 - CANAL_FRONT) * f64::from(i) / 5.0);
                [x, arch(x), CANAL_Z]
            })
            .collect()
    }

    /// The phantom's value at patient position `p`.
    pub fn value(p: V3, teeth: &[f64]) -> f32 {
        let [x, y, z] = p;
        let e = (x / 48.0).powi(2) + ((y + 5.0) / 52.0).powi(2) + (z / 60.0).powi(2);
        if e > 1.0 {
            return AIR;
        }
        if (TOOTH_Z.0..=TOOTH_Z.1).contains(&z) {
            for &tx in teeth {
                let d = (x - tx).hypot(y - arch(tx));
                if d <= TOOTH_RADIUS {
                    return if z >= CROWN_Z { ENAMEL } else { DENTINE };
                }
            }
        }
        if (BONE_Z.0..=BONE_Z.1).contains(&z) && x.abs() <= HALF_WIDTH + 8.0 {
            let (d, u) = to_arch(x, y);
            if d <= BONE_HALF {
                // The canal: along the arch at its level, behind the front end.
                if u.abs() >= CANAL_FRONT && (z - CANAL_Z).hypot(d) <= CANAL_RADIUS {
                    return CANAL;
                }
                return BONE;
            }
        }
        TISSUE
    }

    /// The phantom on `dims` voxels, `pixel` mm in-plane and `spacing` mm apart, centred on
    /// the origin; every slice loaded.
    pub fn volume(dims: [u32; 3], pixel: f64, spacing: f64) -> Volume {
        let half = |n: u32, s: f64| (f64::from(n) - 1.0) * s / 2.0;
        let pos: Vec<V3> = (0..dims[2])
            .map(|k| {
                [
                    -half(dims[0], pixel),
                    -half(dims[1], pixel),
                    f64::from(k) * spacing - half(dims[2], spacing),
                ]
            })
            .collect();
        let g = Geometry::new(
            &pos,
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [pixel, pixel],
            [dims[0], dims[1]],
        );
        let mut v = Volume::new(g, VoxelKind::I16, 1).expect("phantom fits in memory");
        let teeth = teeth();
        let mut buf = vec![0f32; (dims[0] * dims[1]) as usize];
        for k in 0..dims[2] as usize {
            for j in 0..dims[1] {
                for i in 0..dims[0] {
                    let p = v
                        .geometry
                        .to_patient([f64::from(i), f64::from(j), k as f64]);
                    buf[(j * dims[0] + i) as usize] = value(p, &teeth);
                }
            }
            v.put(k, &buf, dims[0], dims[1]).expect("slice fits");
        }
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggests_the_phantom_arch() {
        let vol = phantom::volume([112, 112, 56], 0.7, 0.9);
        let a = suggest(&vol).expect("an arch");
        assert_eq!(a.points.len(), POINTS);
        for p in &a.points {
            let (d, _) = phantom::to_arch(p[0], p[1]);
            assert!(d < 3.0, "{p:?} is {d:.1} mm off the arch");
            assert!(
                (phantom::TOOTH_Z.0..=phantom::TOOTH_Z.1).contains(&p[2]),
                "level {}",
                p[2]
            );
        }
        // It spans the teeth, end to end.
        let xs: Vec<f64> = a.points.iter().map(|p| p[0]).collect();
        assert!(
            xs[0] < -20.0 && xs[POINTS - 1] > 20.0 || xs[0] > 20.0 && xs[POINTS - 1] < -20.0,
            "{xs:?}"
        );
        assert!(a.threshold >= phantom::DENTINE - 1.0);
    }

    #[test]
    fn nothing_dense_gives_no_arch() {
        let mut vol = phantom::volume([24, 24, 8], 3.0, 3.0);
        vol.loaded.iter_mut().for_each(|l| *l = false);
        assert!(suggest(&vol).is_none());
        // A uniform volume has no dense material.
        let flat = crate::curve::tests::volume_of([24, 24, 8], 3.0, 3.0, |_| 40.0);
        assert!(suggest(&flat).is_none());
    }
}
