#!/usr/bin/env bash
# Fetch the Testo .bmt and Fluke .is2 test corpus into tools/corpus/thermal2/ (gitignored:
# nothing here is committed) and write the independent BMT readings (the oracle) to
# expected.tsv beside it. The Fluke oracle is already in the corpus: each .is2 comes with
# the temperature table Fluke SmartView exported from it (.txt, °F).
#
# Usage: tools/corpus/fetch-thermal2.sh
#   OUT defaults to $SPECULAR_THERMAL2_CORPUS, else tools/corpus/thermal2.
#   PYTHON (default python3) needs only its standard library.
#   Then: SPECULAR_THERMAL2_DIR=tools/corpus/thermal2
#   cargo test -p specular-fmt-thermal --test bmt_is2 -- --nocapture
#
# What it covers (about 14 MB):
#   Compressor.is2, Compressor1.is2, WallAnomaly.is2  Fluke IS2 (ZIP layout), 320 x 240,
#       each with its SmartView export (.txt); and Air_Gap, Air_Infiltration, Breaker, Main,
#       Panel (.is2): the older layout, which must be refused. From potynobg/Thermal
#       (no licence stated: local use only).
#   Haus2.BMT  Testo 872, 320 x 240, from Testo's own sample reader testo/read_bmt (no
#       licence stated: local use only).
#
# Downloads are untrusted data: this script only copies them and checks their SHA-256.
# Stops if free space drops below MIN_FREE_GB (default 8).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_THERMAL2_CORPUS:-$ROOT/tools/corpus/thermal2}
MIN_FREE_GB=${MIN_FREE_GB:-8}
PYTHON=${PYTHON:-python3}
FLUKE=41add152d76b4eb654b265b9efba887f71878bfb
TESTO=268ef7e502a01ada73878c43b1df6c60b21cc312
RAW=https://raw.githubusercontent.com
mkdir -p "$OUT"
: > "$OUT/.provenance.tsv"

die() { echo "fetch-thermal2: $*" >&2; exit 1; }

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

F="$RAW/potynobg/Thermal/$FLUKE"
L="no licence stated (local use only)"
get "$F/Compressor.is2" Compressor.is2 "$L" d915d098b598804b3bc51ebe1997abff9e1ec162f21c9e71957e0cd14c252d83
get "$F/Compressor.txt" Compressor.txt "$L" ad999d553986d81c141948ff97a589c347fd2566d1c32ab0bfbb98c35b949361
get "$F/Compressor1.is2" Compressor1.is2 "$L" 0da65acd890c12f539d55fdf5b8022fddc602c62081f7e6545d81c9182b0fe27
get "$F/Compressor1.txt" Compressor1.txt "$L" 5ac07da46f7e2571ce35f4eecd7e487afed43851c583bb010e2e9ff6d3745df9
get "$F/WallAnomaly.is2" WallAnomaly.is2 "$L" b14d4924024b99b0c16b8604d64797543f32d6ba922d1a33c12f0480973bd240
get "$F/WallAnomaly.txt" WallAnomaly.txt "$L" cbabc883b7e563df12a76803dd89b46cd0cf110e6b403e2d1b045898265c09df
get "$F/Air_Gap.is2" Air_Gap.is2 "$L" a6255a7dfb6d406b376f94f3e648c6806bf0fdc6c364397a71fec7fba25cf843
get "$F/Air_Infiltration.is2" Air_Infiltration.is2 "$L" 5eb69cf69df31b15d20bd1370f3927e719af606194e0831b93c1c69f6bcb2a31
get "$F/Breaker.is2" Breaker.is2 "$L" 27377163bbf1acdb5f3bddc200eb71698ffcfedac5a9cc45195277dc6ec68050
get "$F/Main.is2" Main.is2 "$L" 0c32c3d816d322ee198d01b43bb376ec4a1c321b10a4940c5bb17ede8ed84ab5
get "$F/Panel.is2" Panel.is2 "$L" a7bcd749b03f75fbb7709117063dd1273f3535bcc89ca7ddf2050431bfb76d7d
get "$RAW/testo/read_bmt/$TESTO/Haus%202.BMT" Haus2.BMT "$L" \
    6447231cfc583543a87ebc9673482dbb70a03672c2182ecac6a87a060a0c7960

"$PYTHON" -I "$ROOT/crates/formats/specular-fmt-thermal/gen/bmt_oracle.py" "$OUT" "$OUT/expected.tsv"
du -sh "$OUT"
