#!/bin/sh
# Synthetic Vorbis streams (ffmpeg 8 with libvorbis) and libavcodec's own Vorbis decode of each
# as PCM: f32le (.f32) or s16le (.s16). Run from this directory: sh gen_fixtures.sh
set -e
music="0.3*sin(2*PI*440*t)+0.15*sin(2*PI*1234.5*t)*sin(2*PI*3*t)+0.08*sin(2*PI*5000*t)+0.05*(random(0)-0.5)"
enc() {
    name=$1 fmt=$2 src=$3
    shift 3
    ffmpeg -nostdin -loglevel error -y -f lavfi -i "$src" "$@" -c:a libvorbis -map_metadata -1 \
        -fflags +bitexact -flags:a +bitexact "$name.ogg"
    ffmpeg -nostdin -loglevel error -y -c:a vorbis -i "$name.ogg" -f "${fmt}le" "$name.$fmt"
}
# Stereo 44.1 kHz, quality 3.
enc s44q3 f32 "aevalsrc=$music|0.25*sin(2*PI*660*t)+0.1*sin(2*PI*2500*t)+0.05*(random(1)-0.5):s=44100:d=0.28" -q:a 3
# Mono 48 kHz at the lowest quality.
enc m48low f32 "aevalsrc=$music:s=48000:d=0.5" -q:a -1
# 5.1 at 48 kHz, a different tone in each channel.
enc c51 s16 "aevalsrc=0.3*sin(2*PI*300*t)|0.3*sin(2*PI*500*t)|0.3*sin(2*PI*700*t)|0.3*sin(2*PI*60*t)|0.2*sin(2*PI*900*t)+0.05*random(0)|0.2*sin(2*PI*1100*t):s=48000:d=0.15:c=5.1" -q:a 2
# Stereo 22.05 kHz.
enc s22 f32 "aevalsrc=$music|0.2*sin(2*PI*330*t)+0.1*(random(2)-0.5):s=22050:d=0.5" -q:a 1
# Noise bursts on a quiet tone: short blocks.
enc clicks s16 "aevalsrc=if(lt(mod(t\,0.09)\,0.004)\,0.7*(random(0)-0.5)\,0)+0.05*sin(2*PI*440*t)|if(lt(mod(t+0.03\,0.11)\,0.003)\,0.8*(random(1)-0.5)\,0)+0.05*sin(2*PI*620*t):s=44100:d=0.5" -q:a 5
