#!/usr/bin/env python3
"""Writes the CERN ROOT fixtures in tests/data with uproot; gen/oracle.py then records
uproot's reading of them in tests/data/expected.tsv. Run from a venv (uproot, numpy, lz4,
xxhash, zstandard):

    python3 -I make_fixtures.py OUT_DIR

Each fixture stays well under 200 KB.
  types.root       TH1, TH2 and TH3 in F, D, I, S and C (zlib), filled with weights and flow
  variable.root    variable bins on TH1 and TH2 (x, y or both), and a TH3 with a variable z
  labels.root      alphanumeric bin labels on TH1 and TH2 axes, axis titles with units
  profiles.root    TProfile and TProfile2D
  nested.root      histograms in nested directories, two cycles of one name, and a TTree
  comp_*.root      one TH2 in every compression uproot writes: none, zlib, lz4, zstd, lzma
  empty.root       a file without objects
"""

import os
import sys

import numpy as np
import uproot
from uproot.writing import identify as w

OUT = sys.argv[1]
os.makedirs(OUT, exist_ok=True)
rng = np.random.default_rng(20261012)
DT = {"F": np.float32, "D": np.float64, "I": np.int32, "S": np.int16, "C": np.int8}


def path(name):
    return os.path.join(OUT, name)


def axis(name, title, n, lo, hi, edges=None, labels=None):
    lab = None
    if labels is not None:
        objs = []
        for s in labels:
            o = w.to_TObjString(s)
            objs.append(o)  # uproot leaves fUniqueID (ROOT's bin number) at 0
        lab = w.to_THashList(objs)
    if edges is not None:
        edges = np.asarray(edges, np.float64)
        return w.to_TAxis(name, title, len(edges) - 1, float(edges[0]), float(edges[-1]),
                          fXbins=edges, fLabels=lab)
    return w.to_TAxis(name, title, n, lo, hi, fLabels=lab)


def edges_of(ax):
    xb = ax.member("fXbins")
    if len(xb) > 0:
        return np.asarray(xb, np.float64)
    return np.linspace(ax.member("fXmin"), ax.member("fXmax"), ax.member("fNbins") + 1)


def index(e, v):
    """ROOT's bin index (0 underflow, n+1 overflow) of each value."""
    i = np.searchsorted(e, v, side="right")
    return np.clip(i, 0, len(e))


def fill(axes, n, kind):
    """Random weighted entries over (and past) the axes: counts with flow in ROOT's flat cell
    order (x fastest), sumw2 and the stat sums."""
    edges = [edges_of(a) for a in axes]
    shape = [len(e) + 1 for e in edges]  # n + 2 cells per axis
    pts = []
    for e in edges:
        lo, hi = e[0], e[-1]
        span = hi - lo
        pts.append(rng.normal(lo + span * 0.45, span * 0.3, n))
    integer = kind in "ISC"
    wts = np.ones(n) if integer else rng.uniform(0.5, 1.5, n)
    idx = [index(e, p) for e, p in zip(edges, pts)]
    flat = np.zeros(int(np.prod(shape)))
    sw2 = np.zeros_like(flat)
    lin = np.zeros(n, np.int64)
    stride = 1
    for k, ix in enumerate(idx):
        lin += ix * stride
        stride *= shape[k]
    np.add.at(flat, lin, wts)
    np.add.at(sw2, lin, wts * wts)
    if kind == "C":
        flat = np.clip(flat, -127, 127)
    if kind == "S":
        flat = np.clip(flat, -32767, 32767)
    inside = np.ones(n, bool)
    for e, p in zip(edges, pts):
        inside &= (p >= e[0]) & (p < e[-1])
    wi = wts[inside]
    sums = {"fTsumw": wi.sum(), "fTsumw2": (wi * wi).sum()}
    for k, nm in enumerate("xyz"[: len(axes)]):
        p = pts[k][inside]
        sums["fTsumw" + nm] = (wi * p).sum()
        sums["fTsumw" + nm + "2"] = (wi * p * p).sum()
    if len(axes) >= 2:
        sums["fTsumwxy"] = (wi * pts[0][inside] * pts[1][inside]).sum()
    if len(axes) == 3:
        sums["fTsumwxz"] = (wi * pts[0][inside] * pts[2][inside]).sum()
        sums["fTsumwyz"] = (wi * pts[1][inside] * pts[2][inside]).sum()
    return flat.astype(DT[kind]), sw2, float(n), sums


