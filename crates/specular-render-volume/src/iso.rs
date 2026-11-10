//! Isosurfaces of a [`Volume`] for export (binary STL, patient mm).
//!
//! Marching tetrahedra: each cell between eight voxel centres splits into six tetrahedra
//! around its main diagonal (the same split in every cell, so neighbouring cells' faces
//! agree), and each tetrahedron cuts the surface as one triangle or two. Unlike marching
//! cubes there are no ambiguous cases, so the surface is closed and every edge joins exactly
//! two triangles. A layer of "outside" around the volume closes surfaces that reach its
//! edge. Vertices are placed in patient coordinates through [`Geometry::to_patient`], exact
//! under gantry tilt and uneven spacing.
//!
//! [`Mesh::smooth`] is Taubin's λ|μ smoothing, which removes the voxel staircase without
//! the shrinkage of plain Laplacian smoothing.

use std::collections::HashMap;
use std::io::{self, Write};

use specular_dicom::volume::{Geometry, Volume};

use crate::{cross, dot};

/// A triangle mesh in patient coordinates (mm, LPS), triangles counter-clockwise seen from
/// outside.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mesh {
    pub vertices: Vec<[f64; 3]>,
    pub triangles: Vec<[u32; 3]>,
}

/// Cell corner `c` (bits: 1 = +i, 2 = +j, 4 = +k).
fn bits(c: u8) -> [i64; 3] {
    [
        i64::from(c & 1),
        i64::from(c >> 1 & 1),
        i64::from(c >> 2 & 1),
    ]
}

/// The six tetrahedra of a cell: 0 → one axis → two axes → 7.
const TETS: [[u8; 4]; 6] = [
    [0, 1, 3, 7],
    [0, 1, 5, 7],
    [0, 2, 3, 7],
    [0, 2, 6, 7],
    [0, 4, 5, 7],
    [0, 4, 6, 7],
];

/// A triangle as three cell edges (corner pairs, lower corner first).
type Tri = [(u8, u8); 3];

/// For each tetrahedron and inside mask (bit n: its corner n is inside), the triangles,
/// oriented to face from the inside corners to the outside ones.
fn cases() -> Vec<Vec<Tri>> {
    let mut out = Vec::with_capacity(6 * 16);
    for tet in TETS {
        for mask in 0u8..16 {
            let inside: Vec<u8> = (0..4)
                .filter(|n| mask >> n & 1 == 1)
                .map(|n| tet[n])
                .collect();
            let outside: Vec<u8> = (0..4)
                .filter(|n| mask >> n & 1 == 0)
                .map(|n| tet[n])
                .collect();
            let e = |a: u8, b: u8| (a.min(b), a.max(b));
            let tris: Vec<Tri> = match (inside.as_slice(), outside.as_slice()) {
                ([p], [q, r, s]) | ([q, r, s], [p]) => vec![[e(*p, *q), e(*p, *r), e(*p, *s)]],
                ([p, q], [r, s]) => vec![
                    [e(*p, *r), e(*p, *s), e(*q, *s)],
                    [e(*p, *r), e(*q, *s), e(*q, *r)],
                ],
                _ => Vec::new(),
            };
            // Orient by the edge midpoints in cell coordinates: never degenerate, and the
            // same rule in every tetrahedron keeps neighbouring triangles consistent.
            let centroid = |cs: &[u8]| {
                let mut c = [0.0; 3];
                for &k in cs {
                    let b = bits(k);
                    for i in 0..3 {
                        c[i] += b[i] as f64 / cs.len() as f64;
                    }
                }
                c
            };
            let away = sub(centroid(&outside), centroid(&inside));
            let mid = |(a, b): (u8, u8)| {
                let (a, b) = (bits(a), bits(b));
                [0, 1, 2].map(|i| (a[i] + b[i]) as f64 / 2.0)
            };
            out.push(
                tris.into_iter()
                    .map(|t| {
                        let n = cross(sub(mid(t[1]), mid(t[0])), sub(mid(t[2]), mid(t[0])));
                        if dot(n, away) < 0.0 {
                            [t[0], t[2], t[1]]
                        } else {
                            t
                        }
                    })
                    .collect(),
            );
        }
    }
    out
}

/// One thread's part of the surface: vertices keyed by lattice edge.
#[derive(Default)]
struct Part {
    keys: Vec<u64>,
    vertices: Vec<[f64; 3]>,
    triangles: Vec<[u32; 3]>,
}

