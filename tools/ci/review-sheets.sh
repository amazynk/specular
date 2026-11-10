#!/usr/bin/env bash
# Visual review sheets: our decoded output next to a reference decoder's, per format group, as
# PNG sheets and an HTML index in target/review (never committed). See the header of
# tools/ci/review-sheets.py for what is sampled, which references are used and how to read
# the sheets.
#
#   tools/ci/review-sheets.sh [--long PX] [--per-format N] [--jobs N] [--only GROUP,...]
#                             [--root DIR]... [--out DIR] [--no-probe-cache]
#
# Needs the usual cargo environment (CARGO_TARGET_DIR, RUSTC_WRAPPER, ...). Makes the venv
# $CARGO_TARGET_DIR/venv-review on first use (pip: numpy pillow rawpy pydicom imagecodecs
# tifffile pillow-heif), builds `review_decode` (specular-convert example, dev profile) and
# uses ffmpeg, ImageMagick, pdftoppm and inkscape from PATH when present. A rerun over the
# fixtures takes well under a minute; probes are cached in the output folder.
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
TARGET=${CARGO_TARGET_DIR:-$ROOT/target}
VENV=${REVIEW_VENV:-$TARGET/venv-review}
if [[ ! -x $VENV/bin/python ]]; then
  python3 -m venv "$VENV"
  "$VENV/bin/pip" install -q --no-cache-dir numpy pillow rawpy pydicom imagecodecs tifffile \
    pillow-heif
fi
cargo build --offline -q -p specular-convert --example review_decode >&2
exec "$VENV/bin/python" -I "$ROOT/tools/ci/review-sheets.py" \
  --exe "$TARGET/debug/examples/review_decode" "$@"
