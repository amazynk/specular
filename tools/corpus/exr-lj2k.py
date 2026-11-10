#!/usr/bin/env python3
"""LJ2K-compressed OpenEXR files (compression 12, lossy HTJ2K) with an uncompressed twin
holding the source pixels, for specular-exr's LJ2K tests.

Usage: exr-lj2k.py OUT_DIR [--small]

Needs the OpenEXR Python bindings and numpy, e.g. in a venv: `pip install OpenEXR numpy`.
OpenEXR 3.5.2 and later write the current form (header magic `HL`, the transfer function as
an NLT type 4 table in the codestream); 3.5.0 / 3.5.1 the legacy one (magic `HT`, transfer
function applied before coding): run this once with each to get both. Each case writes
`<name>.lj2k.exr` (LJ2K), `<name>.raw.exr` (no compression, the source pixels) and
`<name>.ref.exr` (OpenEXR's own decode of the LJ2K file, stored uncompressed): a decoder must
match `ref` within a small error and stay close to `raw`. Files are tagged with the OpenEXR
version that wrote them (`<name>-<version>.*`).
`--small` writes only the small cases kept in crates/formats/specular-exr/tests/data/lj2k.
"""

import os
import sys

import numpy as np
import OpenEXR


def smooth(rng, h, w, scale, noise, dtype):
    y, x = np.mgrid[0:h, 0:w]
    v = np.sin(x / 7.0) * np.cos(y / 5.0) * scale + (x - y) * scale / 40.0 + scale
    v = v + rng.standard_normal((h, w)) * noise
    return v.astype(dtype)


def write(out, name, chans, tiles=None):
    tag = f"{name}-{OpenEXR.__version__}"
    for suffix, comp in (("lj2k", OpenEXR.LJ2K_COMPRESSION), ("raw", OpenEXR.NO_COMPRESSION)):
        hdr = {"compression": comp, "type": OpenEXR.scanlineimage}
        if tiles:
            hdr["type"] = OpenEXR.tiledimage
            td = OpenEXR.TileDescription()
            td.xSize, td.ySize = tiles
            td.mode, td.roundingMode = OpenEXR.ONE_LEVEL, OpenEXR.ROUND_DOWN
            hdr["tiles"] = td
        OpenEXR.File(hdr, chans).write(os.path.join(out, f"{tag}.{suffix}.exr"))
    back = OpenEXR.File(os.path.join(out, f"{tag}.lj2k.exr"))
    dec = {k: v.pixels for k, v in back.channels().items()}
    hdr = {"compression": OpenEXR.NO_COMPRESSION, "type": OpenEXR.scanlineimage}
    OpenEXR.File(hdr, dec).write(os.path.join(out, f"{tag}.ref.exr"))


def main():
    out = sys.argv[1]
    small = "--small" in sys.argv
    os.makedirs(out, exist_ok=True)
    rng = np.random.default_rng(2026)
    f16, f32 = np.float16, np.float32

    # RGB lossy (ICT, NLT type 4 on half), A lossless.
    write(out, "rgba_half", {
        "R": smooth(rng, 37, 45, 3.0, 0.01, f16),
        "G": smooth(rng, 37, 45, 1.0, 0.002, f16),
        "B": smooth(rng, 37, 45, 2.0, 0.0, f16),
        "A": np.clip(smooth(rng, 37, 45, 0.5, 0.0, f16), 0, 1).astype(f16),
    })
    # Float RGB: the 32-bit table.
    write(out, "rgb_float", {
        "R": smooth(rng, 29, 33, 100.0, 0.05, f32),
        "G": smooth(rng, 29, 33, 10.0, 0.0, f32),
        "B": smooth(rng, 29, 33, 0.5, 0.0, f32),
    })
    if small:
        return
    write(out, "rgb_half_tiled", {
        "R": smooth(rng, 40, 50, 5.0, 0.0, f16),
        "G": smooth(rng, 40, 50, 0.5, 0.0, f16),
        "B": smooth(rng, 40, 50, 3.0, 0.0, f16),
    }, tiles=(32, 16))
    write(out, "rgb_half_big", {
        "R": smooth(rng, 517, 731, 3.0, 0.01, f16),
        "G": smooth(rng, 517, 731, 1.0, 0.002, f16),
        "B": smooth(rng, 517, 731, 2.0, 0.0, f16),
        "Z": smooth(rng, 517, 731, 1000.0, 0.0, f32),
    })


if __name__ == "__main__":
    main()
