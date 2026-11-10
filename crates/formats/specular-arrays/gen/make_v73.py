"""MATLAB v7.3 (HDF5) files and what h5py reads from them, for the HDF5 path of the .mat
reader in specular-decode.

    python -I make_v73.py OUT_DIR [--small]

Files are written by hdf5storage in MATLAB's own layout (MATLAB_class attributes, dimensions
reversed, complex as a {real, imag} compound, cells as references into #refs#, structs as
groups); scipy's MATLAB-written v7.3 test file is copied too. `expected/<name>.json` lists each
array by the path the reader names it (`s.f`, `c{2}`), in MATLAB's shape, with its values
(transposed from the HDF5 layout) as little-endian float32 in C order of that shape; texts of
char arrays; the format is make_samples.py's, kind "mat73".
"""

import json
import os
import shutil
import sys
import warnings

import h5py
import hdf5storage
import numpy as np

warnings.filterwarnings("ignore")
OUT = sys.argv[1]
SMALL = "--small" in sys.argv
EXP = os.path.join(OUT, "expected")
os.makedirs(EXP, exist_ok=True)
rng = np.random.default_rng(73)


def matlab_value(ds):
    a = ds[()]
    if a.dtype.names and "real" in a.dtype.names:
        a = a["real"] + 1j * a["imag"]
    return np.asarray(a).T


def walk(f, path, label, arrays, texts):
    o = f[path]
    cls = o.attrs.get("MATLAB_class", b"")
    cls = cls.decode() if isinstance(cls, bytes) else str(cls)
    if isinstance(o, h5py.Group):
        for k in sorted(o.keys()):
            walk(f, f"{path}/{k}", f"{label}.{k}", arrays, texts)
        return
    if o.attrs.get("MATLAB_empty", 0):
        return
    if o.dtype == h5py.ref_dtype:
        refs = o[()].T  # MATLAB shape
        dims = refs.shape
        for k, idx in enumerate(np.ndindex(*dims[::-1])):
            idx = idx[::-1]
            r = refs[idx]
            if sum(1 for d in dims if d != 1) <= 1:
                sub = str(k + 1)
            else:
                sub = ",".join(str(i + 1) for i in idx)
            name = f"{label}{{{sub}}}" if cls == "cell" else f"{label}({sub})"
            walk(f, f[r].name, name, arrays, texts)
        return
    v = matlab_value(o)
    if cls == "char":
        rows = ["".join(chr(c) for c in row).rstrip() for row in v.reshape(v.shape[0], -1)]
        texts[label] = " / ".join(rows)
        return
    if v.size < 2:
        return
    arrays.append((label, v))


def record(name, arrays, texts):
    out = {"kind": "mat73", "arrays": [], "refused": [], "texts": texts}
    chunks, at = [], 0
    for path, a in arrays:
        if a.dtype.kind == "c":
            mag, re = np.abs(a).astype("<f4"), a.real.astype("<f4")
        else:
            mag, re = a.astype("<f4"), None
        e = {"name": path, "shape": list(a.shape), "offset": at, "complex": re is not None}
        chunks.append(np.ascontiguousarray(mag).reshape(-1))
        at += mag.size
        if re is not None:
            e["re_offset"] = at
            chunks.append(np.ascontiguousarray(re).reshape(-1))
            at += re.size
        out["arrays"].append(e)
    if chunks:
        np.concatenate(chunks).astype("<f4").tofile(os.path.join(EXP, name + ".f32"))
    with open(os.path.join(EXP, name + ".json"), "w") as fh:
        json.dump(out, fh, indent=1, sort_keys=True)


def expect(name):
    arrays, texts = [], {}
    with h5py.File(os.path.join(OUT, name), "r") as f:
        for k in sorted(f.keys()):
            if k.startswith("#"):
                continue
            walk(f, "/" + k, k, arrays, texts)
    record(name, arrays, texts)


def sample(dt, shape):
    dt = np.dtype(dt)
    if dt.kind == "b":
        return rng.integers(0, 2, shape).astype(bool)
    if dt.kind in "iu":
        return rng.integers(0, 100, shape).astype(dt)
    if dt.kind == "c":
        return (rng.standard_normal(shape) + 1j * rng.standard_normal(shape)).astype(dt)
    return (rng.standard_normal(shape) * 10).astype(dt)


for i in range(2 if SMALL else 10):
    d = {
        "a": sample("f8", (5, 7)),
        "vol": sample("f4", (4, 6, 3 + i % 3)),
        "i16": sample("i2", (3, 8)),
        "u64": sample("u8", (2, 9)),
        "flag": sample("b", (4, 4)),
        "z": sample("c16", (3, 5)),
        "zs": sample("c8", (2, 4, 2)),
        "rgb": sample("u1", (6, 8, 3)),
        "vec": sample("f8", (1, 11)),
        "name": "label text",
        "s": {"f": sample("i4", (2, 3)), "t": "inner", "g": {"deep": sample("f8", (3, 3))}},
        "c": np.array([sample("f8", (2, 2)), "cell text", sample("u2", (3, 4))], dtype=object),
    }
    name = f"v73_{i:02d}.mat"
    hdf5storage.savemat(os.path.join(OUT, name), d, format="7.3", matlab_compatible=True,
                        store_python_metadata=False,
                        compress=bool(i % 2))
    expect(name)

import scipy.io  # noqa: E402
import scipy.sparse  # noqa: E402


def sparse_file(name, mats):
    """Sparse matrices in MATLAB's v7.3 layout (hdf5storage cannot write them): a group per
    matrix with data, ir and jc, the row count in MATLAB_sparse; complex data as a compound."""
    path = os.path.join(OUT, name)
    with h5py.File(path, "w", userblock_size=512) as f:
        for k, m in mats.items():
            m = scipy.sparse.csc_matrix(m)
            g = f.create_group(k)
            g.attrs["MATLAB_class"] = np.bytes_(b"double")
            g.attrs["MATLAB_sparse"] = np.uint64(m.shape[0])
            if np.iscomplexobj(m.data):
                dt = np.dtype([("real", "<f8"), ("imag", "<f8")])
                d = np.zeros(m.nnz, dt)
                d["real"], d["imag"] = m.data.real, m.data.imag
                g.create_dataset("data", data=d)
            else:
                g.create_dataset("data", data=m.data.astype("<f8"))
            g.create_dataset("ir", data=m.indices.astype("<u8"))
            g.create_dataset("jc", data=m.indptr.astype("<u8"))
    head = b"MATLAB 7.3 MAT-file, Platform: h5py, HDF5 schema 1.00 ."
    head = head.ljust(116, b" ") + b"\0" * 8 + b"\x00\x02IM"
    with open(path, "r+b") as fh:
        fh.write(head)
    arrays = [(k, scipy.sparse.csc_matrix(m).toarray()) for k, m in sorted(mats.items())]
    record(name, arrays, {})


sparse_file("v73_sparse.mat", {
    "sp": scipy.sparse.random(7, 5, density=0.3, format="csc", random_state=3) * 4,
    "zs": scipy.sparse.random(4, 6, density=0.4, format="csc", random_state=4) * (2 - 1j),
})

src = os.path.join(os.path.dirname(scipy.io.__file__), "matlab", "tests", "data",
                   "testhdf5_7.4_GLNX86.mat")
shutil.copy(src, os.path.join(OUT, "scipy_testhdf5_7.4_GLNX86.mat"))
expect("scipy_testhdf5_7.4_GLNX86.mat")
print("written to", OUT)
