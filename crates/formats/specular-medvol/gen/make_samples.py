#!/usr/bin/env python3
"""Writes the small NIfTI / Analyze / NRRD / MetaImage samples in ../tests/data and their
oracle, ../tests/data/expected.tsv, with nibabel (NIfTI, Analyze), pynrrd (NRRD values) and
SimpleITK (NRRD and MetaImage geometry and values).

The oracle is only for tests: nothing here is used at run time. Run it with -I from a venv
that has nibabel, SimpleITK, pynrrd and numpy:

    python3 -I crates/formats/specular-medvol/gen/make_samples.py

expected.tsv lines (tab-separated):
    file    <name>  <kind>  <dtype>  <orientation>  <volumes>
    geom    <name>  i j k   x y z           stored voxel -> patient LPS mm
    val     <name>  i j k v c  value        scaled value of stored voxel, volume, component
    error   <name>  <words the reason contains>
"""

import math
import os
import shutil
import sys

import nibabel as nib
import nrrd
import numpy as np
import SimpleITK as sitk

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "..", "tests", "data")

rows = []


def rng(seed):
    return np.random.default_rng(seed)


def values_for(dtype, shape, seed):
    r = rng(seed)
    dt = np.dtype(dtype)
    if dt.kind == "f":
        return (r.standard_normal(shape) * 1000).astype(dt)
    info = np.iinfo(dt)
    lo, hi = max(info.min, -(2**40)), min(info.max, 2**40)
    a = r.integers(lo, hi, size=shape, dtype=np.int64 if dt.kind == "i" or dt.itemsize < 8 else np.uint64)
    a = a.astype(dt)
    # The extremes appear too.
    flat = a.reshape(-1)
    flat[0] = info.min
    flat[-1] = info.max if dt.itemsize < 8 else min(info.max, 2**53)
    return a


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


def vals(name, data, comps=False):
    """data indexed [i, j, k, (volume...)] (and a last component axis when comps)."""
    d = np.asarray(data)
    if not comps:
        d = d[..., np.newaxis]
    nc = d.shape[-1]
    sp = d.shape[:-1]
    sp3 = tuple(list(sp[:3]) + [1] * (3 - len(sp[:3])))
    nv = int(np.prod(sp[3:])) if len(sp) > 3 else 1
    d = d.reshape(sp3 + (nv, nc), order="F")
    for v in range(nv):
        for k in range(sp3[2]):
            for j in range(sp3[1]):
                for i in range(sp3[0]):
                    for c in range(nc):
                        rows.append(f"val\t{name}\t{i}\t{j}\t{k}\t{v}\t{c}\t{fmt(d[i, j, k, v, c])}")


RAS_TO_LPS = np.diag([-1.0, -1.0, 1.0, 1.0])


def nib_case(name, img, kind, orient, comps=False, data=None):
    path = os.path.join(OUT, name)
    nib.save(img, path)
    back = nib.load(path)
    aff = RAS_TO_LPS @ back.affine
    shape = back.shape
    if comps:
        raw = np.asanyarray(back.dataobj)
        names = raw.dtype.names
        d = np.stack([raw[n] for n in names], axis=-1)
        vols = 1
        dt = "uint8"
    else:
        d = back.get_fdata(dtype=np.float64)
        vols = int(np.prod(shape[3:])) if len(shape) > 3 else 1
        dt = back.get_data_dtype().name
    record(name, kind, dt, orient, vols)
    geom(name, aff[:3, :], shape)
    vals(name, d, comps)


def oblique_affine(zooms=(1.5, 2.0, 2.5), angle=0.3, shift=(10.0, -20.0, 30.0)):
    c, s = math.cos(angle), math.sin(angle)
    rot = np.array([[c, -s, 0], [s, c, 0], [0, 0, 1.0]]) @ np.array(
        [[1, 0, 0], [0, math.cos(angle / 2), -math.sin(angle / 2)], [0, math.sin(angle / 2), math.cos(angle / 2)]]
    )
    a = np.eye(4)
    a[:3, :3] = rot @ np.diag(zooms)
    a[:3, 3] = shift
    return a


