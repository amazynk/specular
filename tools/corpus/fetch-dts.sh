#!/usr/bin/env bash
# Fetch DTS Coherent Acoustics test streams for specular-dts (gitignored: nothing here is
# committed) and write ffmpeg's core-only decode beside each one as the oracle.
#
# Usage: tools/corpus/fetch-dts.sh
#   OUT defaults to $SPECULAR_DTS_CORPUS, else $CARGO_TARGET_DIR/corpus/dts, else
#   target/corpus/dts.
#   Then: SPECULAR_DTS_CORPUS=<OUT> cargo test -p specular-dts --test corpus -- --ignored
#
# What it covers (about 6 MB), from FFmpeg's FATE suite (fate-suite.ffmpeg.org/dts),
# SHA-256 pinned: raw DTS (dts_es.dts, with an ES extension), DTS in MPEG-TS (dts.ts) and
# dcadec's DTS-HD streams whose core the decoder plays (5.1 core at 768 kbit/s, with XCh,
# with XLL), and two streams made with ffmpeg's DTS encoder. If ffmpeg is on PATH, its decode with `-core_only 1` is written as <name>.f32
# (interleaved f32le in the stream's channel order).
#
# Downloads are untrusted data: this script only copies files and hands them to ffmpeg.
# Stops if free space on the target filesystem drops below MIN_FREE_GB (default 8).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_DTS_CORPUS:-${CARGO_TARGET_DIR:-$ROOT/target}/corpus/dts}
MIN_FREE_GB=${MIN_FREE_GB:-8}
mkdir -p "$OUT"
: > "$OUT/.provenance.tsv"

die() { echo "fetch-dts: $*" >&2; exit 1; }

check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
}

# get PATH SHA256: an empty hash prints the hash instead of checking it (for pinning).
get() {
  local path=$1 sum=$2 base=https://fate-suite.ffmpeg.org/dts
  local name=${path##*/}
  check_space
  if [[ ! -s "$OUT/$name" ]]; then
    curl -fsSL --retry 3 -A 'specular-corpus-fetch/1 (test corpus)' -o "$OUT/$name.part" \
      "$base/$path" || { rm -f "$OUT/$name.part"; die "download failed: $base/$path"; }
    mv "$OUT/$name.part" "$OUT/$name"
  fi
  local got
  got=$(sha256sum "$OUT/$name" | cut -d' ' -f1)
  if [[ -z $sum ]]; then
    echo "unpinned $name $got"
  elif [[ $got != "$sum" ]]; then
    rm -f "$OUT/$name"
    die "$name: SHA-256 $got, expected $sum"
  fi
  printf '%s\t%s\t%s\n' "$name" "$base/$path" "$got" >> "$OUT/.provenance.tsv"
  if command -v ffmpeg > /dev/null; then
    ffmpeg -nostdin -loglevel error -y -core_only 1 -i "$OUT/$name" -map 0:a:0 -f f32le \
      "$OUT/${name%.*}.f32" || true
  fi
}

get dts_es.dts 2e2530d220d51b914d1e7acb63badb5b2dae1578f3274116206fa1aa5e597638
get dts.ts 749561299cf838d55f52d5697f78c273a6c381c5e6c8300f859dfe23252ae2c2
get dcadec-suite/core_51_24_48_768_0.dtshd 95653e7d94307a9d467c28ae10883440dcc39f06ac79d7401b27eb422cd99009
get dcadec-suite/core_51_24_48_768_1.dtshd 3b85a3bd2723c93e4c341403a45c111bc6777cfc6de890108938580f59886cc4
get dcadec-suite/xch_61_24_48_768.dtshd db7c44e47b3c01c57829ac9947a0e77aa93798475773e1f2fd74f99a607c48b7
get dcadec-suite/xll_51_24_48_768.dtshd c0830af121ec78a8271d008999cc54b99843f17b9b33203bb9adbcbc3271d0a9
# ffmpeg's own DTS encoder uses neither ADPCM prediction nor high-frequency VQ: streams it
# makes decode in full (pink noise in 5.1 at 768 kbit/s, a stereo tone).
if command -v ffmpeg > /dev/null; then
  ffmpeg -nostdin -loglevel error -y -f lavfi \
    -i "anoisesrc=d=1:c=pink:r=48000:a=0.3,pan=5.1|c0=c0|c1=0.8*c0|c2=0.5*c0|c3=0.2*c0|c4=0.6*c0|c5=0.4*c0" \
    -c:a dca -strict -2 -b:a 768k "$OUT/noise51.dts"
  ffmpeg -nostdin -loglevel error -y -f lavfi -i sine=f=440:d=1:r=48000 -ac 2 \
    -c:a dca -strict -2 "$OUT/sine2.dts"
  for n in noise51 sine2; do
    ffmpeg -nostdin -loglevel error -y -i "$OUT/$n.dts" -f f32le "$OUT/$n.f32"
  done
fi
echo "fetch-dts: corpus in $OUT"
