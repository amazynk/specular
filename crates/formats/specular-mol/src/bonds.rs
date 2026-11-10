//! Residue classification and bonding: a table of the standard amino acids' and
//! nucleotides' heavy-atom bonds, polymer links (peptide C–N, phosphodiester O3′–P),
//! hydrogens to their nearest heavy atom, and distance-based bonding (covalent radii plus a
//! tolerance, through a uniform grid) for ligands and everything the table does not know.

use rayon::prelude::*;

use crate::{Bond, MolFormat, Molecule, ResKind, element};

/// Heavy-atom bonds by residue: the residue names, then bonded atom-name pairs.
pub const RESIDUE_BONDS: &[(&[&str], &str)] = &[
    (&["ALA"], "CA-CB"),
    (&["ARG"], "CA-CB CB-CG CG-CD CD-NE NE-CZ CZ-NH1 CZ-NH2"),
    (&["ASN"], "CA-CB CB-CG CG-OD1 CG-ND2"),
    (&["ASP"], "CA-CB CB-CG CG-OD1 CG-OD2"),
    (&["CYS"], "CA-CB CB-SG"),
    (&["GLN"], "CA-CB CB-CG CG-CD CD-OE1 CD-NE2"),
    (&["GLU"], "CA-CB CB-CG CG-CD CD-OE1 CD-OE2"),
    (&["GLY"], ""),
    (
        &["HIS"],
        "CA-CB CB-CG CG-ND1 CG-CD2 ND1-CE1 CE1-NE2 NE2-CD2",
    ),
    (&["ILE"], "CA-CB CB-CG1 CB-CG2 CG1-CD1"),
    (&["LEU"], "CA-CB CB-CG CG-CD1 CG-CD2"),
    (&["LYS"], "CA-CB CB-CG CG-CD CD-CE CE-NZ"),
    (&["MET"], "CA-CB CB-CG CG-SD SD-CE"),
    (&["MSE"], "CA-CB CB-CG CG-SE SE-CE"),
    (
        &["PHE"],
        "CA-CB CB-CG CG-CD1 CG-CD2 CD1-CE1 CD2-CE2 CE1-CZ CE2-CZ",
    ),
    (&["PRO"], "CA-CB CB-CG CG-CD CD-N"),
    (&["SER"], "CA-CB CB-OG"),
    (&["THR"], "CA-CB CB-OG1 CB-CG2"),
    (
        &["TRP"],
        "CA-CB CB-CG CG-CD1 CG-CD2 CD1-NE1 NE1-CE2 CD2-CE2 CD2-CE3 CE2-CZ2 CE3-CZ3 CZ2-CH2 CZ3-CH2",
    ),
    (
        &["TYR"],
        "CA-CB CB-CG CG-CD1 CG-CD2 CD1-CE1 CD2-CE2 CE1-CZ CE2-CZ CZ-OH",
    ),
    (&["VAL"], "CA-CB CB-CG1 CB-CG2"),
    (
        &["A", "DA", "G", "DG", "I", "DI"],
        "C1'-N9 N9-C8 C8-N7 N7-C5 C5-C6 C6-N1 N1-C2 C2-N3 N3-C4 C4-C5 C4-N9",
    ),
    (&["A", "DA"], "C6-N6"),
    (&["G", "DG"], "C6-O6 C2-N2"),
    (&["I", "DI"], "C6-O6"),
    (
        &["C", "DC", "U", "DU", "T", "DT"],
        "C1'-N1 N1-C2 C2-O2 C2-N3 N3-C4 C4-C5 C5-C6 C6-N1",
    ),
    (&["C", "DC"], "C4-N4"),
    (&["U", "DU", "T", "DT"], "C4-O4"),
    (&["T", "DT"], "C5-C7 C5-C5M"),
];

const AMINO_BACKBONE: &str = "N-CA CA-C C-O C-OXT";
const NUCLEIC_BACKBONE: &str = "P-OP1 P-OP2 P-OP3 P-O1P P-O2P P-O5' O5'-C5' C5'-C4' C4'-O4' C4'-C3' C3'-O3' C3'-C2' C2'-C1' C1'-O4' C2'-O2'";

