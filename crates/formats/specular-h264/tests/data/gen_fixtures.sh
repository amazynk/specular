#!/usr/bin/env bash
# Fixtures for separate colour planes (src/split_tests.rs). No encoder writes
# separate_colour_plane_flag, so the test builds such a stream from three monochrome streams
# encoded alike (one per colour plane of testsrc2 as 4:4:4) by rewriting their parameter sets
# and slice headers. Each plane of the result decodes as its monochrome stream does, so the
# oracle is libavcodec's decode of the monochrome streams (sep-*.md5: one MD5 per frame per
# plane, display order).
#
#   sep-<kind>-<y|u|v>.264   72x40 (5x3 macroblocks, cropped), 12 frames, I every 8, B-pyramid
#                            kinds: cabac, cavlc, lossless (qp 0: transform bypass)
set -euo pipefail
cd "$(dirname "$0")"
FF=(ffmpeg -hide_banner -loglevel error -y)
X264=keyint=8:min-keyint=8:scenecut=0:bframes=2:b-adapt=0:b-pyramid=normal:ref=2:threads=1:log-level=error
for kind in cabac cavlc lossless; do
  case $kind in
    cabac) extra="" ;;
    cavlc) extra=":cabac=0" ;;
    lossless) extra=":qp=0" ;;
  esac
  : > "sep-$kind.md5"
  for pl in y u v; do
    "${FF[@]}" -f lavfi -i "testsrc2=s=72x40:r=25:d=0.48,format=yuv444p,extractplanes=$pl" \
      -c:v libx264 -pix_fmt gray -x264-params "$X264$extra" -f h264 "sep-$kind-$pl.264"
    "${FF[@]}" -threads 1 -f h264 -i "sep-$kind-$pl.264" -vf extractplanes=y -f framemd5 - \
      | grep -v '^#' | awk -F', *' -v p="$pl" '{print p, $6}' >> "sep-$kind.md5"
  done
done
