#!/usr/bin/env bash
# Fetch the corpus of the VP3 and Sorenson Spark decoders into
# $SPECULAR_VP3_SPARK_CORPUS (default ${CARGO_TARGET_DIR:-target}/corpus/vp3-spark; gitignored, never
# committed) and write ffmpeg's decodes beside each file as NAME.framemd5 (the oracle).
#
#   vp3/    FFmpeg's FATE samples (fate-suite.ffmpeg.org): VP3.1 in AVI, VP3 in Matroska,
#           Theora with a picture offset
#   flv1/   FATE FLV samples carrying Sorenson Spark, and clips made with ffmpeg's own FLV1
#           encoder (versions 0 and 1 of the picture header, odd sizes, many P-frames)
#
# Tests that use it:
#   SPECULAR_VP3_SPARK_CORPUS=... cargo test -p specular-theora --test vp3 -- --ignored
#   SPECULAR_VP3_SPARK_CORPUS=... cargo test -p specular-mpeg4 --test sorenson -- --ignored
# The libvpx VP9 vectors (4:4:4, 4:4:0, RGB) come from tools/corpus/specular-vp9-fetch.sh
# (SPECULAR_VP9_VECTORS=$SPECULAR_VP3_SPARK_CORPUS/vp9).
#
# Downloads are untrusted data: each is checked against its SHA-256 below and nothing from
# them is run. Needs curl and ffmpeg (8.x). Stops below MIN_FREE_GB (default 8).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_VP3_SPARK_CORPUS:-${CARGO_TARGET_DIR:-$ROOT/target}/corpus/vp3-spark}
MIN_FREE_GB=${MIN_FREE_GB:-8}
FATE=https://fate-suite.ffmpeg.org
for t in curl ffmpeg sha256sum; do command -v $t >/dev/null || { echo "$t not found" >&2; exit 1; }; done

check_space() {
  local gb
  gb=$(df -Pk "$OUT" | awk 'NR==2{print int($4/1048576)}')
  if [ "$gb" -lt "$MIN_FREE_GB" ]; then echo "only ${gb} GB free: stopping" >&2; exit 1; fi
}

# fetch DIR NAME SHA256
fetch() {
  local dir=$OUT/$1 name=$2 sum=$3 src=$4
  mkdir -p "$dir"
  if [ ! -s "$dir/$name" ]; then
    check_space
    curl -sSfL -o "$dir/$name.part" "$src"
    mv "$dir/$name.part" "$dir/$name"
  fi
  if [ "$sum" != - ] && ! echo "$sum  $dir/$name" | sha256sum -c --quiet -; then
    echo "checksum mismatch: $dir/$name" >&2
    rm -f "$dir/$name"
    exit 1
  fi
}

# ffmpeg's decode of every frame, as decoded (no scaling, no frame-rate change).
oracle() {
  ffmpeg -nostdin -hide_banner -loglevel error -y -i "$1" -map 0:v:0 -fps_mode passthrough \
    -f framemd5 "$1.framemd5"
}

fetch vp3 vp31.avi 457f17e9ebe46ca7f217f853976199c77c8321c170b40ce8531317137f0cf5c3 "$FATE/vp3/vp31.avi"
fetch vp3 coeff_level64.mkv 759ddbf18c0078c032e578ea8baac59f4b286296f31f118fb42682d5f7843348 "$FATE/vp3/coeff_level64.mkv"
fetch vp3 offset_test.ogv 83ba8d258cc3a1715be473925faf757469f67d2e29a553cf6d5a521488393f3e "$FATE/vp3/offset_test.ogv"
PKTS=$(dirname "$0")/vp3-spark-packets.py
for f in "$OUT"/vp3/*.avi "$OUT"/vp3/*.mkv "$OUT"/vp3/*.ogv; do
  oracle "$f"
  case $f in
    *.avi) python3 -I "$PKTS" "$f" "$f.pkts" ;;
    *) # Theora: its packets through an Ogg remux (headers first).
      ffmpeg -nostdin -hide_banner -loglevel error -y -i "$f" -map 0:v:0 -c copy -f ogg "$f.ogg"
      python3 -I "$PKTS" "$f.ogg" "$f.pkts"
      rm -f "$f.ogg" ;;
  esac
done

# Sorenson Spark: a real file (samples.ffmpeg.org: version 1, 2308 disposable frames, the
# deblocking flag set) and clips from ffmpeg's FLV1 encoder. The oracle decodes with the
# integer IDCT, as the decoder's other MPEG-4/H.263 oracles do.
fetch flv1 11-04-2008.flv bc1c53cce65136f7a47d6413b10862e79b332a2391912035d45e6800871d354c \
  https://samples.ffmpeg.org/FLV/11-04-2008.flv
enc() { # name size frames args...
  local name=$1 size=$2 n=$3
  shift 3
  [ -s "$OUT/flv1/$name.flv" ] && return
  ffmpeg -nostdin -hide_banner -loglevel error -y -f lavfi -i "testsrc2=size=$size:rate=25" \
    -frames:v "$n" -c:v flv "$@" "$OUT/flv1/$name.flv"
}
enc qvga 320x240 40 -g 12 -b:v 400k
enc qcif 176x144 30 -g 50 -q:v 4
enc odd 202x150 20 -g 8 -q:v 10
enc big 720x480 12 -g 6 -b:v 2M
enc mv4 352x288 24 -g 12 -flags +mv4 -q:v 6
enc loop 320x240 16 -g 8 -flags +loop -q:v 8
enc lowq 160x120 30 -g 30 -q:v 31
enc highq 160x120 10 -g 5 -q:v 1
for f in "$OUT"/flv1/*.flv; do
  ffmpeg -nostdin -hide_banner -loglevel error -y -idct int -flags +bitexact -i "$f" -map 0:v:0 \
    -fps_mode passthrough -f framemd5 "$f.framemd5"
  python3 -I "$PKTS" "$f" "$f.pkts"
done

echo "corpus in $OUT ($(du -sh "$OUT" | cut -f1))"
