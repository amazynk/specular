#!/usr/bin/env python3
"""Lens-model oracle: compares Develop's lens models with the cameras' own corrected JPEGs.

Input: a folder written by `cargo test -p specular-develop --test lens_oracle -- --ignored`
(see that file): per raw, a 6x6-binned linear camera RGB image, the largest embedded JPEG
and the model's back-map factors.

Distortion: scene features are matched between the JPEG (corrected in the camera) and the
raw by normalised cross-correlation on log luminance; each match gives the raw radius of a
JPEG radius. The model predicts the same through its factor; one free scale absorbs the
camera's own zoom. Printed: median and 90th-percentile position error in full-resolution
pixels, for the model and for no correction.

Colour fringes: red and blue patches are matched against green in the raw itself; the
radial part of the shift (fitted as a scale about the centre) is compared with the model's
red/green and blue/green factor ratio.

Uses numpy only. Run with `python3 -I`.
"""
import glob
import os
import sys

import numpy as np
from numpy.lib.stride_tricks import sliding_window_view

BIN = 6


def lin(c):
    return np.where(c <= 0.04045, c / 12.92, ((c + 0.055) / 1.055) ** 2.4)


def resize(img, w, h):
    ys = (np.arange(h + 1) * img.shape[0] / h).astype(int)
    xs = (np.arange(w + 1) * img.shape[1] / w).astype(int)
    c = np.pad(img.cumsum(0).cumsum(1), ((1, 0), (1, 0), (0, 0)))
    s = c[ys[1:]][:, xs[1:]] - c[ys[:-1]][:, xs[1:]] - c[ys[1:]][:, xs[:-1]] + c[ys[:-1]][:, xs[:-1]]
    area = np.outer(ys[1:] - ys[:-1], xs[1:] - xs[:-1])[:, :, None]
    return s / area


def norm(a):
    return (a - a.mean()) / (a.std() + 1e-9)


def sub(a, b, c):
    den = a - 2 * b + c
    return 0.0 if abs(den) < 1e-12 else 0.5 * (a - c) / den


def match(A, B, step, half, search, min_std, min_ncc):
    """Positions of B's patches found in A: rows of (x, y, x_in_A, y_in_A)."""
    h, w = A.shape
    out = []
    for y in range(half + search, h - half - search, step):
        for x in range(half + search, w - half - search, step):
            p = B[y - half:y + half + 1, x - half:x + half + 1]
            if p.std() < min_std:
                continue
            p = norm(p)
            win = A[y - half - search:y + half + search + 1, x - half - search:x + half + search + 1]
            sw = sliding_window_view(win, p.shape)
            m = sw.mean((2, 3))
            s = sw.std((2, 3)) + 1e-9
            ncc = ((sw - m[..., None, None]) * p).mean((2, 3)) / s
            j = np.unravel_index(np.argmax(ncc), ncc.shape)
            if ncc[j] < min_ncc or j[0] in (0, 2 * search) or j[1] in (0, 2 * search):
                continue
            dy = j[0] - search + sub(ncc[j[0] - 1, j[1]], ncc[j], ncc[j[0] + 1, j[1]])
            dx = j[1] - search + sub(ncc[j[0], j[1] - 1], ncc[j], ncc[j[0], j[1] + 1])
            out.append((x, y, x + dx, y + dy))
    return np.array(out)


def main(d):
    for binf in sorted(glob.glob(os.path.join(d, "*.bin*"))):
        name = os.path.basename(binf).split(".bin")[0]
        bw, bh = map(int, binf.rsplit("bin", 1)[1].split("x"))
        raw = np.fromfile(binf, dtype="<f4").reshape(bh, bw, 3).astype(np.float64)
        prf = glob.glob(os.path.join(d, name + ".prev*"))[0]
        pw, ph = map(int, prf.rsplit("prev", 1)[1].split("x"))
        prev = np.fromfile(prf, dtype=np.uint8).reshape(ph, pw, 4)[:, :, :3] / 255.0
        model = np.loadtxt(os.path.join(d, name + ".model"))
        rho_t, fr, fg, fb = model.T
        R = np.hypot(bw, bh) / 2
        cx, cy = (bw - 1) / 2, (bh - 1) / 2
        g = lambda a: np.log(np.maximum(a, 1e-5))
        A = norm(g(raw.mean(2)))
        B = norm(g(lin(resize(prev, bw, bh)).mean(2)))
        m = match(A, B, 24, 10, 50, 0.3, 0.85)
        ru = np.hypot(m[:, 0] - cx, m[:, 1] - cy) / R
        rs = np.hypot(m[:, 2] - cx, m[:, 3] - cy) / R
        keep = ru > 0.1
        ru, rs = ru[keep], rs[keep]
        # Model: rs = k * ru * F(k' ru) with the camera's zoom unknown; fit one scale z on the
        # output radius: rs = z ru F(z ru) / F-normalised... a scale on the result suffices.
        pred = ru * np.interp(ru, rho_t, fg)
        z = np.median(rs / pred)
        e_model = np.abs(rs - z * pred) * R * BIN
        z0 = np.median(rs / ru)
        e_none = np.abs(rs - z0 * ru) * R * BIN
        print(f"{name}: {len(ru)} matches; distortion error median {np.median(e_model):.2f} px "
              f"(p90 {np.percentile(e_model, 90):.2f}), uncorrected median {np.median(e_none):.2f} px "
              f"(p90 {np.percentile(e_none, 90):.2f}); camera zoom vs ours {z:.4f}")
        # Colour fringes in the raw: red and blue against green.
        G = norm(g(raw[:, :, 1]))
        for ch, f, label in ((0, fr, "red"), (2, fb, "blue")):
            C = norm(g(raw[:, :, ch]))
            mm = match(C, G, 16, 8, 4, 0.4, 0.9)
            if len(mm) < 20:
                print(f"  {label}: too few matches")
                continue
            vx, vy = mm[:, 0] - cx, mm[:, 1] - cy
            dx, dy = mm[:, 2] - mm[:, 0], mm[:, 3] - mm[:, 1]
            # Shift = a * (position - centre) + b (b: the CFA's half-pixel offsets).
            X = np.zeros((2 * len(mm), 3))
            X[0::2, 0], X[1::2, 0] = vx, vy
            X[0::2, 1] = 1
            X[1::2, 2] = 1
            y = np.empty(2 * len(mm))
            y[0::2], y[1::2] = dx, dy
            a = np.linalg.lstsq(X, y, rcond=None)[0][0]
            rr = np.hypot(vx, vy) / R
            want = np.median(np.interp(rr, rho_t, f / fg) - 1)
            print(f"  {label}: measured scale {a * 1e4:+.2f}e-4, model {want * 1e4:+.2f}e-4 "
                  f"({len(mm)} matches; corner shift measured {a * R * BIN:+.2f} px, "
                  f"model {want * R * BIN:+.2f} px)")


if __name__ == "__main__":
    main(sys.argv[1])
