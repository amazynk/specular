#!/usr/bin/env bash
# Fetch FFmpeg's FATE subtitle samples for specular-subs's oracle test (gitignored: nothing
# here is committed). About 1 MB, SHA-256 pinned.
#
# Usage: tools/corpus/fetch-subs.sh
#   OUT defaults to $SPECULAR_SUBS_CORPUS, else $CARGO_TARGET_DIR/corpus/subs, else
#   target/corpus/subs.
#   Then: SPECULAR_SUBS_CORPUS=<OUT> cargo test -p specular-subs --test fate -- --ignored
#   (the test compares with ffmpeg when it is installed, else it only checks the files read).
#
# Downloads are untrusted data: this script only copies them.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_SUBS_CORPUS:-${CARGO_TARGET_DIR:-$ROOT/target}/corpus/subs}
MIN_FREE_GB=${MIN_FREE_GB:-8}
BASE=https://fate-suite.ffmpeg.org/sub
mkdir -p "$OUT"

die() { echo "fetch-subs: $*" >&2; exit 1; }

avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
(( avail_kb >= MIN_FREE_GB * 1024 * 1024 )) || die "less than ${MIN_FREE_GB} GB free; stopping"

# name sha256
FILES=(
  "SubRip_capability_tester.srt 1f3b306a522433cffe504e176fb9a955020bcd3e342dc8aec75ff96fe17c5732"
  "WebVTT_capability_tester.vtt ed5daa0f1397afbc202f3efd809bef605620d7fe359795221079f3625e71bbbc"
  "WebVTT_extended_tester.vtt 24e4c01a5a44f4ba70453ef7194ae4c3f8ff1a9e66430529857c223a7fd41e52"
  "a9-misc.ssa 634c3d1e8a99d8049bbfe0a48326b3cd1884fef1688e3b086658748e0a3de1d8"
  "1ededcbd7b.ass 813defb6451f51cf7fbce7b7fa415f2af64ea9bfcd00742fa3ec025089fb11d2"
  "badsyntax.srt 49d15287dc75d54bbb495c51eb10ecec0f1418b00cfce188d83b01dbdaf3c13e"
  "madness.srt 16efc4e9ba05cd2308eac73b63b20cf272a9486e297c8b8ec6d9f50badc28b57"
  "empty-events-2167.srt 743067ac7eb3444b0099a04b924c4558ab654b1b7c7eee07d9f38a15fbcbb955"
  "MovText_capability_tester.mp4 636ba1bdf1ab65a3de6c3e99b097a6ce93a4f971d7eae90171e55ab848adf61f"
  "pgs_sub.sup ce6d8ed89cf557e34b90d49c5ce955731935c485b93892f69ab276cfe0c36c27"
  "vobsub.idx c4d073e2bbf515409e9ee8bf32114b81c1a11178d3bf0d52d88ebab0615525e2"
  "vobsub.sub 5f6ac43324e5a35c6e525aa0c76bafa9dd965a17ce0880fd7eb69e19b6853c8b"
)

for entry in "${FILES[@]}"; do
  name=${entry%% *}
  sum=${entry##* }
  if [[ ! -s "$OUT/$name" ]]; then
    echo "fetch-subs: $name ($BASE/$name)"
    curl -fsSL --http1.1 --retry 5 --retry-all-errors -o "$OUT/$name.part" "$BASE/$name" \
      || { rm -f "$OUT/$name.part"; die "download failed: $name"; }
    mv "$OUT/$name.part" "$OUT/$name"
  fi
  if [[ $sum != "-" ]]; then
    got=$(sha256sum "$OUT/$name" | cut -d' ' -f1)
    [[ $got == "$sum" ]] || die "$name: SHA-256 $got, expected $sum"
  fi
done
# Subtitles inside a Matroska movie, for the container side (needs ffmpeg).
if command -v ffmpeg >/dev/null; then
  ffmpeg -v error -y -f lavfi -i color=c=black:s=64x48:d=90 \
    -i "$OUT/SubRip_capability_tester.srt" -i "$OUT/a9-misc.ssa" -i "$OUT/pgs_sub.sup" \
    -i "$OUT/WebVTT_capability_tester.vtt" -i "$OUT/vobsub.idx" \
    -map 0:v -map 1:s -map 2:s -map 3:s -map 4:s -map 5:s -c:v mpeg4 -c:s copy "$OUT/mkv_subs.mkv"
fi
echo "fetch-subs: $(ls "$OUT" | wc -l) files in $OUT"
