#!/usr/bin/env bash
# Fetch the JVT / ITU-T H.264 conformance bitstreams (the copy FFmpeg's FATE suite mirrors, the
# same files as the ITU's draft_conformance tree) into tools/corpus/conformance/h264/
# (gitignored, never committed), and write libavcodec's per-frame MD5s next to them as the
# oracle for crates/formats/specular-h264/tests/conformance.rs.
#
#   <name>.<ext>         the Annex B bitstream as published
#   <name>.framemd5      `ffmpeg -f framemd5` of the decode, output order, native pix_fmt
#   manifest.tsv         name, file, width, height, pix_fmt, field_order, frames
#
# Usage: tools/corpus/h264-conformance.sh [--clean]. Needs curl, ffmpeg and ffprobe (oracle only).
# Downloads are untrusted data: nothing from them is run. Stops below MIN_FREE_GB (default 8).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT="${SPECULAR_H264_CORPUS:-$ROOT/tools/corpus/conformance/h264}"
BASE=https://fate-suite.ffmpeg.org/h264-conformance
MIN_FREE_GB=${MIN_FREE_GB:-8}
for a in "$@"; do
  case $a in
    --clean) rm -rf "$OUT"; echo "removed $OUT"; exit 0 ;;
    *) echo "usage: $0 [--clean]" >&2; exit 2 ;;
  esac
done
for t in curl ffmpeg ffprobe; do
  command -v $t >/dev/null || { echo "h264-conformance: $t not found" >&2; exit 1; }
done
check_space() {
  local gb
  gb=$(df -Pk "$ROOT" | awk 'NR==2{print int($4/1048576)}')
  [ "$gb" -ge "$MIN_FREE_GB" ] || { echo "h264-conformance: only ${gb} GB free" >&2; exit 1; }
}
mkdir -p "$OUT"

listing() {
  curl -fsSL "$1/" | grep -o 'href="[^"?/][^"]*"' | sed 's/^href="//;s/"$//' \
    | grep -Ev '/$|^md5sum$'
}

: > "$OUT/manifest.tsv.part"
fetch_dir() {
  local url=$1 sub=$2
  for f in $(listing "$url"); do
    local dest="$OUT/$sub$f"
    mkdir -p "$(dirname "$dest")"
    if [ ! -s "$dest" ]; then
      check_space
      curl -fsSL --retry 3 -A 'specular-corpus-fetch/1 (test corpus)' -o "$dest.part" "$url/$f"
      mv "$dest.part" "$dest"
    fi
    local stem="${dest%.*}"
    local name="$sub${f%.*}"
    if [ ! -s "$stem.framemd5" ]; then
      # Output order, as FATE decodes them.
      ffmpeg -hide_banner -loglevel error -y -i "$dest" \
        -f framemd5 "$stem.framemd5.part" || { echo "ffmpeg failed on $f" >&2; rm -f "$stem.framemd5.part"; continue; }
      mv "$stem.framemd5.part" "$stem.framemd5"
    fi
    local info
    info=$(ffprobe -v error -select_streams v:0 \
      -show_entries stream=width,height,pix_fmt,field_order -of csv=p=0 "$dest" || true)
    local frames
    frames=$(grep -vc '^#' "$stem.framemd5" || true)
    printf '%s\t%s\t%s\t%s\n' "$name" "$sub$f" "${info//,/	}" "$frames" >> "$OUT/manifest.tsv.part"
  done
}

fetch_dir "$BASE" ""
fetch_dir "$BASE/FRext" "FRext/"
mv "$OUT/manifest.tsv.part" "$OUT/manifest.tsv"
echo "h264-conformance: $(wc -l < "$OUT/manifest.tsv") streams in $OUT"
