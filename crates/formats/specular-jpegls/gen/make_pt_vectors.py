#!/usr/bin/env python3
"""Adds the point-transform (Pt) test vectors to ../tests/vectors.

CharLS (through imagecodecs) cannot write Pt, so its streams are spliced: the SOS Pt field is set
after encoding. The coded data does not depend on Pt (MAXVAL and the thresholds follow P), so the
expected output is the ISO reference libjpeg's (pylibjpeg-libjpeg wheel) reading of the spliced
stream: each sample shifted left by Pt, clamped to 2^P - 1. Both are test oracles only.

Usage: python -I make_pt_vectors.py VECTOR_DIR
  Writes NAME.jls and replaces the `pt_*` lines of VECTOR_DIR/manifest.txt.
"""

import os
import sys

import imagecodecs
import numpy as np
from libjpeg import decode as libjpeg_decode


def fnv(planes):
    h = 0xCBF29CE484222325
    for p in planes:
        for b in np.ascontiguousarray(p.astype("<u2")).tobytes():
            h = ((h ^ b) * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return h


def image(shape, top, seed):
    rng = np.random.default_rng(seed)
    y, x = np.mgrid[0 : shape[0], 0 : shape[1]]
    base = (x * 7 + y * 3) % top
    noise = rng.integers(0, max(top // 8, 2), shape[:2])
    img = (base + noise) % top
    if len(shape) == 3:
        img = np.stack([(img + 17 * c) % top for c in range(shape[2])], axis=-1)
    return img


def with_pt(data, pt):
    i = data.index(b"\xff\xda")
    pos = i + 2 + data[i + 2] * 256 + data[i + 3] - 1
    data = bytearray(data)
    data[pos] = (data[pos] & 0xF0) | pt
    return bytes(data)


def main():
    out = sys.argv[1]
    lines = []
    cases = [
        # name, shape, dtype, coded top (exclusive), NEAR, Pt
        ("pt_grey8_pt1", (29, 41), np.uint8, 128, 0, 1),
        ("pt_grey8_pt3_near2", (33, 40), np.uint8, 32, 2, 3),
        ("pt_grey8_clamp", (17, 23), np.uint8, 256, 0, 2),
        ("pt_grey16_pt4", (19, 37), np.uint16, 4096, 0, 4),
        ("pt_rgb8_pt2", (23, 37, 3), np.uint8, 64, 0, 2),
        ("pt_rgb8_pt1_near1", (21, 30, 3), np.uint8, 128, 1, 1),
    ]
    for name, shape, dt, top, near, pt in cases:
        img = image(shape, top, len(lines) + 90).astype(dt)
        data = with_pt(imagecodecs.jpegls_encode(img, level=near), pt)
        got = libjpeg_decode(data).astype(np.uint16)
        if top << pt <= np.iinfo(dt).max + 1 and near == 0:
            assert (got == img.astype(np.uint16) << pt).all(), name
        planes = [got] if got.ndim == 2 else [got[..., c] for c in range(got.shape[2])]
        with open(os.path.join(out, name + ".jls"), "wb") as f:
            f.write(data)
        lines.append(f"{name} {shape[1]} {shape[0]} {len(planes)} {fnv(planes):016x}")
    path = os.path.join(out, "manifest.txt")
    with open(path) as f:
        keep = [l for l in f.read().splitlines() if not l.startswith("pt_")]
    with open(path, "w") as f:
        f.write("\n".join(keep + lines) + "\n")


if __name__ == "__main__":
    main()
