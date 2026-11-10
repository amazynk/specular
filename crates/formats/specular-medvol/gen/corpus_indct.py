#!/usr/bin/env python3
"""Writes the oracle for the fetched industrial CT corpus (tools/corpus/fetch-indct.sh):
tools/corpus/indct/expected.tsv, read by tests/indct_corpus.rs.

* Zeiss Xradia files: txrm2tiff (over olefile) for the images, olefile for the pixel size.
* Amira lattices: the small independent reader in make_indct.py.
* The SkyScan reconstruction: its log and PIL for the PNG slices.
* Nikon and Volume Graphics headers (fetched without their multi-gigabyte volumes): the
  INI keys read here, as `hdr` rows (name, size, spacing in mm, data file, sample bytes).

The rows are those of expected-indct.tsv (file, geom, val), values sampled. Run with -I from
the oracle venv:

    python3 -I crates/formats/specular-medvol/gen/corpus_indct.py [corpus dir]
"""

import os
import re
import sys

import numpy as np
import olefile
from PIL import Image
from txrm2tiff.txrm import open_txrm

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import make_indct as m  # noqa: E402

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "..", "..")
CORPUS = sys.argv[1] if len(sys.argv) > 1 else os.path.join(ROOT, "tools", "corpus", "indct")


def xradia(rel):
    path = os.path.join(CORPUS, rel)
    ole = olefile.OleFileIO(path)
    px = float(np.frombuffer(ole.openstream("ImageInfo/PixelSize").read()[:4], "<f4")[0])
    dt = int(np.frombuffer(ole.openstream("ImageInfo/DataType").read()[:4], "<i4")[0])
    ole.close()
    t = open_txrm(open(path, "rb").read())
    out = np.asarray(t.get_output(load=True, flip=False))
    t.close()
    n_, h, w = out.shape
    name = {5: "uint16", 10: "float32"}[dt]
    m.record(rel, "Zeiss Xradia TXM", name, "object", 1)
    p = px / 1000.0
    m.geom(rel, [[p, 0, 0, 0], [0, -p, 0, (h - 1) * p], [0, 0, p, 0]], (w, h, n_))
    m.vals(rel, np.transpose(out[:, ::-1, :], (2, 1, 0)))


def amira(rel):
    a, sp, org, typ, comp = m.amira_read(os.path.join(CORPUS, rel))
    if a.shape[2] == 1:
        sp[2] = sp[0]
    colour = typ == "byte" and comp in (3, 4)
    vols = 1 if colour or comp == 1 else comp
    names = {"byte": "uint8", "short": "int16", "ushort": "uint16", "int": "int32",
             "float": "float32", "double": "float64"}
    m.record(rel, "Amira", names[typ], "object", vols)
    aff = [[sp[0], 0, 0, org[0]], [0, -sp[1], 0, -org[1]], [0, 0, sp[2], org[2]]]
    m.geom(rel, aff, a.shape[:3])
    m.vals(rel, a[..., np.newaxis, :3] if colour else a)


def ini(path):
    """key → value (lowercase keys, first occurrence), and per-section."""
    flat, sect, cur = {}, {}, ""
    for line in open(path, encoding="latin-1").read().splitlines():
        s = line.strip()
        if s.startswith("[") and s.endswith("]"):
            cur = s[1:-1].lower()
            continue
        if "=" in s:
            k, v = s.split("=", 1)
            k, v = k.strip().lower(), v.strip()
            flat.setdefault(k, v)
            sect.setdefault((cur, k), v)
    return flat, sect


