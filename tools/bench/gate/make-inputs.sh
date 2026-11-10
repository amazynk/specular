#!/bin/sh
# Regenerates the committed inputs of the instruction-count gate (tools/bench/perf-gate.py) for the
# formats that have no pure-Rust encoder in the workspace. The sources are the gate's own generated
# PNGs (`specular-bench gate gen DIR`). Run this only to change an input on purpose: new bytes
# change the counts, so bless the baseline again in the same commit.
#
# Made with cwebp (libwebp 1.5.0), heif-enc (libheif 1.21.2 with aom 3.13.1) and jpegtran
# (libjpeg-turbo 2.1.5).
#
#   tools/bench/gate/make-inputs.sh GENERATED_DIR
set -eu
src=${1:?usage: make-inputs.sh GENERATED_DIR (the directory written by specular-bench gate gen)}
out=$(dirname "$0")
cwebp -quiet -q 80 -m 4 "$src/png-rgb8.png" -o "$out/webp-lossy.webp"
cwebp -quiet -lossless -z 6 "$src/png-rgba8.png" -o "$out/webp-lossless.webp"
# Arithmetic-coded JPEG: a lossless 1024x768 crop of the generated 4:2:0 JPEG (libjpeg-turbo jpegtran).
jpegtran -crop 1024x768+0+0 -arithmetic -copy none "$src/jpeg-420.jpg" >"$out/jpeg-arith-420.jpg"
heif-enc --avif -q 60 -p chroma=420 -o "$out/avif-420.avif" "$src/png-rgb8.png" >/dev/null
ls -l "$out"/*.webp "$out"/*.avif "$out"/*.jpg
