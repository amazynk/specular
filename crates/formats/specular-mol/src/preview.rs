//! A whole scene as one triangle mesh (cartoon, low-polygon spheres and sticks, the unit
//! cell) and its CPU rendering for previews and thumbnails.

use specular_mesh::{Bounds, Mesh, Part, Up};

use crate::scene::{self, Scene};
use crate::{Colouring, Molecule, Style};

fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn scale(a: [f32; 3], s: f32) -> [f32; 3] {
    [a[0] * s, a[1] * s, a[2] * s]
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn unit(a: [f32; 3]) -> [f32; 3] {
    let l = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
    if l > 1e-12 {
        scale(a, 1.0 / l)
    } else {
        [0.0, 0.0, 1.0]
    }
}

/// A unit sphere: an octahedron subdivided `level` times.
fn sphere(level: u32) -> (Vec<[f32; 3]>, Vec<u32>) {
    let mut v: Vec<[f32; 3]> = vec![
        [1.0, 0.0, 0.0],
        [-1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, -1.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, -1.0],
    ];
    let mut t: Vec<u32> = vec![
        0, 2, 4, 2, 1, 4, 1, 3, 4, 3, 0, 4, 2, 0, 5, 1, 2, 5, 3, 1, 5, 0, 3, 5,
    ];
    for _ in 0..level {
        let mut mid = std::collections::HashMap::new();
        let mut nt = Vec::with_capacity(t.len() * 4);
        let mut m = |a: u32, b: u32, v: &mut Vec<[f32; 3]>| -> u32 {
            *mid.entry((a.min(b), a.max(b))).or_insert_with(|| {
                v.push(unit(add(v[a as usize], v[b as usize])));
                v.len() as u32 - 1
            })
        };
        for f in t.chunks(3) {
            let (a, b, c) = (f[0], f[1], f[2]);
            let ab = m(a, b, &mut v);
            let bc = m(b, c, &mut v);
            let ca = m(c, a, &mut v);
            nt.extend_from_slice(&[a, ab, ca, ab, b, bc, ca, bc, c, ab, bc, ca]);
        }
        t = nt;
    }
    (v, t)
}

struct Out {
    mesh: Mesh,
    cols: Vec<[u8; 4]>,
}

impl Out {
    fn cylinder(&mut self, a: [f32; 3], b: [f32; 3], r: f32, ca: [u8; 4], cb: [u8; 4], sides: u32) {
        let axis = sub(b, a);
        let d = unit(axis);
        let u = unit(cross(
            d,
            if d[0].abs() < 0.9 {
                [1.0, 0.0, 0.0]
            } else {
                [0.0, 1.0, 0.0]
            },
        ));
        let w = cross(d, u);
        let mid = add(a, scale(axis, 0.5));
        let base = self.mesh.positions.len() as u32;
        // Three rings (start, middle twice for the colour change, end).
        for (p, c) in [(a, ca), (mid, ca), (mid, cb), (b, cb)] {
            for k in 0..sides {
                let th = std::f32::consts::TAU * k as f32 / sides as f32;
                let n = add(scale(u, th.cos()), scale(w, th.sin()));
                self.mesh.positions.push(add(p, scale(n, r)));
                self.mesh.normals.push(n);
                self.cols.push(c);
            }
        }
        for ring in [0, 2] {
            let r0 = base + ring * sides;
            let r1 = r0 + sides;
            for k in 0..sides {
                let k1 = (k + 1) % sides;
                self.mesh.indices.extend_from_slice(&[
                    r0 + k,
                    r0 + k1,
                    r1 + k,
                    r0 + k1,
                    r1 + k1,
                    r1 + k,
                ]);
            }
        }
    }

    fn ball(&mut self, s: &(Vec<[f32; 3]>, Vec<u32>), c: [f32; 3], r: f32, col: [u8; 4]) {
        let base = self.mesh.positions.len() as u32;
        for &n in &s.0 {
            self.mesh.positions.push(add(c, scale(n, r)));
            self.mesh.normals.push(n);
            self.cols.push(col);
        }
        self.mesh.indices.extend(s.1.iter().map(|&i| base + i));
    }
}

/// The scene as triangles: what [`render`] draws.
pub fn scene(m: &Molecule, style: Style, colouring: Colouring) -> Mesh {
    tessellate(&scene::build(m, style, colouring), m.info.id.clone())
}

/// A scene's balls and sticks as low-polygon spheres and cylinders, with its cartoon.
pub fn tessellate(s: &Scene, title: Option<String>) -> Mesh {
    let mut out = Out {
        mesh: Mesh {
            up: Up::Z,
            ..Mesh::default()
        },
        cols: Vec::new(),
    };
    if let Some(c) = &s.cartoon {
        out.cols = c.colors.clone().unwrap_or_default();
        out.mesh.positions = c.positions.clone();
        out.mesh.normals = c.normals.clone();
        out.mesh.indices = c.indices.clone();
    }
    // Polygon budget: finer spheres for small molecules.
    let sph = sphere(match s.balls.len() {
        0..=2_000 => 2,
        2_001..=50_000 => 1,
        _ => 0,
    });
    for b in &s.balls {
        out.ball(&sph, b.centre, b.radius, b.colour);
    }
    let sides = if s.sticks.len() > 50_000 { 4 } else { 8 };
    for t in &s.sticks {
        out.cylinder(t.a, t.b, t.radius, t.colours[0], t.colours[1], sides);
    }
    let mut mesh = out.mesh;
    let mut bb = Bounds::EMPTY;
    for &p in &mesh.positions {
        bb.add(p);
    }
    mesh.bounds = bb;
    mesh.colors = Some(out.cols);
    mesh.parts = vec![Part {
        first: 0,
        count: (mesh.indices.len() / 3) as u32,
        material: None,
    }];
    mesh.title = title;
    mesh
}

/// Renders the molecule in its default style and colours, straight sRGB RGBA8.
pub fn render(m: &Molecule, width: u32, height: u32) -> Vec<u8> {
    let style = m.default_style();
    let mesh = scene(m, style, scene::default_colouring(m, style));
    specular_mesh::preview::render(&mesh, &[], width, height)
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spheres_are_closed_and_unit() {
        for level in 0..3 {
            let (v, t) = sphere(level);
            assert!(
                v.iter()
                    .all(|p| ((p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt() - 1.0).abs() < 1e-5)
            );
            // Euler: V − E + F = 2 with E = 3F/2.
            let f = t.len() / 3;
            assert_eq!(v.len() + f - 3 * f / 2, 2);
        }
    }
}
