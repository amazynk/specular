"""numpy oracle for specular-fft's power spectrum.

Writes deterministic test planes and numpy's centred log10 power spectrum of each into
crates/specular-fft/tests/data (little-endian f32, row-major, rows top-down):

    NAME.in.f32    the plane
    NAME.out.f32   log10(|fftshift(fft2(plane))|^2), NaN where the power is 0
    cases.txt      "NAME WIDTH HEIGHT" per line

NaN samples in a plane are replaced by the mean of the others before the transform, as the
viewer does. Run: python -I crates/specular-fft/gen/oracle.py (needs numpy).
"""

import os
import sys

import numpy as np

OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "tests", "data")


def lattice(w, h, rng):
    """A tilted lattice with noise: sharp peaks off the axes, like an HRTEM image."""
    y, x = np.mgrid[0:h, 0:w].astype(np.float64)
    a = np.cos(2 * np.pi * (0.11 * x + 0.05 * y)) + 0.5 * np.cos(2 * np.pi * (-0.07 * x + 0.13 * y))
    return a + 0.3 * rng.standard_normal((h, w)) + 3.0


CASES = [
    # name, width, height, generator
    ("pow2_64x48", 64, 48, lambda w, h, r: r.standard_normal((h, w))),
    ("mixed_35x30", 35, 30, lattice),
    ("bluestein_67x20", 67, 20, lattice),
    ("prime_rows_16x97", 16, 97, lambda w, h, r: r.uniform(0, 1000, (h, w))),
    ("odd_45x77", 45, 77, lattice),
    ("nan_40x24", 40, 24, lattice),
    ("row_128x1", 128, 1, lambda w, h, r: r.standard_normal((h, w))),
]


def main():
    os.makedirs(OUT, exist_ok=True)
    rng = np.random.default_rng(20261011)
    lines = []
    for name, w, h, gen in CASES:
        a = gen(w, h, rng).astype(np.float32)
        if name.startswith("nan"):
            a[3, 5] = np.nan
            a[10, 0:7] = np.nan
        a.astype("<f4").tofile(os.path.join(OUT, name + ".in.f32"))
        b = a.astype(np.float64)
        b[~np.isfinite(b)] = np.nanmean(b)
        p = np.abs(np.fft.fftshift(np.fft.fft2(b))) ** 2
        with np.errstate(divide="ignore"):
            lp = np.where(p > 0, np.log10(p), np.nan)
        lp.astype("<f4").tofile(os.path.join(OUT, name + ".out.f32"))
        lines.append(f"{name} {w} {h}")
    with open(os.path.join(OUT, "cases.txt"), "w") as f:
        f.write("\n".join(lines) + "\n")
    print(f"{len(lines)} cases -> {OUT}", file=sys.stderr)


if __name__ == "__main__":
    main()
