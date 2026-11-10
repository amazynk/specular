#!/usr/bin/env python3
"""Pins colour-demosaicing's bilinear Bayer demosaic for specular-core's `Cfa` tests.

Writes tests/data/debayer_oracle.tsv:

    case  <pattern>  <width>  <height>
    raw   <v,v,...>                     (row-major, top row first)
    rgb   <x>  <y>  <r,g,b>             (every pixel)

Interior pixels are `colour_demosaicing.demosaicing_CFA_Bayer_bilinear` itself. It convolves
with scipy's default edge mode ('reflect', the edge pixel repeated), which breaks the 2x2
alternation at the border. Specular mirrors about the edge pixel instead (whole-sample
symmetric), so edge pixels are pinned from the same masks and kernels convolved with
mode='mirror'. The script checks that the two agree everywhere inside.

Run with a Python that has colour-demosaicing (the lane used a venv under target/):
    target/astrowcs/venv/bin/python -I crates/specular-core/gen/debayer_oracle.py
"""

import pathlib
import warnings

import numpy as np
from scipy.ndimage import convolve

warnings.filterwarnings("ignore")
import colour_demosaicing  # noqa: E402
from colour_demosaicing import masks_CFA_Bayer  # noqa: E402

OUT = pathlib.Path(__file__).resolve().parent.parent / "tests" / "data" / "debayer_oracle.tsv"

H_G = np.array([[0, 1, 0], [1, 4, 1], [0, 1, 0]], dtype=np.float64) / 4
H_RB = np.array([[1, 2, 1], [2, 4, 2], [1, 2, 1]], dtype=np.float64) / 4


def mirrored(cfa, pattern):
    r, g, b = masks_CFA_Bayer(cfa.shape, pattern)
    return np.stack(
        [
            convolve(cfa * r, H_RB, mode="mirror"),
            convolve(cfa * g, H_G, mode="mirror"),
            convolve(cfa * b, H_RB, mode="mirror"),
        ],
        axis=-1,
    )


def main():
    rng = np.random.default_rng(20261011)
    lines = [f"# colour-demosaicing {colour_demosaicing.__version__}, bilinear"]
    for pattern in ("RGGB", "GRBG", "GBRG", "BGGR"):
        for w, h in ((8, 6), (7, 5), (11, 9)):
            cfa = rng.integers(0, 4096, size=(h, w)).astype(np.float64)
            lib = colour_demosaicing.demosaicing_CFA_Bayer_bilinear(cfa, pattern)
            mir = mirrored(cfa, pattern)
            assert np.allclose(lib[1:-1, 1:-1], mir[1:-1, 1:-1]), (pattern, w, h)
            out = mir.copy()
            out[1:-1, 1:-1] = lib[1:-1, 1:-1]
            lines.append(f"case\t{pattern}\t{w}\t{h}")
            lines.append("raw\t" + ",".join(str(int(v)) for v in cfa.ravel()))
            for y in range(h):
                for x in range(w):
                    lines.append(f"rgb\t{x}\t{y}\t" + ",".join(repr(float(v)) for v in out[y, x]))
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text("\n".join(lines) + "\n")
    print(f"wrote {OUT}")


if __name__ == "__main__":
    main()
