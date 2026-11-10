//! Geometry nodes into triangles: the face and triangle sets with their colours, normals and
//! texture coordinates (per vertex, per corner through an index, or per face), elevation
//! grids, extrusions, and the primitives tessellated.

use std::f64::consts::PI;

use super::scene::Scene;
use crate::Result;
use crate::builder::{Affine, Builder};
use crate::poly::{Attr, Poly, emit};

/// A TextureTransform: `Tc' = −C × S × R × C × T × Tc`.
#[derive(Clone, Copy, Debug)]
pub(super) struct TexXf {
    pub center: [f64; 2],
    pub scale: [f64; 2],
    pub rotation: f64,
    pub translation: [f64; 2],
}

impl TexXf {
    fn apply(&self, p: [f64; 2]) -> [f64; 2] {
        let q = [
            p[0] + self.translation[0] + self.center[0],
            p[1] + self.translation[1] + self.center[1],
        ];
        let (s, c) = self.rotation.sin_cos();
        let r = [c * q[0] - s * q[1], s * q[0] + c * q[1]];
        [
            r[0] * self.scale[0] - self.center[0],
            r[1] * self.scale[1] - self.center[1],
        ]
    }
}

/// How a shape looks.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Look {
    pub material: Option<u32>,
    pub tex: Option<TexXf>,
}

/// An attribute with its values owned.
enum Own<T> {
    None,
    Vertex(Vec<T>),
    Corner(Vec<T>),
    Face(Vec<T>),
}

impl<T: Copy> Own<T> {
    fn attr(&self) -> Attr<'_, T> {
        match self {
            Own::None => Attr::None,
            Own::Vertex(v) => Attr::Vertex(v),
            Own::Corner(v) => Attr::Corner(v),
            Own::Face(v) => Attr::Face(v),
        }
    }

    fn is_none(&self) -> bool {
        matches!(self, Own::None)
    }
}

/// Polygons over positions.
#[derive(Default)]
struct Faces {
    counts: Vec<u32>,
    idx: Vec<u32>,
}

impl Faces {
    fn push(&mut self, corners: &[u32]) {
        self.counts.push(corners.len() as u32);
        self.idx.extend_from_slice(corners);
    }
}

fn to_index(v: i64) -> u32 {
    u32::try_from(v).unwrap_or(u32::MAX)
}

fn points3(sc: &Scene, n: u32, field: &str) -> Vec<[f32; 3]> {
    sc.nums(n, field)
        .chunks_exact(3)
        .map(|c| [c[0] as f32, c[1] as f32, c[2] as f32])
        .collect()
}

fn coords(sc: &Scene, g: u32) -> Vec<[f32; 3]> {
    match sc.node(g, "coord") {
        Some(c) if matches!(sc.kind(c), "Coordinate" | "CoordinateDouble") => {
            points3(sc, c, "point")
        }
        _ => Vec::new(),
    }
}

fn unit8(v: f64) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

fn colors(sc: &Scene, g: u32) -> Option<Vec<[u8; 4]>> {
    let c = sc.node(g, "color")?;
    let v = sc.nums(c, "color");
    match sc.kind(c) {
        "Color" => Some(
            v.chunks_exact(3)
                .map(|c| [unit8(c[0]), unit8(c[1]), unit8(c[2]), 255])
                .collect(),
        ),
        "ColorRGBA" => Some(
            v.chunks_exact(4)
                .map(|c| [unit8(c[0]), unit8(c[1]), unit8(c[2]), unit8(c[3])])
                .collect(),
        ),
        _ => None,
    }
}

fn normals(sc: &Scene, g: u32) -> Option<Vec<[f32; 3]>> {
    let n = sc.node(g, "normal")?;
    (sc.kind(n) == "Normal").then(|| points3(sc, n, "vector"))
}

fn uv_of(p: [f64; 2], tex: Option<TexXf>) -> [f32; 2] {
    let p = tex.map_or(p, |t| t.apply(p));
    // glTF orientation: v down.
    [p[0] as f32, (1.0 - p[1]) as f32]
}

