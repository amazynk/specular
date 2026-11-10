use super::*;

fn data(name: &str) -> Vec<u8> {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(name);
    std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn lim() -> Limits {
    Limits::default()
}

const PEPTIDE: &str = "\
HEADER    TEST PEPTIDE                            12-OCT-26   9XYZ
TITLE     A TWO-CHAIN TEST
EXPDTA    X-RAY DIFFRACTION
REMARK   2 RESOLUTION.    1.80 ANGSTROMS.
CRYST1   40.000   50.000   60.000  90.00  90.00  90.00 P 21 21 21     4
ATOM      1  N   GLY A   1       0.000   0.000   0.000  1.00 10.00           N
ATOM      2  CA  GLY A   1       1.458   0.000   0.000  1.00 11.00           C
ATOM      3  C   GLY A   1       2.009   1.420   0.000  1.00 12.00           C
ATOM      4  O   GLY A   1       1.251   2.390   0.000  1.00 13.00           O
ATOM      5  N   ALA A   2       3.332   1.536   0.000  1.00 10.00           N
ATOM      6  CA AALA A   2       3.970   2.845   0.000  0.60 11.00           C
ATOM      7  CA BALA A   2       3.990   2.865   0.100  0.40 11.00           C
ATOM      8  C   ALA A   2       5.486   2.715   0.000  1.00 12.00           C
ATOM      9  O   ALA A   2       6.008   1.601   0.000  1.00 13.00           O
ATOM     10  CB AALA A   2       3.534   3.649   1.220  0.60 14.00           C
TER
ATOM     11  N   GLY B   1      10.000   0.000   0.000  1.00 10.00           N
ATOM     12  CA  GLY B   1      11.458   0.000   0.000  1.00 11.00           C
HETATM   13 ZN    ZN B 101      20.000  20.000  20.000  1.00 30.00          ZN2+
HETATM   14  O   HOH A 201      30.000  30.000  30.000  1.00 40.00           O
END
";

#[test]
fn pdb_records() {
    let m = open(PEPTIDE.as_bytes(), None, &lim()).unwrap();
    assert_eq!(m.format, Some(MolFormat::Pdb));
    // The B altloc of ALA CA is dropped.
    assert_eq!(m.atoms.len(), 13);
    assert_eq!(m.info.id.as_deref(), Some("9XYZ"));
    assert_eq!(m.info.title.as_deref(), Some("A TWO-CHAIN TEST"));
    assert_eq!(m.info.method.as_deref(), Some("X-RAY DIFFRACTION"));
    assert_eq!(m.info.resolution, Some(1.8));
    assert_eq!(m.info.space_group.as_deref(), Some("P 21 21 21"));
    let c = m.cell.unwrap();
    assert_eq!((c.a, c.b, c.c), (40.0, 50.0, 60.0));
    let ids: Vec<&str> = m.chains.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, ["A", "B", "A"]);
    assert_eq!(m.residues.len(), 5);
    assert_eq!(m.ligands(), ["ZN"]);
    let zn = m.atoms.iter().find(|a| a.element == 30).unwrap();
    assert_eq!(zn.charge, 2);
    assert!(zn.het);
    // Peptide bond GLY1 C – ALA2 N, and the backbone within each residue.
    let has = |a: i32, b: i32| {
        let (i, j) = (
            m.atoms.iter().position(|x| x.serial == a).unwrap() as u32,
            m.atoms.iter().position(|x| x.serial == b).unwrap() as u32,
        );
        m.bonds.iter().any(|x| (x.a, x.b) == (i.min(j), i.max(j)))
    };
    assert!(has(3, 5), "peptide bond");
    assert!(has(1, 2) && has(2, 3) && has(3, 4));
    assert!(has(6, 10), "CA–CB");
    assert!(!has(13, 14));
    assert_eq!(m.residues[0].kind, ResKind::Amino);
    assert_eq!(m.residues[4].kind, ResKind::Water);
}