const WATERS: &[&str] = &["HOH", "WAT", "H2O", "DOD", "SOL", "TIP", "TIP3", "D2O"];

fn atom_named(m: &Molecule, r: usize, name: &str) -> Option<usize> {
    m.residues[r]
        .atoms()
        .find(|&i| m.atoms[i].name.as_str() == name)
}

/// Sets each residue's kind and trace atom.
pub(crate) fn classify(m: &mut Molecule) {
    let small = matches!(
        m.format,
        Some(MolFormat::Sdf | MolFormat::Xyz | MolFormat::Cif)
    );
    for ri in 0..m.residues.len() {
        let name = m.residues[ri].name;
        let (kind, trace) = if small {
            (ResKind::Other, None)
        } else if WATERS.contains(&name.as_str()) {
            (ResKind::Water, None)
        } else if let Some(ca) = atom_named(m, ri, "CA").filter(|&i| m.atoms[i].element == 6)
            && (atom_named(m, ri, "N").is_some() || atom_named(m, ri, "C").is_some())
        {
            (ResKind::Amino, Some(ca as u32))
        } else if let Some(c4) = atom_named(m, ri, "C4'").or_else(|| atom_named(m, ri, "C4*"))
            && (atom_named(m, ri, "C1'").is_some()
                || atom_named(m, ri, "C1*").is_some()
                || atom_named(m, ri, "P").is_some())
        {
            (
                ResKind::Nucleic,
                Some(atom_named(m, ri, "P").unwrap_or(c4) as u32),
            )
        } else {
            (ResKind::Other, None)
        };
        // The single "residue" of SDF, XYZ and small-molecule CIF is a molecule, not a polymer.
        let r = &mut m.residues[ri];
        r.kind = kind;
        r.trace = trace;
    }
}

fn d2(a: [f32; 3], b: [f32; 3]) -> f32 {
    let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    d[0] * d[0] + d[1] * d[1] + d[2] * d[2]
}

fn pairs(spec: &str) -> impl Iterator<Item = (&str, &str)> {
    spec.split_whitespace().filter_map(|p| p.split_once('-'))
}

