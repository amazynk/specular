//! Secondary structure: ranges from the file, or our DSSP-like assignment from backbone
//! hydrogen bonds (Kabsch & Sander's electrostatic energy, the amide hydrogen placed from
//! the previous carbonyl): α-helices from two consecutive i→i+4 turns, strands from
//! parallel and antiparallel bridges.

use std::collections::HashMap;

use rayon::prelude::*;

use crate::{Molecule, ResKind, Ss};

/// A helix or strand from the file, by author chain and residue number.
#[derive(Clone, Debug, PartialEq)]
pub struct SsRange {
    pub chain: String,
    pub from: (i32, u8),
    pub to: (i32, u8),
    pub ss: Ss,
}

/// Marks the residues of each range; true when any range matched.
pub(crate) fn apply(m: &mut Molecule, ranges: &[SsRange]) -> bool {
    if ranges.is_empty() {
        return false;
    }
    let mut at: HashMap<(&str, i32, u8), usize> = HashMap::new();
    for (i, r) in m.residues.iter().enumerate() {
        let c = m.chains[r.chain as usize].id.as_str();
        at.entry((c, r.seq, r.icode)).or_insert(i);
    }
    let mut marks: Vec<(usize, usize, Ss)> = Vec::new();
    for g in ranges {
        let (Some(&s), Some(&e)) = (
            at.get(&(g.chain.as_str(), g.from.0, g.from.1)),
            at.get(&(g.chain.as_str(), g.to.0, g.to.1)),
        ) else {
            continue;
        };
        if e >= s && m.residues[s].chain == m.residues[e].chain {
            marks.push((s, e, g.ss));
        }
    }
    for &(s, e, ss) in &marks {
        for r in &mut m.residues[s..=e] {
            r.ss = ss;
        }
    }
    !marks.is_empty()
}

