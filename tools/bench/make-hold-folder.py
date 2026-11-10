#!/usr/bin/env python3
"""Synthetic folder for the hold-→ bench: N mixed images (default
1000), 2-24 MP, about 60% JPEG (4:2:0, q88), 20% PNG and 20% lossy WebP, in one directory.

    nice python3 -I tools/bench/make-hold-folder.py target/x/hold1000 [N] [JOBS]

The content is smooth (gradients, flat shapes, light noise) so the folder stays small on disk
(about 1 GB for 1000 files) while every decode still costs what its pixel count costs. Each file
is a pure function of its index. Stops (exit 2) when free disk on the target's filesystem drops
below 8 GB or available RAM below 2 GB. Re-running skips files that already exist.
"""

import os
import shutil
import sys
from multiprocessing import Pool

import numpy as np
from PIL import Image

MIN_DISK = 8 << 30
MIN_RAM = 2 << 30
# (width, height) by megapixels, 4:3 and 3:2.
SIZES = [
    (1632, 1224),  # 2 MP
    (3000, 2000),  # 6 MP
    (4000, 3000),  # 12 MP
    (4896, 3264),  # 16 MP
    (5472, 3648),  # 20 MP
    (6000, 4000),  # 24 MP
]


def avail_ram():
    with open("/proc/meminfo") as f:
        for line in f:
            if line.startswith("MemAvailable:"):
                return int(line.split()[1]) << 10
    return 1 << 62


def plan(i):
    rng = np.random.default_rng(1_000_003 * i + 7)
    w, h = SIZES[int(rng.integers(0, len(SIZES)))]
    if rng.random() < 0.3:
        w, h = h, w  # portrait
    r = rng.random()
    ext = "jpg" if r < 0.6 else ("png" if r < 0.8 else "webp")
    return rng, w, h, ext


def render(rng, w, h):
    """uint8 RGB, built in row bands to keep memory near one copy of the output."""
    out = np.empty((h, w, 3), np.uint8)
    a = rng.uniform(0.0, 6.28, 3)
    f = rng.uniform(0.5, 3.0, 3) / max(w, h)
    base = rng.uniform(40, 200, 3)
    x = np.arange(w, dtype=np.float32)
    band = 512
    for y0 in range(0, h, band):
        y = np.arange(y0, min(h, y0 + band), dtype=np.float32)[:, None]
        for c in range(3):
            v = base[c] + 50 * np.sin(x[None, :] * f[c] * 6.28 + y * f[(c + 1) % 3] * 3.14 + a[c])
            out[y0 : y0 + len(y), :, c] = np.clip(v, 0, 255).astype(np.uint8)
    for _ in range(12):
        bw, bh = int(rng.integers(w // 20, w // 3)), int(rng.integers(h // 20, h // 3))
        x0, y0 = int(rng.integers(0, w - bw)), int(rng.integers(0, h - bh))
        out[y0 : y0 + bh, x0 : x0 + bw] = rng.integers(0, 256, 3, dtype=np.uint8)
    # A little texture so the decoders see real coefficients (in tiles, to keep it cheap).
    noise = rng.integers(-4, 5, (256, 256, 3), dtype=np.int16)
    for y0 in range(0, h, 256):
        for x0 in range(0, w, 1024):
            blk = out[y0 : y0 + 256, x0 : x0 + 256]
            blk[:] = np.clip(blk.astype(np.int16) + noise[: blk.shape[0], : blk.shape[1]], 0, 255)
    return out


def make(args):
    out, start, stop = args
    for i in range(start, stop):
        rng, w, h, ext = plan(i)
        path = os.path.join(out, f"img{i:04d}.{ext}")
        if os.path.exists(path):
            continue
        if shutil.disk_usage(out).free < MIN_DISK or avail_ram() < MIN_RAM:
            return -1
        im = Image.fromarray(render(rng, w, h))
        tmp = path + ".part"
        if ext == "jpg":
            im.save(tmp, "JPEG", quality=88, subsampling=2)
        elif ext == "png":
            im.save(tmp, "PNG", compress_level=3)
        else:
            im.save(tmp, "WEBP", quality=80, method=2)
        os.replace(tmp, path)
    return stop - start


def main():
    out = sys.argv[1]
    n = int(sys.argv[2]) if len(sys.argv) > 2 else 1000
    jobs = int(sys.argv[3]) if len(sys.argv) > 3 else 4
    os.makedirs(out, exist_ok=True)
    chunks = [(out, s, min(n, s + 10)) for s in range(0, n, 10)]
    with Pool(jobs) as p:
        for r in p.imap_unordered(make, chunks):
            if r < 0:
                print("stopping: low disk or RAM", file=sys.stderr)
                p.terminate()
                sys.exit(2)
    print(f"{n} files in {out}")


if __name__ == "__main__":
    main()
