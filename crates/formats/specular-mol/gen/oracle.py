#!/usr/bin/env python3
"""The independent reading of molecular structure files: gemmi for PDB, mmCIF and
small-molecule CIF (atoms, cells, space groups, symmetry expansion into the unit cell,
secondary structure from the file), RDKit for SDF/MOL and XYZ.

Usage: oracle.py DIR OUT.tsv
Writes one row per fact, tab-separated, the file name (relative to DIR) first:

  FILE  summary  atoms  residues  models          (first model; first altloc per residue)
  FILE  info     id  resolution  space_group
  FILE  cell     a  b  c  alpha  beta  gamma
  FILE  atom     chain  seq  icode  resname  name  element  x  y  z  occupancy  b
  FILE  res      chain  seq  icode  resname  ss    (H helix, E strand, C neither, from the
                                                 file; - when the file has none)
  FILE  site     element  fx  fy  fz              (small-molecule CIF: the filled unit cell)
  FILE  mol      index  element  x  y  z  charge   (SDF, XYZ)
  FILE  bond     a  b  order                      (SDF; 0-based atom indices)
  FILE  records  n
  FILE  notmol                                    (an .xyz that is a point cloud)

Large structures (over 200 000 atoms) list every 97th atom row only.
"""
import gzip
import os
import sys

import gemmi
from rdkit import Chem, RDLogger

RDLogger.DisableLog("rdApp.*")

STRIDE_ABOVE = 200_000
STRIDE = 97


def text_of(path):
    raw = open(path, "rb").read()
    if raw[:2] == b"\x1f\x8b":
        raw = gzip.decompress(raw)
    return raw.decode("latin-1")


def kind_of(name, text):
    n = name.lower()
    for ext in (".gz",):
        if n.endswith(ext):
            n = n[: -len(ext)]
    if n.endswith((".pdb", ".ent")):
        return "pdb"
    if n.endswith((".sdf", ".mol", ".sd")):
        return "sdf"
    if n.endswith(".xyz"):
        return "xyz"
    if n.endswith((".cif", ".mmcif")):
        return "mmcif" if ("_atom_site.Cartn_x" in text or "_atom_site.cartn_x" in text) else "cif"
    return None


def fmt(v):
    return f"{v:.4f}"


def macro(out, name, path):
    st = gemmi.read_structure(path, merge_chain_parts=False)
    st.setup_entities()
    model = st[0]
    rows = []
    residues = []
    first_alt = {}
    for ci, ch in enumerate(model):
        for res in ch:
            key = (ch.name, res.seqid.num, res.seqid.icode)
            icode = res.seqid.icode.strip() or "."
            residues.append((ch.name, res.seqid.num, icode, res.name, res))
            for at in res:
                alt = at.altloc
                if alt != "\0":
                    first = first_alt.setdefault(key, alt)
                    if alt != first:
                        continue
                rows.append(
                    (ch.name, res.seqid.num, icode, res.name, at.name, at.element.name,
                     at.pos.x, at.pos.y, at.pos.z, at.occ, at.b_iso)
                )
    # Residues whose every atom was a skipped altloc do not exist for us either.
    kept_res = {(r[0], r[1], r[2], r[3]) for r in rows}
    residues = [r for r in residues if (r[0], r[1], r[2], r[3]) in kept_res]
    out.append([name, "summary", len(rows), len(residues), len(st)])
    res_id = st.name if st.name else ""
    if path.lower().endswith((".pdb", ".ent", ".pdb.gz", ".ent.gz")):
        res_id = st.info["_entry.id"] if "_entry.id" in st.info else res_id
    resolution = f"{st.resolution:.3f}" if st.resolution and st.resolution > 0 else "."
    out.append([name, "info", res_id or ".", resolution, st.spacegroup_hm or "."])
    c = st.cell
    if c.is_crystal():
        out.append([name, "cell", fmt(c.a), fmt(c.b), fmt(c.c), fmt(c.alpha), fmt(c.beta), fmt(c.gamma)])
    stride = STRIDE if len(rows) > STRIDE_ABOVE else 1
    for i, r in enumerate(rows):
        if i % stride:
            continue
        out.append([name, "atom", r[0], r[1], r[2], r[3], r[4], r[5],
                    fmt(r[6]), fmt(r[7]), fmt(r[8]), f"{r[9]:.2f}", f"{r[10]:.2f}"])
    # Secondary structure from the file's HELIX/SHEET or struct_conf/struct_sheet_range.
    ss = {}

    def mark(start, end, code):
        for ch in model:
            if ch.name != start.chain_name:
                continue
            on = False
            for res in ch:
                if res.seqid == start.res_id.seqid:
                    on = True
                if on:
                    ss[(ch.name, res.seqid.num, res.seqid.icode.strip() or ".")] = code
                if res.seqid == end.res_id.seqid and on:
                    return

    for h in st.helices:
        mark(h.start, h.end, "H")
    for sh in st.sheets:
        for s in sh.strands:
            mark(s.start, s.end, "E")
    from_file = bool(len(st.helices) or len(st.sheets))
    for chname, num, icode, rname, res in residues:
        polymer = res.get_ca() is not None or res.get_p() is not None or res.find_atom("C4'", "*") is not None
        if not polymer:
            continue
        code = ss.get((chname, num, icode), "C") if from_file else "-"
        out.append([name, "res", chname, num, icode, rname, code])


