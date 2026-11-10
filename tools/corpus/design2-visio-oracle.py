"""Compares our Visio page renders with LibreOffice's.

Usage: design2-visio-oracle.py OURS_DIR REF_DIR
OURS_DIR holds `<stem>_p<N>.ppm` from the `visio-render` example; REF_DIR holds
`<stem>-<N>.ppm` from `pdftoppm -r DPI` of LibreOffice's PDF export
(`soffice -env:UserInstallation=file://<private profile> --headless --convert-to pdf`), at
the same dpi. Images of different sizes are compared over their common area (and the size
difference is reported).

Per page: mean absolute channel difference, and two structural measures that tolerate
anti-aliasing, font and small placement differences: ink recall (reference ink pixels with
our ink within R px) and ink precision (our ink pixels with reference ink within R px).
Ink is a pixel whose darkest channel differs from white by more than 64. A page passes when
recall and precision are both at least 90 %.

LibreOffice sometimes places a whole page's drawing a little off (a page origin it derives
differently): when shifting our render by up to MAX_SHIFT pixels (found by cross-correlating
the ink masks) improves the match, the shifted figures are reported with the shift.
"""
import os
import re
import sys

import numpy as np

R = 2
MAX_SHIFT = 60
THRESH = 64


def read_ppm(path):
    with open(path, 'rb') as f:
        data = f.read()
    parts = data.split(maxsplit=4)
    w, h = int(parts[1]), int(parts[2])
    return np.frombuffer(parts[4][: w * h * 3], dtype=np.uint8).reshape(h, w, 3)


def ink(img):
    return (255 - img.astype(np.int16)).max(axis=2) > THRESH


def dilate(m, r):
    out = m.copy()
    for dy in range(-r, r + 1):
        for dx in range(-r, r + 1):
            out |= np.roll(np.roll(m, dy, axis=0), dx, axis=1)
    return out


def metrics(a, b):
    ia, ib = ink(a), ink(b)
    if ia.sum() == 0 and ib.sum() == 0:
        return np.abs(a.astype(np.int16) - b.astype(np.int16)).mean(), 1.0, 1.0
    mean = np.abs(a.astype(np.int16) - b.astype(np.int16)).mean()
    recall = (ib & dilate(ia, R)).sum() / max(ib.sum(), 1)
    precision = (ia & dilate(ib, R)).sum() / max(ia.sum(), 1)
    return mean, recall, precision


def best_shift(a, b):
    ia, ib = ink(a).astype(np.float64), ink(b).astype(np.float64)
    if ia.sum() == 0 or ib.sum() == 0:
        return 0, 0
    c = np.real(np.fft.ifft2(np.fft.fft2(ib) * np.conj(np.fft.fft2(ia))))
    h, w = c.shape
    best, at = -1.0, (0, 0)
    for dy in range(-MAX_SHIFT, MAX_SHIFT + 1):
        for dx in range(-MAX_SHIFT, MAX_SHIFT + 1):
            v = c[dy % h, dx % w]
            if v > best:
                best, at = v, (dy, dx)
    return at


def main():
    ours, refd = sys.argv[1], sys.argv[2]
    refs = {}
    for n in os.listdir(refd):
        m = re.match(r'(.*)-0*(\d+)\.ppm$', n)
        if m:
            refs[(m.group(1), int(m.group(2)))] = n
    rows = []
    for n in sorted(os.listdir(ours)):
        m = re.match(r'(.*)_p(\d+)\.ppm$', n)
        if not m:
            continue
        key = (m.group(1), int(m.group(2)))
        if key not in refs:
            rows.append((n, None, None, None, 'no reference page'))
            continue
        a = read_ppm(os.path.join(ours, n))
        b = read_ppm(os.path.join(refd, refs[key]))
        note = ''
        if a.shape != b.shape:
            note = f'size ours {a.shape[1]}x{a.shape[0]} ref {b.shape[1]}x{b.shape[0]}'
        h, w = min(a.shape[0], b.shape[0]), min(a.shape[1], b.shape[1])
        a, b = a[:h, :w], b[:h, :w]
        mean, recall, precision = metrics(a, b)
        dy, dx = best_shift(a, b)
        if (dy, dx) != (0, 0):
            s = np.full_like(a, 255)
            ys, yd = (slice(0, h - dy), slice(dy, h)) if dy >= 0 else (slice(-dy, h), slice(0, h + dy))
            xs, xd = (slice(0, w - dx), slice(dx, w)) if dx >= 0 else (slice(-dx, w), slice(0, w + dx))
            s[yd, xd] = a[ys, xs]
            m2 = metrics(s, b)
            if m2[1] + m2[2] > recall + precision + 0.02:
                mean, recall, precision = m2
                note = (note + ' ' if note else '') + f'shifted {dx},{dy} px'
        rows.append((n, mean, recall, precision, note))
    ok = [r for r in rows if r[1] is not None]
    for r in ok:
        print(f'{r[0]}\tmean {r[1]:.2f}\trecall {r[2] * 100:.1f}%\tprecision {r[3] * 100:.1f}%\t{r[4]}')
    for r in rows:
        if r[1] is None:
            print(f'{r[0]}\tskipped\t{r[4]}')
    if ok:
        passed = [r for r in ok if r[2] >= 0.9 and r[3] >= 0.9]
        print(f'pages {len(ok)}, passed {len(passed)} ({100 * len(passed) / len(ok):.1f}%), '
              f'mean diff {np.mean([r[1] for r in ok]):.2f}, '
              f'median recall {100 * np.median([r[2] for r in ok]):.1f}%, '
              f'median precision {100 * np.median([r[3] for r in ok]):.1f}%')


if __name__ == '__main__':
    main()
