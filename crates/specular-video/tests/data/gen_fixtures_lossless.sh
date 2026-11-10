#!/usr/bin/env bash
# WavPack and Monkey's Audio fixtures for tests/lossless_files.rs, made with the reference
# coders that tools/corpus/fetch-lossless.sh builds (BIN, default
# $CARGO_TARGET_DIR/corpus/lossless/bin). Each X gets X.pcm: the reference decode as 16-bit
# little-endian PCM.
#   snd-wv.wv      WavPack lossless, stereo 44.1 kHz, 0.25 s, -hh -x3
#   snd-wvh.wv/.wvc WavPack hybrid at 3 bits/sample with its correction file (snd-wvh.pcm is
#                  the lossless decode, snd-wvh-lossy.pcm the decode without the .wvc)
#   snd-ape.ape    Monkey's Audio, stereo 44.1 kHz, 0.25 s, extra high (-c4000)
set -euo pipefail
cd "$(dirname "$0")"
BIN=${BIN:-${CARGO_TARGET_DIR:-../../../../target}/corpus/lossless/bin}
T=$(mktemp -d "${TMPDIR:?set TMPDIR}/lossless.XXXXXX")
ffmpeg -v error -f lavfi -i "sine=f=440:d=0.25,aformat=s16" -f lavfi -i "anoisesrc=d=0.25:a=0.2" \
  -filter_complex "[0][1]amerge=inputs=2,aformat=s16:44100" -ac 2 -y "$T/src.wav"
"$BIN/wavpack" -q -y -hh -x3 "$T/src.wav" -o snd-wv.wv
"$BIN/wvunpack" -q -y --raw snd-wv.wv -o snd-wv.pcm
"$BIN/wavpack" -q -y -b3 -c "$T/src.wav" -o snd-wvh.wv
"$BIN/wvunpack" -q -y --raw snd-wvh.wv -o snd-wvh.pcm
mv snd-wvh.wvc "$T/hidden.wvc"
"$BIN/wvunpack" -q -y --raw snd-wvh.wv -o snd-wvh-lossy.pcm || true
mv "$T/hidden.wvc" snd-wvh.wvc
"$BIN/mac" "$T/src.wav" snd-ape.ape -c4000 >/dev/null 2>&1
"$BIN/mac" snd-ape.ape "$T/out.wav" -d >/dev/null 2>&1
ffmpeg -v error -i "$T/out.wav" -f s16le -y snd-ape.pcm
rm -r "$T"
