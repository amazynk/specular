# Writes the LAZ test fixtures in tests/data/ (see tests/data/README): for each of LAS point
# formats 1, 3, 6, 7 and 8 a pN.las and a pN.laz holding the same 2500 synthetic points.
# The .laz is written by laspy with the lazrs backend; the .las is the uncompressed twin
# that the LAZ reader is checked against (laz_matches_its_las in src/tests.rs).
#
# The cloud is an airborne-style strip scan: 7 scan lines of up to 400 points (the last has
# 100), each line wobbling across track, over a gently rolling terrain around z = 120 m,
# at UTM-like coordinates (offset 500000, 4100000; scale 0.01, 0.01, 0.001). The fields are
# chosen to exercise every LASzip item:
#
#   intensity         random 0..3999, zero on every 7th point
#   returns           number 1..5 (formats 1, 3) or 1..8 (6, 7, 8), return number <= number
#   classification    1 2 3 5 6 7 9 (formats 1, 3), 1 2 3 5 6 9 17 (6, 7, 8)
#   scan angle        -30..29 degrees (rank), or -3000..2999 in 0.006 degree units (6, 7, 8)
#   user data         i % 3
#   point source id   i // 900
#   GPS time          1e6 + i * 1e-4, plus 5 s for every 700 points (time jumps)
#   scanner channel   (i // 250) % 4 (formats 6, 7, 8)
#   RGB               red random (8-bit times 257), green 257 * (i % 256), blue 40000 for
#                     the first half then 1000 (formats 3, 7, 8)
#   NIR               random 16-bit (format 8)
#
# The values come from a seeded generator, so a rerun gives the same files for a given
# laspy/lazrs/numpy. The committed fixtures were written by laspy 2.7.0 with an earlier
# version of this generator, so a rerun does not reproduce their exact random values (the
# layout, fields and ranges are the same). The tests only compare each .laz with its own
# .las, so regenerated files test the same way.
#
# Usage: PYTHON -I make_laz_fixtures.py [OUT_DIR]   (laspy[lazrs], numpy)
#        OUT_DIR defaults to ../tests/data next to this script.
import os
import sys

import laspy
import numpy as np

N = 2500
LINE = 400
SEED = 20261011

FORMATS = {1: "1.2", 3: "1.2", 6: "1.4", 7: "1.4", 8: "1.4"}


def cloud(fmt, version):
    rng = np.random.default_rng(SEED + fmt)
    i = np.arange(N)
    line, j = i // LINE, i % LINE
    phase = rng.uniform(0, 2 * np.pi, line.max() + 1)[line]

    x = 3.0 * line + 0.37 * j + rng.uniform(-0.07, 0.07, N)
    y = 2.0 * line + 1.45 * np.sin(2 * np.pi * j / 125 + phase) + rng.uniform(-0.05, 0.05, N)
    z = (
        120.0
        + 8.0 * np.sin(2 * np.pi * x / 220)
        + 3.0 * np.sin(1.7 * y + 0.4 * x)
        + rng.normal(0, 0.15, N)
    )

    header = laspy.LasHeader(point_format=fmt, version=version)
    header.offsets = np.array([500000.0, 4100000.0, 0.0])
    header.scales = np.array([0.01, 0.01, 0.001])
    las = laspy.LasData(header)
    las.x = 500000.0 + x
    las.y = 4100000.0 + y
    las.z = z

    intensity = rng.integers(0, 4000, N)
    intensity[i % 7 == 0] = 0
    las.intensity = intensity

    extended = fmt >= 6
    nret = rng.integers(1, (8 if extended else 5) + 1, N)
    las.number_of_returns = nret
    las.return_number = 1 + (rng.random(N) * nret).astype(np.int64)

    classes = [1, 2, 3, 5, 6, 9, 17] if extended else [1, 2, 3, 5, 6, 7, 9]
    las.classification = rng.choice(classes, N)
    if extended:
        las.scan_angle = rng.integers(-3000, 3000, N)
        las.scanner_channel = (i // 250) % 4
    else:
        las.scan_angle_rank = rng.integers(-30, 30, N)
    las.user_data = i % 3
    las.point_source_id = i // 900
    las.gps_time = 1e6 + i * 1e-4 + 5.0 * (i // 700)

    if fmt in (3, 7, 8):
        las.red = 257 * rng.integers(0, 256, N)
        las.green = 257 * (i % 256)
        las.blue = np.where(i < N // 2, 40000, 1000)
    if fmt == 8:
        las.nir = rng.integers(0, 65536, N)
    return las


def main():
    here = os.path.dirname(os.path.abspath(__file__))
    out = sys.argv[1] if len(sys.argv) > 1 else os.path.join(here, "..", "tests", "data")
    os.makedirs(out, exist_ok=True)
    for fmt, version in FORMATS.items():
        las = cloud(fmt, version)
        las_path = os.path.join(out, f"p{fmt}.las")
        laz_path = os.path.join(out, f"p{fmt}.laz")
        las.write(las_path)
        las.write(laz_path, laz_backend=laspy.LazBackend.Lazrs)
        # The LAZ must decode to exactly the LAS points.
        a = laspy.read(las_path)
        b = laspy.read(laz_path, laz_backend=laspy.LazBackend.Lazrs)
        assert a.header.point_count == b.header.point_count == N, fmt
        assert a.points.array.tobytes() == b.points.array.tobytes(), fmt
        print(f"p{fmt}.las / p{fmt}.laz  LAS {version} point format {fmt}  {N} points")


if __name__ == "__main__":
    main()
