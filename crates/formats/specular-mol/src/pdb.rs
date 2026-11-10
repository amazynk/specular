//! The PDB format (wwPDB 3.3): ATOM/HETATM of the first model, alternate locations (the
//! first), TER, MODEL/ENDMDL (counted), CONECT, HELIX/SHEET, CRYST1, HEADER, TITLE, EXPDTA
//! and REMARK 2's resolution.

use crate::ss::SsRange;
use crate::{
    Atom, AtomIn, Build, Cell, Error, Limits, MolFormat, Molecule, Result, Ss, SsSource, Tag,
    element, int, num,
};

/// Fixed columns, 1-based inclusive, as the format's documentation gives them.
fn col(line: &[u8], from: usize, to: usize) -> &[u8] {
    let a = (from - 1).min(line.len());
    let b = to.min(line.len());
    &line[a..b]
}

/// At least one coordinate record or a header record in the opening lines.
pub(crate) fn looks_like_pdb(head: &[u8]) -> bool {
    head.split(|&c| c == b'\n').take(64).any(|l| {
        (l.starts_with(b"ATOM  ") || l.starts_with(b"HETATM"))
            && l.len() >= 54
            && num(col(l, 31, 38)).is_some()
            || l.starts_with(b"HEADER    ")
            || l.starts_with(b"CRYST1 ")
    })
}

