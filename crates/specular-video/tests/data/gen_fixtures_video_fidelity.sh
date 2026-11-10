#!/bin/sh
# Fixtures of the video fidelity fixes (tests/video_fidelity_fixes.rs): tiny clips made with FFmpeg
# 8 from its own test source (no third-party content). Run from this directory:
# sh gen_fixtures_video_fidelity.sh
set -e
q="-nostdin -loglevel error -y"
src="-f lavfi -i testsrc2=size=32x32:rate=25 -frames:v 2"
# "RGB" H.264 (matrix 0, identity) and SMPTE 240M.
ffmpeg $q $src -c:v libx264rgb -preset veryfast -pix_fmt bgr0 f7-rgb.mp4
ffmpeg $q $src -c:v libx264 -preset veryfast -pix_fmt yuv420p -color_primaries smpte240m \
    -color_trc smpte240m -colorspace smpte240m f7-240m.mp4
# The VUI's chroma sample location: top-left (H.264 and HEVC).
ffmpeg $q $src -c:v libx264 -preset veryfast -x264-params chromaloc=2 -pix_fmt yuv420p f7-chromaloc.mp4
ffmpeg $q $src -c:v libx265 -preset ultrafast -x265-params log-level=error:chromaloc=2 \
    -pix_fmt yuv420p f7-hevc-chromaloc.mp4
# Non-square pixels: MP4 pasp, and an MPEG-4 Part 2 video object layer in AVI.
ffmpeg $q $src -c:v libx264 -preset veryfast -pix_fmt yuv420p -vf setsar=16/11 f7-sar.mp4
ffmpeg $q $src -c:v mpeg4 -q:v 4 -vf setsar=12/11 f7-mpeg4-sar.avi
# VP9 RGB (profile 1, sRGB colour space).
ffmpeg $q $src -c:v libvpx-vp9 -deadline realtime -pix_fmt gbrp -colorspace rgb f7-vp9-gbr.webm
# Limited-range Motion JPEG (FFmpeg writes a CS=ITU601 comment).
ffmpeg $q $src -c:v mjpeg -q:v 3 -pix_fmt yuv420p -strict -1 -color_range tv f7-mjpeg-tv.avi
# A bare MPEG-2 stream with zero bytes before its first start code.
ffmpeg $q $src -c:v mpeg2video -q:v 4 -f mpeg2video f7-plain.m2v
{ printf '\000\000\000\000\000\000\000\000\000\000'; cat f7-plain.m2v; } > f7-zeros.m2v
rm f7-plain.m2v
