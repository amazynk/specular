#!/usr/bin/env python3
"""Writes the ASDF fixtures in tests/data with asdf-python (and roman_datamodels for the
Roman-like tree); gen/oracle.py then records asdf-python's reading of them in
tests/data/expected.tsv. Run from a venv (asdf, numpy, lz4, astropy, roman_datamodels):

    python3 -I make_fixtures.py OUT_DIR

Each fixture stays well under 200 KB.
"""

import os
import sys

import asdf
import numpy as np

OUT = sys.argv[1]
os.makedirs(OUT, exist_ok=True)
rng = np.random.default_rng(20261012)

DTYPES = ["int8", "uint8", "int16", "uint16", "int32", "uint32", "int64", "uint64",
          "float16", "float32", "float64", "bool", "complex64", "complex128"]


def path(name):
    return os.path.join(OUT, name)


def values(dt, shape):
    n = int(np.prod(shape))
    d = np.dtype(dt)
    if d.kind == "b":
        a = rng.integers(0, 2, n).astype(bool)
    elif d.kind in "iu":
        info = np.iinfo(d)
        a = rng.integers(info.min, info.max, n, dtype=d, endpoint=True)
        a[:4] = [info.min, info.max, 0, 1]
    elif d.kind == "f":
        a = (rng.standard_normal(n) * 1000).astype(d)
        a[:4] = [np.nan, np.inf, -np.inf, 0.0]
        if d.itemsize >= 4:
            a[4] = np.finfo(np.float32).tiny / 4  # an f32 subnormal
    else:
        a = (rng.standard_normal(n) + 1j * rng.standard_normal(n)).astype(d)
    return a.reshape(shape)


def write(name, tree, **kw):
    af = asdf.AsdfFile(tree)
    af.write_to(path(name), **kw)
    return af


# Every datatype, in both byte orders.
for order, code in (("le", "<"), ("be", ">")):
    tree = {}
    for dt in DTYPES:
        a = values(dt, (5, 7))
        if a.dtype.itemsize > 1:
            a = a.astype(a.dtype.newbyteorder(code))
        tree[dt] = a
    write(f"dtypes_{order}.asdf", tree)

# Views of one base: strides, offsets, a transpose, steps, 1-D.
base = np.arange(24 * 30, dtype="<f4").reshape(24, 30)
write("strided.asdf", {
    "base": base,
    "rows": base[3:9],
    "window": base[2:20, 5:25],
    "step": base[::3, ::4],
    "transposed": base.T,
    "column": base[:, 7],
    "row": base[5],
})

# A view on its own (asdf decides whether to keep the base).
write("view_only.asdf", {"v": np.arange(400, dtype=">i2").reshape(20, 20)[4:16:2, 1::3]})

# Inline arrays.
af = asdf.AsdfFile({
    "ints": np.arange(12, dtype="int32").reshape(3, 4),
    "floats": np.array([[1.5, -2.25], [np.nan, 1e30]]),
    "flags": np.array([True, False, True]),
    "cube": np.arange(24, dtype="uint8").reshape(2, 3, 4),
})
for k in ("ints", "floats", "flags", "cube"):
    af.set_array_storage(af.tree[k], "inline")
af.write_to(path("inline.asdf"))

# Compressed blocks, with and without the block index.
img = (np.add.outer(np.arange(96), np.arange(128)) % 97).astype("<u2")
noise = values("float32", (64, 64))
for comp in ("zlib", "bzp2", "lz4"):
    write(f"compressed_{comp}.asdf", {"img": img, "noise": noise, "small": np.arange(5.0)},
          all_array_compression=comp)
write("no_index.asdf", {"img": img, "noise": noise}, include_block_index=False)
write("mixed.asdf", {"plain": img, "z": noise, "z_cube": values("int16", (3, 16, 16))},
      all_array_compression="zlib")

