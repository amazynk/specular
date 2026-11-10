//! The cartoon: a ribbon swept along a Catmull–Rom spline through the Cα atoms, its width
//! direction taken from the carbonyls (C→O, flipped to stay consistent). Helices are flat
//! ribbons, strands wider flat ribbons ending in an arrow, coil a round tube; nucleic acids
//! are a tube through their phosphates (or C4′). The cross-section is a 12-point
//! superellipse, so one ring layout serves every profile.

use specular_mesh::{Bounds, Mesh, Part, Up};

use crate::{Molecule, ResKind, Ss};

/// Cross-section points.
const RING: usize = 12;

/// A run of consecutive polymer residues of one kind, by residue index.
pub fn segments(m: &Molecule) -> Vec<Vec<usize>> {
    let mut out: Vec<Vec<usize>> = Vec::new();
    for c in &m.chains {
        let mut cur: Vec<usize> = Vec::new();
        let mut last: Option<(ResKind, [f32; 3])> = None;
        for ri in c.first as usize..c.end as usize {
            let r = &m.residues[ri];
            let (Some(t), true) = (r.trace, matches!(r.kind, ResKind::Amino | ResKind::Nucleic))
            else {
                continue;
            };
            let p = m.atoms[t as usize].pos;
            let joined = last.is_some_and(|(k, q)| {
                let limit = if k == ResKind::Amino { 4.2f32 } else { 8.0 };
                k == r.kind && d2(p, q) <= limit * limit
            });
            if !joined && !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            cur.push(ri);
            last = Some((r.kind, p));
        }
        if !cur.is_empty() {
            out.push(cur);
        }
    }
    out.retain(|s| s.len() >= 2);
    out
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn scale(a: [f32; 3], s: f32) -> [f32; 3] {
    [a[0] * s, a[1] * s, a[2] * s]
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn d2(a: [f32; 3], b: [f32; 3]) -> f32 {
    let d = sub(a, b);
    dot(d, d)
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn norm(a: [f32; 3]) -> Option<[f32; 3]> {
    let l = dot(a, a).sqrt();
    (l > 1e-6 && l.is_finite()).then(|| scale(a, 1.0 / l))
}

/// Any unit vector perpendicular to `t`.
fn perpendicular(t: [f32; 3]) -> [f32; 3] {
    let a = if t[0].abs() < 0.9 {
        [1.0, 0.0, 0.0]
    } else {
        [0.0, 1.0, 0.0]
    };
    norm(cross(t, a)).unwrap_or([0.0, 0.0, 1.0])
}

/// A cross-section: half-width along the width direction, half-thickness along the other,
/// and the superellipse exponent (2: ellipse; 4: rounded box).
#[derive(Clone, Copy, PartialEq)]
struct Profile {
    hw: f32,
    ht: f32,
    n: f32,
}

const COIL: Profile = Profile {
    hw: 0.3,
    ht: 0.3,
    n: 2.0,
};
const HELIX: Profile = Profile {
    hw: 1.2,
    ht: 0.25,
    n: 4.0,
};
const STRAND: Profile = Profile {
    hw: 0.8,
    ht: 0.22,
    n: 4.0,
};
const ARROW: f32 = 1.35;
const NUCLEIC: Profile = Profile {
    hw: 0.45,
    ht: 0.45,
    n: 2.0,
};

/// Builds the cartoon of every polymer segment, coloured per residue (`colours` is indexed
/// by residue). `None` when there is no polymer to draw.
pub fn build(m: &Molecule, colours: &[[u8; 4]]) -> Option<Mesh> {
    let segs = segments(m);
    if segs.is_empty() {
        return None;
    }
    let total: usize = segs.iter().map(Vec::len).sum();
    // Samples per residue step: even, so the half-residue profile changes land on a sample.
    let per = if total > 20_000 { 2 } else { 6 };
    let mut mesh = Mesh {
        up: Up::Z,
        ..Mesh::default()
    };
    let est = total * per * RING + 1024;
    mesh.positions.reserve(est);
    mesh.normals.reserve(est);
    let mut cols: Vec<[u8; 4]> = Vec::with_capacity(est);
    for seg in &segs {
        sweep(m, seg, per, colours, &mut mesh, &mut cols);
    }
    if mesh.indices.is_empty() {
        return None;
    }
    let mut b = Bounds::EMPTY;
    for &p in &mesh.positions {
        b.add(p);
    }
    mesh.bounds = b;
    mesh.colors = Some(cols);
    mesh.parts = vec![Part {
        first: 0,
        count: (mesh.indices.len() / 3) as u32,
        material: None,
    }];
    mesh.title = m.info.id.clone();
    Some(mesh)
}

fn sweep(
    m: &Molecule,
    seg: &[usize],
    per: usize,
    colours: &[[u8; 4]],
    mesh: &mut Mesh,
    cols: &mut Vec<[u8; 4]>,
) {
    let n = seg.len();
    let res = |k: usize| &m.residues[seg[k]];
    let p: Vec<[f32; 3]> = seg
        .iter()
        .map(|&r| m.atoms[m.residues[r].trace.unwrap_or(0) as usize].pos)
        .collect();
    let nucleic = res(0).kind == ResKind::Nucleic;
    // Width directions from the carbonyls, made consistent.
    let mut side: Vec<Option<[f32; 3]>> = seg
        .iter()
        .map(|&r| {
            if nucleic {
                return None;
            }
            let rr = &m.residues[r];
            let (mut c, mut o) = (None, None);
            for a in &m.atoms[rr.atoms()] {
                match a.name.as_str() {
                    "C" => c = Some(a.pos),
                    "O" => o = Some(a.pos),
                    _ => {}
                }
            }
            norm(sub(o?, c?))
        })
        .collect();
    let mut prev: Option<[f32; 3]> = None;
    for s in side.iter_mut().flatten() {
        if let Some(q) = prev
            && dot(*s, q) < 0.0
        {
            *s = scale(*s, -1.0);
        }
        prev = Some(*s);
    }
    // Residue profile kinds.
    let ss: Vec<Ss> = (0..n)
        .map(|k| if nucleic { Ss::Coil } else { res(k).ss })
        .collect();
    let base = |k: usize| -> Profile {
        if nucleic {
            return NUCLEIC;
        }
        match ss[k] {
            Ss::Helix => HELIX,
            Ss::Strand => STRAND,
            Ss::Coil => COIL,
        }
    };
    let last_strand = |k: usize| ss[k] == Ss::Strand && (k + 1 == n || ss[k + 1] != Ss::Strand);
    // The profile at continuous residue coordinate u (owner residue r = round(u)).
    let profile = |u: f32, r: usize| -> Profile {
        if last_strand(r) {
            // Arrow: full width at r − ½, narrowing to the coil at r + ½.
            let t = (u - (r as f32 - 0.5)).clamp(0.0, 1.0);
            return Profile {
                hw: ARROW + (COIL.hw - ARROW) * t,
                ht: STRAND.ht + (COIL.ht - STRAND.ht) * t,
                n: 4.0,
            };
        }
        base(r)
    };
    let point = |u: f32| -> ([f32; 3], [f32; 3]) {
        // Catmull–Rom through p with clamped ends; value and derivative.
        let i = (u.floor() as usize).min(n - 2);
        let t = u - i as f32;
        let p0 = if i == 0 {
            sub(scale(p[0], 2.0), p[1])
        } else {
            p[i - 1]
        };
        let (p1, p2) = (p[i], p[i + 1]);
        let p3 = if i + 2 < n {
            p[i + 2]
        } else {
            sub(scale(p2, 2.0), p1)
        };
        let (t2, t3) = (t * t, t * t * t);
        let pos = [0, 1, 2].map(|k| {
            0.5 * (2.0 * p1[k]
                + (-p0[k] + p2[k]) * t
                + (2.0 * p0[k] - 5.0 * p1[k] + 4.0 * p2[k] - p3[k]) * t2
                + (-p0[k] + 3.0 * p1[k] - 3.0 * p2[k] + p3[k]) * t3)
        });
        let der = [0, 1, 2].map(|k| {
            0.5 * ((-p0[k] + p2[k])
                + 2.0 * (2.0 * p0[k] - 5.0 * p1[k] + 4.0 * p2[k] - p3[k]) * t
                + 3.0 * (-p0[k] + 3.0 * p1[k] - 3.0 * p2[k] + p3[k]) * t2)
        });
        (pos, der)
    };
    let side_at = |u: f32| -> Option<[f32; 3]> {
        let i = (u.floor() as usize).min(n - 2);
        let t = u - i as f32;
        match (side[i], side[i + 1]) {
            (Some(a), Some(b)) => norm(add(scale(a, 1.0 - t), scale(b, t))),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        }
    };
    // Sample coordinates, with a duplicate at each profile step (arrow bases, ends of
    // helices and strands) so the change is sharp.
    let mut us: Vec<(f32, usize)> = Vec::with_capacity(n * per + 8);
    for i in 0..n - 1 {
        for s in 0..per {
            let u = i as f32 + s as f32 / per as f32;
            let r = if s * 2 < per { i } else { i + 1 };
            if s * 2 == per {
                // u = i + ½: the boundary between residues i and i + 1.
                us.push((u, i));
            }
            us.push((u, r));
        }
    }
    us.push(((n - 1) as f32, n - 1));
    let mut frame_prev: Option<[f32; 3]> = None;
    let mut ring_prev: Option<(u32, Profile)> = None;
    let first_ring = mesh.positions.len() as u32;
    for &(u, r) in &us {
        let (c, d) = point(u);
        let t = norm(d).unwrap_or_else(|| frame_prev.map_or([1.0, 0.0, 0.0], perpendicular));
        // Width direction: the carbonyl side, else the previous frame carried along.
        let w0 = side_at(u)
            .or(frame_prev)
            .unwrap_or_else(|| perpendicular(t));
        let w = norm(sub(w0, scale(t, dot(w0, t))))
            .or_else(|| frame_prev.and_then(|q| norm(sub(q, scale(t, dot(q, t))))))
            .unwrap_or_else(|| perpendicular(t));
        let w = match frame_prev {
            Some(q) if dot(w, q) < 0.0 => scale(w, -1.0),
            _ => w,
        };
        frame_prev = Some(w);
        let bn = cross(t, w);
        let pr = profile(u, r);
        let start = mesh.positions.len() as u32;
        let colour = colours.get(seg[r]).copied().unwrap_or([200, 200, 200, 255]);
        for k in 0..RING {
            let th = std::f32::consts::TAU * k as f32 / RING as f32;
            let (s, co) = th.sin_cos();
            let e = 2.0 / pr.n;
            let x = pr.hw * co.signum() * co.abs().powf(e);
            let y = pr.ht * s.signum() * s.abs().powf(e);
            let ne = 2.0 - e;
            let nx = co.signum() * co.abs().powf(ne) / pr.hw;
            let ny = s.signum() * s.abs().powf(ne) / pr.ht;
            let nn = norm(add(scale(w, nx), scale(bn, ny))).unwrap_or(w);
            mesh.positions.push(add(c, add(scale(w, x), scale(bn, y))));
            mesh.normals.push(nn);
            cols.push(colour);
        }
        if let Some((a, _)) = ring_prev {
            for k in 0..RING as u32 {
                let k1 = (k + 1) % RING as u32;
                let (a0, a1, b0, b1) = (a + k, a + k1, start + k, start + k1);
                mesh.indices.extend_from_slice(&[a0, a1, b0, a1, b1, b0]);
            }
        }
        ring_prev = Some((start, pr));
    }
    // End caps: a fan facing out of each end.
    let last_ring = ring_prev.map_or(first_ring, |r| r.0);
    for (ring, outward) in [(first_ring, -1.0f32), (last_ring, 1.0)] {
        let (u, r) = if outward < 0.0 {
            (0.0, 0)
        } else {
            ((n - 1) as f32, n - 1)
        };
        let (c, d) = point(u);
        let t = scale(norm(d).unwrap_or([1.0, 0.0, 0.0]), outward);
        let centre = mesh.positions.len() as u32;
        let colour = colours.get(seg[r]).copied().unwrap_or([200, 200, 200, 255]);
        mesh.positions.push(c);
        mesh.normals.push(t);
        cols.push(colour);
        // Rim copies with the cap's flat normal.
        let rim = mesh.positions.len() as u32;
        for k in 0..RING as u32 {
            let q = mesh.positions[(ring + k) as usize];
            mesh.positions.push(q);
            mesh.normals.push(t);
            cols.push(colour);
        }
        for k in 0..RING as u32 {
            let k1 = (k + 1) % RING as u32;
            if outward > 0.0 {
                mesh.indices.extend_from_slice(&[centre, rim + k, rim + k1]);
            } else {
                mesh.indices.extend_from_slice(&[centre, rim + k1, rim + k]);
            }
        }
    }
}
