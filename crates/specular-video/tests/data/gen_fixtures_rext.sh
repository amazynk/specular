#!/usr/bin/env bash
# Regenerates the HEVC field-coding and H.264/HEVC range-extension fixtures of
# tests/containers.rs with ffmpeg (8.0; libx264, libx265). Every clip is tiny (64x48 or
# 128x96, a few frames). The .yuv files are ffmpeg's own decodes, the oracle the tests
# compare against.
#
#   hevc-fields.mp4    HEVC field coding (field_seq_flag, pic_timing pic_struct), top field
#                      first, B-frames; .yuv: ffmpeg's fields woven by its weave filter
#   hevc-fields.ts     the same stream in MPEG-TS
#   hevc-444-12.mp4    HEVC Main 4:4:4 12 with B-frames; .yuv (yuv444p12le)
#   hevc-422-12.mkv    HEVC Main 4:2:2 12 with B-frames; .yuv (yuv422p12le)
#   hevc-gray.mp4      HEVC 4:0:0 (monochrome); .yuv (gray: luma only)
#   h264-444.mp4       H.264 High 4:4:4 Predictive, CABAC, 8x8 transforms, B-frames; .yuv
#   h264-444-cavlc.mkv the same with CAVLC; .yuv
#   h264-444-10.mp4    H.264 High 4:4:4 at 10 bits; .yuv (yuv444p10le)
#   h264-444-lossless.mp4  H.264 4:4:4 lossless (qp 0: transform bypass); .yuv
#   h264-444-mbaff.mp4 H.264 4:4:4 MBAFF, top field first; .yuv (woven, as decoded)
set -euo pipefail
cd "$(dirname "$0")"
FF=(ffmpeg -hide_banner -loglevel error -y)
SRC=(-f lavfi -i testsrc2=size=128x96:rate=25)
SMALL=(-f lavfi -i testsrc2=size=64x48:rate=25)
X265=log-level=error:bframes=2:keyint=8

# Field coding: the source split into fields (50 per second), each coded as a picture.
"${FF[@]}" "${SRC[@]}" -frames:v 12 -vf separatefields -pix_fmt yuv420p -c:v libx265 \
  -x265-params "$X265:interlace=tff" hevc-fields.mp4
"${FF[@]}" -i hevc-fields.mp4 -c copy hevc-fields.ts
"${FF[@]}" -i hevc-fields.mp4 -vf weave=first_field=top -fps_mode passthrough -f rawvideo \
  hevc-fields.yuv

"${FF[@]}" "${SMALL[@]}" -frames:v 8 -pix_fmt yuv444p12le -c:v libx265 -x265-params "$X265" \
  hevc-444-12.mp4
"${FF[@]}" "${SMALL[@]}" -frames:v 8 -pix_fmt yuv422p12le -c:v libx265 -x265-params "$X265" \
  hevc-422-12.mkv
"${FF[@]}" "${SMALL[@]}" -frames:v 4 -pix_fmt gray -c:v libx265 -x265-params "$X265" hevc-gray.mp4

X264=(-c:v libx264 -preset medium -bf 2 -g 8)
"${FF[@]}" "${SMALL[@]}" -frames:v 8 -pix_fmt yuv444p "${X264[@]}" -x264-params 8x8dct=1 \
  h264-444.mp4
"${FF[@]}" "${SMALL[@]}" -frames:v 8 -pix_fmt yuv444p "${X264[@]}" -coder 0 h264-444-cavlc.mkv
"${FF[@]}" "${SMALL[@]}" -frames:v 8 -pix_fmt yuv444p10le "${X264[@]}" h264-444-10.mp4
"${FF[@]}" "${SMALL[@]}" -frames:v 4 -pix_fmt yuv444p "${X264[@]}" -qp 0 h264-444-lossless.mp4
"${FF[@]}" "${SRC[@]}" -frames:v 6 -pix_fmt yuv444p "${X264[@]}" -flags +ildct+ilme \
  -x264-params tff=1 h264-444-mbaff.mp4

for f in hevc-444-12.mp4 hevc-422-12.mkv hevc-gray.mp4 h264-444.mp4 h264-444-cavlc.mkv h264-444-10.mp4 \
    h264-444-lossless.mp4 h264-444-mbaff.mp4; do
  "${FF[@]}" -i "$f" -fps_mode passthrough -f rawvideo "${f%.*}.yuv"
done
