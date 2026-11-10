//! The mesh as triangles to draw: faces fan-triangulated, longitudes brought into one
//! 360° window, faces across the antimeridian split into copies on either side, and faces
//! around a pole closed to it. A node on a pole (an icosahedral grid's vertex) has no
//! longitude of its own: each face meeting it takes it at its neighbours' longitudes, so
//! the face reaches the pole as a band, not a sliver to wherever the file put it.

use crate::{Coords, Mesh};

/// Drawable triangles over display vertices.
#[derive(Clone, Debug, Default)]
pub struct Display {
    /// Vertex positions: longitude and latitude in degrees (longitude within
    /// `[west, west + 360)` for most vertices, beyond it for the copies of split faces),
    /// or projected x and y.
    pub positions: Vec<[f64; 2]>,
    /// The mesh node each vertex is (a node's own vertex, or a copy of it).
    pub src: Vec<u32>,
    pub tris: Vec<[u32; 3]>,
    /// The face each triangle belongs to.
    pub tri_face: Vec<u32>,
    /// The faces' edges as vertex pairs, each once.
    pub edges: Vec<[u32; 2]>,
    /// Distinct node pairs along the faces' sides.
    pub edge_count: u64,
    /// What to show: min x, min y, max x, max y.
    pub extent: [f64; 4],
    /// The longitude window starts here (geographic meshes).
    pub west: f64,
    pub geographic: bool,
    /// Faces split across the antimeridian or closed to a pole.
    pub split_faces: u64,
}

fn wrap180(d: f64) -> f64 {
    (d + 180.0).rem_euclid(360.0) - 180.0
}

/// The start of the longitude window: the east side of the widest band of longitude no
/// face covers (a regional mesh, kept in one piece even across the antimeridian), else
/// -180 (a global mesh).
fn window(mesh: &Mesh) -> f64 {
    // Each face's longitude interval, starting in [-180, 180).
    let mut spans: Vec<(f64, f64)> = Vec::with_capacity(mesh.faces());
    for f in 0..mesh.faces() {
        let nodes = mesh.face(f);
        if nodes.len() < 3 {
            continue;
        }
        let (mut lo, mut hi) = (f64::MAX, f64::MIN);
        for &n in nodes {
            let l = wrap180(mesh.nodes[n as usize][0]);
            lo = lo.min(l);
            hi = hi.max(l);
        }
        if !(lo.is_finite() && hi.is_finite()) {
            continue;
        }
        if hi - lo > 180.0 {
            // Across the antimeridian: from its westmost node in the eastern hemisphere
            // to its eastmost one in the western.
            let ls = nodes.iter().map(|&n| wrap180(mesh.nodes[n as usize][0]));
            let east = ls.clone().filter(|&l| l >= 0.0).fold(f64::MAX, f64::min);
            let west = ls.filter(|&l| l < 0.0).fold(f64::MIN, f64::max);
            spans.push((east, west + 360.0));
        } else {
            spans.push((lo, hi));
        }
    }
    if spans.is_empty() {
        return -180.0;
    }
    spans.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
    let first = spans[0].0;
    let mut end = first;
    let mut best = (0.0, -180.0);
    for turn in [0.0, 360.0] {
        for &(a, b) in &spans {
            let (a, b) = (a + turn, b + turn);
            if a >= first + 360.0 {
                break;
            }
            if a - end > best.0 {
                best = (a - end, a);
            }
            end = end.max(b);
        }
    }
    if first + 360.0 - end > best.0 {
        best = (first + 360.0 - end, first);
    }
    if best.0 > 1e-9 {
        wrap180(best.1)
    } else {
        -180.0
    }
}

/// A latitude on a pole.
fn on_pole(lat: f64) -> bool {
    lat.abs() >= 90.0 - 1e-9
}

