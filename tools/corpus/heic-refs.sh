#!/usr/bin/env bash
# Generate the HEIC reference set for crates/formats/specular-heic/tests/libheif_refs.rs into
# tools/corpus/out/heic-ref/ (gitignored). The tests skip themselves when it is absent.
#
#   gen/            synthetic HEICs encoded with heif-enc (x265) from the Kodak images
#                   (tools/corpus/fetch-conformance.sh src_kodak): 8/10-bit, 4:2:0/4:2:2/4:4:4,
#                   lossless (GBR), alpha, odd sizes, 12/48 MP grids.
#   ref/<name>.png  libheif `heif-dec -C nn` output (RGB(A), nearest-neighbour chroma)
#   ref/<name>.y4m  libheif `heif-dec` y4m output (composed 8-bit 4:2:0 YCbCr; bit-exact
#                   reference for 8-bit 4:2:0 images, converted otherwise)
#   ref/<name>.<item>.yuv  libavcodec decode of every coded item (ffmpeg's HEIF demuxer exposes
#                   one stream per item, stream id = item id), native pix_fmt, codec cropping
#                   only: the bit-exact per-tile oracle for every chroma format / bit depth.
#   manifest.tsv    name, path, libheif status, then `item:WxH:pix_fmt` per decoded item.
#
# Inputs: gen/, the specular-decode-host HEIC fixtures and every .heic/.heif/.hif under the
# fetched conformance corpora. Usage: tools/corpus/heic-refs.sh [--no-gen] [--clean]
# Needs heif-enc/heif-dec (libheif >= 1.17), ffmpeg/ffprobe (>= 7.1), magick.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT="$ROOT/tools/corpus/out/heic-ref"
CONF="$ROOT/tools/corpus/conformance"
MIN_FREE_GB=${MIN_FREE_GB:-8}
GEN=1
for a in "$@"; do
  case $a in
    --no-gen) GEN=0 ;;
    --clean) rm -rf "$OUT"; echo "removed $OUT"; exit 0 ;;
    *) echo "usage: $0 [--no-gen] [--clean]" >&2; exit 2 ;;
  esac
done
for t in heif-enc heif-dec ffmpeg ffprobe magick; do
  command -v $t >/dev/null || { echo "heic-refs: $t not found" >&2; exit 1; }
done
check_space() {
  local gb
  gb=$(df -Pk "$ROOT" | awk 'NR==2{print int($4/1048576)}')
  [ "$gb" -ge "$MIN_FREE_GB" ] || { echo "heic-refs: only ${gb} GB free" >&2; exit 1; }
}
check_space
mkdir -p "$OUT/gen" "$OUT/ref" "$OUT/src"

KODAK="$CONF/kodak"
if [ $GEN = 1 ]; then
  [ -f "$KODAK/kodim23.png" ] || { echo "heic-refs: run fetch-conformance.sh src_kodak first" >&2; exit 1; }
  src() { # name W H [extra magick args]  -> $OUT/src/name.png
    local n=$1 w=$2 h=$3; shift 3
    [ -f "$OUT/src/$n.png" ] || magick "$KODAK/kodim23.png" -filter Lanczos -resize "${w}x${h}!" "$@" "$OUT/src/$n.png"
  }
  enc() { # name src [heif-enc args]
    local n=$1 s=$2; shift 2
    [ -f "$OUT/gen/$n.heic" ] || heif-enc "$@" -o "$OUT/gen/$n.heic" "$OUT/src/$s.png" >/dev/null
  }
  src mid 1440 960
  src mid16 1440 960 -depth 16
  src odd 1001 667
  src alpha 1023 769 \( -size 1023x769 radial-gradient:white-black \) -alpha off -compose copy_opacity -composite
  src s12 4032 3024
  src s12_16 4032 3024 -depth 16
  src s48 8064 6048
  src sodd 4033 3021
  enc mid_q50 mid -q 50
  enc mid_420 mid -q 90
  enc mid_422 mid -q 90 -p chroma=422
  enc mid_444 mid -q 90 -p chroma=444
  enc mid_10bit mid16 -q 90 -b 10
  enc mid_10bit_444 mid16 -q 90 -b 10 -p chroma=444
  enc mid_lossless mid -L
  enc odd_1001x667 odd -q 90
  enc alpha_1023x769 alpha -q 90
  enc single12mp_420 s12 -q 90
  enc grid12mp_420 s12 -q 90 --cut-tiles 512
  enc grid12mp_10bit s12_16 -q 90 -b 10 --cut-tiles 512
  enc grid48mp_420 s48 -q 90 --cut-tiles 512
  enc grid_odd_4033x3021 sodd -q 90 --cut-tiles 512
fi

LIST="$OUT/inputs.txt"
{
  find "$OUT/gen" -name '*.heic'
  find "$ROOT/crates/specular-decode-host/tests/data" -maxdepth 1 -name '*.heic' 2>/dev/null
  for d in libheif nokia-heif mpeg-ffc zune-image; do
    [ -d "$CONF/$d" ] && find "$CONF/$d" -type f \( -iname '*.heic' -o -iname '*.heif' -o -iname '*.hif' \)
  done
} | sort -u >"$LIST"

MAN="$OUT/manifest.tsv"
: >"$MAN"
n=0
while IFS= read -r f; do
  rel=${f#"$ROOT/"}
  name=$(echo "$rel" | sed 's|[/ ]|_|g')
  st=ok
  if ! timeout 300 heif-dec --quiet --disable-limits -C nn "$f" "$OUT/ref/$name.png" >/dev/null 2>&1; then
    st=fail
    rm -f "$OUT/ref/$name.png"
  fi
  timeout 300 heif-dec --quiet --disable-limits "$f" "$OUT/ref/$name.y4m" >/dev/null 2>&1 || rm -f "$OUT/ref/$name.y4m"
  items=()
  while IFS='|' read -r idx w h pf id _; do
    idx=${idx#index=} w=${w#width=} h=${h#height=} pf=${pf#pix_fmt=} id=$((${id#id=}))
    [ -n "$pf" ] && [ "$pf" != unknown ] || continue
    if timeout 300 ffmpeg -nostdin -v error -apply_cropping codec -noautorotate -i "$f" -map "0:$idx" \
      -frames:v 1 -f rawvideo "$OUT/ref/$name.$id.yuv" -y 2>/dev/null && [ -s "$OUT/ref/$name.$id.yuv" ]; then
      items+=("$id:${w}x${h}:$pf")
    else
      rm -f "$OUT/ref/$name.$id.yuv"
    fi
  done < <(timeout 60 ffprobe -v error -select_streams v -show_entries stream=index,width,height,pix_fmt,id \
    -of compact=p=0 "$f" 2>/dev/null | head -1024 || true)
  printf '%s\t%s\t%s\t%s\n' "$name" "$rel" "$st" "${items[*]:-}" >>"$MAN"
  n=$((n + 1))
  [ $((n % 25)) = 0 ] && { echo "heic-refs: $n files"; check_space; }
done <"$LIST"
echo "heic-refs: $n files -> $MAN ($(du -sh "$OUT" | cut -f1))"