def nifti_cases():
    shape = (5, 4, 3)
    # Every datatype, with an oblique sform.
    for i, dt in enumerate(["uint8", "int8", "int16", "uint16", "int32", "uint32", "int64", "uint64", "float32", "float64"]):
        img = nib.Nifti1Image(values_for(dt, shape, i), oblique_affine(), dtype=dt)
        img.header.set_data_dtype(dt)
        img.set_sform(oblique_affine(), code=2)
        img.set_qform(None, code=0)
        nib_case(f"dt_{dt}.nii", img, "NIfTI-1", "sform")

    # qform only, left-handed (qfac -1), rotated.
    a = oblique_affine(angle=0.7)
    a[:3, 2] *= -1
    img = nib.Nifti1Image(values_for("int16", shape, 20), None)
    img.set_qform(a, code=1)
    img.set_sform(None, code=0)
    nib_case("qform_only.nii", img, "NIfTI-1", "qform")

    # Both: the sform wins.
    img = nib.Nifti1Image(values_for("int16", shape, 21), None)
    img.set_qform(oblique_affine(angle=0.2), code=1)
    img.set_sform(oblique_affine(angle=-0.4, shift=(1, 2, 3)), code=1)
    nib_case("sform_and_qform.nii", img, "NIfTI-1", "sform")

    # sform code 0 with a stale sform: the qform is used.
    img = nib.Nifti1Image(values_for("int16", shape, 22), None)
    img.set_qform(oblique_affine(angle=0.5), code=1)
    img.set_sform(oblique_affine(angle=-0.9), code=0)
    nib_case("stale_sform.nii", img, "NIfTI-1", "qform")

    # Neither form.
    img = nib.Nifti1Image(values_for("uint8", shape, 23), None)
    img.header.set_zooms((0.5, 0.75, 3.0))
    img.set_qform(None, code=0)
    img.set_sform(None, code=0)
    nib_case("no_form.nii", img, "NIfTI-1", "none")

    # Scaling and a display range.
    img = nib.Nifti1Image(values_for("int16", shape, 24), np.diag([2.0, 2.0, 2.0, 1.0]))
    img.header.set_data_dtype("int16")
    img.header.set_slope_inter(0.25, -100.0)
    img.header["cal_min"] = -50.0
    img.header["cal_max"] = 150.0
    img.header["descrip"] = b"scaled sample"
    img.header.set_intent("z score", name="activation")
    nib_case("scaled.nii", img, "NIfTI-1", "sform")

    # Time series.
    img = nib.Nifti1Image(values_for("float32", shape + (2,), 25), oblique_affine())
    nib_case("time.nii", img, "NIfTI-1", "sform")

    # RGB.
    rgb = values_for("uint8", shape + (3,), 26)
    st = np.zeros(shape, dtype=[("R", "u1"), ("G", "u1"), ("B", "u1")])
    st["R"], st["G"], st["B"] = rgb[..., 0], rgb[..., 1], rgb[..., 2]
    img = nib.Nifti1Image(st, np.diag([1.0, 1.0, 1.0, 1.0]))
    nib_case("rgb.nii", img, "NIfTI-1", "sform", comps=True)

    # Big endian.
    d = values_for("int16", shape, 27).astype(">i2")
    hdr = nib.Nifti1Header(endianness=">")
    img = nib.Nifti1Image(d, oblique_affine(), header=hdr)
    nib_case("big_endian.nii", img, "NIfTI-1", "sform")

    # gzip.
    img = nib.Nifti1Image(values_for("int16", shape, 28), oblique_affine())
    nib_case("gzipped.nii.gz", img, "NIfTI-1", "sform")

    # An extension before the voxels.
    img = nib.Nifti1Image(values_for("float32", shape, 29), oblique_affine())
    img.header.extensions.append(nib.nifti1.Nifti1Extension("comment", b"x" * 100))
    nib_case("extension.nii", img, "NIfTI-1", "sform")

    # NIfTI-2.
    img = nib.Nifti2Image(values_for("float64", shape, 30), oblique_affine(angle=1.1))
    nib_case("nifti2.nii", img, "NIfTI-2", "sform")

    # A .hdr / .img pair.
    img = nib.Nifti1Pair(values_for("uint16", shape, 31), oblique_affine())
    nib_case("pair.hdr", img, "NIfTI-1", "sform")

    # A single coronal slice (its normal along y).
    a = np.array([[1.0, 0, 0, 0], [0, 0, 1.0, 0], [0, 1.0, 0, 0], [0, 0, 0, 1]])
    img = nib.Nifti1Image(values_for("int16", (5, 4, 1), 32), a)
    nib_case("coronal_slice.nii", img, "NIfTI-1", "sform")

    # A sagittal stack stored as (y, z, x).
    a = np.array([[0, 0, 1.2, -5], [1.0, 0, 0, 3], [0, 1.1, 0, 7], [0, 0, 0, 1]])
    img = nib.Nifti1Image(values_for("int16", shape, 33), a)
    nib_case("sagittal.nii", img, "NIfTI-1", "sform")

    # Analyze 7.5 (SPM: origin and scale in the old fields).
    # (Written plain, then SPM's origin and scale patched in: saving through Spm2AnalyzeImage
    # needs scipy for its .mat file.)
    img = nib.AnalyzeImage(values_for("int16", shape, 34), np.diag([-2.0, 2.0, 3.0, 1.0]))
    nib.save(img, os.path.join(OUT, "analyze.hdr"))
    with open(os.path.join(OUT, "analyze.hdr"), "r+b") as f:
        hdr = bytearray(f.read())
        hdr[112:116] = np.array([0.5], "<f4").tobytes()
        hdr[253:259] = np.array([2, 3, 2], "<i2").tobytes()
        f.seek(0)
        f.write(hdr)
    back = nib.load(os.path.join(OUT, "analyze.hdr"))
    assert isinstance(back, nib.Spm2AnalyzeImage), type(back)
    record("analyze.hdr", "Analyze 7.5", "int16", "analyze", 1)
    geom("analyze.hdr", (RAS_TO_LPS @ back.affine)[:3, :], back.shape)
    vals("analyze.hdr", back.get_fdata(dtype=np.float64))

    # Complex: refused.
    img = nib.Nifti1Image((values_for("float32", shape, 35) + 1j).astype(np.complex64), np.eye(4))
    nib.save(img, os.path.join(OUT, "complex.nii"))
    rows.append("error\tcomplex.nii\tcomplex")