def small(out, name, path):
    s = gemmi.read_small_structure(path)
    sites = s.get_all_unit_cell_sites()
    out.append([name, "summary", len(sites), 1, 1])
    out.append([name, "info", ".", ".", s.spacegroup_hm or "."])
    c = s.cell
    out.append([name, "cell", fmt(c.a), fmt(c.b), fmt(c.c), fmt(c.alpha), fmt(c.beta), fmt(c.gamma)])
    for st in sites:
        f = st.fract.wrap_to_unit()
        out.append([name, "site", st.element.name, f"{f.x:.5f}", f"{f.y:.5f}", f"{f.z:.5f}"])


def sdf(out, name, path):
    text = text_of(path)
    supp = Chem.SDMolSupplier()
    supp.SetData(text, sanitize=False, removeHs=False, strictParsing=False)
    n = len(supp)
    mol = supp[0]
    out.append([name, "records", n])
    conf = mol.GetConformer()
    for a in mol.GetAtoms():
        p = conf.GetAtomPosition(a.GetIdx())
        out.append([name, "mol", a.GetIdx(), a.GetSymbol(), fmt(p.x), fmt(p.y), fmt(p.z), a.GetFormalCharge()])
    order = {
        Chem.BondType.SINGLE: 1,
        Chem.BondType.DOUBLE: 2,
        Chem.BondType.TRIPLE: 3,
        Chem.BondType.AROMATIC: 4,
    }
    for b in mol.GetBonds():
        a, c = sorted((b.GetBeginAtomIdx(), b.GetEndAtomIdx()))
        out.append([name, "bond", a, c, order.get(b.GetBondType(), 0)])


def xyz(out, name, path):
    text = text_of(path)
    first = text.split("\n", 1)[0].split()
    if len(first) != 1 or not first[0].isdigit():
        out.append([name, "notmol"])
        return
    mol = Chem.MolFromXYZBlock(text)
    if mol is None:
        # RDKit reads one frame; cut the text to the first.
        n = int(first[0])
        mol = Chem.MolFromXYZBlock("\n".join(text.split("\n")[: n + 2]) + "\n")
    conf = mol.GetConformer()
    for a in mol.GetAtoms():
        p = conf.GetAtomPosition(a.GetIdx())
        out.append([name, "mol", a.GetIdx(), a.GetSymbol(), fmt(p.x), fmt(p.y), fmt(p.z), 0])


def main():
    root, dest = sys.argv[1], sys.argv[2]
    out = []
    names = []
    for dirpath, _, files in os.walk(root):
        for f in files:
            if f.startswith(".") or f.endswith((".tsv", ".part")):
                continue
            names.append(os.path.relpath(os.path.join(dirpath, f), root))
    for name in sorted(names):
        path = os.path.join(root, name)
        try:
            text = text_of(path)
        except OSError:
            continue
        k = kind_of(name, text)
        if k in ("pdb", "mmcif"):
            macro(out, name, path)
        elif k == "cif":
            small(out, name, path)
        elif k == "sdf":
            sdf(out, name, path)
        elif k == "xyz":
            xyz(out, name, path)
    with open(dest, "w") as fh:
        for r in out:
            fh.write("\t".join(str(v) for v in r) + "\n")
    print(f"oracle: {len(out)} rows for {len(set(r[0] for r in out))} files -> {dest}")


if __name__ == "__main__":
    main()