/// A face with a node on a pole, as a ring of (unwrapped longitude, latitude, node): each
/// pole node in it twice, at the longitudes of the nodes before and after it. `None` when
/// every node is on the pole.
fn polar_ring(positions: &[[f64; 2]], nodes: &[u32]) -> Option<Vec<(f64, f64, u32)>> {
    let lat = |n: u32| positions[n as usize][1];
    let first = nodes.iter().position(|&n| !on_pole(lat(n)))?;
    let len = nodes.len();
    let order: Vec<u32> = (0..len).map(|k| nodes[(first + k) % len]).collect();
    // Unwrapped longitudes of the nodes off the pole, in ring order.
    let mut lon = vec![f64::NAN; len];
    let mut u = positions[order[0] as usize][0];
    lon[0] = u;
    for (k, &n) in order.iter().enumerate().skip(1) {
        if !on_pole(lat(n)) {
            u += wrap180(positions[n as usize][0] - u);
            lon[k] = u;
        }
    }
    let mut ring = Vec::with_capacity(len + 2);
    for (k, &n) in order.iter().enumerate() {
        if !on_pole(lat(n)) {
            ring.push((lon[k], lat(n), n));
            continue;
        }
        let before = (0..k)
            .rev()
            .find_map(|j| Some(lon[j]).filter(|l| l.is_finite()));
        let after = (k + 1..len).find_map(|j| Some(lon[j]).filter(|l| l.is_finite()));
        // Past the last node, the ring closes on the first one (a turn on if need be).
        let after = after.unwrap_or_else(|| u + wrap180(lon[0] - u));
        let before = before.unwrap_or(after);
        let pole = 90.0_f64.copysign(lat(n));
        ring.push((before, pole, n));
        if (after - before).abs() > 1e-12 {
            ring.push((after, pole, n));
        }
    }
    Some(ring)
}

/// The longitude span of a face's nodes.
fn span(positions: &[[f64; 2]], nodes: &[u32]) -> f64 {
    let (lo, hi) = nodes.iter().fold((f64::MAX, f64::MIN), |(a, b), &n| {
        let x = positions[n as usize][0];
        (a.min(x), b.max(x))
    });
    hi - lo
}