fn uvs(sc: &Scene, g: u32, tex: Option<TexXf>) -> Option<Vec<[f32; 2]>> {
    let mut t = sc.node(g, "texCoord")?;
    if sc.kind(t) == "MultiTextureCoordinate" {
        t = *sc.nodes(t, "texCoord").first()?;
    }
    (sc.kind(t) == "TextureCoordinate").then(|| {
        sc.nums(t, "point")
            .chunks_exact(2)
            .map(|c| uv_of([c[0], c[1]], tex))
            .collect()
    })
}

fn pick<T: Copy>(v: &[T], i: Option<i64>, default: T) -> T {
    i.and_then(|i| usize::try_from(i).ok())
        .and_then(|i| v.get(i))
        .copied()
        .unwrap_or(default)
}

/// Newell normal of a polygon (counter-clockwise faces point at the viewer).
fn newell(pos: &[[f32; 3]], ix: &[u32]) -> [f32; 3] {
    let mut n = [0f64; 3];
    for k in 0..ix.len() {
        let (Some(a), Some(b)) = (
            pos.get(ix[k] as usize),
            pos.get(ix[(k + 1) % ix.len()] as usize),
        ) else {
            return [0.0; 3];
        };
        let (a, b) = (a.map(f64::from), b.map(f64::from));
        n[0] += (a[1] - b[1]) * (a[2] + b[2]);
        n[1] += (a[2] - b[2]) * (a[0] + b[0]);
        n[2] += (a[0] - b[0]) * (a[1] + b[1]);
    }
    let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if l > 0.0 && l.is_finite() {
        n.map(|v| (v / l) as f32)
    } else {
        [0.0; 3]
    }
}

struct Emit<'a> {
    pos: &'a [[f32; 3]],
    faces: &'a Faces,
    normals: Own<[f32; 3]>,
    uvs: Own<[f32; 2]>,
    colors: Own<[u8; 4]>,
    ccw: bool,
    /// Without normals: shade each face flat.
    faceted: bool,
}

fn finish(b: &mut Builder, e: Emit, look: &Look, xf: &Affine) -> Result<()> {
    let mut normals = e.normals;
    if normals.is_none() && e.faceted {
        let mut at = 0usize;
        let mut v = Vec::with_capacity(e.faces.counts.len());
        for &c in &e.faces.counts {
            let ix = e.faces.idx.get(at..at + c as usize).unwrap_or(&[]);
            at += c as usize;
            let n = newell(e.pos, ix);
            v.push(if e.ccw { n } else { n.map(|x| -x) });
        }
        normals = Own::Face(v);
    }
    let mut p = Poly::new(e.pos, &e.faces.counts, &e.faces.idx);
    p.normals = normals.attr();
    p.uvs = e.uvs.attr();
    p.colors = e.colors.attr();
    p.material = look.material;
    p.clockwise = !e.ccw;
    emit(b, &p, xf)
}

/// Adds geometry node `g`; `false` when its type is not drawn.
pub(super) fn geometry(
    sc: &Scene,
    b: &mut Builder,
    g: u32,
    xf: &Affine,
    look: &Look,
) -> Result<bool> {
    match sc.kind(g) {
        "IndexedFaceSet" => face_set(sc, b, g, xf, look)?,
        "IndexedTriangleSet"
        | "TriangleSet"
        | "IndexedQuadSet"
        | "QuadSet"
        | "IndexedTriangleFanSet"
        | "TriangleFanSet"
        | "IndexedTriangleStripSet"
        | "TriangleStripSet" => triangle_set(sc, b, g, xf, look)?,
        "ElevationGrid" => elevation_grid(sc, b, g, xf, look)?,
        "Extrusion" => extrusion(sc, b, g, xf, look)?,
        "Box" | "Sphere" | "Cylinder" | "Cone" => primitive(sc, b, g, xf, look)?,
        _ => return Ok(false),
    }
    Ok(true)
}

