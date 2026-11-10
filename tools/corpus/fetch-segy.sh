#!/usr/bin/env bash
# Fetch the SEG-Y / Seismic Unix test corpus into tools/corpus/segy/ (gitignored: nothing here
# is committed) and write segyio's reading of it (the oracle) to expected.tsv beside it.
#
# Usage: tools/corpus/fetch-segy.sh
#   OUT defaults to $SPECULAR_SEGY_CORPUS, else tools/corpus/segy.
#   PYTHON (default python3) must have numpy and segyio for the oracle; a venv under target/
#   is fine: python3 -m venv target/segy/venv && target/segy/venv/bin/pip install
#   --no-cache-dir segyio numpy; then PYTHON=target/segy/venv/bin/python.
#   Then: SPECULAR_SEGY_DIR=tools/corpus/segy SPECULAR_SEGY_ORACLE=tools/corpus/segy/expected.tsv
#   cargo test -p specular-segy --test oracle corpus_matches_segyio
#
# What it covers (about 105 MB):
#   segyio/   segyio's test data at a pinned commit (LGPL-3.0): f3.sgy and f3-lsb.sgy (a cropped
#             piece of the F3 Netherlands survey, int16; F3 itself is CC BY-SA 3.0, dGB Earth
#             Sciences / TNO), small / tiny inline-sorted cubes in both byte orders, SU files
#             in both byte orders, rotated surveys, pre-stack, a shot gather, extended textual
#             header stanzas (known and unknown count), rev 2.1 field limits, delay scalars,
#             ASCII textual headers, a header with an embedded NUL, a cut file and a 60000-
#             sample declaration without trace headers
#   xtgeo/    xtgeo-testdata cubes at a pinned commit (LGPL-3.0, Equinor): dead traces,
#             missing traces (gaps), a complete 75x71x26 cube, an IBM synthetic, a 39 MB test
#             cube, and the 59 MB Reek synthetic depth stack (timing file)
# The full F3 block (about 4 GB, CC BY-SA 3.0, opendtect.org / TerraNubis) needs a login and
# is not fetched.
#
# Downloads are untrusted data: this script only copies files, never runs anything from them.
# Stops if free space on the target filesystem drops below MIN_FREE_GB (default 8).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_SEGY_CORPUS:-$ROOT/tools/corpus/segy}
MIN_FREE_GB=${MIN_FREE_GB:-8}
PYTHON=${PYTHON:-python3}
SEGYIO=ce7bf86e4bee75f3296a5a756fa123e981a04fea
XTGEO=cad17f24e22c19c6cefe6f647185395cc0a11add
mkdir -p "$OUT/segyio" "$OUT/xtgeo"
: > "$OUT/.provenance.tsv"

die() { echo "fetch-segy: $*" >&2; exit 1; }

check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
}

# get URL DEST LICENCE
get() {
  local url=$1 dest=$2 lic=$3
  check_space
  if [[ ! -s "$OUT/$dest" ]]; then
    curl -fsSL --retry 3 -o "$OUT/$dest.part" "$url" || die "download failed: $url"
    mv "$OUT/$dest.part" "$OUT/$dest"
  fi
  printf '%s\t%s\t%s\n' "$dest" "$url" "$lic" >> "$OUT/.provenance.tsv"
}

for f in f3.sgy f3-lsb.sgy small.sgy small-lsb.sgy tiny.sgy small.su small-lsb.su \
         normal-small.sgy acute-small.sgy right-small.sgy obtuse-small.sgy straight-small.sgy \
         reflex-small.sgy left-small.sgy inv-acute-small.sgy rotated-small-rev2.sgy \
         small-ps.sgy shot-gather.sgy 1x1.sgy 1xN.sgy Mx1.sgy multi-text.sgy \
         stanzas-known-count.sgy stanzas-unknown-count.sgy decrement.sgy decrement-lsb.sgy \
         increment.sgy increment-lsb.sgy delay-scalar.sgy text-embed-null.sgy broken.sgy \
         long.sgy trace-header-extension1.sgy; do
  lic="LGPL-3.0 (segyio test data)"
  [[ $f == f3* ]] && lic="CC BY-SA 3.0 (F3 Netherlands, dGB Earth Sciences / TNO; cropped by segyio)"
  get "https://raw.githubusercontent.com/equinor/segyio/$SEGYIO/test-data/$f" "segyio/$f" "$lic"
done

for f in cube_w_deadtraces.segy ex1_missing_traces_75x71x26.segy \
         ex2_complete_3d_75x71x26.segy ib_synth_iainb.segy ib_test_cube2.segy; do
  get "https://raw.githubusercontent.com/equinor/xtgeo-testdata/$XTGEO/cubes/etc/$f" \
      "xtgeo/$f" "LGPL-3.0 (xtgeo-testdata, Equinor)"
done
get "https://raw.githubusercontent.com/equinor/xtgeo-testdata/$XTGEO/cubes/reek/syntseis_20000101_seismic_depth_stack.segy" \
    "xtgeo/reek_depth_stack.segy" "LGPL-3.0 (xtgeo-testdata, Equinor; synthetic)"

# The oracle: segyio's reading, per directory, into one TSV with paths relative to OUT.
GEN="$ROOT/crates/formats/specular-segy/gen/oracle.py"
: > "$OUT/expected.tsv"
for d in segyio xtgeo; do
  "$PYTHON" -I "$GEN" "$OUT/$d" "$OUT/$d.tsv" 400 2>/dev/null
  sed -e '/^#/d' -e "s#^\([a-z]*\)\t#\1\t$d/#" "$OUT/$d.tsv" >> "$OUT/expected.tsv"
  rm -f "$OUT/$d.tsv"
done
du -sh "$OUT"
