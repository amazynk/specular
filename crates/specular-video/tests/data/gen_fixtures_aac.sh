#!/usr/bin/env bash
# Regenerates the AAC sound fixtures of tests/containers.rs with ffmpeg (8.0; libx264 and
# ffmpeg's own AAC encoder). 64x48 picture, 0.8 s, about 0.25 MB together. One AAC-LC
# stream is carried in four ways; h264-aac.pcm is ffmpeg's decode of the MP4 (the edit list
# trims the encoder's priming).
#
#   h264-aac.mp4       AAC-LC stereo 48 kHz (AudioSpecificConfig in `esds`)
#   h264-aac.mkv       the same in Matroska (CodecPrivate)
#   h264-aac.ts        the same in MPEG-TS as ADTS
#   h264-aac-latm.ts   the same in MPEG-TS as LOAS/LATM
#   h264-aac51.mp4     AAC-LC 5.1 (heard folded to stereo)
set -euo pipefail
cd "$(dirname "$0")"
FF=(ffmpeg -hide_banner -loglevel error -y)
PIC="color=black:s=64x48:r=25:d=0.8,format=yuv420p,geq=lum='if(between(X,mod(N*2,56),mod(N*2,56)+7)*between(Y,20,27),235,16)':cb=128:cr=128"
# Tones with clicks (short windows) and a noise burst (noise substitution).
SND="aevalsrc='0.35*sin(2*PI*440*t)+0.3*lt(mod(t,0.2),0.004)*sin(2*PI*3000*t)+0.2*between(t,0.4,0.5)*(random(0)-0.5)|0.35*sin(2*PI*660*t+1)+0.15*sin(2*PI*5000*t)':s=48000:d=0.8"
X264=(-c:v libx264 -preset fast -pix_fmt yuv420p -g 8)

"${FF[@]}" -f lavfi -i "$PIC" -f lavfi -i "$SND" "${X264[@]}" -c:a aac -b:a 128k -shortest h264-aac.mp4
"${FF[@]}" -i h264-aac.mp4 -c copy h264-aac.mkv
"${FF[@]}" -i h264-aac.mp4 -c copy -f mpegts h264-aac.ts
"${FF[@]}" -i h264-aac.mp4 -c copy -mpegts_flags latm -f mpegts h264-aac-latm.ts
"${FF[@]}" -i h264-aac.mp4 -map 0:a -f s16le -ac 2 h264-aac.pcm

SND51="aevalsrc='0.3*sin(2*PI*300*t)|0.3*sin(2*PI*500*t)|0.3*sin(2*PI*700*t)|0.2*sin(2*PI*60*t)|0.3*sin(2*PI*900*t)|0.3*sin(2*PI*1100*t)':s=48000:d=0.8:c=5.1"
"${FF[@]}" -f lavfi -i "$PIC" -f lavfi -i "$SND51" "${X264[@]}" -c:a aac -b:a 256k -shortest h264-aac51.mp4
