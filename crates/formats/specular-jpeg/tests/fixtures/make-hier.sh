#!/bin/sh
# Hierarchical JPEG fixtures (T.81 Annex J) for tests/hier.rs, with their expected decodes.
#
# Encoder and oracle: the ISO reference libjpeg command-line tool `jpeg` by Thomas Richter
# (https://github.com/thorfdbg/libjpeg, GPL-3; run as a tool, never linked). It is the only
# freely available codec with hierarchical mode. Build it outside the tree (./configure && make
# final) and pass its path:
#
#   sh make-hier.sh /path/to/jpeg PYTHON
#
# PYTHON needs numpy (for the synthetic source images). Each NAME.jpg gets NAME.pgm / NAME.ppm:
# the reference tool's decode of it. rgb_420_flat is the non-hierarchical twin of rgb_y2_420.
set -eu
J=$1
PY=$2
D=$(dirname "$0")/hier
mkdir -p "$D"
T=$(mktemp -d "${TMPDIR:-.}/hier.XXXXXX")
trap 'rm -rf "$T"' EXIT
"$PY" -I - "$T" <<'EOF'
import sys
import numpy as np

def write(path, w, h, ch, seed, depth=255):
    rng = np.random.default_rng(seed)
    y, x = np.mgrid[0:h, 0:w]
    planes = []
    for c in range(ch):
        v = 0.5 + 0.35 * np.sin(x / (7.0 + 3 * c) + y / 11.0) * np.cos(y / (5.0 + c))
        v = v * depth + rng.normal(0, depth / 20, (h, w))
        planes.append(np.clip(np.rint(v), 0, depth))
    img = np.stack(planes, -1) if ch == 3 else planes[0]
    dt = ">u2" if depth > 255 else "u1"
    with open(path, "wb") as f:
        f.write((b"P6\n" if ch == 3 else b"P5\n") + f"{w} {h}\n{depth}\n".encode())
        f.write(img.astype(dt).tobytes())

d = sys.argv[1]
write(f"{d}/grey.pgm", 77, 53, 1, 1)
write(f"{d}/grey_big.pgm", 131, 97, 1, 2)
write(f"{d}/rgb.ppm", 61, 45, 3, 3)
write(f"{d}/grey12.pgm", 45, 38, 1, 4, 4095)
EOF
enc() { name=$1; src=$2; ext=$3; shift 3; "$J" "$@" "$T/$src" "$D/$name.jpg" >/dev/null; "$J" "$D/$name.jpg" "$D/$name.$ext" >/dev/null; }
enc grey_y2 grey.pgm pgm -q 85 -h -y 2
enc grey_y3_arith grey_big.pgm pgm -q 80 -a -y 3
enc grey_y2_prog grey.pgm pgm -q 90 -h -v -y 2
enc grey_y0_lossless grey.pgm pgm -q 75 -h -y 0
enc rgb_y2_nocolour rgb.ppm ppm -q 85 -h -c -y 2
enc rgb_y2_420 rgb.ppm ppm -q 85 -h -s 1x1,2x2,2x2 -y 2
enc grey12_y2 grey12.pgm pgm -q 85 -h -y 2
enc rgb_420_flat rgb.ppm ppm -q 85 -h -s 1x1,2x2,2x2
