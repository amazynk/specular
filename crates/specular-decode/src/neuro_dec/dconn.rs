//! CIFTI-2 dense connectivity (`.dconn.nii`): brain models along both dimensions, so each
//! row is one seed's connectivity to every brain model. The files are often several
//! gigabytes; nothing is read up front but the header and XML, and a row is read whole
//! ([`Cifti::row`]) from the memory-mapped file when a seed is picked: on the surfaces, a
//! clicked vertex; in MPR, the voxel under the crosshair.

use std::sync::Arc;
use std::time::Instant;

use specular_inspect::ColourMap;
use specular_neuro::cifti::{Cifti, Model, VolumeSpace};

use super::{Data, SurfaceScalars, colour_scalars, map_err, scalar_range, short_structure};
use crate::medvol_dec::medvol;
use crate::{DecodeError, Result};

/// A dense connectivity file, ready to read a seed's row and spread it over the surfaces'
/// mesh vertices or the volume's voxels.
pub struct Connectivity {
    c: Arc<Cifti>,
    data: Data,
    /// The mesh's vertex count (0 when shown as a volume).
    vertices: usize,
    /// (mesh vertex, dimension-0 column) of every dimension-0 surface brainordinate.
    colour_at: Vec<(u32, u32)>,
    /// The seed (dimension-1 index) of each mesh vertex; `u32::MAX` where there is none.
    vertex_seed: Vec<u32>,
    /// (voxel index in dimension 0's volume space, dimension-0 column) of every voxel
    /// brainordinate.
    voxels: Vec<(usize, u32)>,
    /// (voxel index in dimension 1's volume space, seed), sorted by voxel.
    voxel_seed: Vec<(usize, u32)>,
    /// What a seed is: its structure and vertex, or voxel (i, j, k).
    seed_of: Vec<(u32, u32)>,
}

impl std::fmt::Debug for Connectivity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Connectivity")
            .field("shape", &self.c.shape)
            .field("vertices", &self.vertices)
            .finish_non_exhaustive()
    }
}

/// Voxel (i, j, k) as an index into a volume of `dims`, x fastest.
fn voxel_index(v: &[u32; 3], dims: [usize; 3]) -> Option<usize> {
    let [i, j, k] = v.map(|x| x as usize);
    (i < dims[0] && j < dims[1] && k < dims[2]).then(|| i + dims[0] * (j + dims[1] * k))
}

impl Connectivity {
    /// `vertices`: the mesh's vertex count when the file is shown on surfaces (0 when it is
    /// shown as a volume); `base_of`: a structure's first mesh vertex.
    pub(super) fn new(
        c: Arc<Cifti>,
        data: Data,
        vertices: usize,
        base_of: &dyn Fn(&str) -> Option<u32>,
    ) -> Connectivity {
        let total = vertices as u32;
        let mut colour_at = Vec::new();
        let mut voxels = Vec::new();
        let dims0 = c.volume_of(0).map(|v| v.dims);
        for m in c.models_of(0) {
            match &m.model {
                Model::Surface { vertices, .. } => {
                    let Some(base) = base_of(&m.structure) else {
                        continue;
                    };
                    for (k, &v) in vertices.iter().enumerate() {
                        let at = base.saturating_add(v);
                        if at < total {
                            colour_at.push((at, (m.offset + k) as u32));
                        }
                    }
                }
                Model::Voxels(vx) => {
                    let Some(d) = dims0 else { continue };
                    for (k, v) in vx.iter().enumerate() {
                        if let Some(i) = voxel_index(v, d) {
                            voxels.push((i, (m.offset + k) as u32));
                        }
                    }
                }
            }
        }
        let mut vertex_seed = vec![u32::MAX; vertices];
        let mut voxel_seed = Vec::new();
        let mut seed_of = vec![(u32::MAX, 0); c.shape[1]];
        let dims1 = c.volume_of(1).map(|v| v.dims);
        for (n, m) in c.models_of(1).iter().enumerate() {
            match &m.model {
                Model::Surface { vertices, .. } => {
                    let base = base_of(&m.structure);
                    for (k, &v) in vertices.iter().enumerate() {
                        let seed = m.offset + k;
                        if let Some(s) = seed_of.get_mut(seed) {
                            *s = (n as u32, k as u32);
                        }
                        if let Some(at) = base.map(|b| b.saturating_add(v) as usize)
                            && at < vertex_seed.len()
                        {
                            vertex_seed[at] = seed as u32;
                        }
                    }
                }
                Model::Voxels(vx) => {
                    for (k, v) in vx.iter().enumerate() {
                        let seed = m.offset + k;
                        if let Some(s) = seed_of.get_mut(seed) {
                            *s = (n as u32, k as u32);
                        }
                        if let Some(i) = dims1.and_then(|d| voxel_index(v, d)) {
                            voxel_seed.push((i, seed as u32));
                        }
                    }
                }
            }
        }
        voxel_seed.sort_unstable();
        Connectivity {
            c,
            data,
            vertices,
            colour_at,
            vertex_seed,
            voxels,
            voxel_seed,
            seed_of,
        }
    }

