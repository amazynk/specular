//! Drawing styles and colour schemes: per-atom and per-residue sRGB colours by element
//! (CPK), chain, secondary structure, B-factor (through a colour map) and residue index.

use specular_inspect::ColourMap;

use crate::{Molecule, ResKind, Ss, element};

/// How atoms and residues are drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Style {
    /// A ribbon through the backbone (helices, arrows on strands, tubes for coil), the
    /// ligands as ball-and-stick.
    #[default]
    Cartoon,
    BallStick,
    /// Van der Waals spheres.
    Spacefill,
}

impl Style {
    pub const ALL: [Style; 3] = [Style::Cartoon, Style::BallStick, Style::Spacefill];

    pub fn id(self) -> &'static str {
        match self {
            Style::Cartoon => "cartoon",
            Style::BallStick => "ball-and-stick",
            Style::Spacefill => "spacefill",
        }
    }

    /// The next style; `cartoon` is false when the molecule has no polymer to draw it.
    pub fn next(self, cartoon: bool) -> Style {
        let n = match self {
            Style::Cartoon => Style::BallStick,
            Style::BallStick => Style::Spacefill,
            Style::Spacefill => Style::Cartoon,
        };
        if n == Style::Cartoon && !cartoon {
            Style::BallStick
        } else {
            n
        }
    }
}

/// What the colours mean.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Colouring {
    /// CPK colours by element.
    #[default]
    Element,
    /// A palette entry per chain (carbons only in the atom styles; others by element).
    Chain,
    /// Helix, strand and coil.
    Ss,
    /// Atomic displacement through viridis, between the 2nd and 98th percentile.
    BFactor,
    /// Blue at the N (5′) terminus to red at the C (3′) terminus of each chain.
    Rainbow,
}

impl Colouring {
    pub const ALL: [Colouring; 5] = [
        Colouring::Element,
        Colouring::Chain,
        Colouring::Ss,
        Colouring::BFactor,
        Colouring::Rainbow,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Colouring::Element => "element",
            Colouring::Chain => "chain",
            Colouring::Ss => "secondary-structure",
            Colouring::BFactor => "b-factor",
            Colouring::Rainbow => "rainbow",
        }
    }

    pub fn next(self) -> Colouring {
        let i = Self::ALL.iter().position(|&c| c == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }
}

/// Chain colours (sRGB), cycled.
pub const CHAIN_PALETTE: [[u8; 4]; 12] = [
    [0x4E, 0x9A, 0xF0, 255],
    [0xF2, 0x8E, 0x2B, 255],
    [0x59, 0xC2, 0x6A, 255],
    [0xE1, 0x57, 0x59, 255],
    [0xB0, 0x7A, 0xE0, 255],
    [0x3F, 0xC8, 0xC8, 255],
    [0xED, 0xC9, 0x48, 255],
    [0xFF, 0x9D, 0xA7, 255],
    [0x9C, 0x75, 0x5F, 255],
    [0x8C, 0xD1, 0x7D, 255],
    [0x76, 0xB7, 0xB2, 255],
    [0xBA, 0xB0, 0xAC, 255],
];

/// Helix, strand and coil (sRGB).
pub const SS_COLOURS: [[u8; 4]; 3] = [
    [0xE0, 0x30, 0x80, 255],
    [0xF5, 0xC8, 0x00, 255],
    [0xD8, 0xD8, 0xD8, 255],
];

fn ss_colour(s: Ss) -> [u8; 4] {
    match s {
        Ss::Helix => SS_COLOURS[0],
        Ss::Strand => SS_COLOURS[1],
        Ss::Coil => SS_COLOURS[2],
    }
}

fn to_u8(c: [f32; 3]) -> [u8; 4] {
    let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
    [q(c[0]), q(c[1]), q(c[2]), 255]
}

/// The B-factor range the colours span: the 2nd to 98th percentile of the atoms' values.
pub fn b_range(m: &Molecule) -> (f32, f32) {
    let mut v: Vec<f32> = m
        .atoms
        .iter()
        .map(|a| a.b)
        .filter(|b| b.is_finite())
        .collect();
    if v.is_empty() {
        return (0.0, 1.0);
    }
    let lo_i = (v.len() - 1) * 2 / 100;
    let hi_i = (v.len() - 1) * 98 / 100;
    let (_, lo, _) = v.select_nth_unstable_by(lo_i, f32::total_cmp);
    let lo = *lo;
    let (_, hi, _) = v.select_nth_unstable_by(hi_i, f32::total_cmp);
    let hi = *hi;
    if hi > lo { (lo, hi) } else { (lo, lo + 1.0) }
}