def th(name, title, kind, axes, n=4000, sumw2=True, **kw):
    data, sw2, entries, s = fill(axes, n, kind)
    sw2 = sw2 if sumw2 else np.array([], np.float64)
    if len(axes) == 1:
        return w.to_TH1x(name, title, data, entries, s["fTsumw"], s["fTsumw2"], s["fTsumwx"],
                         s["fTsumwx2"], sw2, axes[0], **kw)
    if len(axes) == 2:
        return w.to_TH2x(name, title, data, entries, s["fTsumw"], s["fTsumw2"], s["fTsumwx"],
                         s["fTsumwx2"], s["fTsumwy"], s["fTsumwy2"], s["fTsumwxy"], sw2,
                         axes[0], axes[1], **kw)
    return w.to_TH3x(name, title, data, entries, s["fTsumw"], s["fTsumw2"], s["fTsumwx"],
                     s["fTsumwx2"], s["fTsumwy"], s["fTsumwy2"], s["fTsumwxy"], s["fTsumwz"],
                     s["fTsumwz2"], s["fTsumwxz"], s["fTsumwyz"], sw2, axes[0], axes[1], axes[2],
                     **kw)


def xyz(nx, ny=None, nz=None):
    out = [axis("xaxis", "x [cm]", nx, -5.0, 5.0)]
    if ny:
        out.append(axis("yaxis", "y [cm]", ny, 0.0, 30.0))
    if nz:
        out.append(axis("zaxis", "E (GeV)", nz, 0.0, 100.0))
    return out


# --- types.root: every storage type in 1, 2 and 3 dimensions ---
with uproot.recreate(path("types.root"), compression=uproot.ZLIB(4)) as f:
    for kind in "FDISC":
        f["h1" + kind.lower()] = th("h1" + kind.lower(), f"TH1{kind} test;x [cm];entries",
                                    kind, xyz(40), sumw2=kind in "FD")
        f["h2" + kind.lower()] = th("h2" + kind.lower(), f"TH2{kind} test;x [cm];y [cm]",
                                    kind, xyz(24, 16), n=6000, sumw2=kind in "FD")
        f["h3" + kind.lower()] = th("h3" + kind.lower(), f"TH3{kind} test", kind,
                                    xyz(12, 10, 8), n=6000, sumw2=kind == "D")
    # Stored display limits.
    f["h2range"] = th("h2range", "fMinimum and fMaximum set", "D", xyz(20, 20),
                      fMinimum=-2.0, fMaximum=40.0)

# --- variable.root ---
with uproot.recreate(path("variable.root")) as f:
    ex = np.array([0, 1, 2, 4, 7, 11, 16, 22, 30, 40, 55, 75, 100], float)
    ey = np.array([-3, -2.5, -1, 0, 0.25, 0.5, 2, 3.5, 6], float)
    f["v1"] = th("v1", "variable TH1;p_{T} [GeV];events", "D",
                 [axis("xaxis", "p_{T} [GeV]", 0, 0, 0, edges=ex)])
    f["v2x"] = th("v2x", "variable x;p_{T} [GeV];#eta", "F",
                  [axis("xaxis", "p_{T} [GeV]", 0, 0, 0, edges=ex),
                   axis("yaxis", "#eta", 18, -3.0, 6.0)], n=8000)
    f["v2y"] = th("v2y", "variable y;x;y", "D",
                  [axis("xaxis", "x", 30, 0.0, 3.0), axis("yaxis", "y", 0, 0, 0, edges=ey)],
                  n=8000)
    f["v2xy"] = th("v2xy", "variable x and y", "D",
                   [axis("xaxis", "p_{T} [GeV]", 0, 0, 0, edges=ex),
                    axis("yaxis", "y", 0, 0, 0, edges=ey)], n=8000)
    f["v3z"] = th("v3z", "variable z", "F",
                  [axis("xaxis", "x", 8, 0.0, 8.0), axis("yaxis", "y", 6, 0.0, 6.0),
                   axis("zaxis", "z [mm]", 0, 0, 0, edges=[0, 1, 3, 6, 10, 15])], n=4000)

# --- labels.root ---
with uproot.recreate(path("labels.root")) as f:
    cuts = ["all", "trigger", "2 muons", "opposite sign", "m > 60", "isolated"]
    f["cutflow"] = th("cutflow", "cut flow;;events", "D",
                      [axis("xaxis", "", len(cuts), 0.0, float(len(cuts)), labels=cuts)])
    det = ["barrel", "endcap A", "endcap C"]
    f["occupancy"] = th("occupancy", "occupancy;layer;region", "I",
                        [axis("xaxis", "layer", 12, 0.0, 12.0),
                         axis("yaxis", "region", 3, 0.0, 3.0, labels=det)])