#[test]
fn pdb_models_and_conect() {
    let s = "\
MODEL        1
HETATM    1  C1  LIG A   1       0.000   0.000   0.000  1.00  0.00           C
HETATM    2  O1  LIG A   1       3.000   0.000   0.000  1.00  0.00           O
ENDMDL
MODEL        2
HETATM    1  C1  LIG A   1       0.100   0.000   0.000  1.00  0.00           C
HETATM    2  O1  LIG A   1       3.100   0.000   0.000  1.00  0.00           O
ENDMDL
CONECT    1    2
END
";
    let m = open(s.as_bytes(), None, &lim()).unwrap();
    assert_eq!(m.models, 2);
    assert_eq!(m.atoms.len(), 2);
    // 3 Å is no covalent bond, but CONECT says so.
    assert_eq!(m.bonds.len(), 1);
    assert_eq!(m.file_bonds, 1);
}

#[test]
fn pdb_gzip() {
    // A stored-block gzip member around the peptide.
    let gz = stored_gzip(PEPTIDE.as_bytes());
    let m = open(&gz, None, &lim()).unwrap();
    assert!(m.gzipped);
    assert_eq!(m.atoms.len(), 13);
}

fn stored_gzip(raw: &[u8]) -> Vec<u8> {
    let mut gz = vec![0x1F, 0x8B, 8, 0, 0, 0, 0, 0, 0, 3];
    for (i, chunk) in raw.chunks(65535).enumerate() {
        let last = (i + 1) * 65535 >= raw.len();
        gz.push(u8::from(last));
        let n = chunk.len() as u16;
        gz.extend_from_slice(&n.to_le_bytes());
        gz.extend_from_slice(&(!n).to_le_bytes());
        gz.extend_from_slice(chunk);
    }
    gz.extend_from_slice(&crc32(raw).to_le_bytes());
    gz.extend_from_slice(&(raw.len() as u32).to_le_bytes());
    gz
}

#[test]
fn gzip_unpacks_within_the_memory_budget() {
    // 24 MiB of PDB text in a gzip member: refused by a 8 MiB budget as a size, not a
    // malformed file; read with enough budget, none of it held afterwards.
    let mut raw = PEPTIDE.as_bytes().to_vec();
    raw.resize(24 << 20, b'\n');
    let gz = stored_gzip(&raw);
    let r = specular_core::budget::with_limit(8 << 20, || open(&gz, None, &lim()));
    assert!(
        matches!(&r, Err(Error::TooLarge(m)) if m.contains("free RAM")),
        "{:?}",
        r.err()
    );
    let b = std::sync::Arc::new(specular_core::budget::MemoryBudget::fixed(256 << 20));
    let m = specular_core::budget::with_budget(b.clone(), || open(&gz, None, &lim())).unwrap();
    assert_eq!(m.atoms.len(), 13);
    assert_eq!(b.reserved(), 0);
}

fn crc32(d: &[u8]) -> u32 {
    let mut c = !0u32;
    for &b in d {
        c ^= u32::from(b);
        for _ in 0..8 {
            c = if c & 1 != 0 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
        }
    }
    !c
}

#[test]
fn refusals() {
    let tight = Limits {
        max_atoms: 5,
        ..Limits::default()
    };
    assert!(matches!(
        open(PEPTIDE.as_bytes(), None, &tight),
        Err(Error::TooLarge(_))
    ));
    let sdf = "x\n\n\n999999999  0  0  0  0  0  0  0  0  0999 V2000\n";
    assert!(open(sdf.as_bytes(), None, &lim()).is_err());
    let xyz = "99999999999\n\nC 0 0 0\n";
    assert!(open(xyz.as_bytes(), Some(MolFormat::Xyz), &tight).is_err());
    assert!(matches!(
        open(b"HEADER    NOTHING\nEND\n", Some(MolFormat::Pdb), &lim()),
        Err(Error::Empty)
    ));
    assert!(open(b"hello world", None, &lim()).is_err());
    // Truncated gzip.
    assert!(open(&[0x1F, 0x8B, 8, 0, 0, 0], None, &lim()).is_err());
}