def skyscan(rel):
    log = os.path.join(CORPUS, rel)
    flat, sect = ini(log)
    rec = lambda k: sect.get(("file name convention", k), sect.get(("reconstruction", k), flat[k]))  # noqa: E731,E501
    first, last = int(rec("first section")), int(rec("last section"))
    digits = int(rec("filename index length"))
    prefix = rec("filename prefix")
    ext = rec("result file type").lower()
    lo, hi = float(rec("minimum for cs to image conversion")), float(
        rec("maximum for cs to image conversion"))
    d = os.path.dirname(log)
    stack = []
    for sec in range(first, last + 1):
        im = Image.open(os.path.join(d, f"{prefix}{sec:0{digits}d}.{ext}"))
        if im.mode == "P":
            pal = np.asarray(im.getpalette()[:768]).reshape(-1, 3)
            assert (pal[:, 0] == pal[:, 1]).all() and (pal[:, 0] == pal[:, 2]).all()
            im = im.convert("RGB")
        a = np.asarray(im)
        stack.append(np.asarray(a[..., 0] if a.ndim == 3 else a, np.float64))
    s = np.stack(stack)
    bits8 = s.max() <= 255
    s = s * ((hi - lo) / (255.0 if bits8 else 65535.0)) + lo
    m.record(rel, "Bruker SkyScan", "uint8" if bits8 else "uint16", "object", 1)
    p = float(rec("pixel size (um)")) / 1000.0
    m.geom(rel, [[p, 0, 0, 0], [0, p, 0, 0], [0, 0, p, 0]], (s.shape[2], s.shape[1], s.shape[0]))
    m.vals(rel, np.transpose(s, (2, 1, 0)))


UNIT = {"mm": 1.0, "um": 1e-3, "µm": 1e-3, "m": 1000.0, "nm": 1e-6, "cm": 10.0}


def xtek(rel):
    flat, _ = ini(os.path.join(CORPUS, rel))
    n = [int(flat[f"voxels{a}"]) for a in "xyz"]
    u = UNIT[flat.get("units", "mm").lower()]
    s = [abs(float(flat[f"voxelsize{a}"])) * u for a in "xyz"]
    data = os.path.splitext(os.path.basename(rel))[0] + ".vol"
    m.rows.append("\t".join(["hdr", rel, *map(str, n), *map(m.fmt, s), data, "4", "float32"]))


def vgi(rel):
    text = open(os.path.join(CORPUS, rel), encoding="latin-1").read()
    get = lambda k: re.search(rf"(?im)^\s*{k}\s*=\s*(.+?)\s*$", text).group(1)  # noqa: E731
    n = [int(x) for x in get("size").split()]
    res = [float(x) for x in get("resolution").split()] if re.search(
        r"(?im)^\s*resolution\s*=", text) else [1.0] * 3
    u = UNIT[get("unit").lower()] if re.search(r"(?im)^\s*unit\s*=", text) else 1.0
    data = re.split(r"[\\/]", get("name"))[-1]
    bits = int(get("bitsperelement"))
    t = {8: "uint8", 16: "uint16", 32: "float32"}[bits]
    m.rows.append("\t".join(["hdr", rel, *map(str, n), *(m.fmt(r * u) for r in res), data,
                             str(bits // 8), t]))


def main():
    for rel in ("txrm/ds-uct-007_asphalt_concrete_07um_1600p_Drift.txrm",
                "txrm/20231012_VERSA_3666_AV_battery_scout_bin2_Drift.txrm",
                "txm/Without_defect_0um_reconst.txm"):
        xradia(rel)
    for rel in ("amira/borophagus/8-bit-bin-2.filtered", "amira/borophagus/8-bit-bin-2.labels",
                "amira/ome/ignacio_test.am", "amira/ome/BF_CheckAmira.am",
                "amira/ome/BF_CheckMIB.am", "amira/ahds/test9.am", "amira/ahds/testscalar.am",
                "amira/ahds/testvector3c.am"):
        amira(rel)
    skyscan("skyscan/slicermorph/png_recon/left_side_damaged__rec.log")
    for d, _, files in sorted(os.walk(CORPUS)):
        for f in sorted(files):
            rel = os.path.relpath(os.path.join(d, f), CORPUS)
            low = f.lower()
            if low.endswith((".xtekct", ".xtekvolume")):
                xtek(rel)
            elif low.endswith(".vgi"):
                vgi(rel)
    with open(os.path.join(CORPUS, "expected.tsv"), "w") as f:
        f.write("# Written by crates/formats/specular-medvol/gen/corpus_indct.py.\n")
        f.write("\n".join(m.rows) + "\n")
    print(len(m.rows), "rows")


if __name__ == "__main__":
    main()
