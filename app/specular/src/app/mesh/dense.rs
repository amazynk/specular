//! Dense connectivity (CIFTI `.dconn.nii`) on its surfaces: the surfaces open coloured by
//! the first seed's row; a click on them (not a drag) picks the vertex under the pointer,
//! reads that vertex's seed row from the file (only that row, through the map) and
//! recolours the surfaces with it. The readout names the seed shown.

use std::sync::Arc;
use std::time::Instant;

use specular_decode::MeshModel;
use specular_decode::neuro_dec::Connectivity;
use specular_decode::specular_mesh::Mesh;
use specular_decode::specular_mesh::preview::canonical;
use specular_i18n::t;

/// The connectivity file behind the shown surfaces and the seed whose row colours them.
pub(in crate::app) struct Dense {
    conn: Arc<Connectivity>,
    seed: u32,
}

/// What a click on the surfaces did.
#[derive(Debug)]
pub(in crate::app) enum Picked {
    /// The pointer was not over the surfaces.
    Missed,
    /// It was over a vertex no seed belongs to (outside the medial wall's mask, say).
    NoSeed,
    /// The model recoloured with the picked seed's row.
    Seed(MeshModel),
}

impl Dense {
    /// The surfaces open showing `seed`'s row.
    pub(in crate::app) fn new(conn: Arc<Connectivity>, seed: u32) -> Dense {
        Dense { conn, seed }
    }

    /// The readout lines: the seed shown and how to pick another.
    pub(in crate::app) fn lines(&self) -> [String; 2] {
        [
            t!(
                "dconn-seed",
                seed = specular_i18n::isolate_ltr(&self.conn.seed_name(self.seed))
            ),
            t!("dconn-pick-hint"),
        ]
    }

    /// The seed under the ray from `o` along `d` (canonical frame) on `model`'s surfaces:
    /// its row read and `model` recoloured with it.
    pub(in crate::app) fn pick(
        &mut self,
        model: &MeshModel,
        o: [f64; 3],
        d: [f64; 3],
    ) -> Result<Picked, String> {
        let t0 = Instant::now();
        let Some(v) = nearest_vertex(&model.mesh, o, d) else {
            return Ok(Picked::Missed);
        };
        let Some(seed) = self.conn.seed_at_vertex(v) else {
            return Ok(Picked::NoSeed);
        };
        let (colours, _) = self.conn.surface(seed).map_err(|e| e.to_string())?;
        let mut mesh = (*model.mesh).clone();
        mesh.colors = Some(colours);
        self.seed = seed;
        specular_log::info!(
            target: specular_log::DECODE,
            "dconn: seed {seed} (vertex {v}) picked and shown in {:.1} ms",
            t0.elapsed().as_secs_f64() * 1e3
        );
        Ok(Picked::Seed(MeshModel {
            mesh: Arc::new(mesh),
            textures: model.textures.clone(),
            ifc: None,
        }))
    }
}

/// The vertex of the triangle the ray from `o` along `d` meets first that is nearest the
/// hit point; positions taken to the canonical frame the camera works in.
pub(in crate::app) fn nearest_vertex(mesh: &Mesh, o: [f64; 3], d: [f64; 3]) -> Option<u32> {
    let p = |i: u32| -> Option<[f64; 3]> {
        let q = mesh.positions.get(i as usize)?;
        Some(canonical(*q, mesh.up).map(f64::from))
    };
    let mut best: Option<(f64, [u32; 3], [f64; 3])> = None;
    for tri in mesh.indices.chunks_exact(3) {
        let ids = [tri[0], tri[1], tri[2]];
        let (Some(a), Some(b), Some(c)) = (p(ids[0]), p(ids[1]), p(ids[2])) else {
            continue;
        };
        if let Some((t, u, v)) = ray_triangle(o, d, a, b, c)
            && best.is_none_or(|(bt, ..)| t < bt)
        {
            best = Some((t, ids, [1.0 - u - v, u, v]));
        }
    }
    let (_, ids, w) = best?;
    // The corner with the largest barycentric weight is the nearest to the hit.
    let k = (0..3).max_by(|&i, &j| w[i].total_cmp(&w[j]))?;
    Some(ids[k])
}

/// Möller–Trumbore: the distance along the ray and the barycentric (u, v) of where it
/// meets triangle `a b c` from either side, when it does in front of `o`.
fn ray_triangle(
    o: [f64; 3],
    d: [f64; 3],
    a: [f64; 3],
    b: [f64; 3],
    c: [f64; 3],
) -> Option<(f64, f64, f64)> {
    let sub = |x: [f64; 3], y: [f64; 3]| [x[0] - y[0], x[1] - y[1], x[2] - y[2]];
    let cross = |x: [f64; 3], y: [f64; 3]| {
        [
            x[1] * y[2] - x[2] * y[1],
            x[2] * y[0] - x[0] * y[2],
            x[0] * y[1] - x[1] * y[0],
        ]
    };
    let dot = |x: [f64; 3], y: [f64; 3]| x[0] * y[0] + x[1] * y[1] + x[2] * y[2];
    let e1 = sub(b, a);
    let e2 = sub(c, a);
    let pv = cross(d, e2);
    let det = dot(e1, pv);
    if det.abs() < 1e-12 {
        return None;
    }
    let inv = 1.0 / det;
    let tv = sub(o, a);
    let u = dot(tv, pv) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let qv = cross(tv, e1);
    let v = dot(d, qv) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = dot(e2, qv) * inv;
    (t >= 0.0 && t.is_finite()).then_some((t, u, v))
}