/// The surface where `vol` crosses `threshold` (voxels at or above it are inside), closed at
/// the volume's edges. Slices not loaded count as outside. Runs on all cores.
pub fn isosurface(vol: &Volume, threshold: f32) -> Mesh {
    let [w, h, d] = vol.geometry.dims.map(i64::from);
    let field = |i: i64, j: i64, k: i64| -> Option<f32> {
        let inside = i >= 0 && i < w && j >= 0 && j < h && k >= 0 && k < d;
        (inside && vol.loaded[k as usize])
            .then(|| vol.value(i as u32, j as u32, k as u32) - threshold)
    };
    surface(&vol.geometry, &field)
}

/// The closed surface around the voxels of `vol` for which `inside` holds (a label or
/// segment mask), through the midpoints between inside and outside voxel centres. Slices not
/// loaded count as outside. Runs on all cores.
pub fn label_surface(vol: &Volume, inside: &(dyn Fn(f32) -> bool + Sync)) -> Mesh {
    let [w, h, d] = vol.geometry.dims.map(i64::from);
    let field = |i: i64, j: i64, k: i64| -> Option<f32> {
        let ok = i >= 0 && i < w && j >= 0 && j < h && k >= 0 && k < d;
        (ok && vol.loaded[k as usize]).then(|| {
            if inside(vol.value(i as u32, j as u32, k as u32)) {
                0.5
            } else {
                -0.5
            }
        })
    };
    surface(&vol.geometry, &field)
}

/// A field over the lattice: the value less the threshold at voxel (i, j, k); `None` outside
/// the grid or where nothing is loaded (outside the surface).
type Field<'a> = dyn Fn(i64, i64, i64) -> Option<f32> + Sync + 'a;

