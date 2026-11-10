#!/usr/bin/env python3
"""HEIC display conversion against libheif (pillow-heif), output only.

Usage: heic-oracle.py DUMP_DIR FILE...

DUMP_DIR holds what `SPECULAR_HEIC_DUMP=FILE:... SPECULAR_HEIC_DUMP_OUT=DUMP_DIR cargo test -p
specular-heic --test oracle_dump -- --ignored` wrote: `<name>.new.rgba` (chroma interpolated,
the viewer's conversion), `<name>.old.rgba` (chroma replicated) and `<name>.size`. Prints, per
file, the max and mean absolute difference of R, G, B (and A) from libheif's 8-bit RGB(A),
for both, and the share of samples more than 2 codes off.
"""

import os
import sys

import numpy as np
import pillow_heif


def main():
    out, files = sys.argv[1], sys.argv[2:]
    print(f"{'file':28} {'size':>11}  {'new max/mean/>2':>20}  {'old max/mean/>2':>20}")
    for path in files:
        name = os.path.basename(path)
        h = pillow_heif.open_heif(path, convert_hdr_to_8bit=True)
        ref = np.asarray(h)
        w, hh = (int(v) for v in open(os.path.join(out, name + ".size")).read().split())
        if ref.shape[:2] != (hh, w):
            print(f"{name:28} size {w}x{hh} against libheif {ref.shape[1]}x{ref.shape[0]}")
            continue
        ch = ref.shape[2] if ref.ndim == 3 else 1
        if ch == 1:
            ref = np.repeat(ref[:, :, None], 3, axis=2)
            ch = 3
        cols = []
        for kind in ("new", "old"):
            got = np.fromfile(os.path.join(out, f"{name}.{kind}.rgba"), dtype=np.uint8)
            got = got.reshape(hh, w, 4)[:, :, :ch].astype(np.int32)
            d = np.abs(got - ref.astype(np.int32))
            cols.append(f"{d.max():4d} {d.mean():7.3f} {100 * (d > 2).mean():6.2f}%")
        print(f"{name:28} {w:5d}x{hh:<5d}  {cols[0]:>20}  {cols[1]:>20}")


if __name__ == "__main__":
    main()