/// Bonds what the file did not.
pub(crate) fn complete(m: &mut Molecule) {
    match m.format {
        Some(MolFormat::Cif) => return,
        Some(MolFormat::Sdf | MolFormat::Mol2) if !m.bonds.is_empty() => return,
        Some(MolFormat::Sdf | MolFormat::Xyz | MolFormat::Mol2) | None => {
            let pos: Vec<[f32; 3]> = m.atoms.iter().map(|a| a.pos).collect();
            let els: Vec<u8> = m.atoms.iter().map(|a| a.element).collect();
            m.bonds = bond_by_distance(&pos, &els, None)
                .into_iter()
                .map(|(a, b)| Bond { a, b, order: 0 })
                .collect();
            return;
        }
        Some(MolFormat::Pdb | MolFormat::Mmcif | MolFormat::Gro) => {}
    }
    let mut out: Vec<Bond> = std::mem::take(&mut m.bonds);
    // Residues the table covers; the rest are bonded by distance.
    let mut tabled = vec![false; m.residues.len()];
    let mut names: Vec<(&str, usize)> = Vec::new();
    for (ri, r) in m.residues.iter().enumerate() {
        let rn = r.name.as_str();
        let specs: Vec<&str> = RESIDUE_BONDS
            .iter()
            .filter(|(ns, _)| ns.contains(&rn))
            .map(|(_, s)| *s)
            .collect();
        if r.kind == ResKind::Water {
            tabled[ri] = true;
        }
        let backbone = match r.kind {
            ResKind::Amino if !specs.is_empty() => AMINO_BACKBONE,
            ResKind::Nucleic if !specs.is_empty() => NUCLEIC_BACKBONE,
            ResKind::Water => "",
            _ => continue,
        };
        tabled[ri] = true;
        names.clear();
        names.extend(r.atoms().map(|i| (m.atoms[i].name.as_str(), i)));
        let find = |n: &str| {
            names.iter().find(|x| x.0 == n).map(|x| x.1).or_else(|| {
                // Old-style primes.
                let alt = n.replace('\'', "*");
                names.iter().find(|x| x.0 == alt).map(|x| x.1)
            })
        };
        for (a, b) in specs.iter().flat_map(|s| pairs(s)).chain(pairs(backbone)) {
            if let (Some(i), Some(j)) = (find(a), find(b))
                && d2(m.atoms[i].pos, m.atoms[j].pos) < 2.4 * 2.4
            {
                out.push(Bond {
                    a: i.min(j) as u32,
                    b: i.max(j) as u32,
                    order: 0,
                });
            }
        }
        // Hydrogens to the nearest heavy atom of the residue.
        for i in r.atoms() {
            if m.atoms[i].element != 1 {
                continue;
            }
            let best = r
                .atoms()
                .filter(|&j| m.atoms[j].element != 1)
                .map(|j| (d2(m.atoms[i].pos, m.atoms[j].pos), j))
                .min_by(|a, b| a.0.total_cmp(&b.0));
            if let Some((d, j)) = best
                && d < 1.35 * 1.35
            {
                out.push(Bond {
                    a: i.min(j) as u32,
                    b: i.max(j) as u32,
                    order: 0,
                });
            }
        }
    }
    // Polymer links between consecutive residues of a chain.
    for c in &m.chains {
        for ri in c.first as usize + 1..c.end as usize {
            let (p, q) = (&m.residues[ri - 1], &m.residues[ri]);
            let link = match (p.kind, q.kind) {
                (ResKind::Amino, ResKind::Amino) => Some(("C", "N")),
                (ResKind::Nucleic, ResKind::Nucleic) => Some(("O3'", "P")),
                _ => None,
            };
            if let Some((a, b)) = link
                && let (Some(i), Some(j)) = (atom_named(m, ri - 1, a), atom_named(m, ri, b))
                && d2(m.atoms[i].pos, m.atoms[j].pos) < 2.0 * 2.0
            {
                out.push(Bond {
                    a: i.min(j) as u32,
                    b: i.max(j) as u32,
                    order: 0,
                });
            }
        }
    }
    // Distance bonding where either atom is in a residue the table does not cover (but
    // never for waters' oxygens to anything outside the water), plus disulfides.
    if tabled.iter().any(|t| !t) || m.atoms.iter().any(|a| a.element == 16) {
        let pos: Vec<[f32; 3]> = m.atoms.iter().map(|a| a.pos).collect();
        let els: Vec<u8> = m.atoms.iter().map(|a| a.element).collect();
        let res: Vec<u32> = m.atoms.iter().map(|a| a.residue).collect();
        let water: Vec<bool> = m
            .residues
            .iter()
            .map(|r| r.kind == ResKind::Water)
            .collect();
        let wanted = |i: usize, j: usize| {
            let (ri, rj) = (res[i] as usize, res[j] as usize);
            if water[ri] || water[rj] {
                return false;
            }
            !tabled[ri] || !tabled[rj] || (els[i] == 16 && els[j] == 16 && ri != rj)
        };
        let cand: Vec<bool> = (0..pos.len())
            .map(|i| !tabled[res[i] as usize] || els[i] == 16)
            .collect();
        for (a, b) in distance_pairs(&pos, &els, Some(&cand), wanted, 0.45) {
            out.push(Bond { a, b, order: 0 });
        }
    }
    out.par_sort_unstable_by_key(|b| (b.a, b.b));
    out.dedup_by_key(|b| (b.a, b.b));
    m.bonds = out;
}

/// Bonds by distance between all atoms: `d < r₁ + r₂ + tolerance` (covalent radii;
/// tolerance 0.45 Å unless given), never hydrogen to hydrogen.
pub fn bond_by_distance(pos: &[[f32; 3]], els: &[u8], tol: Option<f32>) -> Vec<(u32, u32)> {
    distance_pairs(pos, els, None, |_, _| true, tol.unwrap_or(0.45))
}

