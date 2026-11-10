//! MDL molfiles and SD files (CTfile V2000 and V3000): the first record's atoms, bonds and
//! charges (`M  CHG` in V2000, `CHG=` in V3000), its data items, and the record count.

use crate::{
    Atom, AtomIn, Bond, Build, Error, Limits, MolFormat, Molecule, Result, Tag, element, int, num,
};

/// A counts line ending in V2000 or V3000 on the fourth line.
pub(crate) fn looks_like_sdf(head: &[u8]) -> bool {
    let mut lines = head.split(|&c| c == b'\n');
    let Some(counts) = lines.nth(3) else {
        return false;
    };
    let c = counts.trim_ascii_end();
    c.ends_with(b"V2000") || c.ends_with(b"V3000")
}

fn lines_of(data: &[u8]) -> Vec<&[u8]> {
    data.split(|&c| c == b'\n')
        .map(|l| l.strip_suffix(b"\r").unwrap_or(l))
        .collect()
}

fn col(line: &[u8], from: usize, to: usize) -> &[u8] {
    let a = from.min(line.len());
    let b = to.min(line.len());
    &line[a..b]
}

pub(crate) fn read(data: &[u8], limits: &Limits) -> Result<Molecule> {
    let lines = lines_of(data);
    if lines.len() < 4 {
        return Err(Error::Malformed("SDF: fewer than four lines".into()));
    }
    let records = count_records(&lines);
    let counts = lines[3];
    let mut b = Build::new(MolFormat::Sdf, limits);
    let title = String::from_utf8_lossy(lines[0].trim_ascii()).into_owned();
    let mut bonds: Vec<Bond> = Vec::new();
    let end;
    if counts.trim_ascii_end().ends_with(b"V3000") {
        end = v3000(&lines, &mut b, &mut bonds, limits)?;
    } else {
        let field = |a, z| int(col(counts, a, z));
        let (na, nb) = match (field(0, 3), field(3, 6)) {
            (Some(a), Some(c)) => (a, c),
            _ => {
                let mut w = counts
                    .split(|c| c.is_ascii_whitespace())
                    .filter(|t| !t.is_empty());
                match (w.next().and_then(int), w.next().and_then(int)) {
                    (Some(a), Some(c)) => (a, c),
                    _ => return Err(Error::Malformed("SDF: unreadable counts line".into())),
                }
            }
        };
        if na < 0 || nb < 0 {
            return Err(Error::Malformed("SDF: negative counts".into()));
        }
        let (na, nb) = (na as usize, nb as usize);
        if na > limits.max_atoms {
            return Err(Error::TooLarge(format!(
                "{na} atoms (limit {})",
                limits.max_atoms
            )));
        }
        if 4 + na + nb > lines.len() {
            return Err(Error::Malformed(format!(
                "SDF: the counts line promises {na} atoms and {nb} bonds; the file is shorter"
            )));
        }
        b.reserve(na)?;
        for (i, l) in lines[4..4 + na].iter().enumerate() {
            let (pos, sym, chg) = if l.len() >= 34 {
                let p = [col(l, 0, 10), col(l, 10, 20), col(l, 20, 30)].map(num);
                (p, col(l, 31, 34), int(col(l, 36, 39)).unwrap_or(0))
            } else {
                ([None; 3], &b""[..], 0)
            };
            let (pos, sym) = match pos {
                [Some(x), Some(y), Some(z)] => ([x, y, z], sym),
                _ => {
                    let w: Vec<&[u8]> = l
                        .split(|c| c.is_ascii_whitespace())
                        .filter(|t| !t.is_empty())
                        .collect();
                    match (
                        w.first().and_then(|t| num(t)),
                        w.get(1).and_then(|t| num(t)),
                        w.get(2).and_then(|t| num(t)),
                        w.get(3),
                    ) {
                        (Some(x), Some(y), Some(z), Some(s)) => ([x, y, z], *s),
                        _ => {
                            return Err(Error::Malformed(format!(
                                "SDF: atom line {} is unreadable",
                                i + 1
                            )));
                        }
                    }
                }
            };
            // V2000 charge codes: 1 → +3, 2 → +2, 3 → +1, 5 → −1, 6 → −2, 7 → −3.
            let charge = match chg {
                1..=3 | 5..=7 => 4 - chg,
                _ => 0,
            } as i8;
            add_atom(&mut b, i, pos, sym, charge)?;
        }
        for (i, l) in lines[4 + na..4 + na + nb].iter().enumerate() {
            let (a, c, o) = (int(col(l, 0, 3)), int(col(l, 3, 6)), int(col(l, 6, 9)));
            let (a, c, o) = match (a, c, o) {
                (Some(a), Some(c), Some(o)) => (a, c, o),
                _ => {
                    let w: Vec<i64> = l
                        .split(|c| c.is_ascii_whitespace())
                        .filter(|t| !t.is_empty())
                        .filter_map(int)
                        .collect();
                    match w.as_slice() {
                        [a, c, o, ..] => (*a, *c, *o),
                        _ => {
                            return Err(Error::Malformed(format!(
                                "SDF: bond line {} is unreadable",
                                i + 1
                            )));
                        }
                    }
                }
            };
            push_bond(&mut bonds, a, c, o, na)?;
        }
        let mut k = 4 + na + nb;
        // Properties block: charges.
        while k < lines.len() && !lines[k].starts_with(b"M  END") && !lines[k].starts_with(b"$$$$")
        {
            let l = lines[k];
            if l.starts_with(b"M  CHG") {
                let w: Vec<i64> = l[6..]
                    .split(|c| c.is_ascii_whitespace())
                    .filter(|t| !t.is_empty())
                    .filter_map(int)
                    .collect();
                for p in w.get(1..).unwrap_or(&[]).chunks(2) {
                    if let [a, q] = p
                        && *a >= 1
                        && (*a as usize) <= b.m.atoms.len()
                    {
                        b.m.atoms[*a as usize - 1].charge = (*q).clamp(-127, 127) as i8;
                    }
                }
            }
            k += 1;
        }
        end = k;
    }
    let mut m = b.m;
    m.file_bonds = bonds.len();
    m.bonds = bonds;
    m.records = records;
    // Data items of the first record.
    let mut k = end;
    let mut items: Vec<(String, String)> = Vec::new();
    while k < lines.len() && !lines[k].starts_with(b"$$$$") {
        let l = lines[k];
        if l.starts_with(b">")
            && let (Some(a), Some(z)) = (
                l.iter().position(|&c| c == b'<'),
                l.iter().rposition(|&c| c == b'>'),
            )
            && z > a
        {
            let name = String::from_utf8_lossy(&l[a + 1..z]).into_owned();
            let value = lines
                .get(k + 1)
                .map(|v| String::from_utf8_lossy(v.trim_ascii()).into_owned())
                .unwrap_or_default();
            items.push((name, value));
        }
        k += 1;
    }
    let item = |n: &str| {
        items
            .iter()
            .find(|(k, v)| k.eq_ignore_ascii_case(n) && !v.is_empty())
            .map(|x| x.1.clone())
    };
    m.info.id = item("PUBCHEM_COMPOUND_CID")
        .or_else(|| item("ID"))
        .or_else(|| (!title.is_empty()).then(|| title.clone()));
    m.info.title = item("PUBCHEM_IUPAC_NAME")
        .or_else(|| item("NAME"))
        .or_else(|| (!title.is_empty()).then(|| title.clone()));
    m.info.formula = item("PUBCHEM_MOLECULAR_FORMULA").or_else(|| item("FORMULA"));
    Ok(m)
}

