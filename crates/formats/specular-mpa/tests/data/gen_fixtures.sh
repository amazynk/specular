#!/bin/sh
# Synthetic MPEG audio Layer II streams and libavcodec's decode of each as s16le PCM
# (ffmpeg 8.0 with libtwolame). Run from this directory: sh gen_fixtures.sh
set -e
tone="-f lavfi -i sine=frequency=440:sample_rate=48000:duration=0.3 -f lavfi -i sine=frequency=1250:sample_rate=48000:duration=0.3 -filter_complex [0][1]amerge=inputs=2,volume=0.7"
# Through Matroska: the raw MPEG audio demuxer drops a first frame whose mode differs from
# the next (twolame's joint stereo streams start so).
enc() {
    name=$1; shift
    ffmpeg -nostdin -loglevel error -y $tone "$@" "$name.mka"
    ffmpeg -nostdin -loglevel error -y -i "$name.mka" -c copy -f mp2 "$name.mp2"
    ffmpeg -nostdin -loglevel error -y -i "$name.mka" -f s16le "$name.s16"
    rm "$name.mka"
}
# Table B.2a: 48 kHz stereo, 192 kbit/s.
enc s48 -c:a mp2 -b:a 192k
# Table B.2b: 44.1 kHz, 128 kbit/s per channel, CRC.
enc s44 -ar 44100 -c:a libtwolame -b:a 256k -error_protection 1
# Joint stereo (intensity), 44.1 kHz, 112 kbit/s: table B.2a.
enc joint -ar 44100 -c:a libtwolame -b:a 112k -mode joint_stereo
# Table B.2c: 48 kHz mono, 48 kbit/s.
enc m48 -ac 1 -c:a mp2 -b:a 48k
# Table B.2d: 32 kHz mono, 32 kbit/s.
enc m32 -ar 32000 -ac 1 -c:a mp2 -b:a 32k
# MPEG-2 lower sampling rate: 24 kHz stereo, 64 kbit/s.
enc lsf -ar 24000 -c:a mp2 -b:a 64k