const MMCIF: &str = "\
data_1ABC
#
_entry.id 1ABC
_struct.title 'Two residues; a helix'
_exptl.method 'X-RAY DIFFRACTION'
_refine.ls_d_res_high 2.10
_cell.length_a 10
_cell.length_b 20
_cell.length_c 30
_cell.angle_alpha 90
_cell.angle_beta 90
_cell.angle_gamma 90
_symmetry.space_group_name_H-M 'P 1'
loop_
_struct_conf.conf_type_id
_struct_conf.id
_struct_conf.beg_auth_asym_id
_struct_conf.beg_auth_seq_id
_struct_conf.pdbx_beg_PDB_ins_code
_struct_conf.end_auth_asym_id
_struct_conf.end_auth_seq_id
_struct_conf.pdbx_end_PDB_ins_code
HELX_P HELX_P1 A 1 ? A 2 ?
loop_
_atom_site.group_PDB
_atom_site.id
_atom_site.type_symbol
_atom_site.label_atom_id
_atom_site.label_alt_id
_atom_site.label_comp_id
_atom_site.label_asym_id
_atom_site.label_seq_id
_atom_site.pdbx_PDB_ins_code
_atom_site.Cartn_x
_atom_site.Cartn_y
_atom_site.Cartn_z
_atom_site.occupancy
_atom_site.B_iso_or_equiv
_atom_site.pdbx_formal_charge
_atom_site.auth_seq_id
_atom_site.auth_asym_id
_atom_site.pdbx_PDB_model_num
ATOM 1 N N . GLY A 1 ? 0.000 0.000 0.000 1.00 10.0 ? 1 A 1
ATOM 2 C CA . GLY A 1 ? 1.458 0.000 0.000 1.00 11.0 ? 1 A 1
ATOM 3 C C . GLY A 1 ? 2.009 1.420 0.000 1.00 12.0 ? 1 A 1
ATOM 4 O O . GLY A 1 ? 1.251 2.390 0.000 1.00 13.0 ? 1 A 1
ATOM 5 N N . ALA A 2 ? 3.332 1.536 0.000 1.00 10.0 ? 2 A 1
ATOM 6 C CA A ALA A 2 ? 3.970 2.845 0.000 0.6 11.0 ? 2 A 1
ATOM 7 C CA B ALA A 2 ? 3.990 2.865 0.100 0.4 11.0 ? 2 A 1
ATOM 8 C C . ALA A 2 ? 5.486 2.715 0.000 1.00 12.0 ? 2 A 1
ATOM 9 O O . ALA A 2 ? 6.008 1.601 0.000 1.00 13.0 ? 2 A 1
HETATM 10 O O . HOH B . ? 9.0 9.0 9.0 1.00 30.0 ? 101 A 1
ATOM 11 N N . GLY A 1 ? 0.100 0.000 0.000 1.00 10.0 ? 1 A 2
#
";

#[test]
fn mmcif_atoms_and_metadata() {
    let m = open(MMCIF.as_bytes(), None, &lim()).unwrap();
    assert_eq!(m.format, Some(MolFormat::Mmcif));
    assert_eq!(m.atoms.len(), 9);
    assert_eq!(m.models, 2);
    assert_eq!(m.info.id.as_deref(), Some("1ABC"));
    assert_eq!(m.info.title.as_deref(), Some("Two residues; a helix"));
    assert_eq!(m.info.resolution, Some(2.1));
    assert_eq!(m.info.space_group.as_deref(), Some("P 1"));
    assert_eq!(m.cell.unwrap().c, 30.0);
    assert_eq!(m.ss_source, SsSource::File);
    assert_eq!(m.residues[0].ss, Ss::Helix);
    assert_eq!(m.residues[1].ss, Ss::Helix);
    // The water's label chain is B, its author chain A: chains go by author id (as gemmi
    // and the PDB format group them), the water a residue of its own.
    assert_eq!(m.chains.len(), 1);
    assert_eq!(m.residues.len(), 3);
    assert_eq!(m.residues[2].kind, ResKind::Water);
}

