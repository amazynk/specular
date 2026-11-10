#!/usr/bin/env bash
# Fetch the JVT professional-profile conformance bitstreams (High 10 Intra, High 4:2:2 Intra,
# High 4:4:4 Predictive and Intra, CAVLC 4:4:4 Intra; up to 14 bits, separate colour planes,
# lossless) into tools/corpus/conformance/h264-pro/ (gitignored, never committed), in the
# layout h264-conformance.sh writes, so crates/formats/specular-h264/tests/conformance.rs
# runs on them with SPECULAR_H264_CORPUS pointing here:
#
#   <name>.264        the Annex B bitstream from the package
#   <name>.txt        the package's readme (profile, chroma format, bit depth, tools)
#   <name>.framemd5   `ffmpeg -f framemd5` of the decode (one thread, display order); for
#                     the streams ffmpeg refuses (separate colour planes), the JM reference
#                     decoder's output in the same form, when JM is built (JM_LDECOD, default
#                     the ldecod tools/corpus/h264-jm-dp.sh builds; tool only, never linked)
#   manifest.tsv      name, file, width, height, pix_fmt, frames
#
# The very large 2008 streams (.bits, 30-110 MB each) are left out. About 250 MB.
# Usage: tools/corpus/h264-professional.sh [--clean]. Needs curl, unzip, ffmpeg, ffprobe.
# OUT can be moved with SPECULAR_H264_PRO_CORPUS. Downloads are untrusted data: only the
# bitstream and readme members are read out of each zip; nothing from them is run.
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
TD=${CARGO_TARGET_DIR:-$ROOT/target}
JM_LDECOD=${JM_LDECOD:-$TD/jm/JM-8b34eee1576952dc2a04cd2fdb52febfde4030b2/bin/ldecod_static}
OUT=${SPECULAR_H264_PRO_CORPUS:-$ROOT/tools/corpus/conformance/h264-pro}
BASE=https://www.itu.int/wftp3/av-arch/jvt-site/draft_conformance/Professional_profiles
MIN_FREE_GB=${MIN_FREE_GB:-8}
for a in "$@"; do
  case $a in
    --clean) rm -rf "$OUT"; echo "removed $OUT"; exit 0 ;;
    *) echo "usage: $0 [--clean]" >&2; exit 2 ;;
  esac
done
for t in curl unzip ffmpeg ffprobe; do
  command -v $t >/dev/null || { echo "h264-professional: $t not found" >&2; exit 1; }
