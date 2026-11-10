#!/usr/bin/env python3
"""Generates the JPEG XR test vectors in ../tests/vectors with jxrlib as the oracle.

jxrlib is only a test oracle: JxrEncApp encodes small synthetic images (the inputs and options
of oracle.py) and JxrDecApp decodes them again for the expected native samples. Cases where
JxrDecApp is known to be wrong (see oracle.py) are not vectors.

Usage: python make_vectors.py OUT_DIR WORK_DIR
  OUT_DIR   receives NAME.jxr and manifest.txt (name, width, height, bytes per pixel, FNV-1a 64 hash of
            the native samples as little-endian bytes, BlackWhite as one byte per pixel).
  WORK_DIR  scratch directory for the TIFF inputs and outputs.
"""

import os
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import oracle  # noqa: E402
import tiffraw  # noqa: E402

SMALL = (19, 37)
TILED = (50, 70)

# (format, mode, alpha option: None, "a2" planar or "a3" interleaved)
CASES = [(("gray8", m, None)) for m in [
    "lossless", "q", "q20", "l0", "l2", "spatial", "seq", "tiles", "tilesvh", "tilessp", "trim",
    "skiphp", "skipflex", "dconly",
]] + [("rgb24", m, None) for m in [
    "lossless", "q", "l0", "l2", "spatial", "trimsp", "d420", "d422", "d420l2", "d422l2", "d0",
    "tiles420", "tiles422",
]] + [
    ("bgr24", "q", None),
    ("bw1", "lossless", None),
    ("gray16", "q", None),
    ("gray16fix", "q", None),
    ("gray16half", "q", None),
    ("gray32fix", "q", None),
    ("gray32f", "q", None),
    ("rgb48", "q", None),
    ("rgb48fix", "q", None),
    ("rgb48half", "lossless", None),
    ("rgb96fix", "q", None),
    ("rgb128f", "q", None),
    ("rgbe", "q", None),
    ("cmyk32", "lossless", None),
    ("cmyk32", "skiphp", None),
    ("bgra32", "q", "a2"),
    ("bgra32", "q", "a3"),
    ("bgra32", "tiles", "a3"),
    ("rgba64", "q", "a3"),
    ("rgba64fix", "q", "a3"),
    ("rgba64half", "q", "a2"),
    ("rgba128fix", "q", "a2"),
    ("rgba128f", "q", "a3"),
    ("bgr555", "q", None),
    ("bgr565", "q", None),
    ("bgr101010", "q", None),
    ("cmyka40", "lossless", "a2"),
    ("cmyka40", "lossless", "a3"),
    ("bgr32", "q", None),
]


def fnv(data):
    h = 0xCBF29CE484222325
    for b in data:
        h = ((h ^ b) * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return h


def main():
    out, work = sys.argv[1], sys.argv[2]
    os.makedirs(out, exist_ok=True)
    os.makedirs(work, exist_ok=True)
    fmts = {name: (fmt, kind) for name, fmt, kind in oracle.FORMATS}
    modes = {name: opts for name, opts, _ in oracle.MODES}
    lines = ["# name width height bytes-per-pixel fnv1a64(native samples, little-endian) - make_vectors.py"]
    for fname, mname, alpha in CASES:
        fmt, kind = fmts[fname]
        opts = modes[mname]
        h, w = TILED if mname.startswith("tiles") else SMALL
        name = f"{fname}-{mname}" + (f"-{alpha}" if alpha else "")
        src = os.path.join(work, f"in-{kind}-{h}x{w}.tif")
        if not os.path.exists(src):
            oracle.inputs(src, kind, h, w, h * 7 + w)
        aopt = {None: [], "a2": ["-a", "2"], "a3": ["-a", "3", "-Q", "4"]}[alpha]
        strip = oracle.STRIP_ALPHA.get(fmt)
        efmt = strip[0] if strip else fmt
        jxr = os.path.join(out, name + ".jxr")
        r = oracle.run(["JxrEncApp", "-i", src, "-o", jxr, "-c", str(efmt)] + opts + (["-a", "2"] if strip else aopt))
        if r.returncode != 0:
            sys.exit(f"{name}: encoder failed")
        if strip:
            oracle.strip_alpha(jxr, strip[1])
        tif = os.path.join(work, name + ".tif")
        if os.path.exists(tif):
            os.remove(tif)
        r = oracle.run(["JxrDecApp", "-i", jxr, "-o", tif])
        if r.returncode != 0 or not os.path.exists(tif):
            sys.exit(f"{name}: oracle decoder failed")
        _, raw = tiffraw.read(tif)
        if fmt == 1:
            raw = np.unpackbits(np.frombuffer(raw, np.uint8).reshape(h, -1), axis=1)[:, :w].tobytes()
        assert len(raw) % (w * h) == 0, name
        lines.append(f"{name} {w} {h} {len(raw) // (w * h)} {fnv(raw):016x}")
        print(lines[-1])
    with open(os.path.join(out, "manifest.txt"), "w") as f:
        f.write("\n".join(lines) + "\n")


if __name__ == "__main__":
    main()
