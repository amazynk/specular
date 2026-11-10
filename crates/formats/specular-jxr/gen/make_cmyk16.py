#!/usr/bin/env python3
"""Generates the 64bppCMYK / 80bppCMYKAlpha test vectors in ../tests/vectors (cmyk16-*.jxr,
cmyk16.txt) with jxrlib as the oracle.

JxrEncApp cannot read 16-bit CMYK TIFFs (exit 150/151), so the files are written by imagecodecs'
jpegxr_encode (jxrlib's encoder library); JxrDecApp decodes them to the expected native samples.
imagecodecs is only a test tool (pip, in a venv).

Usage: python make_cmyk16.py OUT_DIR WORK_DIR
"""

import os
import sys

import imagecodecs
import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import oracle  # noqa: E402
import tiffraw  # noqa: E402
from make_vectors import fnv  # noqa: E402

H, W = 45, 67

# (name, channels, level: None = lossless)
CASES = [
    ("cmyk16-lossless", 4, None),
    ("cmyk16-q", 4, 0.6),
    ("cmyka16-lossless", 5, None),
    ("cmyka16-q", 5, 0.6),
]


def main():
    out, work = sys.argv[1], sys.argv[2]
    os.makedirs(out, exist_ok=True)
    os.makedirs(work, exist_ok=True)
    lines = ["# name width height channels fnv1a64(native samples, little-endian) - make_cmyk16.py"]
    for name, c, level in CASES:
        a = (oracle.content(H, W, c, H * 7 + W + c) * 65536).astype(np.uint16)
        jxr = os.path.join(out, name + ".jxr")
        open(jxr, "wb").write(imagecodecs.jpegxr_encode(a, level=level, photometric="CMYK", hasalpha=c == 5))
        tif = os.path.join(work, name + ".tif")
        if os.path.exists(tif):
            os.remove(tif)
        r = oracle.run(["JxrDecApp", "-i", jxr, "-o", tif])
        if r.returncode != 0 or not os.path.exists(tif):
            sys.exit(f"{name}: oracle decoder failed")
        _, raw = tiffraw.read(tif)
        assert len(raw) == H * W * c * 2, name
        lines.append(f"{name} {W} {H} {c} {fnv(raw):016x}")
        print(lines[-1])
    with open(os.path.join(out, "cmyk16.txt"), "w") as f:
        f.write("\n".join(lines) + "\n")


if __name__ == "__main__":
    main()