done
mkdir -p "$OUT"
check_space() {
  local gb
  gb=$(df -Pk "$OUT" | awk 'NR==2{print int($4/1048576)}')
  [ "$gb" -ge "$MIN_FREE_GB" ] || { echo "h264-professional: only ${gb} GB free" >&2; exit 1; }
}
# jm_md5 NAME: NAME.framemd5 from the JM decoder's YUV (cropped, display order, samples
# deeper than 8 bits as little-endian u16), and NAME.jm with "width,height,jm".
jm_md5() {
  local w="$OUT/jm-work" log fmt depth cf
  mkdir -p "$w"
  log=$(cd "$w" && "$JM_LDECOD" -d "$(dirname "$JM_LDECOD")/../cfg/decoder.cfg" \
    -p InputFile="$OUT/$1.264" -p OutputFile="$w/out.yuv" -p RefFile="$w/none.yuv" 2>&1) \
    || { echo "JM failed on $1" >&2; rm -rf "$w"; return; }
  fmt=$(sed -n 's/^Image Format : \([0-9]*x[0-9]*\).*/\1/p' <<< "$log")
  cf=$(sed -n 's/^Color Format : \(4:[0-9]:[0-9]\) (\([0-9]*\):.*/\1 \2/p' <<< "$log")
  depth=${cf#* }
  python3 -I - "$w/out.yuv" "${fmt%x*}" "${fmt#*x}" "${cf% *}" "$depth" > "$OUT/$1.framemd5" <<'PY'
import hashlib, sys
path, w, h, cf, depth = sys.argv[1], int(sys.argv[2]), int(sys.argv[3]), sys.argv[4], int(sys.argv[5])
c = {"4:2:0": (w + 1) // 2 * ((h + 1) // 2), "4:2:2": (w + 1) // 2 * h, "4:4:4": w * h}[cf]
size = (w * h + 2 * c) * (2 if depth > 8 else 1)
with open(path, "rb") as f:
    n = 0
    while (b := f.read(size)) and len(b) == size:
        print(f"0, {n:10}, {n:10}, {1:8}, {size:8}, {hashlib.md5(b).hexdigest()}")
        n += 1
PY
  printf '%s,%s,jm\n' "${fmt%x*}" "${fmt#*x}" > "$OUT/$1.jm"
  rm -rf "$w"
}
packages=$(curl -fsSL "$BASE/" | grep -o 'Professional_profiles/[A-Za-z0-9_.-]*\.zip' \
  | sed 's|^Professional_profiles/||;s|\.zip$||' | sort -u)
: > "$OUT/manifest.tsv.part"
for pkg in $packages; do
  if [ ! -s "$OUT/$pkg.264" ]; then
    check_space
    zip="$OUT/$pkg.zip.part"
    curl -fsSL --retry 3 -A 'specular-corpus-fetch/1 (test corpus)' -o "$zip" "$BASE/$pkg.zip" \
      || { echo "download failed: $pkg" >&2; rm -f "$zip"; continue; }
    member=$(unzip -Z1 "$zip" | grep -E '^[A-Za-z0-9_./-]+\.(264|26l|jsv|avc|h264|bit|bits)$' | head -1 || true)
    if [ -z "$member" ]; then
      echo "$pkg: no bitstream in the package" >&2
      rm -f "$zip"
      continue
    fi
    unzip -p -C "$zip" "$member" > "$OUT/$pkg.264"
    readme=$(unzip -Z1 "$zip" | grep -iE '^[A-Za-z0-9_./-]+\.txt$' | head -1 || true)
    [ -n "$readme" ] && unzip -p -C "$zip" "$readme" | tr -d '\r' > "$OUT/$pkg.txt"
    rm -f "$zip"
  fi
  if [ ! -s "$OUT/$pkg.framemd5" ]; then
    if ffmpeg -hide_banner -loglevel error -y -threads 1 -f h264 -i "$OUT/$pkg.264" \
        -fps_mode passthrough -f framemd5 "$OUT/$pkg.framemd5.part"; then
      mv "$OUT/$pkg.framemd5.part" "$OUT/$pkg.framemd5"
    elif [ -x "$JM_LDECOD" ]; then
      rm -f "$OUT/$pkg.framemd5.part"
      jm_md5 "$pkg"
    else
      echo "ffmpeg failed on $pkg (no frame MD5s; build JM for its oracle)" >&2
      rm -f "$OUT/$pkg.framemd5.part"
    fi
  fi
  info=$(ffprobe -v error -f h264 -select_streams v:0 \
    -show_entries stream=width,height,pix_fmt -of csv=p=0 "$OUT/$pkg.264" | head -1 || true)
  if [ -s "$OUT/$pkg.jm" ]; then
    info=$(cat "$OUT/$pkg.jm")
  fi
  frames=$( [ -s "$OUT/$pkg.framemd5" ] && grep -vc '^#' "$OUT/$pkg.framemd5" || echo -)
  printf '%s\t%s\t%s\t%s\n' "$pkg" "$pkg.264" "${info//,/	}" "$frames" >> "$OUT/manifest.tsv.part"
done
mv "$OUT/manifest.tsv.part" "$OUT/manifest.tsv"
echo "h264-professional: $(wc -l < "$OUT/manifest.tsv") streams in $OUT"
