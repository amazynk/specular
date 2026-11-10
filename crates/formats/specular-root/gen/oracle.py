#!/usr/bin/env python3
"""uproot's reading of CERN ROOT files, as the oracle for specular-root (tests/oracle.rs).
uproot is never linked; it runs here, from a venv.

    python3 -I oracle.py DIR OUT.tsv

Every *.root under DIR (recursively) gives rows (tab separated; NAME is the path relative
to DIR, PATH an object's directory path and name, KEY is PATH;cycle):
  file   NAME fVersion fCompress nkeys            (keys in every directory, directories too)
  key    NAME KEY class
  hist   NAME KEY class dims title fEntries        (TH1, TH2, TH3, TProfile, TProfile2D)
  axis   NAME KEY x|y|z nbins lo hi title edges labels
                                                   (edges comma separated only when variable,
                                                    labels |-separated or empty)
  stat   NAME KEY member value                     (fTsumw..., fMaximum, fMinimum)
  cells  NAME KEY count sum sumsq                  (every cell with flow, ROOT's flat order:
                                                    x fastest; profiles as bin means)
  cell   NAME KEY index value                      (up to MAX_CELLS cells, strided)
  sumw2  NAME KEY count sum                        (fSumw2, when stored)
  tree   NAME KEY entries
  graph  NAME KEY n sumx sumy                       (TGraph and TGraph2D: point sums)
  error  NAME KEY words                            (uproot could not read it)
Values print with repr (shortest round trip).
"""

import math
import os
import sys

import numpy as np
import uproot

MAX_CELLS = 64


def clean(s):
    return str(s).replace("\t", " ").replace("\n", "\\n").replace("\r", "\\r")


def fmt(x):
    x = float(x)
    if math.isnan(x):
        return "nan"
    if math.isinf(x):
        return "inf" if x > 0 else "-inf"
    return repr(x)


def flat(values):
    """values(flow=True) is indexed [ix, iy, iz]; ROOT's cells run x fastest."""
    a = np.asarray(values, np.float64)
    return a.T.ravel()


def axis_rows(out, name, key, which, ax):
    n = ax.member("fNbins")
    xb = ax.member("fXbins")
    edges = ",".join(fmt(e) for e in xb) if len(xb) > 0 else ""
    labels = ax.labels()
    lab = "|".join(clean(s) for s in labels) if labels else ""
    out.append(["axis", name, key, which, str(n), fmt(ax.member("fXmin")),
                fmt(ax.member("fXmax")), clean(ax.member("fTitle")), edges, lab])


def hist(out, name, key, cls, h):
    dims = 3 if cls.startswith("TH3") else 2 if cls.startswith(("TH2", "TProfile2D")) else 1
    out.append(["hist", name, key, cls, str(dims), clean(h.member("fTitle")),
                fmt(h.member("fEntries"))])
    for k, which in enumerate("xyz"[:dims]):
        axis_rows(out, name, key, which, h.member("f" + which.upper() + "axis"))
    for m in ["fTsumw", "fTsumw2", "fTsumwx", "fTsumwx2", "fTsumwy", "fTsumwy2", "fTsumwxy",
              "fTsumwz", "fTsumwz2", "fTsumwxz", "fTsumwyz", "fMaximum", "fMinimum"]:
        try:
            v = h.member(m)
        except Exception:
            continue
        out.append(["stat", name, key, m, fmt(v)])
    cells = flat(h.values(flow=True))
    out.append(["cells", name, key, str(len(cells)), fmt(cells.sum()),
                fmt((cells * cells).sum())])
    step = max(1, len(cells) // MAX_CELLS)
    for i in range(0, len(cells), step):
        out.append(["cell", name, key, str(i), fmt(cells[i])])
    if not cls.startswith("TProfile"):
        sw2 = np.asarray(h.member("fSumw2"), np.float64)
        if len(sw2) > 0:
            out.append(["sumw2", name, key, str(len(sw2)), fmt(sw2.sum())])


def one(out, name, path):
    f = uproot.open(path)
    try:
        names = f.classnames(recursive=True, cycle=True)
        out.append(["file", name, str(f.file.fVersion), str(f.file.fCompress), str(len(names))])
        for key, cls in names.items():
            out.append(["key", name, key, cls])
        for key, cls in names.items():
            try:
                if cls.startswith(("TH1", "TH2", "TH3", "TProfile")) and cls != "TProfile3D":
                    hist(out, name, key, cls, f[key])
                elif cls in ("TTree", "TNtuple", "TNtupleD"):
                    out.append(["tree", name, key, str(f[key].num_entries)])
                elif cls in ("TGraph", "TGraph2D"):
                    g = f[key]
                    x = np.asarray(g.member("fX"), np.float64)
                    y = np.asarray(g.member("fY"), np.float64)
                    out.append(["graph", name, key, str(len(x)), fmt(x.sum()), fmt(y.sum())])
            except Exception as e:  # noqa: BLE001 - the oracle records any refusal
                out.append(["error", name, key, clean(type(e).__name__ + ": " + str(e))[:200]])
    finally:
        f.close()


def main():
    root, dest = sys.argv[1], sys.argv[2]
    out = [["# uproot", uproot.__version__]]
    for dirpath, _dirs, files in sorted(os.walk(root)):
        for fn in sorted(files):
            if not fn.endswith(".root"):
                continue
            path = os.path.join(dirpath, fn)
            name = os.path.relpath(path, root)
            rows = []
            try:
                one(rows, name, path)
            except Exception as e:  # noqa: BLE001
                rows = [["error", name, "", clean(type(e).__name__ + ": " + str(e))[:200]]]
            out.extend(rows)
    with open(dest, "w") as fh:
        for r in out:
            fh.write("\t".join(r) + "\n")


main()