const NACL: &str = "\
data_nacl
_chemical_name_systematic 'Sodium chloride'
_chemical_formula_sum 'Cl Na'
_cell_length_a 5.64
_cell_length_b 5.64
_cell_length_c 5.64
_cell_angle_alpha 90
_cell_angle_beta 90
_cell_angle_gamma 90
_symmetry_space_group_name_H-M 'F m -3 m'
loop_
_symmetry_equiv_pos_as_xyz
x,y,z
x,y+1/2,z+1/2
x+1/2,y,z+1/2
x+1/2,y+1/2,z
loop_
_atom_site_label
_atom_site_type_symbol
_atom_site_fract_x
_atom_site_fract_y
_atom_site_fract_z
Na1 Na 0 0 0
Cl1 Cl 0.5 0.5 0.5
";

#[test]
fn small_molecule_cif_fills_the_cell() {
    let m = open(NACL.as_bytes(), None, &lim()).unwrap();
    assert_eq!(m.format, Some(MolFormat::Cif));
    assert_eq!(m.asymmetric_atoms, 2);
    // Four Na and four Cl in the conventional cell (the translations of F).
    assert_eq!(m.atoms.iter().filter(|a| a.element == 11).count(), 4);
    assert_eq!(m.atoms.iter().filter(|a| a.element == 17).count(), 4);
    assert_eq!(m.info.title.as_deref(), Some("Sodium chloride"));
    assert_eq!(m.info.formula.as_deref(), Some("Cl Na"));
    assert!(m.cell_edges().is_some());
    let b = m.bounds();
    assert!(b.max[0] >= 5.6);
}

const BENZENE_V2000: &str = "\
benzene
  test

  6  6  0  0  0  0  0  0  0  0999 V2000
    1.3970    0.0000    0.0000 C   0  0  0  0  0  0  0  0  0  0  0  0
    0.6985    1.2098    0.0000 C   0  0  0  0  0  0  0  0  0  0  0  0
   -0.6985    1.2098    0.0000 C   0  0  0  0  0  0  0  0  0  0  0  0
   -1.3970    0.0000    0.0000 C   0  0  0  0  0  0  0  0  0  0  0  0
   -0.6985   -1.2098    0.0000 C   0  0  0  0  0  0  0  0  0  0  0  0
    0.6985   -1.2098    0.0000 N   0  3  0  0  0  0  0  0  0  0  0  0
  1  2  2  0
  2  3  1  0
  3  4  2  0
  4  5  1  0
  5  6  2  0
  6  1  1  0
M  CHG  1   6   1
M  END
> <PUBCHEM_COMPOUND_CID>
241

> <PUBCHEM_MOLECULAR_FORMULA>
C6H6

$$$$
second
  test

  1  0  0  0  0  0  0  0  0  0999 V2000
    0.0000    0.0000    0.0000 O   0  0  0  0  0  0  0  0  0  0  0  0
M  END
$$$$
";

#[test]
fn sdf_v2000() {
    let m = open(BENZENE_V2000.as_bytes(), None, &lim()).unwrap();
    assert_eq!(m.format, Some(MolFormat::Sdf));
    assert_eq!(m.atoms.len(), 6);
    assert_eq!(m.bonds.len(), 6);
    assert_eq!(m.bonds.iter().filter(|b| b.order == 2).count(), 3);
    assert_eq!(m.records, 2);
    assert_eq!(m.atoms[5].charge, 1);
    assert_eq!(m.info.id.as_deref(), Some("241"));
    assert_eq!(m.info.formula.as_deref(), Some("C6H6"));
}

#[test]
fn sdf_v3000() {
    let s = "\
ethanol
  test

  0  0  0     0  0            999 V3000
M  V30 BEGIN CTAB
M  V30 COUNTS 3 2 0 0 0
M  V30 BEGIN ATOM
M  V30 1 C 0 0 0 0
M  V30 2 C 1.5 0 0 0
M  V30 3 O 2.2 1.2 -
M  V30 0 0 CHG=-1
M  V30 END ATOM
M  V30 BEGIN BOND
M  V30 1 1 1 2
M  V30 2 1 2 3
M  V30 END BOND
M  V30 END CTAB
M  END
$$$$
";
    let m = open(s.as_bytes(), None, &lim()).unwrap();
    assert_eq!(m.atoms.len(), 3);
    assert_eq!(m.atoms[2].element, 8);
    assert_eq!(m.atoms[2].charge, -1);
    assert_eq!(m.bonds.len(), 2);
    assert_eq!(m.records, 1);
}