fn add_atom(b: &mut Build, i: usize, pos: [f64; 3], sym: &[u8], charge: i8) -> Result<()> {
    b.push(AtomIn {
        chain: b"",
        res_name: Tag::new(b"UNL"),
        seq: 1,
        icode: 0,
        sub: 0,
        atom: Atom {
            pos: pos.map(|v| v as f32),
            b: 0.0,
            occupancy: 1.0,
            serial: i as i32 + 1,
            residue: 0,
            name: Tag::new(
                format!("{}{}", String::from_utf8_lossy(sym.trim_ascii()), i + 1).as_bytes(),
            ),
            element: element::by_symbol(sym).unwrap_or(0),
            altloc: 0,
            charge,
            het: true,
        },
    })?;
    Ok(())
}

fn push_bond(bonds: &mut Vec<Bond>, a: i64, c: i64, o: i64, na: usize) -> Result<()> {
    if a < 1 || c < 1 || a as usize > na || c as usize > na || a == c {
        return Err(Error::Malformed(format!(
            "SDF: bond {a}–{c} names an atom that does not exist"
        )));
    }
    let (a, c) = (a as u32 - 1, c as u32 - 1);
    bonds.push(Bond {
        a: a.min(c),
        b: a.max(c),
        order: match o {
            1..=4 => o as u8,
            _ => 0,
        },
    });
    Ok(())
}

