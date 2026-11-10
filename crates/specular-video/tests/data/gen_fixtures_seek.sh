#!/usr/bin/env bash
# Long-GOP clips for the H.264 and MPEG-2 seek caches (tests/seek.rs; the oracle is a
# straight decode by the same decoder).
#
#   h264-gop.mp4   H.264 High 128x72, 120 frames, open GOPs of 60 (a recovery-point I
#                  picture at frame 60 with leading B pictures), B-pyramid of 3
#   mpeg2-gop.mp4  MPEG-2 Main 128x72, 120 frames, open GOPs of 60, two B pictures
set -euo pipefail
cd "$(dirname "$0")"
FF=(ffmpeg -hide_banner -loglevel error -y)

"${FF[@]}" -f lavfi -i "testsrc2=s=128x72:r=25:d=4.8" -c:v libx264 -pix_fmt yuv420p \
  -x264-params log-level=error:keyint=60:min-keyint=60:scenecut=0:bframes=3:b-pyramid=normal:open-gop=1 \
  -movflags +faststart h264-gop.mp4

"${FF[@]}" -f lavfi -i "testsrc2=s=128x72:r=25:d=4.8" -c:v mpeg2video -g 60 -bf 2 \
  -sc_threshold 1000000000 -q:v 12 mpeg2-gop.mp4
