#!/usr/bin/env bash
# Generate the small VP9 fixtures of crates/formats/specular-vp9/tests/data/: testsrc2 clips
# encoded with libvpx-vp9 (IVF), and the oracle's per-frame MD5s (libvpx's decoder through
# ffmpeg, `-f framemd5`, the MD5 of each frame's packed planes) as <name>.md5, one per line.
#
# Usage: tools/corpus/specular-vp9-fixtures.sh [--bench]
#   --bench  also writes the 1080p and 4K clips for examples/vp9_bench.rs into
#            tools/corpus/video2/specular-vp9/ (gitignored, not committed).
# Needs ffmpeg with libvpx-vp9. The encoder output depends on the libvpx version, so the
# committed files are the fixtures; rerunning regenerates matching pairs.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT="$ROOT/crates/formats/specular-vp9/tests/data"
mkdir -p "$OUT"
command -v ffmpeg >/dev/null || { echo "ffmpeg not found" >&2; exit 1; }

enc() { # name size frames pix_fmt extra-args...
  local name=$1 size=$2 frames=$3 pix=$4
  shift 4
  ffmpeg -nostdin -hide_banner -loglevel error -y -f lavfi -i "testsrc2=size=$size:rate=25" \
    -frames:v "$frames" -pix_fmt "$pix" -c:v libvpx-vp9 -deadline good -cpu-used 4 "$@" \
    -f ivf "$OUT/$name.ivf"
  ffmpeg -nostdin -hide_banner -loglevel error -c:v libvpx-vp9 -i "$OUT/$name.ivf" \
    -f framemd5 - | grep -v '^#' | awk -F, '{gsub(/ /, "", $6); print $6}' > "$OUT/$name.md5"
  echo "$name: $(stat -c %s "$OUT/$name.ivf") bytes, $(wc -l < "$OUT/$name.md5") frames"
}

# 8-bit 4:2:0, with alt-ref (hidden frames, superframes, compound prediction).
enc basic-176x144 176x144 20 yuv420p -b:v 300k -auto-alt-ref 1 -lag-in-frames 16
# Odd size.
enc odd-178x146 178x146 12 yuv420p -b:v 300k
# Several tile columns and rows.
enc tiles-1280x96 1280x96 6 yuv420p -b:v 600k -tile-columns 2 -tile-rows 1
# Profile 2: 10 and 12 bits.
enc p2-10bit-176x144 176x144 10 yuv420p10le -b:v 300k
enc p2-12bit-96x64 96x64 6 yuv420p12le -b:v 200k
# Lossless (Walsh-Hadamard transform).
enc lossless-96x64 96x64 4 yuv420p -lossless 1
# Profile 1: 4:2:2 and 4:4:0.
enc p1-422-176x144 176x144 8 yuv422p -b:v 300k
enc p1-440-96x64 96x64 6 yuv440p -b:v 200k
# Error resilient and frame parallel (no backward adaptation).
enc errres-176x144 176x144 10 yuv420p -b:v 300k -error-resilient 1
enc fpar-176x144 176x144 10 yuv420p -b:v 300k -frame-parallel 1
# High quality: large transforms and many coefficients.
enc hq-176x144 176x144 6 yuv420p -crf 4 -b:v 0
# 4:4:4 (profiles 1 and 3, with alt-ref), 10-bit 4:4:0 and RGB (GBR planes).
enc p1-444-64x64 64x64 2 yuv444p -b:v 100k
enc p1-444-176x144 176x144 12 yuv444p -b:v 300k -auto-alt-ref 1 -lag-in-frames 8
enc p3-444-10bit-96x64 96x64 6 yuv444p10le -b:v 200k
enc p3-444-12bit-64x64 64x64 4 yuv444p12le -b:v 200k
enc p1-gbr-96x64 96x64 6 gbrp -b:v 200k
enc p3-440-10bit-96x64 96x64 4 yuv440p10le -b:v 200k

if [ "${1:-}" = "--bench" ]; then
  BENCH="$ROOT/tools/corpus/video2/specular-vp9"
  mkdir -p "$BENCH"
  for spec in "1080p 1920x1080 60 4M" "4k 3840x2160 30 12M"; do
    set -- $spec
    ffmpeg -nostdin -hide_banner -loglevel error -y -f lavfi -i "testsrc2=size=$2:rate=30" \
      -frames:v "$3" -pix_fmt yuv420p -c:v libvpx-vp9 -deadline good -cpu-used 4 -row-mt 1 \
      -tile-columns 2 -b:v "$4" -f ivf "$BENCH/bench-$1.ivf"
    echo "bench-$1: $(stat -c %s "$BENCH/bench-$1.ivf") bytes"
  done
fi
