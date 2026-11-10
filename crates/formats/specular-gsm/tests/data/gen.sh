#!/bin/sh
# Regenerates the GSM fixtures (ffmpeg with libgsm). The .pcm files are libgsm's decoding,
# which ffmpeg's own gsm and gsm_ms decoders match byte for byte.
set -e
SIG="aevalsrc=0.4*sin(2*PI*(220+180*sin(2*PI*1.3*t))*t)*(0.6+0.4*sin(2*PI*3*t))+0.05*random(0)-0.025:s=8000:d=1.5"
ffmpeg -nostdin -v error -y -f lavfi -i "$SIG" -ac 1 -c:a libgsm -f gsm sweep.gsm
ffmpeg -nostdin -v error -y -f lavfi -i "$SIG" -ac 1 -c:a libgsm_ms sweep-ms.wav
ffmpeg -nostdin -v error -y -c:a libgsm -i sweep.gsm -f s16le sweep.libgsm.pcm
ffmpeg -nostdin -v error -y -c:a libgsm_ms -i sweep-ms.wav -f s16le sweep-ms.libgsm.pcm
