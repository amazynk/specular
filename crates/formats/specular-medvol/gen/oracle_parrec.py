#!/usr/bin/env python3
"""The oracle over the fetched PAR/REC and ECAT 7 corpus (tools/corpus/fetch-medvol.sh,
section src_parrec): what nibabel reads from every file under DIR/parrec, in the rows of
tests/data/expected.tsv. A PAR header without its REC data gets a `head` row (kind,
orientation, dimensions) and its geometry only. Files nibabel refuses get `skip` rows.

Run with -I:  python3 -I oracle_parrec.py DIR OUT.tsv
"""

import os
import sys
import warnings

import nibabel as nib
import numpy as np
from nibabel import ecat, parrec

RAS_TO_LPS = np.diag([-1.0, -1.0, 1.0, 1.0])


def fmt(x):
    return repr(float(x))


def geom_rows(name, aff, shape):
    nx, ny, nz = (list(shape) + [1, 1, 1])[:3]
    pts = {(i, j, k) for i in (0, nx - 1) for j in (0, ny - 1) for k in (0, nz - 1)}
    pts.add((nx // 2, ny // 2, nz // 2))
    out = []
    for p in sorted(pts):
        x = aff @ np.array([p[0], p[1], p[2], 1.0])
        out.append(f"geom\t{name}\t{p[0]}\t{p[1]}\t{p[2]}\t{fmt(x[0])}\t{fmt(x[1])}\t{fmt(x[2])}")
    return out


def val_rows(name, d):
    if d.ndim == 3:
        d = d[..., np.newaxis]
    flat = d.reshape(-1, order="F")
    step = max(1, flat.size // 4000)
    nx, ny, nz, _ = d.shape
    out = []
    for n in range(0, flat.size, step):
        i, j, k, v = n % nx, n // nx % ny, n // (nx * ny) % nz, n // (nx * ny * nz)
        out.append(f"val\t{name}\t{i}\t{j}\t{k}\t{v}\t0\t{fmt(d[i, j, k, v])}")
    return out


def main():
    root, out_path = sys.argv[1], sys.argv[2]
    rows = [f"# Written by gen/oracle_parrec.py (nibabel {nib.__version__}); do not edit."]
    warnings.simplefilter("ignore")
    for f in sorted(os.listdir(os.path.join(root, "parrec"))):
        name = os.path.join("parrec", f)
        path = os.path.join(root, name)
        low = f.lower()
        if low.endswith(".par"):
            rec = os.path.splitext(path)[0] + ".REC"
            try:
                # Incomplete runs as nibabel reads them when allowed (specular-medvol reads the
                # complete volumes, as nibabel's permit_truncated does).
                try:
                    parrec.load(path) if os.path.isfile(rec) else None
                    trunc = False
                except parrec.PARRECError as e:
                    trunc = "inconsistency" in str(e) or "partial" in str(e)
                    if not trunc:
                        raise
                if os.path.isfile(rec):
                    img = parrec.load(path, permit_truncated=trunc)
                    aff, shape = img.affine, img.shape
                else:
                    with open(path) as fo:
                        h = parrec.PARRECHeader.from_fileobj(fo, permit_truncated=True)
                    aff, shape = h.get_affine(), h.get_data_shape()
            except Exception as e:  # noqa: BLE001 - the oracle's refusal is recorded
                rows.append(f"skip\t{name}\tnibabel: {type(e).__name__}: {e}".replace("\n", " ")[:200])
                continue
            aff = RAS_TO_LPS @ aff
            vols = shape[3] if len(shape) > 3 else 1
            if os.path.isfile(rec):
                d = img.get_fdata(dtype=np.float64)
                sl = img.dataobj._slice_scaling
                uniform = sl is None or (np.all(sl[0] == sl[0].flat[0]) and np.all(sl[1] == sl[1].flat[0]))
                dtype = img.get_data_dtype().name if uniform else "float32"
                rows.append(f"file\t{name}\tPhilips PAR/REC\t{dtype}\tparrec\t{vols}")
                rows += geom_rows(name, aff, img.shape)
                rows += val_rows(name, d)
            else:
                dims = "x".join(str(n) for n in shape)
                rows.append(f"head\t{name}\tPhilips PAR/REC\tparrec\t{dims}")
                rows += geom_rows(name, aff, shape)
        elif low.endswith(".v"):
            img = ecat.load(path)
            d = img.get_fdata(dtype=np.float64)
            rows.append(f"file\t{name}\tECAT 7\t{img.get_data_dtype(0).name}\tecat\t{d.shape[3]}")
            rows += geom_rows(name, RAS_TO_LPS @ img.affine, img.shape)
            rows += val_rows(name, d)
    with open(out_path, "w") as f:
        f.write("\n".join(rows) + "\n")
    print(f"{len(rows)} rows", file=sys.stderr)


if __name__ == "__main__":
    main()
