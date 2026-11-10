#!/usr/bin/env python3
"""pyimzML's reading of imzML data sets, as the oracle for specular-imzml (tests/oracle.rs).
pyimzML (Apache-2.0) is only ever run here, from a venv; it is never linked.

    python3 -I oracle.py DIR OUT.tsv [MAX_SPECTRA [MAX_VALUES]]

Every *.imzML under DIR (with its .ibd) is read with pyimzML.ImzMLParser. Rows (tab separated):
  file   name spectra max_x max_y max_z polarity spectrum_mode mz_dtype int_dtype
  coord  name i x y z                   (every spectrum, 1-based as the file gives them)
  spec   name i n mz_sum int_sum first_mz last_mz   (float64 sums of the arrays as read)
  val    name i k mz int                (up to MAX_VALUES values of each listed spectrum)
  tic    name i sum                     (float64 sum of the intensities)
  win    name w mz tol_da               (the ion windows: mz ± tol_da, both ends included)
  ion    name w i sum pyimzml_sum       (float64 sum in window w: from numpy.searchsorted
                                         over pyimzML's arrays, and from pyimzML's own
                                         _bisect_spectrum indices as getionimage uses them)
  error  name words                     (pyimzML failed, or the arrays are compressed)

At most MAX_SPECTRA spectra (evenly strided; default all) get spec, val, tic and ion rows.
"""

import glob
import os
import sys

import numpy as np
import pyimzml
from pyimzml.ImzMLParser import ImzMLParser, _bisect_spectrum

COMPRESSION = ("zlib compression", "MS-Numpress")

# The fixtures' ions (gen/make_imzml.py) and a window over nothing.
FIXTURE_WINDOWS = [(184.0733, 0.01), (369.3516, 369.3516 * 10e-6), (520.3398, 0.25),
                   (760.5851, 760.5851 * 5e-6), (333.0, 0.001)]


def fmt(x):
    return repr(float(x))


def windows_for(p, n):
    """Corpus windows: the base peaks of the first and middle spectra, at 0.01 Da and
    5 ppm, and a 0.5 Da window at the middle of the m/z range."""
    out = []
    for i in sorted({0, n // 2}):
        mzs, ints = p.getspectrum(i)
        if len(ints):
            m = float(mzs[int(np.argmax(ints))])
            out += [(m, 0.01), (m, m * 5e-6)]
    mzs, _ = p.getspectrum(n // 2)
    if len(mzs):
        out.append((float((mzs[0] + mzs[-1]) / 2), 0.5))
    return out


def main():
    root, out_path = sys.argv[1], sys.argv[2]
    max_spectra = int(sys.argv[3]) if len(sys.argv) > 3 else 0
    max_values = int(sys.argv[4]) if len(sys.argv) > 4 else 64
    rows = [f"# Written by gen/oracle.py (pyimzML {pyimzml.__version__ if hasattr(pyimzml, '__version__') else '1.5'}, "
            f"numpy {np.__version__}); do not edit."]
    paths = glob.glob(os.path.join(root, "**", "*.*"), recursive=True)
    for path in sorted(p for p in paths if p.lower().endswith(".imzml")):
        name = os.path.relpath(path, root)
        with open(path, "rb") as f:
            head = f.read(1 << 20).decode("latin-1")
        if any(c in head for c in COMPRESSION):
            rows.append(f"error\t{name}\tcompressed")
            continue
        try:
            p = ImzMLParser(path)
        except Exception as e:  # noqa: BLE001 - any failure is a row
            rows.append(f"error\t{name}\t{type(e).__name__}: {e}".replace("\n", " "))
            continue
        with p:
            coords = np.asarray(p.coordinates)
            n = len(coords)
            rows.append("\t".join(["file", name, str(n), str(coords[:, 0].max()),
                                   str(coords[:, 1].max()), str(coords[:, 2].max()),
                                   str(p.polarity), str(p.spectrum_mode),
                                   p.mzPrecision, p.intensityPrecision]))
            for i, (x, y, z) in enumerate(p.coordinates):
                rows.append(f"coord\t{name}\t{i}\t{x}\t{y}\t{z}")
            pick = range(n) if not max_spectra or n <= max_spectra else \
                sorted({int(k * (n - 1) / (max_spectra - 1)) for k in range(max_spectra)})
            wins = FIXTURE_WINDOWS if "tests/data" in os.path.abspath(path) or \
                os.path.basename(root) == "data" else windows_for(p, n)
            for w, (m, t) in enumerate(wins):
                rows.append(f"win\t{name}\t{w}\t{fmt(m)}\t{fmt(t)}")
            for i in pick:
                mzs, ints = p.getspectrum(i)
                mzs = np.asarray(mzs)
                ints = np.asarray(ints)
                k = len(mzs)
                rows.append("\t".join(["spec", name, str(i), str(k),
                                       fmt(np.sum(mzs, dtype=np.float64)),
                                       fmt(np.sum(ints, dtype=np.float64)),
                                       fmt(mzs[0]) if k else "nan",
                                       fmt(mzs[-1]) if k else "nan"]))
                step = max(1, k // max_values) if max_values else 1
                for j in range(0, k, step):
                    rows.append(f"val\t{name}\t{i}\t{j}\t{fmt(mzs[j])}\t{fmt(ints[j])}")
                rows.append(f"tic\t{name}\t{i}\t{fmt(np.sum(ints, dtype=np.float64))}")
                for w, (m, t) in enumerate(wins):
                    a = int(np.searchsorted(mzs, m - t, side="left"))
                    b = int(np.searchsorted(mzs, m + t, side="right"))
                    s = float(np.sum(ints[a:b], dtype=np.float64))
                    if k:
                        lo, hi = _bisect_spectrum(mzs, m, t)
                        ps = float(np.sum(ints[lo:hi + 1], dtype=np.float64))
                    else:
                        ps = 0.0
                    rows.append(f"ion\t{name}\t{w}\t{i}\t{fmt(s)}\t{fmt(ps)}")
    with open(out_path, "w") as f:
        f.write("\n".join(rows) + "\n")


if __name__ == "__main__":
    main()