/// The V3000 connection table; returns the line after `M  END`.
fn v3000(lines: &[&[u8]], b: &mut Build, bonds: &mut Vec<Bond>, limits: &Limits) -> Result<usize> {
    // Joins continuation lines (a trailing '-').
    let mut k = 4;
    let mut section = "";
    let mut ids: Vec<(i64, u32)> = Vec::new();
    let mut na_declared = 0usize;
    while k < lines.len() {
        let mut l: Vec<u8> = Vec::new();
        loop {
            let raw = lines[k];
            k += 1;
            let body = raw.strip_prefix(b"M  V30 ").unwrap_or(raw);
            let t = body.trim_ascii_end();
            if let Some(stem) = t.strip_suffix(b"-")
                && k < lines.len()
            {
                l.extend_from_slice(stem);
                continue;
            }
            l.extend_from_slice(t);
            break;
        }
        if lines[k - 1].starts_with(b"M  END") {
            break;
        }
        let w: Vec<&[u8]> = l
            .split(|c| c.is_ascii_whitespace())
            .filter(|t| !t.is_empty())
            .collect();
        match w.as_slice() {
            [b"BEGIN", s, ..] => {
                section = match *s {
                    b"ATOM" => "atom",
                    b"BOND" => "bond",
                    _ => "other",
                }
            }
            [b"END", ..] => section = "",
            [b"COUNTS", na, ..] => {
                na_declared = int(na).unwrap_or(0).max(0) as usize;
                if na_declared > limits.max_atoms {
                    return Err(Error::TooLarge(format!(
                        "{na_declared} atoms (limit {})",
                        limits.max_atoms
                    )));
                }
                // Each atom is a line of its own: no more than the lines left.
                b.reserve(na_declared.min(lines.len().saturating_sub(k)))?;
            }
            [id, sym, x, y, z, rest @ ..] if section == "atom" => {
                let (Some(id), Some(x), Some(y), Some(z)) = (int(id), num(x), num(y), num(z))
                else {
                    return Err(Error::Malformed("SDF V3000: unreadable atom".into()));
                };
                let chg = rest
                    .iter()
                    .find_map(|t| t.strip_prefix(b"CHG=").and_then(int))
                    .unwrap_or(0);
                let i = b.m.atoms.len();
                ids.push((id, i as u32));
                add_atom(b, i, [x, y, z], sym, chg.clamp(-127, 127) as i8)?;
            }
            [_, o, a, c, ..] if section == "bond" => {
                let find = |v: &[u8]| {
                    let id = int(v)?;
                    ids.iter().find(|p| p.0 == id).map(|p| i64::from(p.1) + 1)
                };
                let (Some(o), Some(a), Some(c)) = (int(o), find(a), find(c)) else {
                    return Err(Error::Malformed("SDF V3000: unreadable bond".into()));
                };
                push_bond(bonds, a, c, o, ids.len())?;
            }
            _ => {}
        }
    }
    let _ = na_declared;
    Ok(k)
}

fn count_records(lines: &[&[u8]]) -> usize {
    let mut n = 0;
    let mut content = false;
    for l in lines {
        if l.starts_with(b"$$$$") {
            n += 1;
            content = false;
        } else if !l.trim_ascii().is_empty() {
            content = true;
        }
    }
    n + usize::from(content)
}
