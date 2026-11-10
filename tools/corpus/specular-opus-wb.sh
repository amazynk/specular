#!/usr/bin/env bash
# Wideband SILK training material for the specular-opus upsampler study
# (crates/formats/specular-opus/src/resample_study.rs): broadband signals encoded by
# libopus as SILK-only wideband, and libopus's decodes of them (s16le mono 48 kHz, pre-skip
# trimmed). Made locally with ffmpeg (libopus); nothing is downloaded.
#
# Usage: tools/corpus/specular-opus-wb.sh <out dir>
#   Then: SPECULAR_OPUS_WB_CORPUS=<out dir> cargo test -p specular-opus --lib resample_study \
#         -- --ignored --nocapture
set -euo pipefail
OUT=${1:?usage: specular-opus-wb.sh <out dir>}
mkdir -p "$OUT"
q="-nostdin -loglevel error -y"
D=8
n=0
make() { # <lavfi source> <bitrate>
  n=$((n + 1))
  local f
  f=$(printf '%s/wb%02d' "$OUT" "$n")
  ffmpeg $q -f lavfi -i "$1" -t $D -ac 1 -ar 48000 -c:a libopus -application voip \
    -cutoff 8000 -b:a "$2" "$f.opus"
  ffmpeg $q -c:a libopus -i "$f.opus" -f s16le -ac 1 -ar 48000 "$f.pcm"
}
for br in 16k 24k 32k; do
  make "anoisesrc=color=pink:amplitude=0.3:seed=$n:sample_rate=48000" $br
  make "anoisesrc=color=white:amplitude=0.15:seed=$n:sample_rate=48000" $br
  # A slow chirp 50 Hz to 7.5 kHz, and a sum of tones with a moving pitch.
  make "aevalsrc=0.3*sin(2*PI*(50*t+(7450/(2*$D))*t*t)):s=48000" $br
  make "aevalsrc=0.15*sin(2*PI*(110+40*sin(t))*t)+0.1*sin(2*PI*3*(110+40*sin(t))*t)+0.08*sin(2*PI*7*(110+40*sin(t))*t):s=48000" $br
done
ls "$OUT" | wc -l
