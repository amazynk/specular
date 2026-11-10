//! Our readers against gemmi (PDB, mmCIF, small-molecule CIF with symmetry expansion,
//! secondary structure from the file) and RDKit (SDF, XYZ), through the rows
//! `gen/oracle.py` wrote: tests/data/expected.tsv for the committed fixtures, and the
//! corpus's expected.tsv when SPECULAR_MOL_DIR and SPECULAR_MOL_ORACLE name it.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use specular_mol::{Limits, MolFormat, Molecule, Ss, element, is_molecular_xyz, open};

#[derive(Default)]
struct Tally {
    checked: BTreeMap<&'static str, usize>,
    bad: BTreeMap<&'static str, usize>,
    notes: Vec<String>,
    /// Residues where our DSSP-like assignment agrees with the file, of those compared.
    dssp: (usize, usize),
}

impl Tally {
    fn agree(&mut self, same: bool) {
        self.dssp.0 += usize::from(same);
        self.dssp.1 += 1;
    }
    fn check(&mut self, kind: &'static str, ok: bool, what: impl FnOnce() -> String) {
        *self.checked.entry(kind).or_default() += 1;
        if !ok {
            *self.bad.entry(kind).or_default() += 1;
            if self.notes.len() < 40 {
                self.notes.push(what());
            }
        }
    }
    fn mismatches(&self) -> usize {
        self.bad.values().sum()
    }
}

fn close(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol
}

fn norm_sg(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_uppercase()
}

fn ss_char(s: Ss) -> &'static str {
    match s {
        Ss::Helix => "H",
        Ss::Strand => "E",
        Ss::Coil => "C",
    }
}

fn icode_str(c: u8) -> String {
    if c == 0 || c == b' ' {
        ".".into()
    } else {
        (c as char).to_string()
    }
}

