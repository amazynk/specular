"""Compares our DjVu renders with DjVuLibre's ddjvu, page by page.

Usage: djvu-oracle.py OURS_DIR WORK_DIR DDJVU [--scale N] FILE...
OURS_DIR holds `<stem>_p<page>.ppm` from the `djvu-render` example (same scale). DDJVU is the
ddjvu command (a wrapper is fine). ddjvu renders each page at the same size (`-size`), then
per page: mean and max absolute channel difference and the share of pixels whose largest
channel difference exceeds TOL. A page passes when that share is under 1 %.
"""
import os
import subprocess
import sys

import numpy as np

TOL = 32


def read_ppm(path):
    with open(path, 'rb') as f:
        data = f.read()
    parts = data.split(maxsplit=4)
    assert parts[0] in (b'P6', b'P5'), path
    w, h = int(parts[1]), int(parts[2])
    ch = 3 if parts[0] == b'P6' else 1
    px = np.frombuffer(parts[4][: w * h * ch], dtype=np.uint8).reshape(h, w, ch)
    if ch == 1:
        px = np.repeat(px, 3, axis=2)
    return px


def main():
    ours, work, ddjvu = sys.argv[1:4]
    rest = sys.argv[4:]
    os.makedirs(work, exist_ok=True)
    files = [f for f in rest if not f.startswith('--')]
    rows = []
    for f in files:
        stem = os.path.splitext(os.path.basename(f))[0]
        for name in sorted(os.listdir(ours)):
            if not name.startswith(stem + '_p') or not name.endswith('.ppm'):
                continue
            page = int(name[len(stem) + 2:-4])
            a = read_ppm(os.path.join(ours, name))
            h, w = a.shape[:2]
            ref = os.path.join(work, name)
            if not os.path.exists(ref):
                r = subprocess.run([ddjvu, '-format=ppm', f'-page={page}', f'-size={w}x{h}', '-aspect=no', f, ref],
                                   capture_output=True)
                if r.returncode != 0:
                    rows.append((stem, page, None, None, None, r.stderr.decode().strip()))
                    continue
            b = read_ppm(ref)
            if b.shape != a.shape:
                rows.append((stem, page, None, None, None, f'size {b.shape} vs {a.shape}'))
                continue
            d = np.abs(a.astype(np.int16) - b.astype(np.int16))
            over = (d.max(axis=2) > TOL).mean() * 100
            rows.append((stem, page, d.mean(), int(d.max()), over, ''))
    ok = [r for r in rows if r[2] is not None]
    passed = [r for r in ok if r[4] < 1.0]
    for r in rows:
        if r[2] is None:
            print(f'{r[0]}\tp{r[1]}\tskipped\t{r[5]}')
        else:
            print(f'{r[0]}\tp{r[1]}\tmean {r[2]:.3f}\tmax {r[3]}\tover {r[4]:.3f}%')
    if ok:
        means = [r[2] for r in ok]
        print(f'pages {len(ok)}, passed {len(passed)} ({100 * len(passed) / len(ok):.1f}%), '
              f'mean of means {np.mean(means):.3f}, worst mean {max(means):.3f}, '
              f'max {max(r[3] for r in ok)}, exact {sum(1 for r in ok if r[3] == 0)}')


main()
