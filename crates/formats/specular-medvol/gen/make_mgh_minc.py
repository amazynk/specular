#!/usr/bin/env python3
"""Writes the small FreeSurfer MGH / MGZ and MINC1 / MINC2 samples in ../tests/data and their
oracle, ../tests/data/expected-mgh-minc.tsv (same rows as expected.tsv), read back with
nibabel. MGH files are written by nibabel; MINC2 files by h5py and MINC1 files by nibabel's
NetCDF writer, laid out as the MINC 2.0 and 1.0 references describe.

The oracle is only for tests: nothing here is used at run time. Run it with -I from a venv
that has nibabel, h5py and numpy:

    python3 -I crates/formats/specular-medvol/gen/make_mgh_minc.py
"""

import math
import os
import sys

import h5py
import nibabel as nib
import numpy as np
from nibabel.externals.netcdf import netcdf_file

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "..", "tests", "data")
RAS_TO_LPS = np.diag([-1.0, -1.0, 1.0, 1.0])

rows = []


def fmt(x):
    return repr(float(x))


def record(name, kind, dtype, orient, vols):
    rows.append(f"file\t{name}\t{kind}\t{dtype}\t{orient}\t{vols}")


def geom(name, affine_lps, shape):
    nx, ny, nz = (list(shape) + [1, 1, 1])[:3]
    pts = [(i, j, k) for i in (0, nx - 1) for j in (0, ny - 1) for k in (0, nz - 1)]
    pts.append((min(1, nx - 1), min(1, ny - 1), min(1, nz - 1)))
    for p in sorted(set(pts)):
        x = affine_lps @ np.array([p[0], p[1], p[2], 1.0])
        rows.append(f"geom\t{name}\t{p[0]}\t{p[1]}\t{p[2]}\t{fmt(x[0])}\t{fmt(x[1])}\t{fmt(x[2])}")


def vals(name, d):
    """d indexed [i, j, k, (volume)], i the fastest stored axis."""
    d = np.asarray(d, dtype=np.float64)
    if d.ndim == 3:
        d = d[..., np.newaxis]
    nx, ny, nz, nv = d.shape
    for v in range(nv):
        for k in range(nz):
            for j in range(ny):
                for i in range(nx):
                    rows.append(f"val\t{name}\t{i}\t{j}\t{k}\t{v}\t0\t{fmt(d[i, j, k, v])}")


def values_for(dtype, shape, seed):
    r = np.random.default_rng(seed)
    dt = np.dtype(dtype)
    if dt.kind == "f":
        return (r.standard_normal(shape) * 1000).astype(dt)
    info = np.iinfo(dt)
    a = r.integers(info.min, info.max, size=shape, endpoint=True, dtype=np.int64).astype(dt)
    flat = a.reshape(-1)
    flat[0] = info.min
    flat[-1] = info.max
    return a


def oblique(zooms, angle, shift):
    c, s = math.cos(angle), math.sin(angle)
    rot = np.array([[c, 0, s], [0, 1.0, 0], [-s, 0, c]]) @ np.array([[c, -s, 0], [s, c, 0], [0, 0, 1.0]])
    a = np.eye(4)
    a[:3, :3] = rot @ np.diag(zooms)
    a[:3, 3] = shift
    return a


def mgh_cases():
    shape = (6, 5, 4)
    for i, dt in enumerate(["uint8", "int16", "int32", "float32"]):
        name = f"mgh_{dt}.mgh"
        img = nib.MGHImage(values_for(dt, shape, 40 + i), oblique((1.25, 1.5, 2.0), 0.2 * (i + 1), (5.0, -7.0, 9.0)))
        mgh_case(name, img)
    # Frames, gzip-compressed, with scan parameters.
    img = nib.MGHImage(values_for("float32", shape + (3,), 50), oblique((1.0, 1.0, 1.0), 0.4, (0.0, 12.0, -3.0)))
    img.header["tr"] = 2300.0
    img.header["te"] = 2.98
    img.header["flip_angle"] = math.radians(9.0)
    img.header["ti"] = 900.0
    mgh_case("frames.mgz", img)


def mgh_case(name, img):
    path = os.path.join(OUT, name)
    nib.save(img, path)
    back = nib.load(path)
    d = back.get_fdata(dtype=np.float64)
    vols = d.shape[3] if d.ndim > 3 else 1
    record(name, "FreeSurfer MGH", back.get_data_dtype().name, "vox2ras", vols)
    geom(name, (RAS_TO_LPS @ back.affine)[:3, :], back.shape)
    vals(name, d)


def minc_dims(names, steps, starts, cosines):
    return {n: (st, s0, c) for n, st, s0, c in zip(names, steps, starts, cosines)}


S = math.sqrt(0.5)
GEOM = minc_dims(
    ["xspace", "yspace", "zspace"],
    [-1.5, 2.0, 2.5],
    [30.0, -40.0, 12.5],
    [[1.0, 0.0, 0.0], [0.0, S, S], [0.0, -S, S]],
)