fn compare_file(dir: &Path, file: &str, rows: &[Vec<&str>], t: &mut Tally) {
    let path = dir.join(file);
    let data = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    if rows.iter().any(|r| r[1] == "notmol") {
        t.check(
            "xyz-sniff",
            !is_molecular_xyz(&data[..data.len().min(4096)]),
            || format!("{file}: point cloud taken for a molecule"),
        );
        return;
    }
    let started = std::time::Instant::now();
    let m = match open(&data, None, &Limits::default()) {
        Ok(m) => m,
        Err(e) => {
            t.check("open", false, || format!("{file}: {e}"));
            return;
        }
    };
    let dt = started.elapsed();
    if m.atoms.len() > 100_000 {
        eprintln!(
            "{file}: {} atoms, {} residues, {} bonds parsed in {dt:.2?}",
            m.atoms.len(),
            m.residues.len(),
            m.bonds.len()
        );
    }
    t.check("open", true, String::new);
    let stem = file.split('.').next().unwrap_or(file);
    // Atoms by (chain, seq, icode, residue, name).
    let mut by_key: HashMap<(String, String, String, String, String), Vec<usize>> = HashMap::new();
    let mut res_by_key: HashMap<(String, String, String, String), usize> = HashMap::new();
    let macro_ = matches!(m.format, Some(MolFormat::Pdb | MolFormat::Mmcif));
    if macro_ {
        for (i, a) in m.atoms.iter().enumerate() {
            let r = &m.residues[a.residue as usize];
            let c = &m.chains[r.chain as usize].id;
            by_key
                .entry((
                    c.clone(),
                    r.seq.to_string(),
                    icode_str(r.icode),
                    r.name.as_str().to_owned(),
                    a.name.as_str().to_owned(),
                ))
                .or_default()
                .push(i);
        }
        for (i, r) in m.residues.iter().enumerate() {
            let c = &m.chains[r.chain as usize].id;
            res_by_key
                .entry((
                    c.clone(),
                    r.seq.to_string(),
                    icode_str(r.icode),
                    r.name.as_str().to_owned(),
                ))
                .or_insert(i);
        }
    }
    // Our own assignment, to report how far it agrees with the file's.
    let dssp = macro_.then(|| {
        let mut d = m.clone();
        d.compute_secondary_structure();
        d
    });
    let mut sites: Vec<(u8, [f64; 3])> = Vec::new();
    let mut bonds: Vec<(u32, u32, u8)> = Vec::new();
    for r in rows {
        let f = |k: usize| r.get(k).copied().unwrap_or("");
        let n = |k: usize| f(k).parse::<f64>().unwrap_or(f64::NAN);
        match r[1] {
            "summary" => {
                let (atoms, residues, models) = (n(2) as usize, n(3) as usize, n(4) as usize);
                t.check("atom count", m.atoms.len() == atoms, || {
                    format!("{file}: {} atoms, gemmi {atoms}", m.atoms.len())
                });
                if macro_ {
                    t.check("residue count", m.residues.len() == residues, || {
                        format!("{file}: {} residues, gemmi {residues}", m.residues.len())
                    });
                    t.check("models", m.models == models, || {
                        format!("{file}: {} models, gemmi {models}", m.models)
                    });
                }
            }
            "info" => {
                if macro_ && f(2) != "." && !f(2).eq_ignore_ascii_case(stem) {
                    let ok = m
                        .info
                        .id
                        .as_deref()
                        .is_some_and(|i| i.eq_ignore_ascii_case(f(2)));
                    t.check("entry id", ok, || {
                        format!("{file}: id {:?} vs {}", m.info.id, f(2))
                    });
                }
                if f(3) != "." {
                    let ok = m.info.resolution.is_some_and(|v| close(v, n(3), 0.006));
                    t.check("resolution", ok, || {
                        format!("{file}: resolution {:?} vs {}", m.info.resolution, f(3))
                    });
                }
                if f(4) != "." {
                    let ok = m
                        .info
                        .space_group
                        .as_deref()
                        .is_some_and(|s| norm_sg(s) == norm_sg(f(4)));
                    t.check("space group", ok, || {
                        format!("{file}: space group {:?} vs {}", m.info.space_group, f(4))
                    });
                }
            }
            "cell" => {
                let ok = m.cell.is_some_and(|c| {
                    [c.a, c.b, c.c, c.alpha, c.beta, c.gamma]
                        .iter()
                        .enumerate()
                        .all(|(k, v)| close(*v, n(2 + k), 1e-3))
                });
                t.check("cell", ok, || {
                    format!("{file}: cell {:?} vs {:?}", m.cell, &r[2..])
                });
            }
            "atom" => {
                let key = (
                    f(2).into(),
                    f(3).into(),
                    f(4).into(),
                    f(5).into(),
                    f(6).into(),
                );
                let ok = by_key.get(&key).is_some_and(|v| {
                    v.iter().any(|&i| {
                        let a = &m.atoms[i];
                        element::symbol(a.element).eq_ignore_ascii_case(f(7))
                            && close(f64::from(a.pos[0]), n(8), 2e-3)
                            && close(f64::from(a.pos[1]), n(9), 2e-3)
                            && close(f64::from(a.pos[2]), n(10), 2e-3)
                            && close(f64::from(a.occupancy), n(11), 0.006)
                            && close(f64::from(a.b), n(12), 0.006)
                    })
                });
                t.check("atom", ok, || {
                    let mine = by_key.get(&key).map(|v| {
                        v.iter()
                            .map(|&i| {
                                let a = &m.atoms[i];
                                format!(
                                    "{} {:?} {} {}",
                                    element::symbol(a.element),
                                    a.pos,
                                    a.occupancy,
                                    a.b
                                )
                            })
                            .collect::<Vec<_>>()
                    });
                    format!("{file}: atom {:?} ours {mine:?}", &r[2..])
                });
            }
            "res" => {
                let key = (f(2).into(), f(3).into(), f(4).into(), f(5).into());
                let ours = res_by_key.get(&key).map(|&i| ss_char(m.residues[i].ss));
                if f(6) == "-" {
                    // No helices or sheets in the file: the residue exists, its structure
                    // is ours to compute.
                    t.check("polymer residue", ours.is_some(), || {
                        format!("{file}: residue {:?} missing", &r[2..])
                    });
                } else {
                    t.check("secondary structure", ours == Some(f(6)), || {
                        format!("{file}: residue {:?} ours {ours:?}", &r[2..])
                    });
                    if let (Some(&i), Some(d)) = (res_by_key.get(&key), dssp.as_ref()) {
                        t.agree(ss_char(d.residues[i].ss) == f(6));
                    }
                }
            }
            "site" => {
                let e = element::by_symbol(f(2).as_bytes()).unwrap_or(0);
                sites.push((e, [n(3), n(4), n(5)]));
            }
            "mol" => {
                let i = n(2) as usize;
                let ok = m.atoms.get(i).is_some_and(|a| {
                    element::symbol(a.element).eq_ignore_ascii_case(f(3))
                        && close(f64::from(a.pos[0]), n(4), 2e-3)
                        && close(f64::from(a.pos[1]), n(5), 2e-3)
                        && close(f64::from(a.pos[2]), n(6), 2e-3)
                        && i64::from(a.charge) == n(7) as i64
                });
                t.check("molecule atom", ok, || {
                    format!("{file}: {:?} ours {:?}", &r[2..], m.atoms.get(i))
                });
            }
            "bond" => bonds.push((n(2) as u32, n(3) as u32, n(4) as u8)),
            "records" => {
                t.check("records", m.records == n(2) as usize, || {
                    format!("{file}: {} records vs {}", m.records, f(2))
                });
            }
            _ => {}
        }
    }
    if !bonds.is_empty() {
        let mut ours: Vec<(u32, u32, u8)> = m.bonds.iter().map(|b| (b.a, b.b, b.order)).collect();
        ours.sort_unstable();
        bonds.sort_unstable();
        for b in &bonds {
            t.check("bond", ours.binary_search(b).is_ok(), || {
                format!("{file}: bond {b:?} missing")
            });
        }
        for b in &ours {
            t.check("bond", bonds.binary_search(b).is_ok(), || {
                format!("{file}: extra bond {b:?}")
            });
        }
    }
    if !sites.is_empty() {
        compare_sites(file, &m, &sites, t);
    }
}

