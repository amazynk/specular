#!/usr/bin/env python3
"""HTJ2K-compressed OpenEXR files with an uncompressed twin, for specular-exr's HTJ2K tests.

Usage: exr-htj2k.py OUT_DIR [--small]

Needs the OpenEXR Python bindings (3.4 or later, which write HTJ2K256 / HTJ2K32) and numpy,
e.g. in a venv: `pip install OpenEXR numpy`. Each case writes `<name>.ht.exr` (HTJ2K) and
`<name>.raw.exr` (no compression, same pixels): a decoder must give both the same samples.
`--small` writes only the few small cases kept in crates/formats/specular-exr/tests/data/htj2k.
"""

import os
import sys

import numpy as np
import OpenEXR

HT256 = OpenEXR.HTJ2K256_COMPRESSION
HT32 = OpenEXR.HTJ2K32_COMPRESSION


def smooth(rng, h, w, scale, noise, dtype):
    y, x = np.mgrid[0:h, 0:w]
    v = np.sin(x / 7.0) * np.cos(y / 5.0) * scale + (x - y) * scale / 40.0
    v = v + rng.standard_normal((h, w)) * noise
    return v.astype(dtype)


def specials(h, w):
    """Halves with every awkward bit pattern: zeros of both signs, denormals, inf, NaN."""
    bits = np.arange(h * w, dtype=np.uint32) * 2654435761 % 65536
    v = bits.astype(np.uint16).view(np.float16).reshape(h, w).copy()
    v.flat[:8] = np.array([0.0, -0.0, 6e-8, -6e-8, np.inf, -np.inf, np.nan, 65504],
                          dtype=np.float16)
    return v


def with_specials(v):
    """`v` with signed zeros, a denormal pair, infinities and NaN in its first row."""
    v = v.copy()
    v.flat[:8] = np.array([0.0, -0.0, 6e-8, -6e-8, np.inf, -np.inf, np.nan, 65504],
                          dtype=np.float16)
    return v


def with_specials32(v):
    """`v` (float32) with signed zeros, denormals, infinities, NaN and the largest float."""
    v = v.copy()
    v.flat[:8] = np.array([0.0, -0.0, 1e-45, -1e-45, np.inf, -np.inf, np.nan, 3.4e38],
                          dtype=np.float32)
    return v


def write(out, name, chans, compression, tiles=None):
    for suffix, comp in (("ht", compression), ("raw", OpenEXR.NO_COMPRESSION)):
        hdr = {"compression": comp, "type": OpenEXR.scanlineimage}
        if tiles:
            hdr["type"] = OpenEXR.tiledimage
            td = OpenEXR.TileDescription()
            td.xSize, td.ySize = tiles
            td.mode, td.roundingMode = OpenEXR.ONE_LEVEL, OpenEXR.ROUND_DOWN
            hdr["tiles"] = td
        OpenEXR.File(hdr, chans).write(os.path.join(out, f"{name}.{suffix}.exr"))


