//! Total metabolic tumour volume (TMTV) in a user-drawn region: the PET voxels at or above an
//! SUV threshold (a fixed SUV, e.g. 2.5, or a share of the region's SUVmax, e.g. 41 %), their
//! volume in mL and the number of connected (26-neighbour) groups of them.
//!
//! Read-only analysis (DESIGN §7): the region and the threshold are the user's; the result
//! counts voxels, it does not find, name or judge lesions.

use crate::labels::{Builder, LabelSource, Labels, Segment, voxel_ml};
use crate::volume::{Geometry, Volume};

/// Where to measure, in patient mm.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Region {
    /// The box between two corners, along the volume's axes.
    Box {
        a: [f64; 3],
        b: [f64; 3],
    },
    Sphere {
        centre: [f64; 3],
        radius: f64,
    },
}

/// Which voxels count.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Threshold {
    /// SUV at or above this.
    Suv(f64),
    /// At or above this share (0..1) of the region's SUVmax.
    OfMax(f64),
}

/// The measurement.
#[derive(Clone, Debug, PartialEq)]
pub struct Tmtv {
    pub ml: f64,
    pub voxels: u64,
    /// Connected groups of counted voxels (26-neighbour).
    pub groups: usize,
    /// SUV threshold applied.
    pub threshold: f64,
    pub suv_max: f64,
    /// Mean SUV of the counted voxels (SUVmean of the TMTV).
    pub suv_mean: f64,
    /// Total lesion glycolysis: TMTV mL × SUVmean.
    pub tlg: f64,
    /// Region voxels.
    pub region_voxels: u64,
    /// The counted voxels as one segment, for display.
    pub labels: Labels,
}

/// The voxel index range of `region` (inclusive) in `g`, and whether voxel `v` is inside.
fn bounds(g: &Geometry, region: &Region) -> ([usize; 3], [usize; 3]) {
    let corners: Vec<[f64; 3]> = match *region {
        Region::Box { a, b } => {
            let (va, vb) = (g.to_voxel(a), g.to_voxel(b));
            vec![va, vb]
        }
        Region::Sphere { centre, radius } => {
            let mut out = Vec::new();
            for s in [-1.0, 1.0] {
                for axis in [g.row, g.col, g.normal] {
                    out.push(g.to_voxel([0, 1, 2].map(|c| centre[c] + s * radius * axis[c])));
                }
            }
            out
        }
    };
    let mut lo = [0usize; 3];
    let mut hi = [0usize; 3];
    for c in 0..3 {
        let min = corners.iter().map(|v| v[c]).fold(f64::MAX, f64::min);
        let max = corners.iter().map(|v| v[c]).fold(f64::MIN, f64::max);
        let n = g.dims[c] as f64;
        lo[c] = min.round().clamp(0.0, n - 1.0) as usize;
        hi[c] = max.round().clamp(0.0, n - 1.0) as usize;
    }
    (lo, hi)
}

fn inside(g: &Geometry, region: &Region, v: [usize; 3]) -> bool {
    match *region {
        // The box's voxel range already is the region (within half a voxel).
        Region::Box { .. } => true,
        Region::Sphere { centre, radius } => {
            let p = g.to_patient([v[0] as f64, v[1] as f64, v[2] as f64]);
            let d2: f64 = (0..3).map(|c| (p[c] - centre[c]).powi(2)).sum();
            d2 <= radius * radius
        }
    }
}

/// Measures `region` of `pet` (modality values) with SUV = value × `factor`. `None` when the
/// region holds no voxel of the volume (or the volume is not loaded there).
pub fn measure(pet: &Volume, factor: f64, region: &Region, threshold: Threshold) -> Option<Tmtv> {
    let g = &pet.geometry;
    let (lo, hi) = bounds(g, region);
    let [w, h, _] = g.dims.map(|d| d as usize);
    let (cw, ch, cd) = (hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1);
    // SUV of each region voxel (NaN outside the region).
    let mut suv = vec![f32::NAN; cw * ch * cd];
    let mut max = f64::MIN;
    let mut region_voxels = 0u64;
    for k in lo[2]..=hi[2] {
        if !pet.loaded.get(k).copied().unwrap_or(false) {
            continue;
        }
        for j in lo[1]..=hi[1] {
            for i in lo[0]..=hi[0] {
                if !inside(g, region, [i, j, k]) {
                    continue;
                }
                let s = f64::from(pet.voxels.get((k * h + j) * w + i)) * factor;
                suv[((k - lo[2]) * ch + (j - lo[1])) * cw + (i - lo[0])] = s as f32;
                max = max.max(s);
                region_voxels += 1;
            }
        }
    }
    if region_voxels == 0 {
        return None;
    }
    let t = match threshold {
        Threshold::Suv(t) => t,
        Threshold::OfMax(f) => f * max,
    };
    let on: Vec<bool> = suv
        .iter()
        .map(|&s| !s.is_nan() && f64::from(s) >= t)
        .collect();
    let voxels = on.iter().filter(|&&b| b).count() as u64;
    let sum: f64 = suv
        .iter()
        .zip(&on)
        .filter(|(_, b)| **b)
        .map(|(s, _)| f64::from(*s))
        .sum();
    let groups = components(&on, [cw, ch, cd]);
    let ml = voxels as f64 * voxel_ml(g);
    let suv_mean = if voxels > 0 { sum / voxels as f64 } else { 0.0 };
    // The counted voxels as a segment on the PET grid.
    let mut b = Builder::new(g.clone(), 1);
    for k in 0..cd {
        if !on[k * ch * cw..(k + 1) * ch * cw].iter().any(|&x| x) {
            continue;
        }
        let m = b.mask(0, k + lo[2]);
        for j in 0..ch {
            for i in 0..cw {
                if on[(k * ch + j) * cw + i] {
                    m[(j + lo[1]) * w + i + lo[0]] = 255;
                }
            }
        }
    }
    let labels = Labels {
        source: LabelSource::Seg,
        segments: vec![Segment {
            number: 1,
            label: "TMTV".into(),
            colour: [255, 200, 0],
            algorithm: "MANUAL".into(),
            fractional: false,
            ml,
            voxels,
        }],
        layers: b.finish(&[false]),
        frame_of_reference: String::new(),
        referenced_series: Vec::new(),
        title: String::new(),
    };
    Some(Tmtv {
        ml,
        voxels,
        groups,
        threshold: t,
        suv_max: max,
        suv_mean,
        tlg: ml * suv_mean,
        region_voxels,
        labels,
    })
}