fn face_set(sc: &Scene, b: &mut Builder, g: u32, xf: &Affine, look: &Look) -> Result<()> {
    let pos = coords(sc, g);
    if pos.is_empty() {
        return Ok(());
    }
    let ci = sc.ints(g, "coordIndex");
    let mut faces = Faces::default();
    // Index into coordIndex of each corner (for the other index lists, which run in step).
    let mut raw = Vec::with_capacity(ci.len());
    let mut cur = 0u32;
    for (k, &v) in ci.iter().enumerate() {
        if v < 0 {
            if cur > 0 {
                faces.counts.push(cur);
            }
            cur = 0;
        } else {
            faces.idx.push(to_index(v));
            raw.push(k);
            cur += 1;
        }
    }
    if cur > 0 {
        faces.counts.push(cur);
    }
    let nf = faces.counts.len();
    // Per vertex (through an index in step with coordIndex, or coordIndex itself), or per
    // face (through an index of one per face, or in order).
    fn bind<T: Copy>(
        vals: Vec<T>,
        per_vertex: bool,
        idx: &[i64],
        raw: &[usize],
        nf: usize,
        default: T,
    ) -> Own<T> {
        if per_vertex {
            if idx.is_empty() {
                Own::Vertex(vals)
            } else {
                Own::Corner(
                    raw.iter()
                        .map(|&k| pick(&vals, idx.get(k).copied(), default))
                        .collect(),
                )
            }
        } else {
            Own::Face(
                (0..nf)
                    .map(|f| {
                        let i = if idx.is_empty() {
                            Some(f as i64)
                        } else {
                            idx.get(f).copied()
                        };
                        pick(&vals, i, default)
                    })
                    .collect(),
            )
        }
    }
    let colors = colors(sc, g).map_or(Own::None, |c| {
        bind(
            c,
            sc.boolean(g, "colorPerVertex", true),
            &sc.ints(g, "colorIndex"),
            &raw,
            nf,
            [255; 4],
        )
    });
    let normals = normals(sc, g).map_or(Own::None, |n| {
        bind(
            n,
            sc.boolean(g, "normalPerVertex", true),
            &sc.ints(g, "normalIndex"),
            &raw,
            nf,
            [0.0; 3],
        )
    });
    let uvs = uvs(sc, g, look.tex).map_or(Own::None, |t| {
        bind(t, true, &sc.ints(g, "texCoordIndex"), &raw, nf, [0.0; 2])
    });
    let faceted = sc.num(g, "creaseAngle", 0.0) < 0.5;
    finish(
        b,
        Emit {
            pos: &pos,
            faces: &faces,
            normals,
            uvs,
            colors,
            ccw: sc.boolean(g, "ccw", true),
            faceted,
        },
        look,
        xf,
    )
}

fn triangle_set(sc: &Scene, b: &mut Builder, g: u32, xf: &Affine, look: &Look) -> Result<()> {
    let pos = coords(sc, g);
    if pos.is_empty() {
        return Ok(());
    }
    let kind = sc.kind(g);
    let indexed = kind.starts_with("Indexed");
    let all: Vec<u32>;
    let index: Vec<i64>;
    let seq: &[u32] = if indexed {
        index = sc.ints(g, "index");
        all = index
            .iter()
            .filter(|&&v| v >= 0)
            .map(|&v| to_index(v))
            .collect();
        &all
    } else {
        index = Vec::new();
        all = (0..pos.len() as u32).collect();
        &all
    };
    let mut faces = Faces::default();
    // Runs of a strip or fan set: split at −1 (indexed) or by their counts.
    let runs = |counts_field: &str| -> Vec<Vec<u32>> {
        if indexed {
            index
                .split(|&v| v < 0)
                .filter(|r| !r.is_empty())
                .map(|r| r.iter().map(|&v| to_index(v)).collect())
                .collect()
        } else {
            let mut out = Vec::new();
            let mut at = 0usize;
            for c in sc.ints(g, counts_field) {
                let Ok(c) = usize::try_from(c) else { break };
                let Some(r) = seq.get(at..at.saturating_add(c)) else {
                    break;
                };
                out.push(r.to_vec());
                at += c;
            }
            out
        }
    };
    match kind {
        "IndexedTriangleSet" | "TriangleSet" => {
            for t in seq.chunks_exact(3) {
                faces.push(t);
            }
        }
        "IndexedQuadSet" | "QuadSet" => {
            for q in seq.chunks_exact(4) {
                faces.push(q);
            }
        }
        "IndexedTriangleFanSet" | "TriangleFanSet" => {
            for r in runs("fanCount") {
                for k in 1..r.len().saturating_sub(1) {
                    faces.push(&[r[0], r[k], r[k + 1]]);
                }
            }
        }
        _ => {
            for r in runs("stripCount") {
                for k in 0..r.len().saturating_sub(2) {
                    if k % 2 == 0 {
                        faces.push(&[r[k], r[k + 1], r[k + 2]]);
                    } else {
                        faces.push(&[r[k + 1], r[k], r[k + 2]]);
                    }
                }
            }
        }
    }
    let nf = faces.counts.len();
    let npv = sc.boolean(g, "normalPerVertex", true);
    let normals = normals(sc, g).map_or(Own::None, |n| {
        if npv {
            Own::Vertex(n)
        } else {
            Own::Face(
                (0..nf)
                    .map(|f| n.get(f).copied().unwrap_or([0.0; 3]))
                    .collect(),
            )
        }
    });
    let colors = colors(sc, g).map_or(Own::None, |c| {
        if sc.boolean(g, "colorPerVertex", true) {
            Own::Vertex(c)
        } else {
            Own::Face(
                (0..nf)
                    .map(|f| c.get(f).copied().unwrap_or([255; 4]))
                    .collect(),
            )
        }
    });
    let uvs = uvs(sc, g, look.tex).map_or(Own::None, Own::Vertex);
    finish(
        b,
        Emit {
            pos: &pos,
            faces: &faces,
            normals,
            uvs,
            colors,
            ccw: sc.boolean(g, "ccw", true),
            faceted: !npv,
        },
        look,
        xf,
    )
}

