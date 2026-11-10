//! What a style draws: atoms as balls, bonds and the unit cell as sticks (both drawn as
//! impostors on the GPU, or tessellated for previews), the cartoon as a mesh, and the
//! atoms a click can pick.

use specular_mesh::{Bounds, Mesh};

use crate::{BOND_RADIUS, Colouring, Molecule, Style, atom_radius, cartoon, colour};

/// A sphere.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ball {
    pub centre: [f32; 3],
    pub radius: f32,
    /// sRGB, straight alpha.
    pub colour: [u8; 4],
}

/// A cylinder from `a` to `b`, its first half one colour and its second the other.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stick {
    pub a: [f32; 3],
    pub b: [f32; 3],
    pub radius: f32,
    pub colours: [[u8; 4]; 2],
}

/// Radius a click picks a cartoon's trace atom (Cα, P) within, Å.
pub const TRACE_PICK_RADIUS: f32 = 1.2;

/// A style's scene.
#[derive(Clone, Debug, Default)]
pub struct Scene {
    pub balls: Vec<Ball>,
    pub sticks: Vec<Stick>,
    pub cartoon: Option<Mesh>,
    /// Atoms a click can pick, with their pick radius.
    pub targets: Vec<(u32, f32)>,
    /// Everything drawn (atoms with their radii, the cartoon, the cell).
    pub bounds: Bounds,
}

/// The colouring a style opens in: chains (several) or secondary structure (one) for the
/// cartoon, elements otherwise.
pub fn default_colouring(m: &Molecule, style: Style) -> Colouring {
    if style == Style::Cartoon {
        if m.chains.len() > 1 {
            Colouring::Chain
        } else {
            Colouring::Ss
        }
    } else {
        Colouring::Element
    }
}

/// Builds `style` in `colouring`.
pub fn build(m: &Molecule, style: Style, colouring: Colouring) -> Scene {
    let mut s = Scene::default();
    if style == Style::Cartoon {
        s.cartoon = cartoon::build(m, &colour::residue_colours(m, colouring));
        for seg in cartoon::segments(m) {
            for r in seg {
                if let Some(t) = m.residues[r].trace {
                    s.targets.push((t, TRACE_PICK_RADIUS));
                }
            }
        }
    }
    let show = m.visible_atoms(style);
    let colours = colour::atom_colours(m, colouring);
    s.balls.reserve(show.iter().filter(|&&v| v).count());
    for (i, a) in m.atoms.iter().enumerate() {
        if show[i] {
            let r = atom_radius(style, a.element);
            s.balls.push(Ball {
                centre: a.pos,
                radius: r,
                colour: colours[i],
            });
            s.targets.push((i as u32, r.max(0.3)));
        }
    }
    if style != Style::Spacefill {
        for b in &m.bonds {
            let (i, j) = (b.a as usize, b.b as usize);
            if show[i] && show[j] {
                s.sticks.push(Stick {
                    a: m.atoms[i].pos,
                    b: m.atoms[j].pos,
                    radius: BOND_RADIUS,
                    colours: [colours[i], colours[j]],
                });
            }
        }
    }
    if m.format == Some(crate::MolFormat::Cif)
        && let Some(edges) = m.cell_edges()
    {
        let r = m.cell_edge_radius();
        for (a, b, c) in edges {
            s.sticks.push(Stick {
                a,
                b,
                radius: r,
                colours: [c, c],
            });
        }
    }
    let mut bb = Bounds::EMPTY;
    for b in &s.balls {
        bb.add(b.centre.map(|v| v - b.radius));
        bb.add(b.centre.map(|v| v + b.radius));
    }
    for t in &s.sticks {
        for p in [t.a, t.b] {
            bb.add(p.map(|v| v - t.radius));
            bb.add(p.map(|v| v + t.radius));
        }
    }
    if let Some(c) = &s.cartoon
        && !c.bounds.is_empty()
    {
        bb.add(c.bounds.min);
        bb.add(c.bounds.max);
    }
    if bb.is_empty() {
        bb = m.bounds();
    }
    s.bounds = bb;
    s
}

impl Scene {
    /// The atom nearest along the ray from `origin` in direction `dir` (unit) among the
    /// pick targets, and how far along the ray it is.
    pub fn pick(&self, m: &Molecule, origin: [f64; 3], dir: [f64; 3]) -> Option<(usize, f64)> {
        let mut best: Option<(usize, f64)> = None;
        for &(i, r) in &self.targets {
            let Some(a) = m.atoms.get(i as usize) else {
                continue;
            };
            if let Some(t) = ray_sphere(origin, dir, a.pos.map(f64::from), f64::from(r))
                && best.is_none_or(|(_, bt)| t < bt)
            {
                best = Some((i as usize, t));
            }
        }
        best
    }
}

/// The nearest non-negative distance along a unit ray to a sphere.
pub fn ray_sphere(o: [f64; 3], d: [f64; 3], c: [f64; 3], r: f64) -> Option<f64> {
    let oc = [o[0] - c[0], o[1] - c[1], o[2] - c[2]];
    let b = oc[0] * d[0] + oc[1] * d[1] + oc[2] * d[2];
    let cc = oc[0] * oc[0] + oc[1] * oc[1] + oc[2] * oc[2] - r * r;
    let disc = b * b - cc;
    if disc < 0.0 {
        return None;
    }
    let s = disc.sqrt();
    let t0 = -b - s;
    let t1 = -b + s;
    if t0 >= 0.0 {
        Some(t0)
    } else if t1 >= 0.0 {
        Some(t1)
    } else {
        None
    }
}

impl Molecule {
    /// An atom as people name it: `LYS A42 NZ` (residue, chain and number with any
    /// insertion code, atom name), or the element and index for a small molecule.
    pub fn atom_label(&self, i: usize) -> String {
        let a = &self.atoms[i];
        let r = &self.residues[a.residue as usize];
        if r.name.as_str() == "UNL" && self.chain_of(r).id.is_empty() {
            return a.name.as_str().to_owned();
        }
        let icode = if r.icode == 0 {
            String::new()
        } else {
            char::from(r.icode).to_string()
        };
        format!(
            "{} {}{}{} {}",
            r.name,
            self.chain_of(r).id,
            r.seq,
            icode,
            a.name
        )
    }

    /// The distance between two atoms, Å.
    pub fn distance(&self, i: usize, j: usize) -> f64 {
        let (a, b) = (self.atoms[i].pos, self.atoms[j].pos);
        (0..3)
            .map(|k| (f64::from(a[k]) - f64::from(b[k])).powi(2))
            .sum::<f64>()
            .sqrt()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ray_hits_the_near_side() {
        let t = ray_sphere([0.0, 0.0, -10.0], [0.0, 0.0, 1.0], [0.0; 3], 1.0);
        assert_eq!(t, Some(9.0));
        assert_eq!(
            ray_sphere([0.0, 2.0, -10.0], [0.0, 0.0, 1.0], [0.0; 3], 1.0),
            None
        );
        // From inside: the far side.
        assert_eq!(
            ray_sphere([0.0; 3], [0.0, 0.0, 1.0], [0.0; 3], 1.0),
            Some(1.0)
        );
    }
}
