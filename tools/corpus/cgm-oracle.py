"""Compares our CGM renders with the WebCGM test suites' reference PNGs.

Usage: cgm-oracle.py OURS_DIR SUITE_DIR [SUITE_DIR...]
OURS_DIR holds `<stem>_p1.ppm` from the `cgm-render` example at 488×488 (the suites'
reference size); each SUITE_DIR has `<stem>.cgm` and `ref/<stem>.png`. Pillow reads the PNGs.

Per picture: mean absolute channel difference, and two structural measures that tolerate
anti-aliasing, font and one-pixel placement differences: ink recall (reference ink pixels
with our ink within R px) and ink precision (our ink pixels with reference ink within R px).
Ink is a pixel whose darkest channel differs from the background by more than 64. A picture
passes when recall and precision are both at least 90 %.
"""
import os
import sys

import numpy as np
from PIL import Image

R = 2
THRESH = 64


def read_ppm(path):
    with open(path, 'rb') as f:
        data = f.read()
    parts = data.split(maxsplit=4)
    w, h = int(parts[1]), int(parts[2])
    return np.frombuffer(parts[4][: w * h * 3], dtype=np.uint8).reshape(h, w, 3)


def ink(img):
    # Background: the most common colour.
    flat = img.reshape(-1, 3)
    vals, counts = np.unique(flat, axis=0, return_counts=True)
    bg = vals[counts.argmax()].astype(np.int16)
    return (np.abs(img.astype(np.int16) - bg).max(axis=2) > THRESH)


def dilate(m, r):
    out = m.copy()
    for dy in range(-r, r + 1):
        for dx in range(-r, r + 1):
            out |= np.roll(np.roll(m, dy, axis=0), dx, axis=1)
    return out


def main():
    ours = sys.argv[1]
    rows = []
    for suite in sys.argv[2:]:
        refdir = os.path.join(suite, 'ref')
        rendered = {n[:-7].lower(): n for n in os.listdir(ours) if n.endswith('_p1.ppm')}
        for name in sorted(os.listdir(refdir)):
            stem = name[:-4]
            mine = os.path.join(ours, rendered.get(stem.lower(), stem + '_p1.ppm'))
            if not os.path.exists(mine):
                rows.append((os.path.basename(suite), stem, None, None, None, 'not rendered'))
                continue
            ref = np.asarray(Image.open(os.path.join(refdir, name)).convert('RGB'))
            a = read_ppm(mine)
            if ref.shape != a.shape:
                rows.append((os.path.basename(suite), stem, None, None, None, f'reference {ref.shape[1]}x{ref.shape[0]}'))
                continue
            mean = np.abs(a.astype(np.int16) - ref.astype(np.int16)).mean()
            ia, ib = ink(a), ink(ref)
            if ia.sum() == 0 and ib.sum() == 0:
                rows.append((os.path.basename(suite), stem, mean, 1.0, 1.0, ''))
                continue
            recall = (ib & dilate(ia, R)).sum() / max(ib.sum(), 1)
            precision = (ia & dilate(ib, R)).sum() / max(ia.sum(), 1)
            rows.append((os.path.basename(suite), stem, mean, recall, precision, ''))
    ok = [r for r in rows if r[2] is not None]
    passed = [r for r in ok if r[3] >= 0.9 and r[4] >= 0.9]
    for r in sorted(ok, key=lambda r: min(r[3], r[4])):
        print(f'{r[0]}\t{r[1]}\tmean {r[2]:.2f}\trecall {r[3] * 100:.1f}%\tprecision {r[4] * 100:.1f}%')
    for r in rows:
        if r[2] is None:
            print(f'{r[0]}\t{r[1]}\tskipped\t{r[5]}')
    if ok:
        print(f'pictures {len(ok)}, passed {len(passed)} ({100 * len(passed) / len(ok):.1f}%), '
              f'mean of means {np.mean([r[2] for r in ok]):.2f}, max mean {max(r[2] for r in ok):.2f}, '
              f'median recall {np.median([r[3] for r in ok]) * 100:.1f}%, '
              f'median precision {np.median([r[4] for r in ok]) * 100:.1f}%, skipped {len(rows) - len(ok)}')


main()
