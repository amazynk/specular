#!/usr/bin/env bash
# Regenerates the Matroska turn-and-mirror fixtures of tests/mkv_orientation.rs with ffmpeg
# (8.0, libvpx). One 64x32 VP8 frame (testsrc2: no symmetry), remuxed with each of the eight
# display matrices; ffmpeg writes them as Projection ProjectionPoseYaw (180: mirrored) and
# ProjectionPoseRoll. About 40 KB together.
#
#   flip-base.gray        ffmpeg's decode of the frame as stored (gray, 64x32)
#   flip-r<R>[h].mkv      -display_rotation R (0, 90, 180, 270), h: -display_hflip
#   flip-r<R>[h].gray     ffmpeg's decode of it, turned and mirrored for display (autorotate)
set -euo pipefail
cd "$(dirname "$0")"
FF=(ffmpeg -hide_banner -loglevel error -y)
tmp=$(mktemp -d "${PWD}/.flip.XXXXXX")
trap 'rm -rf "$tmp"' EXIT
"${FF[@]}" -f lavfi -i testsrc2=s=64x32:r=5 -frames:v 1 -c:v libvpx -b:v 2M "$tmp/base.mkv"
"${FF[@]}" -noautorotate -i "$tmp/base.mkv" -f rawvideo -pix_fmt gray flip-base.gray
for r in 0 90 180 270; do
  for h in "" h; do
    flag=()
    [ -n "$h" ] && flag=(-display_hflip)
    "${FF[@]}" -display_rotation "$r" "${flag[@]}" -i "$tmp/base.mkv" -c copy "flip-r$r$h.mkv"
    "${FF[@]}" -i "flip-r$r$h.mkv" -f rawvideo -pix_fmt gray "flip-r$r$h.gray"
  done
done