const WATER_XYZ: &str = "3\nwater molecule\nO 0.0 0.0 0.117\nH 0.0 0.757 -0.467\n1 0.0 -0.757 -0.467\n3\nframe 2\nO 0 0 0\nH 0 0.7 -0.4\nH 0 -0.7 -0.4\n";

#[test]
fn xyz_molecule_and_point_cloud() {
    assert!(is_molecular_xyz(WATER_XYZ.as_bytes()));
    assert!(is_molecular_xyz(b"2\n\nC1 0 0 0\nCl2 1.7 0 0\n"));
    // Point clouds.
    assert!(!is_molecular_xyz(b"0.0 0.0 0.0\n1.0 2.0 3.0\n"));
    assert!(!is_molecular_xyz(b"0.0 0.0 0.0 255 0 0\n1 2 3 0 255 0\n"));
    assert!(!is_molecular_xyz(b"3\n\n1.0 2.0 3.0\n4 5 6\n"));
    let m = open(WATER_XYZ.as_bytes(), None, &lim()).unwrap();
    assert_eq!(m.format, Some(MolFormat::Xyz));
    assert_eq!(m.atoms.len(), 3);
    assert_eq!(m.atoms[2].element, 1);
    assert_eq!(m.models, 2);
    assert_eq!(m.bonds.len(), 2, "O–H twice, never H–H");
    assert_eq!(m.info.title.as_deref(), Some("water molecule"));
}

#[test]
fn sniffing() {
    assert_eq!(sniff(PEPTIDE.as_bytes()), Some(MolFormat::Pdb));
    assert_eq!(sniff(MMCIF.as_bytes()), Some(MolFormat::Mmcif));
    assert_eq!(sniff(NACL.as_bytes()), Some(MolFormat::Cif));
    assert_eq!(sniff(BENZENE_V2000.as_bytes()), Some(MolFormat::Sdf));
    assert_eq!(sniff(WATER_XYZ.as_bytes()), Some(MolFormat::Xyz));
    assert_eq!(sniff(b"ply\nformat ascii 1.0\n"), None);
    assert_eq!(sniff(b"REMARK just a note\nnot pdb at all\n"), None);
}

#[test]
fn crambin() {
    let m = open(&data("1crn.pdb"), None, &lim()).unwrap();
    assert_eq!(m.atoms.len(), 327);
    assert_eq!(m.residues.len(), 46);
    assert_eq!(m.info.id.as_deref(), Some("1CRN"));
    assert_eq!(m.ss_source, SsSource::File);
    let helix = m.residues.iter().filter(|r| r.ss == Ss::Helix).count();
    let strand = m.residues.iter().filter(|r| r.ss == Ss::Strand).count();
    assert!(helix > 15 && strand > 4, "{helix} {strand}");
    // Our own assignment agrees with the file on most residues.
    let mut c = m.clone();
    for r in &mut c.residues {
        r.ss = Ss::Coil;
    }
    assert!(ss::assign(&mut c));
    let agree = m
        .residues
        .iter()
        .zip(&c.residues)
        .filter(|(a, b)| a.ss == b.ss)
        .count();
    assert!(agree >= 38, "{agree}/46");
    // Three disulfides.
    let ss_bonds = m
        .bonds
        .iter()
        .filter(|b| {
            let (x, y) = (&m.atoms[b.a as usize], &m.atoms[b.b as usize]);
            x.element == 16 && y.element == 16
        })
        .count();
    assert_eq!(ss_bonds, 3);
    // Cartoon.
    assert_eq!(m.default_style(), Style::Cartoon);
    let mesh = cartoon::build(&m, &colour::residue_colours(&m, Colouring::Chain)).unwrap();
    assert!(mesh.triangle_count() > 5_000);
    assert!(
        mesh.indices
            .iter()
            .all(|&i| (i as usize) < mesh.positions.len())
    );
    let img = preview::render(&m, 64, 64);
    assert_eq!(img.len(), 64 * 64 * 4);
    assert!(img.chunks(4).filter(|p| p[3] > 0).count() > 400);
}