impl Display {
    /// Triangulates a mesh. Faces with fewer than three nodes or with a node without
    /// coordinates are left out.
    pub fn build(mesh: &Mesh) -> Display {
        let geographic = matches!(mesh.coords, Coords::LonLat);
        let west = if geographic { window(mesh) } else { 0.0 };
        let mut positions: Vec<[f64; 2]> = if geographic {
            mesh.nodes
                .iter()
                .map(|p| [west + (p[0] - west).rem_euclid(360.0), p[1]])
                .collect()
        } else {
            mesh.nodes.clone()
        };
        let mut src: Vec<u32> = (0..positions.len() as u32).collect();
        let faces = mesh.faces();
        let tri_count = (0..faces)
            .map(|f| mesh.face(f).len().saturating_sub(2))
            .sum();
        let mut tris = Vec::with_capacity(tri_count);
        let mut tri_face = Vec::with_capacity(tri_count);
        let mut extra_edges: Vec<[u32; 2]> = Vec::new();
        let mut split_faces = 0;
        let mut lon = Vec::new();
        let ok = |p: [f64; 2]| p[0].is_finite() && p[1].is_finite();
        for f in 0..faces {
            let nodes = mesh.face(f);
            if nodes.len() < 3 || !nodes.iter().all(|&n| ok(positions[n as usize])) {
                continue;
            }
            if geographic && nodes.iter().any(|&n| on_pole(positions[n as usize][1])) {
                // A face meeting a pole node: drawn from its ring with the pole node at its
                // neighbours' longitudes, copied a turn over where it passes the window.
                let Some(ring) = polar_ring(&positions, nodes) else {
                    continue;
                };
                split_faces += 1;
                let (lo, hi) = ring
                    .iter()
                    .fold((f64::MAX, f64::MIN), |(a, b), r| (a.min(r.0), b.max(r.0)));
                let m = ring.len() as u32;
                for shift in [0.0, 360.0, -360.0] {
                    if !(lo + shift < west + 360.0 && hi + shift > west) {
                        continue;
                    }
                    let base = positions.len() as u32;
                    for &(x, y, n) in &ring {
                        positions.push([x + shift, y]);
                        src.push(n);
                    }
                    for k in 1..m - 1 {
                        tris.push([base, base + k, base + k + 1]);
                        tri_face.push(f as u32);
                    }
                    for k in 0..m {
                        let (a, b) = (ring[k as usize], ring[((k + 1) % m) as usize]);
                        // The pole itself is no side.
                        if !(a.2 == b.2 && on_pole(a.1)) {
                            extra_edges.push([base + k, base + (k + 1) % m]);
                        }
                    }
                }
                continue;
            }
            let crossing = geographic && span(&positions, nodes) > 180.0;
            if !crossing {
                for k in 1..nodes.len() - 1 {
                    tris.push([nodes[0], nodes[k], nodes[k + 1]]);
                    tri_face.push(f as u32);
                }
                continue;
            }
            split_faces += 1;
            // Unwrap each step to under 180°.
            lon.clear();
            let mut u = positions[nodes[0] as usize][0];
            lon.push(u);
            for w in nodes.windows(2) {
                u += wrap180(positions[w[1] as usize][0] - positions[w[0] as usize][0]);
                lon.push(u);
            }
            let close = u + wrap180(
                positions[nodes[0] as usize][0] - positions[nodes[nodes.len() - 1] as usize][0],
            );
            let winding = close - lon[0];
            let (lo, hi) = lon
                .iter()
                .fold((f64::MAX, f64::MIN), |(a, b), &x| (a.min(x), b.max(x)));
            if winding.abs() < 180.0 {
                // A face across the antimeridian: drawn at its unwrapped longitudes, and
                // again a turn over to fill the other edge of the window.
                for shift in [0.0, 360.0, -360.0] {
                    if shift != 0.0 && !(lo + shift < west + 360.0 && hi + shift > west) {
                        continue;
                    }
                    let base = positions.len() as u32;
                    for (k, &n) in nodes.iter().enumerate() {
                        positions.push([lon[k] + shift, mesh.nodes[n as usize][1]]);
                        src.push(n);
                    }
                    for k in 1..nodes.len() as u32 - 1 {
                        tris.push([base, base + k, base + k + 1]);
                        tri_face.push(f as u32);
                    }
                    for k in 0..nodes.len() as u32 {
                        extra_edges.push([base + k, base + (k + 1) % nodes.len() as u32]);
                    }
                }
            } else {
                // A face around a pole: a curtain from each side to the pole.
                let mean_lat = nodes
                    .iter()
                    .map(|&n| mesh.nodes[n as usize][1])
                    .sum::<f64>()
                    / nodes.len() as f64;
                let pole = if mean_lat >= 0.0 { 90.0 } else { -90.0 };
                let mut ring: Vec<(f64, f64, u32)> = nodes
                    .iter()
                    .enumerate()
                    .map(|(k, &n)| (lon[k], mesh.nodes[n as usize][1], n))
                    .collect();
                ring.push((close, mesh.nodes[nodes[0] as usize][1], nodes[0]));
                let (lo, hi) = (lo.min(close), hi.max(close));
                for shift in [-360.0, 0.0, 360.0] {
                    if !(lo + shift < west + 360.0 && hi + shift > west) {
                        continue;
                    }
                    for w in ring.windows(2) {
                        let base = positions.len() as u32;
                        positions.push([w[0].0 + shift, w[0].1]);
                        positions.push([w[1].0 + shift, w[1].1]);
                        positions.push([w[1].0 + shift, pole]);
                        positions.push([w[0].0 + shift, pole]);
                        src.extend([w[0].2, w[1].2, w[1].2, w[0].2]);
                        tris.push([base, base + 1, base + 2]);
                        tris.push([base, base + 2, base + 3]);
                        tri_face.extend([f as u32, f as u32]);
                        extra_edges.push([base, base + 1]);
                    }
                }
            }
        }

        // Edges: distinct node pairs along the faces' sides; those of split faces drawn
        // from their copies.
        let mut keys: Vec<u64> = Vec::with_capacity(mesh.face_nodes.len());
        let mut keys_split: Vec<u64> = Vec::new();
        for f in 0..faces {
            let nodes = mesh.face(f);
            if nodes.len() < 2 {
                continue;
            }
            let drawable = nodes.len() >= 3 && nodes.iter().all(|&n| ok(positions[n as usize]));
            let crossing = drawable
                && geographic
                && (span(&positions, nodes) > 180.0
                    || nodes.iter().any(|&n| on_pole(positions[n as usize][1])));
            for k in 0..nodes.len() {
                let (a, b) = (nodes[k], nodes[(k + 1) % nodes.len()]);
                if a == b {
                    continue;
                }
                let key = (u64::from(a.min(b)) << 32) | u64::from(a.max(b));
                keys.push(key);
                let long = geographic
                    && (positions[a as usize][0] - positions[b as usize][0]).abs() > 180.0;
                if crossing || long || !drawable {
                    keys_split.push(key);
                }
            }
        }
        keys.sort_unstable();
        keys.dedup();
        keys_split.sort_unstable();
        keys_split.dedup();
        let edge_count = keys.len() as u64;
        let mut edges: Vec<[u32; 2]> = Vec::with_capacity(keys.len() + extra_edges.len());
        for &k in &keys {
            if keys_split.binary_search(&k).is_ok() {
                continue;
            }
            edges.push([(k >> 32) as u32, k as u32]);
        }
        edges.extend(extra_edges);

        // What to show: the drawn vertices, cut to the window.
        let mut ext = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
        let mut used = vec![false; positions.len()];
        for t in &tris {
            for &v in t {
                used[v as usize] = true;
            }
        }
        for (p, _) in positions.iter().zip(&used).filter(|(_, u)| **u) {
            ext[0] = ext[0].min(p[0]);
            ext[1] = ext[1].min(p[1]);
            ext[2] = ext[2].max(p[0]);
            ext[3] = ext[3].max(p[1]);
        }
        if ext[0] > ext[2] {
            ext = [0.0, 0.0, 1.0, 1.0];
        }
        if geographic {
            ext[0] = ext[0].max(west);
            ext[2] = ext[2].min(west + 360.0);
            ext[1] = ext[1].max(-90.0);
            ext[3] = ext[3].min(90.0);
        }
        if ext[2] - ext[0] <= 0.0 {
            ext[2] = ext[0] + 1.0;
        }
        if ext[3] - ext[1] <= 0.0 {
            ext[3] = ext[1] + 1.0;
        }
        Display {
            positions,
            src,
            tris,
            tri_face,
            edges,
            edge_count,
            extent: ext,
            west,
            geographic,
            split_faces,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fan_and_edges() {
        let nodes = vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0], [2.0, 0.0]];
        let m = Mesh::from_parts(
            nodes,
            &[&[0, 1, 2, 3], &[1, 4, 2]],
            Coords::Projected { unit: None },
        )
        .unwrap();
        let d = Display::build(&m);
        assert_eq!(d.tris.len(), 3);
        assert_eq!(d.tri_face, vec![0, 0, 1]);
        // 4 sides of the quad, 3 of the triangle, one shared.
        assert_eq!(d.edge_count, 6);
        assert_eq!(d.edges.len(), 6);
        assert_eq!(d.extent, [0.0, 0.0, 2.0, 1.0]);
    }

