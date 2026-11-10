#!/usr/bin/env bash
# Makes the movies for the seek-bar scrubbing timings (gitignored, about 350 MB): 1080p and
# 4K, 20 s at 25 fps, from ffmpeg's synthetic source: H.264 and HEVC with a sync sample every
# 250 frames, VP9 and AV1 with the same keyframe interval, and ProRes 422 Proxy (all intra).
# Nothing is downloaded.
#
# Usage: tools/corpus/scrub.sh [OUT]   (default: $CARGO_TARGET_DIR/corpus/scrub)
#   Then: SCRUB_CORPUS=OUT cargo test -p specular-video --test scrub -- --ignored --nocapture
# Needs ffmpeg with libx264, libx265, libvpx-vp9 and libsvtav1. Stops below MIN_FREE_GB (8).
set -euo pipefail

OUT=${1:-${CARGO_TARGET_DIR:?set CARGO_TARGET_DIR or pass OUT}/corpus/scrub}
MIN_FREE_GB=${MIN_FREE_GB:-8}
mkdir -p "$OUT"
avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
  echo "scrub: less than ${MIN_FREE_GB} GB free; stopping" >&2
  exit 1
fi
FF=(ffmpeg -hide_banner -loglevel error -y)
for size in 1920x1080 3840x2160; do
  h=${size#*x}
  src=(-f lavfi -i "testsrc2=s=$size:r=25:d=20")
  [[ -s $OUT/h264-${h}p.mp4 ]] || "${FF[@]}" "${src[@]}" -c:v libx264 -preset veryfast \
    -profile:v high -pix_fmt yuv420p -g 250 -keyint_min 250 -sc_threshold 0 -bf 3 \
    "$OUT/h264-${h}p.mp4"
  [[ -s $OUT/hevc-${h}p.mp4 ]] || "${FF[@]}" "${src[@]}" -c:v libx265 -preset veryfast \
    -pix_fmt yuv420p -x265-params log-level=error:keyint=250:min-keyint=250:scenecut=0:bframes=3 \
    -tag:v hvc1 "$OUT/hevc-${h}p.mp4"
  [[ -s $OUT/vp9-${h}p.webm ]] || "${FF[@]}" "${src[@]}" -c:v libvpx-vp9 -deadline realtime \
    -cpu-used 8 -row-mt 1 -b:v 4M -g 250 -keyint_min 250 "$OUT/vp9-${h}p.webm"
  [[ -s $OUT/av1-${h}p.mp4 ]] || "${FF[@]}" "${src[@]}" -c:v libsvtav1 -preset 12 -g 250 \
    -svtav1-params scd=0 "$OUT/av1-${h}p.mp4"
  [[ -s $OUT/prores-${h}p.mov ]] || "${FF[@]}" "${src[@]}" -c:v prores_ks -profile:v 0 \
    -pix_fmt yuv422p10le "$OUT/prores-${h}p.mov"
done
ls -l "$OUT"