# Cubes: planes along the leading axes, and length-1 axes.
write("cubes.asdf", {
    "cube": values("float32", (4, 20, 30)),
    "hyper": values("int16", (2, 3, 8, 9)),
    "squeezable": values("float64", (20, 24, 1)),
    "one_row": values("uint8", (1, 40)),
    "vector": values("float64", (50,)),
})

# External (exploded) arrays in files beside the main one.
write("external.asdf", {"a": values("float32", (10, 12)), "b": values("int32", (6, 6)).astype(">i4")},
      all_array_storage="external")

# A streamed final block.
af = asdf.AsdfFile({"s": asdf.tags.core.Stream([6], np.float64)})
with open(path("streamed.asdf"), "wb") as fd:
    af.write_to(fd)
    for i in range(9):
        fd.write(np.arange(i * 6, i * 6 + 6, dtype="<f8").tobytes())

# A quantity, a masked array, structured and string data (refused), nested lists.
from astropy import units as u  # noqa: E402

write("odd.asdf", {
    "flux": values("float32", (8, 9)) * u.Unit("DN / s"),
    "masked": np.ma.masked_array(np.arange(20.0).reshape(4, 5), mask=np.arange(20) % 3 == 0),
    "table": np.zeros(4, dtype=[("a", "i4"), ("b", "f8")]),
    "text": np.array([b"abc", b"de"]),
    "nested": {"list": [np.arange(6, dtype="u1").reshape(2, 3), {"deep": np.ones((2, 2))}]},
    "meta": {"name": "odd 'one'", "n": 3, "x": 2.5, "ok": True, "none": None,
             "multi": "line one\nline two", "anchor": [1, 2, 3]},
})

# A Roman-like Level 2 image.
try:
    from roman_datamodels import datamodels as rdd

    m = rdd.ImageModel.create_fake_data(shape=(40, 48))
    m.save(path("roman_like.asdf"))
except Exception as e:  # roman_datamodels missing or changed: keep the rest
    print("roman_like skipped:", e, file=sys.stderr)

# A JWST-like FITS file: SCI, ERR and DQ image extensions and the metadata tree in an ASDF
# extension (a one-row binary table, column ASDF_METADATA of bytes), as stdatamodels writes.
import io  # noqa: E402

from astropy.io import fits  # noqa: E402

tree = {"meta": {"telescope": "JWST", "model_type": "ImageModel",
                 "instrument": {"name": "NIRCAM", "detector": "NRCA1", "filter": "F200W"},
                 "exposure": {"start_time": 60000.5, "effective_exposure_time": 42.5},
                 "target": {"catalog_name": "NGC 1234"},
                 "wcsinfo": {"ctype1": "RA---TAN", "ctype2": "DEC--TAN", "crval1": 150.0,
                             "crval2": 2.5, "crpix1": 8.5, "crpix2": 8.5,
                             "cdelt1": -1.7e-5, "cdelt2": 1.7e-5}}}
buf = io.BytesIO()
asdf.AsdfFile(tree).write_to(buf)
raw = np.frombuffer(buf.getvalue(), np.uint8)
hdus = fits.HDUList([
    fits.PrimaryHDU(header=fits.Header([("TELESCOP", "JWST"), ("INSTRUME", "NIRCAM")])),
    fits.ImageHDU(values("float32", (16, 16)), name="SCI"),
    fits.ImageHDU(values("float32", (16, 16)), name="ERR"),
    fits.ImageHDU((np.arange(256, dtype=np.uint32) % 7).reshape(16, 16), name="DQ"),
    fits.BinTableHDU.from_columns(
        [fits.Column(name="ASDF_METADATA", format=f"{raw.size}B", array=raw[None, :])],
        name="ASDF"),
])
hdus.writeto(path("jwst_like.fits"), overwrite=True)

for name in sorted(os.listdir(OUT)):
    if name.endswith((".asdf", ".fits")):
        size = os.path.getsize(path(name))
        print(f"{name}\t{size}")
        if size > 200_000:
            sys.exit(f"{name} is over 200 KB")