    #[test]
    fn antimeridian_split() {
        // A band of quads around the equator every 20°, the last from 170°E to 170°W.
        let mut nodes = Vec::new();
        for k in 0..18 {
            let l = -170.0 + 20.0 * f64::from(k);
            nodes.push([l, 0.0]);
            nodes.push([l, 10.0]);
        }
        let faces: Vec<Vec<u32>> = (0..18u32)
            .map(|k| {
                let n = (k + 1) % 18;
                vec![2 * k, 2 * n, 2 * n + 1, 2 * k + 1]
            })
            .collect();
        let refs: Vec<&[u32]> = faces.iter().map(Vec::as_slice).collect();
        let m = Mesh::from_parts(nodes, &refs, Coords::LonLat).unwrap();
        let d = Display::build(&m);
        assert_eq!(d.west, -180.0);
        assert_eq!(d.split_faces, 1);
        // 17 plain quads, and the split one twice.
        assert_eq!(d.tris.len(), 2 * 17 + 4);
        assert_eq!(d.extent, [-180.0, 0.0, 180.0, 10.0]);
        assert_eq!(d.edge_count, 18 * 3);
        for t in &d.tris {
            let xs: Vec<f64> = t.iter().map(|&v| d.positions[v as usize][0]).collect();
            let span = xs.iter().cloned().fold(f64::MIN, f64::max)
                - xs.iter().cloned().fold(f64::MAX, f64::min);
            assert!(span <= 180.0, "{xs:?}");
        }
    }

