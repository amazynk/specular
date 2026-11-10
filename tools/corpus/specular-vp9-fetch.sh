#!/usr/bin/env bash
# Fetch a representative set of the libvpx VP9 test vectors (vp90-2-*, vp91-2-*, vp92-2-*,
# vp93-2-*) with their .md5 files (libvpx's per-frame MD5 of the decoded planes) into
# tools/corpus/video2/specular-vp9/ (gitignored), remuxed to IVF with ffmpeg so the tests read
# IVF only, for crates/formats/specular-vp9/tests/conformance.rs. Also fetches the VP9
# specification PDF into target/video2/specs/ (input of specular-vp9-tables.py).
#
# Usage: tools/corpus/specular-vp9-fetch.sh [--clean]. Needs curl and ffmpeg.
# SPECULAR_VP9_VECTORS overrides the output folder (the tests read the same variable);
# ONLY=<substring> fetches just the vectors whose names contain it (e.g. ONLY=vp9[13]).
# Downloads are untrusted data: nothing from them is run. Stops below MIN_FREE_GB (default 8).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT="${SPECULAR_VP9_VECTORS:-$ROOT/tools/corpus/video2/specular-vp9}"
BASE=https://storage.googleapis.com/downloads.webmproject.org/test_data/libvpx
SPEC_URL=https://storage.googleapis.com/downloads.webmproject.org/docs/vp9/vp9-bitstream-specification-v0.7-20170222-draft.pdf
MIN_FREE_GB=${MIN_FREE_GB:-8}
if [ "${1:-}" = "--clean" ]; then rm -rf "$OUT"; echo "removed $OUT"; exit 0; fi
for t in curl ffmpeg; do command -v $t >/dev/null || { echo "$t not found" >&2; exit 1; }; done
mkdir -p "$OUT/src"

# The specification, next to the main checkout's target/ when run from a worktree.
SPECDIR="$ROOT/target/video2/specs"
d="$ROOT"
while [ "$d" != / ]; do
  if [ -d "$d/target/video2" ]; then SPECDIR="$d/target/video2/specs"; break; fi
  d=$(dirname "$d")
done
mkdir -p "$SPECDIR"
[ -s "$SPECDIR/$(basename "$SPEC_URL")" ] || curl -sSfL -o "$SPECDIR/$(basename "$SPEC_URL")" "$SPEC_URL"