pub(crate) fn read(data: &[u8], limits: &Limits) -> Result<Molecule> {
    let mut b = Build::new(MolFormat::Pdb, limits);
    let mut models = 0usize;
    let mut in_first = true;
    let mut model_seen = false;
    let mut conect: Vec<(i32, i32)> = Vec::new();
    let mut ranges: Vec<SsRange> = Vec::new();
    let mut title = String::new();
    let mut method = String::new();
    let mut serial = 0i32;
    let mut lines = 0usize;
    // ATOM lines are 80 bytes or so: refuse before allocating for absurd files.
    let estimate = data.len() / 81;
    b.reserve(estimate.min(limits.max_atoms))?;
    for raw in data.split(|&c| c == b'\n') {
        lines += 1;
        let line = raw.strip_suffix(b"\r").unwrap_or(raw);
        if line.len() < 3 {
            continue;
        }
        let rec = col(line, 1, 6);
        match rec {
            b"ATOM  " | b"HETATM" | b"ATOM" | b"HETATM " => {
                if !in_first {
                    continue;
                }
                let het = rec.starts_with(b"HETATM");
                if line.len() < 54 {
                    return Err(Error::Malformed(format!(
                        "PDB line {lines}: coordinate record too short"
                    )));
                }
                let coord = |a, z| {
                    num(col(line, a, z)).ok_or_else(|| {
                        Error::Malformed(format!("PDB line {lines}: bad coordinate"))
                    })
                };
                let (x, y, z) = (coord(31, 38)?, coord(39, 46)?, coord(47, 54)?);
                serial += 1;
                let name_field = col(line, 13, 16);
                let res_name = Tag::new(col(line, 18, 20));
                let element = element_of(col(line, 77, 78), name_field, het);
                let alt = col(line, 17, 17).first().copied().unwrap_or(b' ');
                let charge = charge_of(col(line, 79, 80));
                let atom = Atom {
                    pos: [x as f32, y as f32, z as f32],
                    b: num(col(line, 61, 66)).unwrap_or(0.0) as f32,
                    occupancy: num(col(line, 55, 60)).unwrap_or(1.0) as f32,
                    serial: serial_of(col(line, 7, 11)).unwrap_or(serial),
                    residue: 0,
                    name: Tag::new(name_field),
                    element,
                    altloc: if alt == b' ' { 0 } else { alt },
                    charge,
                    het,
                };
                let icode = col(line, 27, 27).first().copied().unwrap_or(b' ');
                b.push(AtomIn {
                    chain: col(line, 22, 22),
                    res_name,
                    seq: seq_of(col(line, 23, 26)).unwrap_or(0),
                    icode: if icode == b' ' { 0 } else { icode },
                    sub: 0,
                    atom,
                })?;
            }
            b"TER   " | b"TER" => {
                if in_first {
                    b.ter();
                }
            }
            b"MODEL " => {
                models += 1;
                if model_seen && !b.m.atoms.is_empty() {
                    in_first = false;
                }
                model_seen = true;
            }
            b"ENDMDL" => {
                if !b.m.atoms.is_empty() {
                    in_first = false;
                }
            }
            b"CONECT" => {
                if let Some(a) = serial_of(col(line, 7, 11)) {
                    for (f, t) in [(12, 16), (17, 21), (22, 26), (27, 31)] {
                        if let Some(o) = serial_of(col(line, f, t)) {
                            conect.push((a, o));
                        }
                    }
                }
            }
            b"HELIX " => {
                let class = int(col(line, 39, 40)).unwrap_or(1);
                // Class 5 is a 3-10 helix; all count as helix for drawing.
                let _ = class;
                if let (Some(s), Some(e)) = (seq_of(col(line, 22, 25)), seq_of(col(line, 34, 37))) {
                    ranges.push(SsRange {
                        chain: chain_at(line, 20),
                        from: (s, icode_at(line, 26)),
                        to: (e, icode_at(line, 38)),
                        ss: Ss::Helix,
                    });
                }
            }
            b"SHEET " => {
                if let (Some(s), Some(e)) = (seq_of(col(line, 23, 26)), seq_of(col(line, 34, 37))) {
                    ranges.push(SsRange {
                        chain: chain_at(line, 22),
                        from: (s, icode_at(line, 27)),
                        to: (e, icode_at(line, 38)),
                        ss: Ss::Strand,
                    });
                }
            }
            b"CRYST1" => {
                let f = |a, z| num(col(line, a, z));
                if let (Some(a), Some(bb), Some(c), Some(al), Some(be), Some(ga)) = (
                    f(7, 15),
                    f(16, 24),
                    f(25, 33),
                    f(34, 40),
                    f(41, 47),
                    f(48, 54),
                ) {
                    let cell = Cell {
                        a,
                        b: bb,
                        c,
                        alpha: al,
                        beta: be,
                        gamma: ga,
                    };
                    if cell.is_valid() {
                        b.m.cell = Some(cell);
                    }
                }
                let sg = text(col(line, 56, 66));
                if !sg.is_empty() {
                    b.m.info.space_group = Some(sg);
                }
            }
            b"HEADER" => {
                let class = text(col(line, 11, 50));
                if !class.is_empty() {
                    b.m.info.classification = Some(class);
                }
                let date = text(col(line, 51, 59));
                if !date.is_empty() {
                    b.m.info.date = Some(date);
                }
                let id = text(col(line, 63, 66));
                if !id.is_empty() {
                    b.m.info.id = Some(id);
                }
            }
            b"TITLE " => append(&mut title, col(line, 11, 80)),
            b"EXPDTA" => append(&mut method, col(line, 11, 79)),
            b"REMARK" if col(line, 8, 10) == b"  2" => {
                let rest = col(line, 11, 80);
                if let Some(i) = find(rest, b"RESOLUTION.") {
                    let tail = &rest[i + 11..];
                    let tok = tail
                        .split(|c| c.is_ascii_whitespace())
                        .find(|t| !t.is_empty());
                    if let Some(r) = tok.and_then(num).filter(|r| *r > 0.0) {
                        b.m.info.resolution = Some(r);
                    }
                }
            }
            _ => {}
        }
    }
    let mut m = b.m;
    m.models = models.max(1);
    if !title.is_empty() {
        m.info.title = Some(title);
    }
    if !method.is_empty() {
        m.info.method = Some(method);
    }
    if crate::ss::apply(&mut m, &ranges) {
        m.ss_source = SsSource::File;
    }
    if !conect.is_empty() {
        let mut by_serial: Vec<(i32, u32)> = m
            .atoms
            .iter()
            .enumerate()
            .map(|(i, a)| (a.serial, i as u32))
            .collect();
        by_serial.sort_unstable();
        let find = |s: i32| {
            by_serial
                .binary_search_by_key(&s, |p| p.0)
                .ok()
                .map(|i| by_serial[i].1)
        };
        let mut bonds: Vec<(u32, u32)> = Vec::new();
        for (a, o) in conect {
            if let (Some(x), Some(y)) = (find(a), find(o))
                && x != y
            {
                bonds.push((x.min(y), x.max(y)));
            }
        }
        bonds.sort_unstable();
        // CONECT lists a bond from both ends, and repeats it for double bonds.
        let mut out: Vec<crate::Bond> = Vec::new();
        for (a, bb) in bonds {
            match out.last_mut() {
                Some(l) if l.a == a && l.b == bb => {}
                _ => out.push(crate::Bond { a, b: bb, order: 1 }),
            }
        }
        m.file_bonds = out.len();
        m.bonds = out;
    }
    Ok(m)
}

fn chain_at(line: &[u8], c: usize) -> String {
    String::from_utf8_lossy(&[byte(line, c)]).trim().to_owned()
}

fn byte(line: &[u8], c: usize) -> u8 {
    line.get(c - 1).copied().unwrap_or(b' ')
}