    #[test]
    fn regional_window() {
        // A regional mesh across the antimeridian stays in one piece.
        let nodes = vec![[175.0, 0.0], [-175.0, 0.0], [179.0, 5.0]];
        let m = Mesh::from_parts(nodes, &[&[0, 1, 2]], Coords::LonLat).unwrap();
        let d = Display::build(&m);
        assert_eq!(d.split_faces, 0);
        assert_eq!(d.west, 175.0);
        assert_eq!(d.extent, [175.0, 0.0, 185.0, 5.0]);
    }

    #[test]
    fn polar_cap() {
        // A square around the north pole.
        let nodes = vec![
            [0.0, 80.0],
            [90.0, 80.0],
            [180.0, 80.0],
            [-90.0, 80.0],
            [10.0, 0.0],
        ];
        let m = Mesh::from_parts(nodes, &[&[0, 1, 2, 3]], Coords::LonLat).unwrap();
        let d = Display::build(&m);
        assert_eq!(d.split_faces, 1);
        assert!(d.tris.len() >= 8);
        assert_eq!(d.extent[3], 90.0);
    }

    /// An icosahedral cap: five triangles meeting at a pole node stored at longitude 0
    /// (as ICON stores it). Each reaches the pole across its own longitudes only, and none
    /// is a sliver to longitude 0.
    #[test]
    fn pole_node() {
        let mut nodes = vec![[0.0, -90.0]];
        for k in 0..5 {
            nodes.push([-180.0 + 36.0 + 72.0 * f64::from(k), -60.0]);
        }
        let faces: Vec<Vec<u32>> = (0..5u32).map(|k| vec![0, 1 + k, 1 + (k + 1) % 5]).collect();
        let refs: Vec<&[u32]> = faces.iter().map(Vec::as_slice).collect();
        let m = Mesh::from_parts(nodes, &refs, Coords::LonLat).unwrap();
        let d = Display::build(&m);
        assert_eq!(d.split_faces, 5);
        // Every triangle spans at most its face's 72°.
        for t in &d.tris {
            let xs: Vec<f64> = t.iter().map(|&v| d.positions[v as usize][0]).collect();
            let span = xs.iter().cloned().fold(f64::MIN, f64::max)
                - xs.iter().cloned().fold(f64::MAX, f64::min);
            assert!(span <= 72.0 + 1e-9, "{xs:?}");
        }
        // The cap covers the whole window down to the pole: 360° × 30°, and the face across
        // the antimeridian once more (its copy a turn over).
        let area: f64 = d
            .tris
            .iter()
            .map(|t| {
                let [a, b, c] = t.map(|v| d.positions[v as usize]);
                ((b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1])).abs() / 2.0
            })
            .sum();
        assert!((area - 432.0 * 30.0).abs() < 1e-6, "{area}");
        assert_eq!(d.extent[1], -90.0);
        // No edge runs to the pole node's own vertex (at longitude 0).
        assert!(d.edges.iter().all(|e| !e.contains(&0)), "{:?}", d.edges);
    }
}
