"""WSQ oracle for tools/corpus/fetch-raster-wsq.sh.

Usage: python -I raster-wsq-oracle.py OUT

OUT/files/*.wsq   downloaded samples (already there) plus files encoded here with NBIS
                  (the `wsq` Pillow plugin: NIST's encoder/decoder in a Python extension)
OUT/expect/*.pgm  NBIS's decoding of every file (binary PGM), the oracle

Encoded files: ImageMagick's built-in pictures and the downloaded fingerprints, turned grey,
cropped to odd and even sizes and encoded at several bit rates.
"""

import os
import subprocess
import sys

import ctypes

import _wsq
import numpy as np
import wsq  # noqa: F401  (registers the WSQ plugin with Pillow)
from PIL import Image

# The plugin always encodes at 0.75 bits per pixel; NBIS's encoder itself takes the rate.
_lib = ctypes.CDLL(_wsq.__file__)
_lib.wsq_encode_mem.argtypes = [
    ctypes.POINTER(ctypes.POINTER(ctypes.c_ubyte)), ctypes.POINTER(ctypes.c_int), ctypes.c_float,
    ctypes.c_char_p, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_char_p]


def encode(a, rate, dst):
    h, w = a.shape
    out = ctypes.POINTER(ctypes.c_ubyte)()
    n = ctypes.c_int()
    r = _lib.wsq_encode_mem(ctypes.byref(out), ctypes.byref(n), rate, a.tobytes(), w, h, 8, 500, None)
    if r != 0:
        raise RuntimeError("wsq_encode_mem returned %d" % r)
    with open(dst, "wb") as f:
        f.write(ctypes.string_at(out, n.value))

out = sys.argv[1]
files = os.path.join(out, "files")
expect = os.path.join(out, "expect")
os.makedirs(expect, exist_ok=True)

# Sources: magick built-ins, grey.
src_dir = os.path.join(out, "src")
os.makedirs(src_dir, exist_ok=True)
sources = {}
for name in ["rose", "logo", "wizard", "granite", "netscape"]:
    p = os.path.join(src_dir, name + ".pgm")
    if not os.path.exists(p):
        r = subprocess.run(["magick", name + ":", "-colorspace", "Gray", "-depth", "8", p])
        if r.returncode != 0:
            continue
    sources[name] = np.array(Image.open(p).convert("L"))
# The downloaded fingerprints, decoded by NBIS, as sources too.
for f in sorted(os.listdir(files)):
    if f.endswith(".wsq") and not f.startswith("gen-"):
        sources["fp-" + f[:-4]] = np.array(Image.open(os.path.join(files, f)))

sizes = [None, (257, 301), (300, 256), (513, 511), (77, 100), (64, 64), (1001, 39)]
rates = [0.75, 2.25, 0.3, 4.0]
n = 0
for name, a in sources.items():
    for si, size in enumerate(sizes):
        if size is None:
            b = a
        else:
            w, h = size
            b = np.array(Image.fromarray(a).resize((w, h), Image.BILINEAR))
        rate = rates[(si + n) % len(rates)]
        n += 1
        dst = os.path.join(files, "gen-%s-%dx%d-r%s.wsq" % (name, b.shape[1], b.shape[0], rate))
        if not os.path.exists(dst):
            try:
                encode(np.ascontiguousarray(b), rate, dst)
            except Exception as e:  # tiny images the encoder refuses
                print("skip", dst, e)
                continue

ok = 0
for f in sorted(os.listdir(files)):
    if not f.endswith(".wsq"):
        continue
    try:
        im = Image.open(os.path.join(files, f))
        im.load()
    except Exception as e:
        print("oracle cannot decode", f, e)
        continue
    im.save(os.path.join(expect, f[:-4] + ".pgm"))
    ok += 1
print("oracle decoded", ok, "files")
