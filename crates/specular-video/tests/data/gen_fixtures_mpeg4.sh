#!/bin/sh
# MPEG-4 Part 2 and H.263 in AVI, Matroska, MP4 and 3GPP (ffmpeg 8.0 with libxvid), with
# ffmpeg's decodes as the oracle: .yuv (one thread, `-idct int`, bit-exact flags; every frame
# as decoded) and .pcm (s16le). Run from this directory: sh gen_fixtures_mpeg4.sh
#   xvid-mp2.avi   Xvid with B-VOPs (shown in the chunks' time slots), MP2 stereo sound
#   mpeg4-asp.mkv  MPEG-4 ASP, quarter samples and B-VOPs, in Matroska (V_MPEG4/ISO/ASP)
#   h263.3gp       H.263 baseline in 3GPP
#   msmpeg4.avi    Microsoft MPEG-4 v3 (refused, with its name)
set -e
q="-nostdin -loglevel error -y"
pic="-f lavfi -i testsrc2=size=96x64:rate=25:duration=0.4"
moving="-f lavfi -i mandelbrot=size=96x64:rate=25,trim=duration=0.4"
ref() {
    ffmpeg $q -threads 1 -flags +bitexact -idct int -i "$1" -map 0:v:0 -fps_mode passthrough -f rawvideo "${1%.*}.yuv"
}
ffmpeg $q $moving -f lavfi -i sine=frequency=600:sample_rate=48000:duration=0.4 -ac 2 -c:v libxvid -bf 2 -g 10 -q:v 3 -c:a mp2 -b:a 128k xvid-mp2.avi
ref xvid-mp2.avi
ffmpeg $q -i xvid-mp2.avi -map 0:a:0 -f s16le -ar 48000 -ac 2 xvid-mp2.pcm
ffmpeg $q $moving -c:v mpeg4 -flags +qpel -bf 2 -g 10 -q:v 3 mpeg4-asp.mkv
ref mpeg4-asp.mkv
ffmpeg $q -f lavfi -i testsrc2=size=128x96:rate=25:duration=0.4 -c:v h263 -q:v 4 h263.3gp
ref h263.3gp
ffmpeg $q $pic -frames:v 3 -c:v msmpeg4 msmpeg4.avi