/// The filled cell: every gemmi site has one of our atoms of its element at the same
/// place (modulo lattice translations), and we have no atoms beyond them.
fn compare_sites(file: &str, m: &Molecule, sites: &[(u8, [f64; 3])], t: &mut Tally) {
    let Some(cell) = m.cell else {
        t.check("site", false, || format!("{file}: no cell"));
        return;
    };
    let ours: Vec<(u8, [f64; 3])> = m
        .atoms
        .iter()
        .map(|a| {
            let f = cell.to_fractional(a.pos.map(f64::from));
            (a.element, f.map(|v| v - v.floor()))
        })
        .collect();
    let dist = |a: [f64; 3], b: [f64; 3]| {
        let d = [0, 1, 2].map(|k| {
            let x = a[k] - b[k];
            x - x.round()
        });
        let c = cell.to_cartesian(d);
        (c[0] * c[0] + c[1] * c[1] + c[2] * c[2]).sqrt()
    };
    let mut used = vec![false; ours.len()];
    for (e, p) in sites {
        let hit = ours
            .iter()
            .enumerate()
            .find(|(i, (oe, op))| !used[*i] && oe == e && dist(*op, *p) < 0.01);
        if let Some((i, _)) = hit {
            used[i] = true;
        }
        t.check("site", hit.is_some(), || {
            format!("{file}: site {} {p:?} not ours", element::symbol(*e))
        });
    }
    for (i, (e, p)) in ours.iter().enumerate() {
        if !used[i] {
            t.check("site", false, || {
                format!("{file}: extra atom {} {p:?}", element::symbol(*e))
            });
        }
    }
}

fn run(dir: &Path, expected: &Path) -> Tally {
    let text =
        std::fs::read_to_string(expected).unwrap_or_else(|e| panic!("{}: {e}", expected.display()));
    let mut files: BTreeMap<&str, Vec<Vec<&str>>> = BTreeMap::new();
    for line in text.lines().filter(|l| !l.is_empty()) {
        let r: Vec<&str> = line.split('\t').collect();
        files.entry(r[0]).or_default().push(r);
    }
    let mut t = Tally::default();
    for (file, rows) in &files {
        compare_file(dir, file, rows, &mut t);
    }
    eprintln!("{} files", files.len());
    for (k, n) in &t.checked {
        eprintln!(
            "  {k:<20} {n:>8} checked, {:>5} mismatches",
            t.bad.get(k).unwrap_or(&0)
        );
    }
    if t.dssp.1 > 0 {
        eprintln!(
            "  computed secondary structure agrees with the file on {} of {} residues ({:.1} %)",
            t.dssp.0,
            t.dssp.1,
            100.0 * t.dssp.0 as f64 / t.dssp.1 as f64
        );
    }
    for n in &t.notes {
        eprintln!("  {n}");
    }
    t
}

#[test]
fn fixtures_match_gemmi_and_rdkit() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data");
    let t = run(&dir, &dir.join("expected.tsv"));
    assert_eq!(t.mismatches(), 0);
}

#[test]
fn corpus_matches_gemmi_and_rdkit() {
    let (Ok(dir), Ok(exp)) = (
        std::env::var("SPECULAR_MOL_DIR"),
        std::env::var("SPECULAR_MOL_ORACLE"),
    ) else {
        eprintln!("SPECULAR_MOL_DIR / SPECULAR_MOL_ORACLE unset: skipped");
        return;
    };
    let t = run(Path::new(&dir), Path::new(&exp));
    assert_eq!(t.mismatches(), 0);
}