fn distance_pairs(
    pos: &[[f32; 3]],
    els: &[u8],
    cand: Option<&[bool]>,
    wanted: impl Fn(usize, usize) -> bool + Sync,
    tol: f32,
) -> Vec<(u32, u32)> {
    if pos.is_empty() {
        return Vec::new();
    }
    let rmax = els
        .iter()
        .map(|&e| element::covalent(e))
        .fold(0.0f32, f32::max)
        .min(2.0);
    let mut cell = 2.0 * rmax + tol;
    let mut lo = [f32::MAX; 3];
    let mut hi = [f32::MIN; 3];
    for p in pos {
        for k in 0..3 {
            if p[k].is_finite() {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
    }
    if lo[0] > hi[0] {
        return Vec::new();
    }
    let mut dims = [0usize; 3];
    loop {
        // In f64 and capped: coordinates near ±f32::MAX span an infinite f32 range, and
        // spans past 2⁶⁴ cells would overflow the counts.
        for k in 0..3 {
            let span = f64::from(hi[k]) - f64::from(lo[k]);
            dims[k] = (span / f64::from(cell)).min(1e9) as usize + 1;
        }
        if dims[0].saturating_mul(dims[1]).saturating_mul(dims[2]) <= 1 << 24 {
            break;
        }
        cell *= 1.5;
    }
    let key = |p: [f32; 3]| -> Option<usize> {
        if !(p[0].is_finite() && p[1].is_finite() && p[2].is_finite()) {
            return None;
        }
        let c = [0, 1, 2].map(|k| (((p[k] - lo[k]) / cell) as usize).min(dims[k] - 1));
        Some((c[2] * dims[1] + c[1]) * dims[0] + c[0])
    };
    // Counting sort into cells.
    let ncell = dims[0] * dims[1] * dims[2];
    let mut start = vec![0u32; ncell + 1];
    let keys: Vec<Option<usize>> = pos.iter().map(|&p| key(p)).collect();
    for k in keys.iter().flatten() {
        start[k + 1] += 1;
    }
    for i in 0..ncell {
        start[i + 1] += start[i];
    }
    let mut fill = start.clone();
    let mut order = vec![0u32; pos.len()];
    for (i, k) in keys.iter().enumerate() {
        if let Some(k) = k {
            order[fill[*k] as usize] = i as u32;
            fill[*k] += 1;
        }
    }
    let radii: Vec<f32> = els.iter().map(|&e| element::covalent(e)).collect();
    (0..pos.len())
        .into_par_iter()
        .with_min_len(4096)
        .flat_map_iter(|i| {
            let mut found: Vec<(u32, u32)> = Vec::new();
            let is_cand = |k: usize| cand.is_none_or(|c| c[k]);
            if keys[i].is_none() || !is_cand(i) {
                return found.into_iter();
            }
            let p = pos[i];
            let c = [0, 1, 2].map(|k| (((p[k] - lo[k]) / cell) as usize).min(dims[k] - 1));
            for z in c[2].saturating_sub(1)..=(c[2] + 1).min(dims[2] - 1) {
                for y in c[1].saturating_sub(1)..=(c[1] + 1).min(dims[1] - 1) {
                    for x in c[0].saturating_sub(1)..=(c[0] + 1).min(dims[0] - 1) {
                        let k = (z * dims[1] + y) * dims[0] + x;
                        for &j in &order[start[k] as usize..start[k + 1] as usize] {
                            let j = j as usize;
                            if (j <= i && is_cand(j)) || j == i || (els[i] == 1 && els[j] == 1) {
                                continue;
                            }
                            let lim = radii[i] + radii[j] + tol;
                            let d = d2(p, pos[j]);
                            if d < lim * lim && d > 0.16 && wanted(i, j) {
                                found.push((i.min(j) as u32, i.max(j) as u32));
                            }
                        }
                    }
                }
            }
            found.into_iter()
        })
        .collect()
}

#[cfg(test)]
mod hostile {
    #[test]
    fn coordinates_at_the_f32_limits() {
        for x in [1e20f32, f32::MAX] {
            let pos = [[-x, 0.0, 0.0], [x, 0.0, 0.0], [x, 1.0, 0.0]];
            assert_eq!(super::bond_by_distance(&pos, &[6, 6, 6], None), [(1, 2)]);
        }
    }
}
