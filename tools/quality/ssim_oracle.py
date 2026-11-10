#!/usr/bin/env python3
"""SSIM oracle for specular-inspect's `ssim_reference` (quality.rs tests).

Builds the same test planes as `quality::tests::plane` / `blur` and prints scikit-image's
structural_similarity with the published settings (Wang et al. 2004: 11 px Gaussian window,
sigma 1.5, population covariance, data range 1). Run it in a venv with scikit-image:

    python -I tools/quality/ssim_oracle.py
"""
import numpy as np
from skimage.metrics import structural_similarity

W, H = 40, 30
f = np.float32


def plane():
    p = np.zeros((H, W), dtype=np.float32)
    for y in range(H):
        for x in range(W):
            xf, yf = f(x), f(y)
            p[y, x] = (f(0.5)
                       + f(0.3) * np.sin(xf * f(0.37)) * np.cos(yf * f(0.23))
                       + f(0.1) * ((xf + f(2.0) * yf) % f(7.0)) / f(7.0))
    return p


def blur(p):
    out = np.zeros_like(p)
    for y in range(H):
        for x in range(W):
            s = f(0.0)
            for dy in (-1, 0, 1):
                for dx in (-1, 0, 1):
                    s += p[min(max(y + dy, 0), H - 1), min(max(x + dx, 0), W - 1)]
            out[y, x] = s / f(9.0)
    return out


def ssim(a, b):
    return structural_similarity(a.astype(np.float64), b.astype(np.float64),
                                 gaussian_weights=True, sigma=1.5,
                                 use_sample_covariance=False, data_range=1.0)


a = plane()
print(f"SKIMAGE_BLUR = {ssim(a, blur(a))!r}")
print(f"SKIMAGE_SCALED = {ssim(a, a * f(0.8) + f(0.05))!r}")
