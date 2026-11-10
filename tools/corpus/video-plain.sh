#!/usr/bin/env bash
# Makes the larger movies for plain video playback timings (gitignored, about 120 MB):
# 1080p and 4K, H.264 High (B-frames, 2 s GOP) and HEVC Main 10, 10 s at 30 fps each, with
# AAC stereo, from ffmpeg's synthetic sources. Nothing is downloaded.
#
# Usage: tools/corpus/video-plain.sh
#   Then: cargo test -p specular-video --test movie timings_on_large_clips -- --nocapture
# Needs ffmpeg with libx264 and libx265. Stops if free space drops below MIN_FREE_GB (8).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=$ROOT/tools/corpus/out/video-plain
MIN_FREE_GB=${MIN_FREE_GB:-8}
mkdir -p "$OUT"
avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
  echo "video-plain: less than ${MIN_FREE_GB} GB free; stopping" >&2
  exit 1
fi
FF=(ffmpeg -hide_banner -loglevel error -y)
for size in 1920x1080 3840x2160; do
  h=${size#*x}
  src=(-f lavfi -i "testsrc2=s=$size:r=30:d=10" -f lavfi -i "sine=frequency=440:duration=10")
  "${FF[@]}" "${src[@]}" -c:v libx264 -preset veryfast -profile:v high -pix_fmt yuv420p \
    -g 60 -bf 3 -c:a aac -ac 2 "$OUT/h264-${h}p.mp4"
  "${FF[@]}" "${src[@]}" -c:v libx265 -preset veryfast -pix_fmt yuv420p10le \
    -x265-params log-level=error:keyint=60:bframes=3 -tag:v hvc1 -c:a aac -ac 2 \
    "$OUT/hevc10-${h}p.mp4"
done
ls -l "$OUT"
