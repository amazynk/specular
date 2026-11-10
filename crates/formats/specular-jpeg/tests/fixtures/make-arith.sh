#!/usr/bin/env bash
# Regenerates the arithmetic-coded fixtures (arith_*.jpg) with libjpeg-turbo's cjpeg as the
# reference encoder. Each stays under 64 KiB so the fuzz seed glob picks it up too.
# Needs: magick (ImageMagick 7), cjpeg. Run from anywhere; writes next to this script.
set -euo pipefail
cd "$(dirname "$0")"
src=$(mktemp -p . arith-src.XXXXXX.ppm)
trap 'rm -f "$src"' EXIT
magick -seed 7 -size 203x141 plasma:fractal -blur 0x0.7 -attenuate 0.3 +noise Gaussian \
  -fill none -stroke black -strokewidth 2 -draw 'line 0,0 101,141' \
  -stroke white -draw 'circle 67,47 108,47' "ppm:$src"
enc() {
  local out=$1
  shift
  cjpeg -arithmetic "$@" "$src" >"$out"
}
enc arith_420.jpg -quality 85 -sample 2x2
enc arith_444_rst.jpg -quality 85 -sample 1x1 -restart 2
enc arith_gray.jpg -quality 85 -grayscale
enc arith_420_prog.jpg -quality 85 -sample 2x2 -progressive
enc arith_444_prog_rst.jpg -quality 85 -sample 1x1 -progressive -restart 1B
ls -l arith_*.jpg