def sitk_geom(name, img):
    size = img.GetSize()
    nx, ny, nz = (list(size) + [1, 1, 1])[:3]
    pts = [(i, j, k) for i in (0, nx - 1) for j in (0, ny - 1) for k in (0, nz - 1)]
    pts.append((min(1, nx - 1), min(1, ny - 1), min(1, nz - 1)))
    for p in sorted(set(pts)):
        x = img.TransformContinuousIndexToPhysicalPoint([float(c) for c in p[: img.GetDimension()]])
        rows.append(f"geom\t{name}\t{p[0]}\t{p[1]}\t{p[2]}\t{fmt(x[0])}\t{fmt(x[1])}\t{fmt(x[2])}")


def sitk_vals(name, img):
    a = sitk.GetArrayFromImage(img)  # [k, j, i(, c)]
    comps = img.GetNumberOfComponentsPerPixel() > 1
    if comps:
        a = np.transpose(a, (2, 1, 0, 3))
    else:
        a = np.transpose(a)
    vals(name, a.astype(np.float64), comps)


def nrrd_cases():
    shape = (5, 4, 3)
    dirs = oblique_affine(angle=0.6)[:3, :3]
    base = {
        "space": "right-anterior-superior",
        "space directions": dirs.T,  # rows: axis vectors
        "space origin": np.array([10.0, -20.0, 30.0]),
    }
    for enc in ["raw", "gzip", "bzip2", "ascii"]:
        name = f"enc_{enc}.nrrd"
        d = values_for("int16", shape, 40)
        nrrd.write(os.path.join(OUT, name), d, dict(base, encoding=enc), index_order="F")
        sitk_case(name, "int16", "space")
    # Big endian float.
    name = "big_float.nrrd"
    d = values_for("float32", shape, 41)
    nrrd.write(os.path.join(OUT, name), d, dict(base, encoding="raw", endian="big"), index_order="F")
    sitk_case(name, "float32", "space")
    # LPS, detached.
    name = "detached.nhdr"
    d = values_for("uint16", shape, 42)
    h = dict(base, space="left-posterior-superior", encoding="raw")
    nrrd.write(os.path.join(OUT, name), d, h, detached_header=True, index_order="F")
    sitk_case(name, "uint16", "space")
    # Spacings only, no space.
    name = "spacings.nrrd"
    d = values_for("int32", shape, 43)
    nrrd.write(os.path.join(OUT, name), d, {"spacings": [0.5, 0.7, 2.0], "encoding": "raw"}, index_order="F")
    sitk_case(name, "int32", "spacings")
    # Time series: axes x y z t.
    name = "time.nrrd"
    d = values_for("int16", shape + (2,), 44)
    h = {
        "space": "left-posterior-superior",
        "space directions": np.vstack([dirs.T, [np.nan] * 3]),
        "space origin": np.array([1.0, 2.0, 3.0]),
        "kinds": ["domain", "domain", "domain", "time"],
        "encoding": "gzip",
    }
    nrrd.write(os.path.join(OUT, name), d, h, index_order="F")
    back, _ = nrrd.read(os.path.join(OUT, name), index_order="F")
    record(name, "NRRD", "int16", "space", 2)
    lps = np.eye(4)[:3, :]
    lps[:, :3] = dirs
    lps[:, 3] = [1.0, 2.0, 3.0]
    geom(name, lps, shape)
    vals(name, back.astype(np.float64))
    # RGB colour (component axis first).
    name = "rgb.nrrd"
    d = values_for("uint8", (3,) + shape, 45)
    h = {
        "space": "left-posterior-superior",
        "space directions": np.vstack([[np.nan] * 3, np.eye(3)]),
        "kinds": ["RGB-color", "domain", "domain", "domain"],
        "encoding": "raw",
    }
    nrrd.write(os.path.join(OUT, name), d, h, index_order="F")
    record(name, "NRRD", "uint8", "space", 1)
    geom(name, np.hstack([np.eye(3), np.zeros((3, 1))]), shape)
    vals(name, np.moveaxis(d, 0, -1).astype(np.float64), comps=True)
    # bzip2 cut after its stream header: an error naming bzip2.
    with open(os.path.join(OUT, "bzip2.nrrd"), "wb") as f:
        f.write(b"NRRD0004\ntype: uchar\ndimension: 3\nsizes: 2 2 2\nencoding: bzip2\n\nBZh9")
    rows.append("error\tbzip2.nrrd\tbzip2")


