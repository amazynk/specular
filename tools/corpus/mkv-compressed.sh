#!/usr/bin/env bash
# Generate compressed Matroska files for crates/specular-video/tests/mkv_compressed.rs into
# tools/corpus/out/mkv-compressed/ (gitignored). The test skips itself when it is absent.
#
# Each Matroska fixture in crates/specular-video/tests/data is remuxed by mkvmerge two ways:
#   <name>-none.mkv   no compression (the reference: same clusters and lacing)
#   <name>-zlib.mkv   every track zlib-compressed (ContentCompAlgo 0)
# manifest.tsv lists the names. mkvmerge no longer writes plain header stripping (its
# mpeg4_p2 mode gives frames neither ffmpeg nor we decode); the unit tests in
# crates/specular-video/src/mkv/remux_tests.rs write header-stripped files themselves.
#
# Usage: tools/corpus/mkv-compressed.sh. Needs mkvmerge (MKVToolNix).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
DATA="$ROOT/crates/specular-video/tests/data"
OUT="$ROOT/tools/corpus/out/mkv-compressed"
command -v mkvmerge >/dev/null || { echo "needs mkvmerge" >&2; exit 1; }
mkdir -p "$OUT"
: > "$OUT/manifest.tsv"

for f in h264-aac h264-opus hevc mpeg2 mpeg4-asp mp3-mkv dts-mkv vp8-ac3 vp8-flac vp9-opus \
  alac-mono amr-wb msadpcm-mono snd-mka; do
  src="$DATA/$f.mkv"
  [ -f "$src" ] || src="$DATA/$f.mka"
  [ -f "$src" ] || src="$DATA/$f.webm"
  [ -f "$src" ] || { echo "missing $f" >&2; continue; }
  mkvmerge -q -o "$OUT/$f-none.mkv" --compression -1:none "$src"
  mkvmerge -q -o "$OUT/$f-zlib.mkv" --compression -1:zlib "$src"
  printf '%s\n' "$f" >> "$OUT/manifest.tsv"
done
echo "wrote $(wc -l < "$OUT/manifest.tsv") fixtures to $OUT"
