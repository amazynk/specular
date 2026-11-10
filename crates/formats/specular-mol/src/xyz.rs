//! Molecular XYZ: an atom count, a comment line, then `symbol x y z` (or an atomic number in
//! place of the symbol) per atom; several frames may follow one another (the first is read,
//! the rest counted). Point-cloud `.xyz` files (bare `x y z …` rows) share the extension and
//! are told apart by [`is_molecular_xyz`].

use crate::{
    Atom, AtomIn, Build, Error, Limits, MolFormat, Molecule, Result, Tag, element, int, num,
};

fn words(l: &[u8]) -> impl Iterator<Item = &[u8]> {
    l.split(|c| c.is_ascii_whitespace())
        .filter(|t| !t.is_empty())
}

fn element_of(t: &[u8]) -> Option<u8> {
    if t.first().is_some_and(u8::is_ascii_digit) {
        let z = int(t)?;
        return (1..=118).contains(&z).then_some(z as u8);
    }
    // Labels like "C1" or "Cl2" name the element by their letters.
    let letters = t.iter().take_while(|c| c.is_ascii_alphabetic()).count();
    if !(1..=2).contains(&letters) {
        return None;
    }
    element::by_symbol(&t[..letters])
}

/// An atom line: an element (symbol or atomic number) and three coordinates.
fn atom_line(l: &[u8]) -> Option<(u8, [f64; 3])> {
    let mut w = words(l);
    let e = element_of(w.next()?)?;
    let x = num(w.next()?)?;
    let y = num(w.next()?)?;
    let z = num(w.next()?)?;
    Some((e, [x, y, z]))
}

/// True when `head` starts with an atom count alone on its line, followed by a comment and
/// atom lines whose first word is an element: what tells a molecule from a point cloud.
pub fn is_molecular_xyz(head: &[u8]) -> bool {
    let mut lines = head
        .split(|&c| c == b'\n')
        .map(|l| l.strip_suffix(b"\r").unwrap_or(l));
    let Some(first) = lines.next() else {
        return false;
    };
    let mut w = words(first);
    let (Some(n), None) = (w.next().and_then(int), w.next()) else {
        return false;
    };
    if n < 1 {
        return false;
    }
    if lines.next().is_none() {
        return false;
    }
    // Every complete atom line in the head must parse; at least one must be present.
    let mut seen = 0;
    let complete = head.len() < 4096;
    let rest: Vec<&[u8]> = lines.take(n.min(64) as usize).collect();
    for (k, l) in rest.iter().enumerate() {
        let last = k + 1 == rest.len();
        match atom_line(l) {
            Some((_, _)) => {
                // The first word must be a symbol or a small integer, never a coordinate.
                let t = words(l).next().unwrap_or_default();
                if t.contains(&b'.') {
                    return false;
                }
                seen += 1;
            }
            // A line cut off at the end of the head is no evidence either way.
            None if last && !complete => {}
            None => return false,
        }
    }
    seen > 0
}

pub(crate) fn read(data: &[u8], limits: &Limits) -> Result<Molecule> {
    let mut lines = data
        .split(|&c| c == b'\n')
        .map(|l| l.strip_suffix(b"\r").unwrap_or(l));
    let first = lines.next().unwrap_or_default();
    let n = int(first.trim_ascii())
        .ok_or_else(|| Error::Malformed("XYZ: the first line is not an atom count".into()))?;
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
    let comment =
        String::from_utf8_lossy(lines.next().unwrap_or_default().trim_ascii()).into_owned();
    let mut b = Build::new(MolFormat::Xyz, limits);
    // Never reserve beyond what the file could hold (a line per atom of at least 7 bytes).
    b.reserve(n.min(data.len() / 7 + 1))?;
    for i in 0..n {
        let Some(l) = lines.next() else {
            return Err(Error::Malformed(format!(
                "XYZ: {n} atoms promised, {i} present"
            )));
        };
        let (e, p) = atom_line(l)
            .ok_or_else(|| Error::Malformed(format!("XYZ: atom line {} is unreadable", i + 1)))?;
        let sym = element::symbol(e);
        b.push(AtomIn {
            chain: b"",
            res_name: Tag::new(b"UNL"),
            seq: 1,
            icode: 0,
            sub: 0,
            atom: Atom {
                pos: p.map(|v| v as f32),
                b: 0.0,
                occupancy: 1.0,
                serial: i as i32 + 1,
                residue: 0,
                name: Tag::new(format!("{sym}{}", i + 1).as_bytes()),
                element: e,
                altloc: 0,
                charge: 0,
                het: true,
            },
        })?;
    }
    // Further frames: count those whose count line parses.
    let mut frames = 1;
    while let Some(l) = lines.by_ref().find(|l| !l.trim_ascii().is_empty()) {
        let Some(k) = int(l.trim_ascii()).filter(|&k| k > 0) else {
            break;
        };
        let mut ok = lines.next().is_some();
        for _ in 0..k {
            ok &= lines.next().is_some();
        }
        if !ok {
            break;
        }
        frames += 1;
    }
    let mut m = b.m;
    m.models = frames;
    m.records = frames;
    if !comment.is_empty() {
        m.info.title = Some(comment);
    }
    Ok(m)
}