/// Most grid points or extrusion rings × cross-section points.
const MAX_GENERATED: usize = 16_000_000;

fn elevation_grid(sc: &Scene, b: &mut Builder, g: u32, xf: &Affine, look: &Look) -> Result<()> {
    let xd = sc.num(g, "xDimension", 0.0).max(0.0) as usize;
    let zd = sc.num(g, "zDimension", 0.0).max(0.0) as usize;
    let (xs, zs) = (sc.num(g, "xSpacing", 1.0), sc.num(g, "zSpacing", 1.0));
    let h = sc.nums(g, "height");
    if xd < 2 || zd < 2 || xd.saturating_mul(zd) > MAX_GENERATED || h.len() < xd * zd {
        return Ok(());
    }
    let mut pos = Vec::with_capacity(xd * zd);
    let mut tex = Vec::with_capacity(xd * zd);
    for j in 0..zd {
        for i in 0..xd {
            pos.push([
                (i as f64 * xs) as f32,
                h[i + j * xd] as f32,
                (j as f64 * zs) as f32,
            ]);
            tex.push(uv_of(
                [i as f64 / (xd - 1) as f64, j as f64 / (zd - 1) as f64],
                look.tex,
            ));
        }
    }
    let mut faces = Faces::default();
    for j in 0..zd - 1 {
        for i in 0..xd - 1 {
            let at = |i: usize, j: usize| (i + j * xd) as u32;
            faces.push(&[at(i, j), at(i, j + 1), at(i + 1, j + 1), at(i + 1, j)]);
        }
    }
    let nf = faces.counts.len();
    let colors = colors(sc, g).map_or(Own::None, |c| {
        if sc.boolean(g, "colorPerVertex", true) {
            Own::Vertex(c)
        } else {
            Own::Face(
                (0..nf)
                    .map(|f| c.get(f).copied().unwrap_or([255; 4]))
                    .collect(),
            )
        }
    });
    let normals = normals(sc, g).map_or(Own::None, |n| {
        if sc.boolean(g, "normalPerVertex", true) {
            Own::Vertex(n)
        } else {
            Own::Face(
                (0..nf)
                    .map(|f| n.get(f).copied().unwrap_or([0.0; 3]))
                    .collect(),
            )
        }
    });
    let uvs = Own::Vertex(uvs(sc, g, look.tex).unwrap_or(tex));
    finish(
        b,
        Emit {
            pos: &pos,
            faces: &faces,
            normals,
            uvs,
            colors,
            ccw: sc.boolean(g, "ccw", true),
            faceted: sc.num(g, "creaseAngle", 0.0) < 0.5,
        },
        look,
        xf,
    )
}

