#!/usr/bin/env python3
"""Writes the tile-codec fixtures of specular-fits (tests/data/codec_*.tsv) with astropy's codecs
(astropy.io.fits.hdu.compressed._codecs, which wrap cfitsio's C code) as the oracle.

Usage (from the crate directory, with astropy installed, e.g. in a venv under target/):
    python gen/codec_fixtures.py

Every row: name, parameters, the compressed tile as stored in a FITS heap (hex), and the values
astropy decodes it to (decimal, comma-separated, row-major with the FITS fast axis first).
Deterministic: a fixed seed.
"""

import os
import sys

import numpy as np
from astropy.io.fits.hdu.compressed._codecs import HCompress1, PLIO1, Rice1

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "..", "tests", "data")
rng = np.random.default_rng(20261011)


def u8(b):
    return np.frombuffer(b, dtype=np.uint8)


def vals(a):
    return ",".join(str(int(x)) for x in np.asarray(a).ravel())


def rice_cases():
    rows = []

    def add(name, data, bytepix, blocksize):
        data = np.asarray(data)
        dt = {1: "i1", 2: "i2", 4: "i4"}[bytepix]
        codec = Rice1(blocksize=blocksize, bytepix=bytepix, tilesize=data.size)
        enc = codec.encode(data.astype(dt, copy=False) if bytepix != 1 else data.astype("u1").view("i1"))
        dec = codec.decode(u8(enc))[: data.size]
        if bytepix == 1:
            dec = dec.view("u1")
        assert np.array_equal(dec.astype("i8"), data.astype("i8")), name
        rows.append((name, bytepix, blocksize, enc.hex(), vals(dec)))

    n = 1000
    smooth = (1000 + 50 * np.sin(np.arange(n) / 30.0) + rng.normal(0, 3, n)).astype("i4")
    add("i32_smooth", smooth, 4, 32)
    add("i32_noise", rng.integers(-(2**31), 2**31 - 1, 300, dtype="i8"), 4, 32)
    add("i32_zero_blocks", np.concatenate([np.zeros(70), np.full(40, 5), np.zeros(33)]).astype("i4"), 4, 32)
    add("i32_bs16_short", (rng.normal(0, 200, 77)).astype("i4"), 4, 16)
    add("i32_spike", np.where(np.arange(200) % 37 == 0, 2**30, 7).astype("i4"), 4, 32)
    add("i16_smooth", (smooth - 1000).astype("i2"), 2, 32)
    add("i16_noise", rng.integers(-32768, 32767, 300, dtype="i8"), 2, 32)
    add("i16_extremes", np.array([-32768, 32767] * 40 + [0] * 5), 2, 32)
    add("i16_bs16", rng.normal(100, 10, 101).astype("i2"), 2, 16)
    add("u8_ramp", np.arange(300) % 256, 1, 32)
    add("u8_noise", rng.integers(0, 256, 257), 1, 32)
    add("u8_const", np.full(65, 200), 1, 32)
    add("single", np.array([123456]), 4, 32)
    return rows


def hcompress_cases():
    rows = []

    def add(name, data, scale, smooth, bytepix=4):
        data = np.asarray(data, dtype="i4" if bytepix == 4 else "i8")
        ny, nx = data.shape  # slow, fast
        c = HCompress1(scale=scale, smooth=smooth, bytepix=bytepix, nx=ny, ny=nx)
        enc = c.encode(data)
        dec = c.decode(u8(enc)).reshape(ny, nx)
        if scale <= 1:
            assert np.array_equal(dec, data), name
        rows.append((name, nx, ny, scale, int(smooth), enc.hex(), vals(dec)))

    y, x = np.mgrid[0:12, 0:16]
    img = (500 + 300 * np.exp(-((x - 7.3) ** 2 + (y - 5.1) ** 2) / 8.0) + rng.normal(0, 4, x.shape)).astype("i4")
    add("smooth_16x12_lossless", img, 0, False)
    add("smooth_16x12_scale1", img, 1, False)
    add("smooth_16x12_scale8", img, 8, False)
    add("smooth_16x12_scale8_smooth", img, 8, True)
    add("smooth_16x12_scale25_smooth", img, 25, True)
    add("odd_7x5", rng.integers(-1000, 1000, (5, 7)), 0, False)
    add("odd_5x7", rng.integers(-1000, 1000, (7, 5)), 0, False)
    add("odd_9x13_scale4_smooth", (img[:, :9].repeat(2, 0)[:13] * 3), 4, True)
    add("negative_8x8", -np.abs(rng.integers(0, 70000, (8, 8))), 0, False)
    add("big_33x20", rng.integers(-(2**20), 2**20, (20, 33)), 0, False)
    add("const_4x4", np.full((4, 4), 77), 0, False)
    y, x = np.mgrid[0:40, 0:64]
    star = (1000 + 20000 * np.exp(-((x - 30.2) ** 2 + (y - 18.7) ** 2) / 3.0) + rng.normal(0, 10, x.shape)).astype("i4")
    add("star_64x40_scale16_smooth", star, 16, True)
    add("star_64x40_scale16", star, 16, False)
    add("bytepix8_6x4", np.array([[2**30, -(2**30), 5, 0, 1, -1]] * 4, dtype="i8"), 0, False, bytepix=8)
    return rows


def plio_cases():
    rows = []

    def add(name, data):
        data = np.asarray(data, dtype="i4")
        c = PLIO1(tilesize=data.size)
        enc = c.encode(data)
        dec = c.decode(u8(enc))[: data.size]
        assert np.array_equal(dec, data), name
        # The FITS heap stores the line list as big-endian 16-bit words.
        be = np.frombuffer(enc, dtype="<i2").astype(">i2").tobytes()
        rows.append((name, data.size, be.hex(), vals(dec)))

    add("small", [0, 0, 5, 5, 5, 9, 0, 0, 1, 2])
    mask = np.zeros(500, dtype="i4")
    mask[20:80] = 1
    mask[100:103] = 7
    mask[300:450] = 3
    mask[460] = 1
    add("mask_runs", mask)
    add("big_values", np.array([0, 100000, 100000, 0, 4096, 8191, 5000000, 1, 0, 0, 0], dtype="i4"))
    add("noise", rng.integers(0, 4000, 200))
    add("all_zero", np.zeros(64))
    add("long_run", np.concatenate([np.zeros(4200), np.full(4500, 1), np.zeros(10)]))
    add("trailing_value", np.concatenate([np.zeros(30), [5]]))
    return rows


def write(name, header, rows):
    with open(os.path.join(OUT, name), "w") as f:
        f.write("# Written by gen/codec_fixtures.py with astropy's tile codecs (cfitsio); do not edit.\n")
        f.write("\t".join(header) + "\n")
        for r in rows:
            f.write("\t".join(str(x) for x in r) + "\n")


def main():
    os.makedirs(OUT, exist_ok=True)
    write("codec_rice.tsv", ["name", "bytepix", "blocksize", "compressed", "values"], rice_cases())
    write("codec_hcompress.tsv", ["name", "nx", "ny", "scale", "smooth", "compressed", "values"], hcompress_cases())
    write("codec_plio.tsv", ["name", "npix", "compressed", "values"], plio_cases())
    print("ok", file=sys.stderr)


if __name__ == "__main__":
    main()
