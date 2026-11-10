#!/bin/sh
# DTS (core) sound in its containers, from ffmpeg 8.0's DTS encoder (which uses neither
# ADPCM prediction nor high-frequency VQ), with ffmpeg's decodes as the oracle: .pcm
# (s16le at the stream's rate and channels).
# Run from this directory: sh gen_fixtures_dts.sh
#   dts-mkv.mkv   VP8 with DTS stereo 48 kHz (A_DTS)
#   dts-mp4.mp4   MPEG-4 Part 2 with DTS stereo 48 kHz (`dtsc`)
#   dts-ts.ts     MPEG-2 with DTS stereo 48 kHz (stream type 0x82)
#   dts-avi.avi   MPEG-4 Part 2 with DTS stereo 44.1 kHz (WAVE tag 0x2001)
#   dts-pcm.avi   with DTS frames carried as 16-bit little-endian stereo PCM (as DTS CDs
#                 ripped to WAVE are): .pcm is the DTS stream's own decode
#   dts51.mkv     VP8 with DTS 5.1 48 kHz (facts only)
set -e
q="-nostdin -loglevel error -y"
pic="-f lavfi -i testsrc2=size=96x64:rate=25:duration=0.3"
mix() { echo "-f lavfi -i aevalsrc=0.4*sin(2*PI*$1*t)|0.3*sin(2*PI*($1+150)*t)+0.02*random(0):s=$2:d=0.3"; }
pcm() {
    ffmpeg $q -i "$1" -map 0:a:0 -f s16le "${1%.*}.pcm"
}
mp4v="-c:v mpeg4 -q:v 4 -g 6"
vp8="-c:v libvpx -b:v 200k -g 6"
dts="-c:a dca -strict -2"
ffmpeg $q $pic $(mix 440 48000) $vp8 $dts dts-mkv.mkv && pcm dts-mkv.mkv
ffmpeg $q $pic $(mix 500 48000) $mp4v $dts -strict -2 dts-mp4.mp4 && pcm dts-mp4.mp4
ffmpeg $q $pic $(mix 550 48000) -c:v mpeg2video -q:v 4 -g 6 $dts -f mpegts dts-ts.ts && pcm dts-ts.ts
ffmpeg $q $pic $(mix 600 44100) $mp4v $dts dts-avi.avi && pcm dts-avi.avi
# DTS as PCM: the stream's 16-bit words byte-swapped into little-endian "samples".
ffmpeg $q $(mix 650 44100) $dts -f dts dts-raw.tmp
python3 -I -c "import sys; d=open(sys.argv[1],'rb').read(); d=d[:len(d)//4*4]; open(sys.argv[2],'wb').write(bytes(b for i in range(0,len(d),2) for b in (d[i+1],d[i])))" dts-raw.tmp dts-raw.s16
ffmpeg $q -f dts -i dts-raw.tmp -f s16le dts-pcm.pcm
ffmpeg $q $pic -f s16le -ar 44100 -ac 2 -i dts-raw.s16 $mp4v -c:a pcm_s16le -shortest dts-pcm.avi
rm -f dts-raw.tmp dts-raw.s16
six="-f lavfi -i aevalsrc=0.4*sin(2*PI*300*t)|0.3*sin(2*PI*500*t)|0.2*sin(2*PI*700*t)|0.1*sin(2*PI*60*t)|0.3*sin(2*PI*900*t)|0.2*sin(2*PI*1100*t):s=48000:d=0.3:c=5.1"
ffmpeg $q $pic $six $vp8 $dts dts51.mkv