type V3 = [f64; 3];

fn vsub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn vcross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn vdot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn vnorm(a: V3) -> Option<V3> {
    let l = vdot(a, a).sqrt();
    (l > 1e-12 && l.is_finite()).then(|| a.map(|v| v / l))
}

/// Rotates `v` about the unit `axis` by `angle` (Rodrigues).
pub(super) fn rotate(axis: V3, angle: f64, v: V3) -> V3 {
    let Some(k) = vnorm(axis) else { return v };
    let (s, c) = angle.sin_cos();
    let kv = vcross(k, v);
    let kd = vdot(k, v) * (1.0 - c);
    [0, 1, 2].map(|i| v[i] * c + kv[i] * s + k[i] * kd)
}

fn extrusion(sc: &Scene, b: &mut Builder, g: u32, xf: &Affine, look: &Look) -> Result<()> {
    let chunk3 =
        |v: Vec<f64>| -> Vec<V3> { v.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect() };
    let spine = match sc.field(g, "spine") {
        Some(_) => chunk3(sc.nums(g, "spine")),
        None => vec![[0.0; 3], [0.0, 1.0, 0.0]],
    };
    let cross: Vec<[f64; 2]> = match sc.field(g, "crossSection") {
        Some(_) => sc
            .nums(g, "crossSection")
            .chunks_exact(2)
            .map(|c| [c[0], c[1]])
            .collect(),
        None => vec![
            [1.0, 1.0],
            [1.0, -1.0],
            [-1.0, -1.0],
            [-1.0, 1.0],
            [1.0, 1.0],
        ],
    };
    let scale: Vec<[f64; 2]> = {
        let s: Vec<[f64; 2]> = sc
            .nums(g, "scale")
            .chunks_exact(2)
            .map(|c| [c[0], c[1]])
            .collect();
        if s.is_empty() { vec![[1.0, 1.0]] } else { s }
    };
    let orient: Vec<[f64; 4]> = {
        let o: Vec<[f64; 4]> = sc
            .nums(g, "orientation")
            .chunks_exact(4)
            .map(|c| [c[0], c[1], c[2], c[3]])
            .collect();
        if o.is_empty() {
            vec![[0.0, 0.0, 1.0, 0.0]]
        } else {
            o
        }
    };
    let (n, m) = (spine.len(), cross.len());
    if n < 2 || m < 2 || n.saturating_mul(m) > MAX_GENERATED {
        return Ok(());
    }
    let closed = spine[0] == spine[n - 1];
    // The spine-aligned cross-section planes.
    let y_at = |i: usize| -> V3 {
        if i == 0 || i == n - 1 {
            if closed {
                vsub(spine[1], spine[n - 2])
            } else if i == 0 {
                vsub(spine[1], spine[0])
            } else {
                vsub(spine[n - 1], spine[n - 2])
            }
        } else {
            vsub(spine[i + 1], spine[i - 1])
        }
    };
    let z_at = |i: usize| -> V3 {
        let at = |i: usize, prev: usize, next: usize| {
            vcross(vsub(spine[next], spine[i]), vsub(spine[prev], spine[i]))
        };
        if i == 0 || i == n - 1 {
            if closed && n > 2 {
                at(0, n - 2, 1)
            } else if n > 2 {
                if i == 0 {
                    at(1, 0, 2)
                } else {
                    at(n - 2, n - 3, n - 1)
                }
            } else {
                [0.0; 3]
            }
        } else {
            at(i, i - 1, i + 1)
        }
    };
    let mut frames: Vec<[V3; 3]> = Vec::with_capacity(n);
    let mut prev_z: Option<V3> = None;
    let mut prev_y: V3 = [0.0, 1.0, 0.0];
    for i in 0..n {
        let y = vnorm(y_at(i)).unwrap_or(prev_y);
        let mut z = match (vnorm(z_at(i)), prev_z) {
            (Some(z), _) => z,
            (None, Some(z)) => z,
            (None, None) => {
                // Collinear: the rotation taking +Y to the spine's direction, applied to +Z.
                let axis = vcross([0.0, 1.0, 0.0], y);
                let angle = vdot([0.0, 1.0, 0.0], y).clamp(-1.0, 1.0).acos();
                if vnorm(axis).is_some() {
                    rotate(axis, angle, [0.0, 0.0, 1.0])
                } else if y[1] < 0.0 {
                    [0.0, 0.0, -1.0]
                } else {
                    [0.0, 0.0, 1.0]
                }
            }
        };
        if let Some(p) = prev_z
            && vdot(z, p) < 0.0
        {
            z = z.map(|v| -v);
        }
        let x = vcross(y, z);
        frames.push([x, y, z]);
        prev_z = Some(z);
        prev_y = y;
    }
    let mut pos = Vec::with_capacity(n * m);
    let mut tex = Vec::with_capacity(n * m);
    for (i, &[x, y, z]) in frames.iter().enumerate() {
        let s = scale[i.min(scale.len() - 1)];
        let o = orient[i.min(orient.len() - 1)];
        for (j, c) in cross.iter().enumerate() {
            let l = rotate([o[0], o[1], o[2]], o[3], [c[0] * s[0], 0.0, c[1] * s[1]]);
            let p = [0, 1, 2].map(|k| spine[i][k] + x[k] * l[0] + y[k] * l[1] + z[k] * l[2]);
            pos.push(p.map(|v| v as f32));
            tex.push(uv_of(
                [j as f64 / (m - 1) as f64, i as f64 / (n - 1) as f64],
                look.tex,
            ));
        }
    }
    let mut faces = Faces::default();
    let at = |i: usize, j: usize| (i * m + j) as u32;
    for i in 0..n - 1 {
        for j in 0..m - 1 {
            faces.push(&[at(i, j), at(i, j + 1), at(i + 1, j + 1), at(i + 1, j)]);
        }
    }
    // Caps: the cross-section (without its closing point) at each end.
    let ring = if cross[0] == cross[m - 1] { m - 1 } else { m };
    if ring >= 3 {
        if sc.boolean(g, "beginCap", true) {
            let c: Vec<u32> = (0..ring).rev().map(|j| at(0, j)).collect();
            faces.push(&c);
        }
        if sc.boolean(g, "endCap", true) {
            let c: Vec<u32> = (0..ring).map(|j| at(n - 1, j)).collect();
            faces.push(&c);
        }
    }
    finish(
        b,
        Emit {
            pos: &pos,
            faces: &faces,
            normals: Own::None,
            uvs: Own::Vertex(tex),
            colors: Own::None,
            ccw: sc.boolean(g, "ccw", true),
            faceted: sc.num(g, "creaseAngle", 0.0) < 0.5,
        },
        look,
        xf,
    )
}

