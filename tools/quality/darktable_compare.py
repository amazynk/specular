#!/usr/bin/env python3
"""CIEDE2000 between darktable-cli's and Specular's renders of the same raws.

Usage: darktable_compare.py DT_DIR SPECULAR_DIR (PNG files with the same stem). Each pair is
brought to the smaller one's size (box filter in linear light) before comparing. Prints a
Markdown table: p50, p95, p99 and max ΔE2000 per raw. Uses numpy and pillow; run with
`python3 -I`.
"""
import os
import sys

import numpy as np
from PIL import Image


def linear(path):
    a = np.asarray(Image.open(path).convert('RGB'), dtype=np.float64) / 255.0
    return np.where(a <= 0.04045, a / 12.92, ((a + 0.055) / 1.055) ** 2.4)


def shrink(a, h, w):
    # Box filter by whole factors, then crop: renders differ in size by a few pixels at most.
    fy, fx = max(1, a.shape[0] // h), max(1, a.shape[1] // w)
    a = a[: a.shape[0] // fy * fy, : a.shape[1] // fx * fx]
    a = a.reshape(a.shape[0] // fy, fy, a.shape[1] // fx, fx, 3).mean(axis=(1, 3))
    return a[:h, :w]


def lab(rgb):
    m = np.array([[0.4123908, 0.3575843, 0.1804808],
                  [0.2126390, 0.7151687, 0.0721923],
                  [0.0193308, 0.1191948, 0.9505322]])
    xyz = rgb @ m.T / np.array([0.95047, 1.0, 1.08883])
    f = np.where(xyz > 216 / 24389, np.cbrt(xyz), (24389 / 27 * xyz + 16) / 116)
    return np.stack([116 * f[..., 1] - 16, 500 * (f[..., 0] - f[..., 1]),
                     200 * (f[..., 1] - f[..., 2])], axis=-1)


def de2000(a, b):
    l1, a1, b1 = a[..., 0], a[..., 1], a[..., 2]
    l2, a2, b2 = b[..., 0], b[..., 1], b[..., 2]
    cb = (np.hypot(a1, b1) + np.hypot(a2, b2)) / 2
    g = 0.5 * (1 - np.sqrt(cb ** 7 / (cb ** 7 + 25.0 ** 7)))
    a1p, a2p = (1 + g) * a1, (1 + g) * a2
    c1p, c2p = np.hypot(a1p, b1), np.hypot(a2p, b2)
    h1p = np.mod(np.arctan2(b1, a1p), 2 * np.pi)
    h2p = np.mod(np.arctan2(b2, a2p), 2 * np.pi)
    dl, dc = l2 - l1, c2p - c1p
    dh = h2p - h1p
    dh = np.where(dh > np.pi, dh - 2 * np.pi, np.where(dh < -np.pi, dh + 2 * np.pi, dh))
    dh = np.where(c1p * c2p == 0, 0, dh)
    dhh = 2 * np.sqrt(c1p * c2p) * np.sin(dh / 2)
    lb, cbp = (l1 + l2) / 2, (c1p + c2p) / 2
    hs = h1p + h2p
    hb = np.where(np.abs(h1p - h2p) <= np.pi, hs / 2,
                  np.where(hs < 2 * np.pi, (hs + 2 * np.pi) / 2, (hs - 2 * np.pi) / 2))
    hb = np.where(c1p * c2p == 0, hs, hb)
    t = (1 - 0.17 * np.cos(hb - np.pi / 6) + 0.24 * np.cos(2 * hb)
         + 0.32 * np.cos(3 * hb + np.pi / 30) - 0.20 * np.cos(4 * hb - np.radians(63)))
    dth = np.radians(30) * np.exp(-((np.degrees(hb) - 275) / 25) ** 2)
    rc = 2 * np.sqrt(cbp ** 7 / (cbp ** 7 + 25.0 ** 7))
    sl = 1 + 0.015 * (lb - 50) ** 2 / np.sqrt(20 + (lb - 50) ** 2)
    sc, sh = 1 + 0.045 * cbp, 1 + 0.015 * cbp * t
    rt = -np.sin(2 * dth) * rc
    return np.sqrt((dl / sl) ** 2 + (dc / sc) ** 2 + (dhh / sh) ** 2
                   + rt * (dc / sc) * (dhh / sh))


def main():
    dt_dir, sp_dir = sys.argv[1], sys.argv[2]
    print('| raw | p50 | p95 | p99 | max |')
    print('|---|---|---|---|---|')
    for name in sorted(os.listdir(dt_dir)):
        sp = os.path.join(sp_dir, name)
        if not name.endswith('.png') or not os.path.exists(sp):
            continue
        a, b = linear(os.path.join(dt_dir, name)), linear(sp)
        h, w = min(a.shape[0], b.shape[0]), min(a.shape[1], b.shape[1])
        d = de2000(lab(shrink(a, h, w)), lab(shrink(b, h, w))).ravel()
        p = np.percentile(d, [50, 95, 99])
        print(f'| {name[:-4]} | {p[0]:.2f} | {p[1]:.2f} | {p[2]:.2f} | {d.max():.2f} |')


if __name__ == '__main__':
    main()
