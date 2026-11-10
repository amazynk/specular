#!/usr/bin/env python3
"""Synthetic folder for the gallery scroll bench: N distinct small JPEGs (default 100 000 at
320x240) in one directory, so every file has its own thumbnail cache key.

    nice python3 -I tools/bench/make-scroll-folder.py target/x/bench100k [N] [JOBS]

Stops (exit 2) when free disk on the target's filesystem drops below 8 GB or available RAM
below 2 GB. Re-running skips files that already exist. Delete the folder when done.
"""

import os
import shutil
import sys
from multiprocessing import Pool

import numpy as np
from PIL import Image

W, H = 320, 240
MIN_DISK = 8 << 30
MIN_RAM = 2 << 30


def avail_ram():
    with open("/proc/meminfo") as f:
        for line in f:
            if line.startswith("MemAvailable:"):
                return int(line.split()[1]) << 10
    return 1 << 62


def make(args):
    out, start, stop = args
    yy, xx = np.mgrid[0:H, 0:W].astype(np.float32)
    for i in range(start, stop):
        path = os.path.join(out, f"img{i:06d}.jpg")
        if os.path.exists(path):
            continue
        rng = np.random.default_rng(i)
        a, b, c = rng.uniform(0.0, 6.28, 3)
        f = rng.uniform(0.005, 0.05, 3)
        img = np.stack(
            [
                127 + 120 * np.sin(xx * f[0] + yy * f[1] * 0.5 + a),
                127 + 120 * np.sin(yy * f[1] + xx * f[2] * 0.3 + b),
                127 + 120 * np.sin((xx + yy) * f[2] + c),
            ],
            axis=-1,
        )
        for _ in range(4):
            x0, y0 = int(rng.integers(0, W - 40)), int(rng.integers(0, H - 40))
            img[y0 : y0 + int(rng.integers(10, 80)), x0 : x0 + int(rng.integers(10, 80))] = rng.uniform(0, 255, 3)
        img += rng.normal(0, 6, img.shape)
        Image.fromarray(np.clip(img, 0, 255).astype(np.uint8)).save(path, quality=85)
    return stop - start


def main():
    out = sys.argv[1]
    n = int(sys.argv[2]) if len(sys.argv) > 2 else 100_000
    jobs = int(sys.argv[3]) if len(sys.argv) > 3 else 4
    os.makedirs(out, exist_ok=True)
    step = 1000
    chunks = [(out, s, min(s + step, n)) for s in range(0, n, step)]
    done = 0
    with Pool(jobs) as pool:
        for k in pool.imap_unordered(make, chunks):
            done += k
            disk = shutil.disk_usage(out).free
            ram = avail_ram()
            if done % 10_000 < step:
                print(f"{done}/{n}: disk free {disk >> 30} GB, RAM available {ram >> 30} GB", flush=True)
            if disk < MIN_DISK or ram < MIN_RAM:
                print(f"stopping: disk free {disk >> 30} GB, RAM available {ram >> 30} GB", flush=True)
                pool.terminate()
                sys.exit(2)
    print(f"done: {n} files in {out}")


if __name__ == "__main__":
    main()