#[test]
fn colourings_cover_every_atom() {
    let m = open(&data("1crn.pdb"), None, &lim()).unwrap();
    for c in Colouring::ALL {
        assert_eq!(colour::atom_colours(&m, c).len(), m.atoms.len());
        assert_eq!(colour::residue_colours(&m, c).len(), m.residues.len());
    }
    for s in Style::ALL {
        let mesh = preview::scene(&m, s, Colouring::Element);
        assert!(!mesh.indices.is_empty());
    }
}

/// A synthetic million-atom mmCIF parses in well under a second (optimised dev profile; the
/// bound is loose for a loaded machine).
#[test]
fn million_atoms_mmcif_is_fast() {
    let mut s = String::with_capacity(90 << 20);
    s.push_str("data_big\n_entry.id BIG\nloop_\n_atom_site.group_PDB\n_atom_site.id\n_atom_site.type_symbol\n_atom_site.label_atom_id\n_atom_site.label_alt_id\n_atom_site.label_comp_id\n_atom_site.label_asym_id\n_atom_site.label_seq_id\n_atom_site.Cartn_x\n_atom_site.Cartn_y\n_atom_site.Cartn_z\n_atom_site.occupancy\n_atom_site.B_iso_or_equiv\n_atom_site.auth_seq_id\n_atom_site.auth_asym_id\n_atom_site.pdbx_PDB_model_num\n");
    let names = ["N", "CA", "C", "O", "CB"];
    let els = ["N", "C", "C", "O", "C"];
    let off = [
        [0.0, 0.0, 0.0],
        [1.46, 0.0, 0.0],
        [2.0, 1.42, 0.0],
        [1.25, 2.39, 0.0],
        [1.9, -0.8, 1.2],
    ];
    let n_res = 200_000;
    use std::fmt::Write;
    for r in 0..n_res {
        let chain = r / 1000;
        let base = [
            (r % 1000) as f32 * 3.8,
            (chain % 40) as f32 * 9.0,
            (chain / 40) as f32 * 9.0,
        ];
        for k in 0..5 {
            let id = r * 5 + k + 1;
            let _ = writeln!(
                s,
                "ATOM {id} {} {} . ALA C{chain} {} {:.3} {:.3} {:.3} 1.00 20.00 {} C{chain} 1",
                els[k],
                names[k],
                r % 1000 + 1,
                base[0] + off[k][0],
                base[1] + off[k][1],
                base[2] + off[k][2],
                r % 1000 + 1
            );
        }
    }
    let t = std::time::Instant::now();
    let m = open(s.as_bytes(), None, &lim()).unwrap();
    let dt = t.elapsed();
    assert_eq!(m.atoms.len(), 1_000_000);
    assert_eq!(m.residues.len(), n_res);
    eprintln!("1M-atom mmCIF: {dt:?} ({} MB)", s.len() >> 20);
    assert!(dt.as_secs_f64() < 3.0, "{dt:?}");
}

#[test]
fn sdf_v3000_count_reserves_no_more_than_lines() {
    // A count of 20 million atoms with two atom lines: 20 million atoms were reserved before.
    let s = "\
big
  test

  0  0  0     0  0            999 V3000
M  V30 BEGIN CTAB
M  V30 COUNTS 20000000 0 0 0 0
M  V30 BEGIN ATOM
M  V30 1 C 0 0 0 0
M  V30 2 O 1.2 0 0 0
M  V30 END ATOM
M  V30 END CTAB
M  END
$$$$
";
    let m = open(s.as_bytes(), None, &Limits::default()).unwrap();
    assert_eq!(m.atoms.len(), 2);
    assert!(m.atoms.capacity() < 1000, "{}", m.atoms.capacity());
}
