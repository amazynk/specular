//! Empty-space skipping: the volume cut into bricks of [`BRICK`]³ voxels, each with the
//! smallest and largest value it and its 26 neighbours hold.
//!
//! A ray in a brick whose range the transfer function maps to nothing (or, for MIP, whose
//! largest value cannot beat the running maximum) jumps ahead by less than one brick edge in
//! every index direction, so it stays inside the neighbourhood the range covers: skipping
//! never changes the image.

use std::sync::atomic::{AtomicU64, Ordering};

use specular_dicom::volume::{Volume, Voxels};

/// Brick edge, voxels.
pub const BRICK: u32 = 16;

static IDS: AtomicU64 = AtomicU64::new(1);

/// Per-brick value ranges of a complete volume (see the module docs).
#[derive(Clone, Debug)]
pub struct Bricks {
    pub(crate) id: u64,
    /// Bricks along i, j and k.
    pub grid: [u32; 3],
    /// The smallest and largest value over each brick and its neighbours (k-major, then j,
    /// then i).
    pub range: Vec<[f32; 2]>,
}

impl Bricks {
    /// The ranges of `vol` (expected complete: slices not loaded count as holding nothing).
    /// Brick `b` along an axis covers voxels `b·BRICK ..= (b+1)·BRICK` (one past its cell, so
    /// interpolation inside the cell reads only voxels it covers). Runs on all cores.
    pub fn compute(vol: &Volume) -> Bricks {
        let dims = vol.geometry.dims;
        let grid = dims.map(|d| d.div_ceil(BRICK).max(1));
        let layer = (grid[0] * grid[1]) as usize;
        let mut raw = vec![[f32::INFINITY, f32::NEG_INFINITY]; layer * grid[2] as usize];
        let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
        let per = (grid[2] as usize).div_ceil(threads).max(1);
        std::thread::scope(|s| {
            for (n, chunk) in raw.chunks_mut(layer * per).enumerate() {
                specular_thread::spawn_scoped(s, format!("vr-bricks-{n}"), move || {
                    for (m, out) in chunk.chunks_mut(layer).enumerate() {
                        let bz = (n * per + m) as u32;
                        match &vol.voxels {
                            Voxels::I16(v) => layer_ranges(vol, v, bz, grid, out),
                            Voxels::U16(v) => layer_ranges(vol, v, bz, grid, out),
                            Voxels::F32(v) => layer_ranges(vol, v, bz, grid, out),
                        }
                    }
                });
            }
        });
        Bricks {
            id: IDS.fetch_add(1, Ordering::Relaxed),
            grid,
            range: dilate(&raw, grid),
        }
    }

    /// Per brick `[min, max, occupied, 0]` for the shader; `occupied(min, max)` says whether
    /// the transfer function shows anything in that range.
    pub(crate) fn rows(&self, occupied: impl Fn(f32, f32) -> bool) -> Vec<[f32; 4]> {
        self.range
            .iter()
            .map(|&[lo, hi]| {
                let o = if lo <= hi && occupied(lo, hi) {
                    1.0
                } else {
                    0.0
                };
                [lo, hi, o, 0.0]
            })
            .collect()
    }
}

fn layer_ranges<T: Copy + Into<f32>>(
    vol: &Volume,
    v: &[T],
    bz: u32,
    grid: [u32; 3],
    out: &mut [[f32; 2]],
) {
    let [w, h, d] = vol.geometry.dims;
    let span = |b: u32, n: u32| (b * BRICK).min(n - 1)..=((b + 1) * BRICK).min(n - 1);
    for k in span(bz, d) {
        if !vol.loaded[k as usize] {
            continue;
        }
        for j in 0..h {
            // Rows on a brick boundary belong to both bricks.
            let by0 = (j / BRICK).min(grid[1] - 1);
            let by1 = if j % BRICK == 0 && j > 0 {
                by0 - 1
            } else {
                by0
            };
            let row = &v[((k * h + j) * w) as usize..][..w as usize];
            for bx in 0..grid[0] {
                let mut lo = f32::INFINITY;
                let mut hi = f32::NEG_INFINITY;
                let r = span(bx, w);
                for &x in &row[*r.start() as usize..=*r.end() as usize] {
                    let x: f32 = x.into();
                    lo = lo.min(x);
                    hi = hi.max(x);
                }
                for by in by1..=by0 {
                    let o = &mut out[(by * grid[0] + bx) as usize];
                    o[0] = o[0].min(lo);
                    o[1] = o[1].max(hi);
                }
            }
        }
    }
}

/// Each brick's range widened by its 26 neighbours'.
fn dilate(raw: &[[f32; 2]], grid: [u32; 3]) -> Vec<[f32; 2]> {
    let [gx, gy, gz] = grid.map(|g| g as i64);
    let at = |x: i64, y: i64, z: i64| raw[((z * gy + y) * gx + x) as usize];
    let mut out = vec![[f32::INFINITY, f32::NEG_INFINITY]; raw.len()];
    for z in 0..gz {
        for y in 0..gy {
            for x in 0..gx {
                let o = &mut out[((z * gy + y) * gx + x) as usize];
                for zz in (z - 1).max(0)..=(z + 1).min(gz - 1) {
                    for yy in (y - 1).max(0)..=(y + 1).min(gy - 1) {
                        for xx in (x - 1).max(0)..=(x + 1).min(gx - 1) {
                            let r = at(xx, yy, zz);
                            o[0] = o[0].min(r[0]);
                            o[1] = o[1].max(r[1]);
                        }
                    }
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_dicom::volume::phantom;

    #[test]
    fn ranges_cover_every_voxel_of_the_neighbourhood() {
        let vol = phantom::volume([40, 36, 20], 8.0, 12.0, 0.0);
        let b = Bricks::compute(&vol);
        assert_eq!(b.grid, [3, 3, 2]);
        let [w, h, d] = vol.geometry.dims;
        for k in 0..d {
            for j in 0..h {
                for i in 0..w {
                    let x = vol.value(i, j, k);
                    // Every brick whose dilated cover holds the voxel.
                    let near = |v: u32, g: u32| {
                        let c = (v / BRICK).min(g - 1);
                        c.saturating_sub(1)..=(c + 1).min(g - 1)
                    };
                    for bz in near(k, b.grid[2]) {
                        for by in near(j, b.grid[1]) {
                            for bx in near(i, b.grid[0]) {
                                let r = b.range[((bz * b.grid[1] + by) * b.grid[0] + bx) as usize];
                                assert!(r[0] <= x && x <= r[1]);
                            }
                        }
                    }
                }
            }
        }
    }
}
