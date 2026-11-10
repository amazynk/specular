#!/bin/sh
# Surround sound (tests/soundfiles.rs, played as 5.1 and 7.1 on a device with those
# speakers), made with ffmpeg 8.0 (libvorbis, libopus). Oracle:
#   NAME.pcm   ffmpeg's decode, s16le at the stream's rate and channels, in ffmpeg's
#              order (the WAVE order); Opus through libopus (ffmpeg's own Opus decoder
#              differs from libopus on hybrid streams: 14.6 dB on this file's centre)
# Each channel has its own tone, so a channel in the wrong place shows.
# Run from this directory: sh gen_fixtures_surround.sh
set -e
q="-nostdin -loglevel error -y"
D=0.15
six="0.3*sin(2*PI*300*t)|0.3*sin(2*PI*500*t)|0.2*sin(2*PI*700*t)|0.2*sin(2*PI*60*t)|0.2*sin(2*PI*900*t)|0.2*sin(2*PI*1100*t)"
eight="$six|0.2*sin(2*PI*1300*t)|0.2*sin(2*PI*1500*t)"
s51() { echo "-f lavfi -i aevalsrc=$six:s=$1:d=$D:channel_layout=5.1(side)"; }
s71() { echo "-f lavfi -i aevalsrc=$eight:s=$1:d=$D:channel_layout=7.1"; }
pcm() { ffmpeg $q $2 -i "$1" -map 0:a:0 -f s16le "${1%.*}.pcm"; }

ffmpeg $q $(s51 48000) -c:a ac3 -b:a 384k snd-ac3-51.ac3 && pcm snd-ac3-51.ac3
ffmpeg $q $(s51 48000) -c:a eac3 -b:a 384k snd-eac3-51.eac3 && pcm snd-eac3-51.eac3
ffmpeg $q $(s51 48000) -strict -2 -c:a dca -b:a 1509k snd-dts-51.dts && pcm snd-dts-51.dts
ffmpeg $q $(s71 48000) -c:a flac snd-flac-71.flac && pcm snd-flac-71.flac
ffmpeg $q $(s71 48000) -c:a aac -b:a 512k snd-aac-71.m4a && pcm snd-aac-71.m4a
ffmpeg $q $(s71 48000) -c:a libvorbis -q:a 4 snd-vorbis-71.ogg && pcm snd-vorbis-71.ogg
ffmpeg $q $(s71 48000) -c:a libopus -b:a 256k snd-opus-71.opus && pcm snd-opus-71.opus "-c:a libopus"
