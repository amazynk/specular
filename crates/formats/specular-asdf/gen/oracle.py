#!/usr/bin/env python3
"""asdf-python's (and roman_datamodels') reading of ASDF files, as the oracle for
specular-asdf (tests/oracle.rs). Neither is ever linked; they run here, from a venv.

    python3 -I oracle.py DIR OUT.tsv [MAX_VALUES]

Every *.asdf under DIR (recursively) gives rows (tab separated; NAME is the path relative
to DIR):
  file   NAME version standard library_name library_version
  arr    NAME path dtype byteorder shape unit       (an ndarray in the tree)
  skip   NAME path why                              (an ndarray of string/structured data)
  plane  NAME path lead sum finite                  (sum of the finite values of a plane, as
                                                     float64, and their count)
  val    NAME path lead index value                 (up to MAX_VALUES values, strided)
  tree   NAME path kind value                       (plain scalars of the tree: str int
                                                     float bool null)
  error  NAME words
Planes follow specular-asdf: length-1 axes squeezed (keeping two), the last two axes the
plane, rows in array order; complex values as magnitudes, masked values as NaN, everything as float32.
"""

import math
import os
import sys
from collections.abc import Mapping

import asdf
import numpy as np

MAX_TREE = 400
PLANES = 3


def clean(s):
    return str(s).replace("\t", " ").replace("\n", "\\n").replace("\r", "\\r")


def fmt(x):
    x = float(x)
    if math.isnan(x):
        return "nan"
    if math.isinf(x):
        return "inf" if x > 0 else "-inf"
    return repr(x)


def squeeze(a):
    nd = a.ndim
    keep = [k for k in range(nd) if a.shape[k] != 1]
    if len(keep) < 2 and nd >= 2:
        k = nd
        while len(keep) < 2 and k > 0:
            k -= 1
            if k not in keep:
                keep.append(k)
        keep.sort()
    index = tuple(slice(None) if k in keep else 0 for k in range(nd))
    return a[index]


def as_float(a):
    if np.ma.isMaskedArray(a):
        data = np.asarray(a.data)
        mask = np.ma.getmaskarray(a)
    else:
        data = np.asarray(a)
        mask = None
    if data.dtype.kind == "c":
        f = np.abs(data.astype(np.complex128))
    else:
        f = data.astype(np.float64)
    if mask is not None:
        f = np.where(mask, np.nan, f)
    # Planes are float32, as specular-asdf gives them.
    with np.errstate(over="ignore"):
        return f.astype(np.float32).astype(np.float64)


def leads(shape):
    """Up to PLANES plane positions: first, middle, last (row-major over the lead axes)."""
    n = int(np.prod(shape)) if shape else 1
    picks = sorted({0, n // 2, max(n - 1, 0)})[:PLANES]
    return [tuple(int(i) for i in np.unravel_index(p, shape)) if shape else () for p in picks]


def is_mapping(x):
    return isinstance(x, Mapping)


def walk(x, path, out, depth=0):
    if depth > 64:
        return
    if isinstance(x, np.ndarray) or hasattr(x, "__array__") and hasattr(x, "dtype") \
            and hasattr(x, "shape") and not isinstance(x, (str, bytes)):
        if path:
            out.append(("array", path, x))
        return
    if is_mapping(x):
        for k, v in x.items():
            if not isinstance(k, str):
                continue
            if not path and k in ("history", "asdf_library"):
                continue
            walk(v, f"{path}.{k}" if path else k, out, depth + 1)
        return
    if isinstance(x, (list, tuple)):
        for i, v in enumerate(x):
            walk(v, f"{path}[{i}]", out, depth + 1)
        return
    if type(x) in (str, int, float, bool) or x is None:
        out.append(("scalar", path, x))


def scalar_row(name, path, x):
    if x is None:
        return f"tree\t{name}\t{path}\tnull\t"
    if type(x) is bool:
        return f"tree\t{name}\t{path}\tbool\t{str(x).lower()}"
    if type(x) is int:
        return f"tree\t{name}\t{path}\tint\t{x}"
    if type(x) is float:
        return f"tree\t{name}\t{path}\tfloat\t{fmt(x)}"
    return f"tree\t{name}\t{path}\tstr\t{clean(x)}"


def one(full, name, rows, max_values):
    with open(full, "rb") as fd:
        first = fd.readline().decode("ascii", "replace").strip()
    file_version = first[len("#ASDF "):] if first.startswith("#ASDF ") else ""
    with asdf.open(full, lazy_load=True, memmap=False) as af:
        lib = af.tree.get("asdf_library") or {}
        rows.append(f"file\t{name}\t{file_version}\t{clean(af.version_string)}\t"
                    f"{clean(lib.get('name', ''))}\t{clean(lib.get('version', ''))}")
        found = []
        walk(af.tree, "", found)
        scalars = 0
        for kind, path, x in found:
            if kind == "scalar":
                if scalars < MAX_TREE:
                    rows.append(scalar_row(name, path, x))
                    scalars += 1
                continue
            dt = np.dtype(x.dtype)
            if dt.fields is not None or dt.kind in "SUOV":
                rows.append(f"skip\t{name}\t{path}\t{dt}")
                continue
            order = "big" if dt.byteorder == ">" else "little"
            unit = getattr(x, "unit", None)
            unit = "" if unit is None else clean(unit.to_string())
            rows.append(f"arr\t{name}\t{path}\t{dt.name}\t{order}\t"
                        f"{','.join(str(s) for s in x.shape)}\t{unit}")
            if x.ndim == 0:
                continue
            a = squeeze(x)
            lead_shape = a.shape[:-2] if a.ndim >= 2 else ()
            for lead in leads(lead_shape):
                p = as_float(a[lead] if lead else a)
                p = p.reshape(1, -1) if p.ndim == 1 else p
                finite = np.isfinite(p)
                lt = ",".join(str(i) for i in lead) or "-"
                rows.append(f"plane\t{name}\t{path}\t{lt}\t{fmt(p[finite].sum())}\t"
                            f"{int(finite.sum())}")
                flat = p.ravel()
                step = max(1, len(flat) // max_values)
                for i in range(0, len(flat), step):
                    rows.append(f"val\t{name}\t{path}\t{lt}\t{i}\t{fmt(flat[i])}")


def main():
    root = sys.argv[1]
    out = sys.argv[2]
    max_values = int(sys.argv[3]) if len(sys.argv) > 3 else 64
    rows = ["# asdf-python " + asdf.__version__]
    for dirpath, _, files in sorted(os.walk(root)):
        for f in sorted(files):
            if not f.endswith(".asdf"):
                continue
            full = os.path.join(dirpath, f)
            name = os.path.relpath(full, root)
            mark = len(rows)
            try:
                one(full, name, rows, max_values)
            except Exception as e:  # asdf-python refuses it: record why
                del rows[mark:]
                rows.append(f"error\t{name}\t{clean(type(e).__name__ + ': ' + str(e))[:300]}")
    with open(out, "w", encoding="utf-8") as fd:
        fd.write("\n".join(rows) + "\n")


main()
