#!/usr/bin/env python3
"""Compare specular-xps renders with MuPDF's, page by page.

Usage: xps-oracle.py OURS_DIR WORK_DIR FILE...
  OURS_DIR holds <name>-<page>.png from the xps-render example (same DPI as below).
  MuPDF renders go to WORK_DIR/<name>-<page>.png (`mutool draw -r 96`).

Per page: mean and max absolute difference over RGB (0-255) and the share of pixels whose
largest channel difference exceeds TOL (default 48: anti-aliasing and glyph rasterisation
differ between the two renderers). A page passes when that share is under 1 %.

Run with HOME, XDG_CACHE_HOME and XDG_CONFIG_HOME pointed at a scratch directory so MuPDF's
fontconfig caches stay out of the user's home. Needs numpy and Pillow (a venv under the
CARGO_TARGET_DIR). Untrusted inputs: the script only reads them and runs mutool on them.
"""
import os
import shutil
import subprocess
import sys

import numpy as np
from PIL import Image

DPI = int(os.environ.get("DPI", "96"))
TOL = int(os.environ.get("TOL", "48"))
PASS_SHARE = float(os.environ.get("PASS_SHARE", "0.01"))


def main():
    ours, work, files = sys.argv[1], sys.argv[2], sys.argv[3:]
    os.makedirs(work, exist_ok=True)
    rows = []
    for f in files:
        name = os.path.basename(f)
        src = f
        if name.lower().endswith(".oxps"):
            # MuPDF 1.27 does not read OpenXPS (it opens the ZIP as a comic book archive, its
            # images as pages). The MinBar samples come in both flavours with the same pages:
            # the reference for an .oxps is MuPDF's render of the sibling .xps.
            twin = f[: -len(".oxps")] + ".xps"
            if not os.path.exists(twin):
                rows.append((name, 0, None, None, None, "no XPS twin for the MuPDF reference"))
                continue
            src = os.path.join(work, name + ".xps")
            shutil.copyfile(twin, src)
        for old in os.listdir(work):
            if old.startswith(name + "-") and old.endswith(".png"):
                os.remove(os.path.join(work, old))
        r = subprocess.run(
            ["mutool", "draw", "-q", "-r", str(DPI), "-o",
             os.path.join(work, name + "-%d.png"), src],
            capture_output=True, timeout=600)
        page = 1
        while True:
            theirs = os.path.join(work, f"{name}-{page}.png")
            mine = os.path.join(ours, f"{name}-{page}.png")
            if not os.path.exists(theirs):
                if page == 1:
                    rows.append((name, 0, None, None, None, "mutool failed" if r.returncode else "no pages"))
                break
            if not os.path.exists(mine):
                rows.append((name, page, None, None, None, "not rendered by us"))
                page += 1
                continue
            a = np.asarray(Image.open(theirs).convert("RGB"), dtype=np.int16)
            b = np.asarray(Image.open(mine).convert("RGB"), dtype=np.int16)
            h, w = min(a.shape[0], b.shape[0]), min(a.shape[1], b.shape[1])
            note = "" if a.shape == b.shape else f"size {a.shape[1]}x{a.shape[0]} vs {b.shape[1]}x{b.shape[0]}"
            d = np.abs(a[:h, :w] - b[:h, :w])
            worst = d.max(axis=2)
            rows.append((name, page, float(d.mean()), int(d.max()), float((worst > TOL).mean()), note))
            page += 1
    compared = [r for r in rows if r[2] is not None]
    passed = [r for r in compared if r[4] < PASS_SHARE]
    for r in sorted(compared, key=lambda r: -r[4]):
        print(f"{r[0]:40s} p{r[1]:<3d} mean {r[2]:6.2f} max {r[3]:3d} over{TOL} {100 * r[4]:6.2f}% {r[5]}")
    for r in rows:
        if r[2] is None:
            print(f"{r[0]:40s} p{r[1]:<3d} {r[5]}")
    if compared:
        print(f"pages compared {len(compared)}, passed {len(passed)} "
              f"({100 * len(passed) / len(compared):.1f} %), mean of means "
              f"{sum(r[2] for r in compared) / len(compared):.2f}, tolerance {TOL}, "
              f"pass share {100 * PASS_SHARE:.0f} %")


if __name__ == "__main__":
    main()