/// Connected groups (26-neighbour) of `on` in a `dims` grid.
fn components(on: &[bool], dims: [usize; 3]) -> usize {
    let [w, h, d] = dims;
    let mut seen = vec![false; on.len()];
    let mut stack = Vec::new();
    let mut n = 0;
    for start in 0..on.len() {
        if !on[start] || seen[start] {
            continue;
        }
        n += 1;
        seen[start] = true;
        stack.push(start);
        while let Some(at) = stack.pop() {
            let (i, j, k) = (at % w, at / w % h, at / (w * h));
            for dk in -1i64..=1 {
                for dj in -1i64..=1 {
                    for di in -1i64..=1 {
                        let (x, y, z) = (i as i64 + di, j as i64 + dj, k as i64 + dk);
                        if x < 0
                            || y < 0
                            || z < 0
                            || x >= w as i64
                            || y >= h as i64
                            || z >= d as i64
                        {
                            continue;
                        }
                        let o = (z as usize * h + y as usize) * w + x as usize;
                        if on[o] && !seen[o] {
                            seen[o] = true;
                            stack.push(o);
                        }
                    }
                }
            }
        }
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::volume::Voxels;

    fn volume() -> Volume {
        // 20×20×20 voxels of 2 mm, background 1, two hot blobs.
        let positions: Vec<[f64; 3]> = (0..20).map(|k| [0.0, 0.0, k as f64 * 2.0]).collect();
        let g = Geometry::new(
            &positions,
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [2.0, 2.0],
            [20, 20],
        );
        let mut v = vec![1.0f32; 8000];
        // Blob A: 3×3×3 at (5..8) value 10; blob B: 2×2×2 at (14..16) value 4.
        for k in 5..8 {
            for j in 5..8 {
                for i in 5..8 {
                    v[(k * 20 + j) * 20 + i] = 10.0;
                }
            }
        }
        for k in 14..16 {
            for j in 14..16 {
                for i in 14..16 {
                    v[(k * 20 + j) * 20 + i] = 4.0;
                }
            }
        }
        Volume {
            geometry: g,
            voxels: Voxels::F32(v),
            loaded: vec![true; 20],
            step_xy: 1,
        }
    }

    #[test]
    fn fixed_and_relative_thresholds() {
        let vol = volume();
        let all = Region::Box {
            a: [0.0, 0.0, 0.0],
            b: [38.0, 38.0, 38.0],
        };
        // SUV ≥ 2.5: both blobs, 27 + 8 voxels of 8 µL (0.008 mL).
        let t = measure(&vol, 1.0, &all, Threshold::Suv(2.5)).unwrap();
        assert_eq!((t.voxels, t.groups), (35, 2));
        assert!((t.ml - 35.0 * 0.008).abs() < 1e-12);
        assert_eq!(t.suv_max, 10.0);
        assert!((t.suv_mean - (270.0 + 32.0) / 35.0).abs() < 1e-9);
        assert_eq!(t.region_voxels, 8000);
        assert_eq!(t.labels.segments[0].voxels, 35);
        // 41 % of SUVmax 10 = 4.1: blob A only.
        let t = measure(&vol, 1.0, &all, Threshold::OfMax(0.41)).unwrap();
        assert_eq!((t.voxels, t.groups, t.threshold), (27, 1, 4.1));
        // The factor scales SUV: ×0.5 puts blob B (2) under 2.5.
        let t = measure(&vol, 0.5, &all, Threshold::Suv(2.5)).unwrap();
        assert_eq!((t.voxels, t.groups), (27, 1));
    }

    #[test]
    fn sphere_region() {
        let vol = volume();
        // Around blob B's centre (voxel 15 → 30 mm... it spans 14..15, centre 14.5 → 29 mm).
        let s = Region::Sphere {
            centre: [29.0, 29.0, 29.0],
            radius: 3.0,
        };
        let t = measure(&vol, 1.0, &s, Threshold::Suv(2.5)).unwrap();
        assert_eq!((t.voxels, t.groups), (8, 1));
        // Nothing of blob A.
        assert_eq!(t.suv_max, 4.0);
    }
}
