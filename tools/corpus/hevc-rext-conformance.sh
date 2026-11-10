#!/usr/bin/env bash
# Fetch the JCT-VC HEVC range extension (RExt) conformance bitstreams (4:2:2, 4:4:4, 4:0:0,
# 12 and 16 bits, cross-component prediction, RDPCM, ...) into
# tools/corpus/conformance/hevc-rext/ (gitignored, never committed), in the layout
# hevc-conformance.sh writes, so crates/formats/specular-heic/hevc/tests/conformance.rs runs on
# them with SPECULAR_HEVC_CORPUS pointing here:
#
#   <name>.bit           the Annex B bitstream from the package
#   <name>.framemd5      `ffmpeg -f framemd5` of the decode (one thread, exact crop)
#   <name>.yuv.md5       the package's MD5 of the whole decoded .yuv, where it has one
#   manifest.tsv         name, file, width, height, pix_fmt, frames
#
# Usage: tools/corpus/hevc-rext-conformance.sh [--clean]. Needs curl, unzip, ffmpeg, ffprobe.
# OUT can be moved with SPECULAR_HEVC_REXT_CORPUS. Downloads are untrusted data: only
# bitstream and .md5 members are read out of each zip; nothing from them is run.
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_HEVC_REXT_CORPUS:-$ROOT/tools/corpus/conformance/hevc-rext}
BASE=https://www.itu.int/wftp3/av-arch/jctvc-site/bitstream_exchange/draft_conformance/RExt
MIN_FREE_GB=${MIN_FREE_GB:-8}
for a in "$@"; do
  case $a in
    --clean) rm -rf "$OUT"; echo "removed $OUT"; exit 0 ;;
    *) echo "usage: $0 [--clean]" >&2; exit 2 ;;
  esac
done
for t in curl unzip ffmpeg ffprobe; do
  command -v $t >/dev/null || { echo "hevc-rext-conformance: $t not found" >&2; exit 1; }
done
mkdir -p "$OUT"
check_space() {
  local gb
  gb=$(df -Pk "$OUT" | awk 'NR==2{print int($4/1048576)}')
  [ "$gb" -ge "$MIN_FREE_GB" ] || { echo "hevc-rext-conformance: only ${gb} GB free" >&2; exit 1; }
}
packages=$(curl -fsSL "$BASE/" | grep -o 'RExt/[A-Za-z0-9_.-]*\.zip' | sed 's|^RExt/||;s|\.zip$||' | sort -u)
: > "$OUT/manifest.tsv.part"
for pkg in $packages; do
  # Package names carry a version suffix the bitstream inside may not (`_2`).
  bit=$(ls "$OUT/$pkg".bit 2>/dev/null || true)
  if [ -z "$bit" ]; then
    check_space
    zip="$OUT/$pkg.zip.part"
    curl -fsSL --retry 3 -A 'specular-corpus-fetch/1 (test corpus)' -o "$zip" "$BASE/$pkg.zip" \
      || { echo "download failed: $pkg" >&2; rm -f "$zip"; continue; }
    member=$(unzip -Z1 "$zip" | grep -E '^[A-Za-z0-9_./-]+\.(bit|bin)$' | head -1 || true)
    if [ -z "$member" ]; then
      echo "$pkg: no bitstream in the package" >&2
      rm -f "$zip"
      continue
    fi
    unzip -p -C "$zip" "$member" > "$OUT/$pkg.bit"
    bitsum=$(md5sum < "$OUT/$pkg.bit" | cut -c1-32)
    md5=
    for m in $(unzip -Z1 "$zip" | grep -E '^[A-Za-z0-9_./-]+\.md5$' \
        | awk '{print (tolower($0) ~ /yuv/ ? 0 : 1) "\t" $0}' | sort | cut -f2); do
      md5=$(unzip -p -C "$zip" "$m" 2>/dev/null | grep -a -o -E '\b[0-9a-fA-F]{32}\b' \
        | tr 'A-F' 'a-f' | grep -v -x "$bitsum" | head -1 || true)
      [ -n "$md5" ] && break
    done
    [ -n "$md5" ] && echo "$md5" > "$OUT/$pkg.yuv.md5"
    rm -f "$zip"
  fi
  if [ ! -s "$OUT/$pkg.framemd5" ]; then
    if ffmpeg -hide_banner -loglevel error -y -threads 1 -flags unaligned -f hevc \
        -i "$OUT/$pkg.bit" -fps_mode passthrough -f framemd5 "$OUT/$pkg.framemd5.part"; then
      mv "$OUT/$pkg.framemd5.part" "$OUT/$pkg.framemd5"
    else
      echo "ffmpeg failed on $pkg (no frame MD5s)" >&2
      rm -f "$OUT/$pkg.framemd5.part"
    fi
  fi
  info=$(ffprobe -v error -f hevc -select_streams v:0 \
    -show_entries stream=width,height,pix_fmt -of csv=p=0 "$OUT/$pkg.bit" | head -1 || true)
  frames=$( [ -s "$OUT/$pkg.framemd5" ] && grep -vc '^#' "$OUT/$pkg.framemd5" || echo -)
  printf '%s\t%s\t%s\t%s\n' "$pkg" "$pkg.bit" "${info//,/	}" "$frames" >> "$OUT/manifest.tsv.part"
done
mv "$OUT/manifest.tsv.part" "$OUT/manifest.tsv"
echo "hevc-rext-conformance: $(wc -l < "$OUT/manifest.tsv") streams in $OUT"