/// The surface where `field` crosses 0 on the lattice of `g`, closed at its edges.
fn surface(g: &Geometry, field: &Field) -> Mesh {
    let d = i64::from(g.dims[2]);
    let table = cases();
    // Cell layers k = -1 ..= d - 1 (cells between lattice layers k and k + 1).
    let layers: Vec<i64> = (-1..d).collect();
    let threads = std::thread::available_parallelism()
        .map_or(4, |n| n.get())
        .min(layers.len().max(1));
    let per = layers.len().div_ceil(threads);
    let parts: Vec<Part> = std::thread::scope(|s| {
        let handles: Vec<_> = layers
            .chunks(per)
            .enumerate()
            .map(|(n, chunk)| {
                let table = &table;
                specular_thread::spawn_scoped(s, format!("vr-iso-{n}"), move || {
                    march(g, field, chunk, table)
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("isosurface thread"))
            .collect()
    });
    // Merge, joining the vertices threads share at their boundaries.
    let mut index: HashMap<u64, u32> = HashMap::new();
    let mut mesh = Mesh::default();
    for part in parts {
        let remap: Vec<u32> = part
            .keys
            .iter()
            .zip(&part.vertices)
            .map(|(k, v)| {
                *index.entry(*k).or_insert_with(|| {
                    mesh.vertices.push(*v);
                    (mesh.vertices.len() - 1) as u32
                })
            })
            .collect();
        mesh.triangles
            .extend(part.triangles.iter().map(|t| t.map(|i| remap[i as usize])));
    }
    mesh
}

fn march(g: &Geometry, field: &Field, layers: &[i64], table: &[Vec<Tri>]) -> Part {
    let [w, h, d] = g.dims.map(i64::from);
    let (pw, ph) = (w + 2, h + 2);
    // Inside flags of lattice layer k (padded by one outside point all round).
    let layer = |k: i64| -> Vec<bool> {
        let mut out = vec![false; (pw * ph) as usize];
        if k < 0 || k >= d {
            return out;
        }
        for j in 0..h {
            for i in 0..w {
                out[((j + 1) * pw + i + 1) as usize] = field(i, j, k).is_some_and(|v| v >= 0.0);
            }
        }
        out
    };
    let value = |p: [i64; 3]| -> Option<f32> { field(p[0], p[1], p[2]) };
    let mut part = Part::default();
    let mut local: HashMap<u64, u32> = HashMap::new();
    let Some(&first) = layers.first() else {
        return part;
    };
    let mut below = layer(first);
    for &k in layers {
        let above = layer(k + 1);
        for j in -1..h {
            for i in -1..w {
                let at = |c: u8| {
                    let b = bits(c);
                    let idx = ((j + 1 + b[1]) * pw + i + 1 + b[0]) as usize;
                    if b[2] == 0 { below[idx] } else { above[idx] }
                };
                let mut mask = 0u8;
                for c in 0..8 {
                    mask |= u8::from(at(c)) << c;
                }
                if mask == 0 || mask == 255 {
                    continue;
                }
                for (t, tet) in TETS.iter().enumerate() {
                    let mut m = 0usize;
                    for (n, &c) in tet.iter().enumerate() {
                        m |= usize::from(mask >> c & 1) << n;
                    }
                    for tri in &table[t * 16 + m] {
                        let tri = tri.map(|(a, b)| {
                            let (pa, pb) = (bits(a), bits(b));
                            let lo = [i + pa[0], j + pa[1], k + pa[2]];
                            let off = (b ^ a) as u64;
                            let point = (((lo[2] + 1) * ph + lo[1] + 1) * pw + lo[0] + 1) as u64;
                            let key = point * 7 + off - 1;
                            *local.entry(key).or_insert_with(|| {
                                let hi = [
                                    lo[0] + pb[0] - pa[0],
                                    lo[1] + pb[1] - pa[1],
                                    lo[2] + pb[2] - pa[2],
                                ];
                                part.keys.push(key);
                                part.vertices.push(vertex(g, lo, hi, value(lo), value(hi)));
                                (part.vertices.len() - 1) as u32
                            })
                        });
                        part.triangles.push(tri);
                    }
                }
            }
        }
        below = above;
    }
    part
}

/// Where the surface crosses the lattice edge from `a` to `b` (one inside, one outside):
/// linear between the values, halfway when one end is the outside padding.
fn vertex(g: &Geometry, a: [i64; 3], b: [i64; 3], fa: Option<f32>, fb: Option<f32>) -> [f64; 3] {
    let t = match (fa, fb) {
        (Some(x), Some(y)) if x != y => (f64::from(x) / f64::from(x - y)).clamp(0.0, 1.0),
        _ => 0.5,
    };
    let pa = g.to_patient(a.map(|c| c as f64));
    let pb = g.to_patient(b.map(|c| c as f64));
    [0, 1, 2].map(|c| pa[c] + (pb[c] - pa[c]) * t)
}

impl Mesh {
    /// Taubin smoothing: `passes` pairs of a shrinking (λ = 0.5) and an inflating
    /// (μ = −0.53) Laplacian step.
    pub fn smooth(&mut self, passes: u32) {
        let n = self.vertices.len();
        let mut edges: Vec<(u32, u32)> = Vec::with_capacity(self.triangles.len() * 3);
        for t in &self.triangles {
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                edges.push((a.min(b), a.max(b)));
            }
        }
        edges.sort_unstable();
        edges.dedup();
        let mut start = vec![0usize; n + 1];
        for &(a, b) in &edges {
            start[a as usize + 1] += 1;
            start[b as usize + 1] += 1;
        }
        for i in 0..n {
            start[i + 1] += start[i];
        }
        let mut fill = start.clone();
        let mut nbr = vec![0u32; start[n]];
        for &(a, b) in &edges {
            nbr[fill[a as usize]] = b;
            fill[a as usize] += 1;
            nbr[fill[b as usize]] = a;
            fill[b as usize] += 1;
        }
        let mut next = self.vertices.clone();
        for _ in 0..passes {
            for f in [0.5, -0.53] {
                for (i, out) in next.iter_mut().enumerate() {
                    let ns = &nbr[start[i]..start[i + 1]];
                    let p = self.vertices[i];
                    if ns.is_empty() {
                        *out = p;
                        continue;
                    }
                    let mut m = [0.0; 3];
                    for &j in ns {
                        let q = self.vertices[j as usize];
                        for c in 0..3 {
                            m[c] += q[c];
                        }
                    }
                    let k = ns.len() as f64;
                    *out = [0, 1, 2].map(|c| p[c] + f * (m[c] / k - p[c]));
                }
                std::mem::swap(&mut self.vertices, &mut next);
            }
        }
    }

    /// The enclosed volume, mm³ (positive for an outward-facing closed mesh).
    pub fn volume(&self) -> f64 {
        self.triangles
            .iter()
            .map(|t| {
                let [a, b, c] = t.map(|i| self.vertices[i as usize]);
                dot(a, cross(b, c)) / 6.0
            })
            .sum()
    }

    /// Writes binary STL (coordinates in patient mm).
    pub fn write_stl(&self, out: &mut impl Write) -> io::Result<()> {
        self.write_stl_titled(out, "Specular isosurface, patient coordinates (LPS), mm")
    }

    /// Writes binary STL with `title` in its 80-byte header (ASCII, cut to fit; never
    /// starting with "solid", which some readers take for ASCII STL).
    pub fn write_stl_titled(&self, out: &mut impl Write, title: &str) -> io::Result<()> {
        let mut header = [b' '; 80];
        let mut text: Vec<u8> = title
            .chars()
            .map(|c| {
                if c.is_ascii() && !c.is_ascii_control() {
                    c as u8
                } else {
                    b'?'
                }
            })
            .take(80)
            .collect();
        if text.len() >= 5 && text[..5].eq_ignore_ascii_case(b"solid") {
            text[0] = b'_';
        }
        header[..text.len()].copy_from_slice(&text);
        out.write_all(&header)?;
        out.write_all(&(self.triangles.len() as u32).to_le_bytes())?;
        let mut buf = Vec::with_capacity(50 * 4096);
        for t in &self.triangles {
            let [a, b, c] = t.map(|i| self.vertices[i as usize]);
            let n = cross(sub(b, a), sub(c, a));
            let l = dot(n, n).sqrt();
            let n = if l > 0.0 { n.map(|x| x / l) } else { [0.0; 3] };
            for v in [n, a, b, c] {
                for x in v {
                    buf.extend_from_slice(&(x as f32).to_le_bytes());
                }
            }
            buf.extend_from_slice(&[0, 0]);
            if buf.len() >= 50 * 4096 {
                out.write_all(&buf)?;
                buf.clear();
            }
        }
        out.write_all(&buf)
    }
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_dicom::volume::phantom;

    /// Every edge joins exactly two triangles, once in each direction.
    fn assert_closed(m: &Mesh) {
        let mut directed: HashMap<(u32, u32), u32> = HashMap::new();
        for t in &m.triangles {
            assert!(t[0] != t[1] && t[1] != t[2] && t[0] != t[2]);
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                *directed.entry((a, b)).or_default() += 1;
            }
        }
        for (&(a, b), &n) in &directed {
            assert_eq!(n, 1, "edge {a}→{b} used {n} times");
            assert_eq!(directed.get(&(b, a)), Some(&1), "edge {a}→{b} has no twin");
        }
    }

    fn bone_sphere(tilt: f64) {
        // Only the bone sphere (1000 HU) is above 520 HU.
        let vol = phantom::volume([128, 48, 48], 1.5, 1.5, tilt);
        let mut m = isosurface(&vol, 520.0);
        assert!(m.triangles.len() > 1000);
        assert_closed(&m);
        let (_, r, _) = phantom::SPHERES[0];
        let want = 4.0 / 3.0 * std::f64::consts::PI * r * r * r;
        let got = m.volume();
        eprintln!(
            "tilt {tilt}: {} triangles, {got:.0} mm³ (sphere {want:.0})",
            m.triangles.len()
        );
        assert!((got / want - 1.0).abs() < 0.02, "volume {got} vs {want}");
        // Every vertex lies near the sphere.
        let c = phantom::SPHERES[0].0;
        for v in &m.vertices {
            let d = sub(*v, c);
            assert!((dot(d, d).sqrt() - r).abs() < 1.5, "vertex {v:?}");
        }
        m.smooth(10);
        assert_closed(&m);
        let smoothed = m.volume();
        assert!(
            (smoothed / want - 1.0).abs() < 0.03,
            "smoothed {smoothed} vs {want}"
        );
        let mut stl = Vec::new();
        m.write_stl(&mut stl).unwrap();
        assert_eq!(stl.len(), 84 + 50 * m.triangles.len());
        assert_eq!(
            u32::from_le_bytes(stl[80..84].try_into().unwrap()) as usize,
            m.triangles.len()
        );
    }

    #[test]
    fn bone_sphere_is_closed_with_the_analytic_volume() {
        bone_sphere(0.0);
    }

    #[test]
    fn tilted_bone_sphere_is_closed_with_the_analytic_volume() {
        bone_sphere(20.0);
    }

    #[test]
    fn surfaces_close_at_the_volume_edge() {
        // Tissue fills the volume's middle to its i and j edges: the padding closes it.
        let vol = phantom::volume([24, 24, 10], 4.0, 4.0, 0.0);
        let m = isosurface(&vol, -500.0);
        assert_closed(&m);
        // A box of (24·4) × (24·4) × (10·4) mm at most, minus the cylinder's corners.
        let v = m.volume();
        assert!(v > 0.0 && v <= 96.0 * 96.0 * 40.0 * 1.01, "{v}");
    }
}
