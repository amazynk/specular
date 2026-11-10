"""Compares our WPG renders with LibreOffice's (libwpg import), per file.

Usage: python3 -I design2-wpg-oracle.py DPI OURS_DIR WORK_DIR FILE...

OURS_DIR holds `<stem>_p1.ppm` from the `wpg-render` example at DPI. For each FILE, LibreOffice
converts it to PDF in WORK_DIR (with its own profile under WORK_DIR/lo, never the user's) and
pdftoppm rasterises page 1 at DPI. Reports per file: mean absolute channel difference, and
ink recall (reference ink pixels with our ink within R px) and ink precision (our ink with
reference ink within R px). Ink: darkest channel differs from white by more than 64. Sizes
that differ by a pixel or two are cropped to the common area.
"""
import os
import subprocess
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
    return (255 - img.astype(np.int16)).max(axis=2) > THRESH


def dilate(m, r):
    out = m.copy()
    for dy in range(-r, r + 1):
        for dx in range(-r, r + 1):
            out |= np.roll(np.roll(m, dy, axis=0), dx, axis=1)
    return out


def main():
    dpi, ours, work = sys.argv[1], sys.argv[2], os.path.abspath(sys.argv[3])
    pdfdir = os.path.join(work, 'pdf')
    os.makedirs(pdfdir, exist_ok=True)
    files = sys.argv[4:]
    todo = [f for f in files
            if not os.path.exists(os.path.join(pdfdir, os.path.splitext(os.path.basename(f))[0] + '.pdf'))]
    if todo:
        subprocess.run(['soffice', f'-env:UserInstallation=file://{work}/lo', '--headless',
                        '--convert-to', 'pdf', '--outdir', pdfdir] + todo,
                       capture_output=True, timeout=600)
    rows = []
    for f in files:
        stem = os.path.splitext(os.path.basename(f))[0]
        mine = os.path.join(ours, stem + '_p1.ppm')
        pdf = os.path.join(pdfdir, stem + '.pdf')
        if not os.path.exists(mine) or not os.path.exists(pdf):
            rows.append((stem, None, 'not rendered' if not os.path.exists(mine) else 'no oracle'))
            continue
        base = os.path.join(work, stem + '_lo')
        subprocess.run(['pdftoppm', '-r', dpi, '-singlefile', '-png', pdf, base],
                       capture_output=True, timeout=120)
        ref = np.asarray(Image.open(base + '.png').convert('RGB'))
        a = read_ppm(mine)
        if abs(ref.shape[0] - a.shape[0]) > 3 or abs(ref.shape[1] - a.shape[1]) > 3:
            rows.append((stem, None, f'size ours {a.shape[1]}x{a.shape[0]}, '
                                     f'oracle {ref.shape[1]}x{ref.shape[0]}'))
            continue
        h, w = min(ref.shape[0], a.shape[0]), min(ref.shape[1], a.shape[1])
        ref, a = ref[:h, :w], a[:h, :w]
        mean = np.abs(a.astype(np.int16) - ref.astype(np.int16)).mean()
        ia, ib = ink(a), ink(ref)
        recall = (ib & dilate(ia, R)).sum() / max(ib.sum(), 1)
        precision = (ia & dilate(ib, R)).sum() / max(ia.sum(), 1)
        rows.append((stem, (mean, recall, precision, ib.sum()), ''))
    for stem, m, note in rows:
        if m is None:
            print(f'{stem}\tskipped\t{note}')
        else:
            mean, rc, pr, n = m
            empty = ' (oracle empty)' if n == 0 else ''
            print(f'{stem}\tmean {mean:.2f}\trecall {rc * 100:.1f}%\tprecision {pr * 100:.1f}%{empty}')


if __name__ == '__main__':
    main()