def main():
    out = sys.argv[1]
    small = "--small" in sys.argv
    os.makedirs(out, exist_ok=True)
    rng = np.random.default_rng(2026)
    f16, f32 = np.float16, np.float32

    h, w = 21, 33
    write(out, "rgba_half_256", {
        "R": smooth(rng, h, w, 3.0, 0.01, f16),
        "G": smooth(rng, h, w, 1.0, 0.002, f16),
        "B": with_specials(smooth(rng, h, w, -2.0, 0.0, f16)),
        "A": np.clip(smooth(rng, h, w, 1.0, 0.0, f16), 0, 1).astype(f16),
    }, HT256)
    write(out, "y_half_z_float_32", {
        "Y": smooth(rng, 70, 19, 100.0, 0.5, f16),
        "Z": smooth(rng, 70, 19, 1000.0, 0.001, f32),
    }, HT32)
    write(out, "rgb_half_tiled", {
        "R": smooth(rng, 40, 50, 5.0, 0.0, f16),
        "G": smooth(rng, 40, 50, 0.5, 0.0, f16),
        "B": with_specials(smooth(rng, 40, 50, -3.0, 0.0, f16)),
    }, HT256, tiles=(16, 24))
    # 32-bit channels (the 64-bit decode path): FLOAT with special values, UINT ids.
    write(out, "float_uint_32", {
        "Z": with_specials32(smooth(rng, 37, 29, 1000.0, 0.001, f32)),
        "id": (np.arange(37 * 29, dtype=np.uint32).reshape(37, 29) * 7919 % 100003),
    }, HT32)
    # Smooth values with large magnitudes: neighbours share exponents, so the tiles compress.
    write(out, "rgb_float_tiled", {
        "R": smooth(rng, 30, 40, 1000.0, 0.0, f32),
        "G": smooth(rng, 30, 40, 900.0, 0.0, f32),
        "B": with_specials32(smooth(rng, 30, 40, 800.0, 0.0, f32)),
    }, HT256, tiles=(32, 16))
    if small:
        return

    # 32-bit: RGB float scanline (RCT on 32-bit components), full-range UINT, float noise.
    for i, (h, w) in enumerate([(1, 1), (33, 129), (300, 301)]):
        write(out, f"rgb_float_{i}", {
            "R": smooth(rng, h, w, 4.0, 0.05, f32),
            "G": smooth(rng, h, w, 1.0, 0.0, f32),
            "B": with_specials32(smooth(rng, h, w, 3e30, 1e25, f32)),
            "A": np.ones((h, w), f32),
        }, HT256)
    write(out, "uint_full", {
        "id": rng.integers(0, 2**32, (120, 90), dtype=np.uint32),
        "n": (np.arange(120 * 90, dtype=np.uint64).reshape(120, 90) * 2654435761 % 2**32)
        .astype(np.uint32),
    }, HT32)
    write(out, "float_noise_tiled", {
        "R": rng.standard_normal((150, 170)).astype(f32),
        "G": (rng.standard_normal((150, 170)) * 1e-38).astype(f32),
        "B": rng.integers(0, 2**32, (150, 170), dtype=np.uint32).view(f32),
    }, HT256, tiles=(64, 32))
    write(out, "mixed_half_float_uint", {
        "R": smooth(rng, 99, 77, 2.0, 0.01, f16),
        "G": smooth(rng, 99, 77, 2.0, 0.01, f16),
        "B": smooth(rng, 99, 77, 2.0, 0.01, f16),
        "Z": smooth(rng, 99, 77, 500.0, 0.01, f32),
        "id": rng.integers(0, 2**20, (99, 77), dtype=np.uint32),
    }, HT32)

    # The full set: sizes around the code-block and chunk edges, layers, bit patterns.
    for i, (h, w) in enumerate([(1, 1), (2, 3), (32, 128), (33, 129), (257, 64), (300, 301),
                                (64, 1024), (513, 17)]):
        for comp, cname in ((HT256, "256"), (HT32, "32")):
            write(out, f"rgb_{i}_{cname}", {
                "R": smooth(rng, h, w, 4.0, 0.05, f16),
                "G": smooth(rng, h, w, 1.0, 0.0, f16),
                "B": smooth(rng, h, w, 60000.0, 1.0, f16),
            }, comp)
    write(out, "layers", {
        "diffuse.R": smooth(rng, 90, 77, 1.0, 0.01, f16),
        "diffuse.G": smooth(rng, 90, 77, 1.0, 0.01, f16),
        "diffuse.B": smooth(rng, 90, 77, 1.0, 0.01, f16),
        "spec.red": smooth(rng, 90, 77, 2.0, 0.0, f16),
        "spec.green": smooth(rng, 90, 77, 2.0, 0.0, f16),
        "spec.blue": smooth(rng, 90, 77, 2.0, 0.0, f16),
        "mask": smooth(rng, 90, 77, 1.0, 0.0, f16),
    }, HT256)
    write(out, "noise", {
        "R": rng.standard_normal((200, 200)).astype(f16),
        "G": specials(200, 200),
        "B": (rng.standard_normal((200, 200)) * 1e4).astype(f16),
    }, HT32)
    for i, t in enumerate([(32, 32), (64, 16), (100, 100)]):
        write(out, f"tiled_{i}", {
            "R": smooth(rng, 150, 170, 5.0, 0.01, f16),
            "G": smooth(rng, 150, 170, 0.5, 0.0, f16),
            "B": specials(150, 170),
            "A": smooth(rng, 150, 170, 1.0, 0.0, f16),
        }, HT256, tiles=t)


if __name__ == "__main__":
    main()
