//! Tripos MOL2 and GROMACS GRO structures.
//!
//! - MOL2: `@<TRIPOS>MOLECULE` (name, counts), `@<TRIPOS>ATOM` (id, name, x y z in Å,
//!   SYBYL type naming the element before its dot, substructure id and name, charge) and
//!   `@<TRIPOS>BOND` (id, atoms, type: 1, 2, 3, `ar`, `am`, …). Several molecules may
//!   follow: the first is read, the rest counted.
//! - GRO: a title, the atom count, fixed columns (residue number and name, atom name and
//!   number, x y z in nm, optional velocities), the box. Elements come from atom names;
//!   residues and chains as in a PDB file (a new chain where residue numbers restart).
//!
//! Trajectories beside a structure (`.xtc`, `.trr`, `.dcd`) are not played.

use crate::{
    Atom, AtomIn, Bond, Build, Error, Limits, MolFormat, Molecule, Result, Tag, element, int, num,
};

fn words(l: &[u8]) -> impl Iterator<Item = &[u8]> {
    l.split(|c| c.is_ascii_whitespace())
        .filter(|t| !t.is_empty())
}

pub(crate) fn is_mol2(head: &[u8]) -> bool {
    let t = String::from_utf8_lossy(&head[..head.len().min(2048)]);
    t.lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with('#'))
        .is_some_and(|l| l.starts_with("@<TRIPOS>"))
}

/// A GRO head: a title, an atom count alone, then atom lines whose coordinates sit in the
/// fixed columns 20–44.
pub(crate) fn is_gro(head: &[u8]) -> bool {
    let mut lines = head
        .split(|&c| c == b'\n')
        .map(|l| l.strip_suffix(b"\r").unwrap_or(l));
    let _title = lines.next();
    let Some(n) = lines.next().and_then(|l| int(l.trim_ascii())) else {
        return false;
    };
    if n < 1 {
        return false;
    }
    let mut seen = 0;
    for l in lines.take(4) {
        if l.len() < 44 {
            break;
        }
        let ok = int(l[0..5].trim_ascii()).is_some()
            && (0..3).all(|k| {
                let c = &l[20 + 8 * k..28 + 8 * k];
                c.contains(&b'.') && num(c.trim_ascii()).is_some()
            });
        if !ok {
            return false;
        }
        seen += 1;
    }
    seen > 0
}

/// The element of a SYBYL atom type ("C.ar", "N.am", "Cl", "Du"…).
fn sybyl_element(t: &[u8]) -> Option<u8> {
    let sym = t.split(|&c| c == b'.').next()?;
    element::by_symbol(sym)
}

pub(crate) fn read_mol2(data: &[u8], limits: &Limits) -> Result<Molecule> {
    let text = String::from_utf8_lossy(data);
    let mut section = "";
    let mut b = Build::new(MolFormat::Mol2, limits);
    let mut bonds: Vec<Bond> = Vec::new();
    let mut ids: std::collections::HashMap<i64, u32> = Default::default();
    let mut molecules = 0;
    let mut name_next = false;
    let mut title = String::new();
    for l in text.lines() {
        let t = l.trim();
        if let Some(s) = t.strip_prefix("@<TRIPOS>") {
            section = match s {
                "MOLECULE" => {
                    molecules += 1;
                    name_next = true;
                    "MOLECULE"
                }
                "ATOM" => "ATOM",
                "BOND" => "BOND",
                _ => "",
            };
            continue;
        }
        if molecules > 1 || t.is_empty() || t.starts_with('#') {
            continue;
        }
        match section {
            "MOLECULE" if name_next => {
                title = t.to_owned();
                name_next = false;
            }
            "ATOM" => {
                let w: Vec<&[u8]> = words(t.as_bytes()).collect();
                if w.len() < 6 {
                    return Err(Error::Malformed(format!("MOL2: atom line “{t}”")));
                }
                let id = int(w[0]).unwrap_or(0);
                let p = [num(w[2]), num(w[3]), num(w[4])];
                let [Some(x), Some(y), Some(z)] = p else {
                    return Err(Error::Malformed(format!("MOL2: atom line “{t}”")));
                };
                let e = sybyl_element(w[5])
                    .or_else(|| {
                        let letters = w[1].iter().take_while(|c| c.is_ascii_alphabetic()).count();
                        element::by_symbol(&w[1][..letters.min(2)])
                            .or_else(|| element::by_symbol(&w[1][..letters.min(1)]))
                    })
                    .unwrap_or(0);
                let seq = w.get(6).and_then(|v| int(v)).unwrap_or(1) as i32;
                let res = w.get(7).copied().unwrap_or(b"UNL");
                // Residue names like "ALA12" carry the number too.
                let letters = res
                    .iter()
                    .take_while(|c| !c.is_ascii_digit())
                    .count()
                    .max(1);
                let i = b.m.atoms.len() as u32;
                ids.insert(id, i);
                b.push(AtomIn {
                    chain: b"",
                    res_name: Tag::new(&res[..letters.min(res.len())]),
                    seq,
                    icode: 0,
                    sub: 0,
                    atom: Atom {
                        pos: [x as f32, y as f32, z as f32],
                        b: 0.0,
                        occupancy: 1.0,
                        serial: id as i32,
                        residue: 0,
                        name: Tag::new(w[1]),
                        element: e,
                        altloc: 0,
                        charge: 0,
                        het: false,
                    },
                })?;
            }
            "BOND" => {
                let w: Vec<&[u8]> = words(t.as_bytes()).collect();
                if w.len() < 4 {
                    continue;
                }
                let (Some(a), Some(c)) = (
                    int(w[1]).and_then(|v| ids.get(&v)),
                    int(w[2]).and_then(|v| ids.get(&v)),
                ) else {
                    continue;
                };
                let order = match w[3] {
                    b"1" | b"am" => 1,
                    b"2" => 2,
                    b"3" => 3,
                    b"ar" => 4,
                    _ => 0,
                };
                bonds.push(Bond {
                    a: *a,
                    b: *c,
                    order,
                });
            }
            _ => {}
        }
    }
    let mut m = b.m;
    m.file_bonds = bonds.len();
    m.bonds = bonds;
    m.records = molecules.max(1);
    if !title.is_empty() {
        m.info.title = Some(title);
    }
    Ok(m)
}

