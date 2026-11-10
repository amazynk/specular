#!/usr/bin/env bash
# Fetch the JCT-VC HEVC_v1 conformance bitstreams (the copy FFmpeg's FATE suite mirrors, which
# also carries a few RExt and MV-HEVC streams) into tools/corpus/conformance/hevc/ (gitignored,
# never committed), and write libavcodec's per-frame MD5s next to them as the oracle for
# crates/formats/specular-heic/hevc/tests/conformance.rs.
#
#   <name>.<ext>         the Annex B bitstream as published
#   <name>.framemd5      `ffmpeg -f framemd5` of the decode, output order, native pix_fmt,
#                        conformance window applied exactly (-flags unaligned), one thread
#   <name>.yuv.md5       the JCT-VC package's MD5 of the whole decoded .yuv, where one exists
#   manifest.tsv         name, file, width, height, pix_fmt, frames (- without frame MD5s)
#
# Usage: tools/corpus/hevc-conformance.sh [--clean]. Needs curl, ffmpeg and ffprobe (oracle only).
# Downloads are untrusted data: nothing from them is run. Stops below MIN_FREE_GB (default 8).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT="$ROOT/tools/corpus/conformance/hevc"
BASE=https://fate-suite.ffmpeg.org/hevc-conformance
MIN_FREE_GB=${MIN_FREE_GB:-8}
for a in "$@"; do
  case $a in
    --clean) rm -rf "$OUT"; echo "removed $OUT"; exit 0 ;;
    *) echo "usage: $0 [--clean]" >&2; exit 2 ;;
  esac
done
for t in curl ffmpeg ffprobe; do
  command -v $t >/dev/null || { echo "hevc-conformance: $t not found" >&2; exit 1; }
done
check_space() {
  local gb
  gb=$(df -Pk "$ROOT" | awk 'NR==2{print int($4/1048576)}')
  [ "$gb" -ge "$MIN_FREE_GB" ] || { echo "hevc-conformance: only ${gb} GB free" >&2; exit 1; }
}
mkdir -p "$OUT"

# Only bitstream files, by name; anything else in the listing is ignored.
listing() {
  curl -fsSL "$1/" | grep -o 'href="[^"?/][^"]*"' | sed 's/^href="//;s/"$//' \
    | grep -E '^[A-Za-z0-9_.-]+\.(bit|bin)$'
}

: > "$OUT/manifest.tsv.part"
for f in $(listing "$BASE"); do
  dest="$OUT/$f"
  if [ ! -s "$dest" ]; then
    check_space
    curl -fsSL --retry 3 -A 'specular-corpus-fetch/1 (test corpus)' -o "$dest.part" "$BASE/$f"
    mv "$dest.part" "$dest"
  fi
  stem="${dest%.*}"
  name="${f%.*}"
  if [ ! -s "$stem.framemd5" ]; then
    # Output order, every picture libavcodec outputs, cropped exactly.
    if ! ffmpeg -hide_banner -loglevel error -y -threads 1 -flags unaligned -f hevc -i "$dest" \
        -fps_mode passthrough -f framemd5 "$stem.framemd5.part"; then
      # Listed all the same: the JCT-VC MD5 below may still be an oracle.
      echo "ffmpeg failed on $f (no frame MD5s)" >&2
      rm -f "$stem.framemd5.part"
    else
      mv "$stem.framemd5.part" "$stem.framemd5"
    fi
  fi
  info=$(ffprobe -v error -f hevc -select_streams v:0 \
    -show_entries stream=width,height,pix_fmt -of csv=p=0 "$dest" | head -1 || true)
  frames=$( [ -s "$stem.framemd5" ] && grep -vc '^#' "$stem.framemd5" || echo -)
  printf '%s\t%s\t%s\t%s\n' "$name" "$f" "${info//,/	}" "$frames" >> "$OUT/manifest.tsv.part"
done
mv "$OUT/manifest.tsv.part" "$OUT/manifest.tsv"
echo "hevc-conformance: $(wc -l < "$OUT/manifest.tsv") streams in $OUT"

# The JCT-VC packages carry the reference decoder's MD5 of the whole decoded .yuv. libavcodec
# is not right on every stream (NUT_A_ericsson_5 drops two RADL pictures, for one), so where a
# package of the same name and version exists its MD5 is kept as <name>.yuv.md5, the test's
# second oracle. Only .md5 members are read out of each zip (to stdout); the zip is deleted.
JCTVC=https://www.itu.int/wftp3/av-arch/jctvc-site/bitstream_exchange/draft_conformance/HEVC_v1
itu=$(curl -fsSL "$JCTVC/" | grep -o 'HEVC_v1/[A-Za-z0-9_.-]*\.zip' | sed 's|^HEVC_v1/||;s|\.zip$||' || true)
n=0
for name in $(cut -f1 "$OUT/manifest.tsv"); do
  grep -qx "$name" <<< "$itu" || continue
  [ -s "$OUT/$name.yuv.md5" ] && { n=$((n + 1)); continue; }
  check_space
  tmpzip="$OUT/$name.zip.part"
  if curl -fsSL --retry 3 -A 'specular-corpus-fetch/1 (test corpus)' -o "$tmpzip" "$JCTVC/$name.zip"; then
    # A package's .md5 members hold the bitstream's MD5 and the decoded .yuv's, in one file
    # or in two: members named for the yuv are read first, and the bitstream's MD5 is skipped.
    bitsum=$(md5sum < "$OUT/$(awk -F'\t' -v n="$name" '$1 == n {print $2}' "$OUT/manifest.tsv")" | cut -c1-32)
    md5=
    for m in $(unzip -Z1 "$tmpzip" 2>/dev/null | grep -E '^[A-Za-z0-9_./-]+\.md5$' \
        | awk '{print (tolower($0) ~ /yuv/ ? 0 : 1) "\t" $0}' | sort | cut -f2); do
      md5=$(unzip -p -C "$tmpzip" "$m" 2>/dev/null | grep -a -o -E '\b[0-9a-fA-F]{32}\b' \
        | tr 'A-F' 'a-f' | grep -v -x "$bitsum" | head -1 || true)
      [ -n "$md5" ] && break
    done
    [ -n "$md5" ] && { echo "$md5" > "$OUT/$name.yuv.md5"; n=$((n + 1)); }
  fi
  rm -f "$tmpzip"
done
# FATE carries some streams under an older version's name (RPS_D_ericsson_5 is the very file
# RPS_D_ericsson_6 is): a byte-identical bitstream shares the reference MD5.
sums=$(cut -f2 "$OUT/manifest.tsv" | (cd "$OUT" && xargs md5sum))
while read -r sum file; do
  name="${file%.*}"
  [ -s "$OUT/$name.yuv.md5" ] && continue
  for twin in $(awk -v s="$sum" -v f="$file" '$1 == s && $2 != f {print $2}' <<< "$sums"); do
    if [ -s "$OUT/${twin%.*}.yuv.md5" ]; then
      cp "$OUT/${twin%.*}.yuv.md5" "$OUT/$name.yuv.md5"
      n=$((n + 1))
      break
    fi
  done
done <<< "$sums"
echo "hevc-conformance: $n JCT-VC reference MD5s"
