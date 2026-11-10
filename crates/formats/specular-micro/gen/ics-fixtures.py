#!/usr/bin/env python3
"""Write the synthetic ICS fixtures of specular-micro's tests/data and their oracle values.

Usage: python -I ics-fixtures.py OUT_DIR  (needs numpy and diplib, whose libics writes the
version 1.0 / 2.0 and gzip files; the others are written here byte by byte).

Each fixture's planes are also read back with diplib (dip.ImageReadICS) and their sums
written to OUT_DIR/ics-expected.tsv: name, plane index, sum, min, max.
"""
import gzip
import os
import sys

import numpy as np
import diplib as dip

out = sys.argv[1]
os.makedirs(out, exist_ok=True)
rng = np.random.default_rng(7)


def header(lines):
    return ("\t\n" + "\n".join(lines) + "\n").encode()


def write_dip(name, arr, opts, scale=None):
    img = dip.Image(arr)
    if scale:
        img.SetPixelSize(dip.PixelSize([dip.PhysicalQuantity(s, "um") for s in scale]))
    dip.ImageWriteICS(img, os.path.join(out, name), [], 0, opts)


# 1. libics v1: 16-bit unsigned, x y z (dip's z), with pixel sizes (µm).
a = rng.integers(0, 4096, size=(6, 24, 32), dtype=np.uint16)  # z, y, x
write_dip("v1_u16_zyx", a, {"v1"}, [0.25, 0.25, 0.5])
# 2. libics v2 gzip, 32-bit float, 2-D.
b = (rng.standard_normal((20, 30)) * 100).astype(np.float32)
write_dip("v2_gzip_f32", b, {"v2", "gzip"}, [0.1, 0.1])
# 3. libics v2 uncompressed, 8-bit, 2 channels (tensor) x y.
c = rng.integers(0, 255, size=(16, 18, 2), dtype=np.uint8)
img = dip.Image(c, tensor_axis=2)
dip.ImageWriteICS(img, os.path.join(out, "v2_u8_2ch"), [], 0, {"v2"})

# 4. Hand-written: big-endian signed 16-bit, order bits ch x y t, inline (v2).
d = rng.integers(-3000, 3000, size=(2, 6, 10, 3), dtype=np.int16)  # t, y, x, ch
h = header([
    "ics_version\t2.0",
    "filename\tbe_s16_chxyt.ics",
    "layout\tparameters\t5",
    "layout\torder\tbits\tch\tx\ty\tt",
    "layout\tsizes\t16\t3\t10\t6\t2",
    "layout\tcoordinates\tvideo",
    "layout\tsignificant_bits\t16",
    "representation\tformat\tinteger",
    "representation\tsign\tsigned",
    "representation\tcompression\tuncompressed",
    "representation\tbyte_order\t2\t1",
    "parameter\torigin\t0\t0\t0\t0\t0",
    "parameter\tscale\t1\t1\t0.2\t0.2\t1.5",
    "parameter\tunits\trelative\tundefined\tmicrometers\tmicrometers\tseconds",
    "sensor\ts_params\tLambdaEm\t450\t520\t610",
    "history\tsoftware\tspecular ics-fixtures.py",
    "end",
])
open(os.path.join(out, "be_s16_chxyt.ics"), "wb").write(h + d.astype(">i2").tobytes())

# 5. Hand-written: complex64 (v1 with .ids), x y.
e = (rng.standard_normal((8, 9)) + 1j * rng.standard_normal((8, 9))).astype(np.complex64)
h = header([
    "ics_version\t1.0",
    "filename\tcomplex64",
    "layout\tparameters\t3",
    "layout\torder\tbits\tx\ty",
    "layout\tsizes\t64\t9\t8",
    "representation\tformat\tcomplex",
    "representation\tsign\tsigned",
    "representation\tcompression\tuncompressed",
    "representation\tbyte_order\t1\t2\t3\t4",
    "end",
])
open(os.path.join(out, "complex64.ics"), "wb").write(h)
open(os.path.join(out, "complex64.ids"), "wb").write(e.astype("<c8").tobytes())

# 6. Hand-written v2 with gzip and 64-bit unsigned, x y z.
f = rng.integers(0, 1 << 40, size=(2, 5, 7), dtype=np.uint64)
h = header([
    "ics_version\t2.0",
    "filename\tgz_u64.ics",
    "layout\tparameters\t4",
    "layout\torder\tbits\tx\ty\tz",
    "layout\tsizes\t64\t7\t5\t2",
    "representation\tformat\tinteger",
    "representation\tsign\tunsigned",
    "representation\tcompression\tgzip",
    "representation\tbyte_order\t1\t2\t3\t4\t5\t6\t7\t8",
    "end",
])
open(os.path.join(out, "gz_u64.ics"), "wb").write(h + gzip.compress(f.astype("<u8").tobytes(), mtime=0))

# Oracle: every plane through diplib.
rows = []
for name in sorted(os.listdir(out)):
    if not name.endswith(".ics"):
        continue
    img = dip.ImageReadICS(os.path.join(out, name))
    arr = np.asarray(img)
    rows.append((name, img.Sizes(), img.TensorElements(), arr))
with open(os.path.join(out, "ics-expected.tsv"), "w") as t:
    for name, sizes, tensor, arr in rows:
        # numpy view: (…, y, x[, tensor]); report the sum and extremes of every sample,
        # complex as the magnitude.
        v = np.abs(arr) if np.iscomplexobj(arr) else arr.astype(np.float64)
        t.write(f"{name}\t{'x'.join(map(str, sizes))}\t{tensor}\t{v.sum():.9g}\t{v.min():.9g}\t{v.max():.9g}\n")
print("wrote", len(rows), "fixtures to", out)