/// The element of a GRO atom name: ions named for their residue (NA, CL, MG…), else the
/// first letter (GROMACS names carry no element column).
fn gro_element(name: &[u8], res: &[u8]) -> u8 {
    let n = name.trim_ascii();
    let letters: Vec<u8> = n.iter().copied().filter(u8::is_ascii_alphabetic).collect();
    if letters.is_empty() {
        return 0;
    }
    if n.eq_ignore_ascii_case(res.trim_ascii())
        && letters.len() <= 2
        && let Some(e) = element::by_symbol(&letters)
    {
        return e;
    }
    // Water's and others' hydrogens ("HW1", "OW").
    element::by_symbol(&letters[..1]).unwrap_or(0)
}

pub(crate) fn read_gro(data: &[u8], limits: &Limits) -> Result<Molecule> {
    let mut lines = data
        .split(|&c| c == b'\n')
        .map(|l| l.strip_suffix(b"\r").unwrap_or(l));
    let title = String::from_utf8_lossy(lines.next().unwrap_or_default().trim_ascii()).into_owned();
    let n = lines
        .next()
        .and_then(|l| int(l.trim_ascii()))
        .ok_or_else(|| Error::Malformed("GRO: no atom count".into()))?;
    if n < 1 {
        return Err(Error::Empty);
    }
    let n = n as usize;
    if n > limits.max_atoms {
        return Err(Error::TooLarge(format!(
            "{n} atoms (limit {})",
            limits.max_atoms
        )));
    }
    let mut b = Build::new(MolFormat::Gro, limits);
    b.reserve(n.min(data.len() / 40 + 1))?;
    let mut chain = 0u8;
    let mut last_seq = i32::MIN;
    for i in 0..n {
        let Some(l) = lines.next() else {
            return Err(Error::Malformed(format!(
                "GRO: {n} atoms promised, {i} present"
            )));
        };
        if l.len() < 44 {
            return Err(Error::Malformed(format!(
                "GRO: atom line {} is short",
                i + 1
            )));
        }
        let seq = int(l[0..5].trim_ascii()).unwrap_or(0) as i32;
        let res = &l[5..10];
        let name = &l[10..15];
        let p = [20, 28, 36].map(|c| num(l[c..c + 8].trim_ascii()));
        let [Some(x), Some(y), Some(z)] = p else {
            return Err(Error::Malformed(format!(
                "GRO: coordinates of atom {}",
                i + 1
            )));
        };
        // Residue numbers restarting (or wrapping at 100000) start a new chain.
        if seq < last_seq && seq <= 1 {
            chain = chain.wrapping_add(1);
        }
        last_seq = seq;
        let chain_id = [b'A' + chain % 26];
        b.push(AtomIn {
            chain: &chain_id,
            res_name: Tag::new(res),
            seq,
            icode: 0,
            sub: 0,
            atom: Atom {
                pos: [(x * 10.0) as f32, (y * 10.0) as f32, (z * 10.0) as f32],
                b: 0.0,
                occupancy: 1.0,
                serial: int(l[15..20].trim_ascii()).unwrap_or(i as i64 + 1) as i32,
                residue: 0,
                name: Tag::new(name),
                element: gro_element(name, res),
                altloc: 0,
                charge: 0,
                het: false,
            },
        })?;
    }
    let mut m = b.m;
    if !title.is_empty() {
        m.info.title = Some(title);
    }
    m.notes.push("coordinates in nm, shown in Å".into());
    Ok(m)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_mol2_and_gro() {
        let m2 = b"@<TRIPOS>MOLECULE\nwater\n3 2 1\nSMALL\n\n@<TRIPOS>ATOM\n1 O 0.0 0.0 0.0 O.3 1 HOH 0\n2 H1 0.96 0.0 0.0 H 1 HOH 0\n3 H2 -0.24 0.93 0.0 H 1 HOH 0\n@<TRIPOS>BOND\n1 1 2 1\n2 1 3 1\n";
        assert!(is_mol2(m2));
        let m = read_mol2(m2, &Limits::default()).unwrap();
        assert_eq!((m.atoms.len(), m.bonds.len()), (3, 2));
        assert_eq!(m.atoms[0].element, 8);
        let g = b"t\n2\n    1SOL     OW    1   0.126   1.624   1.679\n    1SOL    HW1    2   0.190   1.661   1.747\n   1.86206   1.86206   1.86206\n";
        assert!(is_gro(g));
        let m = read_gro(g, &Limits::default()).unwrap();
        assert_eq!(m.atoms[1].element, 1);
        assert!((m.atoms[0].pos[0] - 1.26).abs() < 1e-5);
    }
}
