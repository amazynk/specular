#!/usr/bin/env bash
# Fetch a real OGM file with AC-3 sound (DirectShow OGM, XviD video, a subtitle stream) into
# $SPECULAR_OGM_CORPUS (default ${CARGO_TARGET_DIR:-target}/corpus/ogm; never committed) and
# write ffmpeg's decode of its sound beside it as NAME.pcm (s16le), the oracle.
#
# Test that uses it:
#   SPECULAR_OGM_CORPUS=... cargo test -p specular-video --test sound_gaps -- --ignored ogm_corpus
#
# The download is untrusted data: it is checked against its SHA-256 below and nothing from it
# is run. Needs curl and ffmpeg (8.x). About 10 MB in all.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_OGM_CORPUS:-${CARGO_TARGET_DIR:-$ROOT/target}/corpus/ogm}
for t in curl ffmpeg sha256sum; do command -v $t >/dev/null || { echo "$t not found" >&2; exit 1; }; done
mkdir -p "$OUT"
name=bots01.ogm
sum=4d4316db8b5b679c42dbf05d9ad640124aef0a40c81c94477c52097ec58696b3
if [ ! -s "$OUT/$name" ]; then
  echo "fetching $name (5.2 MB) from samples.mplayerhq.hu"
  curl -fL --progress-bar -o "$OUT/$name.part" "https://samples.mplayerhq.hu/ogg/DShow-NEW/ogm+ac3/$name"
  mv "$OUT/$name.part" "$OUT/$name"
fi
if ! echo "$sum  $OUT/$name" | sha256sum -c --quiet -; then
  echo "checksum mismatch: $OUT/$name" >&2
  rm -f "$OUT/$name"
  exit 1
fi
ffmpeg -nostdin -hide_banner -loglevel error -y -i "$OUT/$name" -map 0:a:0 -f s16le "$OUT/${name%.*}.pcm"
echo "corpus in $OUT"
