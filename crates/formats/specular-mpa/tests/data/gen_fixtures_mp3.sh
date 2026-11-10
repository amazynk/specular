#!/bin/sh
# Synthetic MP3 (Layer III) streams and libavcodec's decode of each as s16le PCM (ffmpeg 8.0
# with libmp3lame and libshine). A tone with clicks makes LAME switch to short and mixed
# windows; noise needs the linbits tables. No Xing/LAME header, so ffmpeg trims nothing.
# Run from this directory: sh gen_fixtures_mp3.sh
set -e
src() {
    echo "-f lavfi -i aevalsrc=0.4*sin(2*PI*440*t)+0.5*lt(mod(t\,0.13)\,0.003)*sin(2*PI*2500*t)+$1*random(0)|0.3*sin(2*PI*660*t)+0.45*lt(mod(t+0.05\,0.17)\,0.002)+$1*random(1):s=$2:d=0.3"
}
enc() {
    name=$1; shift
    ffmpeg -nostdin -loglevel error -y "$@" -write_xing 0 -id3v2_version 0 -f mp3 "$name.mp3"
    ffmpeg -nostdin -loglevel error -y -i "$name.mp3" -f s16le "$name.s16"
}
# MPEG-1 44.1 kHz joint (middle/side) stereo, 128 kbit/s, with short blocks.
enc l3-j44 $(src 0.02 44100) -c:a libmp3lame -b:a 128k
# MPEG-1 48 kHz stereo 320 kbit/s over loud noise: big values with linbits.
enc l3-s48 $(src 0.3 48000) -c:a libmp3lame -b:a 320k -joint_stereo 0
# MPEG-1 32 kHz mono, 64 kbit/s.
enc l3-m32 $(src 0.05 32000) -ac 1 -c:a libmp3lame -b:a 64k
# MPEG-1 44.1 kHz VBR (the reservoir reaches back over several frames).
enc l3-vbr $(src 0.05 44100) -c:a libmp3lame -q:a 2
# MPEG-2 22.05 kHz stereo, 64 kbit/s; MPEG-2 16 kHz mono, 32 kbit/s.
enc l3-lsf22 $(src 0.03 22050) -c:a libmp3lame -b:a 64k
enc l3-lsf16 $(src 0.03 16000) -ac 1 -c:a libmp3lame -b:a 32k
# MPEG-2.5 8 kHz mono, 16 kbit/s and 11.025 kHz stereo.
enc l3-m8 $(src 0.03 8000) -ac 1 -c:a libmp3lame -b:a 16k
enc l3-s11 $(src 0.03 11025) -c:a libmp3lame -b:a 32k
# Shine: long blocks only, plain stereo.
enc l3-shine $(src 0.05 48000) -c:a libshine -b:a 128k