/// Segments around a round primitive.
const SEGMENTS: usize = 48;
/// Rings from pole to pole of a sphere.
const RINGS: usize = 24;

#[derive(Default)]
struct Prim {
    pos: Vec<[f32; 3]>,
    nrm: Vec<[f32; 3]>,
    tex: Vec<[f32; 2]>,
    faces: Faces,
}

impl Prim {
    fn v(&mut self, p: V3, n: V3, t: [f64; 2], tx: Option<TexXf>) -> u32 {
        self.pos.push(p.map(|v| v as f32));
        self.nrm.push(n.map(|v| v as f32));
        self.tex.push(uv_of(t, tx));
        (self.pos.len() - 1) as u32
    }
}

fn primitive(sc: &Scene, b: &mut Builder, g: u32, xf: &Affine, look: &Look) -> Result<()> {
    let mut p = Prim::default();
    let tx = look.tex;
    let ring = |j: usize| {
        let phi = 2.0 * PI * j as f64 / SEGMENTS as f64;
        (phi.sin(), phi.cos())
    };
    match sc.kind(g) {
        "Box" => {
            let s = sc.vec(g, "size", [2.0; 3]).map(|v| v / 2.0);
            let sides: [(V3, V3, V3); 6] = [
                ([1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]),
                ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]),
                ([0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, -1.0]),
                ([0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
                ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
                ([0.0, 0.0, -1.0], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
            ];
            for (n, u, v) in sides {
                let mut c = [0u32; 4];
                for (k, (a, bb)) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
                    .into_iter()
                    .enumerate()
                {
                    let q = [0, 1, 2].map(|i| (n[i] + u[i] * a + v[i] * bb) * s[i]);
                    c[k] = p.v(q, n, [(a + 1.0) / 2.0, (bb + 1.0) / 2.0], tx);
                }
                p.faces.push(&c);
            }
        }
        "Sphere" => {
            let r = sc.num(g, "radius", 1.0);
            for i in 0..=RINGS {
                let th = PI * i as f64 / RINGS as f64;
                for j in 0..=SEGMENTS {
                    let (s, c) = ring(j);
                    let n = [th.sin() * s, th.cos(), th.sin() * c];
                    p.v(
                        n.map(|v| v * r),
                        n,
                        [j as f64 / SEGMENTS as f64, 1.0 - i as f64 / RINGS as f64],
                        tx,
                    );
                }
            }
            let w = (SEGMENTS + 1) as u32;
            for i in 0..RINGS as u32 {
                for j in 0..SEGMENTS as u32 {
                    let a = i * w + j;
                    if i == 0 {
                        p.faces.push(&[a, a + w, a + w + 1]);
                    } else if i == RINGS as u32 - 1 {
                        p.faces.push(&[a, a + w, a + 1]);
                    } else {
                        p.faces.push(&[a, a + w, a + w + 1, a + 1]);
                    }
                }
            }
        }
        "Cylinder" | "Cone" => {
            let cone = sc.kind(g) == "Cone";
            let h = sc.num(g, "height", 2.0) / 2.0;
            let (r, top_r) = if cone {
                (sc.num(g, "bottomRadius", 1.0), 0.0)
            } else {
                let r = sc.num(g, "radius", 1.0);
                (r, r)
            };
            if sc.boolean(g, "side", true) {
                // Side normals lean up on a cone.
                let slope = if cone { r / (2.0 * h).max(1e-12) } else { 0.0 };
                let w = (SEGMENTS + 1) as u32;
                let base = p.pos.len() as u32;
                for (y, rr, t) in [(h, top_r, 1.0), (-h, r, 0.0)] {
                    for j in 0..=SEGMENTS {
                        // The apex vertex of a cone sits between its two base corners.
                        let (s, c) = if cone && t == 1.0 {
                            let phi = 2.0 * PI * (j as f64 + 0.5) / SEGMENTS as f64;
                            (phi.sin(), phi.cos())
                        } else {
                            ring(j)
                        };
                        let n = vnorm([s, slope, c]).unwrap_or([s, 0.0, c]);
                        p.v([rr * s, y, rr * c], n, [j as f64 / SEGMENTS as f64, t], tx);
                    }
                }
                for j in 0..SEGMENTS as u32 {
                    let (t, bo) = (base + j, base + w + j);
                    if cone {
                        p.faces.push(&[t, bo, bo + 1]);
                    } else {
                        p.faces.push(&[t, bo, bo + 1, t + 1]);
                    }
                }
            }
            let cap = |p: &mut Prim, y: f64, up: bool| {
                let n = [0.0, if up { 1.0 } else { -1.0 }, 0.0];
                let mut c = Vec::with_capacity(SEGMENTS);
                for j in 0..SEGMENTS {
                    let (s, co) = ring(j);
                    c.push(p.v(
                        [r * s, y, r * co],
                        n,
                        [(s + 1.0) / 2.0, (co + 1.0) / 2.0],
                        tx,
                    ));
                }
                if !up {
                    c.reverse();
                }
                p.faces.push(&c);
            };
            if !cone && sc.boolean(g, "top", true) {
                cap(&mut p, h, true);
            }
            if sc.boolean(g, "bottom", true) {
                cap(&mut p, -h, false);
            }
        }
        _ => {}
    }
    if p.faces.counts.is_empty() {
        return Ok(());
    }
    finish(
        b,
        Emit {
            pos: &p.pos,
            faces: &p.faces,
            normals: Own::Vertex(p.nrm),
            uvs: Own::Vertex(p.tex),
            colors: Own::None,
            ccw: true,
            faceted: false,
        },
        look,
        xf,
    )
}