    /// Seeds (rows): the brain models of dimension 1.
    pub fn seeds(&self) -> usize {
        self.c.shape[1]
    }

    /// Bytes a row takes in the file.
    pub fn row_bytes(&self) -> usize {
        let size = self.c.nifti.sample_size().unwrap_or(0);
        self.c.shape[0].saturating_mul(size)
    }

    /// Whether seeds can be picked in a volume (some are voxels).
    pub fn has_voxel_seeds(&self) -> bool {
        !self.voxel_seed.is_empty()
    }

    /// The seed at mesh vertex `v`, when the surfaces have one there.
    pub fn seed_at_vertex(&self, v: u32) -> Option<u32> {
        self.vertex_seed
            .get(v as usize)
            .copied()
            .filter(|&s| s != u32::MAX)
    }

    /// The seed whose voxel holds `ras` (millimetres, RAS), when there is one.
    pub fn seed_at_ras(&self, ras: [f64; 3]) -> Option<u32> {
        let vs = self.c.volume_of(1)?;
        let ijk = ras_to_voxel(vs, ras)?;
        let i = voxel_index(&ijk, vs.dims)?;
        let at = self.voxel_seed.binary_search_by_key(&i, |e| e.0).ok()?;
        Some(self.voxel_seed[at].1)
    }

    /// What seed `s` is: "CORTEX_LEFT, vertex 1234" or "THALAMUS_LEFT, voxel (3, 4, 2)"
    /// (structure names as the file has them).
    pub fn seed_name(&self, s: u32) -> String {
        let Some(&(m, k)) = self.seed_of.get(s as usize) else {
            return format!("#{s}");
        };
        let Some(model) = self.c.models_of(1).get(m as usize) else {
            return format!("#{s}");
        };
        let structure = short_structure(&model.structure);
        match &model.model {
            Model::Surface { vertices, .. } => {
                let v = vertices.get(k as usize).copied().unwrap_or(k);
                format!("{structure} {v}")
            }
            Model::Voxels(vx) => {
                let [i, j, kk] = vx.get(k as usize).copied().unwrap_or([0; 3]);
                format!("{structure} ({i}, {j}, {kk})")
            }
        }
    }

    /// Seed `s`'s row: its connectivity to every brain model of dimension 0, read from the
    /// file (only that row's bytes).
    pub fn row(&self, s: u32) -> Result<Vec<f32>> {
        let t0 = Instant::now();
        let row = self.c.row(self.data.bytes(), s as usize).map_err(map_err)?;
        specular_log::debug!(
            target: specular_log::DECODE,
            "dconn: row {s} ({} values) read in {:.2} ms",
            row.len(),
            t0.elapsed().as_secs_f64() * 1e3
        );
        Ok(row)
    }

    /// `row` spread over the mesh vertices (NaN where there is no value).
    pub fn vertex_values(&self, row: &[f32]) -> Vec<f32> {
        let mut out = vec![f32::NAN; self.vertices];
        for &(v, col) in &self.colour_at {
            out[v as usize] = row.get(col as usize).copied().unwrap_or(f32::NAN);
        }
        out
    }

    /// The mesh vertices' colours for seed `s`, and the values they show.
    pub fn surface(&self, s: u32) -> Result<(Vec<[u8; 4]>, SurfaceScalars)> {
        let v = self.vertex_values(&self.row(s)?);
        Ok(colours(v))
    }

