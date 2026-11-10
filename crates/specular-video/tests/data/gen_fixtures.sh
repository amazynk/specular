#!/usr/bin/env bash
# Regenerates the movie fixtures of tests/movie.rs with ffmpeg (8.0; libx264, libx265,
# libaom). Every clip but hevc-gop.mp4 (256x144) is 64x48 or smaller and a second or two
# long: about 0.8 MB together.
# The .yuv / .rgb files are ffmpeg's own decodes, the oracle the tests compare against.
#
#   rot90.mov     H.264 High, B-frames (keyint 12), sowt PCM stereo 16 kHz, displayed turned
#                 90 degrees clockwise; quadrants red/green/blue/white with a moving square
#   rot90.yuv     its frames as ffmpeg decodes them (yuv420p, coded orientation)
#   rot90.rgb     frame 0 as ffmpeg shows it (rgb24, rotation applied: 48x64)
#   hlg10.mp4     HEVC Main 10, BT.2020 HLG, limited range; hlg10.yuv (yuv420p10le)
#   av1.mp4       AV1 (libaom) 8-bit; av1.yuv
#   clip.3gp      3GPP: H.264 Baseline with AAC-LC mono
#   mpeg4.mp4     MPEG-4 Part 2 (refused, with a reason)
#   interlaced.mp4 H.264 coded as fields (plays deinterlaced)
#   hevc-gop.mp4  HEVC Main 256x144 (big enough for hardware decoders), 72 frames, open
#                 GOPs of 32 (CRA with leading pictures), B-pyramid: seeks and the seek cache
#                 (tests/seek.rs, tests/seek_hw.rs; the oracle is a straight decode)
set -euo pipefail
cd "$(dirname "$0")"
FF=(ffmpeg -hide_banner -loglevel error -y)
# Quadrants with a white 8x8 square moving left to right through the middle.
PATTERN="color=black:s=64x48:r=25:d=2,format=rgb24,geq=\
r='if(between(X,mod(N*2,56),mod(N*2,56)+7)*between(Y,20,27),255,if(lt(X,32)*lt(Y,24)+gte(X,32)*gte(Y,24),255,0))':\
g='if(between(X,mod(N*2,56),mod(N*2,56)+7)*between(Y,20,27),255,if(gte(X,32),255,0))':\
b='if(between(X,mod(N*2,56),mod(N*2,56)+7)*between(Y,20,27),255,if(gte(Y,24),255,0))'"
X264=(-c:v libx264 -preset slow -pix_fmt yuv420p -x264-params keyint=12:min-keyint=12:scenecut=0:bframes=3:b-pyramid=normal)

# Encoded upright, then remuxed with a display matrix (stream copy, so nothing turns it).
"${FF[@]}" -f lavfi -i "$PATTERN" \
  -f lavfi -i "sine=frequency=1000:sample_rate=16000:duration=2" \
  "${X264[@]}" -profile:v high -c:a pcm_s16le -ac 2 -shortest rot0.mov
"${FF[@]}" -display_rotation:v:0 270 -i rot0.mov -c copy rot90.mov
rm rot0.mov
"${FF[@]}" -noautorotate -i rot90.mov -f rawvideo -pix_fmt yuv420p rot90.yuv
"${FF[@]}" -i rot90.mov -frames:v 1 -f rawvideo -pix_fmt rgb24 rot90.rgb

"${FF[@]}" -f lavfi -i "testsrc2=s=64x48:r=25:d=0.4" -c:v libx265 -pix_fmt yuv420p10le \
  -x265-params log-level=error:keyint=5:bframes=2:colorprim=bt2020:transfer=arib-std-b67:colormatrix=bt2020nc -tag:v hvc1 \
  -color_primaries bt2020 -color_trc arib-std-b67 -colorspace bt2020nc -color_range tv hlg10.mp4
"${FF[@]}" -i hlg10.mp4 -f rawvideo -pix_fmt yuv420p10le hlg10.yuv

"${FF[@]}" -f lavfi -i "testsrc2=s=256x144:r=25:d=2.88" -c:v libx265 -pix_fmt yuv420p \
  -x265-params log-level=error:keyint=32:min-keyint=32:scenecut=0:bframes=3:b-pyramid=1:open-gop=1 \
  -tag:v hvc1 hevc-gop.mp4

"${FF[@]}" -f lavfi -i "testsrc2=s=64x48:r=25:d=0.4" -c:v libaom-av1 -cpu-used 8 -g 5 \
  -pix_fmt yuv420p av1.mp4
"${FF[@]}" -i av1.mp4 -f rawvideo -pix_fmt yuv420p av1.yuv

"${FF[@]}" -f lavfi -i "testsrc2=s=64x48:r=25:d=0.4" -f lavfi -i "sine=sample_rate=16000:duration=0.4" \
  -c:v libx264 -profile:v baseline -pix_fmt yuv420p -c:a aac -ac 1 -b:a 24k clip.3gp
"${FF[@]}" -f lavfi -i "testsrc2=s=64x48:r=25:d=0.2" -c:v mpeg4 mpeg4.mp4
"${FF[@]}" -f lavfi -i "testsrc2=s=64x48:r=25:d=0.2" -c:v libx264 -pix_fmt yuv420p \
  -flags +ilme+ildct -x264-params tff=1 interlaced.mp4
# Raw elementary streams (tests/raw_streams.rs), the video above without its container:
# H.264 with B-frames and AV1 as an OBU stream (HEVC: gen_fixtures_mkv_ts.sh). Their
# oracles are rot90.yuv and av1.yuv.
"${FF[@]}" -i rot90.mov -map 0:v -c:v copy -bsf:v h264_mp4toannexb -f h264 es-bframes.h264
"${FF[@]}" -i av1.mp4 -map 0:v -c:v copy -f obu es.obu
ls -l rot90.* hlg10.* av1.* clip.3gp mpeg4.mp4 interlaced.mp4
