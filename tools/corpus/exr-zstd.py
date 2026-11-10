#!/usr/bin/env python3
"""ZSTD-compressed OpenEXR files (compression 13, OpenEXR 3.5) with an uncompressed twin, for
specular-exr's ZSTD tests.

Usage: exr-zstd.py OUT_DIR [--small]

Needs the OpenEXR Python bindings (3.5 or later, which write ZSTD_COMPRESSION) and numpy,
e.g. in a venv: `pip install OpenEXR numpy`. Each case writes `<name>.zstd.exr` and
`<name>.raw.exr` (no compression, same pixels): a decoder must give both the same samples.
`--small` writes only the small cases kept in crates/formats/specular-exr/tests/data/zstd.
"""

import os
import sys

import numpy as np
import OpenEXR

ZSTD = OpenEXR.ZSTD_COMPRESSION


def smooth(rng, h, w, scale, noise, dtype):
    y, x = np.mgrid[0:h, 0:w]
    v = np.sin(x / 7.0) * np.cos(y / 5.0) * scale + (x - y) * scale / 40.0
    v = v + rng.standard_normal((h, w)) * noise
    return v.astype(dtype)


def with_specials(v):
    """`v` with signed zeros, denormals, infinities and NaN in its first row."""
    v = v.copy()
    big = 65504 if v.dtype == np.float16 else 3.4e38
    tiny = 6e-8 if v.dtype == np.float16 else 1e-45
    v.flat[:8] = np.array([0.0, -0.0, tiny, -tiny, np.inf, -np.inf, np.nan, big], dtype=v.dtype)
    return v


def write(out, name, chans, tiles=None):
    for suffix, comp in (("zstd", ZSTD), ("raw", OpenEXR.NO_COMPRESSION)):
        hdr = {"compression": comp, "type": OpenEXR.scanlineimage}
        if tiles:
            hdr["type"] = OpenEXR.tiledimage
            td = OpenEXR.TileDescription()
            td.xSize, td.ySize = tiles
            td.mode, td.roundingMode = OpenEXR.ONE_LEVEL, OpenEXR.ROUND_DOWN
            hdr["tiles"] = td
        OpenEXR.File(hdr, chans).write(os.path.join(out, f"{name}.{suffix}.exr"))


def write_deep(out, name, h, w, tiles=None):
    """Deep Z (float) and A (half) samples, 0..3 per pixel, smooth enough to compress."""
    z = np.empty((h, w), dtype=object)
    a = np.empty((h, w), dtype=object)
    for y in range(h):
        for x in range(w):
            n = (x // 3 + y // 2) % 4
            z[y, x] = (np.arange(n, dtype=np.float32) + (x + y) / 8.0).astype(np.float32)
            a[y, x] = np.full(n, 0.25 + (x % 4) / 8.0, dtype=np.float16)
    for suffix, comp in (("zstd", ZSTD), ("raw", OpenEXR.NO_COMPRESSION)):
        hdr = {"compression": comp, "type": OpenEXR.deepscanline}
        if tiles:
            hdr["type"] = OpenEXR.deeptile
            td = OpenEXR.TileDescription()
            td.xSize, td.ySize = tiles
            td.mode, td.roundingMode = OpenEXR.ONE_LEVEL, OpenEXR.ROUND_DOWN
            hdr["tiles"] = td
        OpenEXR.File(hdr, {"Z": z, "A": a}).write(os.path.join(out, f"{name}.{suffix}.exr"))


def main():
    out = sys.argv[1]
    small = "--small" in sys.argv
    os.makedirs(out, exist_ok=True)
    rng = np.random.default_rng(2026)
    f16, f32 = np.float16, np.float32

    # Half and float channels in one chunk: two segments (16-bit rows, then 32-bit rows).
    write(out, "rgba_half_z_float", {
        "R": smooth(rng, 23, 31, 3.0, 0.01, f16),
        "G": smooth(rng, 23, 31, 1.0, 0.002, f16),
        "B": with_specials(smooth(rng, 23, 31, -2.0, 0.0, f16)),
        "A": np.clip(smooth(rng, 23, 31, 1.0, 0.0, f16), 0, 1).astype(f16),
        "Z": with_specials(smooth(rng, 23, 31, 1000.0, 0.001, f32)),
    })
    # Only 32-bit samples: one segment of element size 4.
    write(out, "float_uint", {
        "Z": smooth(rng, 17, 40, 100.0, 0.5, f32),
        "id": (np.arange(17 * 40, dtype=np.uint32).reshape(17, 40) * 7919 % 100003),
    })
    write(out, "rgb_half_tiled", {
        "R": smooth(rng, 40, 50, 5.0, 0.0, f16),
        "G": smooth(rng, 40, 50, 0.5, 0.0, f16),
        "B": with_specials(smooth(rng, 40, 50, -3.0, 0.0, f16)),
    }, tiles=(16, 24))
    # Incompressible data: OpenEXR stores such chunks raw.
    write(out, "noise_float", {
        "R": rng.integers(0, 2**32, (9, 64), dtype=np.uint32).view(f32),
    })
    write_deep(out, "deep_scanline", 30, 41)
    if small:
        return
    write_deep(out, "deep_tiled", 70, 90, tiles=(32, 16))
    write(out, "rgb_half_big", {
        "R": smooth(rng, 517, 731, 3.0, 0.01, f16),
        "G": smooth(rng, 517, 731, 1.0, 0.002, f16),
        "B": smooth(rng, 517, 731, -2.0, 0.0, f16),
    })
    write(out, "rgba_float_tiled_big", {
        "R": smooth(rng, 300, 333, 1000.0, 0.1, f32),
        "G": smooth(rng, 300, 333, 900.0, 0.0, f32),
        "B": with_specials(smooth(rng, 300, 333, 800.0, 0.0, f32)),
        "A": np.ones((300, 333), f32),
    }, tiles=(64, 32))


if __name__ == "__main__":
    main()