    /// `row` as a float volume in dimension 0's volume space (zero outside the brain
    /// models), oriented as the volume reader keeps volumes (LPS).
    pub fn volume(&self, row: &[f32]) -> Result<medvol::MedVolume> {
        let vs = self.c.volume_of(0).ok_or(DecodeError::UnsupportedFormat)?;
        let per = vs.dims.iter().try_fold(1usize, |a, &d| a.checked_mul(d));
        let per = per
            .filter(|&n| n > 0 && n.saturating_mul(4) <= super::MAX_VOLUME_BYTES)
            .ok_or(DecodeError::TooLarge)?;
        let mut buf = vec![0f32; per];
        for &(i, col) in &self.voxels {
            if let Some(&x) = row.get(col as usize) {
                buf[i] = if x.is_finite() { x } else { 0.0 };
            }
        }
        super::float_volume(vs, buf, 1, &self.c.nifti.description, self.c.nifti.intent())
    }

    /// Seed `s`'s row as a volume ([`Connectivity::volume`]).
    pub fn volume_of_seed(&self, s: u32) -> Result<medvol::MedVolume> {
        self.volume(&self.row(s)?)
    }

    /// The first seed: what is shown when the file opens.
    pub(super) fn first_seed(&self) -> u32 {
        0
    }
}

/// Colours of per-vertex connectivity values over their robust range (a diverging map when
/// they straddle zero evenly).
pub(super) fn colours(v: Vec<f32>) -> (Vec<[u8; 4]>, SurfaceScalars) {
    let (range, diverging) = scalar_range(&v);
    let map = if diverging {
        ColourMap::Vik
    } else {
        ColourMap::Viridis
    };
    let c = colour_scalars(&v, range, map);
    (
        c,
        SurfaceScalars {
            values: v.into(),
            range,
            diverging,
        },
    )
}

/// The voxel (i, j, k) holding `ras` in `vs`, rounded to the nearest centre.
fn ras_to_voxel(vs: &VolumeSpace, ras: [f64; 3]) -> Option<[u32; 3]> {
    let a = &vs.affine;
    let m = [
        [a[0][0], a[0][1], a[0][2]],
        [a[1][0], a[1][1], a[1][2]],
        [a[2][0], a[2][1], a[2][2]],
    ];
    let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
    if det.abs() < 1e-12 || !det.is_finite() {
        return None;
    }
    let b = [ras[0] - a[0][3], ras[1] - a[1][3], ras[2] - a[2][3]];
    // Cramer's rule.
    let col = |c: usize| {
        let mut n = m;
        for r in 0..3 {
            n[r][c] = b[r];
        }
        n[0][0] * (n[1][1] * n[2][2] - n[1][2] * n[2][1])
            - n[0][1] * (n[1][0] * n[2][2] - n[1][2] * n[2][0])
            + n[0][2] * (n[1][0] * n[2][1] - n[1][1] * n[2][0])
    };
    let ijk = [col(0) / det, col(1) / det, col(2) / det].map(f64::round);
    ijk.iter()
        .all(|&x| (0.0..f64::from(u32::MAX)).contains(&x))
        .then(|| ijk.map(|x| x as u32))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ras_round_trips_through_an_oblique_affine() {
        let vs = VolumeSpace {
            dims: [10, 12, 8],
            affine: [
                [-2.0, 0.1, 0.0, 9.0],
                [0.0, 2.0, 0.3, -12.0],
                [0.2, 0.0, 2.0, -8.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        };
        for v in [[0u32, 0, 0], [3, 4, 2], [9, 11, 7]] {
            let a = &vs.affine;
            let p = [0, 1, 2].map(|r| {
                a[r][0] * f64::from(v[0])
                    + a[r][1] * f64::from(v[1])
                    + a[r][2] * f64::from(v[2])
                    + a[r][3]
            });
            assert_eq!(ras_to_voxel(&vs, p), Some(v));
        }
        let flat = VolumeSpace {
            dims: [1, 1, 1],
            affine: [[0.0; 4]; 4],
        };
        assert_eq!(ras_to_voxel(&flat, [0.0; 3]), None);
    }
}
