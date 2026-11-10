#!/usr/bin/env bash
# Fetch the VC-1 corpus for specular-vc1 into $SPECULAR_VC1_CORPUS (default
# ${CARGO_TARGET_DIR:-target}/corpus/vc1/fate; gitignored, never committed): FFmpeg's FATE
# copies of the SMPTE VC-1 conformance streams (raw Advanced-profile .vc1, Simple/Main .rcv)
# and its interlaced sample. About 6 MB.
#
# The oracle is FFmpeg itself: the test decodes each file with `ffmpeg` (8.x on PATH) and
# compares every picture.
#   SPECULAR_VC1_CORPUS=... cargo test -p specular-vc1 --test oracle -- --ignored --nocapture
# VC-1 in its containers (ASF/WMV, AVI, Matroska, MP4, transport streams) is checked by
# tools/corpus/fetch-videocodecs4.sh.
#
# Downloads are untrusted data: each is checked against its size and SHA-256 below and
# nothing from them is run. Stops below MIN_FREE_GB (default 6).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_VC1_CORPUS:-${CARGO_TARGET_DIR:-$ROOT/target}/corpus/vc1/fate}
MIN_FREE_GB=${MIN_FREE_GB:-6}
FATE=https://fate-suite.ffmpeg.org/vc1
mkdir -p "$OUT"

# name | size | sha256
FILES=(
  "ilaced_twomv.vc1|2386818|8052c6f2514d7b9e0e6322b4cde440a590cbf6c7e97487336eafa4145ba74d30"
  "SA00040.vc1|24097|898035e63cfe0ddb1cc2c8749d7dafdfcc1b12fe5dac7e5b65941e6c4e41c860"
  "SA00050.vc1|124823|29cf8bebc87be73a1b7a6169aff4c1c36d8891a20c200da7676560fefe3e461c"
  "SA10091.vc1|437691|c60a0da89873b40d7e436f1c29757c6cc71f3c51f2ac2ea8c6bd023762c94b6c"
  "SA10143.vc1|671739|5d4a5dff66ca45f32331810fc7f845ab0600850a0ee97d05db32572bb588222e"
  "SA20021.vc1|772025|99098ebb84a71b3a62d48988d79169843d73dbfd12c620961ab5f1973a9f1b05"
  "SMM0005.rcv|1000819|2de6e6efe960bb0d93b0bfc0229e0b3b1fcdca72eb4b3e290de5f24fafa456f9"
  "SMM0015.rcv|437157|67baabd762ed35f70cd70c492e83e8744d068a5a567d9b19cc509851bf9f1d44"
)

for t in curl sha256sum; do
  command -v $t >/dev/null || { echo "fetch-vc1: $t not found" >&2; exit 1; }
done

for e in "${FILES[@]}"; do
  IFS='|' read -r name size sum <<< "$e"
  f="$OUT/$name"
  if [ -s "$f" ] && echo "$sum  $f" | sha256sum -c --status; then
    continue
  fi
  gb=$(df -Pk "$OUT" | awk 'NR==2{print int($4/1048576)}')
  [ "$gb" -ge "$MIN_FREE_GB" ] || { echo "fetch-vc1: only ${gb} GB free" >&2; exit 1; }
  curl -fsSL --retry 3 --retry-all-errors -A 'specular-corpus-fetch/1 (test corpus)' -o "$f.part" "$FATE/$name"
  [ "$(stat -c %s "$f.part")" = "$size" ] || { echo "fetch-vc1: $name has the wrong size" >&2; exit 1; }
  echo "$sum  $f.part" | sha256sum -c --status || { echo "fetch-vc1: $name hash mismatch" >&2; exit 1; }
  mv "$f.part" "$f"
done
echo "fetch-vc1: ${#FILES[@]} streams in $OUT"
