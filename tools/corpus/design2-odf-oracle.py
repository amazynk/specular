"""Compares our OpenDocument page renders with LibreOffice's.

Usage: python3 -I design2-odf-oracle.py OURS_DIR REF_DIR
OURS_DIR holds `<stem>_p<N>.ppm` from the `odf-render` example; REF_DIR holds
`<stem>-<N>.ppm` from `pdftoppm -r DPI` of LibreOffice's PDF export (same DPI, so the same
page size up to a pixel of rounding; the common area is compared).

Per page: mean absolute channel difference, and two structural measures that tolerate
anti-aliasing, font and small placement differences: ink recall (reference ink pixels with
our ink within R px) and ink precision (our ink pixels with reference ink within R px). Ink
is a pixel whose largest channel difference from the page's most common colour is over 64.
A page passes when recall and precision are both at least 90 %.
"""
import os
import re
import sys

import numpy as np

R = 2
THRESH = 64


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
    ours, refs = sys.argv[1], sys.argv[2]
    rows = []
    for name in sorted(os.listdir(ours)):
        m = re.match(r'(.*)_p(\d+)\.ppm$', name)
        if not m:
            continue
        stem, page = m.group(1), int(m.group(2))
        cands = [f'{stem}-{page}.ppm', f'{stem}-{page:02d}.ppm', f'{stem}-{page:03d}.ppm']
        ref = next((os.path.join(refs, c) for c in cands if os.path.exists(os.path.join(refs, c))), None)
        if ref is None:
            rows.append((stem, page, None, None, None, 'no reference'))
            continue
        a, b = read_ppm(os.path.join(ours, name)), read_ppm(ref)
        h, w = min(a.shape[0], b.shape[0]), min(a.shape[1], b.shape[1])
        if abs(a.shape[0] - b.shape[0]) > 2 or abs(a.shape[1] - b.shape[1]) > 2:
            rows.append((stem, page, None, None, None, f'size {a.shape[1]}x{a.shape[0]} vs {b.shape[1]}x{b.shape[0]}'))
            continue
        a, b = a[:h, :w], b[:h, :w]
        mean = np.abs(a.astype(np.int16) - b.astype(np.int16)).mean()
        ia, ib = ink(a), ink(b)
        if ia.sum() == 0 and ib.sum() == 0:
            rows.append((stem, page, mean, 1.0, 1.0, ''))
            continue
        recall = (ib & dilate(ia, R)).sum() / max(ib.sum(), 1)
        precision = (ia & dilate(ib, R)).sum() / max(ia.sum(), 1)
        rows.append((stem, page, mean, recall, precision, ''))
    ok = [r for r in rows if r[2] is not None]
    passed = [r for r in ok if r[3] >= 0.9 and r[4] >= 0.9]
    for r in ok:
        print(f'{r[0]}\tp{r[1]}\tmean {r[2]:.2f}\trecall {r[3] * 100:.1f}%\tprecision {r[4] * 100:.1f}%')
    for r in rows:
        if r[2] is None:
            print(f'{r[0]}\tp{r[1]}\tskipped\t{r[5]}')
    if ok:
        print(f'pages {len(ok)}, passed {len(passed)} ({100 * len(passed) / len(ok):.1f}%), '
              f'mean diff {np.mean([r[2] for r in ok]):.2f}')


if __name__ == '__main__':
    main()
