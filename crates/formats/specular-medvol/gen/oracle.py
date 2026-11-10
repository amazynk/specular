#!/usr/bin/env python3
"""The oracle over a fetched corpus (tools/corpus/fetch-medvol.sh): for every volume under
DIR, what nibabel (NIfTI, Analyze) or pynrrd and SimpleITK (NRRD, MetaImage) read from it, in
the format of tests/data/expected.tsv (see make_samples.py), with up to about 4000 voxel
values per file. Files the oracle cannot read are listed as `error` rows with the words
specular-medvol's reason should contain, when known.

Run with -I:  python3 -I oracle.py DIR OUT.tsv
"""

import os
import sys

import nibabel as nib
import nrrd
import numpy as np
import SimpleITK as sitk

RAS_TO_LPS = np.diag([-1.0, -1.0, 1.0, 1.0])
EXTS = (".nii", ".nii.gz", ".hdr", ".nrrd", ".nhdr", ".mha", ".mhd")


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
    """d: [i, j, k, volume] scaled values."""
    n = d.size
    step = max(1, n // 4000)
    out = []
    flat = d.reshape(-1, order="F")
    sx, sy, sz, sv = d.shape
    for idx in range(0, n, step):
        i = idx % sx
        j = (idx // sx) % sy
        k = (idx // (sx * sy)) % sz
        v = idx // (sx * sy * sz)
        out.append(f"val\t{name}\t{i}\t{j}\t{k}\t{v}\t0\t{fmt(flat[idx])}")
    return out


def as4(d):
    d = np.asarray(d, dtype=np.float64)
    if d.ndim < 3:
        d = d.reshape(d.shape + (1,) * (3 - d.ndim), order="F")
    return d.reshape(d.shape[:3] + (-1,), order="F")


def nifti(path, name):
    img = nib.load(path)
    kind = {nib.Nifti1Image: "NIfTI-1", nib.Nifti2Image: "NIfTI-2",
            nib.Nifti1Pair: "NIfTI-1", nib.Nifti2Pair: "NIfTI-2"}.get(type(img), "Analyze 7.5")
    h = img.header
    if kind.startswith("NIfTI"):
        orient = "sform" if h["sform_code"] != 0 else "qform" if h["qform_code"] != 0 else "none"
    else:
        orient = "analyze"
    d = as4(img.get_fdata(dtype=np.float64))
    rows = [f"file\t{name}\t{kind}\t{img.get_data_dtype().name}\t{orient}\t{d.shape[3]}"]
    rows += geom_rows(name, (RAS_TO_LPS @ img.affine)[:3, :], img.shape)
    rows += val_rows(name, d)
    return rows


def nrrd_rows(path, name):
    data, hdr = nrrd.read(path, index_order="F")
    if data.ndim > 3:
        # Only volumes whose extra axes come last are compared value by value.
        kinds = hdr.get("kinds", ["domain"] * data.ndim)
        if any(k not in ("domain", "space") for k in kinds[:3]):
            return [f"skip\t{name}"]
    orient = "space" if "space" in hdr and "space directions" in hdr else "spacings"
    d = as4(data)
    rows = [f"file\t{name}\tNRRD\t{data.dtype.name}\t{orient}\t{d.shape[3]}"]
    try:
        img = sitk.ReadImage(path)
        if img.GetDimension() == 3 and img.GetNumberOfComponentsPerPixel() == 1:
            size = img.GetSize()
            pts = {(i, j, k) for i in (0, size[0] - 1) for j in (0, size[1] - 1) for k in (0, size[2] - 1)}
            for p in sorted(pts):
                x = img.TransformContinuousIndexToPhysicalPoint([float(c) for c in p])
                rows.append(f"geom\t{name}\t{p[0]}\t{p[1]}\t{p[2]}\t{fmt(x[0])}\t{fmt(x[1])}\t{fmt(x[2])}")
    except RuntimeError:
        pass
    rows += val_rows(name, d)
    return rows


def meta_rows(path, name):
    img = sitk.ReadImage(path)
    if img.GetNumberOfComponentsPerPixel() != 1 or img.GetDimension() != 3:
        return [f"skip\t{name}"]
    a = np.transpose(sitk.GetArrayFromImage(img))
    d = as4(a)
    rows = [f"file\t{name}\tMetaImage\t{a.dtype.name}\ttransform\t1"]
    size = img.GetSize()
    pts = {(i, j, k) for i in (0, size[0] - 1) for j in (0, size[1] - 1) for k in (0, size[2] - 1)}
    for p in sorted(pts):
        x = img.TransformContinuousIndexToPhysicalPoint([float(c) for c in p])
        rows.append(f"geom\t{name}\t{p[0]}\t{p[1]}\t{p[2]}\t{fmt(x[0])}\t{fmt(x[1])}\t{fmt(x[2])}")
    rows += val_rows(name, d)
    return rows


def main():
    root, out = sys.argv[1], sys.argv[2]
    rows = []
    for dirpath, _, files in sorted(os.walk(root)):
        for f in sorted(files):
            if not f.lower().endswith(EXTS):
                continue
            path = os.path.join(dirpath, f)
            name = os.path.relpath(path, root)
            if name.startswith("speed" + os.sep):
                continue
            try:
                if f.lower().endswith((".nrrd", ".nhdr")):
                    with open(path, "rb") as fh:
                        head = fh.read(4096)
                    if b"bzip2" in head or b"bz2" in head:
                        rows.append(f"error\t{name}\tbzip2")
                        continue
                    rows += nrrd_rows(path, name)
                elif f.lower().endswith((".mha", ".mhd")):
                    rows += meta_rows(path, name)
                else:
                    rows += nifti(path, name)
            except Exception as e:  # noqa: BLE001 - the oracle refusing is itself a result
                print(f"oracle: {name}: {e}", file=sys.stderr)
                rows.append(f"error\t{name}\t")
    with open(out, "w") as fh:
        fh.write("# Written by crates/formats/specular-medvol/gen/oracle.py; do not edit.\n")
        fh.write("\n".join(rows) + "\n")
    print(f"oracle: {sum(r.startswith('file') for r in rows)} volumes, "
          f"{sum(r.startswith('error') for r in rows)} refused", file=sys.stderr)


if __name__ == "__main__":
    main()