/// The colour map B-factors are drawn through.
pub const B_MAP: ColourMap = ColourMap::Viridis;

/// Rainbow position of each residue: 0 at its chain's first polymer residue, 1 at the last;
/// `None` for residues outside the polymer.
fn rainbow(m: &Molecule) -> Vec<Option<f32>> {
    let mut out = vec![None; m.residues.len()];
    for c in &m.chains {
        let idx: Vec<usize> = (c.first as usize..c.end as usize)
            .filter(|&r| matches!(m.residues[r].kind, ResKind::Amino | ResKind::Nucleic))
            .collect();
        let n = idx.len();
        for (k, r) in idx.into_iter().enumerate() {
            out[r] = Some(if n > 1 {
                k as f32 / (n - 1) as f32
            } else {
                0.5
            });
        }
    }
    out
}

fn rainbow_colour(t: f32) -> [u8; 4] {
    to_u8(ColourMap::Jet.sample(t))
}

/// One colour per atom.
pub fn atom_colours(m: &Molecule, c: Colouring) -> Vec<[u8; 4]> {
    let res_of = |i: usize| &m.residues[m.atoms[i].residue as usize];
    match c {
        Colouring::Element => m.atoms.iter().map(|a| element::cpk(a.element)).collect(),
        Colouring::Chain => m
            .atoms
            .iter()
            .enumerate()
            .map(|(i, a)| {
                if a.element == 6 {
                    CHAIN_PALETTE[res_of(i).chain as usize % CHAIN_PALETTE.len()]
                } else {
                    element::cpk(a.element)
                }
            })
            .collect(),
        Colouring::Ss => m
            .atoms
            .iter()
            .enumerate()
            .map(|(i, a)| {
                let r = res_of(i);
                if matches!(r.kind, ResKind::Amino | ResKind::Nucleic) {
                    ss_colour(r.ss)
                } else {
                    element::cpk(a.element)
                }
            })
            .collect(),
        Colouring::BFactor => {
            let (lo, hi) = b_range(m);
            m.atoms
                .iter()
                .map(|a| to_u8(B_MAP.sample((a.b - lo) / (hi - lo))))
                .collect()
        }
        Colouring::Rainbow => {
            let rb = rainbow(m);
            m.atoms
                .iter()
                .map(|a| match rb[a.residue as usize] {
                    Some(t) => rainbow_colour(t),
                    None => element::cpk(a.element),
                })
                .collect()
        }
    }
}

/// One colour per residue, for the cartoon. Element colouring draws the cartoon by
/// secondary structure (a ribbon has no element of its own).
pub fn residue_colours(m: &Molecule, c: Colouring) -> Vec<[u8; 4]> {
    match c {
        Colouring::Element | Colouring::Ss => m.residues.iter().map(|r| ss_colour(r.ss)).collect(),
        Colouring::Chain => m
            .residues
            .iter()
            .map(|r| CHAIN_PALETTE[r.chain as usize % CHAIN_PALETTE.len()])
            .collect(),
        Colouring::BFactor => {
            let (lo, hi) = b_range(m);
            m.residues
                .iter()
                .map(|r| {
                    let b = r.trace.map(|t| m.atoms[t as usize].b).unwrap_or_else(|| {
                        let s = &m.atoms[r.atoms()];
                        s.iter().map(|a| a.b).sum::<f32>() / s.len().max(1) as f32
                    });
                    to_u8(B_MAP.sample((b - lo) / (hi - lo)))
                })
                .collect()
        }
        Colouring::Rainbow => rainbow(m)
            .into_iter()
            .map(|t| rainbow_colour(t.unwrap_or(0.5)))
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cycles() {
        let mut c = Colouring::Element;
        for _ in 0..5 {
            c = c.next();
        }
        assert_eq!(c, Colouring::Element);
        assert_eq!(Style::Spacefill.next(false), Style::BallStick);
        assert_eq!(Style::Spacefill.next(true), Style::Cartoon);
        assert_eq!(Style::Cartoon.next(true), Style::BallStick);
    }
}
