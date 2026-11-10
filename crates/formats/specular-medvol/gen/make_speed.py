#!/usr/bin/env python3
"""Writes the open-timing volumes into DIR: a 256^3 int16 .nii.gz (a smooth head-like
phantom with noise, so it compresses as a scan does) and a 512x512x300 float32 raw .nrrd.

Run with -I:  python3 -I make_speed.py DIR
"""

import os
import sys

import nibabel as nib
import nrrd
import numpy as np


def phantom(shape, dtype, seed):
    r = np.random.default_rng(seed)
    axes = [np.linspace(-1, 1, n, dtype=np.float32) for n in shape]
    x, y, z = np.meshgrid(*axes, indexing="ij", sparse=True)
    rr = x * x / 0.8 + y * y / 0.6 + z * z / 0.9
    v = np.where(rr < 1, 1000 * (1.2 - rr), -1000).astype(np.float32)
    v += r.normal(0, 20, size=shape).astype(np.float32)
    return v.astype(dtype)


def main():
    out = sys.argv[1]
    os.makedirs(out, exist_ok=True)
    a = np.diag([-0.9, 0.9, 1.0, 1.0])
    img = nib.Nifti1Image(phantom((256, 256, 256), np.int16, 1), a, dtype=np.int16)
    nib.save(img, os.path.join(out, "head256_i16.nii.gz"))
    hdr = {
        "space": "left-posterior-superior",
        "space directions": np.diag([0.7, 0.7, 1.25]),
        "space origin": np.array([-180.0, -180.0, -150.0]),
        "encoding": "raw",
    }
    nrrd.write(os.path.join(out, "ct512x300_f32.nrrd"), phantom((512, 512, 300), np.float32, 2), hdr,
               index_order="F")


if __name__ == "__main__":
    main()