/// Backbone of one amino acid.
#[derive(Clone, Copy)]
struct Bb {
    n: [f32; 3],
    ca: [f32; 3],
    c: [f32; 3],
    o: [f32; 3],
    /// The amide hydrogen (none for the first of a segment, or proline).
    h: Option<[f32; 3]>,
    /// Residue index.
    res: usize,
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn d2(a: [f32; 3], b: [f32; 3]) -> f32 {
    let d = sub(a, b);
    d[0] * d[0] + d[1] * d[1] + d[2] * d[2]
}

/// Kabsch–Sander energy (kcal/mol) of the hydrogen bond from the N–H of `d` to the C=O of `a`.
fn energy(a: &Bb, d: &Bb) -> f32 {
    let Some(h) = d.h else {
        return 0.0;
    };
    let r_on = d2(a.o, d.n).sqrt();
    let r_ch = d2(a.c, h).sqrt();
    let r_oh = d2(a.o, h).sqrt();
    let r_cn = d2(a.c, d.n).sqrt();
    if r_on < 0.5 || r_ch < 0.5 || r_oh < 0.5 || r_cn < 0.5 {
        return -9.9;
    }
    0.084 * 332.0 * (1.0 / r_on + 1.0 / r_ch - 1.0 / r_oh - 1.0 / r_cn)
}

/// Assigns helices and strands; false when there is no backbone to assign from.
pub(crate) fn assign(m: &mut Molecule) -> bool {
    // Backbones with all four heavy atoms, in residue order.
    let mut bb: Vec<Bb> = Vec::new();
    // Index into `bb` of the next residue in the same unbroken chain segment.
    for (ri, r) in m.residues.iter().enumerate() {
        if r.kind != ResKind::Amino {
            continue;
        }
        let mut f = [None; 4];
        for a in &m.atoms[r.atoms()] {
            match a.name.as_str() {
                "N" => f[0] = Some(a.pos),
                "CA" => f[1] = Some(a.pos),
                "C" => f[2] = Some(a.pos),
                "O" => f[3] = Some(a.pos),
                _ => {}
            }
        }
        if let [Some(n), Some(ca), Some(c), Some(o)] = f {
            bb.push(Bb {
                n,
                ca,
                c,
                o,
                h: None,
                res: ri,
            });
        }
    }
    if bb.len() < 3 {
        return false;
    }
    // Chain breaks: C(i)–N(i+1) longer than 2.5 Å.
    let n = bb.len();
    let linked: Vec<bool> = (0..n)
        .map(|i| {
            i + 1 < n
                && m.residues[bb[i].res].chain == m.residues[bb[i + 1].res].chain
                && d2(bb[i].c, bb[i + 1].n) < 2.5 * 2.5
        })
        .collect();
    for i in 1..n {
        if linked[i - 1] && m.residues[bb[i].res].name.as_str() != "PRO" {
            // H along the bisector-ish: N + unit(C=O of the previous residue), reversed.
            let p = &bb[i - 1];
            let co = sub(p.c, p.o);
            let l = (co[0] * co[0] + co[1] * co[1] + co[2] * co[2])
                .sqrt()
                .max(1e-6);
            let nn = bb[i].n;
            bb[i].h = Some([0, 1, 2].map(|k| nn[k] + co[k] / l));
        }
    }
    // Neighbours by Cα within 9 Å, through a grid.
    let cell = 9.0f32;
    let key = |p: [f32; 3]| p.map(|v| (v / cell).floor() as i32);
    let mut grid: HashMap<[i32; 3], Vec<usize>> = HashMap::new();
    for (i, b) in bb.iter().enumerate() {
        grid.entry(key(b.ca)).or_default().push(i);
    }
    // hb[i] holds each j whose N–H donates to the C=O of i.
    let hb: Vec<Vec<usize>> = (0..n)
        .into_par_iter()
        .map(|i| {
            let k = key(bb[i].ca);
            let mut out = Vec::new();
            for dx in -1..=1 {
                for dy in -1..=1 {
                    for dz in -1..=1 {
                        let Some(v) = grid.get(&[k[0] + dx, k[1] + dy, k[2] + dz]) else {
                            continue;
                        };
                        for &j in v {
                            if j.abs_diff(i) >= 2
                                && d2(bb[i].ca, bb[j].ca) <= 81.0
                                && energy(&bb[i], &bb[j]) < -0.5
                            {
                                out.push(j);
                            }
                        }
                    }
                }
            }
            out
        })
        .collect();
    let bonded = |i: usize, j: usize| hb.get(i).is_some_and(|v| v.contains(&j));
    // Consecutive within a segment.
    let seg = |i: usize, j: usize| -> bool { (i.min(j)..i.max(j)).all(|k| linked[k]) };
    let mut ss = vec![Ss::Coil; n];
    // α-helix: 4-turns at i-1 and i ⇒ residues i..=i+3.
    let turn4: Vec<bool> = (0..n)
        .map(|i| i + 4 < n && seg(i, i + 4) && bonded(i, i + 4))
        .collect();
    for i in 1..n {
        if turn4[i - 1] && turn4[i] {
            for s in &mut ss[i..(i + 4).min(n)] {
                *s = Ss::Helix;
            }
        }
    }
    // Bridges.
    let mut strand = vec![false; n];
    for i in 1..n.saturating_sub(1) {
        if !(seg(i - 1, i + 1)) {
            continue;
        }
        let k = key(bb[i].ca);
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let Some(v) = grid.get(&[k[0] + dx, k[1] + dy, k[2] + dz]) else {
                        continue;
                    };
                    for &j in v {
                        if j < 1 || j + 1 >= n || j.abs_diff(i) < 3 || !seg(j - 1, j + 1) {
                            continue;
                        }
                        let parallel = (bonded(i - 1, j) && bonded(j, i + 1))
                            || (bonded(j - 1, i) && bonded(i, j + 1));
                        let anti = (bonded(i, j) && bonded(j, i))
                            || (bonded(i - 1, j + 1) && bonded(j - 1, i + 1));
                        if parallel || anti {
                            strand[i] = true;
                            strand[j] = true;
                        }
                    }
                }
            }
        }
    }
    // Ladders: fill single-residue gaps between bridge residues (β-bulges).
    for i in 1..n.saturating_sub(1) {
        if !strand[i] && strand[i - 1] && strand[i + 1] && seg(i - 1, i + 1) {
            strand[i] = true;
        }
    }
    for i in 0..n {
        if strand[i] && ss[i] != Ss::Helix {
            ss[i] = Ss::Strand;
        }
    }
    // Lone strand residues (isolated bridges) read as coil in a cartoon.
    for i in 0..n {
        if ss[i] == Ss::Strand {
            let prev = i > 0 && seg(i - 1, i) && ss[i - 1] == Ss::Strand;
            let next = i + 1 < n && seg(i, i + 1) && ss[i + 1] == Ss::Strand;
            if !prev && !next {
                ss[i] = Ss::Coil;
            }
        }
    }
    let mut any = false;
    for (b, s) in bb.iter().zip(&ss) {
        m.residues[b.res].ss = *s;
        any |= *s != Ss::Coil;
    }
    any
}
