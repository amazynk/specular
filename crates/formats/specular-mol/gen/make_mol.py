#!/usr/bin/env python3
"""Writes the committed test fixtures in tests/data: synthetic files made with gemmi and
RDKit, plus copies of a few tiny public-domain (CC0) entries from the corpus.

Usage: make_mol.py CORPUS_DIR OUT_DIR
  CORPUS_DIR is what tools/corpus/fetch-mol.sh fetched (1crn.pdb, 1bna.pdb and
  cod_1011176.cif are copied from it: RCSB PDB and COD data are CC0).
Deterministic: RDKit embeds with a fixed seed.
"""
import gzip
import os
import shutil
import sys

import gemmi
from rdkit import Chem
from rdkit.Chem import AllChem


def gz(src, dest):
    # mtime 0: the bytes do not depend on when this ran.
    with open(src, "rb") as f, open(dest, "wb") as raw:
        with gzip.GzipFile(fileobj=raw, mode="wb", mtime=0, filename="") as g:
            g.write(f.read())


def embed(smiles, seed=42):
    mol = Chem.AddHs(Chem.MolFromSmiles(smiles))
    AllChem.EmbedMolecule(mol, randomSeed=seed)
    AllChem.MMFFOptimizeMolecule(mol)
    return mol


def main():
    corpus, out = sys.argv[1], sys.argv[2]
    os.makedirs(out, exist_ok=True)
    for name in ("1crn.pdb", "1bna.pdb", "cod_1011176.cif"):
        shutil.copyfile(os.path.join(corpus, name), os.path.join(out, name))

    # Crambin as PDBx/mmCIF (gemmi's writer), gzip-compressed, and as gzip PDB.
    st = gemmi.read_structure(os.path.join(out, "1crn.pdb"))
    st.setup_entities()
    doc = st.make_mmcif_document()
    tmp = os.path.join(out, "1crn.cif")
    doc.write_file(tmp)
    gz(tmp, os.path.join(out, "1crn.cif.gz"))
    os.remove(tmp)
    gz(os.path.join(out, "1crn.pdb"), os.path.join(out, "1crn.pdb.gz"))

    # An NMR-like ensemble: crambin twice, the second model shifted.
    st2 = gemmi.read_structure(os.path.join(out, "1crn.pdb"))
    m2 = gemmi.Model(2)
    for ch in st2[0]:
        c = ch.clone()
        for res in c:
            for at in res:
                at.pos = gemmi.Position(at.pos.x + 0.5, at.pos.y, at.pos.z)
        m2.add_chain(c)
    st2.add_model(m2)
    st2.setup_entities()
    with open(os.path.join(out, "ensemble.pdb"), "w") as f:
        f.write(st2.make_pdb_string(gemmi.PdbWriteOptions(minimal=True)))

    # A small-molecule CIF with an inversion centre: the C1–C1 bond crosses it (geom_bond
    # with site_symmetry_2), so only symmetry expansion completes the molecule.
    with open(os.path.join(out, "dimer.cif"), "w") as f:
        f.write(
            "data_dimer\n"
            "_chemical_name_common 'synthetic dimer'\n"
            "_chemical_formula_sum 'C2 O2'\n"
            "_cell_length_a 6.0\n_cell_length_b 7.0\n_cell_length_c 8.0\n"
            "_cell_angle_alpha 90\n_cell_angle_beta 90\n_cell_angle_gamma 90\n"
            "_space_group_name_H-M_alt 'P -1'\n"
            "loop_\n_space_group_symop_id\n_space_group_symop_operation_xyz\n"
            "1 x,y,z\n2 -x,-y,-z\n"
            "loop_\n_atom_site_label\n_atom_site_type_symbol\n_atom_site_fract_x\n"
            "_atom_site_fract_y\n_atom_site_fract_z\n_atom_site_U_iso_or_equiv\n"
            "C1 C 0.128333 0.0 0.0 0.02\n"
            "O1 O 0.328333 0.0 0.0 0.03\n"
            "loop_\n_geom_bond_atom_site_label_1\n_geom_bond_atom_site_label_2\n"
            "_geom_bond_distance\n_geom_bond_site_symmetry_2\n"
            "C1 O1 1.200 .\n"
            "C1 C1 1.540 2_555\n"
        )

    # SDF: aspirin (two records), caffeine as V3000; caffeine as XYZ; a point cloud .xyz.
    aspirin = embed("CC(=O)OC1=CC=CC=C1C(=O)O")
    aspirin.SetProp("_Name", "aspirin")
    aspirin.SetProp("PUBCHEM_COMPOUND_CID", "2244")
    aspirin.SetProp("PUBCHEM_MOLECULAR_FORMULA", "C9H8O4")
    ammonium = embed("[NH4+]")
    ammonium.SetProp("_Name", "ammonium")
    w = Chem.SDWriter(os.path.join(out, "aspirin.sdf"))
    w.write(aspirin)
    w.write(ammonium)
    w.close()
    caffeine = embed("CN1C=NC2=C1C(=O)N(C(=O)N2C)C")
    caffeine.SetProp("_Name", "caffeine")
    with open(os.path.join(out, "caffeine_v3000.mol"), "w") as f:
        f.write(Chem.MolToV3KMolBlock(caffeine))
    with open(os.path.join(out, "caffeine.xyz"), "w") as f:
        f.write(Chem.MolToXYZBlock(caffeine))
    acetate = embed("CC(=O)[O-]")
    with open(os.path.join(out, "acetate.sdf"), "w") as f:
        f.write(Chem.MolToMolBlock(acetate) + "$$$$\n")
    with open(os.path.join(out, "cloud.xyz"), "w") as f:
        for i in range(50):
            f.write(f"{i * 0.1:.3f} {(i * 7 % 13) * 0.2:.3f} {(i * 3 % 11) * 0.3:.3f}\n")


if __name__ == "__main__":
    main()
