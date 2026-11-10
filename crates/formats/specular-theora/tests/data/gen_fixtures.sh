#!/bin/sh
# Small Theora clips (ffmpeg 8 + libtheora) and ffmpeg's frame MD5s of their decode.
# Run from this directory: sh gen_fixtures.sh
set -e
enc() {
    name=$1; shift
    ffmpeg -nostdin -loglevel error -y "$@" -c:v libtheora "$name"
    ffmpeg -nostdin -loglevel error -flags unaligned -i "$name" -f framemd5 - | grep -v '^#' > "${name%.*}.framemd5"
}
# 4:2:0, frame = picture, three key frames.
enc c420.ogv -f lavfi -i testsrc2=size=176x144:rate=25 -frames:v 25 -g 12 -pix_fmt yuv420p -q:v 6
# Odd size: the picture region is smaller than the 176x112 frame; zooming motion.
enc odd.ogv -f lavfi -i mandelbrot=size=170x98:rate=25 -frames:v 12 -g 12 -pix_fmt yuv420p -q:v 4
# Low quality (coarse quantisers, strong loop filter).
enc low.ogv -f lavfi -i testsrc2=size=176x144:rate=25 -frames:v 12 -g 12 -pix_fmt yuv420p -q:v 0
# 4:4:4 and 4:2:2.
enc c444.ogv -f lavfi -i testsrc2=size=176x144:rate=25 -frames:v 12 -g 12 -pix_fmt yuv444p -q:v 8
enc c422.ogv -f lavfi -i mandelbrot=size=170x98:rate=25 -frames:v 12 -g 12 -pix_fmt yuv422p -q:v 5
# Constant bit rate (rate control may vary qi per frame).
enc cbr.ogv -f lavfi -i testsrc2=size=176x144:rate=25 -frames:v 12 -g 12 -pix_fmt yuv420p -b:v 200k
# A frame wider than 8192 (a key frame, then inter frames).
enc wide.ogv -f lavfi -i testsrc2=size=8240x48:rate=25 -frames:v 4 -g 3 -pix_fmt yuv420p -q:v 6