# --- profiles.root ---


def profile(name, title, axes, n=5000):
    edges = [edges_of(a) for a in axes]
    shape = [len(e) + 1 for e in edges]
    pts = [rng.normal(e[0] + (e[-1] - e[0]) * 0.5, (e[-1] - e[0]) * 0.35, n) for e in edges]
    y = 2.0 * pts[0] + rng.normal(0, 1, n)
    if len(axes) == 2:
        y = y + pts[1]
    wts = rng.uniform(0.5, 1.5, n)
    lin = np.zeros(n, np.int64)
    stride = 1
    for k, e in enumerate(edges):
        lin += index(e, pts[k]) * stride
        stride *= shape[k]
    size = int(np.prod(shape))
    swy = np.zeros(size)
    swy2 = np.zeros(size)
    sw = np.zeros(size)
    sw2 = np.zeros(size)
    np.add.at(swy, lin, wts * y)
    np.add.at(swy2, lin, wts * y * y)
    np.add.at(sw, lin, wts)
    np.add.at(sw2, lin, wts * wts)
    inside = np.ones(n, bool)
    for e, p in zip(edges, pts):
        inside &= (p >= e[0]) & (p < e[-1])
    wi = wts[inside]
    x0 = pts[0][inside]
    yi = y[inside]
    if len(axes) == 1:
        return w.to_TProfile(name, title, swy, float(n), wi.sum(), (wi * wi).sum(),
                             (wi * x0).sum(), (wi * x0 * x0).sum(), (wi * yi).sum(),
                             (wi * yi * yi).sum(), swy2, sw, sw2, axes[0])
    y0 = pts[1][inside]
    return w.to_TProfile2D(name, title, swy, float(n), wi.sum(), (wi * wi).sum(),
                           (wi * x0).sum(), (wi * x0 * x0).sum(), (wi * y0).sum(),
                           (wi * y0 * y0).sum(), (wi * x0 * y0).sum(), (wi * yi).sum(),
                           (wi * yi * yi).sum(), swy2, sw, sw2, axes[0], axes[1])


with uproot.recreate(path("profiles.root")) as f:
    f["prof"] = profile("prof", "profile;x [cm];<y> [cm]", xyz(25))
    f["prof2d"] = profile("prof2d", "profile 2D;x [cm];y [cm]", xyz(20, 15), n=8000)

# --- nested.root ---
with uproot.recreate(path("nested.root")) as f:
    f["top"] = th("top", "at the top", "F", xyz(10, 8))
    f["run1/hits"] = th("hits", "hits;x [cm];y [cm]", "D", xyz(16, 12))
    f["run1/tracker/layer0"] = th("layer0", "layer 0", "I", xyz(20))
    f["run1/tracker/inner/occ"] = th("occ", "deep", "F", xyz(8, 8))
    f["run2/hits"] = th("hits", "hits run 2", "D", xyz(16, 12))
    t = f.mktree("events", {"px": np.float64, "n": np.int64}, title="event tree")
    t.extend({"px": rng.normal(0, 10, 1234), "n": rng.integers(0, 5, 1234)})
    f["ntuple"] = {"px": rng.normal(0, 10, 321)}  # an RNTuple: listed, not read
# A second cycle of one name: uproot keeps the old key when overwriting in update mode.
with uproot.update(path("nested.root")) as f:
    f["top"] = th("top", "at the top, cycle 2", "F", xyz(10, 8))

# --- comp_*.root: one TH2 in every compression ---
for tag, comp in [("none", None), ("zlib", uproot.ZLIB(6)), ("lz4", uproot.LZ4(4)),
                  ("zstd", uproot.ZSTD(5)), ("lzma", uproot.LZMA(6))]:
    rng = np.random.default_rng(7)  # the same histograms in every file
    with uproot.recreate(path(f"comp_{tag}.root"), compression=comp) as f:
        f["hpxpy"] = th("hpxpy", "py vs px;px;py", "F",
                        [axis("xaxis", "px", 64, -4.0, 4.0), axis("yaxis", "py", 48, -4.0, 4.0)],
                        n=25000)
        f["hpx"] = th("hpx", "px distribution;px", "F", [axis("xaxis", "px", 100, -4.0, 4.0)],
                      n=25000)

with uproot.recreate(path("empty.root")):
    pass

for name in sorted(os.listdir(OUT)):
    if name.endswith(".root"):
        size = os.path.getsize(path(name))
        assert size < 200_000, (name, size)
        print(f"{name}\t{size}")