def sitk_case(name, dt, orient, kind="NRRD"):
    img = sitk.ReadImage(os.path.join(OUT, name))
    record(name, kind, dt, orient, 1)
    sitk_geom(name, img)
    sitk_vals(name, img)


def meta_cases():
    shape = (5, 4, 3)  # x, y, z
    dirs = oblique_affine(angle=0.8)[:3, :3]
    dirs = dirs / np.linalg.norm(dirs, axis=0)
    for name, dt, compress in [
        ("short.mha", "int16", False),
        ("zlib.mha", "float32", True),
        ("meta_detached.mhd", "uint8", False),
        ("double_zlib.mhd", "float64", True),
    ]:
        arr = values_for(dt, shape, len(name)).transpose()  # [z, y, x]
        img = sitk.GetImageFromArray(np.ascontiguousarray(arr))
        img.SetSpacing((0.8, 1.2, 2.5))
        img.SetOrigin((-3.0, 4.0, 5.5))
        img.SetDirection(tuple(dirs.reshape(-1)))
        sitk.WriteImage(img, os.path.join(OUT, name), useCompression=compress)
        sitk_case(name, dt, "transform", kind="MetaImage")
    # Three channels.
    name = "rgb.mha"
    arr = values_for("uint8", (3, 4, 5, 3), 50)  # [z, y, x, c]
    img = sitk.GetImageFromArray(arr, isVector=True)
    img.SetSpacing((1.0, 1.0, 2.0))
    sitk.WriteImage(img, os.path.join(OUT, name))
    sitk_case(name, "uint8", "transform", kind="MetaImage")


def main():
    if os.path.isdir(OUT):
        shutil.rmtree(OUT)
    os.makedirs(OUT)
    nifti_cases()
    nrrd_cases()
    meta_cases()
    with open(os.path.join(OUT, "expected.tsv"), "w") as f:
        f.write("# Written by gen/make_samples.py (nibabel %s, SimpleITK %s, pynrrd %s); do not edit.\n"
                % (nib.__version__, sitk.Version_VersionString(), nrrd.__version__))
        f.write("\n".join(rows) + "\n")
    print(f"{len(rows)} rows", file=sys.stderr)


if __name__ == "__main__":
    main()