#[cfg(test)]
mod tests {
    use specular_decode::specular_mesh::Up;

    use super::*;

    fn quads() -> Mesh {
        // Two unit squares facing +z, one at z = 0 and one in front of it at z = 1,
        // shifted so only part of the far one is hidden.
        Mesh {
            positions: vec![
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [1.0, 1.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.5, 0.0, 1.0],
                [1.5, 0.0, 1.0],
                [1.5, 1.0, 1.0],
                [0.5, 1.0, 1.0],
            ],
            indices: vec![0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7],
            up: Up::Z,
            ..Mesh::default()
        }
    }

    #[test]
    fn the_nearest_surface_and_corner_are_picked() {
        let m = quads();
        let down = [0.0, 0.0, -1.0];
        // Only the far square is under (0.1, 0.1): its corner 0.
        assert_eq!(nearest_vertex(&m, [0.1, 0.1, 5.0], down), Some(0));
        // Both are under (0.9, 0.9): the near one wins, its corner 7 at (0.5, 1).
        assert_eq!(nearest_vertex(&m, [0.6, 0.9, 5.0], down), Some(7));
        assert_eq!(nearest_vertex(&m, [1.4, 0.1, 5.0], down), Some(5));
        // From behind, the far square is first.
        assert_eq!(
            nearest_vertex(&m, [0.9, 0.9, -5.0], [0.0, 0.0, 1.0]),
            Some(2)
        );
        // Beside both, and pointing away.
        assert_eq!(nearest_vertex(&m, [3.0, 3.0, 5.0], down), None);
        assert_eq!(nearest_vertex(&m, [0.1, 0.1, 5.0], [0.0, 0.0, 1.0]), None);
    }

    /// On the decoder's fixture (two small hemispheres, a seed per vertex but the two of the
    /// left medial wall): a ray at a vertex along its normal shows that vertex's seed row,
    /// coloured as the decoder colours it; on a medial-wall vertex nothing changes.
    #[test]
    fn a_click_on_a_vertex_shows_its_seed_row() {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../crates/formats/specular-neuro/tests/data/sub.dconn.nii");
        let s = specular_decode::Registry::default()
            .open_path(&path)
            .unwrap();
        let model = s.mesh().cloned().unwrap();
        let conn = s.neuro().unwrap().connectivity.clone().unwrap().0;
        let mut dense = Dense::new(conn.clone(), 0);
        let m = &model.mesh;
        let (mut shown, mut seedless) = (0, 0);
        for v in 0..m.positions.len() as u32 {
            let p = canonical(m.positions[v as usize], m.up).map(f64::from);
            let n = canonical(m.normals[v as usize], m.up).map(f64::from);
            let o = std::array::from_fn(|k| p[k] + 0.01 * n[k]);
            let d = n.map(|x| -x);
            // Only where the vertex itself is what the ray meets first.
            if nearest_vertex(m, o, d) != Some(v) {
                continue;
            }
            match dense.pick(&model, o, d).unwrap() {
                Picked::Seed(got) => {
                    let seed = conn.seed_at_vertex(v).unwrap();
                    let (want, _) = conn.surface(seed).unwrap();
                    assert_eq!(got.mesh.colors.as_deref(), Some(&want[..]), "vertex {v}");
                    assert_eq!(got.mesh.positions, m.positions);
                    assert_eq!(dense.seed, seed);
                    assert!(dense.lines()[0].contains(&conn.seed_name(seed)));
                    shown += 1;
                }
                Picked::NoSeed => {
                    assert!(conn.seed_at_vertex(v).is_none(), "vertex {v}");
                    seedless += 1;
                }
                Picked::Missed => panic!("vertex {v} missed"),
            }
        }
        assert!(shown >= 40, "{shown}");
        assert!(seedless <= 2);
        let far = [1e6, 1e6, 1e6];
        assert!(matches!(
            dense.pick(&model, far, [1.0, 0.0, 0.0]).unwrap(),
            Picked::Missed
        ));
    }

    #[test]
    fn y_up_files_are_picked_in_the_camera_frame() {
        let mut m = quads();
        m.up = Up::Y;
        // canonical (x, y, z) = (x, -z, y): the near square (file z = 1) is at y = -1,
        // seen looking along +y from in front.
        assert_eq!(
            nearest_vertex(&m, [1.4, -5.0, 0.1], [0.0, 1.0, 0.0]),
            Some(5)
        );
    }
}
