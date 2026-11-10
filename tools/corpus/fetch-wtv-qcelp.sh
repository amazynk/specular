#!/usr/bin/env bash
# Fetch the WTV / DVR-MS / QCELP / EVRC / OGM sample corpus (gitignored: nothing here is
# committed). The ffmpeg sample archive is unlicensed user uploads: dev corpus only.
#
# Usage: tools/corpus/fetch-wtv-qcelp.sh
#   OUT defaults to $SPECULAR_WTV_CORPUS, else $CARGO_TARGET_DIR/corpus/wtv1, else
#   target/corpus/wtv1.
#   Then: SPECULAR_WTV_CORPUS=<OUT> cargo test -p specular-video --test wtv_qcelp -- --ignored
#   and   SPECULAR_WTV_CORPUS=<OUT> cargo test -p specular-qcelp --test oracle -- --ignored
#   (the oracle tests run the system ffmpeg on these files).
#
# About 120 MB. The mirror is slow and flaky: failed files are reported and skipped, and a
# rerun only fetches what is missing. Downloads are untrusted data: only copied here.
# Stops if free space drops below MIN_FREE_GB (default 8).
set -uo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_WTV_CORPUS:-${CARGO_TARGET_DIR:-$ROOT/target}/corpus/wtv1}
MIN_FREE_GB=${MIN_FREE_GB:-8}
mkdir -p "$OUT"

die() { echo "fetch-wtv-qcelp: $*" >&2; exit 1; }

check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
}

failed=0
# get SUBDIR/NAME URL
get() {
  local name=$1 url=$2
  check_space
  mkdir -p "$OUT/$(dirname "$name")"
  [[ -s "$OUT/$name" ]] && return 0
  echo "fetch-wtv-qcelp: downloading $name"
  if curl -fL --progress-bar --retry 3 --connect-timeout 30 -A 'Mozilla/5.0 (specular-corpus-fetch)' \
      -o "$OUT/$name.part" "$url"; then
    mv "$OUT/$name.part" "$OUT/$name"
  else
    rm -f "$OUT/$name.part"
    echo "fetch-wtv-qcelp: failed: $url" >&2
    failed=$((failed + 1))
  fi
}

S=https://samples.ffmpeg.org
for f in for474.qcp 0047704457.qcp qcpclip.qcp 0036580847.QCP Mariah.qcp gumanoidi.qcp \
    2009-02-25T13_06_59-08_00.qcp 0084706783.QCP; do
  get "qcp/$f" "$S/A-codecs/qcp/$f"
done
for f in 13k_fixed_full.qcp 13k_fixed_half.qcp 13k_var_full.qcp 13k_var_half.qcp test8.qcp \
    saycheese.qcp warp.qcp; do
  get "suite-qcp/$f" "$S/A-codecs/suite/QCP/$f"
done
for f in 01_ENTERSANDMAN_evrc_.qcp 01_Sign_Of_The_Cross_evrc.qcp 02_Lord_Of_The_Flies_evrc.qcp; do
  get "evrc/$f" "$S/A-codecs/suite/EVRC/$f"
done
for f in 3gpp2_50k_h263_qcelp_o.3g2 3gpp2_50k_mpeg4_qcelp_o.3g2; do
  get "suite-3g2/$f" "$S/A-codecs/suite/3G2/$f"
done
for f in test.3g2 qtaudio-qcelp-problem.3g2 codeblue-interrupt01.mov tube.3g2 h263.mov \
    Test3gpp-mp4a.3g2; do
  get "qclp/$f" "$S/A-codecs/qclp/$f"
done
for f in SS2.dvr-ms microsoft-new-way-to-shove-mpeg2-in-asf.dvr-ms; do
  get "dvr_ms/$f" "$S/dvr_ms/$f"
done
get "wtv/law-and-order.wtv" "$S/wtv/Law+and+Order-+SVU_Five_2008_08_12_23_36_55.wtv"
get "wtv/bbc-two.wtv" "$S/wtv/BBC%20News_BBC%20TWO_2010_06_30_01_23_00.wtv"
for f in md_ogg.ogm mpeg-in-ogm.ogm; do
  get "ogm/$f" "$S/ogg/$f"
done

# Synthetic WTV files from ffmpeg's own muxer (small, regenerated every run).
if command -v ffmpeg > /dev/null; then
  mkdir -p "$OUT/synth"
  ff() { ffmpeg -hide_banner -loglevel error -y "$@"; }
  ff -f lavfi -i testsrc=size=320x240:rate=25:duration=2 -f lavfi -i sine=f=440:d=2:r=48000 \
    -c:v mpeg2video -q:v 4 -c:a ac3 -b:a 128k -f wtv "$OUT/synth/mpeg2-ac3.wtv"
  ff -f lavfi -i testsrc=size=320x240:rate=25:duration=2 -f lavfi -i sine=f=440:d=2:r=48000 \
    -c:v mpeg2video -q:v 4 -c:a mp2 -b:a 128k -f wtv "$OUT/synth/mpeg2-mp2.wtv"
  ff -f lavfi -i testsrc=size=320x240:rate=25:duration=2 -f lavfi -i sine=f=440:d=2:r=48000 \
    -f lavfi -i sine=f=660:d=2:r=48000 -map 0 -map 1 -map 2 \
    -c:v mpeg2video -q:v 4 -c:a mp2 -b:a 128k -f wtv "$OUT/synth/two-audio.wtv"
  ff -f lavfi -i testsrc=size=320x240:rate=25:duration=2 -f lavfi -i sine=f=440:d=2:r=48000 \
    -c:v libx264 -pix_fmt yuv420p -g 25 -c:a aac -b:a 96k -f wtv "$OUT/synth/h264-aac.wtv" 2> /dev/null \
    || echo "fetch-wtv-qcelp: no libx264; h264-aac.wtv skipped"
else
  echo "fetch-wtv-qcelp: no ffmpeg; synthetic WTV files skipped"
fi

echo "fetch-wtv-qcelp: done ($(du -sh "$OUT" | cut -f1) in $OUT, $failed failed)"
