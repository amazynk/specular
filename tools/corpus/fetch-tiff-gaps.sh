#!/usr/bin/env bash
# The NeXT / PixarLog / JBIG / lossless-JPEG corpus for crates/formats/specular-tiff/tests/gaps.rs,
# in $SPECULAR_TIFF_GAPS (default $CARGO_TARGET_DIR/corpus/tiff-gaps; gitignored, nothing here
# is committed).
#
# * NeXT 2-bit samples from GDAL's autotest data (MIT), pinned to v3.9.0 (147-152 bytes each).
# * PixarLog and JBIG fixtures written by the system libtiff (tools/corpus/tiff-gaps.py).
# * libtiff's decode of every file (NAME.rgba / NAME.raw / NAME.txt), the test's reference.
#
# Needs curl, the system libtiff (with JBIG-KIT) and a python with numpy ($PYTHON). Downloads
# are untrusted data: only copied, and read with `python -I`.
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_TIFF_GAPS:-${CARGO_TARGET_DIR:-$ROOT/target}/corpus/tiff-gaps}
PYTHON=${PYTHON:-python3}
mkdir -p "$OUT"
base=https://raw.githubusercontent.com/OSGeo/gdal/v3.9.0/autotest/gcore/data
for f in next_default_case next_literalrow next_literalspan; do
  [[ -s $OUT/$f.tif ]] || curl -sfL --max-filesize 100000 -o "$OUT/$f.tif" "$base/$f.tif"
done
"$PYTHON" -I "$ROOT/tools/corpus/tiff-gaps.py" generate "$OUT"
"$PYTHON" -I "$ROOT/tools/corpus/tiff-gaps.py" reference "$OUT"