def write_minc2(path, dimorder, data, valid_range, imin, imax, scale_dims, gzip=False):
    with h5py.File(path, "w") as f:
        root = f.create_group("minc-2.0")
        root.attrs["ident"] = np.bytes_(b"specular test sample")
        root.attrs["minc_version"] = np.bytes_(b"2.0")
        dims = root.create_group("dimensions")
        for n, size in zip(dimorder, data.shape):
            d = dims.create_dataset(n, data=np.int32(0))
            d.attrs["length"] = np.int32(size)
            d.attrs["varid"] = np.bytes_(b"MINC standard variable")
            d.attrs["vartype"] = np.bytes_(b"dimension____")
            d.attrs["spacing"] = np.bytes_(b"regular__")
            if n in GEOM:
                step, start, cos = GEOM[n]
                d.attrs["step"] = np.float64(step)
                d.attrs["start"] = np.float64(start)
                d.attrs["direction_cosines"] = np.array(cos, np.float64)
                d.attrs["units"] = np.bytes_(b"mm")
            else:
                d.attrs["step"] = np.float64(1.0)
                d.attrs["start"] = np.float64(0.0)
        img = root.create_group("image").create_group("0")
        kw = dict(compression="gzip", chunks=tuple(max(1, s // 2) for s in data.shape)) if gzip else {}
        im = img.create_dataset("image", data=data, **kw)
        im.attrs["dimorder"] = np.bytes_(",".join(dimorder).encode())
        if valid_range is not None:
            im.attrs["valid_range"] = np.array(valid_range, np.float64)
        if imin is not None:
            order = np.bytes_(",".join(dimorder[:scale_dims]).encode())
            for key, v in (("image-min", imin), ("image-max", imax)):
                ds = img.create_dataset(key, data=np.asarray(v, np.float64))
                ds.attrs["dimorder"] = order


def minc_case(name, kind, per_slice=False):
    """per_slice: integers scaled slice by slice, which the reader holds as float32."""
    path = os.path.join(OUT, name)
    back = nib.load(path)
    # nibabel keeps MINC's (slowest first) axis order; the reader's i is the fastest.
    d = back.get_fdata(dtype=np.float64)
    perm = list(range(d.ndim))[::-1]
    # Time, the slowest MINC axis, ends up last: the volume axis.
    d = d.transpose(perm)
    aff = np.eye(4)
    aff[:3, :3] = back.affine[:3, :3][:, ::-1]
    aff[:3, 3] = back.affine[:3, 3]
    vols = d.shape[3] if d.ndim > 3 else 1
    dtype = "float32" if per_slice else back.get_data_dtype().name
    record(name, kind, dtype, "minc", vols)
    geom(name, (RAS_TO_LPS @ aff)[:3, :], d.shape[:3])
    vals(name, d)


def minc2_cases():
    zyx = ["zspace", "yspace", "xspace"]
    shape = (4, 5, 6)
    # Signed shorts, a scale per slice.
    d = values_for("int16", shape, 60)
    write_minc2(os.path.join(OUT, "minc2_slices.mnc"), zyx, d, [-32768, 32767],
                [-10.0 * (k + 1) for k in range(4)], [100.0 * (k + 1) for k in range(4)], 1)
    minc_case("minc2_slices.mnc", "MINC2", per_slice=True)
    # Unsigned bytes, one scale (scalar image-min / image-max), gzip chunks, y slowest.
    yzx = ["yspace", "zspace", "xspace"]
    d = values_for("uint8", (5, 4, 6), 61)
    write_minc2(os.path.join(OUT, "minc2_gzip.mnc"), yzx, d, [0, 255], -1.0, 1.0, 0, gzip=True)
    minc_case("minc2_gzip.mnc", "MINC2")
    # Floats are never rescaled.
    d = values_for("float32", shape, 62)
    write_minc2(os.path.join(OUT, "minc2_float.mnc"), zyx, d, None, -5.0, 5.0, 0)
    minc_case("minc2_float.mnc", "MINC2")
    # Time points, scaled per (time, slice).
    d = values_for("uint16", (2,) + shape, 63)
    lo = np.arange(8, dtype=np.float64).reshape(2, 4) - 3.0
    write_minc2(os.path.join(OUT, "minc2_time.mnc"), ["time"] + zyx, d, [0, 65535], lo, lo + 50.0, 2)
    minc_case("minc2_time.mnc", "MINC2", per_slice=True)


def minc1_cases():
    path = os.path.join(OUT, "minc1_bytes.mnc")
    shape = (4, 5, 6)
    d = values_for("uint8", shape, 70)
    f = netcdf_file(path, "w")
    for n, size in zip(["zspace", "yspace", "xspace"], shape):
        f.createDimension(n, size)
        v = f.createVariable(n, "i", ())
        step, start, cos = GEOM[n]
        v.step = step
        v.start = start
        v.direction_cosines = np.array(cos, np.float64)
        v.spacing = b"regular__"
        v.units = b"mm"
        v.varid = b"MINC standard variable"
    im = f.createVariable("image", "b", ("zspace", "yspace", "xspace"))
    im.signtype = b"unsigned"
    im.valid_range = np.array([0.0, 255.0])
    im[:] = d.astype(np.int8)
    lo = f.createVariable("image-min", "d", ("zspace",))
    hi = f.createVariable("image-max", "d", ("zspace",))
    lo[:] = np.array([0.0, -1.0, -2.0, -3.0])
    hi[:] = np.array([1.0, 2.0, 4.0, 8.0])
    f.close()
    minc_case("minc1_bytes.mnc", "MINC1", per_slice=True)


def main():
    os.makedirs(OUT, exist_ok=True)
    mgh_cases()
    minc2_cases()
    minc1_cases()
    with open(os.path.join(OUT, "expected-mgh-minc.tsv"), "w") as f:
        f.write("# Written by gen/make_mgh_minc.py (nibabel %s, h5py %s); do not edit.\n"
                % (nib.__version__, h5py.__version__))
        f.write("\n".join(rows) + "\n")
    print(f"{len(rows)} rows", file=sys.stderr)


if __name__ == "__main__":
    main()
