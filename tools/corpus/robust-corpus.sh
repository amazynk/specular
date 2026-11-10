#!/usr/bin/env bash
# Generate the JPEG robustness corpus (tools/corpus/out/robust, gitignored): ~5,500 mutated
# JPEGs (bit flips, truncations, deleted / duplicated / inserted chunks, stray markers, edited
# segment lengths and header fields, splices) read by
# crates/formats/specular-jpeg/tests/robust_corpus.rs.
#
#   tools/corpus/robust-corpus.sh [OUT_DIR]     # OUT_DIR is replaced
#
# The mutants come from the built-in seeds of crates/formats/specular-jpeg/examples/robust.rs
# (with `cjpeg` on PATH also arithmetic coding and libjpeg-turbo progressive scripts) plus four
# small files from the synthetic corpus generator (EXIF orientation 1 and 6, an absurd and a
# zero SOF size), made in a scratch directory under target/. That step needs cjpeg, ImageMagick
# (`magick`) and exiftool; without them only the built-in seeds are used. Everything is
# deterministic, so CI caches the output keyed on this script and the two generators.
#
# Both generators are cheap, so they run in the dev profile (no fat-LTO release build).
# Environment: CARGO_TARGET_DIR is honoured; ROBUST_PER_SEED (default 200 here) sets the mutants
# per small seed.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
out=${1:-$root/tools/corpus/out/robust}
target=${CARGO_TARGET_DIR:-$root/target}
scratch=$target/robust-seeds
cd "$root"

seeds=()
if command -v cjpeg >/dev/null && command -v magick >/dev/null && command -v exiftool >/dev/null; then
  rm -rf "$scratch"
  for only in broken/absurd-65535x65535.jpg broken/zero-height.jpg \
    orientation/orientation-1.jpg orientation/orientation-6.jpg; do
    cargo run --locked -q -p specular-corpus -- --small --no-network \
      --out "$scratch" --only "$only"
    seeds+=("$scratch/$only")
  done
else
  echo "robust-corpus: cjpeg, magick or exiftool missing; built-in seeds only" >&2
fi

rm -rf "$out"
ROBUST_PER_SEED=${ROBUST_PER_SEED:-200} cargo run --locked -q -p specular-jpeg --example robust -- gen "$out" "${seeds[@]}"
