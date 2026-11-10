#!/usr/bin/env bash
# Fetch a small spread of JVET VVC (H.266) conformance bitstreams and libheif's VVC test
# image into tools/corpus/conformance/vvc/ (gitignored, never committed), and write the
# oracle for crates/formats/specular-vvc/tests/conformance.rs next to them.
#
#   <name>.bit           the Annex B bitstream from the JVET conformance package
#   <name>.yuv.md5       the package's MD5 of the whole decoded .yuv, where one exists
#   <name>.framemd5      `ffmpeg -f framemd5` of the decode (native pix_fmt, one thread)
#   <name>.f0.yuv        the first output picture from ffmpeg's native vvc decoder, raw
#   vvc32.heif           libheif's fuzzing-corpus VVC image (plain file)
#   manifest.tsv         name, width, height, pix_fmt, frames
#
# Usage: tools/corpus/fetch-vvc1.sh [--clean]. Needs curl, unzip and ffmpeg (oracle only).
# Downloads are untrusted data: nothing from them is run. Stops below MIN_FREE_GB (default 8).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT="$ROOT/tools/corpus/conformance/vvc"
BASE=https://www.itu.int/wftp3/av-arch/jvet-site/bitstream_exchange/VVC/FDIS_r1
LIBHEIF=https://raw.githubusercontent.com/strukturag/libheif/master/fuzzing/data/corpus/vvc32.heif
MIN_FREE_GB=${MIN_FREE_GB:-8}
for a in "$@"; do
  case $a in
    --clean) rm -rf "$OUT"; echo "removed $OUT"; exit 0 ;;
    *) echo "usage: $0 [--clean]" >&2; exit 2 ;;
  esac
done
for t in curl unzip ffmpeg ffprobe; do
  command -v $t >/dev/null || { echo "fetch-vvc1: $t not found" >&2; exit 1; }
done
check_space() {
  local gb
  gb=$(df -Pk "$ROOT" | awk 'NR==2{print int($4/1048576)}')
  [ "$gb" -ge "$MIN_FREE_GB" ] || { echo "fetch-vvc1: only ${gb} GB free" >&2; exit 1; }
}
mkdir -p "$OUT"

# Still-picture and intra-tool streams first, then a spread of the in-loop filters, layouts
# and chroma formats. Most non-still streams also carry inter pictures.
STREAMS="
STILL_A_1 STILL_B_1 STILL444_A_1 STILL444_B_1
MIP_A_3 MIP_B_3 ISP_A_3 ISP_B_3 MRLP_A_2 MRLP_B_2 LFNST_A_4 LFNST_C_3 CCLM_A_2 PDPC_A_3
IBC_A_2 IBC_B_2 PALETTE_B_2 DQ_A_3 BDPCM_A_2 TRANS_A_2 JCCR_A_2 SCALING_A_1 MTS_A_4
QUANT_A_2 SDH_A_2 LMCS_A_3 DEBLOCKING_A_3 SAO_A_3 ALF_A_3 CCALF_A_3 VIRTUAL_A_3
SUBPIC_A_3 TILE_A_2 SLICES_A_3 WPP_A_3 CTU_A_4 BOUNDARY_A_3 8b400_A_2 8b420_A_2
10b422_A_5 8b444_A_2 10b444_A_3 ACT_A_3 CodingToolsSets_A_2
"

: > "$OUT/manifest.tsv.part"
n=0
total=$(echo $STREAMS | wc -w)
for s in $STREAMS; do
  n=$((n + 1))
  zip="$OUT/$s.zip"
  if [ ! -s "$OUT/$s.bit" ]; then
    check_space
    echo "[$n/$total] $s.zip"
    curl -fL --retry 3 --progress-bar -A 'specular-corpus-fetch/1 (test corpus)' \
      -o "$zip.part" "$BASE/$s.zip"
    mv "$zip.part" "$zip"
    # Only the bitstream and its MD5s; any decoded .yuv in the package is skipped.
    bit=$(unzip -Z1 "$zip" | grep -E '\.(bit|bin|266|vvc)$' | head -1 || true)
    [ -n "$bit" ] || { echo "fetch-vvc1: no bitstream in $s.zip" >&2; rm -f "$zip"; continue; }
    unzip -p "$zip" "$bit" > "$OUT/$s.bit"
    md5=$(unzip -Z1 "$zip" | grep -E '\.yuv\.md5$|_yuv\.md5$' | head -1 || true)
    [ -z "$md5" ] || unzip -p "$zip" "$md5" > "$OUT/$s.yuv.md5"
    rm -f "$zip"
  fi
  if [ ! -s "$OUT/$s.framemd5" ]; then
    if ! ffmpeg -hide_banner -loglevel error -y -threads 1 -f vvc -i "$OUT/$s.bit" \
        -fps_mode passthrough -f framemd5 "$OUT/$s.framemd5.part"; then
      echo "ffmpeg failed on $s (no frame MD5s)" >&2
      rm -f "$OUT/$s.framemd5.part"
    else
      mv "$OUT/$s.framemd5.part" "$OUT/$s.framemd5"
    fi
  fi
  if [ ! -s "$OUT/$s.f0.yuv" ]; then
    ffmpeg -hide_banner -loglevel error -y -threads 1 -f vvc -i "$OUT/$s.bit" -frames:v 1 \
      -f rawvideo "$OUT/$s.f0.yuv" || rm -f "$OUT/$s.f0.yuv"
  fi
  info=$(ffprobe -v error -select_streams v:0 -show_entries stream=width,height,pix_fmt \
    -of csv=p=0 "$OUT/$s.bit" | head -1 | tr ',' '\t')
  frames=$(grep -vc '^#' "$OUT/$s.framemd5" 2>/dev/null || echo -)
  printf '%s\t%s\t%s\n' "$s" "$info" "$frames" >> "$OUT/manifest.tsv.part"
done
mv "$OUT/manifest.tsv.part" "$OUT/manifest.tsv"

if [ ! -s "$OUT/vvc32.heif" ]; then
  check_space
  echo "vvc32.heif (libheif)"
  curl -fL --retry 3 --progress-bar -o "$OUT/vvc32.heif.part" "$LIBHEIF"
  mv "$OUT/vvc32.heif.part" "$OUT/vvc32.heif"
fi
du -sh "$OUT"
