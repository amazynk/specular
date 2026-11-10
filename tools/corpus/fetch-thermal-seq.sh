#!/usr/bin/env bash
# Fetch the FLIR sequence test corpus into tools/corpus/thermal-seq/ (gitignored: nothing here
# is committed) and write the independent readings of it (the oracle) to expected.tsv beside it.
#
# Usage: tools/corpus/fetch-thermal-seq.sh
#   OUT defaults to $SPECULAR_SEQ_CORPUS, else tools/corpus/thermal-seq.
#   PYTHON (default python3) must have numpy, Pillow and pyjpegls, and flirpy for the second
#   reading of SEQ frames; a venv under target/ is fine:
#   python3 -m venv target/s12/venv &&
#   target/s12/venv/bin/pip install --no-cache-dir numpy pillow pyjpegls flirpy; then
#   PYTHON=target/s12/venv/bin/python. exiftool reads the frames.
#   Then: SPECULAR_SEQ_DIR=tools/corpus/thermal-seq
#   SPECULAR_SEQ_ORACLE=tools/corpus/thermal-seq/expected.tsv
#   cargo test -p specular-fmt-thermal --test seq_oracle corpus_matches_the_oracles -- --nocapture
#   and SPECULAR_SEQ_DIR=tools/corpus/thermal-seq cargo test -p specular-decode
#   thermal_seq_corpus_timings -- --nocapture for index and frame timings.
#
# What it covers (about 7 MB):
#   SampleSEQ.seq   FLIR SEQ, 2 frames 640 x 480, raw counts (ResearchIR), and
#   SampleCSQ.csq   FLIR CSQ, T1030sc, 4 frames 1024 x 768, JPEG-LS counts: both from the
#                   Thermimage R package's example data (GPL-3 package; the files are only read)
#   flirpy-test.seq FLIR SEQ, 7 frames 640 x 512, big-endian FFF, from flirpy's tests (MIT)
#
# Downloads are untrusted data: this script only copies them and checks their SHA-256.
# Stops if free space drops below MIN_FREE_GB (default 8).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_SEQ_CORPUS:-$ROOT/tools/corpus/thermal-seq}
MIN_FREE_GB=${MIN_FREE_GB:-8}
PYTHON=${PYTHON:-python3}
THERMIMAGE=cb244dac8fffaac50b1ba8bbbf58b7d37b5d96e1
FLIRPY=ce0cfbb57472c6d42575db45e67a292f74ac9058
RAW=https://raw.githubusercontent.com
mkdir -p "$OUT" "$OUT/.work"
: > "$OUT/.provenance.tsv"

die() { echo "fetch-thermal-seq: $*" >&2; exit 1; }

check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
}

# get URL DEST LICENCE SHA256
get() {
  local url=$1 dest=$2 lic=$3 sha=$4
  check_space
  if [[ ! -s "$OUT/$dest" ]]; then
    curl -fsSL --retry 3 -o "$OUT/$dest.part" "$url" || die "download failed: $url"
    mv "$OUT/$dest.part" "$OUT/$dest"
  fi
  [[ $(sha256sum "$OUT/$dest" | cut -d' ' -f1) == "$sha" ]] || die "checksum mismatch: $dest"
  printf '%s\t%s\t%s\n' "$dest" "$url" "$lic" >> "$OUT/.provenance.tsv"
}

T="$RAW/gtatters/Thermimage/$THERMIMAGE/inst/extdata"
get "$T/SampleSEQ.seq" SampleSEQ.seq "GPL-3 package data (Thermimage)" \
    abbcc3070b158d630fd737c2be2a7129dba89540284173b569f7ab1876a05de9
get "$T/SampleCSQ.csq" SampleCSQ.csq "GPL-3 package data (Thermimage)" \
    7c8505a40790615bb67bc264c45366bb9e8b3e94bd74b217e4b06c6ea8146151
get "$RAW/LJMUAstroecology/flirpy/$FLIRPY/test/test_data/test.seq" flirpy-test.seq \
    "MIT (flirpy test data)" 8b191b29ab738962bdded5e23277fbcd1adea61d85d203eefa4fe42e978cb9ec

GEN="$ROOT/crates/formats/specular-fmt-thermal/gen/seq_oracle.py"
# Every 13th pixel (a prime, so the samples walk across columns).
SEQ_ORACLE_STRIDE=${SEQ_ORACLE_STRIDE:-13} "$PYTHON" -I "$GEN" "$OUT" "$OUT/expected.tsv" "$OUT/.work"
rm -rf "$OUT/.work"
du -sh "$OUT"
