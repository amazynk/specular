#!/usr/bin/env bash
# Fixtures for separate colour planes (src/split_tests.rs). No encoder writes
# separate_colour_plane_flag, so the test builds such a stream from three monochrome streams
# encoded alike (one per colour plane of testsrc2 as 4:4:4) by rewriting their SPS and slice
# headers. Each plane of the result decodes as its monochrome stream does, so the oracle is
# libavcodec's decode of the monochrome streams (sep.md5: per plane, one MD5 per frame,
# display order).
#
#   sep-<y|u|v>.hevc   96x64, 12 frames, I every 8, B pictures, WPP, x265
set -euo pipefail
cd "$(dirname "$0")"
FF=(ffmpeg -hide_banner -loglevel error -y)
X265=log-level=error:keyint=8:min-keyint=8:scenecut=0:bframes=2:b-adapt=0:rc-lookahead=4
X265=$X265:frame-threads=1:pools=none:ref=2:wpp=1
: > sep.md5
for pl in y u v; do
  "${FF[@]}" -f lavfi -i "testsrc2=s=96x64:r=25:d=0.48,format=yuv444p,extractplanes=$pl" \
    -c:v libx265 -pix_fmt gray -x265-params "$X265" -f hevc "sep-$pl.hevc"
  "${FF[@]}" -threads 1 -f hevc -i "sep-$pl.hevc" -vf extractplanes=y -f framemd5 - \
    | grep -v '^#' | awk -F', *' -v p="$pl" '{print p, $6}' >> sep.md5
done