VECTORS="
vp90-2-00-quantizer-00.webm vp90-2-00-quantizer-16.webm vp90-2-00-quantizer-32.webm
vp90-2-00-quantizer-48.webm vp90-2-00-quantizer-63.webm
vp90-2-01-sharpness-1.webm vp90-2-01-sharpness-4.webm vp90-2-01-sharpness-7.webm
vp90-2-02-size-08x08.webm vp90-2-02-size-08x10.webm vp90-2-02-size-10x08.webm
vp90-2-02-size-16x16.webm vp90-2-02-size-18x18.webm vp90-2-02-size-32x32.webm
vp90-2-02-size-34x34.webm vp90-2-02-size-64x64.webm vp90-2-02-size-66x66.webm
vp90-2-02-size-08x66.webm vp90-2-02-size-66x08.webm vp90-2-02-size-130x132.webm
vp90-2-02-size-132x130.webm vp90-2-02-size-178x180.webm vp90-2-02-size-lf-1920x1080.webm
vp90-2-03-size-196x196.webm vp90-2-03-size-198x202.webm vp90-2-03-size-202x210.webm
vp90-2-03-size-210x226.webm vp90-2-03-size-226x226.webm vp90-2-03-deltaq.webm
vp90-2-05-resize.ivf vp90-2-06-bilinear.webm
vp90-2-07-frame_parallel.webm vp90-2-07-frame_parallel-1.webm
vp90-2-08-tile_1x2_frame_parallel.webm vp90-2-08-tile_1x2.webm vp90-2-08-tile_1x4.webm
vp90-2-08-tile_1x8_frame_parallel.webm vp90-2-08-tile-4x4.webm vp90-2-08-tile-4x1.webm
vp90-2-09-aq2.webm vp90-2-09-lf_deltas.webm vp90-2-09-subpixel.webm
vp90-2-10-show-existing-frame.webm vp90-2-10-show-existing-frame2.webm
vp90-2-11-size-351x287.webm vp90-2-11-size-351x288.webm vp90-2-11-size-352x287.webm
vp90-2-12-droppable_1.ivf vp90-2-12-droppable_2.ivf vp90-2-12-droppable_3.ivf
vp90-2-13-largescaling.webm
vp90-2-14-resize-fp-tiles-1-2.webm vp90-2-14-resize-fp-tiles-2-1.webm
vp90-2-14-resize-fp-tiles-4-8.webm vp90-2-14-resize-fp-tiles-16-8.webm
vp90-2-14-resize-10frames-fp-tiles-1-2.webm vp90-2-14-resize-10frames-fp-tiles-8-4.webm
vp90-2-15-segkey.webm vp90-2-15-segkey_adpq.webm
vp90-2-16-intra-only.webm vp90-2-17-show-existing-frame.webm vp90-2-18-resize.ivf
vp90-2-19-skip.webm vp90-2-19-skip-01.webm vp90-2-19-skip-02.webm
vp90-2-20-big_superframe-01.webm vp90-2-20-big_superframe-02.webm
vp90-2-21-resize_inter_320x180_5_1-2.webm vp90-2-21-resize_inter_320x180_7_3-4.webm
vp90-2-21-resize_inter_640x360_5_3-4.webm vp90-2-21-resize_inter_640x360_7_1-2.webm
vp90-2-21-resize_inter_1280x720_5_1-2.webm vp90-2-21-resize_inter_320x240_5_1-2.webm
vp90-2-22-svc_1280x720_3.ivf
vp91-2-04-yuv440.webm vp91-2-04-yuv422.webm vp91-2-04-yuv444.webm
vp92-2-20-10bit-yuv420.webm vp92-2-20-12bit-yuv420.webm
vp93-2-20-10bit-yuv422.webm vp93-2-20-12bit-yuv422.webm vp93-2-20-10bit-yuv440.webm
vp93-2-20-12bit-yuv440.webm vp93-2-20-10bit-yuv444.webm vp93-2-20-12bit-yuv444.webm
"

check_space() {
  local gb
  gb=$(df -Pk "$ROOT" | awk 'NR==2{print int($4/1048576)}')
  if [ "$gb" -lt "$MIN_FREE_GB" ]; then echo "only ${gb} GB free: stopping" >&2; exit 1; fi
}

n=0
for v in $VECTORS; do
  if [ -n "${ONLY:-}" ] && ! [[ $v =~ $ONLY ]]; then continue; fi
  check_space
  base=${v%.*}
  if [ ! -s "$OUT/$base.ivf" ] || [ ! -s "$OUT/$base.md5" ]; then
    if ! curl -sSfL -o "$OUT/src/$v" "$BASE/$v" || ! curl -sSfL -o "$OUT/$base.md5" "$BASE/$v.md5"; then
      echo "skip $v (not available)"; rm -f "$OUT/src/$v" "$OUT/$base.md5"; continue
    fi
    if [ "${v##*.}" = ivf ]; then
      cp "$OUT/src/$v" "$OUT/$base.ivf"
    else
      # Our own copy of the blocks: ffmpeg's remux drops frames before the first keyframe.
      python3 -I "$(dirname "$0")/specular-vp9-webm2ivf.py" "$OUT/src/$v" "$OUT/$base.ivf"
    fi
    rm -f "$OUT/src/$v"
  fi
  n=$((n + 1))
done
rmdir "$OUT/src" 2>/dev/null || true
echo "$n vectors in $OUT ($(du -sh "$OUT" | cut -f1))"
