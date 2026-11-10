#!/usr/bin/env bash
# SILK study material for specular-opus (crates/formats/specular-opus/src/silk_study.rs):
# SILK-only streams libopus encodes from generated signals (narrow, medium and wideband;
# mono and stereo; 10, 20 and 60 ms frames; with in-band FEC), plus the RFC 8251 vectors,
# each decoded by the system libopus (a black box, through ctypes) at 8, 12 and 16 kHz with
# no loss, with concealed losses and with FEC (tools/corpus/specular-opus-silk-oracle.py).
# Made locally; nothing is downloaded. Needs tools/corpus/specular-opus-fetch.sh first.
#
# Usage: tools/corpus/specular-opus-silk.sh
#   Then: cargo test -p specular-opus --lib silk_study -- --ignored --nocapture
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
VEC=$ROOT/tools/corpus/video2/specular-opus
OUT=$ROOT/tools/corpus/video2/specular-opus-silk
mkdir -p "$OUT/src"
q="-nostdin -loglevel error -y"
D=6
# Voiced: harmonics of a moving pitch, switched on and off (inactive frames between);
# unvoiced: pink noise; mixed: both.
V="(0.2*sin(2*PI*(140+50*sin(1.3*t))*t)+0.12*sin(2*PI*2*(140+50*sin(1.3*t))*t)+0.08*sin(2*PI*3*(140+50*sin(1.3*t))*t)+0.05*sin(2*PI*5*(140+50*sin(1.3*t))*t))*gt(sin(2*PI*0.7*t)\,-0.3)"
sig() { # <kind> <channels> <file>
  case $1 in
    voiced) ffmpeg $q -f lavfi -i "aevalsrc=$V|$V*0.6+0.02*sin(2*PI*300*t):s=48000" -t $D -ac "$2" "$3" ;;
    noise) ffmpeg $q -f lavfi -i "anoisesrc=color=pink:amplitude=0.25:seed=7:sample_rate=48000" -t $D -ac "$2" "$3" ;;
    mixed) ffmpeg $q -f lavfi -i "aevalsrc=$V:s=48000" -f lavfi \
      -i "anoisesrc=color=white:amplitude=0.05:seed=3:sample_rate=48000" \
      -filter_complex "[0][1]amix=inputs=2:normalize=0" -t $D -ac "$2" "$3" ;;
  esac
}
for kind in voiced noise mixed; do
  for ch in 1 2; do
    sig $kind $ch "$OUT/src/$kind$ch.wav"
  done
done
enc() { # <name> <kind> <channels> <cutoff> <bitrate> <frame ms>
  ffmpeg $q -i "$OUT/src/$2$3.wav" -c:a libopus -application voip -cutoff "$4" -b:a "$5" \
    -frame_duration "$6" -fec 1 -packet_loss 20 "$OUT/src/$1.opus"
}
for kind in voiced noise mixed; do
  for bw in nb:4000:12k mb:6000:16k wb:8000:20k; do
    IFS=: read -r b cut br <<<"$bw"
    enc "$kind-$b-m20" $kind 1 "$cut" "$br" 20
    enc "$kind-$b-s20" $kind 2 "$cut" "$br" 20
  done
  enc "$kind-wb-m10" $kind 1 8000 20k 10
  enc "$kind-wb-m60" $kind 1 8000 20k 60
  enc "$kind-nb-s60" $kind 2 4000 12k 60
done
inputs=("$OUT"/src/*.opus)
for n in 02 03 04 05 06 12; do
  inputs+=("$VEC/testvector$n.bit")
done
python3 -I "$ROOT/tools/corpus/specular-opus-silk-oracle.py" "$OUT" "${inputs[@]}" | wc -l
du -sh "$OUT"
