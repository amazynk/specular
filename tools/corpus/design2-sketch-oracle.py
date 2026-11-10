"""Compares our Sketch renders with the documents' own previews.

Usage: design2-sketch-oracle.py OUT_DIR
OUT_DIR holds, from the specular-sketch `sketch-render` example, `<stem>-overview.ppm` (the
page last viewed, drawn whole at the preview's size) and `<stem>-preview.png` (the preview
Sketch wrote). Pillow reads the PNG; transparent preview pixels are composited on white.

Per document: mean absolute channel difference, and two structural measures that tolerate
anti-aliasing, font and one-pixel placement differences: ink recall (preview ink pixels with
our ink within R px) and ink precision (our ink pixels with preview ink within R px). Ink is a
pixel whose largest channel difference from the background (the most common colour) is over
THRESH. A document passes when recall and precision are both at least 90 %.
"""
import os
import sys

import numpy as np
from PIL import Image

R = 2
THRESH = 48


def read_ppm(path):
    with open(path, 'rb') as f:
        data = f.read()
    parts = data.split(maxsplit=4)
    w, h = int(parts[1]), int(parts[2])
    return np.frombuffer(parts[4][: w * h * 3], dtype=np.uint8).reshape(h, w, 3)


def ink(img):
    flat = img.reshape(-1, 3)
    vals, counts = np.unique(flat, axis=0, return_counts=True)
    bg = vals[counts.argmax()].astype(np.int16)
    return np.abs(img.astype(np.int16) - bg).max(axis=2) > THRESH


def dilate(m, r):
    out = m.copy()
    for dy in range(-r, r + 1):
        for dx in range(-r, r + 1):
            out |= np.roll(np.roll(m, dy, axis=0), dx, axis=1)
    return out


def main():
    out = sys.argv[1]
    rows = []
    for name in sorted(os.listdir(out)):
        if not name.endswith('-overview.ppm'):
            continue
        stem = name[: -len('-overview.ppm')]
        ref_path = os.path.join(out, stem + '-preview.png')
        if not os.path.exists(ref_path):
            continue
        ref = Image.open(ref_path).convert('RGBA')
        white = Image.new('RGBA', ref.size, (255, 255, 255, 255))
        ref = np.asarray(Image.alpha_composite(white, ref).convert('RGB'))
        a = read_ppm(os.path.join(out, name))
        if ref.shape != a.shape:
            print(f'{stem}\tskipped\tsize {a.shape} vs preview {ref.shape}')
            continue
        mean = np.abs(a.astype(np.int16) - ref.astype(np.int16)).mean()
        ia, ib = ink(a), ink(ref)
        recall = (ib & dilate(ia, R)).sum() / max(ib.sum(), 1)
        precision = (ia & dilate(ib, R)).sum() / max(ia.sum(), 1)
        rows.append((stem, mean, recall, precision))
        print(f'{stem}\t{a.shape[1]}x{a.shape[0]}\tmean {mean:.2f}\t'
              f'recall {recall * 100:.1f}%\tprecision {precision * 100:.1f}%')
    if rows:
        passed = [r for r in rows if r[2] >= 0.9 and r[3] >= 0.9]
        print(f'documents {len(rows)}, passed {len(passed)}')


if __name__ == '__main__':
    main()
