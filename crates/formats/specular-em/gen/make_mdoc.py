#!/usr/bin/env python3
"""SerialEM .mdoc samples and their oracle.

With no arguments: writes the hand-made tilt series in ../tests/data/mdoc (a small MRC stack,
tilt.mrc, written with mrcfile, and its sidecar tilt.mrc.mdoc in dose-symmetric order) and
the oracle ../tests/data/mdoc/expected-mdoc.tsv for every .mdoc there.

With DIR: writes DIR/expected-mdoc.tsv for every .mdoc in DIR (the fetched public examples,
tools/corpus/fetch-mdoc.sh).

The oracle is the `mdocfile` package (teamtomo, BSD-3-Clause). Rows (tab-separated):
    file NAME SECTIONS TITLES
    global NAME KEY VALUE          (PixelSpacing, ImageFile, ImageSize, DataMode, Voltage)
    section NAME K KIND INDEX      (KIND: ZValue, MontSection or FrameSet)
    field NAME K KEY VALUE         (TiltAngle, Defocus, ExposureDose, Magnification, DateTime)
Run with -I from a venv that has mdocfile, mrcfile and numpy.
"""

import os
import sys

import mdocfile
import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
DATA = os.path.join(HERE, "..", "tests", "data", "mdoc")
GLOBAL = ("PixelSpacing", "ImageFile", "ImageSize", "DataMode", "Voltage")
FIELDS = ("TiltAngle", "Defocus", "ExposureDose", "Magnification", "DateTime")


def value(v):
    if isinstance(v, float):
        return repr(v)
    if isinstance(v, tuple):
        return " ".join(str(x) for x in v)
    return str(v)


def oracle(root):
    rows = ["# Written by gen/make_mdoc.py (mdocfile); do not edit."]
    for name in sorted(os.listdir(root)):
        if not name.endswith(".mdoc"):
            continue
        m = mdocfile.data_models.Mdoc.from_file(os.path.join(root, name))
        rows.append(f"file\t{name}\t{len(m.section_data)}\t{len(m.titles)}")
        g = m.global_data.model_dump()
        for k in GLOBAL:
            if g.get(k) is not None:
                rows.append(f"global\t{name}\t{k}\t{value(g[k])}")
        for i, s in enumerate(m.section_data):
            d = s.model_dump()
            for kind in ("ZValue", "MontSection", "FrameSet"):
                if d.get(kind) is not None:
                    rows.append(f"section\t{name}\t{i}\t{kind}\t{d[kind]}")
                    break
            for k in FIELDS:
                if d.get(k) is not None:
                    rows.append(f"field\t{name}\t{i}\t{k}\t{value(d[k])}")
    with open(os.path.join(root, "expected-mdoc.tsv"), "w") as f:
        f.write("\n".join(rows) + "\n")
    print(f"{len(rows)} rows", file=sys.stderr)


def samples():
    import mrcfile

    os.makedirs(DATA, exist_ok=True)
    tilts = [0.0, 3.0, -3.0, -6.0, 6.0]
    data = np.random.default_rng(5).integers(0, 1000, size=(len(tilts), 6, 8)).astype(np.int16)
    with mrcfile.new(os.path.join(DATA, "tilt.mrc"), overwrite=True) as m:
        m.set_data(data)
    text = ["PixelSpacing = 2.17", "ImageFile = tilt.mrc", "ImageSize = 8 6", "DataMode = 1",
            "", "[T = SerialEM: Digitized on a test scope    02-Nov-26  10:00:00    ]", ""]
    for z, t in enumerate(tilts):
        text += [f"[ZValue = {z}]", f"TiltAngle = {t + 0.00012}", "Magnification = 42000",
                 f"Defocus = {-2.5 - 0.1 * z}", f"ExposureDose = {3.0 * (z + 1)}",
                 f"DateTime = 02-Nov-26  10:0{z}:00", ""]
    with open(os.path.join(DATA, "tilt.mrc.mdoc"), "w") as f:
        f.write("\n".join(text))
    oracle(DATA)


if __name__ == "__main__":
    if len(sys.argv) > 1:
        oracle(sys.argv[1])
    else:
        samples()