fn icode_at(line: &[u8], c: usize) -> u8 {
    match byte(line, c) {
        b' ' => 0,
        x => x,
    }
}

fn find(h: &[u8], n: &[u8]) -> Option<usize> {
    h.windows(n.len()).position(|w| w == n)
}

fn text(s: &[u8]) -> String {
    String::from_utf8_lossy(s.trim_ascii()).into_owned()
}

fn append(out: &mut String, s: &[u8]) {
    let t = text(s);
    if t.is_empty() {
        return;
    }
    if !out.is_empty() && !out.ends_with('-') {
        out.push(' ');
    }
    out.push_str(&t);
}

/// Serial numbers: decimal, or hybrid-36 past 99999.
fn serial_of(s: &[u8]) -> Option<i32> {
    let t = s.trim_ascii();
    if t.is_empty() {
        return None;
    }
    if let Some(v) = int(t) {
        return i32::try_from(v).ok();
    }
    hy36(t, 5)
}

/// Residue numbers: decimal, or hybrid-36 past 9999.
fn seq_of(s: &[u8]) -> Option<i32> {
    let t = s.trim_ascii();
    if let Some(v) = int(t) {
        return i32::try_from(v).ok();
    }
    hy36(t, 4)
}

/// Hybrid-36 (the PDB's overflow encoding for serials and residue numbers).
fn hy36(s: &[u8], width: u32) -> Option<i32> {
    if s.len() != width as usize {
        return None;
    }
    let digit = |c: u8| match c {
        b'0'..=b'9' => Some(u32::from(c - b'0')),
        b'A'..=b'Z' => Some(u32::from(c - b'A') + 10),
        b'a'..=b'z' => Some(u32::from(c - b'a') + 10),
        _ => None,
    };
    let upper = s[0].is_ascii_uppercase();
    if !s[0].is_ascii_alphabetic() {
        return None;
    }
    let mut v: u64 = 0;
    for &c in s {
        let d = digit(c)?;
        if c.is_ascii_alphabetic() && c.is_ascii_uppercase() != upper {
            return None;
        }
        v = v * 36 + u64::from(d);
    }
    let base = 10u64.pow(width);
    let first = 10 * 36u64.pow(width - 1);
    let n = if upper {
        v - first + base
    } else {
        v - first + base + 26 * 36u64.pow(width - 1)
    };
    i32::try_from(n).ok()
}

fn charge_of(s: &[u8]) -> i8 {
    let t = s.trim_ascii();
    match t {
        [d @ b'0'..=b'9', b'+'] => (d - b'0') as i8,
        [d @ b'0'..=b'9', b'-'] => -((d - b'0') as i8),
        _ => 0,
    }
}

/// The element from columns 77–78, else from the atom name as the format lays it out (a
/// one-letter element right-justified in columns 13–14, so column 13 is blank or a digit).
pub(crate) fn element_of(field: &[u8], name: &[u8], het: bool) -> u8 {
    if let Some(e) = element::by_symbol(field) {
        return e;
    }
    let n = name;
    if n.len() >= 2 {
        let c0 = n[0];
        if c0 == b' ' || c0.is_ascii_digit() {
            if let Some(e) = element::by_symbol(&n[1..2]) {
                return e;
            }
        } else if het && let Some(e) = element::by_symbol(&n[0..2]) {
            return e;
        }
        if n[0].is_ascii_alphabetic()
            && let Some(e) = element::by_symbol(&n[0..1])
        {
            return e;
        }
        if let Some(e) = element::by_symbol(&n[1..2]) {
            return e;
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hybrid_36() {
        assert_eq!(serial_of(b"99999"), Some(99999));
        assert_eq!(serial_of(b"A0000"), Some(100000));
        assert_eq!(serial_of(b"A0001"), Some(100001));
        assert_eq!(seq_of(b"A000"), Some(10000));
        assert_eq!(serial_of(b"a0000"), Some(100000 + 26 * 36i32.pow(4)));
        assert_eq!(seq_of(b"  -3"), Some(-3));
        assert_eq!(serial_of(b"     "), None);
    }

    #[test]
    fn elements_from_names() {
        assert_eq!(element_of(b"  ", b" CA ", false), 6);
        assert_eq!(element_of(b"  ", b"CA  ", true), 20);
        assert_eq!(element_of(b"FE", b"FE  ", true), 26);
        assert_eq!(element_of(b"  ", b"1HG1", false), 1);
        assert_eq!(element_of(b" N", b" N  ", false), 7);
    }

    #[test]
    fn charges() {
        assert_eq!(charge_of(b"2+"), 2);
        assert_eq!(charge_of(b"1-"), -1);
        assert_eq!(charge_of(b"  "), 0);
    }
}
