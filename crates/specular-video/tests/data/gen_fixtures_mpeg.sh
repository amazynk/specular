#!/bin/sh
# MPEG-1/2 video and MPEG audio in every container (ffmpeg 8.0), with ffmpeg's decodes as the
# oracle: .yuv (`-idct int`, every frame as decoded) and .pcm (s16le). Run from this
# directory: sh gen_fixtures_mpeg.sh
#   mpeg2i-mp2.ts   MPEG-2 MP@ML interlaced (field DCT and prediction), B-frames, MP2 stereo
#   mpeg1-mp2.mpg  MPEG-1 system stream, B-frames, MP2 stereo 44.1 kHz
#   dvd-ac3.vob    MPEG-2 programme stream (DVD), AC-3 sound (silent, named)
#   mpeg2.mkv      MPEG-2 4:2:2 in Matroska
#   mpeg2-mp2.mp4  MPEG-2 in MP4 (`mp4v`, object type 0x61) with MP2 sound
#   bare.m2v       a bare MPEG-2 video stream
set -e
q="-nostdin -loglevel error -y"
pic="-f lavfi -i testsrc2=size=96x64:rate=25:duration=0.4"
snd="-f lavfi -i sine=frequency=500:sample_rate=48000:duration=0.4 -ac 2"
ref() {
    ffmpeg $q -idct int -i "$1" -map 0:v:0 -fps_mode passthrough -f rawvideo "${1%.*}.yuv"
}
pcm() {
    ffmpeg $q -i "$1" -map 0:a:0 -f s16le -ar "$2" -ac 2 "${1%.*}.pcm"
}
ffmpeg $q $pic $snd -vf "tinterlace=interleave_top,setfield=tff" -c:v mpeg2video -flags +ildct+ilme -top 1 -g 6 -bf 2 -q:v 4 -c:a mp2 -b:a 128k -f mpegts mpeg2i-mp2.ts
ref mpeg2i-mp2.ts
pcm mpeg2i-mp2.ts 48000
ffmpeg $q $pic -f lavfi -i sine=frequency=700:sample_rate=44100:duration=0.4 -ac 2 -c:v mpeg1video -g 6 -bf 2 -q:v 5 -c:a mp2 -b:a 128k -f mpeg mpeg1-mp2.mpg
ref mpeg1-mp2.mpg
pcm mpeg1-mp2.mpg 44100
ffmpeg $q $pic $snd -c:v mpeg2video -g 6 -bf 2 -q:v 4 -c:a ac3 -b:a 96k -f vob dvd-ac3.vob
ref dvd-ac3.vob
ffmpeg $q $pic -pix_fmt yuv422p -c:v mpeg2video -g 5 -bf 1 -q:v 3 mpeg2.mkv
ref mpeg2.mkv
ffmpeg $q $pic $snd -c:v mpeg2video -g 6 -bf 2 -q:v 4 -c:a mp2 -b:a 128k mpeg2-mp2.mp4
ref mpeg2-mp2.mp4
ffmpeg $q $pic -c:v mpeg2video -g 4 -bf 1 -q:v 4 -f mpeg2video bare.m2v
ref bare.m2v
# The same pictures as a programme stream: ffmpeg gives the I pictures only a DTS, so the
# first picture comes before the first PTS (fidelity round 8).
ffmpeg $q -i bare.m2v -c copy -f vob bare-nopts.vob
