#!/bin/sh
# Synthetic MPEG-1/2 elementary streams and their ffmpeg frame MD5s (ffmpeg 8.0, the
# integer IDCT the decoder matches). Run from this directory: sh gen_fixtures.sh
set -e
src="-f lavfi -i testsrc2=size=176x144:rate=25 -frames:v 12"
moving="-f lavfi -i mandelbrot=size=176x144:rate=25 -frames:v 12"
enc() {
    name=$1; shift
    ffmpeg -nostdin -loglevel error -y "$@" "$name"
    ffmpeg -nostdin -loglevel error -idct int -i "$name" -f framemd5 - | grep -v '^#' > "${name%.*}.framemd5"
}
# Main profile, progressive, I/P/B.
enc ibp.m2v $src -c:v mpeg2video -g 6 -bf 2 -q:v 4
# Interlaced frame pictures: field DCT and field prediction, top field first.
enc interlaced.m2v $moving -vf "tinterlace=interleave_top,setfield=tff" -c:v mpeg2video -flags +ildct+ilme -top 1 -g 6 -bf 2 -q:v 3
# 4:2:2 profile, intra VLC table one, alternate scan, non-linear quantiser scale, DC 10 bits.
enc p422.m2v $src -pix_fmt yuv422p -c:v mpeg2video -profile:v 0 -level:v 5 -intra_vlc 1 -alternate_scan 1 -non_linear_quant 1 -qmax 28 -dc 10 -g 4 -bf 1 -q:v 2
# High bit rate (escapes, large levels), odd size.
ffmpeg -nostdin -loglevel error -y -f lavfi -i testsrc2=size=174x142:rate=25 -frames:v 6 -c:v mpeg2video -q:v 1 -g 3 -bf 1 odd.m2v
ffmpeg -nostdin -loglevel error -idct int -i odd.m2v -f framemd5 - | grep -v '^#' > odd.framemd5
# MPEG-1.
enc mpeg1.m1v $src -c:v mpeg1video -g 6 -bf 2 -q:v 5
